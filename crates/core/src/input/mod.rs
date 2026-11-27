//! Wejście i wyjście sterowania: klawisze chodu i biegu (SendInput po kodach skanowania),
//! nazwy klawiszy i kombinacje, globalne skróty (F6/F7/F8 i Ctrl+Shift+F8), odczyt pada
//! (XInput, winmm, DualSense/DualShock 4 przez HID). Port bieznia/klawisze.py, pad.py,
//! hotkeys.py, outputs/ i części winapi.py.
//!
//! Logika (walidacja klawiszy, skoki gałki, raporty pada, powtarzanie klawisza) nie zależy
//! od systemu i ma testy. Windows jest za `cfg(windows)` w [`system`] i w odczycie pada;
//! poza Windows funkcje systemowe nic nie robią (jak w Pythonie).
//!
//! Typowe użycie w aplikacji:
//! - wybór klawisza w oknie: [`keys::key_from_code`] (z `KeyboardEvent.code`/`key`) +
//!   [`keys::mods_from_flags`], sprawdzenie [`keys::problem_with`] (szybciej/wolniej:
//!   [`keys::speed_problem`]), zapis [`Key::to_value`], odczyt [`Key::from_value`]
//!   (szybciej/wolniej: [`Key::speed_from_value`]),
//! - sterowanie: [`KeyboardOutput::set_keys`] co klatkę, [`KeyboardOutput::release`] przy pauzie,
//! - skróty: [`Hotkeys`] + [`HotkeyWatcher::spawn`],
//! - pad: [`PadReader::system`] co klatkę, [`PadState::deflection`] + [`StepPicker`].

pub mod hotkeys;
pub mod keys;
pub mod pad;
pub mod send;
pub mod system;

#[cfg(windows)]
mod pad_win;

pub use hotkeys::{Binding, HotkeyWatcher, Hotkeys, KeyState, SystemKeys};
pub use keys::{
    Key, Mod, combo_problem, key_from_code, key_from_native, problem, problem_with, speed_problem,
};
pub use pad::{PadReader, PadSource, PadState, PadStep, StepPicker, button_label, parse_steps};
pub use send::{DryOutput, KeySender, KeyboardOutput, Output, SystemSender, create_output};

/// Błędy modułu wejścia.
#[derive(Debug, thiserror::Error)]
pub enum InputError {
    /// Windows odrzucił SendInput (ekran blokady, okno UAC, Ctrl+Alt+Del).
    #[error("Windows odrzucił naciśnięcie klawisza: {0}")]
    SendRejected(String),
    /// Zły skrót w konfiguracji (nieznana nazwa albo kod poza zakresem).
    #[error("{0}")]
    BadHotkey(String),
    /// Nieznany rodzaj wyjścia (dozwolone: dry, keys).
    #[error("Nieznane wyjście '{0}' (dozwolone: dry, keys)")]
    UnknownOutput(String),
    /// Nie udało się uruchomić wątku (skróty).
    #[error("Nie udało się uruchomić wątku: {0}")]
    Thread(String),
}
