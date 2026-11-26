//! Komendy Tauri (UI -> rdzeń), 1:1 z `src/lib/api.ts` i docs/kontrakt.md rozdz. 6.
//! Wszystkie `async` (komendy synchroniczne Tauri szłyby w wątku okien); sieć, dialogi i dysk
//! w `spawn_blocking`, bez blokady stanu przez `.await`.

use std::collections::HashSet;
use std::path::PathBuf;

use serde_json::Value;
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;
use walklevel_core::backup;

use crate::akcje;
use crate::konto;
use crate::okna::Okno;
use crate::stan::{Core, lock};
use crate::widoki::*;

async fn blocking<T: Send + 'static>(
    core: &Core,
    f: impl FnOnce(Core) -> CmdResult<T> + Send + 'static,
) -> CmdResult<T> {
    let core = core.clone();
    tauri::async_runtime::spawn_blocking(move || f(core))
        .await
        .map_err(|e| CmdError::unexpected(&format!("zadanie przerwane: {e}")))?
}

fn into_path(p: Option<tauri_plugin_dialog::FilePath>) -> Option<PathBuf> {
    p.and_then(|p| p.into_path().ok())
}

// --- 6.1 start, okna, ogólne ----------------------------------------------------------------

#[tauri::command]
pub async fn app_snapshot(core: State<'_, Core>) -> CmdResult<Value> {
    Ok(core.snapshot())
}

#[tauri::command]
pub async fn ui_ready(core: State<'_, Core>, window: String) -> CmdResult<()> {
    if window != "main" {
        return Ok(());
    }
    core.with(|app| {
        let geometry = app
            .settings
            .get("window_geometry")
            .and_then(Value::as_array)
            .filter(|a| a.len() == 4)
            .and_then(|a| {
                let v: Vec<i64> = a.iter().filter_map(Value::as_i64).collect();
                (v.len() == 4).then(|| [v[0], v[1], v[2], v[3]])
            });
        app.window(Okno::ShowMainAt(geometry));
        if app.b("always_on_top") {
            app.window(Okno::MainAlwaysOnTop(true));
        }
        if app.b("floating_button") {
            let pos = app.floating_pos();
            app.window(Okno::BubbleVisible(true, pos));
        }
        if app.store.is_none() {
            app.nav("wizard", "firstRun", None);
        }
    });
    core.0
        .main_visible
        .store(true, std::sync::atomic::Ordering::Relaxed);
    Ok(())
}

#[tauri::command]
pub async fn app_quit(core: State<'_, Core>) -> CmdResult<()> {
    blocking(&core, |c| {
        akcje::quit(&c);
        Ok(())
    })
    .await
}

#[tauri::command]
pub async fn window_show_main(core: State<'_, Core>) -> CmdResult<()> {
    core.with(|a| a.window(Okno::ShowMain));
    Ok(())
}

#[tauri::command]
pub async fn open_folder(core: State<'_, Core>, kind: String) -> CmdResult<()> {
    let data = lock(&core.0.app).paths.data.clone();
    let path = match kind.as_str() {
        "dane" => data.join("dane"),
        "logi" => data.join("logs"),
        "zgloszenia" => data.join("dane").join("zgloszenia"),
        "nagrania" => data.join("nagrania"),
        _ => return Err(CmdError::unexpected("Nieznany folder.")),
    };
    crate::okna::open_folder(&path);
    Ok(())
}

#[tauri::command]
pub async fn open_link(core: State<'_, Core>, kind: String) -> CmdResult<()> {
    let terms = lock(&core.0.app)
        .config
        .terms_url()
        .unwrap_or(walklevel_core::config::DEFAULT_TERMS_URL)
        .to_string();
    let url = match kind.as_str() {
        "strona" => "https://damnit.com.pl".to_string(),
        "prywatnosc" => "https://github.com/Damianejroo/WalkLevel-by-damnit.com.pl/blob/main/POLITYKA-PRYWATNOSCI.md".to_string(),
        "kod" => "https://github.com/Damianejroo/WalkLevel-by-damnit.com.pl".to_string(),
        "regulamin" => terms,
        _ => return Err(CmdError::unexpected("Nieznany adres.")),
    };
    crate::okna::shell_open(&url);
    Ok(())
}

#[tauri::command]
pub async fn log_tail(core: State<'_, Core>) -> CmdResult<Vec<String>> {
    Ok(lock(&core.0.log_tail).iter().cloned().collect())
}

#[tauri::command]
pub async fn diagnostics_run(core: State<'_, Core>) -> CmdResult<String> {
    core.with(|app| {
        if app.diagnostics.running {
            return Err(CmdError::new(
                ErrorCode::Busy,
                walklevel_core::treadmill::worker::report_busy(),
            ));
        }
        let path = walklevel_core::reports::diagnostics::report_path(&app.paths.logs());
        app.diagnostics.running = true;
        app.diagnostics.text = walklevel_core::treadmill::worker::report_busy();
        app.bieznia.worker().diagnose(path.clone());
        app.mark(&[SliceName::Device]);
        Ok(path.to_string_lossy().into_owned())
    })
}

// --- 6.2 ustawienia ---------------------------------------------------------------------------

#[tauri::command]
pub async fn settings_set(core: State<'_, Core>, key: String, value: Value) -> CmdResult<()> {
    core.with(|a| a.settings_set(&key, value))
}

#[tauri::command]
pub async fn thresholds_set(core: State<'_, Core>, which: String, kmh: f64) -> CmdResult<()> {
    core.with(|a| a.thresholds_set(&which, kmh))
}

#[tauri::command]
pub async fn pad_steps_count(core: State<'_, Core>, count: u8) -> CmdResult<()> {
    core.with(|a| a.pad_steps_count(count as usize))
}

#[tauri::command]
pub async fn pad_step_set(
    core: State<'_, Core>,
    index: u8,
    at: Option<u8>,
    kmh: Option<f64>,
) -> CmdResult<()> {
    core.with(|a| a.pad_step_set(index as usize, at.map(u32::from), kmh))
}

#[tauri::command]
pub async fn training_speed_set(core: State<'_, Core>, index: u8, kmh: f64) -> CmdResult<()> {
    core.with(|a| a.training_speed_set(index as usize, kmh))
}

#[tauri::command]
pub async fn key_capture(core: State<'_, Core>, role: Option<String>) -> CmdResult<()> {
    core.with(|a| a.key_capture(role));
    Ok(())
}

#[tauri::command]
pub async fn key_set(
    core: State<'_, Core>,
    role: String,
    key: KeyCapture,
) -> CmdResult<KeyBinding> {
    core.with(|a| a.key_set(&role, &key))
}

#[tauri::command]
pub async fn pad_capture_start(core: State<'_, Core>) -> CmdResult<()> {
    let now = crate::petla::now_s();
    core.with(|a| a.pad_capture_start(now));
    Ok(())
}

#[tauri::command]
pub async fn pad_capture_cancel(core: State<'_, Core>) -> CmdResult<()> {
    core.with(|a| {
        a.pad_capture_until = 0.0;
        a.mark(&[SliceName::Control]);
    });
    Ok(())
}

// --- 6.3 sterowanie ---------------------------------------------------------------------------

#[tauri::command]
pub async fn control_set(core: State<'_, Core>, on: bool) -> CmdResult<()> {
    core.with(|a| a.set_control(on));
    Ok(())
}

#[tauri::command]
pub async fn control_toggle(core: State<'_, Core>) -> CmdResult<()> {
    core.with(|a| {
        let on = !a.bridge.state().enabled;
        a.set_control(on);
    });
    Ok(())
}

#[tauri::command]
pub async fn training_level_set(core: State<'_, Core>, level: u8) -> CmdResult<()> {
    core.with(|a| a.set_training_level(level as usize));
    Ok(())
}

#[tauri::command]
pub async fn training_level_step(core: State<'_, Core>, step: i8) -> CmdResult<()> {
    core.with(|a| a.change_training_level(i64::from(step.signum())));
    Ok(())
}

#[tauri::command]
pub async fn training_stop(core: State<'_, Core>) -> CmdResult<()> {
    core.with(|a| a.training_stop());
    Ok(())
}

// --- 6.4 bieżnia ------------------------------------------------------------------------------

#[tauri::command]
pub async fn device_scan(core: State<'_, Core>) -> CmdResult<()> {
    core.with(|a| a.start_scan());
    Ok(())
}

#[tauri::command]
pub async fn device_connect(core: State<'_, Core>, address: String, name: String) -> CmdResult<()> {
    core.with(|a| a.connect_device(address, name));
    Ok(())
}

#[tauri::command]
pub async fn device_disconnect(core: State<'_, Core>) -> CmdResult<()> {
    core.with(|a| a.bieznia.worker().disconnect());
    Ok(())
}

#[tauri::command]
pub async fn device_forget(core: State<'_, Core>) -> CmdResult<()> {
    core.with(|a| a.device_forget());
    Ok(())
}

#[tauri::command]
pub async fn device_status_action(core: State<'_, Core>) -> CmdResult<()> {
    core.with(|a| a.status_action());
    Ok(())
}

#[tauri::command]
pub async fn models_refresh(core: State<'_, Core>) -> CmdResult<()> {
    core.with(|a| a.models_refresh());
    Ok(())
}

#[tauri::command]
pub async fn models_fetch(core: State<'_, Core>) -> CmdResult<String> {
    blocking(&core, |c| {
        let mut registry = {
            let mut app = lock(&c.0.app);
            app.models.busy = true;
            app.models.status = crate::teksty::t("modele.sprawdzam", &[]);
            app.mark(&[SliceName::Models]);
            app.registry.clone()
        };
        c.flush();
        let result = walklevel_core::treadmill::profiles::HttpFetcher::new().and_then(|f| {
            registry.download(&f, &walklevel_core::treadmill::profiles::default_base_url())
        });
        let t = crate::teksty::t;
        let status = match &result {
            Ok(r) => {
                let mut parts = Vec::new();
                if !r.added.is_empty() {
                    parts.push(t("modele.nowe", &[("lista", &r.added.join(", "))]));
                }
                if !r.updated.is_empty() {
                    parts.push(t(
                        "modele.zaktualizowane",
                        &[("lista", &r.updated.join(", "))],
                    ));
                }
                if parts.is_empty() {
                    parts.push(t("modele.aktualne", &[]));
                }
                if !r.rejected.is_empty() {
                    parts.push(t(
                        "modele.odrzucone",
                        &[("liczba", &r.rejected.len().to_string())],
                    ));
                }
                parts.join(" ")
            }
            Err(e) => t("modele.blad", &[("powod", &e.to_string())]),
        };
        c.with(|app| {
            if result.is_ok() {
                app.registry = registry;
            }
            app.models.busy = false;
            app.models.status = status.clone();
            app.models_refresh();
        });
        Ok(status)
    })
    .await
}

#[tauri::command]
pub async fn models_add_file(app: AppHandle, core: State<'_, Core>) -> CmdResult<Option<String>> {
    blocking(&core, move |c| {
        let picked = into_path(
            app.dialog()
                .file()
                .set_title(crate::teksty::t("modele.dodaj_okno", &[]))
                .add_filter(crate::teksty::t("modele.dodaj_filtr", &[]), &["json"])
                .blocking_pick_file(),
        );
        let Some(path) = picked else { return Ok(None) };
        c.with(|a| {
            let r = a
                .registry
                .install_from_file(&path)
                .map_err(|e| CmdError::invalid(e.to_string()))?;
            a.models_refresh();
            Ok(Some(r.name))
        })
    })
    .await
}

#[tauri::command]
pub async fn models_remove(core: State<'_, Core>, id: String) -> CmdResult<()> {
    core.with(|a| {
        a.registry.remove_user_profile(&id);
        a.models_refresh();
    });
    Ok(())
}

// --- 6.5 osoby i profile ----------------------------------------------------------------------

#[tauri::command]
pub async fn wizard_seed(core: State<'_, Core>, mode: String) -> CmdResult<WizardSeed> {
    let _ = mode;
    Ok(lock(&core.0.app).wizard_seed())
}

#[tauri::command]
pub async fn name_random(core: State<'_, Core>) -> CmdResult<String> {
    Ok(lock(&core.0.app).name_random())
}

#[tauri::command]
pub async fn wizard_finish(core: State<'_, Core>, result: WizardResult) -> CmdResult<()> {
    let r = core.with(|a| a.wizard_finish(result));
    konto::start(&core);
    r
}

#[tauri::command]
pub async fn person_switch(core: State<'_, Core>, person_id: String) -> CmdResult<()> {
    core.with(|a| a.person_switch(&person_id))
}

#[tauri::command]
pub async fn person_update(
    core: State<'_, Core>,
    person_id: String,
    patch: PersonPatch,
) -> CmdResult<()> {
    core.with(|a| a.person_update(&person_id, patch))
}

#[tauri::command]
pub async fn person_remove(core: State<'_, Core>, person_id: String) -> CmdResult<()> {
    let removed = core.with(|a| a.person_remove(&person_id))?;
    if removed {
        blocking(&core, move |c| {
            konto::forget_person(&c, &person_id);
            konto::start(&c);
            Ok(())
        })
        .await?;
    }
    Ok(())
}

#[tauri::command]
pub async fn profile_switch(core: State<'_, Core>, profile_id: String) -> CmdResult<()> {
    core.with(|a| a.profile_switch(&profile_id))
}

#[tauri::command]
pub async fn game_pick(
    app: AppHandle,
    core: State<'_, Core>,
    kind: String,
) -> CmdResult<Option<GamePick>> {
    blocking(&core, move |c| {
        let dialog = app.dialog().file();
        let path = if kind == "file" {
            into_path(
                dialog
                    .set_title(crate::teksty::t("profil.wskaz_plik_okno", &[]))
                    .add_filter(
                        crate::teksty::t("profil.filtr_plikow", &[])
                            .split(" / ")
                            .next()
                            .unwrap_or_default(),
                        &["exe", "lnk"],
                    )
                    .blocking_pick_file(),
            )
        } else {
            into_path(
                dialog
                    .set_title(crate::teksty::t("profil.wskaz_folder_okno", &[]))
                    .blocking_pick_folder(),
            )
        };
        let Some(path) = path else { return Ok(None) };
        c.with(|a| a.game_from(&path)).map(Some)
    })
    .await
}

#[tauri::command]
pub async fn game_pick_discard(core: State<'_, Core>, pick_id: String) -> CmdResult<()> {
    core.with(|a| a.game_pick_discard(&pick_id));
    Ok(())
}

#[tauri::command]
pub async fn profile_create(core: State<'_, Core>, draft: ProfileDraft) -> CmdResult<String> {
    core.with(|a| a.profile_create(draft))
}

#[tauri::command]
pub async fn profile_update(
    core: State<'_, Core>,
    profile_id: String,
    patch: ProfilePatch,
) -> CmdResult<()> {
    core.with(|a| a.profile_update(&profile_id, patch))
}

#[tauri::command]
pub async fn profile_duplicate(core: State<'_, Core>, profile_id: String) -> CmdResult<String> {
    core.with(|a| a.profile_duplicate(&profile_id))
}

#[tauri::command]
pub async fn profile_remove(core: State<'_, Core>, profile_id: String) -> CmdResult<()> {
    core.with(|a| a.profile_remove(&profile_id))
}

// --- 6.6 historia -----------------------------------------------------------------------------

#[tauri::command]
pub async fn history_query(core: State<'_, Core>, query: HistoryQuery) -> CmdResult<HistoryView> {
    blocking(&core, move |c| c.with(|a| a.history_query(query))).await
}

#[tauri::command]
pub async fn history_session(
    core: State<'_, Core>,
    session_id: String,
) -> CmdResult<Option<SessionDetail>> {
    core.with(|a| a.history_session(&session_id))
}

#[tauri::command]
pub async fn history_session_delete(core: State<'_, Core>, session_id: String) -> CmdResult<()> {
    if let Some(person) = core.with(|a| a.history_session_delete(&session_id))? {
        konto::delete_session_online(&core, person, session_id);
    }
    Ok(())
}

#[tauri::command]
pub async fn history_session_hide_online(
    core: State<'_, Core>,
    session_id: String,
    hidden: bool,
) -> CmdResult<()> {
    core.with(|a| -> CmdResult<()> {
        if let Some(h) = a.history.as_ref() {
            h.set_hidden_online(&session_id, hidden)?;
        }
        a.mark(&[SliceName::History]);
        Ok(())
    })?;
    let person = lock(&core.0.app).person_id();
    konto::upload_person(&core, person);
    Ok(())
}

// --- 6.7 poziomy ------------------------------------------------------------------------------

#[tauri::command]
pub async fn levels_refresh(core: State<'_, Core>) -> CmdResult<()> {
    core.with(|a| a.refresh_levels(false));
    Ok(())
}

#[tauri::command]
pub async fn levels_sample_popup(core: State<'_, Core>) -> CmdResult<()> {
    core.with(|a| a.sample_popup());
    let c = core.inner().clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(1400)).await;
        c.with(|a| a.sample_level_popup());
    });
    Ok(())
}

// --- 6.8 konto ----------------------------------------------------------------------------------

#[tauri::command]
pub async fn account_login(
    core: State<'_, Core>,
    email: String,
    password: String,
    new_person: bool,
) -> CmdResult<AccountView> {
    blocking(&core, move |c| {
        konto::login(&c, email, password, new_person)
    })
    .await
}

#[tauri::command]
pub async fn account_register(
    core: State<'_, Core>,
    code: String,
    email: String,
    password: String,
    name: String,
    new_person: bool,
) -> CmdResult<AccountView> {
    blocking(&core, move |c| {
        konto::register(&c, code, email, password, name, new_person)
    })
    .await
}

#[tauri::command]
pub async fn account_remind_password(core: State<'_, Core>, email: String) -> CmdResult<()> {
    blocking(&core, move |c| konto::remind(&c, email)).await
}

#[tauri::command]
pub async fn account_logout(core: State<'_, Core>) -> CmdResult<()> {
    blocking(&core, |c| konto::logout(&c)).await
}

#[tauri::command]
pub async fn account_delete(core: State<'_, Core>, password: String) -> CmdResult<()> {
    blocking(&core, move |c| konto::delete(&c, password)).await
}

#[tauri::command]
pub async fn account_change_password(
    core: State<'_, Core>,
    old_password: String,
    new_password: String,
) -> CmdResult<()> {
    blocking(&core, move |c| {
        konto::change_password(&c, old_password, new_password)
    })
    .await
}

#[tauri::command]
pub async fn account_send_verification(core: State<'_, Core>) -> CmdResult<String> {
    blocking(&core, |c| konto::send_verification(&c)).await
}

#[tauri::command]
pub async fn account_check_verified(core: State<'_, Core>, force: bool) -> CmdResult<bool> {
    blocking(&core, move |c| konto::check_verified(&c, force)).await
}

#[tauri::command]
pub async fn account_set_remember(core: State<'_, Core>, on: bool) -> CmdResult<()> {
    blocking(&core, move |c| {
        konto::simple(&c, "profile", None, move |a, _| a.set_remember(on))
    })
    .await
}

#[tauri::command]
pub async fn account_set_upload(core: State<'_, Core>, on: bool) -> CmdResult<()> {
    blocking(&core, move |c| {
        konto::simple(&c, "profile", None, move |a, _| a.set_upload(on))
    })
    .await
}

#[tauri::command]
pub async fn account_set_visibility(core: State<'_, Core>, value: String) -> CmdResult<()> {
    blocking(&core, move |c| konto::set_visibility(&c, value)).await
}

#[tauri::command]
pub async fn account_set_live(core: State<'_, Core>, on: bool) -> CmdResult<()> {
    blocking(&core, move |c| {
        konto::simple(&c, "live", None, move |a, _| a.set_live(on))
    })
    .await
}

#[tauri::command]
pub async fn account_set_profile_visibility(core: State<'_, Core>, value: String) -> CmdResult<()> {
    blocking(&core, move |c| {
        konto::simple(&c, "profile", Some("Zapisano."), move |a, _| {
            a.set_profile_visibility(&value)
        })
    })
    .await
}

#[tauri::command]
pub async fn account_set_games_visibility(core: State<'_, Core>, value: String) -> CmdResult<()> {
    blocking(&core, move |c| {
        konto::simple(&c, "profile", Some("Zapisano."), move |a, _| {
            a.set_games_visibility(&value)
        })
    })
    .await
}

#[tauri::command]
pub async fn account_rename(core: State<'_, Core>, name: String) -> CmdResult<()> {
    blocking(&core, move |c| {
        konto::simple(&c, "profile", Some("Zapisano."), move |a, _| {
            a.rename(&name)
        })
    })
    .await
}

#[tauri::command]
pub async fn account_accept_terms(core: State<'_, Core>) -> CmdResult<()> {
    blocking(&core, |c| konto::accept_terms(&c)).await
}

#[tauri::command]
pub async fn account_upload_now(core: State<'_, Core>) -> CmdResult<u32> {
    blocking(&core, |c| konto::upload_now(&c)).await
}

#[tauri::command]
pub async fn account_export_my_data(core: State<'_, Core>) -> CmdResult<String> {
    blocking(&core, |c| konto::export_my_data(&c)).await
}

// --- 6.9 dane online --------------------------------------------------------------------------

#[tauri::command]
pub async fn online_load(core: State<'_, Core>, tab: String, force: bool) -> CmdResult<()> {
    blocking(&core, move |c| konto::online_load(&c, &tab, force)).await
}

#[tauri::command]
pub async fn online_ranking_scope(core: State<'_, Core>, scope: String) -> CmdResult<()> {
    blocking(&core, move |c| konto::ranking_scope(&c, scope)).await
}

#[tauri::command]
pub async fn online_people_search(
    core: State<'_, Core>,
    search: String,
    page: u32,
) -> CmdResult<()> {
    blocking(&core, move |c| konto::people_search(&c, search, page)).await
}

#[tauri::command]
pub async fn online_person_open(core: State<'_, Core>, user_id: String) -> CmdResult<OnlinePerson> {
    blocking(&core, move |c| konto::person_open(&c, user_id)).await
}

#[tauri::command]
pub async fn online_invite_name(core: State<'_, Core>, name: String) -> CmdResult<String> {
    blocking(&core, move |c| konto::invite_name(&c, name)).await
}

#[tauri::command]
pub async fn online_invite_user(core: State<'_, Core>, user_id: String) -> CmdResult<String> {
    blocking(&core, move |c| konto::invite_user(&c, user_id)).await
}

#[tauri::command]
pub async fn online_invitation_answer(
    core: State<'_, Core>,
    invitation_id: String,
    accept: bool,
) -> CmdResult<()> {
    blocking(&core, move |c| {
        let toast = crate::teksty::t(
            if accept {
                "znajomi.przyjeto_zaproszenie"
            } else {
                "znajomi.odrzucono_zaproszenie"
            },
            &[],
        );
        konto::friends_action(&c, "answer", &toast, move |a| {
            a.client_mut().answer_invitation(&invitation_id, accept)
        })
    })
    .await
}

#[tauri::command]
pub async fn online_invitation_withdraw(
    core: State<'_, Core>,
    invitation_id: String,
) -> CmdResult<()> {
    blocking(&core, move |c| {
        let toast = crate::teksty::t("znajomi.wycofano", &[]);
        konto::friends_action(&c, "answer", &toast, move |a| {
            a.client_mut().withdraw_invitation(&invitation_id)
        })
    })
    .await
}

#[tauri::command]
pub async fn online_friend_remove(core: State<'_, Core>, friend_id: String) -> CmdResult<()> {
    blocking(&core, move |c| {
        let toast = crate::teksty::t("znajomi.usunieto", &[]);
        konto::friends_action(&c, "removeFriend", &toast, move |a| {
            a.client_mut().remove_friend(&friend_id)
        })
    })
    .await
}

#[tauri::command]
pub async fn online_registration_code(core: State<'_, Core>) -> CmdResult<RegistrationCode> {
    blocking(&core, |c| konto::registration_code(&c)).await
}

// --- 6.10 kopia danych ------------------------------------------------------------------------

#[tauri::command]
pub async fn backup_export_summary(
    core: State<'_, Core>,
    choice: ExportChoice,
) -> CmdResult<ExportSummary> {
    blocking(&core, move |c| {
        let app = lock(&c.0.app);
        let store = app
            .store
            .as_ref()
            .ok_or_else(|| CmdError::invalid(crate::teksty::t("kopia.blad.brak_danych", &[])))?;
        backup::summarize(
            store,
            app.history.as_ref(),
            &crate::stan::App::export_choice(&choice),
        )
        .map(|s| akcje::summary(&s))
        .map_err(akcje::backup_error)
    })
    .await
}

#[tauri::command]
pub async fn backup_export(
    app_handle: AppHandle,
    core: State<'_, Core>,
    choice: ExportChoice,
) -> CmdResult<Option<ExportSummary>> {
    blocking(&core, move |c| {
        let target = into_path(
            app_handle
                .dialog()
                .file()
                .set_title(crate::teksty::t("kopia.eksport.okno", &[]))
                .set_file_name(backup::default_name())
                .add_filter(
                    crate::teksty::t("kopia.filtr", &[])
                        .split(" (")
                        .next()
                        .unwrap_or_default(),
                    &["bieznia"],
                )
                .blocking_save_file(),
        );
        let Some(target) = target else {
            return Ok(None);
        };
        let result = {
            let app = lock(&c.0.app);
            let store = app.store.as_ref().ok_or_else(|| {
                CmdError::invalid(crate::teksty::t("kopia.blad.brak_danych", &[]))
            })?;
            backup::export(
                store,
                app.history.as_ref(),
                &crate::stan::App::export_choice(&choice),
                &target,
                &app.paths.logs(),
            )
            .map_err(akcje::backup_error)?
        };
        c.with(|a| {
            a.toast(crate::teksty::t(
                "kopia.eksport.gotowe",
                &[
                    ("liczba", &result.people.to_string()),
                    ("profile", &result.profiles.to_string()),
                    ("sesje", &result.sessions.to_string()),
                ],
            ))
        });
        Ok(Some(akcje::summary(&result)))
    })
    .await
}

#[tauri::command]
pub async fn backup_import_open(
    app_handle: AppHandle,
    core: State<'_, Core>,
) -> CmdResult<Option<ImportPreview>> {
    blocking(&core, move |c| {
        let path = into_path(
            app_handle
                .dialog()
                .file()
                .set_title(crate::teksty::t("kopia.import.okno", &[]))
                .add_filter(
                    crate::teksty::t("kopia.filtr", &[])
                        .split(" (")
                        .next()
                        .unwrap_or_default(),
                    &["bieznia"],
                )
                .blocking_pick_file(),
        );
        let Some(path) = path else { return Ok(None) };
        let preview = backup::read(&path).map_err(akcje::backup_error)?;
        let mut app = lock(&c.0.app);
        let existing: HashSet<String> = app
            .store
            .as_ref()
            .map(|s| s.people().iter().map(|p| p.id.clone()).collect())
            .unwrap_or_default();
        let view = akcje::preview_view(&preview, &existing);
        app.import_preview = Some(preview);
        Ok(Some(view))
    })
    .await
}

#[tauri::command]
pub async fn backup_import_apply(
    core: State<'_, Core>,
    choice: ImportChoice,
) -> CmdResult<ImportReport> {
    blocking(&core, move |c| {
        c.with(|app| {
            let preview = app.import_preview.take().ok_or_else(|| {
                CmdError::invalid(crate::teksty::t("kopia.blad.brak_wyboru", &[]))
            })?;
            let history_path = walklevel_core::history::History::default_path(&app.paths.data);
            let choice = backup::ImportChoice {
                people: choice.people.into_iter().collect(),
                settings: choice.settings,
                results: choice.results,
                new_person: choice.new_person.into_iter().collect(),
                keep_my_settings: choice.keep_my_settings,
            };
            let (Some(store), Some(history)) = (app.store.as_mut(), app.history.as_ref()) else {
                return Err(CmdError::new(
                    ErrorCode::Unavailable,
                    crate::teksty::t("historia.niedostepna", &[]),
                ));
            };
            let r = backup::apply_import(store, history, &preview, &choice, &history_path)
                .map_err(akcje::backup_error)?;
            app.reload_store();
            Ok(ImportReport {
                people_added: r.people_added,
                people_merged: r.people_merged,
                profiles_added: r.profiles_added,
                profiles_updated: r.profiles_updated,
                sessions_added: r.sessions_added,
                backup: r
                    .backup
                    .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned())),
            })
        })
    })
    .await
}

#[tauri::command]
pub async fn backup_restore_latest(core: State<'_, Core>) -> CmdResult<()> {
    blocking(&core, |c| {
        c.with(|app| {
            let folder = backup::latest_backup(&app.paths.data_files()).ok_or_else(|| {
                CmdError::new(
                    ErrorCode::NotFound,
                    crate::teksty::t("kopia.blad.brak_przed", &[]),
                )
            })?;
            let store_path = walklevel_core::store::Store::default_path(&app.paths.data_files());
            let history_path = walklevel_core::history::History::default_path(&app.paths.data);
            if let Some(h) = app.history.take() {
                let _ = app.tracker.finish(&h);
            } // baza zamknięta na czas przywracania
            backup::restore(&folder, &store_path, &history_path).map_err(akcje::backup_error)?;
            app.reload_store();
            app.toast(crate::teksty::t("kopia.import.przywrocono", &[]));
            Ok(())
        })
    })
    .await
}

// --- 6.11 zgłoszenia, 6.12 aktualizacje -------------------------------------------------------

#[tauri::command]
pub async fn report_send(
    core: State<'_, Core>,
    description: String,
    contact: String,
    period_s: f64,
) -> CmdResult<()> {
    blocking(&core, move |c| {
        akcje::report(&c, description, contact, period_s, false).map(|_| ())
    })
    .await
}

#[tauri::command]
pub async fn report_save(
    core: State<'_, Core>,
    description: String,
    contact: String,
    period_s: f64,
) -> CmdResult<String> {
    blocking(&core, move |c| {
        akcje::report(&c, description, contact, period_s, true)
    })
    .await
}

#[tauri::command]
pub async fn update_check(core: State<'_, Core>, install: bool) -> CmdResult<()> {
    blocking(&core, move |c| akcje::update_check(&c, install)).await
}

// --- 6.13 okna pomocnicze ---------------------------------------------------------------------

#[tauri::command]
pub async fn bubble_click(core: State<'_, Core>) -> CmdResult<()> {
    core.with(|a| {
        let on = !a.bridge.state().enabled;
        a.set_control(on);
    });
    Ok(())
}

#[tauri::command]
pub async fn bubble_open(core: State<'_, Core>) -> CmdResult<()> {
    core.with(|a| a.window(Okno::ShowMain));
    Ok(())
}

#[tauri::command]
pub async fn bubble_drag_start(core: State<'_, Core>) -> CmdResult<()> {
    lock(&core.0.app).bubble_dragging = true;
    Ok(())
}

#[tauri::command]
pub async fn bubble_drag_end(app: AppHandle, core: State<'_, Core>) -> CmdResult<()> {
    let pos = crate::okna::bubble_position(&app);
    core.with(|a| {
        a.bubble_dragging = false;
        if let Some([x, y]) = pos {
            a.change("floating_pos", serde_json::json!([x as i64, y as i64]));
        }
    });
    Ok(())
}

#[tauri::command]
pub async fn popup_ready(core: State<'_, Core>) -> CmdResult<PopupsView> {
    Ok(lock(&core.0.app).popups.view())
}
