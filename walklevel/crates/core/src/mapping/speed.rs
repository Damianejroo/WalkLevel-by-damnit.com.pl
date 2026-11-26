//! Prędkość taśmy -> chód (W), bieg (Shift+W) i wychylenie gałki - port bieznia/mapping.py.

use std::collections::VecDeque;

use serde::{Deserialize, Serialize};

use super::py::fsum;

/// Poniżej tej prędkości taśma stoi - postać nigdy nie rusza (nawet przy progu chodu 0).
pub const STOPPED_KMH: f64 = 0.05;

/// Błędne ustawienia sterowania.
#[derive(Debug, thiserror::Error)]
pub enum MappingError {
    #[error(
        "Wymagane: 0 <= stand_below < run_off <= run_on <= max_speed, jest: \
         stand_below={stand_below}, run_off={run_off}, run_on={run_on}, max_speed={max_speed}"
    )]
    Thresholds {
        stand_below: f64,
        run_off: f64,
        run_on: f64,
        max_speed: f64,
    },
    #[error("smoothing_samples musi być >= 1")]
    Smoothing,
    #[error("Błędne ustawienia sterowania: {0}")]
    Format(#[from] serde_json::Error),
}

/// Progi i zachowanie mapowania. Nazwy pól jak w config.toml i ustawieniach 0.5.x;
/// brakujące pola dostają wartości domyślne, nieznane są pomijane.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MappingConfig {
    /// poniżej - postać stoi (km/h)
    pub stand_below: f64,
    /// od tej prędkości bieg się włącza...
    pub run_on: f64,
    /// ...a wyłącza dopiero poniżej tej (histereza, żeby Shift nie migał)
    pub run_off: f64,
    /// prędkość pełnego wychylenia gałki
    pub max_speed: f64,
    /// ile ostatnich odczytów uśredniać (1 = bez wygładzania)
    pub smoothing_samples: usize,
    /// po tylu sekundach bez odczytu postać staje
    pub stale_after_s: f64,
    /// szybkie zatrzymanie: duży spadek prędkości = hamowanie, postać staje od razu
    pub quick_stop: bool,
    /// spadek na odczyt uznawany za hamowanie (pilot zmienia do 0,5 km/h, stop to 0,7-1,0)
    pub brake_drop: f64,
    /// blisko progu chodu (stand_below + brake_near) każdy spadek to zatrzymywanie
    pub brake_near: f64,
    /// ile sekund bez dalszego spadku, zanim uznamy, że to było tylko zwolnienie
    pub brake_hold_s: f64,
}

impl Default for MappingConfig {
    fn default() -> Self {
        Self {
            stand_below: 0.8,
            run_on: 6.3,
            run_off: 5.7,
            max_speed: 12.0,
            smoothing_samples: 1,
            stale_after_s: 3.0,
            quick_stop: true,
            brake_drop: 0.6,
            brake_near: 0.6,
            brake_hold_s: 3.0,
        }
    }
}

impl MappingConfig {
    /// Sprawdza progi jak `MappingConfig.__post_init__` w Pythonie.
    pub fn validate(&self) -> Result<(), MappingError> {
        let ok = 0.0 <= self.stand_below
            && self.stand_below < self.run_off
            && self.run_off <= self.run_on
            && self.run_on <= self.max_speed;
        if !ok {
            return Err(MappingError::Thresholds {
                stand_below: self.stand_below,
                run_off: self.run_off,
                run_on: self.run_on,
                max_speed: self.max_speed,
            });
        }
        if self.smoothing_samples < 1 {
            return Err(MappingError::Smoothing);
        }
        Ok(())
    }

    /// `MappingConfig.from_dict`: obiekt JSON (nieznane klucze pomijane) + sprawdzenie progów.
    pub fn from_value(data: &serde_json::Value) -> Result<Self, MappingError> {
        let config: Self = serde_json::from_value(data.clone())?;
        config.validate()?;
        Ok(config)
    }
}

/// Stan wyjścia w jednej klatce.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Control {
    /// wygładzona prędkość, km/h
    pub speed: f64,
    /// 0.0-1.0, wychylenie lewej gałki do przodu
    pub stick: f64,
    /// klawisz chodu (W)
    pub walk: bool,
    /// klawisz biegu (Shift, razem z chodem)
    pub run: bool,
}

impl Control {
    /// Nic nie wciśnięte.
    pub fn idle() -> Self {
        Self::default()
    }

    fn standing(speed: f64) -> Self {
        Self {
            speed,
            ..Self::default()
        }
    }
}

/// Zamiana kolejnych odczytów prędkości na [`Control`]. Pamięta stan biegu (histereza)
/// i hamowania, więc jeden obiekt na jedno źródło odczytów.
#[derive(Debug, Clone)]
pub struct SpeedMapper {
    config: MappingConfig,
    window: VecDeque<f64>,
    running: bool,
    last_raw: Option<f64>,
    braking: bool,
    last_fall: f64,
}

impl SpeedMapper {
    /// Nowe mapowanie. Ustawienia sprawdza [`MappingConfig::validate`]; okno wygładzania
    /// ma co najmniej 1 odczyt nawet przy błędnym `smoothing_samples`.
    pub fn new(config: MappingConfig) -> Self {
        Self {
            config,
            window: VecDeque::with_capacity(config.smoothing_samples.max(1)),
            running: false,
            last_raw: None,
            braking: false,
            last_fall: 0.0,
        }
    }

    pub fn config(&self) -> &MappingConfig {
        &self.config
    }

    /// Czy właśnie trwa szybkie zatrzymanie (postać stoi mimo ruchu taśmy).
    pub fn braking(&self) -> bool {
        self.braking
    }

    /// Od nowa: puste okno, bez biegu i hamowania.
    pub fn reset(&mut self) {
        self.window.clear();
        self.running = false;
        self.last_raw = None;
        self.braking = false;
    }

    fn track_braking(&mut self, raw: f64, now: f64) {
        let cfg = self.config;
        let last = self.last_raw;
        self.last_raw = Some(raw);
        if !cfg.quick_stop || raw <= STOPPED_KMH {
            self.braking = false;
            return;
        }
        match last {
            Some(last) if raw < last - 1e-9 => {
                if last - raw >= cfg.brake_drop || raw <= cfg.stand_below + cfg.brake_near {
                    self.braking = true;
                }
                self.last_fall = now; // każdy dalszy spadek podtrzymuje hamowanie
            }
            Some(last) if raw > last + 1e-9 => self.braking = false, // taśma przyspiesza
            _ => {}
        }
        if self.braking && now - self.last_fall > cfg.brake_hold_s {
            self.braking = false; // przestało spadać - to było tylko zwolnienie
        }
    }

    /// Nowy odczyt prędkości (km/h) z chwili `now` (sekundy, zegar monotoniczny).
    pub fn update(&mut self, speed_kmh: f64, now: f64) -> Control {
        let cfg = self.config;
        let raw = if speed_kmh > 0.0 { speed_kmh } else { 0.0 }; // max(0.0, x) z Pythona
        self.track_braking(raw, now);
        if self.window.len() >= cfg.smoothing_samples.max(1) {
            self.window.pop_front();
        }
        self.window.push_back(raw);
        let speed = fsum(&self.window) / self.window.len() as f64;

        if self.braking {
            self.running = false;
            return Control::standing(speed);
        }
        // stojąca taśma nigdy nie rusza postaci, nawet przy progu chodu 0 km/h
        if speed < cfg.stand_below || speed <= STOPPED_KMH {
            self.running = false;
            return Control::standing(speed);
        }
        self.running = if self.running {
            speed >= cfg.run_off
        } else {
            speed >= cfg.run_on
        };

        let stick = (speed - cfg.stand_below) / (cfg.max_speed - cfg.stand_below);
        let stick = if stick > 0.0 { stick } else { 0.0 };
        let stick = if stick < 1.0 { stick } else { 1.0 };
        Control {
            speed,
            stick,
            walk: true,
            run: self.running,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mapper() -> SpeedMapper {
        SpeedMapper::new(MappingConfig::default())
    }

    fn cfg(f: impl FnOnce(&mut MappingConfig)) -> MappingConfig {
        let mut c = MappingConfig::default();
        f(&mut c);
        c
    }

    #[test]
    fn standing_presses_nothing() {
        let c = mapper().update(0.5, 0.0);
        assert_eq!(
            c,
            Control {
                speed: 0.5,
                stick: 0.0,
                walk: false,
                run: false
            }
        );
    }

    #[test]
    fn walking_presses_w_only() {
        let c = mapper().update(3.0, 0.0);
        assert!(c.walk && !c.run);
    }

    #[test]
    fn run_has_hysteresis() {
        let mut m = mapper();
        assert!(!m.update(6.0, 0.0).run); // poniżej run_on
        assert!(m.update(6.3, 1.0).run); // włącza się od run_on
        assert!(m.update(5.8, 2.0).run); // nadal bieg nad run_off
        assert!(!m.update(5.6, 3.0).run); // wyłącza się poniżej run_off
        assert!(!m.update(6.0, 4.0).run); // i znów potrzeba run_on
    }

    #[test]
    fn stopping_resets_run() {
        let mut m = mapper();
        m.update(8.0, 0.0);
        assert_eq!(m.update(0.0, 1.0), Control::idle());
        assert!(!m.update(6.0, 2.0).run);
    }

    #[test]
    fn stick_is_linear_and_clamped() {
        let mut m = SpeedMapper::new(cfg(|c| {
            c.stand_below = 1.0;
            c.max_speed = 11.0;
        }));
        assert!((m.update(6.0, 0.0).stick - 0.5).abs() < 1e-12);
        assert_eq!(m.update(20.0, 1.0).stick, 1.0);
    }

    #[test]
    fn smoothing_averages_last_samples() {
        let mut m = SpeedMapper::new(cfg(|c| c.smoothing_samples = 3));
        m.update(3.0, 0.0);
        m.update(6.0, 1.0);
        assert!((m.update(9.0, 2.0).speed - 6.0).abs() < 1e-12);
        assert!((m.update(9.0, 3.0).speed - 8.0).abs() < 1e-12);
    }

    #[test]
    fn negative_speed_is_treated_as_zero() {
        assert_eq!(mapper().update(-5.0, 0.0).speed, 0.0);
    }

    #[test]
    fn invalid_config_is_rejected() {
        let bad = [
            cfg(|c| {
                c.run_on = 5.0;
                c.run_off = 5.5;
            }),
            cfg(|c| c.stand_below = 7.0),
            cfg(|c| c.max_speed = 6.0),
            cfg(|c| c.smoothing_samples = 0),
            cfg(|c| c.stand_below = f64::NAN),
        ];
        for c in bad {
            assert!(c.validate().is_err(), "{c:?}");
        }
        assert!(MappingConfig::default().validate().is_ok());
    }

    #[test]
    fn from_value_ignores_unknown_keys() {
        let c = MappingConfig::from_value(&serde_json::json!({"run_on": 7.0, "foo": 1}))
            .expect("poprawne ustawienia");
        assert_eq!(c.run_on, 7.0);
        assert!(MappingConfig::from_value(&serde_json::json!({"stand_below": "x"})).is_err());
    }

    #[test]
    fn full_x21_range_and_zero_walk_threshold() {
        let mut m = SpeedMapper::new(cfg(|c| {
            c.stand_below = 0.0;
            c.run_on = 12.0;
            c.run_off = 11.4;
        }));
        assert!(!m.update(0.0, 0.0).walk); // stojąca taśma nie rusza postaci
        assert!(m.update(0.5, 1.0).walk);
        assert!(m.update(12.0, 2.0).run);
        assert_eq!(m.update(12.0, 3.0).stick, 1.0);
    }

    // --- szybkie zatrzymanie: prawdziwe sekwencje z bieżni KS-NACH-X21C (log testera) ----
    // Bieżnia podaje prędkość co ~2 s; między ramkami aplikacja co 0,75 s powtarza ostatnią
    // prędkość (sygnał życia), więc symulujemy oba źródła odczytów.

    /// Czas, w którym postać przestała iść (albo `None`).
    fn replay(frames: &[(f64, f64)], quick: bool, walk_from: f64) -> Option<f64> {
        let mut m = SpeedMapper::new(cfg(|c| {
            c.stand_below = walk_from;
            c.run_on = 4.0;
            c.run_off = 3.4;
            c.quick_stop = quick;
        }));
        let (last_t, last_v) = *frames.last()?;
        let mut next: Vec<(f64, f64)> = frames[1..].to_vec();
        next.push((last_t + 2.0, last_v));
        let mut readings = Vec::new();
        for (&(t, v), &(nt, _)) in frames.iter().zip(&next) {
            let mut hb = t;
            while hb < nt {
                readings.push((hb, v));
                hb += 0.75;
            }
        }
        let mut walking = false;
        for (t, v) in readings {
            if m.update(v, t).walk {
                walking = true;
            } else if walking {
                return Some(t);
            }
        }
        None
    }

    fn approx(a: Option<f64>, b: f64) -> bool {
        a.is_some_and(|a| (a - b).abs() < 1e-9)
    }

    const STOP_1: &[(f64, f64)] = &[
        (44.6, 2.3),
        (46.6, 2.2),
        (48.6, 1.2),
        (50.6, 1.0),
        (53.0, 0.0),
        (55.0, 0.0),
    ];
    const STOP_2: &[(f64, f64)] = &[
        (12.8, 3.6),
        (14.8, 2.7),
        (16.8, 1.8),
        (18.8, 1.6),
        (20.7, 1.4),
        (22.7, 1.0),
        (25.5, 0.0),
    ];
    const STOP_3: &[(f64, f64)] = &[
        (0.0, 1.7),
        (2.0, 1.7),
        (4.0, 1.7),
        (6.1, 1.0),
        (8.0, 0.0),
        (10.0, 0.0),
    ];

    #[test]
    fn quick_stop_cuts_stop_lag() {
        for (frames, before, after) in [
            (STOP_1, 53.0, 48.6),
            (STOP_2, 25.5, 14.8),
            (STOP_3, 8.0, 6.1),
        ] {
            assert!(approx(replay(frames, false, 1.0), before));
            assert!(approx(replay(frames, true, 1.0), after));
        }
    }

    #[test]
    fn slowing_down_with_remote_does_not_stop() {
        let frames = [(0.0, 3.0), (2.0, 2.8), (4.0, 2.5), (6.0, 2.5), (8.0, 2.5)];
        assert_eq!(replay(&frames, true, 1.0), None);
    }

    #[test]
    fn quick_slowdown_resumes_walking_when_speed_holds() {
        let mut m = SpeedMapper::new(cfg(|c| c.stand_below = 1.0));
        assert!(m.update(5.0, 0.0).walk);
        assert!(!m.update(3.5, 2.0).walk); // spadek 1,5 = hamowanie
        assert!(!m.update(3.5, 4.0).walk); // nadal w oknie 3 s
        assert!(m.update(3.5, 5.5).walk); // nie spada dalej - to było tylko zwolnienie
        assert!(m.update(4.0, 6.0).walk);
    }

    #[test]
    fn slow_walk_and_ramp_stops() {
        let slow = [
            (0.0, 1.7),
            (2.0, 1.7),
            (4.0, 1.5),
            (6.0, 1.0),
            (8.0, 0.0),
            (10.0, 0.0),
        ];
        let ramp = [
            (0.0, 2.0),
            (2.0, 2.0),
            (4.0, 1.9),
            (6.0, 1.0),
            (8.0, 0.0),
            (10.0, 0.0),
        ];
        assert!(approx(replay(&slow, false, 1.0), 8.0));
        assert!(approx(replay(&slow, true, 1.0), 4.0)); // 1,7 -> 1,5 przy progu 1,0
        assert!(approx(replay(&ramp, true, 1.0), 6.0)); // 1,9 -> 1,0 (0,9 km/h)
    }

    #[test]
    fn remote_steps_of_half_kmh_do_not_stop_running() {
        let mut m = SpeedMapper::new(cfg(|c| {
            c.stand_below = 1.0;
            c.run_on = 4.0;
            c.run_off = 3.4;
        }));
        for (i, v) in [5.0, 4.5, 4.0, 3.5].into_iter().enumerate() {
            assert!(m.update(v, i as f64 * 2.0).walk);
        }
    }
}
