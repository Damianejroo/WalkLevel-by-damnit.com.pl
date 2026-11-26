//! Tryby, w których gracz ustawia prędkość taśmy: Klawisze (trzymany chód/bieg), Pad
//! (skoki wychylenia gałki) i Trening (4 poziomy). W trybie Chodzenie jest odwrotnie - taśma
//! steruje postacią przez [`super::Bridge`]. Port bieznia/gui/ekrany/sterowanie.py (bez
//! okna) i skoków z bieznia/pad.py.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::engine::OutputKind;
use crate::py::{py_float, py_int, round_tenth};

/// Gałka w drodze przez skoki - wysyłamy prędkość dopiero, gdy skok trzyma się tyle sekund.
pub const PAD_SETTLE_S: f64 = 0.15;
/// Gałka przechodzi przez środek przy zmianie kierunku - stop dopiero po tylu sekundach.
pub const PAD_STOP_SETTLE_S: f64 = 0.4;
/// Przy cofaniu gałki mały zapas (ułamek wychylenia), żeby skok nie migał.
pub const STEP_HYSTERESIS: f64 = 0.04;
/// Domyślne poziomy treningu (km/h).
pub const TRAINING_LEVELS: [f64; 4] = [1.5, 2.5, 3.5, 4.5];

const STEPS_MIN: usize = 2;
const STEPS_MAX: usize = 5;
/// Skoki muszą różnić się co najmniej o tyle punktów procentowych.
const MIN_GAP: u32 = 5;

/// Kierunek sterowania (ustawienie "direction").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    /// Klawisze: trzymany chód/bieg ustawia prędkość taśmy.
    #[default]
    Keyboard,
    /// Pad: skok wychylenia gałki ustawia prędkość taśmy.
    Gamepad,
    /// Chodzenie: ruch taśmy = klawisze w grze.
    Treadmill,
    /// Trening: 4 poziomy prędkości, bez gry.
    Training,
}

impl Direction {
    /// Wartość z ustawień; nieznana -> Klawisze (jak `normalize` w 0.5.x).
    pub fn from_setting(value: &str) -> Self {
        match value {
            "gamepad" => Self::Gamepad,
            "treadmill" => Self::Treadmill,
            "training" => Self::Training,
            _ => Self::Keyboard,
        }
    }

    /// Wartość do zapisu w ustawieniach.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Keyboard => "keyboard",
            Self::Gamepad => "gamepad",
            Self::Treadmill => "treadmill",
            Self::Training => "training",
        }
    }

    /// Klawisze, pad albo trening ustawiają prędkość taśmy (a nie odwrotnie).
    pub fn sets_speed(self) -> bool {
        self != Self::Treadmill
    }

    /// Wyjście mostu w tym trybie: gdy gracz ustawia prędkość, aplikacja niczego nie naciska.
    pub fn bridge_output(self, user_output: OutputKind) -> OutputKind {
        if self.sets_speed() {
            OutputKind::Dry
        } else {
            user_output
        }
    }

    /// Tryb sesji w historii: "trening" albo "gra".
    pub fn session_mode(self) -> &'static str {
        if self == Self::Training {
            "trening"
        } else {
            "gra"
        }
    }
}

/// Co robi taśma przy puszczonej gałce (ustawienie "pad_idle").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PadIdle {
    /// taśma staje, ruch gałki ją budzi
    #[default]
    Stop,
    /// prędkość skoku 1
    Slow,
    /// 0 km/h bez zatrzymania (tryb testowy)
    Zero,
}

impl PadIdle {
    /// Wartość z ustawień; nieznana -> stop.
    pub fn from_setting(value: &str) -> Self {
        match value {
            "slow" => Self::Slow,
            "zero" => Self::Zero,
            _ => Self::Stop,
        }
    }

    /// Wartość do zapisu w ustawieniach.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Stop => "stop",
            Self::Slow => "slow",
            Self::Zero => "zero",
        }
    }
}

/// Strefa wskaźnika: bez ruchu, chód albo bieg.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Held {
    /// bez ruchu (skok 1 pada, brak klawisza)
    Idle,
    /// chód
    Walk,
    /// bieg (ostatni skok pada, chód + bieg, poziomy 3-4)
    Run,
}

impl Held {
    /// Nazwa strefy jak w 0.5.x ("idle", "walk", "run").
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Walk => "walk",
            Self::Run => "run",
        }
    }
}

/// Skok gałki: (wychylenie od, w %; prędkość taśmy, km/h). W JSON: `[25, 1.5]`.
pub type PadStep = (u32, f64);

/// Gotowe skoki dla 2-5 skoków (pierwszy zawsze od 0% = gałka puszczona).
pub fn pad_preset(count: usize) -> Option<Vec<PadStep>> {
    let steps: &[PadStep] = match count {
        2 => &[(0, 1.0), (100, 2.5)],
        3 => &[(0, 1.0), (50, 2.0), (100, 3.0)],
        4 => &[(0, 1.0), (25, 1.5), (50, 2.5), (100, 4.0)],
        5 => &[(0, 1.0), (25, 1.5), (50, 2.5), (75, 3.5), (100, 4.5)],
        _ => return None,
    };
    Some(steps.to_vec())
}

fn default_steps() -> Vec<PadStep> {
    vec![(0, 1.0), (25, 1.5), (50, 2.5), (100, 4.0)]
}

/// Skoki z ustawień ("pad_steps"); błędne - gotowe 4 skoki (`pad.parse_steps`).
pub fn parse_pad_steps(data: &Value) -> Vec<PadStep> {
    try_parse_steps(data).unwrap_or_else(default_steps)
}

fn try_parse_steps(data: &Value) -> Option<Vec<PadStep>> {
    let mut parsed: Vec<(i64, f64)> = Vec::new();
    for item in data.as_array()? {
        let [at, speed] = item.as_array()?.as_slice() else {
            return None;
        };
        parsed.push((py_int(at)?, round_tenth(py_float(speed)?)));
    }
    if !(STEPS_MIN..=STEPS_MAX).contains(&parsed.len()) || parsed[0].0 != 0 {
        return None;
    }
    for pair in parsed.windows(2) {
        let (a, b) = (pair[0].0, pair[1].0);
        if b - a < i64::from(MIN_GAP) || b > 100 {
            return None;
        }
    }
    if parsed
        .iter()
        .any(|&(_, speed)| !(0.5..=12.0).contains(&speed))
    {
        return None;
    }
    // po sprawdzeniu: 0 = pierwszy, kolejne rosną do najwyżej 100
    parsed
        .into_iter()
        .map(|(at, speed)| Some((u32::try_from(at).ok()?, speed)))
        .collect()
}

/// Który skok gałki teraz obowiązuje (z zapasem przy cofaniu gałki).
#[derive(Debug, Clone, Default)]
pub struct StepPicker {
    index: usize,
}

impl StepPicker {
    /// Ostatnio wybrany skok (indeks od 0).
    pub fn index(&self) -> usize {
        self.index
    }

    /// Nowe wychylenie 0..1; zwraca indeks skoku. Puste `steps` -> 0.
    pub fn update(&mut self, steps: &[PadStep], deflection: f64) -> usize {
        let reached = |margin: f64| {
            steps
                .iter()
                .enumerate()
                .filter(|(_, (at, _))| deflection >= f64::from(*at) / 100.0 - margin)
                .map(|(i, _)| i)
                .max()
                .unwrap_or(0)
        };
        self.index = self.index.min(steps.len().saturating_sub(1));
        let up = reached(0.0);
        if up >= self.index {
            self.index = up;
        } else {
            self.index = self.index.min(reached(STEP_HYSTERESIS));
        }
        self.index
    }
}

/// Cztery poziomy treningu w km/h z ustawień; błędny zapis -> [`TRAINING_LEVELS`].
pub fn parse_training_levels(data: &Value) -> [f64; 4] {
    let parsed: Option<Vec<f64>> = data
        .as_array()
        .and_then(|a| a.iter().map(|v| py_float(v).map(round_tenth)).collect());
    match parsed.as_deref() {
        Some(&[a, b, c, d]) if [a, b, c, d].iter().all(|v| (0.5..=20.0).contains(v)) => {
            [a, b, c, d]
        }
        _ => TRAINING_LEVELS,
    }
}

/// Wybrany poziom treningu z ustawień (0-3); błędny -> 0.
pub fn parse_training_level(data: &Value) -> usize {
    let level = match data {
        Value::Bool(b) => Some(i64::from(*b)),
        other => other.as_i64(),
    };
    match level {
        Some(l @ 0..=3) => l as usize,
        _ => 0,
    }
}

/// Klik w poziom: poziom przycięty do 0-3.
pub fn clamp_training_level(level: i64) -> usize {
    level.clamp(0, 3) as usize
}

/// Poziom w górę/w dół (skróty): nowy poziom albo `None`, gdy wyszedłby poza 1-4.
pub fn step_training_level(level: usize, step: i64) -> Option<usize> {
    let next = i64::try_from(level).ok()? + step;
    (0..=3).contains(&next).then_some(next as usize)
}

/// Ustawienia trybów sterujących taśmą (pola jak w ustawieniach 0.5.x).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SteeringSettings {
    pub direction: Direction,
    /// start i stop taśmy z aplikacji
    pub belt_control: bool,
    /// Klawisze: prędkość bez klawisza, z chodem i z chodem + biegiem
    pub idle_speed: f64,
    pub walk_speed: f64,
    pub run_speed: f64,
    pub pad_steps: Vec<PadStep>,
    pub pad_idle: PadIdle,
    pub training_levels: [f64; 4],
    pub training_level: usize,
}

impl Default for SteeringSettings {
    fn default() -> Self {
        Self {
            direction: Direction::Keyboard,
            belt_control: true,
            idle_speed: 1.0,
            walk_speed: 1.5,
            run_speed: 2.5,
            pad_steps: default_steps(),
            pad_idle: PadIdle::Stop,
            training_levels: TRAINING_LEVELS,
            training_level: 0,
        }
    }
}

impl SteeringSettings {
    /// Prędkość „bez ruchu”: wyłączenie sterowania bez stopu taśmy zwalnia do niej.
    pub fn idle_target(&self) -> f64 {
        if self.direction == Direction::Gamepad {
            if self.pad_idle == PadIdle::Zero {
                return 0.0;
            }
            return self.pad_steps.first().map_or(self.idle_speed, |s| s.1);
        }
        self.idle_speed
    }

    /// Pad z puszczoną gałką = stop: taśmę uruchamia dopiero ruch gałki.
    pub fn pad_stops_belt(&self) -> bool {
        self.direction == Direction::Gamepad && self.pad_idle == PadIdle::Stop
    }

    /// Prędkość z trzymanych klawiszy: chód + bieg, sam chód albo nic (sam bieg to nie bieg).
    pub fn keyboard_target(&self, walk_down: bool, run_down: bool) -> (f64, Held) {
        match (walk_down, run_down) {
            (true, true) => (self.run_speed, Held::Run),
            (true, false) => (self.walk_speed, Held::Walk),
            _ => (self.idle_speed, Held::Idle),
        }
    }

    /// Prędkość z wybranego poziomu (1-4). Kolor wskaźnika: 1-2 chód, 3-4 bieg.
    pub fn training_target(&self) -> (f64, Held) {
        let level = self.training_level.min(3);
        (
            self.training_levels[level],
            if level < 2 { Held::Walk } else { Held::Run },
        )
    }
}

/// Polecenie dla bieżni (wykonuje je moduł treadmill).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum BeltCommand {
    /// uruchom taśmę (czy już jedzie, sprawdza sterownik)
    Start,
    /// zatrzymaj taśmę
    Stop,
    /// Prędkość docelowa (nie zaokrąglona - sterownik zaokrągla do 0,1 km/h).
    /// `start_if_stopped`: stojącą taśmę najpierw uruchom (ruch gałki budzi bieżnię).
    #[serde(rename_all = "camelCase")]
    SetSpeed { kmh: f64, start_if_stopped: bool },
}

/// Wejście jednej klatki.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct FrameInput {
    /// zegar monotoniczny, sekundy
    pub now: f64,
    /// sterowanie włączone ([`super::Snapshot::enabled`])
    pub enabled: bool,
    /// właśnie wybierany jest klawisz - nic nie wysyłamy
    pub capturing: bool,
    /// trzymane klawisze chodu i biegu (tryb Klawisze)
    pub walk_down: bool,
    pub run_down: bool,
    /// wychylenie gałki 0..1 (0, gdy nie ma pada)
    pub deflection: f64,
    /// prędkość taśmy z bieżni (świeży odczyt), None - brak danych
    pub belt_kmh: Option<f64>,
    /// zakres prędkości bieżni (min, max); (0, 0) - nieznany
    pub belt_range: (f64, f64),
}

/// Trening: po ilu sekundach niezgodności prędkości taśmy z poziomem wysłać ją jeszcze raz
/// (bieżnia gubi polecenie wysłane w trakcie hamowania albo rozruchu).
pub const RESEND_AFTER_S: f64 = 3.0;
/// Różnica prędkości (km/h) uznawana za niezgodność (odczyt ma rozdzielczość 0,1).
pub const RESEND_TOLERANCE_KMH: f64 = 0.15;

/// Wynik klatki: cel taśmy, strefa wskaźnika i ewentualne polecenie.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Frame {
    /// prędkość docelowa (Chodzenie: 0)
    pub target: f64,
    pub held: Held,
    /// bieżący skok gałki (indeks od 0)
    pub pad_step: usize,
    /// pad: skok trzyma się już chwilę (można wysłać); inne tryby zawsze true
    pub settled: bool,
    /// pad: gałka puszczona przy „stop” - taśma stoi
    pub pad_stopped: bool,
    pub command: Option<BeltCommand>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum BeltTarget {
    /// nic nie wysłane od zmiany (prędkość pójdzie od nowa)
    Unknown,
    /// taśma zatrzymana przez puszczoną gałkę
    Stopped,
    Speed(f64),
}

/// Pilnuje, co ostatnio wysłano na taśmę, żeby nie powtarzać poleceń co klatkę.
#[derive(Debug, Clone)]
pub struct BeltSteering {
    belt_target: BeltTarget,
    picker: StepPicker,
    pad_step: usize,
    pad_step_since: f64,
    /// od kiedy jadąca taśma ma inną prędkość niż wysłany poziom treningu
    mismatch_since: Option<f64>,
}

impl Default for BeltSteering {
    fn default() -> Self {
        Self::new()
    }
}

impl BeltSteering {
    /// Nic jeszcze nie wysłane, gałka puszczona.
    pub fn new() -> Self {
        Self {
            belt_target: BeltTarget::Unknown,
            picker: StepPicker::default(),
            pad_step: 0,
            pad_step_since: 0.0,
            mismatch_since: None,
        }
    }

    /// Trening: jadąca taśma od [`RESEND_AFTER_S`] ma inną prędkość niż wysłany poziom -
    /// zapominamy wysłaną prędkość, więc klatka wyśle ją jeszcze raz. Stojącej taśmy nie
    /// ruszamy (mogła ją zatrzymać osoba pilotem).
    fn resend_if_lost(&mut self, target: f64, input: &FrameInput) {
        let (min, max) = input.belt_range;
        let want = if max > min { target.clamp(min, max) } else { target };
        let lost = match (input.belt_kmh, self.belt_target) {
            (Some(kmh), BeltTarget::Speed(sent)) if sent == target && kmh > 0.05 => {
                (kmh - want).abs() > RESEND_TOLERANCE_KMH
            }
            _ => false,
        };
        if !lost {
            self.mismatch_since = None;
            return;
        }
        let since = *self.mismatch_since.get_or_insert(input.now);
        if input.now - since >= RESEND_AFTER_S {
            self.mismatch_since = None;
            self.belt_target = BeltTarget::Unknown;
        }
    }

    /// Bieżący skok gałki (indeks od 0).
    pub fn pad_step(&self) -> usize {
        self.pad_step
    }

    /// Zapomina wysłaną prędkość - następna klatka wyśle ją od nowa (zmiana trybu,
    /// nowa prędkość poziomu treningu).
    pub fn forget_target(&mut self) {
        self.belt_target = BeltTarget::Unknown;
    }

    fn sent(&self, target: f64) -> bool {
        matches!(self.belt_target, BeltTarget::Speed(s) if s == target)
    }

    /// Włączenie/wyłączenie sterowania (skrót, przełącznik, przycisk). `was_enabled` - stan
    /// sprzed zmiany; `belt_allowed` - bieżnia przyjmuje start i stop (możliwości sterownika).
    /// Wyłączenie: najpierw klawisze puszcza [`super::Bridge`], potem te polecenia.
    pub fn set_control(
        &mut self,
        settings: &SteeringSettings,
        was_enabled: bool,
        on: bool,
        belt_allowed: bool,
    ) -> Vec<BeltCommand> {
        let changed = on != was_enabled;
        self.belt_target = BeltTarget::Unknown; // po zmianie prędkość idzie od nowa
        let commands_possible = settings.belt_control && belt_allowed;
        let pad_stop = settings.pad_stops_belt(); // taśmę uruchamia dopiero ruch gałki
        let training = settings.direction == Direction::Training; // trening zawsze steruje
        let mut out = Vec::new();
        if changed && !on {
            if commands_possible || pad_stop || training {
                out.push(BeltCommand::Stop);
            } else if settings.direction.sets_speed() {
                out.push(BeltCommand::SetSpeed {
                    kmh: settings.idle_target(),
                    start_if_stopped: false,
                });
            }
        }
        if changed && on && (commands_possible || training) && !pad_stop {
            out.push(BeltCommand::Start); // czy taśma już jedzie, sprawdza sterownik
        }
        out
    }

    fn pad_target(
        &mut self,
        settings: &SteeringSettings,
        input: &FrameInput,
    ) -> (f64, Held, bool, bool) {
        let steps = &settings.pad_steps;
        let index = self.picker.update(steps, input.deflection);
        if index != self.pad_step {
            self.pad_step = index;
            self.pad_step_since = input.now;
        }
        let idle_text = index == 0 && settings.pad_idle != PadIdle::Slow;
        let speed = if idle_text {
            0.0
        } else {
            steps.get(index).map_or(0.0, |s| s.1)
        };
        let held = if index == 0 {
            Held::Idle
        } else if index + 1 == steps.len() {
            Held::Run
        } else {
            Held::Walk
        };
        let stopped = index == 0 && settings.pad_idle == PadIdle::Stop;
        let wait = if stopped {
            PAD_STOP_SETTLE_S
        } else {
            PAD_SETTLE_S
        };
        (
            speed,
            held,
            input.now - self.pad_step_since >= wait,
            stopped,
        )
    }

    fn send_pad(&mut self, settings: &SteeringSettings, target: f64) -> Option<BeltCommand> {
        if self.pad_step == 0 && settings.pad_idle == PadIdle::Stop {
            if self.belt_target == BeltTarget::Stopped {
                return None;
            }
            self.belt_target = BeltTarget::Stopped;
            return Some(BeltCommand::Stop);
        }
        if self.sent(target) {
            return None;
        }
        self.belt_target = BeltTarget::Speed(target);
        let wake = target > 0.0 && (settings.pad_stops_belt() || settings.belt_control);
        Some(BeltCommand::SetSpeed {
            kmh: target,
            start_if_stopped: wake,
        })
    }

    /// Jedna klatka: cel taśmy z trybu i polecenie, jeśli trzeba coś wysłać.
    pub fn frame(&mut self, settings: &SteeringSettings, input: &FrameInput) -> Frame {
        let direction = settings.direction;
        let (target, held, settled, pad_stopped) = match direction {
            Direction::Gamepad => self.pad_target(settings, input),
            Direction::Keyboard => {
                let (t, h) = settings.keyboard_target(input.walk_down, input.run_down);
                (t, h, true, false)
            }
            Direction::Training => {
                let (t, h) = settings.training_target();
                (t, h, true, false)
            }
            Direction::Treadmill => (0.0, Held::Idle, true, false),
        };
        let sends =
            direction != Direction::Treadmill && input.enabled && settled && !input.capturing;
        if sends && direction == Direction::Training {
            self.resend_if_lost(target, input);
        }
        let command = if sends && direction == Direction::Gamepad {
            self.send_pad(settings, target)
        } else if sends && !self.sent(target) {
            self.belt_target = BeltTarget::Speed(target);
            Some(BeltCommand::SetSpeed {
                kmh: target,
                start_if_stopped: false,
            })
        } else {
            None
        };
        Frame {
            target,
            held,
            pad_step: self.pad_step,
            settled,
            pad_stopped,
            command,
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn speed(kmh: f64) -> Option<BeltCommand> {
        Some(BeltCommand::SetSpeed {
            kmh,
            start_if_stopped: false,
        })
    }

    fn at(now: f64) -> FrameInput {
        FrameInput {
            now,
            enabled: true,
            ..FrameInput::default()
        }
    }

    #[test]
    fn saved_steps_are_validated() {
        assert_eq!(
            parse_pad_steps(&json!([[0, 1.0], [25, 1.5], [50, 2.5], [100, 4.0]])),
            default_steps()
        );
        assert_eq!(
            parse_pad_steps(&json!([[0, 1], [60, 3]])),
            vec![(0, 1.0), (60, 3.0)]
        );
        for bad in [
            json!(null),
            json!([[0, 1.0]]),
            json!([
                [0, 1.0],
                [5, 1.0],
                [10, 1.0],
                [15, 1.0],
                [20, 1.0],
                [25, 1.0]
            ]),
            json!([[10, 1.0], [50, 2.0]]),
            json!([[0, 1.0], [52, 2.0], [50, 3.0]]),
            json!([[0, 1.0], [50, 20.0]]),
            json!("zepsute"),
        ] {
            assert_eq!(parse_pad_steps(&bad), default_steps(), "{bad}");
        }
        for n in 2..=5 {
            let preset = pad_preset(n).expect("gotowe skoki");
            let as_json = serde_json::to_value(&preset).expect("JSON");
            assert_eq!(parse_pad_steps(&as_json), preset);
        }
        assert_eq!(pad_preset(6), None);
    }

    #[test]
    fn step_picker_follows_stick_with_hysteresis_on_the_way_back() {
        let steps = default_steps(); // 0 / 25 / 50 / 100 %
        let mut p = StepPicker::default();
        for (d, want) in [
            (0.0, 0),
            (0.24, 0),
            (0.30, 1),
            (0.55, 2),
            (1.0, 3),
            (0.98, 3), // tuż pod końcem - bez migania
            (0.90, 2),
            (0.48, 2), // w zapasie
            (0.40, 1),
            (0.0, 0),
        ] {
            assert_eq!(p.update(&steps, d), want, "wychylenie {d}");
        }
        p.update(&steps, 1.0);
        let two = pad_preset(2).expect("2 skoki");
        assert_eq!(p.update(&two, 1.0), 1); // mniej skoków po zmianie w ustawieniach
        assert_eq!(p.update(&[], 1.0), 0);
    }

    #[test]
    fn training_levels_are_validated() {
        assert_eq!(
            parse_training_levels(&json!([1, 2, 3, 4])),
            [1.0, 2.0, 3.0, 4.0]
        );
        assert_eq!(parse_training_levels(&json!([1, 2, 3])), TRAINING_LEVELS);
        assert_eq!(
            parse_training_levels(&json!([0.4, 2, 3, 4])),
            TRAINING_LEVELS
        );
        assert_eq!(parse_training_level(&json!(2)), 2);
        assert_eq!(parse_training_level(&json!(true)), 1);
        assert_eq!(parse_training_level(&json!(2.0)), 0);
        assert_eq!(parse_training_level(&json!(4)), 0);
        assert_eq!(clamp_training_level(9), 3);
        assert_eq!(clamp_training_level(-1), 0);
        assert_eq!(step_training_level(3, 1), None);
        assert_eq!(step_training_level(0, -1), None);
        assert_eq!(step_training_level(1, 1), Some(2));
    }

    #[test]
    fn settings_parse_like_normalize() {
        assert_eq!(Direction::from_setting("pad"), Direction::Keyboard);
        assert_eq!(Direction::from_setting("training"), Direction::Training);
        assert_eq!(PadIdle::from_setting("zero"), PadIdle::Zero);
        assert_eq!(PadIdle::from_setting(""), PadIdle::Stop);
        assert_eq!(
            Direction::Treadmill.bridge_output(OutputKind::Keys),
            OutputKind::Keys
        );
        assert_eq!(
            Direction::Gamepad.bridge_output(OutputKind::Keys),
            OutputKind::Dry
        );
        assert_eq!(Direction::Training.session_mode(), "trening");
    }

    #[test]
    fn held_keys_set_belt_speed() {
        let mut s = SteeringSettings::default();
        let mut b = BeltSteering::new();
        let off = FrameInput {
            enabled: false,
            ..at(0.0)
        };
        assert_eq!(b.frame(&s, &off).command, None); // wyłączone - taśma w spokoju
        assert_eq!(
            b.set_control(&s, false, true, true),
            vec![BeltCommand::Start]
        );
        assert_eq!(b.frame(&s, &at(0.0)).command, speed(1.0)); // bez klawisza: jałowy
        assert_eq!(b.frame(&s, &at(0.05)).command, None); // wysłany raz
        let walk = FrameInput {
            walk_down: true,
            ..at(0.1)
        };
        assert_eq!(b.frame(&s, &walk).command, speed(1.5));
        let run = FrameInput {
            run_down: true,
            ..walk
        };
        assert_eq!(b.frame(&s, &run).command, speed(2.5));
        let shift = FrameInput {
            walk_down: false,
            ..run
        }; // sam Shift to nie bieg
        assert_eq!(b.frame(&s, &shift).command, speed(1.0));
        s.walk_speed = 1.6;
        assert_eq!(b.frame(&s, &walk).command, speed(1.6));
        assert_eq!(
            b.set_control(&s, true, false, true),
            vec![BeltCommand::Stop]
        );
    }

    #[test]
    fn without_belt_control_turning_off_slows_down() {
        let s = SteeringSettings {
            belt_control: false,
            ..SteeringSettings::default()
        };
        let mut b = BeltSteering::new();
        assert!(b.set_control(&s, false, true, true).is_empty());
        let walk = FrameInput {
            walk_down: true,
            ..at(0.0)
        };
        assert_eq!(b.frame(&s, &walk).command, speed(1.5));
        assert_eq!(
            b.set_control(&s, true, false, true),
            vec![BeltCommand::SetSpeed {
                kmh: 1.0,
                start_if_stopped: false
            }]
        );
    }

    #[test]
    fn treadmill_direction_never_sets_speed() {
        let s = SteeringSettings {
            direction: Direction::Treadmill,
            ..SteeringSettings::default()
        };
        let mut b = BeltSteering::new();
        let keys = FrameInput {
            walk_down: true,
            run_down: true,
            ..at(0.0)
        };
        assert_eq!(b.frame(&s, &keys).command, None);
    }

    #[test]
    fn released_stick_stops_belt_and_movement_wakes_it() {
        let s = SteeringSettings {
            direction: Direction::Gamepad,
            ..SteeringSettings::default()
        };
        let mut b = BeltSteering::new();
        assert!(b.set_control(&s, false, true, true).is_empty()); // F8 nie rusza taśmy
        assert_eq!(b.frame(&s, &at(1.0)).command, Some(BeltCommand::Stop));
        assert_eq!(b.frame(&s, &at(2.0)).command, None); // bez powtarzania co klatkę
        let stick = FrameInput {
            deflection: 0.37,
            ..at(3.0)
        };
        assert_eq!(b.frame(&s, &stick).command, None); // w drodze - jeszcze nic
        let held = FrameInput { now: 3.2, ..stick };
        assert_eq!(
            b.frame(&s, &held).command,
            Some(BeltCommand::SetSpeed {
                kmh: 1.5,
                start_if_stopped: true
            })
        );
        assert_eq!(b.frame(&s, &at(3.3)).command, None); // krótko przez środek - nie stop
        assert_eq!(b.frame(&s, &at(3.8)).command, Some(BeltCommand::Stop));
    }

    #[test]
    fn training_level_sets_speed_and_stop_turns_off() {
        let mut s = SteeringSettings {
            direction: Direction::Training,
            ..SteeringSettings::default()
        };
        let mut b = BeltSteering::new();
        s.training_level = clamp_training_level(1);
        assert_eq!(
            b.set_control(&s, false, true, false),
            vec![BeltCommand::Start]
        );
        let f = b.frame(&s, &at(0.0));
        assert_eq!((f.command, f.held), (speed(2.5), Held::Walk));
        s.training_level = step_training_level(s.training_level, 1).expect("poziom 3");
        let f = b.frame(&s, &at(0.1));
        assert_eq!((f.command, f.held), (speed(3.5), Held::Run));
        assert_eq!(
            b.set_control(&s, true, false, false),
            vec![BeltCommand::Stop]
        );
    }

    #[test]
    fn training_resends_level_lost_by_belt() {
        let s = SteeringSettings {
            direction: Direction::Training,
            training_level: clamp_training_level(0),
            ..SteeringSettings::default()
        };
        let mut b = BeltSteering::new();
        let biegnie = |now: f64, kmh: f64| FrameInput {
            belt_kmh: Some(kmh),
            belt_range: (0.5, 6.0),
            ..at(now)
        };
        let poziom = b.frame(&s, &at(0.0)).command;
        let Some(BeltCommand::SetSpeed { kmh: cel, .. }) = poziom else {
            panic!("pierwsza klatka wysyła poziom");
        };
        // taśma zgubiła polecenie (np. w trakcie hamowania) i jedzie inną prędkością
        let inna = cel + 1.0;
        assert_eq!(b.frame(&s, &biegnie(1.0, inna)).command, None);
        assert_eq!(b.frame(&s, &biegnie(3.9, inna)).command, None);
        assert_eq!(b.frame(&s, &biegnie(4.0, inna)).command, speed(cel));
        // zgodna prędkość, stojąca taśma albo brak danych - nic nie powtarzamy
        assert_eq!(b.frame(&s, &biegnie(10.0, cel)).command, None);
        assert_eq!(b.frame(&s, &biegnie(20.0, 0.0)).command, None);
        assert_eq!(b.frame(&s, &at(30.0)).command, None);
    }

    #[test]
    fn belt_command_json() {
        let c = BeltCommand::SetSpeed {
            kmh: 1.5,
            start_if_stopped: true,
        };
        let v = serde_json::to_value(c).expect("JSON");
        assert_eq!(
            v,
            json!({"kind": "setSpeed", "kmh": 1.5, "startIfStopped": true})
        );
    }
}
