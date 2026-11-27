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
                "[speed-ok] Prędkość {target:.1}: przyjęta po {took:.1} s (ponowienia {})",
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
            log::warn!(
                "[speed-no-effect] Taśma nie przyjęła {target:.1} km/h (jedzie {speed:.1}) - poddaję się"
            );
            self.stats.failed += 1;
            self.confirmed = true;
            return None;
        }
        self.resends += 1;
        self.stats.resends += 1;
        log::info!(
            "[speed-no-effect] Taśma nie przyjęła {target:.1} km/h (jedzie {speed:.1}) - ponawiam ({})",
            self.resends
        );
        Some(self.prepare(target, reading))
    }
}

// ---------------------------------------------------------------------------------------------
// Hamowanie po stopie (0.6.2, zgłoszenie Velczera, KS-NACH-X21C)
// ---------------------------------------------------------------------------------------------

/// Prędkość nie spada od tylu sekund (od stopu albo od ostatniego spadku) - stop ponawiamy.
pub const BRAKE_NO_DROP_S: f64 = 3.5;
/// Taśma po tylu sekundach od pierwszego stopu wciąż jedzie - alarm.
pub const BRAKE_MAX_S: f64 = 15.0;
/// Poniżej tej prędkości (km/h) taśma stoi.
pub const STOPPED_KMH: f64 = 0.05;
/// Mniejsza zmiana prędkości to szum odczytu, nie spadek.
const DROP_KMH: f64 = 0.05;

/// Co widać po poleceniu stopu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Braking {
    /// taśma stoi (runState 0 albo prędkość 0)
    Stopped,
    /// prędkość spada albo czekamy na pierwsze ramki - spokojnie dalej
    Slowing,
    /// prędkość nie spada od [`BRAKE_NO_DROP_S`] - wysłać stop jeszcze raz
    Stuck,
    /// po [`BRAKE_MAX_S`] taśma wciąż jedzie - alarm
    TooLong,
}

/// Przebieg hamowania po stopie. X21C hamuje 3,5 › 3,0 › 2,0 › 1,0 km/h z runState 1 do
/// samego końca - spadająca prędkość jest potwierdzeniem, że bieżnia posłuchała.
#[derive(Debug, Clone)]
pub struct BrakeWatch {
    no_drop_s: f64,
    max_s: f64,
    began: f64,
    /// ostatni stop albo ostatni spadek prędkości
    calm_since: f64,
    last_at: f64,
    last_kmh: Option<f64>,
    /// prędkości kolejnych ramek (do dziennika)
    trace: Vec<f64>,
}

impl BrakeWatch {
    /// Stop wysłany w chwili `now`; `reading` - ostatnia ramka sprzed stopu (prędkość, czas).
    pub fn new(now: f64, reading: Option<(f64, f64)>) -> Self {
        Self {
            no_drop_s: BRAKE_NO_DROP_S,
            max_s: BRAKE_MAX_S,
            began: now,
            calm_since: now,
            last_at: reading.map_or(f64::NEG_INFINITY, |r| r.1),
            last_kmh: reading.map(|r| r.0),
            trace: reading.map(|r| vec![r.0]).unwrap_or_default(),
        }
    }

    /// Inne granice niż [`BRAKE_NO_DROP_S`] i [`BRAKE_MAX_S`] (testy sterownika je skracają).
    pub fn with_limits(mut self, no_drop_s: f64, max_s: f64) -> Self {
        self.no_drop_s = no_drop_s;
        self.max_s = max_s;
        self
    }

    /// Stop wysłany jeszcze raz - znowu dajemy taśmie czas na reakcję.
    pub fn resent(&mut self, now: f64) {
        self.calm_since = now;
    }

    /// Ramka z bieżni (prędkość, czas ramki); ta sama ramka drugi raz nic nie zmienia.
    pub fn feed(&mut self, kmh: f64, at: f64) {
        if at <= self.last_at {
            return;
        }
        if self.last_kmh.is_some_and(|last| kmh < last - DROP_KMH) {
            self.calm_since = self.calm_since.max(at);
        }
        self.last_at = at;
        self.last_kmh = Some(kmh);
        self.trace.push(kmh);
    }

    /// Stan w chwili `now`; `running` - runState z bieżni (None - nie wiadomo).
    pub fn state(&self, now: f64, running: Option<bool>) -> Braking {
        let fresh = self.last_at >= self.began;
        if running == Some(false) || (fresh && self.last_kmh.is_some_and(|k| k <= STOPPED_KMH)) {
            Braking::Stopped
        } else if now - self.began >= self.max_s {
            Braking::TooLong
        } else if now - self.calm_since >= self.no_drop_s {
            Braking::Stuck
        } else {
            Braking::Slowing
        }
    }

    /// „3.5 › 3.0 › 2.0 › 1.0 › 0.0 km/h w 6.1 s” (do dziennika).
    pub fn describe(&self, now: f64) -> String {
        let mut speeds: Vec<String> = Vec::new();
        for kmh in &self.trace {
            let s = format!("{kmh:.1}");
            if speeds.last() != Some(&s) {
                speeds.push(s);
            }
        }
        let path = if speeds.is_empty() {
            "bez ramek z prędkością".to_string()
        } else {
            format!("{} km/h", speeds.join(" › "))
        };
        format!("{path} w {:.1} s", now - self.began)
    }
}

// ---------------------------------------------------------------------------------------------
// Prędkość sprzed zatrzymania (0.6.2, Chodzenie)
// ---------------------------------------------------------------------------------------------

/// Tyle sekund ta sama prędkość = tempo, którym osoba szła (a nie chwila hamowania).
pub const PLATEAU_S: f64 = 3.0;
/// Pamiętamy tyle sekund odczytów.
const RESUME_HISTORY_S: f64 = 90.0;

/// Ostatnia prędkość taśmy sprzed zatrzymania. KingSmith po starcie z aplikacji rusza
/// z prędkością startową z aplikacji producenta (np. 1 km/h) zamiast tej sprzed stopu - w trybie
/// Chodzenie postać by stała. Po starcie z aplikacji worker wysyła tę prędkość raz.
#[derive(Debug, Clone, Default)]
pub struct ResumeSpeed {
    /// (czas ramki, km/h) jadącej taśmy, rosnąco
    history: Vec<(f64, f64)>,
    last_at: f64,
    /// stop z aplikacji: prędkość ustalona w chwili stopu (hamowanie jej nie zmienia)
    locked: Option<f64>,
    resume: Option<f64>,
}

impl ResumeSpeed {
    pub fn new() -> Self {
        Self {
            last_at: f64::NEG_INFINITY,
            ..Self::default()
        }
    }

    /// Ramka z bieżni (prędkość, czas ramki).
    pub fn observe(&mut self, kmh: f64, at: f64) {
        if at <= self.last_at {
            return;
        }
        self.last_at = at;
        if kmh > STOPPED_KMH {
            self.history.push((at, kmh));
            self.history.retain(|(t, _)| at - t <= RESUME_HISTORY_S);
            return;
        }
        // taśma stanęła (stop z aplikacji albo pilotem)
        if let Some(v) = self.locked.take().or_else(|| plateau(&self.history)) {
            self.resume = Some(v);
        }
        self.history.clear();
    }

    /// Stop z aplikacji: zapamiętaj tempo teraz, zanim hamowanie je zmieni.
    pub fn stop_requested(&mut self) {
        if let Some(v) = plateau(&self.history) {
            self.locked = Some(v);
        }
    }

    /// Prędkość do ustawienia po starcie (None - nie znamy, zostaje jak dziś).
    pub fn target(&self) -> Option<f64> {
        self.resume.or(self.locked)
    }
}

/// Tempo przed końcowym hamowaniem: idąc od końca po malejącym przebiegu (wstecz rosnącym),
/// pierwsza prędkość trzymana co najmniej [`PLATEAU_S`]; bez takiej - szczyt przed spadkiem.
fn plateau(history: &[(f64, f64)]) -> Option<f64> {
    let mut end = history.len();
    let mut peak: Option<f64> = None;
    while end > 0 {
        let (t_end, v) = history[end - 1];
        let mut start = end - 1;
        while start > 0 && (history[start - 1].1 - v).abs() <= DROP_KMH {
            start -= 1;
        }
        if t_end - history[start].0 >= PLATEAU_S {
            return Some(v);
        }
        peak = Some(peak.map_or(v, |p| p.max(v)));
        if start > 0 && history[start - 1].1 < v - DROP_KMH {
            break; // wcześniej było wolniej - to szczyt przed hamowaniem
        }
        end = start;
    }
    peak
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
    // Nagrany przebieg KS-NACH-X21C (zgłoszenie Velczera): ramki co ok. 2 s, runState 1 do końca.
    const X21C_BRAKING: [(f64, f64); 5] = [
        (100.4, 3.5),
        (102.4, 3.0),
        (104.4, 2.0),
        (106.4, 1.0),
        (108.4, 0.0),
    ];

    #[test]
    fn falling_speed_confirms_stop_without_alarm() {
        let mut w = BrakeWatch::new(100.0, Some((3.5, 99.6)));
        let mut t = 100.0;
        for &(at, kmh) in &X21C_BRAKING {
            while t < at {
                assert_eq!(w.state(t, Some(true)), Braking::Slowing, "t={t}");
                t += 0.25;
            }
            w.feed(kmh, at);
        }
        assert_eq!(w.state(110.5, Some(true)), Braking::Stopped);
        assert_eq!(
            w.describe(110.5),
            "3.5 › 3.0 › 2.0 › 1.0 › 0.0 km/h w 10.5 s"
        );
    }

    #[test]
    fn speed_that_does_not_drop_is_stuck_and_long_braking_alarms() {
        let mut w = BrakeWatch::new(0.0, Some((3.5, -0.5)));
        for i in 1..=4 {
            w.feed(3.5, f64::from(i));
        }
        assert_eq!(w.state(3.0, Some(true)), Braking::Slowing);
        assert_eq!(w.state(3.6, Some(true)), Braking::Stuck);
        w.resent(3.6);
        assert_eq!(w.state(4.0, Some(true)), Braking::Slowing);
        // spada, ale bardzo wolno - po 15 s alarm mimo spadków
        let mut kmh = 3.5;
        for i in 5..=16 {
            kmh -= 0.1;
            w.feed(kmh, f64::from(i));
        }
        assert_eq!(w.state(14.9, Some(true)), Braking::Slowing);
        assert_eq!(w.state(15.0, Some(true)), Braking::TooLong);
        assert_eq!(w.state(15.0, Some(false)), Braking::Stopped);
        // stara ramka z zerem sprzed stopu nie jest potwierdzeniem
        let old = BrakeWatch::new(10.0, Some((0.0, 9.0)));
        assert_eq!(old.state(10.5, Some(true)), Braking::Slowing);
    }

    #[test]
    fn resume_speed_is_the_walking_pace_before_braking() {
        // pilot: szła 3,3 km/h, potem hamowanie X21C
        let mut r = ResumeSpeed::new();
        assert_eq!(r.target(), None);
        for i in 0..10 {
            r.observe(3.3, 80.0 + f64::from(i) * 2.0);
        }
        for &(at, kmh) in &X21C_BRAKING[1..] {
            r.observe(kmh, at);
        }
        assert_eq!(r.target(), Some(3.3));
        // stop z aplikacji: tempo ustalone od razu, hamowanie go nie zmienia
        let mut r = ResumeSpeed::new();
        for i in 0..6 {
            r.observe(3.0, f64::from(i) * 2.0);
        }
        r.observe(3.2, 12.0); // pilotem chwilę przed stopem
        r.stop_requested();
        assert_eq!(r.target(), Some(3.2));
        for (i, kmh) in [2.5, 1.0, 1.0, 1.0, 0.0].into_iter().enumerate() {
            r.observe(kmh, 14.0 + i as f64 * 2.0);
        }
        assert_eq!(r.target(), Some(3.2));
        // szybkie wahania bez tempa trzymanego 3 s - szczyt przed spadkiem
        assert_eq!(
            plateau(&[(0.0, 1.0), (1.0, 2.5), (2.0, 2.0), (3.0, 1.5)]),
            Some(2.5)
        );
        assert_eq!(plateau(&[]), None);
    }
}
