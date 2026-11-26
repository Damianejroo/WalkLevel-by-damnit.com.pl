//! Poziomy, osiągnięcia i popupy (bieznia/gui/ekrany/poziomy.py): przeliczenie co ok. 2 s,
//! gdy zmieniła się historia albo osoba; popup zbiorczy przy pierwszym liczeniu.

use walklevel_core::levels::{self, EvalOptions, Progress, Status};
use walklevel_core::store::palette;

use crate::popupy::PopupData;
use crate::stan::App;
use crate::teksty;
use crate::widoki::SliceName;

impl App {
    fn levels_stamp(&self) -> Option<String> {
        let v = self.history.as_ref()?.version().ok()?;
        Some(format!(
            "{}:{}:{}:{}",
            self.person_id(),
            v.count,
            v.steps,
            v.last_end
        ))
    }

    pub fn levels_tick(&mut self) {
        let Some(key) = self.levels_stamp() else {
            return;
        };
        if self.levels_key.as_deref() != Some(key.as_str()) {
            self.levels_key = Some(key);
            let popups = !std::mem::take(&mut self.levels_quiet);
            self.refresh_levels(popups);
        }
    }

    fn compute_levels(&self, person: &str) -> Option<Status> {
        let history = self.history.as_ref()?;
        if person.is_empty() {
            return None;
        }
        let sessions = history.sessions(0.0, Some(person), None, None).ok()?;
        let walks = levels::walks_from_sessions(&sessions);
        let friends = self.online.friends.as_ref().map_or(0, |f| f.len() as u32);
        let opts = EvalOptions {
            friends,
            earned_before: history.earned(person).unwrap_or_default(),
            ..EvalOptions::default()
        };
        Some(levels::evaluate(self.defs, &walks, &opts))
    }

    /// Przeliczenie; `popups` - pokazać nowe zdobycze (ekran Osiągnięcia: bez popupów).
    pub fn refresh_levels(&mut self, popups: bool) {
        let person = self.person_id();
        let status = self.compute_levels(&person);
        self.mark(&[SliceName::Levels]);
        let (Some(status), Some(history)) = (status, self.history.as_ref()) else {
            self.levels = None;
            return;
        };
        let stored = history.earned(&person).unwrap_or_default();
        let new: Vec<Progress> = status
            .progress
            .iter()
            .filter(|p| p.earned() && !stored.contains_key(&p.achievement.id))
            .cloned()
            .collect();
        if !new.is_empty() {
            let now = levels::now_unix();
            let earned: std::collections::HashMap<String, f64> = new
                .iter()
                .map(|p| (p.achievement.id.clone(), p.earned_at.unwrap_or(now)))
                .collect();
            if let Err(e) = history.add_earned(&person, &earned, self.defs.version) {
                log::warn!("Osiągnięcia: {e}");
            }
        }
        let shown = history.shown_level(&person).ok().flatten();
        let _ = history.set_shown_level(&person, i64::from(status.level));
        if popups && self.b("achievement_popups") {
            match shown {
                None => {
                    if !status.earned().is_empty() || status.level > 1 {
                        self.show_history_summary(&status);
                    }
                }
                Some(shown) => self.show_new(&status, &new, shown),
            }
        }
        self.levels = Some(status);
    }

    fn accent(&self) -> String {
        let name = self.store.as_ref().map(|s| s.profile().color.clone());
        palette::color(name.as_deref()).swatch().to_string()
    }

    fn achievement_popup(&self, p: &Progress) -> PopupData {
        let a = &p.achievement;
        let xp = self.defs.rarity_xp.get(&a.rarity).copied().unwrap_or(0) as f64;
        PopupData {
            kicker: teksty::t(
                "powiadomienie.osiagniecie.nad",
                &[("rzadkosc", &teksty::rarity_name(&a.rarity).to_uppercase())],
            ),
            title: a.name.clone(),
            line: teksty::t(
                "powiadomienie.osiagniecie.linia",
                &[("xp", &teksty::int(xp)), ("warunek", &a.condition)],
            ),
            color: teksty::rarity_color(&a.rarity).into(),
            rarity: a.rarity.clone(),
            mark: a.mark.clone(),
            badge_text: None,
            progress: None,
        }
    }

    fn level_popup(&self, status: &Status) -> PopupData {
        let new_rank = status.rank.level == status.level;
        let next = teksty::km(status.km_to_next_level);
        let level = status.level.to_string();
        let vars = [("poziom", level.as_str()), ("km", next.as_str())];
        PopupData {
            kicker: teksty::t("powiadomienie.awans.nad", &vars),
            title: if new_rank {
                status.rank.name.clone()
            } else {
                teksty::t("powiadomienie.awans.tytul", &vars)
            },
            line: teksty::t(
                if new_rank {
                    "powiadomienie.awans.ranga_linia"
                } else {
                    "powiadomienie.awans.linia"
                },
                &vars,
            ),
            color: self.accent(),
            rarity: teksty::rank_rarity(status.level).into(),
            mark: String::new(),
            badge_text: Some(status.level.to_string()),
            progress: Some(status.level_fraction),
        }
    }

    fn show_new(&mut self, status: &Status, new: &[Progress], shown: i64) {
        let queue = levels::popup_queue(self.defs, new.iter(), levels::MAX_POPUPS_AT_ONCE);
        let mut items: Vec<PopupData> = queue
            .single
            .iter()
            .map(|p| self.achievement_popup(p))
            .collect();
        if !queue.grouped.is_empty() {
            items.push(PopupData {
                kicker: teksty::t("powiadomienie.wiele.nad", &[]),
                title: teksty::t(
                    "powiadomienie.wiele.tytul",
                    &[("liczba", &queue.grouped.len().to_string())],
                ),
                line: teksty::t("powiadomienie.wiele.linia", &[]),
                color: self.accent(),
                rarity: "rzadkie".into(),
                mark: "korona".into(),
                badge_text: None,
                progress: None,
            });
        }
        let level_up = i64::from(status.level) > shown;
        if level_up {
            items.push(self.level_popup(status));
        }
        if items.is_empty() {
            return;
        }
        for item in items {
            self.popups.push(item);
        }
        if self.b("achievement_sound") {
            crate::okna::play_sound(level_up);
        }
    }

    fn show_history_summary(&mut self, status: &Status) {
        let n = status.earned().len().to_string();
        let level = status.level.to_string();
        let km = teksty::km(status.km);
        let vars = [
            ("liczba", n.as_str()),
            ("poziom", level.as_str()),
            ("ranga", status.rank.name.as_str()),
            ("km", km.as_str()),
        ];
        self.popups.push(PopupData {
            kicker: teksty::t("powiadomienie.historia.nad", &vars),
            title: teksty::t("powiadomienie.historia.tytul", &vars),
            line: teksty::t("powiadomienie.historia.linia", &vars),
            color: self.accent(),
            rarity: teksty::rank_rarity(status.level).into(),
            mark: String::new(),
            badge_text: Some(status.level.to_string()),
            progress: None,
        });
    }

    /// Przykładowy popup (km483) - awans dokłada wywołujący po 1,4 s.
    pub fn sample_popup(&mut self) {
        if let Some(a) = self.defs.by_id("km483").cloned() {
            let mut p = Progress::new(a);
            p.earned_at = Some(levels::now_unix());
            let item = self.achievement_popup(&p);
            self.popups.push(item);
            if self.b("achievement_sound") {
                crate::okna::play_sound(false);
            }
        }
    }

    pub fn sample_level_popup(&mut self) {
        if let Some(status) = self.levels.clone() {
            let item = self.level_popup(&status);
            self.popups.push(item);
            if self.b("achievement_sound") {
                crate::okna::play_sound(true);
            }
        }
    }
}
