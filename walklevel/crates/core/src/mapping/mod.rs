//! Sterowanie: prędkość taśmy -> klawisze chodu i biegu oraz gałka pada (tryb Chodzenie),
//! a w drugą stronę klawisze, pad i poziomy treningu -> prędkość taśmy (tryby Klawisze, Pad,
//! Trening). Czysta logika bez Win32 - port bieznia/mapping.py, bieznia/engine.py i części
//! bieznia/gui/ekrany/sterowanie.py oraz bieznia/pad.py (skoki gałki).
//!
//! - [`SpeedMapper`] - prędkość -> [`Control`] (progi, histereza biegu, szybkie zatrzymanie),
//! - [`Bridge`] - silnik jednej klatki: odczyt -> mapowanie -> [`Output`], liczniki, historia,
//! - [`BeltSteering`] - tryby, w których gracz ustawia prędkość taśmy (polecenia [`BeltCommand`]).
//!
//! Naciskanie klawiszy (SendInput) realizuje moduł `input` przez trait [`Output`].

mod engine;
mod speed;
mod steering;

pub use crate::py::round_tenth;
pub use engine::{
    Bridge, ControlSource, DryOutput, HISTORY_LEN, HISTORY_S, Output, OutputKind, Reading,
    Snapshot, control_log_line,
};
pub use speed::{Control, MappingConfig, MappingError, STOPPED_KMH, SpeedMapper};
pub use steering::{
    BeltCommand, BeltSteering, Direction, Frame, FrameInput, Held, KEY_HOLD_MAX_S, KeyIdle,
    PAD_SETTLE_S, PAD_STOP_SETTLE_S, PadIdle, PadStep, STEP_HYSTERESIS, SteeringSettings,
    StepPicker, TRAINING_LEVELS, clamp_training_level, pad_preset, parse_key_hold, parse_pad_steps,
    parse_training_level, parse_training_levels, step_training_level,
};
