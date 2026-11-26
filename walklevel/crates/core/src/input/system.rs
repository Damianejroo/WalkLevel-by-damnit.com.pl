//! Cienka warstwa na Windows API: stan klawiszy, SendInput, okno na pierwszym planie - port
//! części bieznia/winapi.py. Poza Windows funkcje nic nie robią (stan: nic nie wciśnięte).

use std::sync::OnceLock;
use std::time::Instant;

use super::InputError;
use super::send::KeyEvent;

/// Sekundy od pierwszego wywołania (zegar monotoniczny, jak `time.monotonic` w Pythonie).
pub fn monotonic_s() -> f64 {
    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_secs_f64()
}

/// Czy klawisz (kod VK) jest teraz wciśnięty - działa też, gdy okno nie ma fokusu.
pub fn is_key_down(vk: u32) -> bool {
    #[cfg(windows)]
    {
        let Ok(vk) = i32::try_from(vk) else {
            return false;
        };
        // SAFETY: GetAsyncKeyState przyjmuje dowolny kod i tylko czyta stan klawiatury.
        let state = unsafe { windows::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState(vk) };
        state as u16 & 0x8000 != 0
    }
    #[cfg(not(windows))]
    {
        let _ = vk;
        false
    }
}

/// Kod wirtualny dla kodu skanowania (lewy/prawy Shift, Ctrl i Alt rozróżnione). Poza
/// Windows - 0.
pub fn vk_from_scan(scan: u16, extended: bool) -> u32 {
    #[cfg(windows)]
    {
        use windows::Win32::UI::Input::KeyboardAndMouse::{MAPVK_VSC_TO_VK_EX, MapVirtualKeyW};
        let code = if extended {
            0xE000 | u32::from(scan)
        } else {
            u32::from(scan)
        };
        // SAFETY: MapVirtualKeyW tylko tłumaczy kody, nie dotyka pamięci programu.
        unsafe { MapVirtualKeyW(code, MAPVK_VSC_TO_VK_EX) }
    }
    #[cfg(not(windows))]
    {
        let _ = (scan, extended);
        0
    }
}

/// Wysyła zdarzenie klawiatury przez SendInput (gry z DirectInput je widzą). Poza Windows nic.
pub fn send_event(event: KeyEvent) -> Result<(), InputError> {
    #[cfg(windows)]
    {
        use windows::Win32::UI::Input::KeyboardAndMouse::{
            INPUT, INPUT_0, INPUT_KEYBOARD, KEYBD_EVENT_FLAGS, KEYBDINPUT, SendInput, VIRTUAL_KEY,
        };
        // Dane klawisza MUSZĄ trafić do unii (Anonymous.ki) - pusty INPUT Windows przyjmuje
        // bez błędu i nic się nie dzieje (błąd z wersji 0.2.2 w Pythonie).
        let input = INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VIRTUAL_KEY(0),
                    wScan: event.scan,
                    dwFlags: KEYBD_EVENT_FLAGS(event.flags),
                    time: 0,
                    dwExtraInfo: event.extra,
                },
            },
        };
        let size = std::mem::size_of::<INPUT>() as i32;
        // SAFETY: jedna poprawnie wypełniona struktura INPUT i jej prawdziwy rozmiar.
        let sent = unsafe { SendInput(&[input], size) };
        if sent != 1 {
            return Err(InputError::SendRejected(
                std::io::Error::last_os_error().to_string(),
            ));
        }
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let _ = event;
        Ok(())
    }
}

/// Naciska albo puszcza klawisz po kodzie skanowania.
pub fn send_scancode(scan: u16, down: bool, extended: bool) -> Result<(), InputError> {
    send_event(KeyEvent::new(scan, down, extended))
}

/// Środek okna na pierwszym planie (zwykle gra) w pikselach ekranu - żeby popup pokazał się
/// na tym samym monitorze. None poza Windows albo gdy nie ma takiego okna.
pub fn foreground_center() -> Option<(i32, i32)> {
    #[cfg(windows)]
    {
        use windows::Win32::Foundation::RECT;
        use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowRect};
        // SAFETY: GetForegroundWindow nie przyjmuje argumentów, zwraca uchwyt albo NULL.
        let hwnd = unsafe { GetForegroundWindow() };
        if hwnd.0.is_null() {
            return None;
        }
        let mut rect = RECT::default();
        // SAFETY: uchwyt z GetForegroundWindow, rect to poprawny wskaźnik na lokalną strukturę.
        unsafe { GetWindowRect(hwnd, &mut rect) }.ok()?;
        Some(((rect.left + rect.right) / 2, (rect.top + rect.bottom) / 2))
    }
    #[cfg(not(windows))]
    {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(windows)]
    #[test]
    fn input_struct_matches_windows_abi() {
        // SendInput odrzuca wywołanie, jeśli rozmiar struktury się nie zgadza.
        let size = std::mem::size_of::<windows::Win32::UI::Input::KeyboardAndMouse::INPUT>();
        assert_eq!(
            size,
            if cfg!(target_pointer_width = "64") {
                40
            } else {
                28
            }
        );
    }

    #[test]
    fn system_calls_do_not_crash() {
        let _ = is_key_down(0x77);
        let _ = is_key_down(u32::MAX);
        let _ = foreground_center();
        #[cfg(windows)]
        assert_ne!(vk_from_scan(0x11, false), 0); // W (albo Z w układzie AZERTY)
        assert!(monotonic_s() >= 0.0);
    }
}
