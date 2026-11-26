//! Token konta w Menedżerze poświadczeń Windows (chroniony przez DPAPI dla tego konta
//! Windows) - port bieznia/konto/sejf.py.
//!
//! Nigdy w plikach z danymi, w kopii danych ani w logach. Hasła nie zapisujemy wcale - tylko
//! token, który serwer może unieważnić (zmiana hasła = wylogowanie wszędzie). Poza Windows
//! (i w testach) token żyje tylko w pamięci ([`MemoryStore`]).
//!
//! Cel dla osoby: `Bieznia/konto/<id osoby>`; wersja z jednym kontem używała samego
//! `Bieznia/konto` - token stamtąd przenosi [`super::Accounts`] przy wczytaniu (kontrakt 8.4).

use std::io;
use std::sync::{Arc, Mutex, PoisonError};

/// Cel wersji z jednym kontem (i przedrostek celów osób).
pub const TARGET: &str = "Bieznia/konto";
/// Najdłuższy token mieszczący się w poświadczeniu (CRED_MAX_CREDENTIAL_BLOB_SIZE / 2).
pub const MAX_BLOB: usize = 2560;
/// Opis i nazwa użytkownika poświadczenia (jak w 0.5.x).
pub const COMMENT: &str = "WalkLevel - logowanie do konta online";
pub const USER_NAME: &str = "bieznia";

/// Miejsce na token jednego konta.
pub trait TokenStore: Send + Sync {
    /// Zapisany token ("" - brak).
    fn get(&self) -> String;
    fn set(&self, token: &str) -> io::Result<()>;
    fn clear(&self);
}

/// Token tylko w pamięci (do zamknięcia aplikacji).
#[derive(Debug, Default)]
pub struct MemoryStore {
    token: Mutex<String>,
}

impl MemoryStore {
    pub fn new() -> Self {
        Self::default()
    }
}

impl TokenStore for MemoryStore {
    fn get(&self) -> String {
        self.token
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    fn set(&self, token: &str) -> io::Result<()> {
        *self.token.lock().unwrap_or_else(PoisonError::into_inner) = token.to_string();
        Ok(())
    }

    fn clear(&self) {
        self.token
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clear();
    }
}

/// Cel w Menedżerze poświadczeń dla osoby (pusta osoba - cel wersji z jednym kontem).
pub fn vault_target(person: &str) -> String {
    if person.is_empty() {
        TARGET.to_string()
    } else {
        format!("{TARGET}/{person}")
    }
}

/// Sejf dla celu: Menedżer poświadczeń na Windows, inaczej pamięć.
pub fn default_store(target: &str) -> Arc<dyn TokenStore> {
    #[cfg(windows)]
    {
        Arc::new(WindowsStore::new(target))
    }
    #[cfg(not(windows))]
    {
        log::warn!("Menedżer poświadczeń niedostępny ({target}) - logowanie tylko do zamknięcia");
        Arc::new(MemoryStore::new())
    }
}

/// Menedżer poświadczeń Windows (CredWriteW / CredReadW / CredDeleteW), typ GENERIC,
/// trwałość LOCAL_MACHINE, token jako UTF-8.
#[cfg(windows)]
#[derive(Debug, Clone)]
pub struct WindowsStore {
    target: String,
}

#[cfg(windows)]
impl WindowsStore {
    pub fn new(target: &str) -> Self {
        Self {
            target: target.to_string(),
        }
    }

    pub fn target(&self) -> &str {
        &self.target
    }
}

/// Napis UTF-16 zakończony zerem.
#[cfg(windows)]
fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(windows)]
impl TokenStore for WindowsStore {
    fn get(&self) -> String {
        use windows::Win32::Security::Credentials::{
            CRED_TYPE_GENERIC, CREDENTIALW, CredFree, CredReadW,
        };
        use windows::core::PCWSTR;

        let target = wide(&self.target);
        let mut found: *mut CREDENTIALW = std::ptr::null_mut();
        // SAFETY: `target` to napis UTF-16 zakończony zerem żyjący do końca wywołania;
        // `found` to poprawny wskaźnik na miejsce na wynik.
        let read =
            unsafe { CredReadW(PCWSTR(target.as_ptr()), CRED_TYPE_GENERIC, None, &mut found) };
        if read.is_err() || found.is_null() {
            return String::new();
        }
        // SAFETY: CredReadW się powiodło, więc `found` wskazuje na strukturę przydzieloną przez
        // system; blob ma `CredentialBlobSize` bajtów (albo jest pusty). Kopiujemy bajty przed
        // zwolnieniem pamięci przez CredFree, potem już jej nie dotykamy.
        unsafe {
            let cred = &*found;
            let token = if cred.CredentialBlob.is_null() || cred.CredentialBlobSize == 0 {
                String::new()
            } else {
                let bytes = std::slice::from_raw_parts(
                    cred.CredentialBlob,
                    cred.CredentialBlobSize as usize,
                );
                String::from_utf8_lossy(bytes).into_owned()
            };
            CredFree(found as *const std::ffi::c_void);
            token
        }
    }

    fn set(&self, token: &str) -> io::Result<()> {
        use windows::Win32::Security::Credentials::{
            CRED_PERSIST_LOCAL_MACHINE, CRED_TYPE_GENERIC, CREDENTIALW, CredWriteW,
        };
        use windows::core::PWSTR;

        let mut blob = token.as_bytes().to_vec();
        if blob.len() > MAX_BLOB {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "token za długi dla Menedżera poświadczeń",
            ));
        }
        let mut target = wide(&self.target);
        let mut comment = wide(COMMENT);
        let mut user = wide(USER_NAME);
        let cred = CREDENTIALW {
            Type: CRED_TYPE_GENERIC,
            TargetName: PWSTR(target.as_mut_ptr()),
            Comment: PWSTR(comment.as_mut_ptr()),
            CredentialBlobSize: blob.len() as u32,
            CredentialBlob: blob.as_mut_ptr(),
            Persist: CRED_PERSIST_LOCAL_MACHINE,
            UserName: PWSTR(user.as_mut_ptr()),
            ..Default::default()
        };
        // SAFETY: wszystkie wskaźniki w `cred` wskazują na bufory (`target`, `comment`, `user`,
        // `blob`) żyjące do końca tej funkcji; system tylko je czyta i kopiuje.
        unsafe { CredWriteW(&cred, 0) }.map_err(|e| io::Error::other(format!("CredWriteW: {e}")))
    }

    fn clear(&self) {
        use windows::Win32::Security::Credentials::{CRED_TYPE_GENERIC, CredDeleteW};
        use windows::core::PCWSTR;

        let target = wide(&self.target);
        // SAFETY: `target` to napis UTF-16 zakończony zerem żyjący do końca wywołania. Brak
        // poświadczenia (ERROR_NOT_FOUND) to też dobry wynik - pomijamy błąd.
        let _ = unsafe { CredDeleteW(PCWSTR(target.as_ptr()), CRED_TYPE_GENERIC, None) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn targets_per_person_and_legacy() {
        assert_eq!(vault_target(""), "Bieznia/konto");
        assert_eq!(vault_target("a1b2c3d4e5"), "Bieznia/konto/a1b2c3d4e5");
    }

    #[test]
    fn memory_store_keeps_token_until_cleared() {
        let store = MemoryStore::new();
        assert_eq!(store.get(), "");
        store.set("tok-1").ok();
        assert_eq!(store.get(), "tok-1");
        store.clear();
        assert_eq!(store.get(), "");
    }

    /// Prawdziwy Menedżer poświadczeń: zapis, odczyt, usunięcie (cel testowy z losową
    /// nazwą, sprzątany na końcu). Ręcznie: `cargo test -p walklevel-core -- --ignored vault`.
    #[cfg(windows)]
    #[test]
    #[ignore = "dotyka Menedżera poświadczeń bieżącego użytkownika Windows"]
    fn windows_store_round_trip() {
        let target = format!("WalkLevel-test/{}", uuid::Uuid::new_v4().simple());
        let store = WindowsStore::new(&target);
        assert_eq!(store.get(), "");
        assert!(store.set("tok-żółw-123").is_ok());
        assert_eq!(store.get(), "tok-żółw-123");
        assert!(store.set(&"x".repeat(MAX_BLOB + 1)).is_err());
        store.clear();
        assert_eq!(store.get(), "");
        store.clear(); // drugie usunięcie nie szkodzi
    }
}
