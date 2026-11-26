//! Osoby i profile gier (bieznia/gui/ekrany/osoba.py, profil.py): konfigurator, zmiana osoby
//! i profilu, profil z gry (ikona z exe), usuwanie.

use std::path::Path;

use serde_json::json;
use walklevel_core::store::{self, NewProfile, ProfileChanges, Store, names, palette};
use walklevel_core::{icons, store::names::NameRng};

use crate::stan::{App, Wybor};
use crate::widoki::*;

impl App {
    fn icons_dir(&self) -> std::path::PathBuf {
        self.paths.data_files().join(store::ICONS_DIR)
    }

    fn taken_names(&self) -> Vec<String> {
        self.store.as_ref().map(Store::names).unwrap_or_default()
    }

    pub fn wizard_seed(&self) -> WizardSeed {
        let mut rng = NameRng::new();
        let taken = self.taken_names();
        let name = names::random_name(&taken, &mut rng);
        let mut more = taken.clone();
        more.push(name.clone());
        let suggestions = names::suggestions(3, &more, &mut rng);
        let used: Vec<String> = self
            .store
            .as_ref()
            .map(|s| s.people().iter().map(|p| p.color.clone()).collect())
            .unwrap_or_default();
        let free: Vec<&str> = palette::names()
            .into_iter()
            .filter(|c| !used.iter().any(|u| u == c) && *c != palette::DEFAULT)
            .collect();
        let pool = if free.is_empty() {
            palette::names()
        } else {
            free
        };
        let pick = (rand_u64() % pool.len() as u64) as usize;
        WizardSeed {
            name,
            suggestions,
            color: pool[pick].to_string(),
        }
    }

    pub fn name_random(&self) -> String {
        names::random_name(&self.taken_names(), &mut NameRng::new())
    }

    /// Gra z folderu albo pliku: ikona 128 px do `dane/ikony/nowa-*.png` i kolor z ikony.
    pub fn game_from(&mut self, path: &Path) -> CmdResult<GamePick> {
        let Some(found) = icons::resolve(path) else {
            return Err(CmdError::new(
                ErrorCode::NotFound,
                crate::teksty::t("profil.brak_pliku", &[]),
            ));
        };
        let dir = self.icons_dir();
        std::fs::create_dir_all(&dir)?;
        let pick_id = format!("{:08x}", rand_u64() as u32);
        let tmp = format!("nowa-{pick_id}.png");
        let (icon_file, color) = match icons::extract_icon(&found.exe, &dir.join(&tmp)) {
            Some(c) => (Some(tmp), c.name.to_string()),
            None => (None, palette::DEFAULT.to_string()),
        };
        let exe = found.exe.to_string_lossy().into_owned();
        log::info!(
            "Gra: {} ({}), kolor z ikony {color}",
            found.name,
            found
                .exe
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default()
        );
        let wybor = Wybor {
            exe: exe.clone(),
            icon_file: icon_file.clone(),
            icon_color: color.clone(),
            name: found.name.clone(),
        };
        self.picks.insert(pick_id.clone(), wybor);
        let url = icon_file
            .as_ref()
            .map(|f| format!("http://ikona.localhost/{f}?v=0"));
        Ok(GamePick {
            pick_id,
            name: found.name,
            exe_name: found
                .exe
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
            folder_name: found
                .exe
                .parent()
                .and_then(|p| p.file_name())
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
            exe,
            icon: IconRef {
                color: color.clone(),
                glyph: None,
                url,
            },
            icon_color: color,
        })
    }

    pub fn game_pick_discard(&mut self, pick_id: &str) {
        if let Some(w) = self.picks.remove(pick_id)
            && let Some(f) = w.icon_file
        {
            let _ = std::fs::remove_file(self.icons_dir().join(f));
        }
    }

    /// Tymczasowa ikona wyboru -> `<id profilu>.png` i zapis w profilu.
    fn attach_pick(&mut self, profile_id: &str, pick_id: &str) -> CmdResult<()> {
        let Some(w) = self.picks.remove(pick_id) else {
            return Err(CmdError::invalid(crate::teksty::t(
                "profil.wybierz_ponownie",
                &[],
            )));
        };
        let dir = self.icons_dir();
        let mut changes = ProfileChanges {
            exe: Some(Some(w.exe)),
            icon_color: Some(Some(w.icon_color)),
            ..ProfileChanges::default()
        };
        if let Some(tmp) = w.icon_file {
            let final_name = format!("{profile_id}.png");
            std::fs::rename(dir.join(&tmp), dir.join(&final_name))?;
            changes.icon_file = Some(Some(final_name));
            changes.glyph = Some(None);
        }
        if let Some(store) = self.store.as_mut() {
            store.update_profile(profile_id, changes)?;
        }
        Ok(())
    }

    pub fn wizard_finish(&mut self, r: WizardResult) -> CmdResult<()> {
        if r.mode == "first" || self.store.is_none() {
            let had_legacy = self.legacy.is_some();
            let old = self.paths.data_files().join(store::OLD_SETTINGS);
            if let Err(e) = store::backup_legacy(&old) {
                log::warn!("Kopia ustawień 0.2: {e}");
            }
            let flat = self.settings.clone(); // z ustawieniami 0.2 i zmianami z tej sesji
            let path = Store::default_path(&self.paths.data_files());
            self.store = Some(Store::create(&path, &r.name, &r.color, Some(&flat))?);
            self.legacy = None;
            log::info!(
                "Pierwsza osoba: {}{}",
                r.name,
                if had_legacy {
                    " (ustawienia z 0.2)"
                } else {
                    ""
                }
            );
        } else if let Some(store) = self.store.as_mut() {
            let copy = r.copy_profiles.then(|| store.person().id.clone());
            store.add_person(&r.name, &r.color, copy.as_deref())?;
            log::info!("Nowa osoba: {}", r.name);
        }
        if let Some(game) = r.game {
            let store = self.store.as_mut().expect("osoba utworzona wyżej");
            let base = store.profile().id.clone();
            let icon_color = self.picks.get(&game.pick_id).map(|w| w.icon_color.clone());
            let name = if game.name.trim().is_empty() {
                self.picks
                    .get(&game.pick_id)
                    .map(|w| w.name.clone())
                    .unwrap_or_default()
            } else {
                game.name.clone()
            };
            let id = store
                .add_profile(
                    &name,
                    NewProfile {
                        color: Some(game.color.clone()),
                        icon_color,
                        copy_from: Some(base),
                        ..NewProfile::default()
                    },
                )?
                .id
                .clone();
            self.attach_pick(&id, &game.pick_id)?;
        }
        if let Some(store) = self.store.as_mut() {
            store.set("direction", json!(r.direction))?;
        }
        self.first_run = false;
        self.apply_profile();
        Ok(())
    }

    pub fn person_switch(&mut self, person_id: &str) -> CmdResult<()> {
        let Some(store) = self.store.as_mut() else {
            return Ok(());
        };
        if person_id != store.person().id {
            store.switch_person(person_id)?;
            log::info!("Osoba: {}", store.person().name);
            self.online = OnlineView::default();
            self.account_message.clear();
            self.apply_profile();
        }
        Ok(())
    }

    pub fn person_update(&mut self, person_id: &str, patch: PersonPatch) -> CmdResult<()> {
        let Some(store) = self.store.as_mut() else {
            return Ok(());
        };
        let name = patch.name.unwrap_or_default();
        store.rename_person(person_id, &name, patch.color.as_deref())?;
        self.settings
            .insert("person_color".into(), json!(store.person().color));
        self.people_rev += 1;
        self.mark(&[SliceName::People, SliceName::Settings, SliceName::History]);
        Ok(())
    }

    pub fn person_remove(&mut self, person_id: &str) -> CmdResult<bool> {
        let Some(store) = self.store.as_mut() else {
            return Ok(false);
        };
        if !store.remove_person(person_id)? {
            return Err(CmdError::invalid(crate::teksty::t(
                "osoba.blad.ostatnia",
                &[],
            )));
        }
        if let Some(h) = self.history.as_ref()
            && let Err(e) = h.forget_person(person_id)
        {
            log::warn!("Historia: {e}");
        }
        self.konta.remove(person_id);
        self.apply_profile();
        Ok(true)
    }

    pub fn profile_switch(&mut self, profile_id: &str) -> CmdResult<()> {
        let Some(store) = self.store.as_mut() else {
            return Ok(());
        };
        if profile_id != store.profile().id {
            store.switch_profile(profile_id)?;
            log::info!("Profil: {}", store.profile().name);
            self.apply_profile();
        }
        Ok(())
    }

    pub fn profile_create(&mut self, d: ProfileDraft) -> CmdResult<String> {
        let Some(store) = self.store.as_mut() else {
            return Err(CmdError::invalid(crate::teksty::t("osoba.blad.brak", &[])));
        };
        let copy = d.copy_from_active.then(|| store.profile().id.clone());
        let id = store
            .add_profile(
                &d.name,
                NewProfile {
                    color: Some(d.color),
                    icon_color: d.icon_color,
                    glyph: d.glyph,
                    copy_from: copy,
                    ..NewProfile::default()
                },
            )?
            .id
            .clone();
        if let Some(pick) = d.pick_id {
            self.attach_pick(&id, &pick)?;
        }
        log::info!("Nowy profil: {}", d.name);
        self.apply_profile();
        Ok(id)
    }

    pub fn profile_update(&mut self, profile_id: &str, p: ProfilePatch) -> CmdResult<()> {
        if let Some(pick) = &p.pick_id {
            self.attach_pick(profile_id, pick)?;
        }
        let Some(store) = self.store.as_mut() else {
            return Ok(());
        };
        let online_before = store
            .person()
            .profiles()
            .iter()
            .find(|x| x.id == profile_id)
            .map(|x| x.online);
        let mut changes = ProfileChanges {
            name: p.name,
            color: p.color,
            online: p.online,
            ..ProfileChanges::default()
        };
        if let Some(glyph) = p.glyph {
            changes.glyph = Some(Some(glyph));
            changes.icon_file = Some(None);
        }
        store.update_profile(profile_id, changes)?;
        if p.online.is_some()
            && p.online != online_before
            && let Some(h) = self.history.as_ref()
        {
            let _ = h.forget_sent(); // sesje pójdą jeszcze raz (z nazwą gry albo bez)
        }
        if store.profile().id == profile_id {
            self.settings
                .insert("profile_color".into(), json!(store.profile().color));
            self.set_tracker_owner();
        }
        self.people_rev += 1;
        self.mark(&[
            SliceName::People,
            SliceName::Settings,
            SliceName::Bubble,
            SliceName::History,
        ]);
        Ok(())
    }

    pub fn profile_duplicate(&mut self, profile_id: &str) -> CmdResult<String> {
        let Some(store) = self.store.as_mut() else {
            return Err(CmdError::new(
                ErrorCode::NotFound,
                crate::teksty::t("profil.blad.brak", &[]),
            ));
        };
        let id = store
            .duplicate_profile(profile_id)?
            .map(|p| p.id.clone())
            .ok_or_else(|| {
                CmdError::new(
                    ErrorCode::NotFound,
                    crate::teksty::t("profil.blad.brak", &[]),
                )
            })?;
        self.people_rev += 1;
        self.mark(&[SliceName::People]);
        Ok(id)
    }

    pub fn profile_remove(&mut self, profile_id: &str) -> CmdResult<()> {
        let Some(store) = self.store.as_mut() else {
            return Ok(());
        };
        if !store.remove_profile(profile_id)? {
            return Err(CmdError::invalid(crate::teksty::t(
                "profil.blad.ostatni",
                &[],
            )));
        }
        self.apply_profile();
        Ok(())
    }

    /// Usuwa porzucone tymczasowe ikony (`nowa-*.png` starsze niż doba).
    pub fn clean_temp_icons(&self) {
        let Ok(entries) = std::fs::read_dir(self.icons_dir()) else {
            return;
        };
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            let old = e
                .metadata()
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.elapsed().ok())
                .is_some_and(|age| age.as_secs() > 86_400);
            if name.starts_with("nowa-") && name.ends_with(".png") && old {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
}

/// Losowa liczba bez dodatkowego crate'a (RandomState ma losowe ziarno procesu).
pub fn rand_u64() -> u64 {
    use std::hash::{BuildHasher, Hasher};
    let mut h = std::collections::hash_map::RandomState::new().build_hasher();
    h.write_u128(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos()),
    );
    h.finish()
}
