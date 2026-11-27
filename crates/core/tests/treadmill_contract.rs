//! Kontrakt sterownika (port tests/test_kontrakt.py) na symulatorze - te same scenariusze
//! przejdą moduły KingSmith i FTMS, gdy druga fala doda ich połączenie BLE.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use walklevel_core::treadmill::simulator::{self, SimulatorDriver};
use walklevel_core::treadmill::{
    Capabilities, Clock, Driver, DriverKind, FrameCallback, StartResult, TelemetryFrame,
};

struct Harness {
    driver: Arc<SimulatorDriver>,
    readings: Arc<Mutex<Vec<TelemetryFrame>>>,
    stop: Arc<AtomicBool>,
    pump: tokio::task::JoinHandle<()>,
}

impl Harness {
    async fn new() -> Self {
        let clock = Arc::new(AtomicU64::new(0f64.to_bits()));
        let readings = Arc::new(Mutex::new(Vec::new()));
        let sink_readings = readings.clone();
        let sink: FrameCallback = Arc::new(move |r| sink_readings.lock().expect("odczyty").push(r));
        let tick = clock.clone();
        let now: Clock = Arc::new(move || f64::from_bits(tick.load(Ordering::SeqCst)));
        let caps = Capabilities {
            start_delay_s: 0.01,
            ..simulator::CAPS
        };
        let driver = Arc::new(SimulatorDriver::new(sink, now, caps));
        driver.start().await.expect("start");
        let stop = Arc::new(AtomicBool::new(false));
        let (pumped, stopped) = (driver.clone(), stop.clone());
        // bieżnia „żyje” w tle - jak prawdziwa, co chwilę ramka
        let pump = tokio::spawn(async move {
            while !stopped.load(Ordering::SeqCst) {
                let t = f64::from_bits(clock.load(Ordering::SeqCst)) + 1.0;
                clock.store(t.to_bits(), Ordering::SeqCst);
                pumped.advance();
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        });
        Self {
            driver,
            readings,
            stop,
            pump,
        }
    }

    async fn settle(&self, predicate: impl Fn(&SimulatorDriver) -> bool) -> bool {
        for _ in 0..300 {
            if predicate(&self.driver) {
                return true;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        predicate(&self.driver)
    }

    async fn finish(self) {
        self.stop.store(true, Ordering::SeqCst);
        self.pump.await.expect("pompa");
    }
}

fn speed_of(driver: &SimulatorDriver) -> f64 {
    driver.belt_speed().map_or(-1.0, |r| r.0)
}

#[tokio::test]
async fn identity_and_capabilities() {
    let h = Harness::new().await;
    let driver: &dyn Driver = h.driver.as_ref();
    let caps = driver.capabilities();
    assert!(0.0 < caps.speed_min && caps.speed_min < caps.speed_max);
    assert!(caps.control);
    assert_eq!(driver.info().driver, DriverKind::Simulator);
    assert!(!driver.info().model.is_empty() && !driver.describe().is_empty());
    assert!(!driver.busy());
    h.finish().await;
}

#[tokio::test]
async fn start_speed_stop() {
    let h = Harness::new().await;
    assert_eq!(
        h.driver.start_belt().await.expect("start"),
        StartResult::Started
    );
    assert!(h.settle(|d| d.belt_running() == Some(true)).await);
    assert!(h.driver.set_speed(2.5).await.expect("prędkość"));
    assert!(
        h.settle(|d| (speed_of(d) - 2.5).abs() < 0.15).await,
        "{}",
        speed_of(&h.driver)
    );
    h.driver.stop_belt(true).await.expect("stop");
    assert!(h.settle(|d| d.belt_running() == Some(false)).await);
    assert!(!h.driver.busy());
    assert!(
        !h.readings.lock().expect("odczyty").is_empty(),
        "sterownik nie oddał żadnego odczytu"
    );
    h.finish().await;
}

#[tokio::test]
async fn speed_stays_within_capabilities() {
    let h = Harness::new().await;
    let caps = h.driver.capabilities();
    h.driver.start_belt().await.expect("start");
    h.driver.set_speed(99.0).await.expect("prędkość");
    assert!(h.settle(|d| speed_of(d) > 0.0).await);
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert!(speed_of(&h.driver) <= caps.speed_max + 0.05);
    h.finish().await;
}

#[tokio::test]
async fn running_belt_start_is_a_no_op() {
    let h = Harness::new().await;
    assert_eq!(
        h.driver.start_belt().await.expect("start"),
        StartResult::Started
    );
    assert!(h.settle(|d| d.belt_running() == Some(true)).await);
    let before = h.driver.counts().commands;
    assert_eq!(
        h.driver.start_belt().await.expect("start"),
        StartResult::Started
    );
    assert!(h.driver.counts().commands - before <= 1);
    h.finish().await;
}
