//! Przypadki testowe z docs/analiza-dystans.md (rozdział 6): dystans i czas ruchu (I1-I14),
//! kadencja (K1-K6) i sesja (S1-S10) - w tym odtworzenie prawdziwych zapisów: X21
//! (`x21-ramki.txt`), wycinek Urevo (`urevo-5l-ramki.txt`) i pełna 37-minutowa sesja Urevo
//! z bluetooth.log (`urevo-5l-sesja-37min.txt`).

use std::sync::{Arc, Mutex};

use walklevel_core::history::{History, SessionSource, SessionTracker};
use walklevel_core::treadmill::kingsmith_proto as ks;
use walklevel_core::treadmill::{
    DriverKind, Meter, TelemetryFrame, TelemetrySpec, ftms_proto as ftms,
};

fn recording(name: &str) -> Vec<(f64, Vec<String>)> {
    let path = format!(
        "{}/tests/fixtures/protokoly/{name}",
        env!("CARGO_MANIFEST_DIR")
    );
    let text = std::fs::read_to_string(&path).expect("nagranie");
    text.lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
        .map(|l| {
            let mut parts = l.split_whitespace();
            let t = parts.next().and_then(|t| t.parse().ok()).expect("czas");
            (t, parts.map(String::from).collect())
        })
        .collect()
}

fn ftms_spec() -> TelemetrySpec {
    TelemetrySpec::for_driver(DriverKind::Ftms, &ftms::DEFAULT_CAPS)
}

fn ks_spec() -> TelemetrySpec {
    TelemetrySpec::for_driver(DriverKind::KingSmith, &ks::DEFAULT_CAPS)
}

fn no_counters(spec: TelemetrySpec) -> TelemetrySpec {
    TelemetrySpec {
        distance: None,
        ..spec
    }
}

fn speed_frame(t: f64, kmh: f64) -> TelemetryFrame {
    TelemetryFrame::data(t, Some(kmh))
}

fn close(a: f64, b: f64, tol: f64) -> bool {
    (a - b).abs() <= tol
}

/// Taśma ze „sprawdzonymi” licznikami jak Urevo: dystans co 10 m, kroki, czas co 1 s.
struct Belt {
    t: f64,
    m: f64,
    steps: f64,
    elapsed: f64,
    base: (u64, u64, u64),
    counters: bool,
    with_steps: bool,
}

impl Belt {
    fn new(t: f64) -> Self {
        Self {
            t,
            m: 0.0,
            steps: 0.0,
            elapsed: 0.0,
            base: (0, 0, 0),
            counters: true,
            with_steps: true,
        }
    }

    fn frame(&self, kmh: f64) -> TelemetryFrame {
        let (d0, s0, e0) = self.base;
        TelemetryFrame {
            distance_m: self
                .counters
                .then(|| d0 + (self.m / 10.0).floor() as u64 * 10),
            steps: self.with_steps.then(|| s0 + self.steps as u64),
            elapsed_s: self.counters.then(|| e0 + self.elapsed as u64),
            ..speed_frame(self.t, kmh)
        }
    }

    /// Taśma jedzie `kmh` przez `dt`, kroki z kadencją `cadence` na minutę.
    fn advance(&mut self, kmh: f64, cadence: f64, dt: f64) {
        self.t += dt;
        self.m += kmh / 3.6 * dt;
        self.steps += cadence / 60.0 * dt;
        if kmh > 0.0 {
            self.elapsed += dt;
        }
    }
}

// --- 6.3. Dystans i czas ruchu -----------------------------------------------------------

#[test]
fn i1_constant_speed_without_counter() {
    let mut meter = Meter::new(no_counters(ftms_spec()));
    for t in 0..=600 {
        meter.feed(&speed_frame(f64::from(t), 3.0));
    }
    let totals = meter.totals();
    assert!(close(totals.distance_m, 500.0, 0.01), "{totals:?}");
    assert!(close(totals.moving_s, 600.0, 1e-9));
}

#[test]
fn i2_counter_keeps_distance_within_one_quantum() {
    let mut meter = Meter::new(ftms_spec());
    let mut belt = Belt::new(0.0);
    for _ in 0..=600 {
        meter.feed(&belt.frame(3.0));
        let c = meter.counter_distance_m() as f64;
        let d = meter.totals().distance_m;
        assert!(d >= c - 10.0 - 1e-9 && d <= c + 10.0 + 1e-9, "D {d}, C {c}");
        belt.advance(3.0, 80.0, 1.0);
    }
    assert!(close(meter.totals().distance_m, 500.0, 10.0));
}

#[test]
fn i3_slow_walk_counts_regardless_of_game_threshold() {
    let mut meter = Meter::new(no_counters(ks_spec()));
    for i in 0..=180 {
        meter.feed(&speed_frame(f64::from(i) * 2.0, 0.5));
    }
    let totals = meter.totals();
    assert!(close(totals.distance_m, 50.0, 0.5), "{totals:?}");
    assert!(close(totals.moving_s, 360.0, 1e-9));
}

#[test]
fn i4_braking_counts_to_the_last_meter() {
    let mut meter = Meter::new(no_counters(ftms_spec()));
    for (t, v) in [(0.0, 3.0), (1.0, 2.0), (2.0, 1.0), (3.0, 0.0)] {
        meter.feed(&speed_frame(t, v));
    }
    let totals = meter.totals();
    assert!(close(totals.distance_m, 6.0 / 3.6, 1e-3), "{totals:?}");
    assert!(close(totals.moving_s, 3.0, 1e-9));
}

#[test]
fn i5_i6_integral_is_clamped_to_counter() {
    // długi odstęp ramek (telemetry_s 100) - jeden odcinek całki i jeden przyrost licznika
    let spec = TelemetrySpec {
        telemetry_s: 100.0,
        ..ftms_spec()
    };
    for (kmh, want) in [(115.0 / 115.0 * 3.6, 110.0), (85.0 / 115.0 * 3.6, 90.0)] {
        let mut meter = Meter::new(spec);
        for (t, raw) in [(0.0, 0), (115.0, 100)] {
            meter.feed(&TelemetryFrame {
                distance_m: Some(raw),
                ..speed_frame(t, kmh)
            });
        }
        assert!(close(meter.totals().distance_m, want, 1e-9), "{kmh}");
    }
}

#[test]
fn i7_gap_is_filled_from_treadmill_counters() {
    let mut meter = Meter::new(ftms_spec());
    let mut belt = Belt::new(0.0);
    for _ in 0..=10 {
        meter.feed(&belt.frame(3.6));
        belt.advance(3.6, 90.0, 1.0);
    }
    assert!(close(meter.totals().distance_m, 10.0, 1e-9));
    assert!(close(meter.totals().moving_s, 10.0, 1e-9));
    for _ in 0..49 {
        belt.advance(3.6, 90.0, 1.0); // przerwa w połączeniu: bieżnia liczy dalej
    }
    meter.feed(&belt.frame(3.6));
    let totals = meter.totals();
    assert!(close(totals.distance_m, 60.0, 1e-9), "{totals:?}");
    assert!(close(totals.moving_s, 60.0, 1e-9), "{totals:?}");
}

#[test]
fn i8_i9_gap_depends_on_telemetry_rate() {
    let mut kingsmith = Meter::new(no_counters(ks_spec()));
    kingsmith.feed(&speed_frame(0.0, 3.6));
    kingsmith.feed(&speed_frame(5.9, 3.6));
    assert!(close(kingsmith.totals().distance_m, 5.9, 1e-9));
    let mut ftms = Meter::new(no_counters(ftms_spec()));
    ftms.feed(&speed_frame(0.0, 3.6));
    ftms.feed(&speed_frame(3.5, 3.6));
    assert_eq!(ftms.totals().distance_m, 0.0);
}

#[test]
fn i10_clock_going_back_is_skipped() {
    let mut meter = Meter::new(no_counters(ftms_spec()));
    for t in [10.0, 9.0, 11.0] {
        meter.feed(&speed_frame(t, 3.6));
    }
    let totals = meter.totals();
    assert!(close(totals.distance_m, 1.0, 1e-9), "{totals:?}");
    assert!(totals.moving_s >= 0.0);
}

#[test]
fn i11_standing_belt_counts_nothing() {
    let mut meter = Meter::new(ftms_spec());
    let belt = Belt::new(0.0);
    for t in 0..300 {
        meter.feed(&TelemetryFrame {
            t: f64::from(t),
            ..belt.frame(0.0)
        });
    }
    assert_eq!(meter.totals().distance_m, 0.0);
    assert_eq!(meter.totals().moving_s, 0.0);
}

/// Ramki X21 z zapisu (tablica v5) przez część odbiorczą KingSmith.
fn x21_frames() -> Vec<TelemetryFrame> {
    let mut rx = ks::Telemetry::new(&["v5"]);
    let channel = "00011234-0000-1000-8000-00805f9b34fb";
    let mut frames = Vec::new();
    for (t, rest) in recording("x21-ramki.txt") {
        let raw = hex::decode(&rest[0]).expect("hex");
        frames.extend(rx.feed(channel, &raw, t));
    }
    frames
}

#[test]
fn i12_x21_recording_replay() {
    let frames = x21_frames();
    let mut meter = Meter::new(ks_spec());
    let mut integral = Meter::new(no_counters(ks_spec()));
    for f in &frames {
        meter.feed(f);
        integral.feed(f);
    }
    let totals = meter.totals();
    assert_eq!(totals.steps, 230);
    assert_eq!(meter.counter_distance_m(), 100);
    assert!(close(integral.totals().distance_m, 110.2, 0.05));
    assert!(close(totals.distance_m, 108.2, 0.5), "{totals:?}");
    assert!(close(totals.moving_s, 166.7, 1.0), "{totals:?}");
}

/// Pełna sesja Urevo z 2026-11-24 (2208 ramek Treadmill Data).
fn urevo_session() -> Vec<TelemetryFrame> {
    let mut rx = ftms::Telemetry::new(ftms::DEFAULT_CAPS);
    recording("urevo-5l-sesja-37min.txt")
        .into_iter()
        .filter_map(|(t, rest)| rx.on_data(&hex::decode(&rest[0]).expect("hex"), t))
        .collect()
}

#[test]
fn i13_full_urevo_session_replay() {
    let frames = urevo_session();
    assert_eq!(frames.len(), 2208);
    let mut meter = Meter::new(ftms_spec());
    for f in &frames {
        meter.feed(f);
    }
    let totals = meter.totals();
    assert_eq!(totals.steps, 3061);
    assert_eq!(meter.counter_distance_m(), 1900);
    assert!(close(totals.distance_m, 1910.0, 1.0), "{totals:?}");
    assert!(close(totals.moving_s, 2193.0, 5.0), "{totals:?}");
    let stride = totals.distance_m / totals.steps as f64;
    assert!(close(stride, 0.62, 0.01), "krok {stride}");
    let cadence = totals.steps as f64 / (totals.moving_s / 60.0);
    assert!(close(cadence, 83.7, 0.5), "kadencja {cadence}");
}

#[test]
fn i14_urevo_excerpt_pause_keeps_counters() {
    let mut rx = ftms::Telemetry::new(ftms::DEFAULT_CAPS);
    let mut meter = Meter::new(ftms_spec());
    let mut max_speed: f64 = 0.0;
    let mut after_pause = Vec::new();
    let mut paused = false;
    for (t, rest) in recording("urevo-5l-ramki.txt") {
        let raw = hex::decode(&rest[1]).expect("hex");
        match rest[0].as_str() {
            "data" => {
                if let Some(f) = rx.on_data(&raw, t) {
                    max_speed = max_speed.max(f.speed_kmh.unwrap_or(0.0));
                    meter.feed(&f);
                    if paused {
                        after_pause.push(f);
                    }
                }
            }
            "status" => {
                rx.on_status(&raw);
                paused |= raw.first() == Some(&ftms::STATUS_STOPPED);
            }
            _ => {} // polecenia aplikacji - tylko kontekst
        }
    }
    assert_eq!(max_speed, 4.0);
    assert_eq!(meter.counter_distance_m(), 10);
    assert_eq!(meter.totals().steps, 0);
    assert!(!after_pause.is_empty());
    for f in after_pause {
        assert_eq!(f.speed_kmh, Some(0.0));
        assert_eq!((f.distance_m, f.elapsed_s), (Some(10), Some(7)));
    }
}

// --- 6.4. Kadencja --------------------------------------------------------------------------

/// X21: kroki +3 co 2 s.
fn x21_steps(meter: &mut Meter, until: u32, heartbeats: bool) {
    for i in 0..=until / 2 {
        let t = f64::from(i) * 2.0;
        meter.feed(&TelemetryFrame {
            steps: Some(u64::from(i) * 3),
            ..speed_frame(t, 2.0)
        });
        if heartbeats {
            for k in 1..=2 {
                let hb = t + 0.75 * f64::from(k);
                meter.feed(&TelemetryFrame::heartbeat(hb, Some(2.0), Some(0)));
            }
        }
    }
}

#[test]
fn k1_k2_k3_k6_cadence_window() {
    let mut meter = Meter::new(ks_spec());
    x21_steps(&mut meter, 20, false);
    let k1 = meter.cadence(20.0).expect("kadencja");
    assert!(close(k1, 90.0, 1.0), "{k1}");
    let mut short = Meter::new(ks_spec());
    x21_steps(&mut short, 4, false);
    assert_eq!(short.cadence(4.0), None); // K2: za krótko
    for t in [21.0, 22.0, 23.0] {
        meter.feed(&TelemetryFrame {
            steps: Some(30),
            ..speed_frame(t, 0.0)
        });
    }
    assert_eq!(meter.cadence(24.0), None); // K3: taśma stoi
    let mut with_hb = Meter::new(ks_spec());
    x21_steps(&mut with_hb, 20, true);
    assert_eq!(with_hb.cadence(20.0), Some(k1)); // K6: sygnały życia bez wpływu
}

#[test]
fn k4_counter_reset_does_not_freeze_cadence() {
    let mut meter = Meter::new(ftms_spec());
    let mut steps: f64 = 5000.0;
    for t in 0..=20 {
        meter.feed(&TelemetryFrame {
            steps: Some(steps as u64),
            ..speed_frame(f64::from(t), 3.0)
        });
        steps += 1.5;
    }
    steps = 0.0; // bieżnia wyzerowała licznik
    for t in 21..=40 {
        meter.feed(&TelemetryFrame {
            steps: Some(steps as u64),
            ..speed_frame(f64::from(t), 3.0)
        });
        steps += 1.5;
    }
    let cadence = meter.cadence(40.0).expect("kadencja");
    assert!((85.0..=95.0).contains(&cadence), "{cadence}");
}

#[test]
fn k5_urevo_cadence_at_constant_speed() {
    let frames = urevo_session();
    let mut meter = Meter::new(ftms_spec());
    let mut since: Option<f64> = None;
    let mut found = None;
    for f in &frames {
        meter.feed(f);
        if f.speed_kmh == Some(2.5) {
            let start = *since.get_or_insert(f.t);
            if f.t - start >= 60.0 {
                found = meter.cadence(f.t);
                break;
            }
        } else {
            since = None;
        }
    }
    let cadence = found.expect("60 s przy 2,5 km/h");
    assert!(close(cadence, 74.0, 3.0), "{cadence}");
}

// --- 6.5. Sesja -----------------------------------------------------------------------------

/// Tracker z ręcznym zegarem (ten sam dla sesji i ramek) i bazą w pamięci.
struct Day {
    clock: Arc<Mutex<f64>>,
    tracker: SessionTracker,
    history: History,
}

impl Day {
    fn new(start: f64) -> Self {
        let clock = Arc::new(Mutex::new(start));
        let c = clock.clone();
        let mut tracker = SessionTracker::with_clock(Box::new(move || *c.lock().expect("zegar")));
        let history = History::open_in_memory().expect("baza");
        tracker
            .set_owner(&history, "p1", "Ala", "g1", "PEAK")
            .expect("osoba");
        tracker.set_spec(ftms_spec());
        Self {
            clock,
            tracker,
            history,
        }
    }

    fn set_time(&self, t: f64) {
        *self.clock.lock().expect("zegar") = t;
    }

    fn feed(&mut self, frame: TelemetryFrame) {
        self.set_time(frame.t);
        self.tracker.feed(&self.history, &frame).expect("ramka");
    }

    fn tick(&mut self, t: f64) {
        self.set_time(t);
        self.tracker.tick(&self.history).expect("tik");
    }

    /// Chód `seconds` przy `kmh`, ramka co sekundę.
    fn walk(&mut self, belt: &mut Belt, kmh: f64, seconds: u32) {
        for _ in 0..seconds {
            self.feed(belt.frame(kmh));
            let cadence = if kmh > 0.4 { 40.0 + 14.0 * kmh } else { 0.0 };
            belt.advance(kmh, cadence, 1.0);
        }
    }

    fn saved(&self) -> Vec<walklevel_core::model::Session> {
        let mut all = self.history.sessions(0.0, None, None, None).expect("sesje");
        all.sort_by(|a, b| a.start.total_cmp(&b.start));
        all
    }
}

const T0: f64 = 1_800_000_000.0;

#[test]
fn s1_session_ends_after_standing_and_end_is_last_move() {
    let mut day = Day::new(T0);
    let mut belt = Belt::new(T0);
    day.walk(&mut belt, 2.5, 301);
    let last_frame = belt.t - 1.0;
    day.walk(&mut belt, 0.0, 122);
    assert!(day.tracker.current().is_none());
    let saved = day.saved();
    assert_eq!(saved.len(), 1);
    let s = &saved[0];
    assert!(close(s.distance_km * 1000.0, 208.3, 10.0), "{s:?}");
    assert!(
        close(s.end, last_frame + 1.0, 1e-9),
        "koniec {}",
        s.end - T0
    );
}

#[test]
fn s2_session_does_not_hang_without_data() {
    let mut day = Day::new(T0);
    let mut belt = Belt::new(T0);
    day.walk(&mut belt, 2.5, 300);
    let last = belt.t;
    for i in 1..=121 {
        day.tick(last + f64::from(i));
        if i < 119 {
            assert!(day.tracker.current().is_some(), "za wcześnie ({i} s)");
        }
    }
    assert!(day.tracker.current().is_none());
    assert_eq!(day.saved().len(), 1);
}

#[test]
fn s3_data_after_hours_starts_a_new_session() {
    let mut day = Day::new(T0);
    let mut belt = Belt::new(T0);
    day.walk(&mut belt, 3.0, 120);
    belt.t += 3.0 * 3600.0; // utrata połączenia na 3 h (bez tików)
    belt.m += 5000.0; // bieżnia jechała z kimś innym
    belt.steps += 6000.0;
    day.walk(&mut belt, 3.0, 120);
    day.tracker.finish(&day.history).expect("koniec");
    let saved = day.saved();
    assert_eq!(saved.len(), 2);
    assert!(
        saved[1].steps < 200,
        "bez fantomowych kroków: {}",
        saved[1].steps
    );
    assert!(saved[0].end - saved[0].start < 200.0);
}

#[test]
fn s4_s6_s9_new_tracker_and_running_counters_start_from_zero() {
    let mut day = Day::new(T0);
    let mut belt = Belt::new(T0);
    belt.base = (390, 808, 763); // trening na bieżni zaczął się wcześniej
    day.walk(&mut belt, 3.0, 61);
    let s = day.tracker.current().expect("sesja");
    assert!(close(s.steps as f64, 82.0, 2.0), "kroki {}", s.steps); // nie 891
    assert!(
        close(s.distance_km * 1000.0, 50.0, 1.0),
        "{}",
        s.distance_km
    );
    let live = day.tracker.live(belt.t);
    assert_eq!(live.steps, Some(s.steps));
    assert!(
        live.cadence.is_some_and(|c| close(c, 82.0, 3.0)),
        "{live:?}"
    );
}

#[test]
fn s5_pause_with_kept_counters_is_one_session() {
    let mut day = Day::new(T0);
    let mut belt = Belt::new(T0);
    day.walk(&mut belt, 3.0, 120);
    day.walk(&mut belt, 0.0, 100); // pauza, liczniki zachowane
    day.walk(&mut belt, 3.0, 120);
    day.tracker.finish(&day.history).expect("koniec");
    let saved = day.saved();
    assert_eq!(saved.len(), 1);
    let want = belt.steps as i64;
    assert!(
        close(saved[0].steps as f64, want as f64, 2.0),
        "{}",
        saved[0].steps
    );
}

#[test]
fn s7_person_change_splits_steps_without_loss() {
    let mut day = Day::new(T0);
    let mut belt = Belt::new(T0);
    day.walk(&mut belt, 3.0, 90);
    day.tracker
        .set_owner(&day.history, "p2", "Bartek", "g1", "PEAK")
        .expect("osoba");
    day.walk(&mut belt, 3.0, 90);
    day.tracker.finish(&day.history).expect("koniec");
    let saved = day.saved();
    assert_eq!(saved.len(), 2);
    let sum: i64 = saved.iter().map(|s| s.steps).sum();
    let counter = (belt.steps - 82.0 / 60.0) as i64; // ostatnia ramka sprzed ostatniego kroku
    assert!(close(sum as f64, counter as f64, 2.0), "{sum} vs {counter}");
}

#[test]
fn s8_treadmill_without_step_counter() {
    let mut day = Day::new(T0);
    let mut belt = Belt::new(T0);
    belt.with_steps = false;
    day.walk(&mut belt, 3.0, 601);
    let s = day.tracker.current().expect("sesja").clone();
    assert_eq!(s.steps, 0);
    assert!(
        close(s.distance_km * 1000.0, 500.0, 10.0),
        "{}",
        s.distance_km
    );
    assert_eq!(day.tracker.live(belt.t).steps, None); // kafelek „–”
}

#[test]
fn s10_short_session_without_steps_is_not_saved() {
    let mut day = Day::new(T0);
    let mut belt = Belt::new(T0);
    belt.with_steps = false;
    day.walk(&mut belt, 1.0, 5);
    day.tracker.finish(&day.history).expect("koniec");
    assert!(day.saved().is_empty());
}

#[test]
fn source_change_resets_counters_c9() {
    let mut day = Day::new(T0);
    let mut sim = Belt::new(T0);
    day.tracker
        .set_source(
            &day.history,
            SessionSource {
                source: "symulator".into(),
                ..SessionSource::default()
            },
        )
        .expect("symulator");
    day.walk(&mut sim, 3.0, 60);
    day.tracker
        .set_source(
            &day.history,
            SessionSource {
                model: "Urevo SpaceWalk 5L".into(),
                driver: "ftms".into(),
                ..SessionSource::default()
            },
        )
        .expect("bieżnia");
    let mut belt = Belt::new(sim.t + 1.0);
    belt.base = (2000, 3000, 900);
    day.walk(&mut belt, 3.0, 3);
    let s = day.tracker.current().expect("sesja bieżni");
    assert!(s.steps <= 5, "bez +2950: {}", s.steps);
    assert_eq!(s.source, "bieznia");
}
