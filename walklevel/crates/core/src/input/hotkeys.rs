//! Globalne skróty (F6/F7/F8, Ctrl+Shift+F8) - port bieznia/hotkeys.py.
//!
//! Wykrywa moment wciśnięcia klawisza także wtedy, gdy gra ma fokus. Sprawdza wyłącznie
//! klawisze z konfiguracji, niczego nie zapisuje.
//!
//! Działa przez odpytywanie stanu klawiszy (GetAsyncKeyState), jak w Pythonie, a nie przez
//! RegisterHotKey: RegisterHotKey zabiera klawisz grze, nie działa, gdy inny program zajął
//! ten sam skrót, i wymaga dokładnego zestawu modyfikatorów (Shift trzymany do biegu
//! blokowałby F8). Aplikacja Tauri odpytuje [`Hotkeys::poll`] w swojej pętli albo uruchamia
//! [`HotkeyWatcher`], który robi to w osobnym wątku.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::Duration;

use super::InputError;
use super::keys::{Key, Mod};
use super::system;

/// Kod wirtualny F1 (F2 = 0x71 ... F12 = 0x7B).
pub const VK_F1: u32 = 0x70;

/// Klawisz skrótu: nazwa F1-F12 (z config.toml), kod wirtualny albo kod z modyfikatorami.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Binding {
    /// "F8" (wielkość liter bez znaczenia)
    Name(String),
    /// kod wirtualny, np. 0x24 = Home
    Vk(i64),
    /// kod wirtualny z wymaganymi modyfikatorami (Ctrl+Shift+F8)
    Combo(i64, Vec<Mod>),
}

impl Binding {
    /// Skrót z wybranego klawisza start/stop (kod wirtualny + modyfikatory).
    pub fn from_key(key: &Key) -> Binding {
        Binding::Combo(key.vk, key.mods.clone())
    }
}

/// Stan klawiszy: czy kod wirtualny jest teraz wciśnięty.
pub trait KeyState {
    fn is_down(&self, vk: u32) -> bool;
}

impl<F: Fn(u32) -> bool> KeyState for F {
    fn is_down(&self, vk: u32) -> bool {
        self(vk)
    }
}

/// Prawdziwa klawiatura (GetAsyncKeyState; poza Windows nic nie jest wciśnięte).
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemKeys;

impl KeyState for SystemKeys {
    fn is_down(&self, vk: u32) -> bool {
        system::is_key_down(vk)
    }
}

struct Action {
    name: String,
    vk: u32,
    mods: Vec<u32>,
    was_down: bool,
}

/// Skróty: akcja -> klawisz. [`Hotkeys::poll`] zwraca akcje, których klawisz został właśnie
/// wciśnięty (jeden raz na naciśnięcie).
pub struct Hotkeys<K: KeyState = SystemKeys> {
    actions: Vec<Action>,
    keys: K,
}

/// `repr()` napisu z Pythona (do komunikatów jak w 0.5.x).
fn py_repr(s: &str) -> String {
    if s.contains('\'') && !s.contains('"') {
        format!("\"{s}\"")
    } else {
        format!("'{}'", s.replace('\\', "\\\\").replace('\'', "\\'"))
    }
}

fn vk_of(action: &str, binding: &Binding) -> Result<u32, InputError> {
    let code = match binding {
        Binding::Vk(vk) | Binding::Combo(vk, _) => *vk,
        Binding::Name(name) => {
            let upper = name.to_uppercase();
            let f = upper
                .strip_prefix('F')
                .and_then(|n| n.parse::<u32>().ok())
                .filter(|n| (1..=12).contains(n));
            // tylko „F” + numer bez zer z przodu i znaków ("F08" i "F+8" nie są nazwami)
            return match f {
                Some(n) if upper == format!("F{n}") => Ok(VK_F1 + n - 1),
                _ => Err(InputError::BadHotkey(format!(
                    "Nieobsługiwany klawisz {} dla {} (dozwolone F1-F12)",
                    py_repr(name),
                    py_repr(action)
                ))),
            };
        }
    };
    match u32::try_from(code) {
        Ok(vk) if vk > 0 && vk < 0x100 => Ok(vk),
        _ => Err(InputError::BadHotkey(format!(
            "Zły kod klawisza {code} dla {}",
            py_repr(action)
        ))),
    }
}

impl<K: KeyState> Hotkeys<K> {
    /// Skróty z listy (akcja, klawisz). Zła nazwa albo kod - błąd z opisem.
    /// Klawisze trzymane przy starcie nie liczą się jako wciśnięcie.
    pub fn new(bindings: Vec<(String, Binding)>, keys: K) -> Result<Hotkeys<K>, InputError> {
        let mut actions = Vec::with_capacity(bindings.len());
        for (name, binding) in bindings {
            let vk = vk_of(&name, &binding)?;
            let mods = match &binding {
                Binding::Combo(_, mods) => mods.iter().map(|m| m.vk()).collect(),
                _ => Vec::new(),
            };
            let action = Action {
                name,
                vk,
                mods,
                was_down: false,
            };
            // powtórzona akcja zastępuje wcześniejszą (jak klucz słownika w Pythonie)
            match actions.iter().position(|a: &Action| a.name == action.name) {
                Some(i) => actions[i] = action,
                None => actions.push(action),
            }
        }
        let mut hotkeys = Hotkeys { actions, keys };
        for i in 0..hotkeys.actions.len() {
            hotkeys.actions[i].was_down = hotkeys.down(i);
        }
        Ok(hotkeys)
    }

    /// Klawisz wciśnięty razem z wymaganymi modyfikatorami (inne nie przeszkadzają - np.
    /// Shift trzymany do biegu nie blokuje pojedynczego F8).
    fn down(&self, i: usize) -> bool {
        let action = &self.actions[i];
        self.keys.is_down(action.vk) && action.mods.iter().all(|m| self.keys.is_down(*m))
    }

    /// Akcje, których klawisz został właśnie wciśnięty (w kolejności z konfiguracji).
    pub fn poll(&mut self) -> Vec<String> {
        let mut pressed = Vec::new();
        for i in 0..self.actions.len() {
            let down = self.down(i);
            let action = &mut self.actions[i];
            if down && !action.was_down {
                pressed.push(action.name.clone());
            }
            action.was_down = down;
        }
        pressed
    }
}

/// Wątek odpytujący skróty co `every` i wołający `on_press(akcja)` przy wciśnięciu.
/// Zatrzymuje się przy [`HotkeyWatcher::stop`] albo przy porzuceniu (Drop). Zmiana skrótów
/// = zatrzymać i uruchomić nowy (trzymany właśnie klawisz się nie liczy).
pub struct HotkeyWatcher {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl HotkeyWatcher {
    /// Domyślny odstęp odpytywania (jak klatka okna).
    pub const EVERY: Duration = Duration::from_millis(20);

    pub fn spawn<K, F>(
        mut hotkeys: Hotkeys<K>,
        every: Duration,
        mut on_press: F,
    ) -> Result<HotkeyWatcher, InputError>
    where
        K: KeyState + Send + 'static,
        F: FnMut(&str) + Send + 'static,
    {
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let thread = std::thread::Builder::new()
            .name("skroty".into())
            .spawn(move || {
                while !flag.load(Ordering::SeqCst) {
                    for action in hotkeys.poll() {
                        on_press(&action);
                    }
                    std::thread::sleep(every);
                }
            })
            .map_err(|e| InputError::Thread(e.to_string()))?;
        Ok(HotkeyWatcher {
            stop,
            thread: Some(thread),
        })
    }

    /// Zatrzymuje wątek i czeka na jego koniec.
    pub fn stop(mut self) {
        self.shutdown();
    }

    fn shutdown(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(t) = self.thread.take()
            && t.join().is_err()
        {
            log::warn!("Wątek skrótów zakończył się błędem");
        }
    }
}

impl Drop for HotkeyWatcher {
    fn drop(&mut self) {
        self.shutdown();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::collections::HashSet;
    use std::rc::Rc;

    const F8: u32 = 0x77;

    #[derive(Clone, Default)]
    struct FakeKeyboard(Rc<RefCell<HashSet<u32>>>);

    impl FakeKeyboard {
        fn set(&self, keys: &[u32]) {
            *self.0.borrow_mut() = keys.iter().copied().collect();
        }
    }

    impl KeyState for FakeKeyboard {
        fn is_down(&self, vk: u32) -> bool {
            self.0.borrow().contains(&vk)
        }
    }

    fn hk(bindings: &[(&str, Binding)], kb: &FakeKeyboard) -> Hotkeys<FakeKeyboard> {
        let list = bindings
            .iter()
            .map(|(a, b)| (a.to_string(), b.clone()))
            .collect();
        Hotkeys::new(list, kb.clone()).unwrap()
    }

    #[test]
    fn hotkey_fires_once_per_press() {
        let kb = FakeKeyboard::default();
        let mut h = hk(&[("toggle", Binding::Name("F8".into()))], &kb);
        assert!(h.poll().is_empty());
        kb.set(&[F8]);
        assert_eq!(h.poll(), ["toggle"]);
        assert!(h.poll().is_empty());
        kb.set(&[]);
        assert!(h.poll().is_empty());
    }

    #[test]
    fn key_held_at_startup_is_ignored() {
        let kb = FakeKeyboard::default();
        kb.set(&[F8]);
        let mut h = hk(&[("toggle", Binding::Name("f8".into()))], &kb);
        assert!(h.poll().is_empty());
    }

    #[test]
    fn unknown_hotkey_is_rejected() {
        let bad = |b: Binding| {
            Hotkeys::new(vec![("toggle".into(), b)], |_| false)
                .err()
                .map(|e| e.to_string())
        };
        assert_eq!(
            bad(Binding::Name("Q".into())).as_deref(),
            Some("Nieobsługiwany klawisz 'Q' dla 'toggle' (dozwolone F1-F12)")
        );
        assert_eq!(
            bad(Binding::Vk(0)).as_deref(),
            Some("Zły kod klawisza 0 dla 'toggle'")
        );
        assert!(bad(Binding::Name("F12".into())).is_none());
    }

    #[test]
    fn hotkey_by_virtual_key_code() {
        let kb = FakeKeyboard::default();
        let mut h = hk(
            &[
                ("toggle", Binding::Vk(0x24)),
                ("sim_faster", Binding::Name("F7".into())),
            ],
            &kb,
        );
        kb.set(&[0x24]);
        assert_eq!(h.poll(), ["toggle"]);
    }

    #[test]
    fn hotkey_combination_needs_its_modifiers() {
        let kb = FakeKeyboard::default();
        let mut h = hk(
            &[("toggle", Binding::Combo(0x77, vec![Mod::Ctrl, Mod::Shift]))],
            &kb,
        );
        kb.set(&[F8]);
        assert!(h.poll().is_empty());
        kb.set(&[]);
        h.poll();
        kb.set(&[0x11, 0x10, F8]);
        assert_eq!(h.poll(), ["toggle"]);
        assert!(h.poll().is_empty());
    }

    #[test]
    fn repeated_action_uses_the_last_binding() {
        let kb = FakeKeyboard::default();
        let mut h = hk(
            &[
                ("toggle", Binding::Name("F8".into())),
                ("toggle", Binding::Vk(0x24)),
            ],
            &kb,
        );
        kb.set(&[F8]);
        assert!(h.poll().is_empty());
        kb.set(&[0x24]);
        assert_eq!(h.poll(), ["toggle"]);
    }

    #[test]
    fn single_hotkey_works_while_shift_is_held() {
        let kb = FakeKeyboard::default();
        let mut h = hk(
            &[("toggle", Binding::from_key(&Key::toggle_default()))],
            &kb,
        );
        kb.set(&[0x10, F8]);
        assert_eq!(h.poll(), ["toggle"]);
    }

    #[test]
    fn watcher_reports_presses_from_its_thread() {
        let down = Arc::new(AtomicBool::new(false));
        let d = down.clone();
        let keys = move |vk: u32| vk == F8 && d.load(Ordering::SeqCst);
        let hotkeys =
            Hotkeys::new(vec![("toggle".into(), Binding::Name("F8".into()))], keys).unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        let watcher = HotkeyWatcher::spawn(hotkeys, Duration::from_millis(2), move |a| {
            let _ = tx.send(a.to_string());
        })
        .unwrap();
        down.store(true, Ordering::SeqCst);
        assert_eq!(
            rx.recv_timeout(Duration::from_secs(2)).ok().as_deref(),
            Some("toggle")
        );
        watcher.stop();
    }
}
