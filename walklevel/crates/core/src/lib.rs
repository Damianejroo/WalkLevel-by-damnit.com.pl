//! Rdzeń WalkLevel - cała logika bez interfejsu. Aplikacja Tauri (src-tauri) tylko łączy
//! te moduły z oknem; każdy moduł ma testy i da się go sprawdzić bez okna i bez bieżni.
//!
//! Zgodność z wersją 0.5.x (Python): te same pliki danych w %LOCALAPPDATA%\Bieznia
//! (dane/osoby.json, dane/konto.json, dane/historia.db schemat 5), ten sam cel w Menedżerze
//! poświadczeń ("Bieznia/konto"), te same definicje poziomów (poziomy/definicje.json).

pub mod model;
pub(crate) mod py; // odpowiedniki funkcji Pythona (wyniki bit w bit jak w 0.5.x)

pub mod paths; // foldery danych i zasobów (exe / kod / testy)
pub mod config; // config.toml (adresy serwera, zgłoszeń)
pub mod logs; // logi w pliku (rotacja), poziomy szczegółowości

pub mod store; // osoby, profile gier, ustawienia (dane/osoby.json) - port bieznia/dane.py
pub mod icons; // ikony profili (port bieznia/ikony.py)
pub mod history; // historia.db, SessionTracker, zakresy, godziny - port bieznia/historia.py
pub mod levels; // poziomy, rangi, osiągnięcia, szlak - port bieznia/poziomy

pub mod mapping; // prędkość -> klawisze, progi, tryby - port bieznia/mapping.py + engine.py
pub mod input; // wysyłanie klawiszy (SendInput), odczyt pada (XInput) - klawisze.py, pad.py
pub mod treadmill; // bieżnia: kontrakt, KingSmith, FTMS, symulator, profile modeli, pilnowanie

pub mod account; // konto online: API, stan, wysyłka sesji, sejf (Menedżer poświadczeń)
pub mod reports; // zgłoszenia problemu: paczka logów, maskowanie, wysyłka
pub mod backup; // kopia danych .bieznia (eksport/import) - port bieznia/kopia.py
pub mod teksty; // teksty dla użytkownika ze słownika docs/teksty.md
pub mod updates; // sprawdzanie i pobieranie aktualizacji (GitHub Releases)

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
