//! Bieżnia: kontrakt, KingSmith, FTMS, symulator, profile modeli, pilnowanie (port
//! bieznia/sterowniki).
//!
//! Rdzeń aplikacji używa tego modułu i kontraktu ([`kontrakt`]). Moduły protokołów:
//! [`kingsmith_proto`] (WalkingPad), [`ftms_proto`] (standard Bluetooth, np. Urevo) i
//! [`simulator`]. Model rozpoznajemy po usługach GATT (który moduł) i nazwie Bluetooth
//! (który profil - dane w [`profiles`]). Sterowniki połączenia: [`kingsmith`] i [`ftms`]
//! (na [`GattClient`]), warstwa Bluetooth [`ble`] (btleplug, raport diagnostyczny) i
//! [`worker`] (skanowanie, łączenie z ponawianiem, polecenia taśmy, zdarzenia przez kanał).
//! Każdy sterownik oddaje pełne ramki [`telemetry::TelemetryFrame`]; dystans, czas ruchu,
//! kroki i kadencję liczy z nich [`telemetry::Meter`] - niezależnie od mapowania gry.

pub mod ble;
pub mod contract;
pub mod ftms;
pub mod ftms_proto;
pub mod kingsmith;
pub mod kingsmith_proto;
pub mod profiles;
pub mod simulator;
pub mod telemetry;
pub mod watchdog;
pub mod worker;

pub use ble::{Advertisement, BleBackend, BleError, BleLink, BtleplugBackend, Notification};
pub use contract::{
    Capabilities, Clock, Counts, DeviceInfo, Driver, DriverError, DriverKind, FrameCallback,
    GattCharacteristic, GattClient, GattService, StartResult, monotonic_clock,
};
pub use ftms::FtmsDriver;
pub use kingsmith::KingSmithDriver;
pub use profiles::{DownloadError, DownloadResult, ModelProfile, ProfileError, ProfileRegistry};
pub use simulator::SimulatorDriver;
pub use telemetry::{CounterSpec, FrameKind, Meter, MeterTotals, TelemetryFrame, TelemetrySpec};
pub use watchdog::{BrakeWatch, Braking, ResumeSpeed, SpeedGuard, SpeedStats};
pub use worker::{BeltCommand, BeltOutcome, BleWorker, FoundDevice, Link, WorkerEvent};

/// Sterownik dla połączonej bieżni: moduł po usługach GATT, profil po nazwie (jak
/// `make_driver` w Pythonie). Urządzenie bez usług bieżni: [`DriverError::UnsupportedDevice`].
pub fn make_driver(
    client: std::sync::Arc<dyn GattClient>,
    device_name: &str,
    registry: &ProfileRegistry,
    on_frame: FrameCallback,
    clock: Clock,
) -> Result<std::sync::Arc<dyn Driver>, DriverError> {
    let services = client.services();
    let found = detect(&services, device_name, registry)?;
    let caps = found.capabilities(&services);
    let info = found.device_info(device_name);
    Ok(match found {
        Detected::KingSmith { channels, profile } => {
            let table = profile.as_ref().and_then(|p| p.option_str("table"));
            let order = kingsmith_proto::Telemetry::order_for(device_name, table);
            std::sync::Arc::new(KingSmithDriver::new(
                client, channels, &order, caps, info, on_frame, clock,
            ))
        }
        Detected::Ftms { .. } => std::sync::Arc::new(FtmsDriver::new(
            client, &services, caps, info, on_frame, clock,
        )),
    })
}

/// Możliwości modułu bez profilu.
pub fn default_capabilities(kind: DriverKind) -> Capabilities {
    match kind {
        DriverKind::KingSmith => kingsmith_proto::DEFAULT_CAPS,
        DriverKind::Ftms => ftms_proto::DEFAULT_CAPS,
        DriverKind::Simulator => simulator::CAPS,
    }
}

/// Rozpoznana bieżnia po połączeniu: moduł, kanały i profil modelu.
#[derive(Debug, Clone, PartialEq)]
pub enum Detected {
    KingSmith {
        channels: Vec<kingsmith_proto::Channel>,
        profile: Option<ModelProfile>,
    },
    Ftms {
        profile: Option<ModelProfile>,
    },
}

impl Detected {
    pub fn kind(&self) -> DriverKind {
        match self {
            Detected::KingSmith { .. } => DriverKind::KingSmith,
            Detected::Ftms { .. } => DriverKind::Ftms,
        }
    }

    pub fn profile(&self) -> Option<&ModelProfile> {
        match self {
            Detected::KingSmith { profile, .. } | Detected::Ftms { profile } => profile.as_ref(),
        }
    }

    /// Możliwości modułu poprawione profilem (FTMS bez Control Point: bez sterowania).
    pub fn capabilities(&self, services: &[GattService]) -> Capabilities {
        let base = default_capabilities(self.kind());
        let mut caps = self.profile().map_or(base, |p| p.apply(&base));
        if self.kind() == DriverKind::Ftms
            && ftms_proto::find_char(services, ftms_proto::CONTROL_POINT).is_none()
        {
            caps.control = false;
        }
        caps
    }

    /// Opis bieżni do historii (firmware uzupełnia sterownik po połączeniu).
    pub fn device_info(&self, device_name: &str) -> DeviceInfo {
        let fallback = match self.kind() {
            DriverKind::Ftms => "Bieżnia FTMS",
            _ => "KingSmith",
        };
        let profile = self.profile();
        DeviceInfo {
            model: profile.map_or_else(
                || {
                    if device_name.is_empty() {
                        fallback.to_string()
                    } else {
                        device_name.to_string()
                    }
                },
                |p| p.name.clone(),
            ),
            driver: self.kind(),
            profile: profile.map(|p| p.id.clone()).unwrap_or_default(),
            name: device_name.to_string(),
            firmware: String::new(),
        }
    }
}

/// Moduł dla połączonej bieżni: po usługach GATT, profil po nazwie (jak make_driver).
pub fn detect(
    services: &[GattService],
    device_name: &str,
    registry: &ProfileRegistry,
) -> Result<Detected, DriverError> {
    let channels = kingsmith_proto::find_channels(services);
    if !channels.is_empty() {
        let profile = registry
            .profile_for(device_name, Some(DriverKind::KingSmith))
            .cloned();
        return Ok(Detected::KingSmith { channels, profile });
    }
    if ftms_proto::has_ftms(services) {
        let profile = registry
            .profile_for(device_name, Some(DriverKind::Ftms))
            .cloned();
        return Ok(Detected::Ftms { profile });
    }
    Err(DriverError::UnsupportedDevice)
}

/// Możliwości przed połączeniem (z profilu po zapamiętanej nazwie) - okno nie czeka na
/// bieżnię, żeby pokazać właściwy zakres prędkości.
pub fn known_capabilities(
    registry: &ProfileRegistry,
    device_name: &str,
    simulated: bool,
) -> Capabilities {
    if simulated {
        return simulator::CAPS;
    }
    match registry.profile_for(device_name, None) {
        Some(profile) => profile.apply(&default_capabilities(profile.driver)),
        None if device_name.to_uppercase().starts_with("KS-") => kingsmith_proto::DEFAULT_CAPS,
        None => Capabilities::DEFAULT,
    }
}

/// Czytelna nazwa modelu z profilu (URTM054 -> Urevo SpaceWalk 5L).
pub fn model_name(registry: &ProfileRegistry, device_name: &str) -> String {
    match registry.profile_for(device_name, None) {
        None => device_name.to_string(),
        // ogólny profil marki: model zostaje z nazwy Bluetooth
        Some(profile) if profile.id == "urevo" => format!("Urevo {device_name}"),
        Some(profile) => profile.name.clone(),
    }
}

/// Do sortowania wyników skanowania: znany model (profil) albo usługa bieżni.
pub fn looks_like_treadmill<S: AsRef<str>>(
    registry: &ProfileRegistry,
    name: Option<&str>,
    service_uuids: &[S],
) -> bool {
    if let Some(name) = name.filter(|n| !n.is_empty())
        && registry.profile_for(name, None).is_some()
    {
        return true;
    }
    let upper = name.unwrap_or("").to_uppercase();
    if upper.starts_with("KS-") || upper.starts_with("WALKINGPAD") {
        return true;
    }
    service_uuids.iter().any(|u| {
        let u = u.as_ref();
        u.to_lowercase() == ftms_proto::FTMS_SERVICE || kingsmith_proto::is_kingsmith_service(u)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn char_(service: &str, uuid: &str) -> GattCharacteristic {
        GattCharacteristic {
            service_uuid: service.into(),
            uuid: uuid.into(),
            properties: vec!["notify".into()],
        }
    }

    #[test]
    fn treadmill_names() {
        let registry = ProfileRegistry::builtin();
        let none: [&str; 0] = [];
        assert!(looks_like_treadmill(
            &registry,
            Some("KS-HDSY-X21C-ABCD"),
            &none
        ));
        assert!(looks_like_treadmill(&registry, Some("URTM054"), &none));
        assert!(looks_like_treadmill(
            &registry,
            None,
            &[ftms_proto::FTMS_SERVICE]
        ));
        assert!(!looks_like_treadmill(&registry, Some("JBL Flip 5"), &none));
    }

    #[test]
    fn urevo_is_recognized_and_named() {
        let registry = ProfileRegistry::builtin();
        let service = ftms_proto::FTMS_SERVICE;
        let services = vec![GattService {
            uuid: service.into(),
            characteristics: vec![
                char_(service, ftms_proto::TREADMILL_DATA),
                char_(service, ftms_proto::CONTROL_POINT),
            ],
        }];
        let found = detect(&services, "URTM054", &registry).expect("bieżnia");
        assert_eq!(found.kind(), DriverKind::Ftms);
        assert_eq!(found.device_info("URTM054").model, "Urevo SpaceWalk 5L");
        let caps = found.capabilities(&services);
        assert!(caps.control && caps.pause);
        assert_eq!(model_name(&registry, "URTM061"), "Urevo URTM061");
        assert!(matches!(
            detect(&[], "URTM054", &registry),
            Err(DriverError::UnsupportedDevice)
        ));
    }
}
