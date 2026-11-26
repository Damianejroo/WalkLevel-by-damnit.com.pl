//! Standard Bluetooth FTMS (Fitness Machine Service): Urevo SpaceWalk 5L i inne bieżnie FTMS
//! (port bieznia/sterowniki/ftms/protokol.py i części odbiorczej sterownika).
//!
//! Dane (Treadmill Data 0x2ACD, co ok. 1 s), stan maszyny (0x2ADA) i polecenia przez
//! Control Point (0x2AD9) - każde polecenie dostaje odpowiedź (indykację) z wynikiem.
//!
//! Urevo SpaceWalk 5L (URTM054, firmware V90.08.15, sprawdzone na żywym urządzeniu):
//! prędkość 1,00-6,00 km/h co 0,10, nachylenie 0-9 % co 1 %, flagi danych 0x259C.
//! Bit 13 flag (spoza FTMS 1.0) niesie 2 bajty - u Urevo najpewniej liczba kroków.

use serde::{Deserialize, Serialize};

use super::kontrakt::{Capabilities, Counts, GattCharacteristic, GattService};
use super::telemetry::TelemetryFrame;

pub const FTMS_SERVICE: &str = "00001826-0000-1000-8000-00805f9b34fb";
pub const FEATURE: &str = "00002acc-0000-1000-8000-00805f9b34fb";
pub const TREADMILL_DATA: &str = "00002acd-0000-1000-8000-00805f9b34fb";
pub const TRAINING_STATUS: &str = "00002ad3-0000-1000-8000-00805f9b34fb";
pub const SPEED_RANGE: &str = "00002ad4-0000-1000-8000-00805f9b34fb";
pub const INCLINE_RANGE: &str = "00002ad5-0000-1000-8000-00805f9b34fb";
pub const CONTROL_POINT: &str = "00002ad9-0000-1000-8000-00805f9b34fb";
pub const MACHINE_STATUS: &str = "00002ada-0000-1000-8000-00805f9b34fb";
pub const DEVICE_INFO: &str = "0000180a-0000-1000-8000-00805f9b34fb";
pub const FIRMWARE_REVISION: &str = "00002a26-0000-1000-8000-00805f9b34fb";

pub const OP_REQUEST_CONTROL: u8 = 0x00;
pub const OP_SET_SPEED: u8 = 0x02;
pub const OP_SET_INCLINE: u8 = 0x03;
pub const OP_START: u8 = 0x07;
pub const OP_STOP_PAUSE: u8 = 0x08;
pub const STOP: u8 = 0x01;
pub const PAUSE: u8 = 0x02;
pub const RESPONSE: u8 = 0x80;

pub const RESULT_OK: u8 = 1;
pub const RESULT_NOT_PERMITTED: u8 = 5;

/// Opis wyniku polecenia Control Point.
pub fn result_name(code: u8) -> Option<&'static str> {
    match code {
        1 => Some("przyjęte"),
        2 => Some("nieobsługiwane"),
        3 => Some("zła wartość"),
        4 => Some("nie powiodło się"),
        5 => Some("brak kontroli"),
        _ => None,
    }
}

// Fitness Machine Status (0x2ADA)
pub const STATUS_RESET: u8 = 0x01;
/// zatrzymana albo pauza (przez użytkownika albo polecenie)
pub const STATUS_STOPPED: u8 = 0x02;
pub const STATUS_SAFETY_KEY: u8 = 0x03;
pub const STATUS_STARTED: u8 = 0x04;
pub const STATUS_SPEED: u8 = 0x05;
pub const STATUS_INCLINE: u8 = 0x06;

/// Opis stanu maszyny do logu.
pub fn status_name(code: u8) -> String {
    match code {
        STATUS_RESET => "reset".into(),
        STATUS_STOPPED => "zatrzymana".into(),
        STATUS_SAFETY_KEY => "zatrzymana kluczem bezpieczeństwa".into(),
        STATUS_STARTED => "uruchomiona".into(),
        STATUS_SPEED => "zmiana prędkości".into(),
        STATUS_INCLINE => "zmiana nachylenia".into(),
        _ => format!("kod {code}"),
    }
}

/// Spoza FTMS 1.0 - Urevo: liczba kroków (uint16).
pub const STEPS_FLAG: u16 = 0x2000;

/// Domyślne możliwości modułu - profil modelu je uzupełnia.
pub const DEFAULT_CAPS: Capabilities = Capabilities {
    speed_min: 1.0,
    speed_max: 6.0,
    speed_step: 0.1,
    start_speed: 1.0,
    start_delay_s: 0.0,
    acks_commands: true,
    pause: true,
    steps: false, // standard nie ma kroków - Urevo podaje je w polu spoza FTMS 1.0
    incline: None,
    telemetry_s: 1.0,
    min_command_gap_s: 0.8,
    control: true,
};

/// Pola obecne w jednej ramce Treadmill Data.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TreadmillData {
    pub speed_kmh: Option<f64>,
    pub distance_m: Option<u32>,
    pub incline_pct: Option<f64>,
    pub energy_kcal: Option<u16>,
    pub heart_rate: Option<u8>,
    pub elapsed_s: Option<u16>,
    pub steps: Option<u16>,
    /// bajty na końcu ramki, których nie opisuje żadna flaga (Urevo: 1)
    pub trailing: usize,
}

impl TreadmillData {
    /// Pola z `self`, a brakujące z wcześniejszej ramki „More Data”.
    fn or(self, older: TreadmillData) -> TreadmillData {
        TreadmillData {
            speed_kmh: self.speed_kmh.or(older.speed_kmh),
            distance_m: self.distance_m.or(older.distance_m),
            incline_pct: self.incline_pct.or(older.incline_pct),
            energy_kcal: self.energy_kcal.or(older.energy_kcal),
            heart_rate: self.heart_rate.or(older.heart_rate),
            elapsed_s: self.elapsed_s.or(older.elapsed_s),
            steps: self.steps.or(older.steps),
            trailing: self.trailing,
        }
    }
}

struct Cursor<'a> {
    data: &'a [u8],
    pos: usize,
}

impl Cursor<'_> {
    fn take<const N: usize>(&mut self) -> Option<[u8; N]> {
        let bytes = self.data.get(self.pos..self.pos + N)?;
        self.pos += N;
        bytes.try_into().ok()
    }

    fn u8(&mut self) -> Option<u8> {
        self.take::<1>().map(|b| b[0])
    }

    fn u16(&mut self) -> Option<u16> {
        self.take().map(u16::from_le_bytes)
    }

    fn i16(&mut self) -> Option<i16> {
        self.take().map(i16::from_le_bytes)
    }
}

/// Pola obecne w jednej ramce (wg flag). None, gdy ramka jest za krótka albo ucięta.
pub fn decode_treadmill_data(data: &[u8]) -> Option<TreadmillData> {
    if data.len() < 2 {
        return None;
    }
    let flags = u16::from_le_bytes([data[0], data[1]]);
    let mut c = Cursor { data, pos: 2 };
    let mut out = TreadmillData::default();
    if flags & 0x0001 == 0 {
        // bit 0 odwrotny: „More Data” = 0 -> jest prędkość
        out.speed_kmh = Some(f64::from(c.u16()?) / 100.0);
    }
    if flags & 0x0002 != 0 {
        c.u16()?; // średnia prędkość
    }
    if flags & 0x0004 != 0 {
        let lo = c.u16()?;
        let hi = c.u8()?;
        out.distance_m = Some(u32::from(lo) | (u32::from(hi) << 16));
    }
    if flags & 0x0008 != 0 {
        let incline = c.i16()?;
        c.i16()?; // kąt rampy
        out.incline_pct = Some(f64::from(incline) / 10.0);
    }
    if flags & 0x0010 != 0 {
        c.take::<4>()?; // przewyższenie
    }
    if flags & 0x0020 != 0 {
        c.u8()?; // tempo
    }
    if flags & 0x0040 != 0 {
        c.u8()?; // średnie tempo
    }
    if flags & 0x0080 != 0 {
        let total = c.u16()?;
        c.take::<3>()?; // na godzinę i na minutę
        if total != 0xFFFF {
            out.energy_kcal = Some(total);
        }
    }
    if flags & 0x0100 != 0 {
        out.heart_rate = Some(c.u8()?);
    }
    if flags & 0x0200 != 0 {
        c.u8()?; // MET
    }
    if flags & 0x0400 != 0 {
        out.elapsed_s = Some(c.u16()?);
    }
    if flags & 0x0800 != 0 {
        c.u16()?; // pozostały czas
    }
    if flags & 0x1000 != 0 {
        c.take::<4>()?; // siła i moc
    }
    if flags & STEPS_FLAG != 0 {
        out.steps = Some(c.u16()?);
    }
    out.trailing = data.len().saturating_sub(c.pos);
    Some(out)
}

/// Sama prędkość w km/h albo None.
pub fn parse_treadmill_data(data: &[u8]) -> Option<f64> {
    decode_treadmill_data(data)?.speed_kmh
}

/// Polecenie spoza zakresu pola (np. ujemna prędkość).
#[derive(Debug, Clone, Copy, PartialEq, thiserror::Error)]
#[error("Wartość {0} poza zakresem polecenia FTMS")]
pub struct OutOfRange(pub f64);

pub fn encode_request_control() -> Vec<u8> {
    vec![OP_REQUEST_CONTROL]
}

pub fn encode_start() -> Vec<u8> {
    vec![OP_START]
}

pub fn encode_stop() -> Vec<u8> {
    vec![OP_STOP_PAUSE, STOP]
}

pub fn encode_pause() -> Vec<u8> {
    vec![OP_STOP_PAUSE, PAUSE]
}

/// Prędkość w setnych km/h (uint16, zaokrąglenie jak `round()` w Pythonie).
pub fn encode_set_speed(kmh: f64) -> Result<Vec<u8>, OutOfRange> {
    let value = (kmh * 100.0).round_ties_even();
    if !(0.0..=f64::from(u16::MAX)).contains(&value) {
        return Err(OutOfRange(kmh));
    }
    let [lo, hi] = (value as u16).to_le_bytes();
    Ok(vec![OP_SET_SPEED, lo, hi])
}

/// Nachylenie w dziesiątych % (int16).
pub fn encode_set_incline(pct: f64) -> Result<Vec<u8>, OutOfRange> {
    let value = (pct * 10.0).round_ties_even();
    if !(f64::from(i16::MIN)..=f64::from(i16::MAX)).contains(&value) {
        return Err(OutOfRange(pct));
    }
    let [lo, hi] = (value as i16).to_le_bytes();
    Ok(vec![OP_SET_INCLINE, lo, hi])
}

/// Odpowiedź Control Point: (kod polecenia, wynik) albo None, gdy to nie odpowiedź.
pub fn parse_response(data: &[u8]) -> Option<(u8, u8)> {
    if data.len() < 3 || data[0] != RESPONSE {
        return None;
    }
    Some((data[1], data[2]))
}

/// Zakres prędkości (od, do, co) w km/h.
pub fn decode_speed_range(data: &[u8]) -> Option<(f64, f64, f64)> {
    if data.len() < 6 {
        return None;
    }
    let v = |i: usize| f64::from(u16::from_le_bytes([data[i], data[i + 1]])) / 100.0;
    Some((v(0), v(2), v(4)))
}

/// Zakres nachylenia (od, do, co) w %.
pub fn decode_incline_range(data: &[u8]) -> Option<(f64, f64, f64)> {
    if data.len() < 6 {
        return None;
    }
    let lo = f64::from(i16::from_le_bytes([data[0], data[1]])) / 10.0;
    let hi = f64::from(i16::from_le_bytes([data[2], data[3]])) / 10.0;
    let step = f64::from(u16::from_le_bytes([data[4], data[5]])) / 10.0;
    Some((lo, hi, step))
}

/// Charakterystyka o danym UUID (wielkość liter bez znaczenia) w dowolnej usłudze.
pub fn find_char<'a>(services: &'a [GattService], uuid: &str) -> Option<&'a GattCharacteristic> {
    services
        .iter()
        .flat_map(|s| s.characteristics.iter())
        .find(|c| c.uuid.to_lowercase() == uuid)
}

/// Czy urządzenie ma dane bieżni FTMS.
pub fn has_ftms(services: &[GattService]) -> bool {
    find_char(services, TREADMILL_DATA).is_some()
}

/// Firmware z charakterystyki 0x2A26 (tekst ASCII, bez zer i spacji na końcach).
pub fn decode_firmware(raw: &[u8]) -> String {
    let text: String = raw
        .iter()
        .map(|b| {
            if b.is_ascii() {
                char::from(*b)
            } else {
                '\u{fffd}'
            }
        })
        .collect();
    text.trim_matches(['\0', ' '])
        .trim_matches(crate::py::is_space)
        .to_string()
}

// --- odbiór danych --------------------------------------------------------------------------

/// Co zmienił stan maszyny (0x2ADA).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusEvent {
    Started,
    /// zatrzymana, klucz bezpieczeństwa albo reset (`reset` - utrata kontroli)
    Stopped {
        reset: bool,
    },
    Other,
}

/// Część odbiorcza sterownika FTMS: ramki danych i stanu -> odczyty i stan taśmy.
#[derive(Debug, Clone)]
pub struct Telemetry {
    pub capabilities: Capabilities,
    /// prędkość z ostatniej ramki i jej czas
    pub last_speed: Option<(f64, f64)>,
    pub running: Option<bool>,
    pub has_control: bool,
    pub counts: Counts,
    /// pola z ramki „More Data” (bez prędkości) - czekają na następną ramkę z prędkością
    pending: Option<TreadmillData>,
    trailing_logged: bool,
}

impl Telemetry {
    pub fn new(capabilities: Capabilities) -> Self {
        Self {
            capabilities,
            last_speed: None,
            running: None,
            has_control: false,
            counts: Counts::default(),
            pending: None,
            trailing_logged: false,
        }
    }

    /// Ramka Treadmill Data. None, gdy ramki nie da się odczytać albo gdy to pierwsza część
    /// danych („More Data”, bez prędkości) - jej pola trafią do następnej ramki z prędkością.
    pub fn on_data(&mut self, raw: &[u8], now: f64) -> Option<TelemetryFrame> {
        let Some(part) = decode_treadmill_data(raw) else {
            self.counts.undecoded += 1;
            log::debug!("<< FTMS (nie do odczytania) {}", hex::encode(raw));
            return None;
        };
        self.counts.frames += 1;
        if part.trailing > 0 && !self.trailing_logged {
            self.trailing_logged = true;
            log::info!(
                "FTMS: {} nieopisane bajty na końcu ramki danych ({})",
                part.trailing,
                hex::encode(raw)
            );
        }
        let Some(speed) = part.speed_kmh else {
            let older = self.pending.take().unwrap_or_default();
            self.pending = Some(part.or(older));
            return None;
        };
        let decoded = match self.pending.take() {
            Some(older) => part.or(older),
            None => part,
        };
        if decoded.steps.is_some() && !self.capabilities.steps {
            self.capabilities.steps = true;
        }
        self.last_speed = Some((speed, now));
        if speed > 0.0 {
            self.running = Some(true);
        } else if self.running.is_none() {
            self.running = Some(false);
        }
        Some(TelemetryFrame {
            distance_m: decoded.distance_m.map(u64::from),
            steps: decoded.steps.map(u64::from),
            elapsed_s: decoded.elapsed_s.map(u64::from),
            ..TelemetryFrame::data(now, Some(speed))
        })
    }

    /// Ramka Machine Status. None dla pustej ramki.
    pub fn on_status(&mut self, raw: &[u8]) -> Option<StatusEvent> {
        let code = *raw.first()?;
        log::info!("Stan bieżni: {} ({})", status_name(code), hex::encode(raw));
        match code {
            STATUS_STARTED => {
                self.running = Some(true);
                Some(StatusEvent::Started)
            }
            STATUS_STOPPED | STATUS_SAFETY_KEY | STATUS_RESET => {
                self.running = Some(false);
                let reset = code == STATUS_RESET;
                if reset {
                    self.has_control = false;
                }
                Some(StatusEvent::Stopped { reset })
            }
            _ => Some(StatusEvent::Other),
        }
    }

    /// Prędkość, którą bieżnia naprawdę pojedzie (zakres zgłoszony przez bieżnię).
    pub fn reachable_speed(&self, kmh: f64) -> f64 {
        let caps = &self.capabilities;
        crate::py::round_tenth(caps.speed_max.min(caps.speed_min.max(kmh)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(text: &str) -> Vec<u8> {
        hex::decode(text.replace(' ', "")).expect("hex")
    }

    const REAL_IDLE: &str =
        "9c 25 00 00 5a 00 00 5a 00 00 00 00 00 00 00 0c 00 00 00 00 00 e0 00 23 01 00";
    const REAL_WALK: &str =
        "9c 25 64 00 14 00 00 00 00 00 00 00 00 00 00 00 02 00 00 00 00 00 4a 00 00 00 00";

    #[test]
    fn decodes_real_urevo_frames() {
        let idle = decode_treadmill_data(&hex(REAL_IDLE)).expect("ramka");
        assert_eq!(idle.speed_kmh, Some(0.0));
        assert_eq!(idle.distance_m, Some(90));
        assert_eq!(idle.incline_pct, Some(9.0));
        assert_eq!(idle.energy_kcal, Some(12));
        assert_eq!(idle.elapsed_s, Some(224));
        assert_eq!(idle.steps, Some(291));
        let walk = decode_treadmill_data(&hex(REAL_WALK)).expect("ramka");
        assert_eq!((walk.speed_kmh, walk.distance_m), (Some(1.0), Some(20)));
        assert_eq!(decode_treadmill_data(&hex(REAL_IDLE)[..10]), None);
        assert_eq!(decode_treadmill_data(b"\x01"), None);
        assert_eq!(parse_treadmill_data(&hex(REAL_WALK)), Some(1.0));
        assert_eq!(parse_treadmill_data(&[0x00, 0x00, 0x5E, 0x01]), Some(3.5));
        assert_eq!(parse_treadmill_data(&[0x01, 0x00, 0x5E, 0x01]), None);
        assert_eq!(parse_treadmill_data(b"\x00"), None);
    }

    const D1: &str = "9c25fa008601002800000000000000250000000000fb02280300";

    /// Przypadki D1-D6 z docs/analiza-dystans.md.
    #[test]
    fn decodes_full_frames_like_analysis_d1_to_d6() {
        let d1 = decode_treadmill_data(&hex(D1)).expect("D1");
        assert_eq!(d1.speed_kmh, Some(2.5));
        assert_eq!(d1.distance_m, Some(390));
        assert_eq!(d1.incline_pct, Some(4.0));
        assert_eq!(d1.energy_kcal, Some(37));
        assert_eq!(
            (d1.elapsed_s, d1.steps, d1.trailing),
            (Some(763), Some(808), 1)
        );
        let d2 =
            decode_treadmill_data(&hex("9c25640000000000000000000000000000000000000000000000"))
                .expect("D2");
        assert_eq!(
            (d2.speed_kmh, d2.distance_m, d2.elapsed_s, d2.steps),
            (Some(1.0), Some(0), Some(0), Some(0))
        );
        let d3 =
            decode_treadmill_data(&hex("9c255a000a000000000000000000000000000000000700000000"))
                .expect("D3");
        assert_eq!(
            (d3.speed_kmh, d3.distance_m, d3.elapsed_s, d3.steps),
            (Some(0.9), Some(10), Some(7), Some(0))
        );
        // D4: ucięta ramka (bez kroków) - nic nie trafia dalej
        assert_eq!(decode_treadmill_data(&hex(D1)[..24]), None);
        let mut t = Telemetry::new(DEFAULT_CAPS);
        assert_eq!(t.on_data(&hex(D1)[..24], 0.0), None);
        // D5: „More Data” bez prędkości - pola scalone z następną ramką z prędkością
        let full = hex(D1);
        let mut more = vec![0x9d, 0x25];
        more.extend_from_slice(&full[4..]);
        assert_eq!(t.on_data(&more, 1.0), None);
        let speed_only = [0x00, 0x00, 0xfa, 0x00];
        let merged = t.on_data(&speed_only, 1.1).expect("scalona ramka");
        assert_eq!(merged.speed_kmh, Some(2.5));
        assert_eq!(
            (merged.distance_m, merged.elapsed_s, merged.steps),
            (Some(390), Some(763), Some(808))
        );
        let next = t.on_data(&speed_only, 2.1).expect("ramka");
        assert_eq!(next.steps, None); // scalone pola tylko raz
        // D6: energia 0xFFFF = brak
        let d6 = decode_treadmill_data(&hex("8000fa00ffff000000")).expect("D6");
        assert_eq!((d6.speed_kmh, d6.energy_kcal), (Some(2.5), None));
    }

    #[test]
    fn encodes_control_point_commands() {
        assert_eq!(encode_request_control(), b"\x00");
        assert_eq!(encode_start(), b"\x07");
        assert_eq!(encode_pause(), b"\x08\x02");
        assert_eq!(encode_stop(), b"\x08\x01");
        assert_eq!(encode_set_speed(2.5), Ok(hex("02 fa 00")));
        assert!(encode_set_speed(-1.0).is_err());
        assert_eq!(parse_response(&hex("80 07 01")), Some((0x07, 1)));
        assert_eq!(parse_response(&hex("01 02")), None);
        assert_eq!(
            decode_speed_range(&hex("64 00 58 02 0a 00")),
            Some((1.0, 6.0, 0.1))
        );
    }

    #[test]
    fn reachable_speed_respects_treadmill_range() {
        let t = Telemetry::new(DEFAULT_CAPS);
        assert_eq!(t.reachable_speed(0.0), 1.0);
        assert_eq!(t.reachable_speed(9.0), 6.0);
    }

    #[test]
    fn firmware_text_is_trimmed() {
        assert_eq!(decode_firmware(b"V90.08.15\x00\x00 "), "V90.08.15");
    }
}
