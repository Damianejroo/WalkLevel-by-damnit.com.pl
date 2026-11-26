//! Kolejka popupów osiągnięć i awansów (odpowiednik PopupStack z bieznia/gui/popupy.py):
//! najwyżej 3 widoczne, nowsze na dole, reszta czeka; każdy 6 s, potem 220 ms znikania.

use std::collections::VecDeque;

use crate::widoki::{PopupItem, PopupsView};

pub const MAX_VISIBLE: usize = 3;
pub const SHOW_S: f64 = 6.0;
pub const LEAVE_S: f64 = 0.22;

/// Treść popupu bez numeru i czasu.
#[derive(Debug, Clone)]
pub struct PopupData {
    pub kicker: String,
    pub title: String,
    pub line: String,
    pub color: String,
    pub rarity: String,
    pub mark: String,
    pub badge_text: Option<String>,
    pub progress: Option<f64>,
}

struct Visible {
    item: PopupItem,
    shown_at: f64,
}

#[derive(Default)]
pub struct Popupy {
    next_id: u64,
    waiting: VecDeque<PopupData>,
    visible: Vec<Visible>,
}

impl Popupy {
    pub fn push(&mut self, data: PopupData) {
        self.waiting.push_back(data);
    }

    pub fn view(&self) -> PopupsView {
        PopupsView {
            items: self.visible.iter().map(|v| v.item.clone()).collect(),
        }
    }

    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.visible.is_empty() && self.waiting.is_empty()
    }

    /// Jedna klatka: znikanie, wypadanie, nowe z kolejki. Zwraca (zmiana listy, nowy popup).
    pub fn tick(&mut self, now: f64) -> (bool, bool) {
        let mut changed = false;
        for v in &mut self.visible {
            if !v.item.leaving && now - v.shown_at >= SHOW_S {
                v.item.leaving = true;
                changed = true;
            }
        }
        let before = self.visible.len();
        self.visible.retain(|v| now - v.shown_at < SHOW_S + LEAVE_S);
        changed |= self.visible.len() != before;
        let mut added = false;
        while self.visible.len() < MAX_VISIBLE {
            let Some(data) = self.waiting.pop_front() else {
                break;
            };
            self.next_id += 1;
            self.visible.push(Visible {
                item: PopupItem {
                    id: self.next_id,
                    kicker: data.kicker,
                    title: data.title,
                    line: data.line,
                    color: data.color,
                    rarity: data.rarity,
                    mark: data.mark,
                    badge_text: data.badge_text,
                    progress: data.progress,
                    leaving: false,
                },
                shown_at: now,
            });
            added = true;
        }
        (changed || added, added)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data(n: u32) -> PopupData {
        PopupData {
            kicker: "K".into(),
            title: format!("{n}"),
            line: String::new(),
            color: "#fff".into(),
            rarity: "rzadkie".into(),
            mark: String::new(),
            badge_text: None,
            progress: None,
        }
    }

    #[test]
    fn at_most_three_then_queue_moves() {
        let mut p = Popupy::default();
        for n in 0..5 {
            p.push(data(n));
        }
        assert_eq!(p.tick(0.0), (true, true));
        assert_eq!(p.view().items.len(), 3);
        assert_eq!(p.tick(1.0), (false, false));
        assert!(p.tick(6.0).0);
        assert!(p.view().items.iter().all(|i| i.leaving));
        p.tick(6.3);
        let titles: Vec<String> = p.view().items.into_iter().map(|i| i.title).collect();
        assert_eq!(titles, ["3", "4"]);
        p.tick(13.0);
        assert!(p.is_empty());
    }
}
