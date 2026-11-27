//! Aplikacja WalkLevel: okno Tauri + rdzeń (crate `walklevel_core`). Tu tylko klej: komendy
//! dla UI, zdarzenia ze stanem, okna (główne, popup nad grą, pływający przycisk), pętla
//! sterowania na osobnym wątku.

mod account;
mod actions;
#[cfg(test)]
mod app_tests;
mod commands;
mod control_loop;
mod levels;
mod people;
mod popups;
mod state;
mod texts;
mod treadmill_source;
mod views;
mod window;

use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

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

use crate::state::{App, Core, lock};

/// Domyślny config.toml wbudowany w program (gdy obok exe nie ma pliku).
const EMBEDDED_CONFIG: &str = include_str!("../../config.toml");

fn load_config(paths: &AppPaths) -> Config {
    let (config, embedded) = config::load_config_or(&paths.resources, &paths.data, EMBEDDED_CONFIG);
    match embedded {
        Some(e) if walklevel_core::paths::is_installed() => {
            log::info!(
                "config.toml z programu ({})",
                walklevel_core::logs::redact(&e.to_string())
            );
        }
        Some(e) => log::warn!(
            "config.toml: {} - używam wbudowanego",
            walklevel_core::logs::redact(&e.to_string())
        ),
        None => {}
    }
    config
}

fn build_app(
    paths: AppPaths,
    config: Config,
    clock: treadmill::Clock,
    store: Option<Store>,
) -> App {
    let defaults = state::defaults_from(&config);
    let mut legacy = None;
    let settings = match &store {
        Some(s) => state::normalize(s.flat(&defaults)),
        None => {
            legacy = store::legacy_settings(&paths.data_files().join(store::OLD_SETTINGS));
            let mut flat = defaults.clone();
            for (k, v) in legacy.iter().flatten() {
                if flat.contains_key(k) {
                    flat.insert(k.clone(), v.clone());
                }
            }
            state::normalize(flat)
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
    let bieznia = treadmill_source::Bieznia::spawn(registry.clone(), clock);
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
    let (faster, slower) = state::speed_keys(&settings, &config);
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
        diagnostics: views::Diagnostics {
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
        history_rev: 0,
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
            log::info!("[update-from] Zaktualizowano z {previous} do {current}");
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
    match actions::icon_file(data_files, name).and_then(|p| std::fs::read(p).ok()) {
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

static STARTED: OnceLock<Instant> = OnceLock::new();

/// Seconds since the process started.
fn uptime_s() -> f64 {
    STARTED.get_or_init(Instant::now).elapsed().as_secs_f64()
}

/// Version, build, Windows, WebView2 and which data folder - without the path itself (it
/// holds the Windows user name).
fn log_start() {
    STARTED.get_or_init(Instant::now);
    let build = if cfg!(debug_assertions) {
        "dev"
    } else if walklevel_core::paths::is_installed() {
        "instalacja"
    } else {
        "exe"
    };
    let data = if std::env::var_os(walklevel_core::paths::DATA_ENV).is_some() {
        walklevel_core::paths::DATA_ENV
    } else {
        "domyślne"
    };
    log::info!(
        "{} WalkLevel {} ({build}), {}, WebView2 {}, dane: {data}",
        walklevel_core::logs::APP_START_EVENT,
        walklevel_core::paths::app_version(),
        window::windows_version(),
        tauri::webview_version().unwrap_or_else(|e| format!("niedostępny ({e})"))
    );
}

/// Time from the start of the process to the main window on screen (once per run).
pub(crate) fn log_window_shown() {
    static SHOWN: AtomicBool = AtomicBool::new(false);
    if !SHOWN.swap(true, Ordering::Relaxed) {
        log::info!("[app-ready] Okno pokazane po {:.1} s od startu", uptime_s());
    }
}

/// The last entry of a normal exit; without it the next start reports a crash.
pub(crate) fn log_exit(reason: &str) {
    log::info!(
        "{} Zamknięcie aplikacji: {reason}, po {:.0} min",
        walklevel_core::logs::APP_EXIT_EVENT,
        uptime_s() / 60.0
    );
}

/// `[logs] level = "debug"` in config.toml (or config.local.toml) - DEBUG also in bieznia.log.
fn apply_log_level(config: &Config) {
    let Some(text) = config.str_value("logs", "level") else {
        return;
    };
    match walklevel_core::logs::parse_level(text) {
        Some(level) => {
            walklevel_core::logs::set_file_level(level);
            if level != walklevel_core::logs::DEFAULT_FILE_LEVEL {
                log::info!("Poziom logu aplikacji: {text}");
            }
        }
        None => log::warn!("config.toml: nieznany poziom logu „{text}” - zostaje info"),
    }
}

/// osoby.json cannot be read: an error instead of the first-run wizard, which would
/// overwrite the file.
fn unreadable_store(handle: &tauri::AppHandle, e: &store::LoadError) {
    use tauri_plugin_dialog::{DialogExt, MessageDialogKind};
    log::error!(
        "osoby.json: {} - zamykam bez nadpisywania",
        walklevel_core::logs::redact(&e.to_string())
    );
    let text = match &e.copy {
        Some(copy) => texts::t(
            "osoby.nieczytelne",
            &[("powod", &e.reason), ("kopia", &copy.display().to_string())],
        ),
        None => texts::t("osoby.nieczytelne_bez_kopii", &[("powod", &e.reason)]),
    };
    let exit = handle.clone();
    handle
        .dialog()
        .message(text)
        .title(texts::t("app.nazwa", &[]))
        .kind(MessageDialogKind::Error)
        .show(move |_| {
            log_exit("nieczytelny osoby.json");
            exit.exit(1);
        });
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
    let previous_closed = walklevel_core::logs::previous_run_closed(&paths.logs());
    if let Err(e) = walklevel_core::logs::init(&paths.data, cfg!(debug_assertions), detailed) {
        eprintln!("Logi: {e}");
    }
    log_start();
    if previous_closed == Some(false) {
        log::warn!(
            "[app-exit-missing] Poprzednie uruchomienie nie zapisało zamknięcia (awaria, zabicie procesu albo wyłączenie komputera)"
        );
    }
    std::thread::spawn(|| {
        log::info!(
            "[app-bluetooth] Adapter Bluetooth: {}",
            treadmill::ble::adapter_summary()
        );
    });
    let data_files = paths.data_files();
    let webview_dir = window::webview_dir(&paths.data);

    let result = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            // druga instancja: przywraca okno pierwszej
            if let Some(core) = app.try_state::<Core>() {
                core.with(|a| a.window(window::Okno::ShowMain));
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
            apply_log_level(&config);
            let account_url = config.account_url().unwrap_or("").to_string();
            let data = paths.data.clone();
            let store = match Store::open(&Store::default_path(&paths.data_files())) {
                Ok(store) => store,
                Err(e) => {
                    unreadable_store(&handle, &e);
                    return Ok(());
                }
            };
            let app = build_app(paths.clone(), config, clock.clone(), store);
            walklevel_core::logs::set_detailed(app.b("detailed_logs"));
            let konto_ctx = account::KontoCtx::open(&account_url, &data);
            let sink = Box::new(window::TauriSink(handle.clone()));
            let core = Core::new(app, konto_ctx, sink, handle.clone(), clock);
            {
                let c = core.clone();
                walklevel_core::logs::set_listener(Some(Box::new(move |line: &str| {
                    lock(&c.0.log_new).push(line.to_string());
                })));
            }
            tauri_app.manage(core.clone());
            window::create_windows(&handle, &webview_dir)?;
            core.with(|a| a.connect_source(true));
            account::start(&core);
            control_loop::spawn(core.clone());
            let c = core.clone();
            std::thread::spawn(move || {
                let _ = actions::update_check(&c, false); // tylko informacja, bez instalowania
            });
            log::info!("Okno gotowe po {:.1} s", uptime_s());
            Ok(())
        })
        .on_window_event(|window, event| {
            let Some(core) = window.app_handle().try_state::<Core>() else {
                return;
            };
            let core = core.inner().clone();
            if window.label() != window::MAIN {
                return;
            }
            match event {
                WindowEvent::CloseRequested { api, .. } => {
                    api.prevent_close();
                    std::thread::spawn(move || actions::quit(&core, "zamknięcie okna"));
                }
                WindowEvent::Focused(focused) => {
                    let focused = *focused;
                    core.with(|a| {
                        a.window_focused = focused;
                        a.mark(&[views::SliceName::App]);
                    });
                    if focused {
                        // powrót z przeglądarki po kliknięciu linku z maila
                        std::thread::spawn(move || {
                            let _ = account::check_verified(&core, false);
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
            commands::app_snapshot,
            commands::ui_ready,
            commands::app_quit,
            commands::window_show_main,
            commands::open_folder,
            commands::open_link,
            commands::log_tail,
            commands::diagnostics_run,
            commands::settings_set,
            commands::thresholds_set,
            commands::pad_steps_count,
            commands::pad_step_set,
            commands::training_speed_set,
            commands::key_capture,
            commands::key_set,
            commands::pad_capture_start,
            commands::pad_capture_cancel,
            commands::control_set,
            commands::control_toggle,
            commands::training_level_set,
            commands::training_level_step,
            commands::training_stop,
            commands::device_belt_stop,
            commands::device_scan,
            commands::device_connect,
            commands::device_disconnect,
            commands::device_forget,
            commands::device_status_action,
            commands::models_refresh,
            commands::models_fetch,
            commands::models_add_file,
            commands::models_remove,
            commands::wizard_seed,
            commands::name_random,
            commands::wizard_finish,
            commands::person_switch,
            commands::person_update,
            commands::person_remove,
            commands::profile_switch,
            commands::game_pick,
            commands::game_pick_discard,
            commands::profile_create,
            commands::profile_update,
            commands::profile_duplicate,
            commands::profile_remove,
            commands::history_query,
            commands::history_session,
            commands::history_session_delete,
            commands::history_session_hide_online,
            commands::levels_refresh,
            commands::levels_sample_popup,
            commands::account_login,
            commands::account_register,
            commands::account_remind_password,
            commands::account_logout,
            commands::account_delete,
            commands::account_change_password,
            commands::account_send_verification,
            commands::account_check_verified,
            commands::account_set_remember,
            commands::account_set_upload,
            commands::account_set_visibility,
            commands::account_set_live_status,
            commands::window_theme,
            commands::account_set_description,
            commands::account_set_profile_visibility,
            commands::account_set_games_visibility,
            commands::account_rename,
            commands::account_accept_terms,
            commands::account_upload_now,
            commands::account_export_my_data,
            commands::online_load,
            commands::online_ranking_scope,
            commands::online_people_search,
            commands::online_person_open,
            commands::online_invite_name,
            commands::online_invite_user,
            commands::online_invitation_answer,
            commands::online_invitation_withdraw,
            commands::online_friend_remove,
            commands::online_registration_code,
            commands::backup_export_summary,
            commands::backup_export,
            commands::backup_import_open,
            commands::backup_import_apply,
            commands::backup_restore_latest,
            commands::report_send,
            commands::report_save,
            commands::update_check,
            commands::bubble_click,
            commands::bubble_open,
            commands::bubble_drag_start,
            commands::bubble_drag_end,
            commands::popup_ready,
            commands::log_ui_error,
        ])
        .run(tauri::generate_context!());
    if let Err(e) = result {
        log::error!("Nie udało się uruchomić okna WalkLevel: {e}");
    }
}
