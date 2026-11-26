//! Źródło danych bieżni dla pętli sterowania: `treadmill::BleWorker` z rdzenia (Bluetooth
//! przez btleplug albo symulator - ten sam kontrakt sterownika). Tu tylko uruchomienie
//! workera na runtime Tauri i odbiór zdarzeń bez blokowania pętli.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::sync::mpsc;
use walklevel_core::treadmill::{BleWorker, BtleplugBackend, Clock, ProfileRegistry, WorkerEvent};

pub struct Bieznia {
    worker: BleWorker,
    events: Mutex<mpsc::UnboundedReceiver<WorkerEvent>>,
}

impl Bieznia {
    /// Worker na runtime Tauri; `clock` - zegar ramek (ten sam co w pętli).
    pub fn spawn(registry: ProfileRegistry, clock: Clock) -> Self {
        let handle = tauri::async_runtime::handle();
        let runtime = handle.inner().clone();
        let (worker, events) =
            BleWorker::spawn(&runtime, Arc::new(BtleplugBackend::new()), registry, clock);
        Self {
            worker,
            events: Mutex::new(events),
        }
    }

    pub fn worker(&self) -> &BleWorker {
        &self.worker
    }

    /// Zdarzenia od ostatniej klatki.
    pub fn events(&self) -> Vec<WorkerEvent> {
        let mut rx = self.events.lock().unwrap_or_else(|p| p.into_inner());
        let mut out = Vec::new();
        while let Ok(e) = rx.try_recv() {
            out.push(e);
        }
        out
    }

    /// Stop taśmy przy zamykaniu (blokuje najwyżej `timeout`); `true` - stop wysłany.
    pub fn stop_belt_now(&self, timeout: Duration) -> bool {
        let worker = self.worker.clone();
        tauri::async_runtime::block_on(async move { worker.stop_belt_now(timeout).await })
    }

    pub fn shutdown(&self) {
        let worker = self.worker.clone();
        tauri::async_runtime::block_on(
            async move { worker.shutdown(Duration::from_secs(4)).await },
        );
    }
}
