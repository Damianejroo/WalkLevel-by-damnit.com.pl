//! Okna i zdarzenia do interfejsu (docs/kontrakt.md rozdz. 3 i 7): okno główne, popup
//! osiągnięć nad grą (bez fokusu, klik przechodzi), pływający przycisk; emisja zdarzeń
//! `wl:*` przez [`UiSink`]; drobne wywołania Windows (styl okien, otwieranie folderów,
//! dźwięki).

use std::path::{Path, PathBuf};

use tauri::{
    AppHandle, Emitter, EventTarget, LogicalPosition, LogicalSize, Manager, PhysicalPosition,
    PhysicalSize, WebviewUrl, WebviewWindow, WebviewWindowBuilder,
};

use crate::widoki::{NavRequest, PopupsView, SliceEvent, Telemetry, Toast};

pub const MAIN: &str = "main";
pub const POPUP: &str = "popup";
pub const BUBBLE: &str = "przycisk";

pub const DEFAULT_W: f64 = 740.0;
pub const DEFAULT_H: f64 = 890.0;
pub const MIN_W: f64 = 420.0;
pub const MIN_H: f64 = 560.0;
pub const BUBBLE_SIZE: f64 = 56.0;
/// 3 popupy po 96 px z odstępami 10 px
pub const POPUP_W: f64 = 380.0;
pub const POPUP_H: f64 = 3.0 * 96.0 + 2.0 * 10.0;
pub const POPUP_MARGIN: f64 = 16.0;

/// Prośby rdzenia o zmianę okien (kontrakt 5.1 `WindowRequest`).
#[derive(Debug, Clone)]
pub enum Okno {
    /// pokazanie okna głównego w zapamiętanym miejscu (`ui_ready`) - geometria logiczna
    ShowMainAt(Option<[i64; 4]>),
    ShowMain,
    MainAlwaysOnTop(bool),
    BubbleVisible(bool, Option<[f64; 2]>),
    PopupShow,
    PopupHide,
    Quit,
}

/// Granica rdzeń -> okna (kontrakt 5.1). Implementacja dla Tauri: [`TauriSink`].
pub trait UiSink: Send + Sync + 'static {
    fn slice(&self, ev: &SliceEvent);
    fn telemetry(&self, t: &Telemetry);
    fn toast(&self, t: &Toast);
    fn log_line(&self, line: &str);
    fn navigate(&self, n: &NavRequest);
    fn popups(&self, p: &PopupsView);
    fn window(&self, r: &Okno);
}

pub struct TauriSink(pub AppHandle);

impl UiSink for TauriSink {
    fn slice(&self, ev: &SliceEvent) {
        let _ = self.0.emit("wl:stan", ev);
    }
    fn telemetry(&self, t: &Telemetry) {
        // jedno zdarzenie dla obu okien (nasłuch „dowolny cel” dostałby dwa przy emit_to)
        let _ = self
            .0
            .emit_filter("wl:telemetria", t, |target| match target {
                EventTarget::WebviewWindow { label }
                | EventTarget::Webview { label }
                | EventTarget::Window { label } => label == MAIN || label == BUBBLE,
                _ => false,
            });
    }
    fn toast(&self, t: &Toast) {
        let _ = self.0.emit_to(MAIN, "wl:komunikat", t);
    }
    fn log_line(&self, line: &str) {
        let _ = self.0.emit_to(MAIN, "wl:log", line);
    }
    fn navigate(&self, n: &NavRequest) {
        let _ = self.0.emit_to(MAIN, "wl:nawigacja", n);
    }
    fn popups(&self, p: &PopupsView) {
        let _ = self.0.emit_to(POPUP, "wl:popupy", p);
    }
    fn window(&self, r: &Okno) {
        let app = self.0.clone();
        let r = r.clone();
        // operacje na oknach zawsze w wątku okien (bez czekania na niego z pętli)
        let _ = self.0.run_on_main_thread(move || apply(&app, r));
    }
}

fn apply(app: &AppHandle, r: Okno) {
    match r {
        Okno::ShowMainAt(geometry) => {
            if let Some(w) = app.get_webview_window(MAIN) {
                place_main(&w, geometry);
                let _ = w.show();
                let _ = w.set_focus();
            }
        }
        Okno::ShowMain => {
            if let Some(w) = app.get_webview_window(MAIN) {
                let _ = w.unminimize();
                let _ = w.show();
                let _ = w.set_focus();
            }
        }
        Okno::MainAlwaysOnTop(on) => {
            if let Some(w) = app.get_webview_window(MAIN) {
                let _ = w.set_always_on_top(on);
            }
        }
        Okno::BubbleVisible(on, pos) => {
            if let Some(w) = app.get_webview_window(BUBBLE) {
                if on {
                    place_bubble(app, &w, pos);
                    show_no_activate(&w);
                } else {
                    let _ = w.hide();
                }
            }
        }
        Okno::PopupShow => {
            if let Some(w) = app.get_webview_window(POPUP) {
                place_popup(app, &w);
                show_no_activate(&w);
            }
        }
        Okno::PopupHide => {
            if let Some(w) = app.get_webview_window(POPUP) {
                let _ = w.hide();
            }
        }
        Okno::Quit => app.exit(0),
    }
}

// --- tworzenie okien ------------------------------------------------------------------------

/// Okno główne (ukryte do `ui_ready`), popup i przycisk - wszystkie z danymi WebView2
/// w `<folder danych>\webview` (nie mieszają się z `dane/` i nie trafiają do kopii).
pub fn create_windows(app: &AppHandle, webview_dir: &Path) -> tauri::Result<()> {
    let title = format!("WalkLevel {} by damnit.com.pl", env!("CARGO_PKG_VERSION"));
    let main = WebviewWindowBuilder::new(app, MAIN, WebviewUrl::App("index.html".into()))
        .title(title)
        .inner_size(DEFAULT_W, DEFAULT_H)
        .min_inner_size(MIN_W, MIN_H)
        .resizable(true)
        .visible(false)
        .data_directory(webview_dir.to_path_buf())
        .build()?;
    harden_webview(&main);

    let popup = WebviewWindowBuilder::new(app, POPUP, WebviewUrl::App("popup.html".into()))
        .title("WalkLevel - osiągnięcia")
        .inner_size(POPUP_W, POPUP_H)
        .transparent(true)
        .decorations(false)
        .shadow(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .focused(false)
        .focusable(false)
        .visible(false)
        .data_directory(webview_dir.to_path_buf())
        .build()?;
    let _ = popup.set_ignore_cursor_events(true);
    overlay_style(&popup, true);
    harden_webview(&popup);

    let bubble = WebviewWindowBuilder::new(app, BUBBLE, WebviewUrl::App("przycisk.html".into()))
        .title("WalkLevel - przycisk")
        .inner_size(BUBBLE_SIZE, BUBBLE_SIZE)
        .transparent(true)
        .decorations(false)
        .shadow(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .focused(false)
        .focusable(false)
        .visible(false)
        .data_directory(webview_dir.to_path_buf())
        .build()?;
    overlay_style(&bubble, false);
    harden_webview(&bubble);
    Ok(())
}

/// Wersja wydana: bez klawiszy przeglądarki (F5, Ctrl+R, F12...) i bez menu kontekstowego.
fn harden_webview(w: &WebviewWindow) {
    if cfg!(debug_assertions) {
        return;
    }
    #[cfg(windows)]
    let _ = w.with_webview(|wv| {
        use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2Settings3;
        use windows::core::Interface;
        // SAFETY: wywołania COM na kontrolerze WebView2 w wątku okna (with_webview).
        unsafe {
            let Ok(core) = wv.controller().CoreWebView2() else {
                return;
            };
            let Ok(settings) = core.Settings() else {
                return;
            };
            let _ = settings.SetAreDefaultContextMenusEnabled(false);
            let _ = settings.SetAreDevToolsEnabled(false);
            if let Ok(s3) = settings.cast::<ICoreWebView2Settings3>() {
                let _ = s3.SetAreBrowserAcceleratorKeysEnabled(false);
            }
        }
    });
    #[cfg(not(windows))]
    let _ = w;
}

/// Okno nad grą: bez aktywacji i paska zadań; `click_through` - klik przechodzi dalej.
fn overlay_style(w: &WebviewWindow, click_through: bool) {
    #[cfg(windows)]
    {
        use windows::Win32::UI::WindowsAndMessaging::{
            GWL_EXSTYLE, GetWindowLongPtrW, HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
            SetWindowLongPtrW, SetWindowPos, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
            WS_EX_TRANSPARENT,
        };
        let Ok(hwnd) = w.hwnd() else { return };
        let mut extra = (WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW).0;
        if click_through {
            extra |= (WS_EX_TRANSPARENT | WS_EX_LAYERED).0;
        }
        // SAFETY: uchwyt okna z Tauri, zmiana stylu rozszerzonego i kolejności okien.
        unsafe {
            let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
            SetWindowLongPtrW(hwnd, GWL_EXSTYLE, style | extra as isize);
            let _ = SetWindowPos(
                hwnd,
                Some(HWND_TOPMOST),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            );
        }
    }
    #[cfg(not(windows))]
    let _ = (w, click_through);
}

/// Pokazanie bez zabierania fokusu grze (SW_SHOWNOACTIVATE zamiast `show()`).
fn show_no_activate(w: &WebviewWindow) {
    #[cfg(windows)]
    {
        use windows::Win32::UI::WindowsAndMessaging::{SW_SHOWNOACTIVATE, ShowWindow};
        if let Ok(hwnd) = w.hwnd() {
            // SAFETY: uchwyt okna z Tauri.
            unsafe {
                let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
            }
            return;
        }
    }
    let _ = w.show();
}

// --- miejsce okien ------------------------------------------------------------------------

/// Monitor (pozycja, rozmiar, skala) zawierający punkt w pikselach fizycznych.
fn monitor_at(app: &AppHandle, x: f64, y: f64) -> Option<tauri::Monitor> {
    app.available_monitors().ok()?.into_iter().find(|m| {
        let p = m.position();
        let s = m.size();
        x >= p.x as f64
            && y >= p.y as f64
            && x < (p.x + s.width as i32) as f64
            && y < (p.y + s.height as i32) as f64
    })
}

/// Monitor z punktem w pikselach logicznych (tak zapisywało Qt: współrzędne logiczne).
fn monitor_at_logical(app: &AppHandle, x: f64, y: f64) -> Option<tauri::Monitor> {
    app.available_monitors().ok()?.into_iter().find(|m| {
        let k = m.scale_factor();
        let p = m.position();
        let s = m.size();
        let (left, top) = (p.x as f64 / k, p.y as f64 / k);
        x >= left && y >= top && x < left + s.width as f64 / k && y < top + s.height as f64 / k
    })
}

fn place_main(w: &WebviewWindow, geometry: Option<[i64; 4]>) {
    let app = w.app_handle();
    if let Some([x, y, width, height]) = geometry {
        let (xf, yf) = (x as f64, y as f64);
        if let Some(m) = monitor_at_logical(app, xf + width as f64 / 2.0, yf + 20.0) {
            let k = m.scale_factor();
            let area = m.work_area();
            let (ax, ay) = (area.position.x as f64 / k, area.position.y as f64 / k);
            let (aw, ah) = (area.size.width as f64 / k, area.size.height as f64 / k);
            let width = (width as f64).min(aw).max(MIN_W);
            let height = (height as f64).min(ah).max(MIN_H);
            let x = xf.min(ax + aw - width).max(ax);
            let y = yf.min(ay + ah - height).max(ay);
            let _ = w.set_position(LogicalPosition::new(x, y));
            let _ = w.set_size(LogicalSize::new(width, height));
            return;
        }
    }
    let Some(m) = app.primary_monitor().ok().flatten() else {
        let _ = w.center();
        return;
    };
    let k = m.scale_factor();
    let area = m.work_area();
    let (aw, ah) = (area.size.width as f64 / k, area.size.height as f64 / k);
    let width = DEFAULT_W.min(aw - 40.0);
    let height = DEFAULT_H.min(ah - 60.0);
    let x = area.position.x as f64 / k + (aw - width) / 2.0;
    let y = area.position.y as f64 / k + (ah - height) / 2.0;
    let _ = w.set_position(LogicalPosition::new(x, y));
    let _ = w.set_size(LogicalSize::new(width, height));
}

/// Geometria okna głównego do zapamiętania: [x, y, szer., wys.] w pikselach logicznych.
/// `None`, gdy okno zminimalizowane, zmaksymalizowane albo poza ekranami.
pub fn main_geometry(app: &AppHandle) -> Option<[i64; 4]> {
    let w = app.get_webview_window(MAIN)?;
    if w.is_minimized().unwrap_or(true) || w.is_maximized().unwrap_or(true) {
        return None;
    }
    if !w.is_visible().unwrap_or(false) {
        return None;
    }
    let k = w.scale_factor().ok()?;
    let pos = w.outer_position().ok()?;
    let size = w.inner_size().ok()?;
    let center_x = pos.x as f64 + size.width as f64 / 2.0;
    let center_y = pos.y as f64 + size.height as f64 / 2.0;
    monitor_at(app, center_x, center_y)?;
    Some([
        (pos.x as f64 / k).round() as i64,
        (pos.y as f64 / k).round() as i64,
        (size.width as f64 / k).round() as i64,
        (size.height as f64 / k).round() as i64,
    ])
}

fn place_bubble(app: &AppHandle, w: &WebviewWindow, pos: Option<[f64; 2]>) {
    if let Some([x, y]) = pos
        && monitor_at_logical(app, x + 28.0, y + 28.0).is_some()
    {
        let _ = w.set_position(LogicalPosition::new(x, y));
        return;
    }
    if let Some(m) = app.primary_monitor().ok().flatten() {
        let k = m.scale_factor();
        let area = m.work_area();
        let right = (area.position.x as f64 + area.size.width as f64) / k;
        let middle = (area.position.y as f64 + area.size.height as f64 / 2.0) / k;
        let _ = w.set_position(LogicalPosition::new(right - BUBBLE_SIZE - 24.0, middle));
    }
}

/// Pozycja przycisku w pikselach logicznych (po przeciągnięciu).
pub fn bubble_position(app: &AppHandle) -> Option<[f64; 2]> {
    let w = app.get_webview_window(BUBBLE)?;
    let k = w.scale_factor().ok()?;
    let p = w.outer_position().ok()?;
    Some([(p.x as f64 / k).round(), (p.y as f64 / k).round()])
}

/// Prawy dolny róg obszaru roboczego monitora z oknem na pierwszym planie (gra), w pikselach
/// fizycznych; liczone przy każdym nowym popupie.
fn place_popup(app: &AppHandle, w: &WebviewWindow) {
    let area = foreground_work_area().or_else(|| {
        let m = app.primary_monitor().ok().flatten()?;
        let a = m.work_area();
        Some((
            a.position.x,
            a.position.y,
            a.position.x + a.size.width as i32,
            a.position.y + a.size.height as i32,
        ))
    });
    let Some((left, top, right, bottom)) = area else {
        return;
    };
    let k = monitor_at(
        app,
        (left + right) as f64 / 2.0,
        (top + bottom) as f64 / 2.0,
    )
    .map_or(1.0, |m| m.scale_factor());
    let (pw, ph) = ((POPUP_W * k).round(), (POPUP_H * k).round());
    let margin = (POPUP_MARGIN * k).round();
    let x = right as f64 - pw - margin;
    let y = bottom as f64 - ph - margin;
    let _ = w.set_size(PhysicalSize::new(pw as u32, ph as u32));
    let _ = w.set_position(PhysicalPosition::new(
        x.max(left as f64) as i32,
        y.max(top as f64) as i32,
    ));
}

fn foreground_work_area() -> Option<(i32, i32, i32, i32)> {
    #[cfg(windows)]
    {
        use windows::Win32::Graphics::Gdi::{
            GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow,
        };
        use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;
        // SAFETY: tylko odczyt okna na pierwszym planie i jego monitora.
        unsafe {
            let hwnd = GetForegroundWindow();
            if hwnd.is_invalid() {
                return None;
            }
            let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
            let mut info = MONITORINFO {
                cbSize: std::mem::size_of::<MONITORINFO>() as u32,
                ..Default::default()
            };
            if !GetMonitorInfoW(monitor, &mut info).as_bool() {
                return None;
            }
            let r = info.rcWork;
            Some((r.left, r.top, r.right, r.bottom))
        }
    }
    #[cfg(not(windows))]
    None
}

// --- system ------------------------------------------------------------------------------

/// Otwiera folder w Eksploratorze albo adres w przeglądarce (ShellExecuteW „open”).
pub fn shell_open(target: &str) {
    #[cfg(windows)]
    {
        use windows::Win32::UI::Shell::ShellExecuteW;
        use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
        use windows::core::{HSTRING, w};
        let target = HSTRING::from(target);
        // SAFETY: napisy żyją do końca wywołania.
        unsafe {
            ShellExecuteW(None, w!("open"), &target, None, None, SW_SHOWNORMAL);
        }
    }
    #[cfg(not(windows))]
    let _ = target;
}

pub fn open_folder(path: &Path) {
    let _ = std::fs::create_dir_all(path);
    shell_open(&path.to_string_lossy());
}

static ACHIEVEMENT_WAV: &[u8] = include_bytes!("../dzwieki/osiagniecie.wav");
static LEVEL_WAV: &[u8] = include_bytes!("../dzwieki/awans.wav");

/// Krótki dźwięk popupu (w tle, bez systemowego „ding”, gdy coś nie tak).
pub fn play_sound(level: bool) {
    #[cfg(windows)]
    {
        use windows::Win32::Media::Audio::{PlaySoundW, SND_ASYNC, SND_MEMORY, SND_NODEFAULT};
        use windows::core::PCWSTR;
        let data = if level { LEVEL_WAV } else { ACHIEVEMENT_WAV };
        // SAFETY: SND_MEMORY - wskaźnik na statyczne bajty pliku WAV (żyją do końca programu).
        unsafe {
            let _ = PlaySoundW(
                PCWSTR(data.as_ptr().cast()),
                None,
                SND_MEMORY | SND_ASYNC | SND_NODEFAULT,
            );
        }
    }
    #[cfg(not(windows))]
    let _ = (level, ACHIEVEMENT_WAV, LEVEL_WAV);
}

/// Wersja Windows do zgłoszeń (np. „Windows 10.0.26100”).
pub fn windows_version() -> String {
    #[cfg(windows)]
    {
        use windows::Win32::System::Registry::{HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ, RegGetValueW};
        use windows::core::w;
        let mut buf = [0u16; 128];
        let mut len = (buf.len() * 2) as u32;
        // SAFETY: odczyt napisu z rejestru do bufora o podanej długości.
        let ok = unsafe {
            RegGetValueW(
                HKEY_LOCAL_MACHINE,
                w!(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion"),
                w!("CurrentBuild"),
                RRF_RT_REG_SZ,
                None,
                Some(buf.as_mut_ptr().cast()),
                Some(&mut len),
            )
            .is_ok()
        };
        if ok {
            let n = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
            return format!("Windows 10.0.{}", String::from_utf16_lossy(&buf[..n]));
        }
        "Windows".into()
    }
    #[cfg(not(windows))]
    std::env::consts::OS.into()
}

/// Folder danych WebView2.
pub fn webview_dir(data: &Path) -> PathBuf {
    data.join("webview")
}
