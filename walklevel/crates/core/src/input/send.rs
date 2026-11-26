//! Wyjścia sterowania: klawisze przez SendInput (port bieznia/outputs/keyboard.py) i wyjście
//! „na sucho” (outputs/dry.py).
//!
//! Chód = klawisz chodu, bieg = klawisz biegu + chodu. Klawisz jest wciśnięty przez cały czas
//! chodu, a jak na prawdziwej klawiaturze trzymany klawisz chodu powtarza się po chwili - gry,
//! które czytają zdarzenia zamiast stanu klawisza, też widzą ciągły ruch.

use super::InputError;
use super::keys::Key;
use super::system;

pub const INPUT_KEYBOARD: u32 = 1;
pub const KEYEVENTF_EXTENDEDKEY: u32 = 0x0001;
pub const KEYEVENTF_KEYUP: u32 = 0x0002;
pub const KEYEVENTF_SCANCODE: u32 = 0x0008;
/// Znacznik w `dwExtraInfo`, po którym da się rozpoznać nasze wstrzyknięte klawisze.
pub const INJECT_TAG: usize = 0xB1E5;

/// Jak domyślne opóźnienie powtarzania w Windows.
pub const REPEAT_DELAY_S: f64 = 0.35;
/// Ok. 20 powtórzeń na sekundę.
pub const REPEAT_EVERY_S: f64 = 0.05;

/// Zawartość zdarzenia klawiatury dla SendInput (pola KEYBDINPUT; wVk zawsze 0).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyEvent {
    pub scan: u16,
    pub flags: u32,
    pub extra: usize,
}

impl KeyEvent {
    /// Naciśnięcie albo puszczenie klawisza po kodzie skanowania.
    pub fn new(scan: u16, down: bool, extended: bool) -> KeyEvent {
        let mut flags = KEYEVENTF_SCANCODE | if down { 0 } else { KEYEVENTF_KEYUP };
        if extended {
            // strzałki, prawy Ctrl/Alt, Insert, Home...
            flags |= KEYEVENTF_EXTENDEDKEY;
        }
        KeyEvent {
            scan,
            flags,
            extra: INJECT_TAG,
        }
    }
}

/// Wysyłanie klawiszy - prawdziwe ([`SystemSender`]) albo udawane w testach.
pub trait KeySender: Send {
    fn send(&mut self, scan: u16, down: bool, extended: bool) -> Result<(), InputError>;
}

/// Klawisze przez SendInput (poza Windows - nic).
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemSender;

impl KeySender for SystemSender {
    fn send(&mut self, scan: u16, down: bool, extended: bool) -> Result<(), InputError> {
        system::send_scancode(scan, down, extended)
    }
}

/// Co robimy z wyliczonym sterowaniem (wywoływane co klatkę).
pub trait Output: Send {
    /// Ustawia stan wyjścia: chód, bieg (bieg naciska też chód) i prędkość do logu.
    fn apply(&mut self, walk: bool, run: bool, speed: f64);
    /// Puszcza wszystko (pauza, błąd, wyjście).
    fn release_all(&mut self);
}

/// Wyjście po nazwie z konfiguracji: "dry" albo "keys".
pub fn create_output(kind: &str, walk: Key, run: Key) -> Result<Box<dyn Output>, InputError> {
    match kind {
        "dry" => Ok(Box::new(DryOutput::default())),
        "keys" => Ok(Box::new(KeyboardOutput::new(walk, run))),
        other => Err(InputError::UnknownOutput(other.to_string())),
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Which {
    Walk,
    Run,
}

/// Klawisze chodu i biegu naciskane przez [`KeySender`].
pub struct KeyboardOutput<S: KeySender = SystemSender> {
    pub walk: Key,
    pub run: Key,
    sender: S,
    clock: Box<dyn Fn() -> f64 + Send>,
    walk_down: bool,
    run_down: bool,
    walk_since: f64,
    last_repeat: f64,
    failing: bool,
}

impl KeyboardOutput<SystemSender> {
    /// Prawdziwe klawisze (SendInput) i zegar monotoniczny.
    pub fn new(walk: Key, run: Key) -> KeyboardOutput<SystemSender> {
        KeyboardOutput::with_sender(walk, run, SystemSender, Box::new(system::monotonic_s))
    }
}

impl<S: KeySender> KeyboardOutput<S> {
    /// Własne wysyłanie i zegar (sekundy) - do testów.
    pub fn with_sender(walk: Key, run: Key, sender: S, clock: Box<dyn Fn() -> f64 + Send>) -> Self {
        KeyboardOutput {
            walk,
            run,
            sender,
            clock,
            walk_down: false,
            run_down: false,
            walk_since: 0.0,
            last_repeat: 0.0,
            failing: false,
        }
    }

    /// Dostęp do wysyłania (w testach - odczyt wysłanych zdarzeń).
    pub fn sender(&self) -> &S {
        &self.sender
    }

    fn key(&self, which: Which) -> &Key {
        match which {
            Which::Walk => &self.walk,
            Which::Run => &self.run,
        }
    }

    fn send(&mut self, which: Which, down: bool) -> bool {
        let (scan, extended) = {
            let k = self.key(which);
            (k.scan, k.extended)
        };
        if let Err(e) = self.sender.send(scan, down, extended) {
            // Np. ekran blokady, okno UAC albo Ctrl+Alt+Del - spróbujemy w następnej klatce.
            if !self.failing {
                log::warn!("{e}");
                self.failing = true;
            }
            return false;
        }
        if self.failing {
            log::info!("Naciskanie klawiszy znów działa");
            self.failing = false;
        }
        true
    }

    fn set(&mut self, which: Which, down: bool) {
        let state = match which {
            Which::Walk => self.walk_down,
            Which::Run => self.run_down,
        };
        if state == down || !self.send(which, down) {
            return;
        }
        match which {
            Which::Walk => self.walk_down = down,
            Which::Run => self.run_down = down,
        }
        if which == Which::Walk && down {
            let now = (self.clock)();
            self.walk_since = now;
            self.last_repeat = now;
        }
    }

    fn repeat_walk(&mut self) {
        let now = (self.clock)();
        let held = now - self.walk_since;
        if held >= REPEAT_DELAY_S && now - self.last_repeat >= REPEAT_EVERY_S {
            self.send(Which::Walk, true); // kolejne „wciśnięcie” bez puszczania = powtórzenie
            self.last_repeat = now;
        }
    }

    /// Stan klawiszy: chód, bieg (bieg trzyma też klawisz chodu).
    pub fn set_keys(&mut self, walk: bool, run: bool) {
        if run {
            // Najpierw klawisz biegu, potem chodu - gra od razu widzi bieg.
            self.set(Which::Run, true);
            self.set(Which::Walk, true);
        } else if walk {
            self.set(Which::Run, false);
            self.set(Which::Walk, true);
        } else {
            self.set(Which::Walk, false);
            self.set(Which::Run, false);
        }
        if self.walk_down {
            self.repeat_walk();
        }
    }

    /// Puszcza oba klawisze, także gdy zapamiętany stan się rozjechał.
    pub fn release(&mut self) {
        self.set_keys(false, false);
        for which in [Which::Walk, Which::Run] {
            let (scan, extended, name) = {
                let k = self.key(which);
                (k.scan, k.extended, k.name.clone())
            };
            if let Err(e) = self.sender.send(scan, false, extended) {
                log::warn!("Nie udało się puścić klawisza {name}: {e}");
            }
        }
    }
}

impl<S: KeySender> Output for KeyboardOutput<S> {
    fn apply(&mut self, walk: bool, run: bool, _speed: f64) {
        self.set_keys(walk, run);
    }

    fn release_all(&mut self) {
        self.release();
    }
}

/// Wyjście „na sucho”: niczego nie naciska, tylko zapisuje zmiany stanu do logu.
#[derive(Debug, Default)]
pub struct DryOutput {
    state: (bool, bool),
}

impl Output for DryOutput {
    fn apply(&mut self, walk: bool, run: bool, speed: f64) {
        if (walk, run) != self.state {
            let py = |b: bool| if b { "True" } else { "False" };
            log::info!("[dry] W={} Shift={} ({speed:.1} km/h)", py(walk), py(run));
            self.state = (walk, run);
        }
    }

    fn release_all(&mut self) {
        self.apply(false, false, 0.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    type Sent = Arc<Mutex<Vec<(u16, bool, bool)>>>;

    struct Fake {
        sent: Sent,
        fail_first: usize,
        calls: usize,
    }

    impl KeySender for Fake {
        fn send(&mut self, scan: u16, down: bool, extended: bool) -> Result<(), InputError> {
            self.calls += 1;
            if self.calls <= self.fail_first {
                return Err(InputError::SendRejected("ekran blokady".into()));
            }
            self.sent.lock().unwrap().push((scan, down, extended));
            Ok(())
        }
    }

    fn output(
        walk: Key,
        run: Key,
        fail_first: usize,
        now: Arc<Mutex<f64>>,
    ) -> (KeyboardOutput<Fake>, Sent) {
        let sent: Sent = Arc::default();
        let fake = Fake {
            sent: sent.clone(),
            fail_first,
            calls: 0,
        };
        (
            KeyboardOutput::with_sender(walk, run, fake, Box::new(move || *now.lock().unwrap())),
            sent,
        )
    }

    const W: u16 = 0x11;
    const SHIFT: u16 = 0x2A;

    #[test]
    fn key_event_carries_the_key() {
        let e = KeyEvent::new(0x11, true, false);
        assert_eq!(
            (e.scan, e.flags, e.extra),
            (0x11, KEYEVENTF_SCANCODE, INJECT_TAG)
        );
        let up = KeyEvent::new(0x48, false, true);
        assert_eq!(up.scan, 0x48);
        assert_eq!(
            up.flags,
            KEYEVENTF_SCANCODE | KEYEVENTF_KEYUP | KEYEVENTF_EXTENDEDKEY
        );
    }

    #[test]
    fn keys_are_sent_only_on_change() {
        let (mut out, sent) = output(Key::walk_default(), Key::run_default(), 0, Arc::default());
        out.set_keys(true, false);
        out.set_keys(true, false);
        out.set_keys(true, true);
        out.set_keys(false, false);
        assert_eq!(
            *sent.lock().unwrap(),
            [
                (W, true, false),
                (SHIFT, true, false),
                (W, false, false),
                (SHIFT, false, false)
            ]
        );
    }

    #[test]
    fn custom_keys_arrow_up_and_ctrl() {
        let up = Key::new(0x48, true, "Strzałka w górę", 0);
        let ctrl = Key::new(0x1D, false, "Ctrl", 0);
        let (mut out, sent) = output(up, ctrl, 0, Arc::default());
        out.set_keys(true, true);
        assert_eq!(
            *sent.lock().unwrap(),
            [(0x1D, true, false), (0x48, true, true)]
        );
        sent.lock().unwrap().clear();
        out.release_all();
        let s = sent.lock().unwrap();
        assert!(s.contains(&(0x48, false, true)) && s.contains(&(0x1D, false, false)));
    }

    #[test]
    fn rejected_key_is_retried_instead_of_crashing() {
        let (mut out, sent) = output(Key::walk_default(), Key::run_default(), 1, Arc::default());
        out.set_keys(true, false); // odrzucone
        out.set_keys(true, false); // ponowiona próba
        assert_eq!(out.sender().calls, 2);
        assert_eq!(*sent.lock().unwrap(), [(W, true, false)]);
    }

    #[test]
    fn held_walk_key_repeats_like_a_real_keyboard() {
        let now = Arc::new(Mutex::new(0.0));
        let (mut out, sent) = output(Key::walk_default(), Key::run_default(), 0, now.clone());
        for step in 0..20 {
            *now.lock().unwrap() = f64::from(step) * 0.05;
            out.set_keys(true, false);
        }
        {
            let s = sent.lock().unwrap();
            assert!(s.iter().filter(|e| **e == (W, true, false)).count() > 5);
            assert!(s.iter().all(|e| e.1));
        }
        out.set_keys(false, false);
        assert_eq!(sent.lock().unwrap().last(), Some(&(W, false, false)));
    }

    #[test]
    fn outputs_by_name() {
        assert!(create_output("dry", Key::walk_default(), Key::run_default()).is_ok());
        assert!(create_output("keys", Key::walk_default(), Key::run_default()).is_ok());
        let err = create_output("pad", Key::walk_default(), Key::run_default())
            .err()
            .map(|e| e.to_string());
        assert_eq!(
            err.as_deref(),
            Some("Nieznane wyjście 'pad' (dozwolone: dry, keys)")
        );
        let mut dry = DryOutput::default();
        dry.apply(true, false, 3.0);
        dry.release_all();
        assert_eq!(dry.state, (false, false));
    }
}
