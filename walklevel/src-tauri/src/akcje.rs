//! Akcje komend na stanie aplikacji: ustawienia, klawisze, pad, historia, profile modeli
//! bieżni, kopia danych, zgłoszenia, aktualizacje, zamykanie.

use std::collections::{BTreeMap, HashSet};
use std::path::Path;
use std::time::Duration;

use serde_json::{Value, json};
use walklevel_core::backup;
use walklevel_core::history::{GroupKey, HistoryRange, now_unix};
use walklevel_core::input;
use walklevel_core::mapping::round_tenth;
use walklevel_core::store::palette;
use walklevel_core::treadmill;

use crate::okna::Okno;
use crate::stan::{App, Core, lock};
use crate::widoki::*;

const HYSTERESIS: f64 = 0.6;

impl App {
    /// `settings_set` - walidacja jak `normalize()` i skutki uboczne (kontrakt 6.2).
    pub fn settings_set(&mut self, key: &str, value: Value) -> CmdResult<()> {
        let bad = || CmdError::unexpected("Nieprawidłowa wartość ustawienia.");
        let file_key = match key {
            "beltControl" => "belt_control",
            "quickStop" => "quick_stop",
            "togglePad" => "toggle_pad",
            "simulator" => "simulator",
            "record" => "record",
            "detailedLogs" => "detailed_logs",
            "theme" => "theme",
            "largeText" => "large_text",
            "alwaysOnTop" => "always_on_top",
            "floatingButton" => "floating_button",
            "showPersonSwitch" => "show_person_switch",
            "showProfileSwitch" => "show_profile_switch",
            "achievementPopups" => "achievement_popups",
            "achievementSound" => "achievement_sound",
            "direction" => "direction",
            "output" => "output",
            "idleSpeed" => "idle_speed",
            "walkSpeed" => "walk_speed",
            "runSpeed" => "run_speed",
            "padIdle" => "pad_idle",
            _ => return Err(CmdError::unexpected("Tego ustawienia nie da się zmienić.")),
        };
        let allowed: Option<&[&str]> = match file_key {
            "theme" => Some(&["system", "light", "dark"]),
            "direction" => Some(&["keyboard", "gamepad", "treadmill", "training"]),
            "output" => Some(&["dry", "keys"]),
            "pad_idle" => Some(&["stop", "slow", "zero"]),
            _ => None,
        };
        let value = match (file_key, &value) {
            ("toggle_pad", Value::String(_)) => value,
            (_, Value::String(s)) if allowed.is_some_and(|a| a.contains(&s.as_str())) => value,
            ("idle_speed" | "walk_speed" | "run_speed", Value::Number(n)) => {
                let kmh = n.as_f64().ok_or_else(bad)?;
                if !(0.0..=20.0).contains(&kmh) {
                    return Err(bad());
                }
                json!(round_tenth(kmh))
            }
            (_, Value::Bool(_)) if allowed.is_none() && !file_key.ends_with("_speed") => value,
            _ => return Err(bad()),
        };
        if self.settings.get(file_key) == Some(&value) {
            return Ok(());
        }
        let on = value.as_bool().unwrap_or(false);
        match file_key {
            "simulator" => {
                return {
                    self.on_simulator(on);
                    Ok(())
                };
            }
            "toggle_pad" => {
                let button = value.as_str().unwrap_or("").to_string();
                self.set_toggle_pad(button);
                return Ok(());
            }
            _ => {}
        }
        self.change(file_key, value.clone());
        match file_key {
            "always_on_top" => self.window(Okno::MainAlwaysOnTop(on)),
            "floating_button" => {
                let pos = self.floating_pos();
                self.window(Okno::BubbleVisible(on, pos));
            }
            "record" => self.apply_recording(),
            "detailed_logs" => {
                walklevel_core::logs::set_detailed(on);
                log::info!(
                    "Szczegółowe logi Bluetooth {}",
                    if on { "włączone" } else { "wyłączone" }
                );
            }
            "direction" => {
                log::info!("Kierunek sterowania: {}", self.s("direction"));
                self.apply_direction();
            }
            "output" => self.apply_keys_output(),
            "quick_stop" => self.apply_mapping(),
            "idle_speed" | "walk_speed" | "run_speed" | "pad_idle" => {
                self.belt_target = None;
                self.steering.forget_target();
            }
            "achievement_sound" if on => crate::okna::play_sound(false),
            _ => {}
        }
        Ok(())
    }

    /// Progi chodu i biegu: zmieniony wygrywa, drugi przesuwa się o odstęp (on_threshold).
    pub fn thresholds_set(&mut self, which: &str, value: f64) -> CmdResult<()> {
        let top = self.max_speed();
        let gap = HYSTERESIS + 0.2;
        let (mut walk, mut run) = (self.f("stand_below"), self.f("run_on"));
        match which {
            "walk" => {
                walk = value;
                if run < walk + gap {
                    run = top.min(walk + gap);
                    walk = walk.min(run - gap);
                }
            }
            "run" => {
                run = value;
                if run < walk + gap {
                    walk = (run - gap).max(0.0);
                    run = run.max(walk + gap);
                }
            }
            _ => return Err(CmdError::unexpected("Nieznany próg.")),
        }
        let (walk, run) = (round_tenth(walk), round_tenth(run));
        self.settings.insert("stand_below".into(), json!(walk));
        self.settings.insert("run_on".into(), json!(run));
        self.settings
            .insert("run_off".into(), json!(round_tenth(run - HYSTERESIS)));
        self.apply_mapping();
        self.save();
        self.mark(&[SliceName::Settings, SliceName::Control]);
        Ok(())
    }

    fn pad_steps(&self) -> Vec<(u32, f64)> {
        walklevel_core::mapping::parse_pad_steps(
            self.settings.get("pad_steps").unwrap_or(&Value::Null),
        )
    }

    pub fn pad_steps_count(&mut self, count: usize) -> CmdResult<()> {
        let steps = walklevel_core::mapping::pad_preset(count).ok_or_else(|| {
            CmdError::invalid(crate::teksty::t("ustawienia.pad.blad.liczba", &[]))
        })?;
        if count != self.pad_steps().len() {
            self.change("pad_steps", json!(steps));
            self.belt_target = None;
            self.steering.forget_target();
        }
        Ok(())
    }

    pub fn pad_step_set(
        &mut self,
        index: usize,
        at: Option<u32>,
        kmh: Option<f64>,
    ) -> CmdResult<()> {
        let mut steps = self.pad_steps();
        let Some(step) = steps.get_mut(index) else {
            return Err(CmdError::invalid(crate::teksty::t(
                "ustawienia.pad.blad.brak",
                &[],
            )));
        };
        if let Some(at) = at {
            step.0 = at;
        }
        if let Some(kmh) = kmh {
            step.1 = round_tenth(kmh);
        }
        let candidate = json!(steps);
        if walklevel_core::mapping::parse_pad_steps(&candidate) != steps {
            return Err(CmdError::invalid(crate::teksty::t(
                "ustawienia.pad.blad.progi",
                &[],
            )));
        }
        self.change("pad_steps", candidate);
        if kmh.is_some() {
            self.belt_target = None;
            self.steering.forget_target();
        }
        Ok(())
    }

    pub fn training_speed_set(&mut self, index: usize, kmh: f64) -> CmdResult<()> {
        if index > 3 || !(0.5..=20.0).contains(&kmh) {
            return Err(CmdError::invalid(crate::teksty::t(
                "ustawienia.trening.blad_predkosc",
                &[],
            )));
        }
        let mut levels = walklevel_core::mapping::parse_training_levels(
            self.settings.get("training_levels").unwrap_or(&Value::Null),
        );
        levels[index] = round_tenth(kmh);
        self.change("training_levels", json!(levels));
        self.belt_target = None;
        self.steering.forget_target(); // nowa prędkość poziomu od razu na taśmę
        Ok(())
    }

    pub fn key_capture(&mut self, role: Option<String>) {
        self.key_capturing = match role.as_deref() {
            Some("walk") => Some("walk"),
            Some("run") => Some("run"),
            Some("toggle") => Some("toggle"),
            Some("faster") => Some("faster"),
            Some("slower") => Some("slower"),
            _ => None,
        };
        self.mark(&[SliceName::Control]);
    }

    pub fn key_set(&mut self, role: &str, cap: &KeyCapture) -> CmdResult<KeyBinding> {
        self.key_capturing = None;
        self.mark(&[SliceName::Control]);
        let Some(mut key) = input::key_from_code(&cap.code, &cap.key) else {
            return Err(CmdError::new(
                ErrorCode::Key,
                crate::teksty::t("klawisz.blad.nieznany", &[]),
            ));
        };
        // skróty globalne (włączanie, szybciej, wolniej) mogą być kombinacjami
        if matches!(role, "toggle" | "faster" | "slower") {
            key.mods = input::keys::mods_from_flags(cap.ctrl, cap.alt, cap.shift, cap.meta);
            if input::keys::is_modifier_code(&cap.code) {
                key.mods.clear();
            }
        }
        let (mut walk, mut run, mut toggle) = (
            self.walk_key.clone(),
            self.run_key.clone(),
            self.toggle_key.clone(),
        );
        let (mut faster, mut slower) = (self.faster_key.clone(), self.slower_key.clone());
        match role {
            "walk" => walk = key.clone(),
            "run" => run = key.clone(),
            "toggle" => toggle = key.clone(),
            "faster" => faster = key.clone(),
            "slower" => slower = key.clone(),
            _ => return Err(CmdError::unexpected("Nieznany klawisz.")),
        }
        let problem = if matches!(role, "faster" | "slower") {
            input::speed_problem(&faster, &slower, &walk, &run, &toggle)
        } else {
            input::problem_with(&walk, &run, &toggle, &faster, &slower)
        };
        if let Some(problem) = problem {
            self.hint_override = Some(problem.clone());
            return Err(CmdError::new(ErrorCode::Key, problem));
        }
        self.settings.insert("walk_key".into(), walk.to_value());
        self.settings.insert("run_key".into(), run.to_value());
        let hotkeys_changed =
            toggle != self.toggle_key || faster != self.faster_key || slower != self.slower_key;
        self.walk_key = walk;
        self.run_key = run;
        if hotkeys_changed {
            self.settings.insert("toggle_key".into(), toggle.to_value());
            if faster != self.faster_key || slower != self.slower_key {
                self.settings.insert("faster_key".into(), faster.to_value());
                self.settings.insert("slower_key".into(), slower.to_value());
                log::info!(
                    "Klawisze szybciej/wolniej: {} / {}",
                    faster.label(),
                    slower.label()
                );
            }
            self.toggle_key = toggle;
            self.faster_key = faster;
            self.slower_key = slower;
            self.make_hotkeys(); // trzymany właśnie klawisz się nie liczy
        }
        self.apply_keys_output();
        self.hint_override = None;
        self.save();
        // karta symulatora (Device) i podpowiedzi pokazują nazwy klawiszy szybciej/wolniej
        self.mark(&[
            SliceName::Settings,
            SliceName::Control,
            SliceName::Device,
            SliceName::Bubble,
        ]);
        Ok(Self::key_binding(&key))
    }

    pub fn pad_capture_start(&mut self, now: f64) {
        self.pad_capture_until = now + crate::petla::PAD_CAPTURE_S;
        self.pad_buttons = self
            .pad_state
            .as_ref()
            .map(|p| p.buttons.clone())
            .unwrap_or_default();
        self.mark(&[SliceName::Control]);
    }

    // --- bieżnia ---------------------------------------------------------------------------

    pub fn device_forget(&mut self) {
        self.bieznia.worker().disconnect();
        self.settings.insert("device_address".into(), json!(""));
        self.settings.insert("device_name".into(), json!(""));
        self.save();
        self.auto_connect = true;
        self.device = None;
        log::info!("Bieżnia zapomniana");
        self.mark(&[SliceName::Device, SliceName::Settings]);
    }

    pub fn status_action(&mut self) {
        let remembered = !self.s("device_address").is_empty();
        use treadmill::Link as L;
        if self.b("simulator") {
            self.on_simulator(false);
        } else if matches!(
            self.link,
            L::Streaming | L::Connected | L::Connecting | L::Retrying
        ) || (self.link == L::Searching && remembered)
        {
            self.bieznia.worker().disconnect();
        } else if self.link == L::Searching {
            // „Anuluj” przy skanowaniu: wynik skanu nie otworzy listy ani nie połączy
            self.scan_cancelled = true;
            self.auto_connect = false;
            self.scanning = false;
            self.bieznia.worker().disconnect();
            self.mark(&[SliceName::Device]);
        } else if remembered && matches!(self.link, L::BluetoothOff | L::Idle) {
            self.bieznia
                .worker()
                .connect(self.s("device_address"), self.s("device_name"));
        } else {
            self.start_scan();
            self.nav("devices", "scan", None);
        }
    }

    pub fn models_refresh(&mut self) {
        let name = self.s("device_name");
        let mine = self.registry.profile_for(&name, None).map(|p| p.id.clone());
        self.models.items = self
            .registry
            .all()
            .iter()
            .map(|p| ModelItem {
                id: p.id.clone(),
                name: p.name.clone(),
                driver: p.driver.as_str().into(),
                driver_name: match p.driver {
                    treadmill::DriverKind::KingSmith => "KingSmith".into(),
                    treadmill::DriverKind::Ftms => crate::teksty::t("modele.rodzaj.ftms", &[]),
                    treadmill::DriverKind::Simulator => "Symulator".into(),
                },
                r#match: p.match_prefixes.clone(),
                caps: p.apply(&treadmill::default_capabilities(p.driver)),
                generic: p.match_prefixes.iter().all(|m| m.chars().count() <= 4),
                mine: !name.is_empty() && mine.as_deref() == Some(p.id.as_str()),
                source: if p.source == treadmill::profiles::SOURCE_BUILTIN {
                    "wbudowany".into()
                } else {
                    "dodatkowy".into()
                },
                removable: p.source != treadmill::profiles::SOURCE_BUILTIN,
            })
            .collect();
        self.mark(&[SliceName::Models]);
    }

    // --- historia --------------------------------------------------------------------------

    fn person_colors(&self, key: GroupKey) -> BTreeMap<String, String> {
        let mut out = BTreeMap::new();
        let Some(store) = self.store.as_ref() else {
            return out;
        };
        match key {
            GroupKey::Person => {
                for p in store.people() {
                    out.insert(
                        p.name.clone(),
                        palette::color(Some(&p.color)).swatch().into(),
                    );
                }
            }
            GroupKey::Profile => {
                for p in store.person().profiles() {
                    out.insert(
                        p.name.clone(),
                        palette::color(Some(&p.color)).swatch().into(),
                    );
                }
            }
        }
        out
    }

    fn find_profile(&self, profile_id: &str) -> Option<&walklevel_core::store::Profile> {
        self.store
            .as_ref()?
            .people()
            .iter()
            .flat_map(|p| p.profiles())
            .find(|p| p.id == profile_id)
    }

    fn session_icon(&self, profile_id: &str) -> IconRef {
        match self.find_profile(profile_id) {
            Some(p) => self.icon_ref(p),
            None => removed_profile_icon(),
        }
    }

    fn session_row(
        &self,
        s: &walklevel_core::model::Session,
        hidden: &HashSet<String>,
        rejected: &std::collections::HashMap<String, String>,
    ) -> SessionRow {
        let linked = self
            .konta
            .get(&s.person)
            .is_some_and(|k| k.state.logged_in());
        let online = (linked && s.source == "bieznia").then(|| OnlineMark {
            hidden: hidden.contains(&s.id),
            sent: self
                .history
                .as_ref()
                .and_then(|h| h.was_sent(&s.id).ok())
                .unwrap_or(false),
            rejected: rejected.get(&s.id).cloned(),
        });
        SessionRow {
            id: s.id.clone(),
            start: s.start,
            end: s.end,
            person_id: s.person.clone(),
            person_name: s.person_name.clone(),
            profile_id: s.profile.clone(),
            profile_name: s.profile_name.clone(),
            steps: s.steps,
            distance_km: s.distance_km,
            active_s: s.active_s,
            avg_kmh: s.avg_kmh(),
            source: s.source.clone(),
            mode: s.mode.clone(),
            icon: self.session_icon(&s.profile),
            online,
        }
    }

    pub fn history_query(&mut self, q: HistoryQuery) -> CmdResult<HistoryView> {
        if let Some(h) = self.history.as_ref() {
            let _ = self.tracker.flush(h);
        }
        let stamp = self.history_stamp().stamp;
        let Some(h) = self.history.as_ref() else {
            return Err(CmdError::new(
                ErrorCode::Unavailable,
                crate::teksty::t("historia.niedostepna", &[]),
            ));
        };
        let many = self.store.as_ref().is_some_and(|s| s.people().len() > 1);
        let everyone = many && q.scope == "all";
        let me = self.person_id();
        let person = (!everyone).then_some(me.as_str());
        let range = match q.range.as_str() {
            "day" => HistoryRange::Day,
            "week" => HistoryRange::Week,
            "month" => HistoryRange::Month,
            "all" => HistoryRange::All,
            _ => return Err(CmdError::unexpected("Nieznany zakres.")),
        };
        let now = now_unix();
        let since = range.since(now)?;
        let totals = h.totals(since, person)?;
        let key = if everyone {
            GroupKey::Person
        } else {
            GroupKey::Profile
        };
        let colors = self.person_colors(key);
        let hourly = range.hourly();
        let series = if hourly {
            h.by_hour(person, key, now)?
        } else {
            h.by_day(range.chart_days(), person, key, now)?
        };
        let mut names: Vec<String> = series
            .iter()
            .flat_map(|(_, p)| p.keys().cloned())
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();
        let empty = series.iter().all(|(_, p)| p.is_empty());
        names.truncate(6);
        let mut chart_colors = BTreeMap::new();
        let bars = series
            .iter()
            .map(|(label, parts)| ChartBar {
                weekday: (!hourly)
                    .then(|| {
                        chrono::NaiveDate::parse_from_str(label, "%Y-%m-%d")
                            .ok()
                            .map(|d| {
                                use chrono::Datelike;
                                d.weekday().num_days_from_monday() as u8
                            })
                    })
                    .flatten(),
                label: label.clone(),
                parts: parts
                    .iter()
                    .map(|(name, steps)| {
                        let c = colors
                            .get(name)
                            .cloned()
                            .unwrap_or_else(|| palette::default_color().swatch().into());
                        chart_colors.insert(name.clone(), c);
                        ChartPart {
                            name: name.clone(),
                            steps: *steps,
                        }
                    })
                    .collect(),
            })
            .collect();
        let groups_raw = h.by_group(since, person, key)?;
        let total_steps: i64 = groups_raw.iter().map(|(_, t)| t.steps).sum::<i64>().max(1);
        let store = self.store.as_ref();
        let groups = groups_raw
            .iter()
            .map(|(name, t)| {
                let color = colors
                    .get(name)
                    .cloned()
                    .unwrap_or_else(|| palette::default_color().swatch().into());
                // profil albo osoba usunięte (albo przemianowane) - ikona zastępcza, żeby wiersz
                // miał zawsze ikonę i nazwę (bez niej układ wiersza się rozsypywał)
                let (icon, avatar) = match key {
                    GroupKey::Profile => (
                        Some(
                            store
                                .and_then(|s| {
                                    s.person().profiles().iter().find(|p| &p.name == name)
                                })
                                .map_or_else(removed_profile_icon, |p| self.icon_ref(p)),
                        ),
                        None,
                    ),
                    GroupKey::Person => (
                        None,
                        Some(
                            store
                                .and_then(|s| s.people().iter().find(|p| &p.name == name))
                                .map_or_else(|| palette::DEFAULT.into(), |p| p.color.clone()),
                        ),
                    ),
                };
                HistoryGroup {
                    name: name.clone(),
                    totals: t.into(),
                    share: t.steps as f64 / total_steps as f64,
                    color,
                    icon,
                    avatar_color: avatar,
                }
            })
            .collect();
        let recent_raw = h.sessions(since, person, None, Some(8))?;
        let any_online = self.konta.values().any(|k| k.state.logged_in());
        let hidden = if any_online {
            h.hidden_online().unwrap_or_default()
        } else {
            HashSet::new()
        };
        let rejected = if any_online {
            h.rejected().unwrap_or_default()
        } else {
            Default::default()
        };
        let recent = recent_raw
            .iter()
            .map(|s| self.session_row(s, &hidden, &rejected))
            .collect();
        Ok(HistoryView {
            query: q,
            stamp,
            many,
            everyone,
            totals: (&totals).into(),
            chart: HistoryChart {
                hourly,
                bars,
                colors: chart_colors,
                legend: names,
                empty,
            },
            group_key: if everyone { "person" } else { "profile" },
            groups,
            recent,
            online: any_online,
        })
    }

    pub fn history_session(&mut self, id: &str) -> CmdResult<Option<SessionDetail>> {
        if let Some(h) = self.history.as_ref() {
            let _ = self.tracker.flush(h);
        }
        let Some(h) = self.history.as_ref() else {
            return Ok(None);
        };
        let Some(s) = h.session(id)? else {
            return Ok(None);
        };
        let hidden = h.hidden_online().unwrap_or_default();
        let rejected = h.rejected().unwrap_or_default();
        let row = self.session_row(&s, &hidden, &rejected);
        Ok(Some(SessionDetail {
            row,
            cadence: (s.active_s >= 30.0).then(|| s.steps as f64 / (s.active_s / 60.0)),
            whole_s: s.end - s.start,
            device_model: s.device_model.clone(),
            driver: s.driver.clone(),
            firmware: s.firmware.clone(),
            app_version: s.app_version.clone(),
            tz_offset_min: s.tz_offset_min,
            linked: self
                .konta
                .get(&s.person)
                .is_some_and(|k| k.state.logged_in()),
        }))
    }

    /// Usunięcie sesji; zwraca osobę z kontem, gdy trzeba ją usunąć też z serwera.
    pub fn history_session_delete(&mut self, id: &str) -> CmdResult<Option<String>> {
        let Some(h) = self.history.as_ref() else {
            return Ok(None);
        };
        if self.tracker.current().is_some_and(|s| s.id == id) {
            let _ = self.tracker.finish(h); // trwająca sesja - kończymy (i usuwamy niżej)
        }
        let gone = h.session(id)?;
        h.delete(id)?;
        log::info!("Usunięto sesję z historii");
        self.toast(crate::teksty::t("sesja.usunieta", &[]));
        self.levels_key = None;
        self.mark(&[SliceName::History, SliceName::People]);
        Ok(gone
            .map(|s| s.person)
            .filter(|p| self.konta.get(p).is_some_and(|k| k.state.logged_in())))
    }

    // --- kopia danych ----------------------------------------------------------------------

    pub fn export_choice(c: &ExportChoice) -> backup::ExportChoice {
        let people = c
            .people
            .iter()
            .filter(|(_, ids)| !ids.is_empty())
            .map(|(p, ids)| (p.clone(), Some(ids.iter().cloned().collect())))
            .collect();
        backup::ExportChoice {
            people,
            settings: c.settings,
            results: c.results,
            logs: c.logs,
            since: backup::range_since(&c.range),
        }
    }

    pub fn reload_store(&mut self) {
        let path = walklevel_core::store::Store::default_path(&self.paths.data_files());
        if let Some(h) = self.history.as_ref() {
            let _ = self.tracker.finish(h);
        }
        self.store = walklevel_core::store::Store::load(&path);
        self.history = walklevel_core::history::History::open(
            &walklevel_core::history::History::default_path(&self.paths.data),
        )
        .map_err(|e| log::warn!("Historia kroków niedostępna: {e}"))
        .ok();
        self.apply_profile();
        self.set_tracker_source();
    }
}

pub fn backup_error(e: backup::BackupError) -> CmdError {
    match e {
        backup::BackupError::Invalid(m) => CmdError::invalid(m),
        other => CmdError::io(other),
    }
}

pub fn summary(s: &backup::ExportSummary) -> ExportSummary {
    ExportSummary {
        people: s.people,
        profiles: s.profiles,
        sessions: s.sessions,
        bytes: s.bytes,
    }
}

pub fn preview_view(p: &backup::Preview, existing: &HashSet<String>) -> ImportPreview {
    ImportPreview {
        file: p
            .path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default(),
        app_version: p.app_version.clone(),
        has_settings: p.has_settings,
        has_results: p.has_results,
        description: backup::describe(p),
        people: p
            .people
            .iter()
            .map(|x| ImportPerson {
                id: x.id.clone(),
                name: x.name.clone(),
                color: x.color.clone(),
                profiles: x.profiles().len() as u32,
                sessions: p.sessions_of(&x.id).len() as u32,
                exists: existing.contains(&x.id),
            })
            .collect(),
    }
}

// --- zgłoszenia ------------------------------------------------------------------------------

/// Paczka zgłoszenia i (bez `save_only`) wysyłka albo e-mail. Blokujące - poza pętlą.
pub fn report(
    core: &Core,
    description: String,
    contact: String,
    period_s: f64,
    save_only: bool,
) -> CmdResult<String> {
    let trimmed = description.trim().to_string();
    if !save_only && let Some(problem) = walklevel_core::reports::description_problem(&trimmed) {
        return Err(CmdError::invalid(problem));
    }
    let (root, endpoint) = {
        let mut app = lock(&core.0.app);
        if app.report.busy {
            return Err(CmdError::new(
                ErrorCode::Busy,
                crate::teksty::t("zgloszenie.trwa", &[]),
            ));
        }
        app.report.busy = true;
        app.report.status = crate::teksty::t("zgloszenie.przygotowuje", &[]);
        app.mark(&[SliceName::Report]);
        (
            app.paths.data.clone(),
            app.config
                .reports_url()
                .map(str::to_string)
                .filter(|u| !u.trim().is_empty()),
        )
    };
    core.flush();
    let description = if trimmed.is_empty() {
        crate::teksty::t("zgloszenie.bez_opisu", &[])
    } else {
        trimmed
    };
    let version = walklevel_core::paths::app_version().to_string();
    let info = walklevel_core::reports::ReportInfo {
        description: description.clone(),
        contact: contact.clone(),
        version: version.clone(),
        system: crate::okna::windows_version(),
        period_s,
    };
    let folders = walklevel_core::reports::ReportFolders::in_root(&root);
    let result = walklevel_core::reports::build_report(&folders, &info).and_then(|path| {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        if save_only {
            let _ = walklevel_core::reports::show_in_folder(&path);
            Ok((
                name.clone(),
                crate::teksty::t(
                    "zgloszenie.zapisano",
                    &[
                        ("plik", &name),
                        ("email_autora", walklevel_core::reports::CONTACT),
                    ],
                ),
                None,
            ))
        } else if let Some(endpoint) = &endpoint {
            let number = walklevel_core::reports::send_report(
                &path,
                endpoint,
                &description,
                &contact,
                Some(period_s),
                &version,
            )?;
            Ok((
                name,
                walklevel_core::reports::sent_message(&number),
                Some(number),
            ))
        } else {
            crate::okna::shell_open(&walklevel_core::reports::mailto_link(
                &description,
                &name,
                &version,
            ));
            let _ = walklevel_core::reports::show_in_folder(&path);
            Ok((
                name.clone(),
                crate::teksty::t("zgloszenie.email_gotowy", &[("plik", &name)]),
                None,
            ))
        }
    });
    let mut app = lock(&core.0.app);
    app.report.busy = false;
    let out = match result {
        Ok((name, status, number)) => {
            app.report.status = status;
            app.report.last_file = Some(name.clone());
            app.report.sent_number = number.filter(|n| !n.is_empty());
            Ok(name)
        }
        Err(e) => {
            app.report.status = e.to_string();
            Err(CmdError::new(ErrorCode::Server, e.to_string()))
        }
    };
    app.mark(&[SliceName::Report]);
    drop(app);
    core.flush();
    out
}

// --- aktualizacje ----------------------------------------------------------------------------

fn set_update(core: &Core, f: impl FnOnce(&mut UpdateView)) {
    let mut app = lock(&core.0.app);
    f(&mut app.update);
    app.mark(&[SliceName::Update]);
    drop(app);
    core.flush();
}

/// Sprawdzenie (i na żądanie instalacja) aktualizacji. Blokujące - poza pętlą.
pub fn update_check(core: &Core, install: bool) -> CmdResult<()> {
    use walklevel_core::updates;
    if !walklevel_core::paths::is_installed() {
        if install {
            return Err(CmdError::new(
                ErrorCode::Unavailable,
                crate::teksty::t("glowny.pasek.aktualizuj_niedostepne", &[]),
            ));
        }
        return Ok(());
    }
    if install {
        let mut app = lock(&core.0.app);
        if app.update.busy {
            return Err(CmdError::new(
                ErrorCode::Busy,
                crate::teksty::t("aktualizacja.trwa", &[]),
            ));
        }
        app.update.busy = true;
        app.update.status = crate::teksty::t("aktualizacja.sprawdzam", &[]);
        app.mark(&[SliceName::Update]);
    }
    core.flush();
    let result = (|| -> Result<Option<std::path::PathBuf>, updates::UpdateError> {
        let client = updates::ReqwestClient::new()?;
        let release = updates::fetch_latest(&client)?;
        if !updates::is_newer(&release.version, env!("CARGO_PKG_VERSION")) {
            if install {
                set_update(core, |u| {
                    u.status = crate::teksty::t(
                        "aktualizacja.najnowsza",
                        &[("wersja", env!("CARGO_PKG_VERSION"))],
                    )
                });
            }
            return Ok(None);
        }
        let version = release.version.clone();
        set_update(core, |u| u.available = Some(version.clone()));
        if !install {
            return Ok(None);
        }
        let mut last = 101;
        let installer = updates::download(&release, None, &client, |done, total| {
            if let Some(percent) = (done * 100).checked_div(total)
                && percent != last
            {
                last = percent;
                set_update(core, |u| {
                    u.status = crate::teksty::t(
                        "aktualizacja.pobieram",
                        &[("nowa_wersja", &version), ("procent", &percent.to_string())],
                    )
                });
            }
        })?;
        Ok(Some(installer))
    })();
    match result {
        Ok(Some(installer)) => {
            let version = lock(&core.0.app)
                .update
                .available
                .clone()
                .unwrap_or_default();
            set_update(core, |u| {
                u.status = crate::teksty::t("aktualizacja.instaluje", &[("nowa_wersja", &version)]);
                u.installing = true;
            });
            quit_prepare(core); // zatrzymanie taśmy i zapis stanu przed instalatorem
            if let Err(e) = updates::run_installer(&installer) {
                log::error!("Instalator: {e}");
                set_update(core, |u| {
                    u.status = crate::teksty::t("aktualizacja.instalator_blad", &[]);
                    u.installing = false;
                    u.busy = false;
                });
                return Err(CmdError::new(ErrorCode::Io, e.to_string()));
            }
            std::thread::sleep(Duration::from_millis(400));
            lock(&core.0.app).window(Okno::Quit);
            core.flush();
            Ok(())
        }
        Ok(None) => {
            set_update(core, |u| u.busy = false);
            Ok(())
        }
        Err(e) => {
            log::warn!("Aktualizacja: {e}");
            set_update(core, |u| {
                u.busy = false;
                if install {
                    u.status = e.to_string();
                }
            });
            if install {
                Err(CmdError::new(ErrorCode::Offline, e.to_string()))
            } else {
                Ok(())
            }
        }
    }
}

// --- zamykanie -------------------------------------------------------------------------------

/// Puszcza klawisze, zatrzymuje taśmę (do 8 s), zapisuje geometrię i stan, kończy sesję.
pub fn quit_prepare(core: &Core) {
    if core
        .0
        .quitting
        .swap(true, std::sync::atomic::Ordering::SeqCst)
    {
        return;
    }
    let geometry = crate::okna::main_geometry(&core.0.handle);
    {
        let mut guard = lock(&core.0.app);
        let app = &mut *guard;
        app.bridge.shutdown();
        if let Some(g) = geometry {
            app.settings.insert("window_geometry".into(), json!(g));
        }
        app.save();
        if let Some(h) = app.history.as_ref() {
            let _ = app.tracker.finish(h);
        }
    }
    // zamknięcie zawsze zatrzymuje jadącą taśmę (worker sprawdza, czy w ogóle jedzie)
    let app = lock(&core.0.app);
    if app.bieznia.stop_belt_now(Duration::from_secs(8)) {
        log::info!("Zamykanie: zatrzymano taśmę");
    }
    app.bieznia.shutdown();
}

pub fn quit(core: &Core) {
    quit_prepare(core);
    lock(&core.0.app).window(Okno::Quit);
    core.flush();
}

/// Paczka nie może zawierać ścieżki spoza `dane/ikony` (protokół ikon).
pub fn icon_file(data_files: &Path, name: &str) -> Option<std::path::PathBuf> {
    let ok = !name.is_empty()
        && !name.contains(['/', '\\'])
        && !name.contains("..")
        && name.to_ascii_lowercase().ends_with(".png");
    ok.then(|| data_files.join(walklevel_core::store::ICONS_DIR).join(name))
}

/// Ikona sesji i grupy profilu, którego już nie ma (pad na polu w kolorze domyślnym).
fn removed_profile_icon() -> IconRef {
    IconRef {
        color: palette::DEFAULT.into(),
        glyph: Some("\u{e7fc}".into()),
        url: None,
    }
}
