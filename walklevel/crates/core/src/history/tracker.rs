//! Bieżąca sesja liczona z ramek telemetrii bieżni (port SessionTracker z bieznia/historia.py,
//! zasady z docs/analiza-dystans.md, R8).
//!
//! Tracker trzyma własny licznik ([`Meter`]) - kroki, dystans i czas ruchu nie zależą od
//! mapowania gry ani od sum aplikacji. Sesja zaczyna się od pierwszego ruchu taśmy i kończy po
//! [`IDLE_END_S`] bez ruchu albo bez żadnych danych ([`SessionTracker::tick`]); koniec sesji
//! to chwila ostatniego ruchu.

use serde::{Deserialize, Serialize};

use super::{
    History, IDLE_END_S, MIN_SESSION_S, Result, SAVE_EVERY_S, Session, SessionSource, apply_source,
    new_session, now_unix, utc_offset_min, uuid7,
};
use crate::treadmill::telemetry::{MOVING_KMH, Meter, MeterTotals, TelemetryFrame, TelemetrySpec};

/// Zegar: bieżący czas uniksowy w sekundach (w testach - sterowany ręcznie).
pub type Clock = Box<dyn Fn() -> f64 + Send>;

/// Właściciel sesji: osoba i profil gry (id oraz nazwa w chwili sesji).
#[derive(Debug, Clone, Default, PartialEq)]
struct Owner {
    person: String,
    person_name: String,
    profile: String,
    profile_name: String,
}

/// Wartości do kafelków i podglądu na żywo: bieżąca sesja, a po jej końcu - ostatnia
/// sesja do następnego ruchu.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveStats {
    /// None - bieżnia nie podaje kroków (kafelek „–”)
    pub steps: Option<i64>,
    /// kroki/min z okna 15 s (None - taśma stoi albo za mało danych)
    pub cadence: Option<f64>,
    pub distance_km: f64,
    /// czas ruchu taśmy (s)
    pub active_s: f64,
}

/// Liczy bieżącą sesję z ramek bieżni i zapisuje ją do historii.
///
/// Zapis co [`SAVE_EVERY_S`], koniec po [`IDLE_END_S`] bez ruchu. Zmiana osoby, profilu
/// albo trybu kończy sesję; zmiana bieżni dodatkowo zaczyna liczniki od nowa.
pub struct SessionTracker {
    clock: Clock,
    current: Option<Session>,
    last: Option<Session>,
    meter: Meter,
    seen: MeterTotals,
    /// dystans trwającej sesji w metrach (w sesji zaokrąglony do metra)
    session_m: f64,
    last_move: f64,
    saved_at: f64,
    owner: Owner,
    origin: SessionSource,
    mode: String,
}

impl Default for SessionTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionTracker {
    /// Tracker z zegarem systemowym.
    pub fn new() -> Self {
        Self::with_clock(Box::new(now_unix))
    }

    /// Tracker z własnym zegarem (testy, odtwarzanie).
    pub fn with_clock(clock: Clock) -> Self {
        Self {
            clock,
            current: None,
            last: None,
            meter: Meter::default(),
            seen: MeterTotals::default(),
            session_m: 0.0,
            last_move: 0.0,
            saved_at: 0.0,
            owner: Owner::default(),
            origin: SessionSource::default(),
            mode: "gra".into(),
        }
    }

    /// Trwająca sesja (None - taśma stoi dłużej niż [`IDLE_END_S`] albo jeszcze nie ruszyła).
    pub fn current(&self) -> Option<&Session> {
        self.current.as_ref()
    }

    /// Ostatnia zakończona sesja (do kafelków przed następnym ruchem).
    pub fn last(&self) -> Option<&Session> {
        self.last.as_ref()
    }

    /// Licznik ramek bieżącej bieżni.
    pub fn meter(&self) -> &Meter {
        &self.meter
    }

    /// Bieżąca bieżnia (albo symulator).
    pub fn source(&self) -> &SessionSource {
        &self.origin
    }

    /// "gra" albo "trening".
    pub fn mode(&self) -> &str {
        &self.mode
    }

    /// Zmiana osoby albo profilu kończy sesję poprzedniego (liczniki bieżni ciągną się dalej).
    pub fn set_owner(
        &mut self,
        history: &History,
        person: &str,
        person_name: &str,
        profile: &str,
        profile_name: &str,
    ) -> Result<()> {
        let new = Owner {
            person: person.into(),
            person_name: person_name.into(),
            profile: profile.into(),
            profile_name: profile_name.into(),
        };
        if new != self.owner {
            self.finish(history)?;
            self.owner = new;
        }
        Ok(())
    }

    /// Gra albo trening (tryb Trening) - zmiana kończy bieżącą sesję.
    pub fn set_mode(&mut self, history: &History, mode: &str) -> Result<()> {
        if mode != self.mode {
            self.finish(history)?;
            self.mode = mode.into();
        }
        Ok(())
    }

    /// Inna bieżnia (albo symulator) to nowa sesja i liczniki od nowa - wyniki i liczniki nie
    /// mieszają się między nimi. Firmware odczytany później uzupełnia trwającą sesję.
    pub fn set_source(&mut self, history: &History, origin: SessionSource) -> Result<()> {
        let same = origin.model == self.origin.model
            && origin.driver == self.origin.driver
            && origin.source == self.origin.source;
        if !same {
            self.finish(history)?;
            self.reset_meter(*self.meter.spec());
        }
        if let Some(s) = self.current.as_mut()
            && s.firmware.is_empty()
            && !origin.firmware.is_empty()
        {
            s.firmware.clone_from(&origin.firmware);
        }
        self.origin = origin;
        Ok(())
    }

    /// Liczniki bieżącej bieżni (z [`crate::treadmill::WorkerEvent::Device`]). Inne niż
    /// dotąd - liczniki od nowego punktu odniesienia (sesja trwa dalej).
    pub fn set_spec(&mut self, spec: TelemetrySpec) {
        if spec != *self.meter.spec() {
            self.reset_meter(spec);
        }
    }

    fn reset_meter(&mut self, spec: TelemetrySpec) {
        self.meter = Meter::new(spec);
        self.seen = MeterTotals::default();
    }

    /// Ramka z bieżni (albo symulatora).
    pub fn feed(&mut self, history: &History, frame: &TelemetryFrame) -> Result<()> {
        let now = (self.clock)();
        if self.current.is_some() && now - self.last_move >= IDLE_END_S {
            // dane wróciły po długiej przerwie - poprzednia sesja już się skończyła
            self.finish(history)?;
        }
        self.meter.feed(frame);
        let totals = self.meter.totals();
        let d_steps = totals.steps.saturating_sub(self.seen.steps);
        let d_m = (totals.distance_m - self.seen.distance_m).max(0.0);
        let d_active = (totals.moving_s - self.seen.moving_s).max(0.0);
        self.seen = totals;
        let moving = frame.speed_kmh.unwrap_or(0.0) > MOVING_KMH || d_steps > 0 || d_active > 0.0;
        if moving {
            self.last_move = now;
            if self.current.is_none() {
                self.begin(now);
            }
        }
        let Some(s) = self.current.as_mut() else {
            return Ok(());
        };
        s.steps += i64::try_from(d_steps).unwrap_or(i64::MAX);
        self.session_m += d_m;
        s.distance_km = self.session_m.round() / 1000.0;
        s.active_s += d_active;
        s.end = self.last_move;
        self.after_update(history, now)
    }

    /// Co sekundę, także bez danych: koniec sesji po [`IDLE_END_S`] bez ruchu i zapis co
    /// [`SAVE_EVERY_S`].
    pub fn tick(&mut self, history: &History) -> Result<()> {
        let now = (self.clock)();
        if self.current.is_some() {
            self.after_update(history, now)?;
        }
        Ok(())
    }

    fn after_update(&mut self, history: &History, now: f64) -> Result<()> {
        if now - self.last_move >= IDLE_END_S {
            self.finish(history)
        } else if now - self.saved_at >= SAVE_EVERY_S {
            self.save(history)
        } else {
            Ok(())
        }
    }

    fn begin(&mut self, now: f64) {
        let o = &self.owner;
        let mut s = new_session(
            &uuid7(),
            &o.person,
            &o.person_name,
            &o.profile,
            &o.profile_name,
            now,
            now,
        );
        apply_source(&mut s, &self.origin);
        s.tz_offset_min = utc_offset_min(now);
        s.mode.clone_from(&self.mode);
        self.session_m = 0.0;
        self.meter.clear_cadence();
        self.current = Some(s);
    }

    /// Kafelki: bieżąca sesja (albo ostatnia do następnego ruchu). `now` - zegar ramek.
    pub fn live(&self, now: f64) -> LiveStats {
        let shown = self.current.as_ref().or(self.last.as_ref());
        LiveStats {
            steps: self.meter.has_steps().then(|| shown.map_or(0, |s| s.steps)),
            cadence: self.current.as_ref().and_then(|_| self.meter.cadence(now)),
            distance_km: shown.map_or(0.0, |s| s.distance_km),
            active_s: shown.map_or(0.0, |s| s.active_s),
        }
    }

    fn save(&mut self, history: &History) -> Result<()> {
        self.saved_at = (self.clock)();
        let Some(s) = self.current.as_mut() else {
            return Ok(());
        };
        if s.person.is_empty() {
            return Ok(());
        }
        if s.end - s.start >= MIN_SESSION_S || s.steps > 0 {
            history.save(s)?;
        }
        Ok(())
    }

    /// Bieżąca sesja na dysk teraz (np. przed pokazaniem historii).
    pub fn flush(&mut self, history: &History) -> Result<()> {
        if self.current.is_some() {
            self.save(history)?;
        }
        Ok(())
    }

    /// Kończy bieżącą sesję (zapis, jeśli nie była przypadkowym ruchem taśmy).
    pub fn finish(&mut self, history: &History) -> Result<()> {
        if self.current.is_some() {
            self.save(history)?;
            if let Some(s) = &self.current {
                let stride = if s.steps >= 50 {
                    format!(", krok {:.2} m", s.distance_km * 1000.0 / s.steps as f64)
                } else {
                    String::new()
                };
                log::info!(
                    "Sesja kroków: {} / {} - {} kroków, {:.2} km, {:.0} min{stride}",
                    s.person_name,
                    s.profile_name,
                    s.steps,
                    s.distance_km,
                    s.active_s / 60.0
                );
            }
        }
        if let Some(s) = self.current.take() {
            self.last = Some(s);
        }
        self.session_m = 0.0;
        Ok(())
    }
}
