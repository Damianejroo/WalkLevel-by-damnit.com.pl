//! Warstwa Bluetooth: skanowanie, wyszukanie zapamiętanej bieżni, połączenie GATT i raport
//! diagnostyczny (port części bieznia/ble/worker.py i bieznia/ble/diagnostics.py).
//!
//! Worker i sterowniki znają tylko cechy [`BleBackend`] i [`BleLink`] - w aplikacji to
//! btleplug ([`BtleplugBackend`]), w testach wirtualna bieżnia bez radia.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::teksty::t;

use futures::future::BoxFuture;
use futures::{FutureExt, StreamExt};
use tokio::sync::mpsc;

use super::kontrakt::{
    Clock, DriverError, FrameCallback, GattCharacteristic, GattClient, GattService,
};
use super::profiles::ProfileRegistry;

/// Urządzenie z ogłoszenia Bluetooth (skan).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Advertisement {
    /// "AA:BB:CC:DD:EE:FF" (jak w 0.5 - zapamiętany adres bieżni w osoby.json)
    pub address: String,
    pub name: Option<String>,
    pub rssi: Option<i16>,
    /// UUID usług z ogłoszenia (małe litery)
    pub services: Vec<String>,
    pub manufacturer: Vec<(u16, Vec<u8>)>,
}

/// Powiadomienie z subskrybowanej charakterystyki.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notification {
    /// UUID charakterystyki (małe litery)
    pub characteristic_uuid: String,
    pub data: Vec<u8>,
}

/// Błędy Bluetooth z komunikatem dla użytkownika (docs/teksty.md, ble.powod.*).
#[derive(Debug, thiserror::Error)]
pub enum BleError {
    #[error("{}", t("ble.powod.bt_wylaczony", &[]))]
    BluetoothOff,
    #[error("{}", t("ble.powod.nie_widze", &[]))]
    NotFound,
    #[error("{}", t("ble.powod.timeout", &[]))]
    Timeout,
    #[error("{}", t("ble.powod.inny", &[("szczegoly", .0)]))]
    Other(String),
    #[error(transparent)]
    Driver(#[from] DriverError),
}

/// Radio Bluetooth: skan, wyszukanie urządzenia, połączenie.
pub trait BleBackend: Send + Sync + 'static {
    /// Urządzenia widoczne przez `duration`.
    fn scan(&self, duration: Duration) -> BoxFuture<'_, Result<Vec<Advertisement>, BleError>>;

    /// Zapamiętana bieżnia: po adresie, a gdy adresu nie widać - po tej samej nazwie.
    fn find(
        &self,
        address: String,
        name: String,
        timeout: Duration,
    ) -> BoxFuture<'_, Result<Option<Advertisement>, BleError>>;

    /// Połączenie z urządzeniem (adres z [`BleBackend::find`] albo skanu).
    fn connect(
        &self,
        address: String,
        timeout: Duration,
    ) -> BoxFuture<'_, Result<Arc<dyn BleLink>, BleError>>;
}

/// Połączone urządzenie: GATT i strumień powiadomień.
pub trait BleLink: GattClient {
    /// Powiadomienia ze wszystkich subskrypcji; `None` z odbiornika = połączenie zerwane.
    fn notifications(
        &self,
    ) -> BoxFuture<'_, Result<mpsc::UnboundedReceiver<Notification>, BleError>>;

    fn disconnect(&self) -> BoxFuture<'_, ()>;
}

/// Wybór z listy ogłoszeń: adres (bez względu na wielkość liter), potem nazwa.
pub fn pick<'a>(
    found: &'a [Advertisement],
    address: &str,
    name: &str,
) -> Option<&'a Advertisement> {
    found
        .iter()
        .find(|a| !address.is_empty() && a.address.eq_ignore_ascii_case(address))
        .or_else(|| {
            found
                .iter()
                .find(|a| !name.is_empty() && a.name.as_deref() == Some(name))
        })
}

// --- btleplug ------------------------------------------------------------------------------

use btleplug::api::{
    Central, CentralEvent, CentralState, CharPropFlags, Characteristic, Manager as _,
    Peripheral as _, ScanFilter, WriteType,
};
use btleplug::platform::{Adapter, Manager, Peripheral};

fn ble_error(e: btleplug::Error) -> BleError {
    match e {
        btleplug::Error::NoAdapterAvailable | btleplug::Error::PermissionDenied => {
            BleError::BluetoothOff
        }
        btleplug::Error::DeviceNotFound => BleError::NotFound,
        btleplug::Error::TimedOut(_) => BleError::Timeout,
        other => BleError::Other(other.to_string()),
    }
}

fn gatt_error(e: btleplug::Error) -> DriverError {
    DriverError::Gatt(e.to_string())
}

fn property_names(flags: CharPropFlags) -> Vec<String> {
    [
        (CharPropFlags::READ, "read"),
        (CharPropFlags::WRITE, "write"),
        (
            CharPropFlags::WRITE_WITHOUT_RESPONSE,
            "write-without-response",
        ),
        (CharPropFlags::NOTIFY, "notify"),
        (CharPropFlags::INDICATE, "indicate"),
    ]
    .into_iter()
    .filter(|(f, _)| flags.contains(*f))
    .map(|(_, n)| n.to_string())
    .collect()
}

/// Bluetooth Windows (WinRT) przez btleplug. Adapter pobierany przy pierwszym użyciu.
#[derive(Default)]
pub struct BtleplugBackend {
    adapter: tokio::sync::Mutex<Option<Adapter>>,
}

impl BtleplugBackend {
    pub fn new() -> Self {
        Self::default()
    }

    async fn adapter(&self) -> Result<Adapter, BleError> {
        let mut slot = self.adapter.lock().await;
        if let Some(adapter) = slot.as_ref() {
            return Ok(adapter.clone());
        }
        let manager = Manager::new().await.map_err(ble_error)?;
        let adapter = manager
            .adapters()
            .await
            .map_err(ble_error)?
            .into_iter()
            .next()
            .ok_or(BleError::BluetoothOff)?;
        *slot = Some(adapter.clone());
        Ok(adapter)
    }

    async fn powered(&self) -> Result<Adapter, BleError> {
        let adapter = self.adapter().await?;
        if matches!(adapter.adapter_state().await, Ok(CentralState::PoweredOff)) {
            return Err(BleError::BluetoothOff);
        }
        Ok(adapter)
    }

    async fn advertisements(
        adapter: &Adapter,
    ) -> Result<Vec<(Peripheral, Advertisement)>, BleError> {
        let mut out = Vec::new();
        for p in adapter.peripherals().await.map_err(ble_error)? {
            let Ok(Some(props)) = p.properties().await else {
                continue;
            };
            let adv = Advertisement {
                address: p.address().to_string(),
                name: props.local_name.or(props.advertisement_name),
                rssi: props.rssi,
                services: props
                    .services
                    .iter()
                    .map(|u| u.to_string().to_lowercase())
                    .collect(),
                manufacturer: props.manufacturer_data.into_iter().collect(),
            };
            out.push((p, adv));
        }
        Ok(out)
    }
}

impl BleBackend for BtleplugBackend {
    fn scan(&self, duration: Duration) -> BoxFuture<'_, Result<Vec<Advertisement>, BleError>> {
        async move {
            let adapter = self.powered().await?;
            adapter
                .start_scan(ScanFilter::default())
                .await
                .map_err(ble_error)?;
            tokio::time::sleep(duration).await;
            let _ = adapter.stop_scan().await; // skan i tak kończy się sam
            Ok(Self::advertisements(&adapter)
                .await?
                .into_iter()
                .map(|(_, a)| a)
                .collect())
        }
        .boxed()
    }

    fn find(
        &self,
        address: String,
        name: String,
        timeout: Duration,
    ) -> BoxFuture<'_, Result<Option<Advertisement>, BleError>> {
        async move {
            let adapter = self.powered().await?;
            adapter
                .start_scan(ScanFilter::default())
                .await
                .map_err(ble_error)?;
            let deadline = Instant::now() + timeout;
            let found = loop {
                let ads: Vec<Advertisement> = Self::advertisements(&adapter)
                    .await?
                    .into_iter()
                    .map(|(_, a)| a)
                    .collect();
                // adres wygrywa; po nazwie dopiero pod koniec czasu (adres mógł się zmienić)
                let by_address = ads
                    .iter()
                    .find(|a| a.address.eq_ignore_ascii_case(&address))
                    .cloned();
                if by_address.is_some() {
                    break by_address;
                }
                if Instant::now() >= deadline {
                    break pick(&ads, "", &name).cloned();
                }
                tokio::time::sleep(Duration::from_millis(500)).await;
            };
            let _ = adapter.stop_scan().await;
            Ok(found)
        }
        .boxed()
    }

    fn connect(
        &self,
        address: String,
        timeout: Duration,
    ) -> BoxFuture<'_, Result<Arc<dyn BleLink>, BleError>> {
        async move {
            let adapter = self.powered().await?;
            let peripheral = Self::advertisements(&adapter)
                .await?
                .into_iter()
                .find(|(_, a)| a.address.eq_ignore_ascii_case(&address))
                .map(|(p, _)| p)
                .ok_or(BleError::NotFound)?;
            let ready = async {
                tokio::time::timeout(timeout, peripheral.connect())
                    .await
                    .map_err(|_| BleError::Timeout)?
                    .map_err(ble_error)?;
                tokio::time::timeout(timeout, peripheral.discover_services())
                    .await
                    .map_err(|_| BleError::Timeout)?
                    .map_err(ble_error)
            };
            if let Err(e) = ready.await {
                // połączenie w połowie (np. bez listy usług) nie może zostać otwarte
                let _ = peripheral.disconnect().await;
                return Err(e);
            }
            let characteristics: Vec<Characteristic> =
                peripheral.characteristics().into_iter().collect();
            let services = peripheral
                .services()
                .into_iter()
                .map(|s| GattService {
                    uuid: s.uuid.to_string().to_lowercase(),
                    characteristics: s
                        .characteristics
                        .iter()
                        .map(|c| GattCharacteristic {
                            service_uuid: c.service_uuid.to_string().to_lowercase(),
                            uuid: c.uuid.to_string().to_lowercase(),
                            properties: property_names(c.properties),
                        })
                        .collect(),
                })
                .collect();
            let link: Arc<dyn BleLink> = Arc::new(BtleplugLink {
                adapter,
                peripheral,
                services,
                characteristics,
            });
            Ok(link)
        }
        .boxed()
    }
}

/// Połączenie btleplug.
struct BtleplugLink {
    adapter: Adapter,
    peripheral: Peripheral,
    services: Vec<GattService>,
    characteristics: Vec<Characteristic>,
}

impl BtleplugLink {
    fn native(&self, c: &GattCharacteristic) -> Result<&Characteristic, DriverError> {
        self.characteristics
            .iter()
            .find(|n| {
                n.uuid.to_string().eq_ignore_ascii_case(&c.uuid)
                    && n.service_uuid
                        .to_string()
                        .eq_ignore_ascii_case(&c.service_uuid)
            })
            .ok_or_else(|| DriverError::Gatt(format!("brak charakterystyki {}", c.uuid)))
    }
}

impl GattClient for BtleplugLink {
    fn services(&self) -> Vec<GattService> {
        self.services.clone()
    }

    fn read<'a>(
        &'a self,
        characteristic: &'a GattCharacteristic,
    ) -> BoxFuture<'a, Result<Vec<u8>, DriverError>> {
        async move {
            let c = self.native(characteristic)?;
            self.peripheral.read(c).await.map_err(gatt_error)
        }
        .boxed()
    }

    fn write<'a>(
        &'a self,
        characteristic: &'a GattCharacteristic,
        data: &'a [u8],
        with_response: bool,
    ) -> BoxFuture<'a, Result<(), DriverError>> {
        async move {
            let c = self.native(characteristic)?;
            let kind = if with_response {
                WriteType::WithResponse
            } else {
                WriteType::WithoutResponse
            };
            self.peripheral
                .write(c, data, kind)
                .await
                .map_err(gatt_error)
        }
        .boxed()
    }

    fn subscribe<'a>(
        &'a self,
        characteristic: &'a GattCharacteristic,
    ) -> BoxFuture<'a, Result<(), DriverError>> {
        async move {
            let c = self.native(characteristic)?;
            self.peripheral.subscribe(c).await.map_err(gatt_error)
        }
        .boxed()
    }
}

impl BleLink for BtleplugLink {
    fn notifications(
        &self,
    ) -> BoxFuture<'_, Result<mpsc::UnboundedReceiver<Notification>, BleError>> {
        async move {
            let mut values = self.peripheral.notifications().await.map_err(ble_error)?;
            let mut events = self.adapter.events().await.map_err(ble_error)?;
            let id = self.peripheral.id();
            let (tx, rx) = mpsc::unbounded_channel();
            // jedno zadanie: powiadomienia do kanału, koniec przy rozłączeniu urządzenia
            tokio::spawn(async move {
                loop {
                    tokio::select! {
                        // odbiorca zamknięty (koniec połączenia) - zadanie nie czeka dalej
                        () = tx.closed() => break,
                        value = values.next() => match value {
                            Some(v) => {
                                let n = Notification {
                                    characteristic_uuid: v.uuid.to_string().to_lowercase(),
                                    data: v.value,
                                };
                                if tx.send(n).is_err() {
                                    break;
                                }
                            }
                            None => break,
                        },
                        event = events.next() => match event {
                            Some(CentralEvent::DeviceDisconnected(gone)) if gone == id => break,
                            Some(_) => {}
                            None => break,
                        },
                    }
                }
            });
            Ok(rx)
        }
        .boxed()
    }

    fn disconnect(&self) -> BoxFuture<'_, ()> {
        async move {
            if let Err(e) = self.peripheral.disconnect().await {
                log::debug!("Rozłączanie: {e}");
            }
        }
        .boxed()
    }
}

// --- raport diagnostyczny ------------------------------------------------------------------

/// Tyle sekund słuchamy powiadomień w raporcie.
pub const LISTEN_S: f64 = 12.0;

/// Raport diagnostyczny Bluetooth - jeden plik tekstowy do wysłania, gdy coś nie działa:
/// urządzenia w pobliżu, usługi i charakterystyki bieżni, odczyt charakterystyk (zakresy,
/// firmware), surowe powiadomienia i próba rozmowy sterownikiem. Bez danych osobowych poza
/// nazwami i adresami urządzeń Bluetooth w pobliżu.
pub async fn run_diagnostics(
    backend: &dyn BleBackend,
    registry: &ProfileRegistry,
    path: &Path,
    address: Option<String>,
    clock: Clock,
    listen_s: f64,
) -> std::io::Result<PathBuf> {
    let mut lines = Vec::new();
    let t0 = clock();
    lines.push(format!(
        "Raport WalkLevel {} - {}",
        crate::version(),
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
    ));
    lines.push(format!(
        "{} {} | rdzeń {}",
        std::env::consts::OS,
        std::env::consts::ARCH,
        crate::version()
    ));
    lines.push(String::new());
    if let Err(e) = diagnose(backend, registry, address, clock, t0, listen_s, &mut lines).await {
        lines.push(format!("BŁĄD: {e}"));
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, lines.join("\n") + "\n")?;
    Ok(path.to_path_buf())
}

async fn diagnose(
    backend: &dyn BleBackend,
    registry: &ProfileRegistry,
    address: Option<String>,
    clock: Clock,
    t0: f64,
    listen_s: f64,
    lines: &mut Vec<String>,
) -> Result<(), BleError> {
    lines.push("== Skan (8 s) ==".into());
    let mut found = backend.scan(Duration::from_secs(8)).await?;
    found.sort_by_key(|a| std::cmp::Reverse(a.rssi.unwrap_or(i16::MIN)));
    for a in &found {
        let name = a.name.clone().unwrap_or_else(|| "(bez nazwy)".into());
        let treadmill = super::looks_like_treadmill(registry, a.name.as_deref(), &a.services);
        let rssi = a.rssi.map_or("?".into(), |r| r.to_string());
        lines.push(format!(
            "{}  {rssi:>4} dBm  {name}{}",
            a.address,
            if treadmill { "  <-- bieżnia?" } else { "" }
        ));
        if !a.services.is_empty() {
            lines.push(format!("    usługi: {}", a.services.join(", ")));
        }
        for (company, data) in &a.manufacturer {
            lines.push(format!(
                "    producent 0x{company:04x}: {}",
                hex::encode(data)
            ));
        }
    }
    let address = address.or_else(|| {
        found
            .iter()
            .find(|a| super::looks_like_treadmill(registry, a.name.as_deref(), &a.services))
            .map(|a| a.address.clone())
    });
    lines.push(String::new());
    let Some(address) = address else {
        lines.push(t("dziennik.raport_brak_biezni", &[]));
        return Ok(());
    };
    lines.push(format!("== Połączenie z {address} =="));
    let name = found
        .iter()
        .find(|a| a.address.eq_ignore_ascii_case(&address))
        .and_then(|a| a.name.clone())
        .unwrap_or_default();
    if backend
        .find(address.clone(), String::new(), Duration::from_secs(10))
        .await?
        .is_none()
    {
        lines.push("Urządzenie zniknęło przed połączeniem.".into());
        return Ok(());
    }
    let link = backend.connect(address, Duration::from_secs(20)).await?;
    let result = listen(&link, registry, &name, clock, t0, listen_s, lines).await;
    link.disconnect().await;
    result
}

async fn listen(
    link: &Arc<dyn BleLink>,
    registry: &ProfileRegistry,
    name: &str,
    clock: Clock,
    t0: f64,
    listen_s: f64,
    lines: &mut Vec<String>,
) -> Result<(), BleError> {
    let services = link.services();
    for s in &services {
        lines.push(format!("usługa {}", s.uuid));
        for c in &s.characteristics {
            lines.push(format!(
                "    char {}  [{}]",
                c.uuid,
                c.properties.join(", ")
            ));
        }
    }
    lines.push(String::new());
    lines.push("== Odczyt charakterystyk (zakresy, firmware - do profilu modelu) ==".into());
    let all: Vec<&GattCharacteristic> = services
        .iter()
        .flat_map(|s| s.characteristics.iter())
        .collect();
    for c in all
        .iter()
        .filter(|c| c.properties.iter().any(|p| p == "read"))
    {
        let short = c.uuid.get(..8).unwrap_or(&c.uuid);
        match link.read(c).await {
            Ok(value) => {
                let text = String::from_utf8(value.clone())
                    .ok()
                    .map(|t| {
                        t.trim_matches(|c: char| c == '\0' || c.is_whitespace())
                            .to_string()
                    })
                    .filter(|t| !t.is_empty() && t.chars().all(|ch| !ch.is_control()));
                let shown = text.map_or(String::new(), |t| format!("  '{t}'"));
                lines.push(format!("    {short}  {}{shown}", hex::encode(&value)));
            }
            Err(e) => lines.push(format!("    {short}  nie da się odczytać: {e}")),
        }
    }
    lines.push(String::new());
    lines.push(format!("== Powiadomienia przez {listen_s:.0} s =="));
    let channels = super::kingsmith_proto::find_channels(&services);
    let channel_notify: Vec<&str> = channels.iter().map(|c| c.notify.uuid.as_str()).collect();
    for c in all.iter().filter(|c| {
        c.properties
            .iter()
            .any(|p| p == "notify" || p == "indicate")
            && !channel_notify
                .iter()
                .any(|u| u.eq_ignore_ascii_case(&c.uuid))
    }) {
        if let Err(e) = link.subscribe(c).await {
            lines.push(format!("    nie da się subskrybować {}: {e}", c.uuid));
        }
    }
    let mut notes = link.notifications().await?;
    let readings = Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
    let stamp = {
        let clock = clock.clone();
        move || format!("{:7.2}s", clock() - t0)
    };
    let driver = if channels.is_empty() {
        None
    } else {
        lines.push(format!(
            "Protokół KingSmith: kanały {}",
            channels
                .iter()
                .map(|c| c.label().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        ));
        let sink = readings.clone();
        let stamp_frames = stamp.clone();
        let on_frame: FrameCallback = Arc::new(move |f| {
            let line = format!(
                "{} ODCZYT prędkość={:?} km/h kroki={:?} dystans={:?} czas={:?}",
                stamp_frames(),
                f.speed_kmh,
                f.steps,
                f.distance_m,
                f.elapsed_s
            );
            sink.lock().unwrap_or_else(|p| p.into_inner()).push(line);
        });
        let gatt: Arc<dyn GattClient> = link.clone();
        super::make_driver(gatt, name, registry, on_frame, clock.clone()).ok()
    };
    let deadline = tokio::time::Instant::now()
        + Duration::try_from_secs_f64(listen_s.max(0.0)).unwrap_or(Duration::ZERO);
    let pump = async {
        while let Some(n) = notes.recv().await {
            let line = format!(
                "{} << {}  {}",
                stamp(),
                n.characteristic_uuid
                    .get(..8)
                    .unwrap_or(&n.characteristic_uuid),
                hex::encode(&n.data)
            );
            readings
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .push(line);
            if let Some(d) = &driver {
                d.on_notification(&n.characteristic_uuid, &n.data);
            }
        }
    };
    let talk = async {
        if let Some(d) = &driver {
            if let Err(e) = d.start().await {
                readings
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .push(format!("Start sterownika: {e}"));
            }
            while tokio::time::Instant::now() < deadline {
                let _ = d.poll().await; // błędy widać w powiadomieniach i w logu
                tokio::time::sleep(Duration::from_millis(750)).await;
            }
        }
        tokio::time::sleep_until(deadline).await;
    };
    tokio::select! {
        () = pump => {}
        () = talk => {}
    }
    lines.extend(readings.lock().unwrap_or_else(|p| p.into_inner()).drain(..));
    if let Some(d) = &driver {
        lines.push(format!("Wynik: {}", d.describe()));
    }
    Ok(())
}
