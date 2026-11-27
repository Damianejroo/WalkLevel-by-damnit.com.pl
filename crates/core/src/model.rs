//! Wspólne typy domeny używane przez wiele modułów (historia, poziomy, konto, kopia).
//! Pola i ich znaczenie 1:1 z bieznia/historia.py (Session) - baza historia.db się nie zmienia.

use serde::{Deserialize, Serialize};

/// Sesja chodzenia - jeden wiersz tabeli `sesje` (schemat 5, kolejność kolumn jak w Pythonie).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    /// 32 znaki hex (UUID v7 bez kresek); na serwer idzie z kreskami (konto::canonical_uuid)
    pub id: String,
    /// id osoby (10 znaków hex) i jej nazwa w chwili sesji
    pub person: String,
    pub person_name: String,
    /// id profilu gry i jego nazwa w chwili sesji
    pub profile: String,
    pub profile_name: String,
    /// czasy uniksowe UTC w sekundach
    pub start: f64,
    pub end: f64,
    pub steps: i64,
    pub distance_km: f64,
    /// czas ruchu taśmy w sekundach
    pub active_s: f64,
    pub device_model: String,
    /// "kingsmith", "ftms", "" (stare sesje bez zapisanego sterownika)
    pub driver: String,
    pub device_profile: String,
    pub firmware: String,
    pub app_version: String,
    /// "bieznia" albo "symulator"
    pub source: String,
    /// "prywatna" | "znajomi" | "publiczna"
    pub visibility: String,
    /// strefa czasowa w chwili startu (minuty od UTC)
    pub tz_offset_min: i64,
    /// kiedy ostatnio zmieniona (do wysyłki zmienionych sesji)
    pub updated: f64,
    /// "gra" albo "trening"
    pub mode: String,
}

impl Session {
    pub fn avg_kmh(&self) -> f64 {
        if self.active_s > 0.0 { self.distance_km / (self.active_s / 3600.0) } else { 0.0 }
    }
}

/// Suma kroków, dystansu i czasu ruchu.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Totals {
    pub steps: i64,
    pub distance_km: f64,
    pub active_s: f64,
}

impl Totals {
    pub fn avg_kmh(&self) -> f64 {
        if self.active_s > 0.0 { self.distance_km / (self.active_s / 3600.0) } else { 0.0 }
    }
}
