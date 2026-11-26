//! Bieżąca sesja liczona z odczytów aplikacji (port SessionTracker z bieznia/historia.py).

use super::{
    History, IDLE_END_S, MIN_SESSION_S, Result, SAVE_EVERY_S, Session, SessionSource, apply_source,
    new_session, now_unix, utc_offset_min, uuid7,
};

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

/// Liczy bieżącą sesję z odczytów aplikacji i zapisuje ją do historii.
///
/// Kroki z licznika bieżni (zerowanie licznika nie gubi kroków), dystans i czas ruchu
/// z narastających liczników aplikacji. Zapis co [`SAVE_EVERY_S`], koniec po
/// [`IDLE_END_S`] bez ruchu. Zmiana osoby, profilu, trybu albo bieżni kończy sesję.
pub struct SessionTracker {
    clock: Clock,
    current: Option<Session>,
    last_steps: Option<i64>,
    last_distance: f64,
    last_active: f64,
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
            last_steps: None,
            last_distance: 0.0,
            last_active: 0.0,
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

    /// Bieżąca bieżnia (albo symulator).
    pub fn source(&self) -> &SessionSource {
        &self.origin
    }

    /// "gra" albo "trening".
    pub fn mode(&self) -> &str {
        &self.mode
    }

    /// Zmiana osoby albo profilu kończy sesję poprzedniego.
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

    /// Inna bieżnia (albo symulator) to nowa sesja - wyniki nie mieszają się między nimi.
    /// Firmware odczytany później uzupełnia trwającą sesję.
    pub fn set_source(&mut self, history: &History, origin: SessionSource) -> Result<()> {
        let same = origin.model == self.origin.model
            && origin.driver == self.origin.driver
            && origin.source == self.origin.source;
        if !same {
            self.finish(history)?;
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

    /// Wywoływane z pętli aplikacji. `distance_km` i `active_s` to liczniki narastające
    /// aplikacji, `steps` - licznik bieżni (None, gdy bieżnia go nie podaje).
    pub fn update(
        &mut self,
        history: &History,
        speed: f64,
        steps: Option<i64>,
        distance_km: f64,
        active_s: f64,
    ) -> Result<()> {
        let now = (self.clock)();
        let d_dist = (distance_km - self.last_distance).max(0.0);
        let d_active = (active_s - self.last_active).max(0.0);
        self.last_distance = distance_km;
        self.last_active = active_s;
        let mut d_steps = 0;
        if let Some(steps) = steps {
            if let Some(last) = self.last_steps {
                // licznik bieżni wyzerowany - liczymy od zera
                d_steps = if steps >= last { steps - last } else { steps };
            }
            self.last_steps = Some(steps);
        }
        let moving = speed > 0.05 || d_steps > 0;
        if moving {
            self.last_move = now;
            if self.current.is_none() {
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
                self.current = Some(s);
            }
        }
        let Some(s) = self.current.as_mut() else {
            return Ok(());
        };
        s.steps += d_steps.max(0);
        s.distance_km += d_dist;
        s.active_s += d_active;
        s.end = now;
        if !moving && now - self.last_move >= IDLE_END_S {
            self.finish(history)
        } else if now - self.saved_at >= SAVE_EVERY_S {
            self.save(history)
        } else {
            Ok(())
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
                log::info!(
                    "Sesja kroków: {} / {} - {} kroków, {:.2} km, {:.0} min",
                    s.person_name,
                    s.profile_name,
                    s.steps,
                    s.distance_km,
                    s.active_s / 60.0
                );
            }
        }
        self.current = None;
        Ok(())
    }
}
