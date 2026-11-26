//! Aplikacja WalkLevel: okno Tauri + rdzeń (crate `walklevel_core`). Tu tylko klej: komendy
//! dla UI, zdarzenia ze stanem, okna (główne, popup nad grą, pływający przycisk), pętla
//! sterowania na osobnym wątku.

mod akcje;
mod komendy;
mod konto;
mod okna;
mod osoby;
mod petla;
mod popupy;
mod poziomy;
mod stan;
mod teksty;
mod widoki;
mod zrodlo;

use std::sync::atomic::Ordering;

use serde_json::{Value, json};
use tauri::{Manager, WindowEvent};
use walklevel_core::config::{self, Config};
use walklevel_core::history::{History, SessionTracker};
use walklevel_core::input::{Key, PadReader};
use walklevel_core::levels::Definitions;
use walklevel_core::mapping::{BeltSteering, Bridge, DryOutput, MappingConfig, OutputKind};
use walklevel_core::paths::AppPaths;
use walklevel_core::store::{self, Store};
use walklevel_core::treadmill::{self, ProfileRegistry};

use crate::stan::{App, Core, lock};

/// Domyślny config.toml wbudowany w program (gdy obok exe nie ma pliku).
const EMBEDDED_CONFIG: &str = include_str!("../../config.toml");

fn load_config(paths: &AppPaths) -> Config {
    let (config, embedded) = config::load_config_or(&paths.resources, &paths.data, EMBEDDED_CONFIG);
    match embedded {
        Some(e) if walklevel_core::paths::is_installed() => {
            log::info!("config.toml z programu ({e})");
        }
        Some(e) => log::warn!("config.toml: {e} - używam wbudowanego"),
        None => {}
    }
    config
}

fn build_app(paths: AppPaths, config: Config, clock: treadmill::Clock) -> App {
    let defaults = stan::defaults_from(&config);
    let store_path = Store::default_path(&paths.data_files());
    let store = Store::load(&store_path);
    let mut legacy = None;
    let settings = match &store {
        Some(s) => stan::normalize(s.flat(&defaults)),
        None => {
            legacy = store::legacy_settings(&paths.data_files().join(store::OLD_SETTINGS));
            let mut flat = defaults.clone();
            for (k, v) in legacy.iter().flatten() {
                if flat.contains_key(k) {
                    flat.insert(k.clone(), v.clone());
                }
            }
            stan::normalize(flat)
        }
    };
    let history = History::open(&History::default_path(&paths.data))
        .map_err(|e| log::error!("Historia kroków niedostępna: {e}"))
        .ok();
    let mapping_section = config.section("mapping").cloned().unwrap_or_default();
    let mapping = MappingConfig::from_value(&Value::Object(mapping_section)).unwrap_or_default();
    let resumed = store::settings::load_resumed_enabled(
        &paths.data_files().join(store::settings::STATE_FILE),
    )
    .unwrap_or(false);
    let bridge = Bridge::new(
        mapping,
        OutputKind::Dry,
        Box::new(DryOutput::default()),
        resumed,
    );
    let registry = ProfileRegistry::load(paths.data_files().join("profile"));
    let bieznia = zrodlo::Bieznia::spawn(registry.clone(), clock);
    let accounts_enabled = config
        .account_url()
        .is_some_and(|u| walklevel_core::account::check_base_url(u).is_ok());
    let defs = Definitions::builtin().expect("wbudowane definicje poziomów");
    let mut toggle = Key::from_value(
        settings.get("toggle_key").unwrap_or(&Value::Null),
        &Key::toggle_default(),
    );
    if toggle.vk == 0 {
        toggle = Key::toggle_default(); // zapis sprzed kodów wirtualnych
    }
    let (faster, slower) = stan::speed_keys(&settings, &config);
    let now = walklevel_core::input::system::monotonic_s();
    let mut app = App {
        caps: treadmill::known_capabilities(
            &registry,
            settings
                .get("device_name")
                .and_then(Value::as_str)
                .unwrap_or(""),
            settings
                .get("simulator")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        ),
        scan_cancelled: false,
        auto_connect: settings
            .get("device_address")
            .and_then(Value::as_str)
            .unwrap_or("")
            .is_empty(),
        first_run: store.is_none(),
        paths,
        config,
        defaults,
        settings,
        store,
        legacy,
        history,
        tracker: SessionTracker::new(),
        defs,
        bridge,
        steering: BeltSteering::new(),
        toggle_key: toggle,
        faster_key: faster,
        slower_key: slower,
        walk_key: Key::walk_default(),
        run_key: Key::run_default(),
        hotkeys: None,
        pad: PadReader::system(),
        pad_state: None,
        pad_buttons: Default::default(),
        pad_capture_until: 0.0,
        key_capturing: None,
        belt_target: None,
        hint_override: None,
        pending_reading: None,
        bieznia,
        registry,
        link: treadmill::Link::Idle,
        link_text: String::new(),
        device: None,
        devices: Vec::new(),
        scanning: false,
        scan_text: String::new(),
        diagnostics: widoki::Diagnostics {
            running: false,
            text: String::new(),
        },
        recorder: None,
        levels: None,
        levels_key: None,
        levels_quiet: false,
        popups: Default::default(),
        accounts_enabled,
        konta: Default::default(),
        account_busy: Default::default(),
        account_message: String::new(),
        account_timers: walklevel_core::account::AccountTimers::new(now),
        verify_until: 0.0,
        verify_at: 0.0,
        verify_checked_at: 0.0,
        live_on: false,
        online: Default::default(),
        achievement_stats_at: 0.0,
        ranking_scope: "tydzien".into(),
        update: Default::default(),
        report: Default::default(),
        models: Default::default(),
        picks: Default::default(),
        import_preview: None,
        updated_from: None,
        window_focused: false,
        bubble_dragging: false,
        bubble_layout: None,
        bubble_moving: false,
        bubble_moving_since: None,
        status_key: String::new(),
        frame: 0,
        people_rev: 0,
        dirty: Default::default(),
        out: Vec::new(),
    };
    let (walk, run) = app.profile_keys();
    app.walk_key = walk;
    app.run_key = run;
    app.apply_mapping();
    app.apply_keys_output();
    app.make_hotkeys();
    app.set_tracker_owner();
    app.set_tracker_source();
    app.apply_recording();
    app.clean_temp_icons();
    let previous = app.s("last_version");
    let current = walklevel_core::paths::app_version().to_string();
    if previous != current {
        if !previous.is_empty() {
            log::info!("Zaktualizowano z {previous} do {current}");
            app.updated_from = Some(previous);
        }
        app.settings.insert("last_version".into(), json!(current));
        app.save();
    }
    app
}

fn icon_response(data_files: &std::path::Path, uri_path: &str) -> tauri::http::Response<Vec<u8>> {
    let name = uri_path.trim_start_matches('/');
    let name = name.split('?').next().unwrap_or("");
    match akcje::icon_file(data_files, name).and_then(|p| std::fs::read(p).ok()) {
        Some(bytes) => tauri::http::Response::builder()
            .header("Content-Type", "image/png")
            .header("Access-Control-Allow-Origin", "*")
            .body(bytes)
            .unwrap_or_default(),
        None => tauri::http::Response::builder()
            .status(404)
            .body(Vec::new())
            .unwrap_or_default(),
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let paths = AppPaths::detect();
    if let Err(e) = paths.ensure() {
        eprintln!(
            "Nie da się utworzyć folderu danych {}: {e}",
            paths.data.display()
        );
    }
    let detailed = true;
    if let Err(e) = walklevel_core::logs::init(&paths.data, cfg!(debug_assertions), detailed) {
        eprintln!("Logi: {e}");
    }
    log::info!(
        "WalkLevel {} - dane: {}",
        walklevel_core::paths::app_version(),
        paths.data.display()
    );
    let data_files = paths.data_files();
    let webview_dir = okna::webview_dir(&paths.data);

    let result = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            // druga instancja: przywraca okno pierwszej
            if let Some(core) = app.try_state::<Core>() {
                core.with(|a| a.window(okna::Okno::ShowMain));
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .register_uri_scheme_protocol("ikona", move |_ctx, request| {
            icon_response(&data_files, request.uri().path())
        })
        .setup(move |tauri_app| {
            let handle = tauri_app.handle().clone();
            let clock = treadmill::monotonic_clock();
            let config = load_config(&paths);
            let account_url = config.account_url().unwrap_or("").to_string();
            let data = paths.data.clone();
            let app = build_app(paths.clone(), config, clock.clone());
            walklevel_core::logs::set_detailed(app.b("detailed_logs"));
            let konto_ctx = konto::KontoCtx::open(&account_url, &data);
            let sink = Box::new(okna::TauriSink(handle.clone()));
            let core = Core::new(app, konto_ctx, sink, handle.clone(), clock);
            {
                let c = core.clone();
                walklevel_core::logs::set_listener(Some(Box::new(move |line: &str| {
                    lock(&c.0.log_new).push(line.to_string());
                })));
            }
            tauri_app.manage(core.clone());
            okna::create_windows(&handle, &webview_dir)?;
            core.with(|a| a.connect_source(true));
            konto::start(&core);
            petla::spawn(core.clone());
            let c = core.clone();
            std::thread::spawn(move || {
                let _ = akcje::update_check(&c, false); // tylko informacja, bez instalowania
            });
            log::info!("Okno gotowe");
            Ok(())
        })
        .on_window_event(|window, event| {
            let Some(core) = window.app_handle().try_state::<Core>() else {
                return;
            };
            let core = core.inner().clone();
            if window.label() != okna::MAIN {
                return;
            }
            match event {
                WindowEvent::CloseRequested { api, .. } => {
                    api.prevent_close();
                    std::thread::spawn(move || akcje::quit(&core));
                }
                WindowEvent::Focused(focused) => {
                    let focused = *focused;
                    core.with(|a| {
                        a.window_focused = focused;
                        a.mark(&[widoki::SliceName::App]);
                    });
                    if focused {
                        // powrót z przeglądarki po kliknięciu linku z maila
                        std::thread::spawn(move || {
                            let _ = konto::check_verified(&core, false);
                        });
                    }
                }
                WindowEvent::Resized(size) => {
                    let visible = size.width > 0 && size.height > 0;
                    core.0.main_visible.store(visible, Ordering::Relaxed);
                }
                _ => {}
            }
        })
        .invoke_handler(tauri::generate_handler![
            komendy::app_snapshot,
            komendy::ui_ready,
            komendy::app_quit,
            komendy::window_show_main,
            komendy::open_folder,
            komendy::open_link,
            komendy::log_tail,
            komendy::diagnostics_run,
            komendy::settings_set,
            komendy::thresholds_set,
            komendy::pad_steps_count,
            komendy::pad_step_set,
            komendy::training_speed_set,
            komendy::key_capture,
            komendy::key_set,
            komendy::pad_capture_start,
            komendy::pad_capture_cancel,
            komendy::control_set,
            komendy::control_toggle,
            komendy::training_level_set,
            komendy::training_level_step,
            komendy::training_stop,
            komendy::device_belt_stop,
            komendy::device_scan,
            komendy::device_connect,
            komendy::device_disconnect,
            komendy::device_forget,
            komendy::device_status_action,
            komendy::models_refresh,
            komendy::models_fetch,
            komendy::models_add_file,
            komendy::models_remove,
            komendy::wizard_seed,
            komendy::name_random,
            komendy::wizard_finish,
            komendy::person_switch,
            komendy::person_update,
            komendy::person_remove,
            komendy::profile_switch,
            komendy::game_pick,
            komendy::game_pick_discard,
            komendy::profile_create,
            komendy::profile_update,
            komendy::profile_duplicate,
            komendy::profile_remove,
            komendy::history_query,
            komendy::history_session,
            komendy::history_session_delete,
            komendy::history_session_hide_online,
            komendy::levels_refresh,
            komendy::levels_sample_popup,
            komendy::account_login,
            komendy::account_register,
            komendy::account_remind_password,
            komendy::account_logout,
            komendy::account_delete,
            komendy::account_change_password,
            komendy::account_send_verification,
            komendy::account_check_verified,
            komendy::account_set_remember,
            komendy::account_set_upload,
            komendy::account_set_visibility,
            komendy::account_set_live,
            komendy::account_set_profile_visibility,
            komendy::account_set_games_visibility,
            komendy::account_rename,
            komendy::account_accept_terms,
            komendy::account_upload_now,
            komendy::account_export_my_data,
            komendy::online_load,
            komendy::online_ranking_scope,
            komendy::online_people_search,
            komendy::online_person_open,
            komendy::online_invite_name,
            komendy::online_invite_user,
            komendy::online_invitation_answer,
            komendy::online_invitation_withdraw,
            komendy::online_friend_remove,
            komendy::online_registration_code,
            komendy::backup_export_summary,
            komendy::backup_export,
            komendy::backup_import_open,
            komendy::backup_import_apply,
            komendy::backup_restore_latest,
            komendy::report_send,
            komendy::report_save,
            komendy::update_check,
            komendy::bubble_click,
            komendy::bubble_open,
            komendy::bubble_drag_start,
            komendy::bubble_drag_end,
            komendy::popup_ready,
        ])
        .run(tauri::generate_context!());
    if let Err(e) = result {
        log::error!("Nie udało się uruchomić okna WalkLevel: {e}");
    }
}
