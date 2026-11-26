//! Pilnowanie prędkości dla bieżni, które nie potwierdzają poleceń (KingSmith X21) - port
//! bieznia/sterowniki/pilnowanie.py.
//!
//! X21 po cichu gubi część poleceń prędkości. Czy posłuchał, widać dopiero w ramce z danymi:
//! bez reakcji wysyłamy to samo polecenie jeszcze raz (najwyżej [`RESENDS`] razy), a taśmie,
//! która już zmienia prędkość, dajemy czas na rozpędzenie.
//!
//! [`SpeedGuard`] nie wysyła sam - mówi, co wysłać, a sterownik po wysyłce zgłasza wynik
//! przez [`SpeedGuard::sent`]. Dzięki temu działa w każdym sterowniku i w testach bez BLE:
//!
//! ```ignore
//! let kmh = guard.set(2.5, clock(), driver.belt_speed());
//! let ok = send(kmh).await;
//! guard.sent(ok, clock());
//! // w poll():
//! if let Some(kmh) = guard.check(clock(), driver.belt_speed(), |v| caps.clamp(v)) {
//!     let ok = send(kmh).await;
//!     guard.sent(ok, clock());
//! }
//! ```

use serde::{Deserialize, Serialize};

pub const TOLERANCE: f64 = 0.15;
pub const RESEND_S: f64 = 2.5;
/// tyle najwyżej czekamy, gdy taśma już zmienia prędkość
pub const RAMP_S: f64 = 8.0;
pub const RESENDS: u32 = 5;

/// Liczniki poleceń prędkości jednego połączenia (do podsumowania w logu).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpeedStats {
    pub sent: u64,
    pub ok: u64,
    pub failed: u64,
    pub resends: u64,
    /// ile sekund od pierwszego wysłania do ramki z przyjętą prędkością
    pub latencies: Vec<f64>,
}

#[derive(Debug, Clone, Copy)]
struct Pending {
    kmh: f64,
    speed_at_send: f64,
}

/// Pilnowanie zadanej prędkości (patrz opis modułu).
#[derive(Debug, Clone)]
pub struct SpeedGuard {
    pub stats: SpeedStats,
    target: Option<f64>,
    confirmed: bool,
    sent_at: f64,
    first_sent_at: f64,
    speed_at_send: f64,
    resends: u32,
    pending: Option<Pending>,
}

impl Default for SpeedGuard {
    fn default() -> Self {
        Self::new()
    }
}

impl SpeedGuard {
    pub fn new() -> Self {
        Self {
            stats: SpeedStats::default(),
            target: None,
            confirmed: true,
            sent_at: 0.0,
            first_sent_at: 0.0,
            speed_at_send: 0.0,
            resends: 0,
            pending: None,
        }
    }

    /// Pilnowana prędkość (None po stopie).
    pub fn target(&self) -> Option<f64> {
        self.target
    }

    /// Taśma posłuchała (albo się poddaliśmy) - pilot może zmieniać prędkość bez walki.
    pub fn confirmed(&self) -> bool {
        self.confirmed
    }

    /// Stop taśmy: zadanej prędkości już nie pilnujemy.
    pub fn reset(&mut self) {
        self.target = None;
        self.confirmed = true;
    }

    /// Nowa prędkość docelowa. Zwraca prędkość do wysłania od razu; po wysyłce wołaj
    /// [`SpeedGuard::sent`]. `reading` - ostatnia ramka bieżni (prędkość, czas).
    pub fn set(&mut self, kmh: f64, now: f64, reading: Option<(f64, f64)>) -> f64 {
        if self.target != Some(kmh) || self.confirmed {
            self.stats.sent += 1;
        }
        self.resends = 0;
        self.first_sent_at = now;
        self.prepare(kmh, reading)
    }

    fn prepare(&mut self, kmh: f64, reading: Option<(f64, f64)>) -> f64 {
        self.pending = Some(Pending {
            kmh,
            speed_at_send: reading.map_or(0.0, |r| r.0),
        });
        kmh
    }

    /// Wynik wysyłki z [`SpeedGuard::set`] albo [`SpeedGuard::check`]. Zwraca `ok`.
    pub fn sent(&mut self, ok: bool, now: f64) -> bool {
        let Some(pending) = self.pending.take() else {
            return ok;
        };
        if ok {
            self.target = Some(pending.kmh);
            self.sent_at = now;
            self.speed_at_send = pending.speed_at_send;
            self.confirmed = false;
        }
        ok
    }

    /// Czy taśma posłuchała? Bez reakcji zwraca prędkość do ponownego wysłania (potem
    /// [`SpeedGuard::sent`]). `reachable` - prędkość, którą taśma naprawdę pojedzie (np. 0 km/h
    /// przy bieżni, która jedzie najwolniej 1,0).
    pub fn check(
        &mut self,
        now: f64,
        reading: Option<(f64, f64)>,
        reachable: impl Fn(f64) -> f64,
    ) -> Option<f64> {
        let target = self.target?;
        if self.confirmed {
            return None;
        }
        let (speed, at) = reading?;
        let want = reachable(target);
        if (speed - want).abs() <= TOLERANCE {
            self.confirmed = true; // od teraz pilot może zmieniać prędkość bez walki
            let took = at - self.first_sent_at;
            self.stats.ok += 1;
            self.stats.latencies.push(took);
            log::info!(
                "Prędkość {target:.1}: przyjęta po {took:.1} s (ponowienia {})",
                self.resends
            );
            return None;
        }
        if speed <= 0.0 || at <= self.sent_at || now - self.sent_at < RESEND_S {
            return None; // taśma stoi albo nie ma jeszcze świeżej ramki
        }
        let start = self.speed_at_send;
        let moving = (speed - start) * (want - start) > 0.0;
        if moving && now - self.sent_at < RAMP_S {
            return None;
        }
        if self.resends >= RESENDS {
            log::warn!("Taśma nie przyjęła {target:.1} km/h (jedzie {speed:.1}) - poddaję się");
            self.stats.failed += 1;
            self.confirmed = true;
            return None;
        }
        self.resends += 1;
        self.stats.resends += 1;
        log::info!(
            "Taśma nie przyjęła {target:.1} km/h (jedzie {speed:.1}) - ponawiam ({})",
            self.resends
        );
        Some(self.prepare(target, reading))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Jak X21 z logu testera: część poleceń prędkości przepada bez śladu.
    struct ForgetfulBelt {
        calls: Vec<f64>,
        ignore: u32,
        speed: f64,
        at: f64,
    }

    impl ForgetfulBelt {
        fn new(ignore: u32) -> Self {
            Self {
                calls: vec![],
                ignore,
                speed: 1.0,
                at: 0.0,
            }
        }

        fn send(&mut self, kmh: f64) -> bool {
            self.calls.push(kmh);
            if self.ignore > 0 {
                self.ignore -= 1;
            } else {
                self.speed = kmh;
            }
            true
        }

        fn reading(&self) -> Option<(f64, f64)> {
            Some((self.speed, self.at))
        }
    }

    fn set(guard: &mut SpeedGuard, belt: &mut ForgetfulBelt, kmh: f64, now: f64) {
        let kmh = guard.set(kmh, now, belt.reading());
        let ok = belt.send(kmh);
        guard.sent(ok, now);
    }

    fn step(guard: &mut SpeedGuard, belt: &mut ForgetfulBelt, clock: &mut f64, seconds: f64) {
        *clock += seconds;
        belt.at = *clock; // świeża ramka z danymi
        if let Some(kmh) = guard.check(*clock, belt.reading(), |v| v) {
            let ok = belt.send(kmh);
            guard.sent(ok, *clock);
        }
    }

    #[test]
    fn ignored_speed_command_is_resent() {
        let mut clock = 100.0;
        let mut belt = ForgetfulBelt::new(1);
        let mut guard = SpeedGuard::new();
        set(&mut guard, &mut belt, 2.5, clock);
        assert_eq!(belt.calls, [2.5]);
        step(&mut guard, &mut belt, &mut clock, 1.0);
        assert_eq!(belt.calls, [2.5]); // za wcześnie na ponowienie
        step(&mut guard, &mut belt, &mut clock, 2.0);
        assert_eq!(belt.calls, [2.5, 2.5]); // taśma nie drgnęła - ponowione
        step(&mut guard, &mut belt, &mut clock, 3.0);
        assert!(guard.confirmed());
        belt.speed = 3.5; // pilot zmienia prędkość - aplikacja z nim nie walczy
        step(&mut guard, &mut belt, &mut clock, 5.0);
        assert_eq!(belt.calls.len(), 2);
        assert_eq!((guard.stats.ok, guard.stats.resends), (1, 1));
    }

    #[test]
    fn resending_gives_up() {
        let mut clock = 100.0;
        let mut belt = ForgetfulBelt::new(99);
        let mut guard = SpeedGuard::new();
        set(&mut guard, &mut belt, 2.5, clock);
        for _ in 0..20 {
            step(&mut guard, &mut belt, &mut clock, 3.0);
        }
        assert_eq!(belt.calls.len(), 1 + RESENDS as usize);
        assert_eq!(guard.stats.failed, 1);
    }

    #[test]
    fn ramping_belt_is_not_hurried() {
        let mut clock = 100.0;
        let mut belt = ForgetfulBelt::new(1);
        let mut guard = SpeedGuard::new();
        set(&mut guard, &mut belt, 2.5, clock);
        belt.speed = 1.8; // taśma już przyspiesza
        step(&mut guard, &mut belt, &mut clock, 3.0);
        assert_eq!(belt.calls, [2.5]);
    }
}
