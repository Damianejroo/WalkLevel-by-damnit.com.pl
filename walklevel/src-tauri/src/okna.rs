//! Okna i zdarzenia do interfejsu (docs/kontrakt.md rozdz. 3 i 7): okno główne, popup
//! osiągnięć nad grą (bez fokusu, klik przechodzi), pływający przycisk; emisja zdarzeń
//! `wl:*` przez [`UiSink`]; drobne wywołania Windows (styl okien, otwieranie folderów,
//! dźwięki).

use std::path::{Path, PathBuf};

use tauri::{
    AppHandle, Emitter, EventTarget, LogicalPosition, LogicalSize, Manager, PhysicalPosition,
    PhysicalSize, WebviewUrl, WebviewWindow, WebviewWindowBuilder,
};

use tauri::window::Color;

use crate::widoki::{NavRequest, PopupsView, SliceEvent, Telemetry, Toast};

pub const MAIN: &str = "main";
pub const POPUP: &str = "popup";
pub const BUBBLE: &str = "przycisk";

pub const DEFAULT_W: f64 = 740.0;
pub const DEFAULT_H: f64 = 890.0;
pub const MIN_W: f64 = 420.0;
/// Panel znajomych (0.6.2, docs/wyglad.md 8.16): pełny 288 px, zwinięty pasek 64 px.
pub const PANEL_FULL_W: f64 = 288.0;
pub const PANEL_BAR_W: f64 = 64.0;
/// Minimalna szerokość okna z pełnym panelem i z paskiem (bez panelu - [`MIN_W`]).
pub const MIN_W_PANEL_FULL: f64 = 720.0;
pub const MIN_W_PANEL_BAR: f64 = 484.0;

/// Tło motywu (`--bg` z src/styles/tokeny.css): ciemny #151514, jasny #fbfaf7.
pub fn theme_color(dark: bool) -> Color {
    if dark {
        Color(0x15, 0x15, 0x14, 0xff)
    } else {
        Color(0xfb, 0xfa, 0xf7, 0xff)
    }
}

/// Układ panelu znajomych (ustawienia `friends_panel*`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PanelLayout {
    pub on: bool,
    pub collapsed: bool,
    /// panel z lewej strony okna
    pub left: bool,
}

impl PanelLayout {
    /// Szerokość panelu w oknie (px logiczne).
    pub fn width(self) -> f64 {
        match (self.on, self.collapsed) {
            (false, _) => 0.0,
            (true, true) => PANEL_BAR_W,
            (true, false) => PANEL_FULL_W,
        }
    }

    /// Najmniejsza szerokość okna z tym układem.
    pub fn min_window(self) -> f64 {
        match (self.on, self.collapsed) {
            (false, _) => MIN_W,
            (true, true) => MIN_W_PANEL_BAR,
            (true, false) => MIN_W_PANEL_FULL,
        }
    }

    /// Panel z lewej zwinięty do paska: WebView jest o tyle szerszy od okna i wysunięty w lewo
    /// poza nie (ukryte miejsce po pełnym panelu, strona układa je jako puste). Dzięki temu przy
    /// zwijaniu i rozwijaniu z lewej WebView nie zmienia rozmiaru ani miejsca na ekranie - okno
    /// przesuwa tylko lewą krawędź. Wcześniej WebView2 przez klatkę pokazywał starą klatkę strony
    /// przyklejoną do nowej lewej krawędzi (główna kolumna przesunięta o 224 px i ucięta).
    pub fn hidden_left(self) -> f64 {
        if self.on && self.collapsed && self.left {
            PANEL_FULL_W - PANEL_BAR_W
        } else {
            0.0
        }
    }

    /// Zmiana szerokości okna i przesunięcie w poziomie przy przejściu `self` -> `after`, żeby
    /// główna kolumna stała w miejscu: zwinięcie zwęża okno o 224 px, rozwinięcie poszerza;
    /// panel z lewej - okno przesuwa się o tyle samo w drugą stronę. Zmiana strony panelu nie
    /// zmienia rozmiaru.
    pub fn resize_to(self, after: PanelLayout) -> (f64, f64) {
        if self.left != after.left {
            return (0.0, 0.0);
        }
        let delta = after.width() - self.width();
        (delta, if after.left { -delta } else { 0.0 })
    }
}
pub const MIN_H: f64 = 560.0;
pub const BUBBLE_SIZE: f64 = 56.0;
/// Tryb Trening: kółko + kolumna 28 px (+, poziom, −) obok i rząd Stop 24 px pod spodem
/// (odstępy 4 px; układ w src/okna/Przycisk.svelte).
pub const BUBBLE_TRAINING_W: f64 = BUBBLE_SIZE + 4.0 + 28.0;
pub const BUBBLE_TRAINING_H: f64 = BUBBLE_SIZE + 4.0 + 24.0;
/// Poza Treningiem, gdy taśma jedzie: kółko i pod nim Stop 24 px (56 × 84).
pub const BUBBLE_STOP_H: f64 = BUBBLE_TRAINING_H;

/// Układ okna przycisku: samo kółko, kółko ze Stopem, Trening (+, poziom, −, Stop).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BubbleLayout {
    Plain,
    Stop,
    Training,
}
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
    /// panel znajomych: układ przed i po (minimalna szerokość, rozmiar, przesunięcie okna)
    MainPanel(PanelLayout, PanelLayout),
    /// tło okna głównego i WebView2 w kolorze motywu (true - ciemny)
    MainBackground(bool),
    BubbleVisible(bool, Option<[f64; 2]>),
    /// rozmiar okna przycisku za układem
    BubbleSize(BubbleLayout),
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
        Okno::MainBackground(dark) => {
            if let Some(w) = app.get_webview_window(MAIN) {
                let _ = w.set_background_color(Some(theme_color(dark)));
            }
        }
        Okno::MainPanel(before, after) => {
            if let Some(w) = app.get_webview_window(MAIN) {
                resize_for_panel(&w, before, after);
            }
        }
        Okno::BubbleVisible(true, pos) => {
            if let Some(w) = window(app, BUBBLE, build_bubble) {
                place_bubble(app, &w, pos);
                show_no_activate(&w);
            }
        }
        // wyłączony przycisk nie trzyma osobnego procesu WebView2 (pozycja zapisana po
        // przeciągnięciu; ponowne włączenie tworzy okno od nowa)
        Okno::BubbleVisible(false, _) => {
            if let Some(w) = app.get_webview_window(BUBBLE) {
                let _ = w.hide();
                let _ = w.destroy();
            }
        }
        Okno::BubbleSize(layout) => {
            if let Some(w) = app.get_webview_window(BUBBLE) {
                resize_bubble(app, &w, layout);
            }
        }
        Okno::PopupShow => {
            if let Some(w) = window(app, POPUP, build_popup) {
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

/// Folder danych WebView2 wszystkich okien (`<folder danych>\webview` - nie miesza się
/// z `dane/` i nie trafia do kopii).
struct WebviewDir(PathBuf);

/// Okno główne (ukryte do `ui_ready`). Popup i przycisk powstają dopiero, gdy są potrzebne
/// (pierwsze powiadomienie, włączenie przycisku) - każde okno to osobny proces WebView2.
/// Strona nowego okna sama pobiera stan przy starcie (`popup_ready` / `app_snapshot`),
/// więc zdarzenia wysłane przed jej załadowaniem nie giną.
pub fn create_windows(app: &AppHandle, webview_dir: &Path) -> tauri::Result<()> {
    app.manage(WebviewDir(webview_dir.to_path_buf()));
    let title = format!("WalkLevel {} by damnit.com.pl", env!("CARGO_PKG_VERSION"));
    let main = WebviewWindowBuilder::new(app, MAIN, WebviewUrl::App("index.html".into()))
        .title(title)
        .inner_size(DEFAULT_W, DEFAULT_H)
        .min_inner_size(MIN_W, MIN_H)
        // ciemne tło od startu (okno i tak jest ukryte do `ui_ready`, potem tło idzie za motywem)
        .background_color(theme_color(true))
        .resizable(true)
        .visible(false)
        .data_directory(webview_dir.to_path_buf())
        .build()?;
    harden_webview(&main);
    remember_webview(&main);
    Ok(())
}

/// Istniejące okno `label` albo nowe z `build` (w wątku okien).
fn window(
    app: &AppHandle,
    label: &str,
    build: fn(&AppHandle, PathBuf) -> tauri::Result<WebviewWindow>,
) -> Option<WebviewWindow> {
    if let Some(w) = app.get_webview_window(label) {
        return Some(w);
    }
    let dir = app.state::<WebviewDir>().0.clone();
    build(app, dir)
        .map_err(|e| log::warn!("Okno {label}: {e}"))
        .ok()
}

/// Okno pomocnicze nad grą: przezroczyste, bez ramki, bez fokusu i paska zadań.
fn overlay<'a>(
    app: &'a AppHandle,
    label: &str,
    page: &str,
    title: String,
    dir: PathBuf,
) -> WebviewWindowBuilder<'a, tauri::Wry, AppHandle> {
    WebviewWindowBuilder::new(app, label, WebviewUrl::App(page.into()))
        .title(title)
        .transparent(true)
        .decorations(false)
        .shadow(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .focused(false)
        .focusable(false)
        .visible(false)
        .data_directory(dir)
}

fn build_popup(app: &AppHandle, dir: PathBuf) -> tauri::Result<WebviewWindow> {
    let title = crate::teksty::t(
        "powiadomienie.tytul_okna",
        &[("tytul", &crate::teksty::t("osiagniecia.tytul", &[]))],
    );
    let popup = overlay(app, POPUP, "popup.html", title, dir)
        .inner_size(POPUP_W, POPUP_H)
        .build()?;
    let _ = popup.set_ignore_cursor_events(true);
    overlay_style(&popup, true);
    harden_webview(&popup);
    Ok(popup)
}

fn build_bubble(app: &AppHandle, dir: PathBuf) -> tauri::Result<WebviewWindow> {
    let title = crate::teksty::t("dymek.tytul_okna", &[]);
    let bubble = overlay(app, BUBBLE, "przycisk.html", title, dir)
        .inner_size(BUBBLE_SIZE, BUBBLE_SIZE)
        .build()?;
    overlay_style(&bubble, false);
    harden_webview(&bubble);
    Ok(bubble)
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

/// Okno pod panel znajomych: minimalna szerokość z układu `after`; zmiana szerokości (i przy
/// panelu z lewej położenia) tylko dla zwykłego okna - zmaksymalizowane albo przyciągnięte do
/// krawędzi ekranu zostaje, jak jest (panel tylko się zwija). Zwężenie: najpierw minimum, potem
/// rozmiar; poszerzenie odwrotnie.
fn resize_for_panel(w: &WebviewWindow, before: PanelLayout, after: PanelLayout) {
    let min = LogicalSize::new(after.min_window(), MIN_H);
    let fixed = w.is_maximized().unwrap_or(false) || is_snapped(w);
    log::info!("Okno: panel {before:?} -> {after:?}, bez zmiany rozmiaru: {fixed}");
    let (Ok(k), Ok(pos), Ok(size)) = (w.scale_factor(), w.outer_position(), w.inner_size()) else {
        let _ = w.set_min_size(Some(min));
        return;
    };
    let width = size.width as f64 / k;
    let height = size.height as f64 / k;
    let (delta, shift) = if fixed {
        (0.0, 0.0)
    } else {
        before.resize_to(after)
    };
    let mut new_width = (width + delta).max(after.min_window());
    if fixed {
        new_width = width;
    }
    let shift = shift - (new_width - width - delta).max(0.0) * f64::from(u8::from(after.left));
    if new_width < width {
        let _ = w.set_min_size(Some(min));
    }
    let moves = (new_width - width).abs() >= 0.5 || shift.abs() >= 0.5;
    // wysunięcie WebView w lewo (panel z lewej zwinięty): przy zmianie okna nakłada je obsługa
    // WM_SIZE w tym samym SetWindowPos; bez zmiany okna - od razu
    set_hidden_left((after.hidden_left() * k).round() as i32, !moves);
    if moves {
        // rozmiar i położenie jednym SetWindowPos (panel z lewej: bez klatki z przesuniętą
        // główną kolumną między zmianą rozmiaru a przesunięciem)
        let dw = ((new_width - width) * k).round() as i32;
        let dx = (shift * k).round() as i32;
        if !move_and_resize(w, dx, dw) {
            let _ = w.set_size(LogicalSize::new(new_width, height));
            let x = pos.x as f64 / k + shift;
            let _ = w.set_position(LogicalPosition::new(x, pos.y as f64 / k));
        }
    }
    if new_width >= width {
        let _ = w.set_min_size(Some(min));
    }
}

// --- WebView wysunięty w lewo (panel z lewej zwinięty) ---------------------------------------

#[cfg(windows)]
thread_local! {
    /// Kontroler WebView2 okna głównego i wysunięcie w lewo (piksele fizyczne; wątek okien).
    static MAIN_WEBVIEW: std::cell::RefCell<
        Option<webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2Controller>,
    > = const { std::cell::RefCell::new(None) };
    static HIDDEN_LEFT: std::cell::Cell<i32> = const { std::cell::Cell::new(0) };
    /// Trwa nasz SetWindowPos przy panelu z lewej (zwijanie/rozwijanie).
    static LEFT_MOVE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    /// Uchwyt okna głównego (do nałożenia wysunięcia bez zmiany okna).
    static MAIN_HWND: std::cell::Cell<isize> = const { std::cell::Cell::new(0) };
}

/// Zapamiętuje kontroler WebView2 okna głównego i podpina obsługę WM_SIZE (po obsłudze wry,
/// która ustawia WebView na cały obszar klienta, nakłada wysunięcie w lewo).
fn remember_webview(w: &WebviewWindow) {
    #[cfg(windows)]
    {
        let Ok(hwnd) = w.hwnd() else { return };
        let hwnd_raw = hwnd.0 as isize;
        let _ = w.with_webview(move |wv| {
            use windows::Win32::Foundation::HWND;
            use windows::Win32::UI::Shell::SetWindowSubclass;
            let controller = wv.controller();
            MAIN_WEBVIEW.with(|c| *c.borrow_mut() = Some(controller));
            MAIN_HWND.set(hwnd_raw);
            // SAFETY: uchwyt okna głównego z Tauri; podklasa żyje tyle co okno (bez danych).
            unsafe {
                let _ = SetWindowSubclass(
                    HWND(hwnd_raw as *mut _),
                    Some(webview_subclass),
                    WEBVIEW_SUBCLASS_ID,
                    0,
                );
            }
        });
    }
    #[cfg(not(windows))]
    let _ = w;
}

#[cfg(windows)]
const WEBVIEW_SUBCLASS_ID: usize = 0x574c_0062;

#[cfg(windows)]
unsafe extern "system" fn webview_subclass(
    hwnd: windows::Win32::Foundation::HWND,
    msg: u32,
    wparam: windows::Win32::Foundation::WPARAM,
    lparam: windows::Win32::Foundation::LPARAM,
    _id: usize,
    _data: usize,
) -> windows::Win32::Foundation::LRESULT {
    use windows::Win32::Foundation::LRESULT;
    use windows::Win32::UI::Shell::DefSubclassProc;
    use windows::Win32::UI::WindowsAndMessaging::{SIZE_MINIMIZED, WM_SIZE, WM_WINDOWPOSCHANGED};
    // Nasze przesunięcie lewej krawędzi (panel z lewej): bez WM_SIZE dla wry - ten ustawiłby
    // WebView na nową szerokość i strona narysowałaby klatkę w złym miejscu. WebView zostaje
    // w tym samym miejscu ekranu i rozmiarze (granice z wysunięciem nakładamy sami, od razu).
    if msg == WM_WINDOWPOSCHANGED && LEFT_MOVE.get() {
        apply_hidden_left(hwnd);
        return LRESULT(0);
    }
    // SAFETY: wywołanie dalszej obsługi (wry, tao) z niezmienionymi argumentami.
    let result = unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) };
    if msg == WM_SIZE && wparam.0 != SIZE_MINIMIZED as usize && HIDDEN_LEFT.get() != 0 {
        apply_hidden_left(hwnd);
    }
    result
}

/// Granice WebView: obszar klienta, przy wysunięciu - od `-wysunięcie` (szerszy o tyle).
/// Pojemnik WebView (okno-rodzic kontrolera) - na cały obszar klienta.
#[cfg(windows)]
fn apply_hidden_left(hwnd: windows::Win32::Foundation::HWND) {
    use windows::Win32::Foundation::RECT;
    use windows::Win32::UI::WindowsAndMessaging::{
        GetClientRect, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOZORDER, SetWindowPos,
    };
    let mut client = RECT::default();
    // SAFETY: odczyt obszaru klienta okna głównego.
    if unsafe { GetClientRect(hwnd, &mut client) }.is_err() {
        return;
    }
    MAIN_WEBVIEW.with(|c| {
        if let Some(controller) = c.borrow().as_ref() {
            let mut container = windows::Win32::Foundation::HWND::default();
            // SAFETY: okno-rodzic kontrolera WebView2 (pojemnik wry) w wątku okien.
            if unsafe { controller.ParentWindow(&mut container) }.is_ok() && container != hwnd {
                // SAFETY: zmiana rozmiaru pojemnika WebView (okno potomne okna głównego).
                let _ = unsafe {
                    SetWindowPos(
                        container,
                        None,
                        0,
                        0,
                        client.right,
                        client.bottom,
                        SWP_NOZORDER | SWP_NOACTIVATE | SWP_NOMOVE,
                    )
                };
            }
        }
    });
    let bounds = RECT {
        left: -HIDDEN_LEFT.get(),
        top: 0,
        right: client.right,
        bottom: client.bottom,
    };
    MAIN_WEBVIEW.with(|c| {
        if let Some(controller) = c.borrow().as_ref() {
            // SAFETY: kontroler WebView2 okna głównego, wywołanie w wątku okien.
            let _ = unsafe { controller.SetBounds(bounds) };
        }
    });
}

/// Nowe wysunięcie WebView w lewo (piksele fizyczne). `now` - nałożyć od razu (okno się nie
/// zmienia); inaczej nałoży je obsługa WM_SIZE w trakcie najbliższego SetWindowPos.
fn set_hidden_left(px: i32, now: bool) {
    #[cfg(windows)]
    {
        let before = HIDDEN_LEFT.replace(px);
        let main = MAIN_HWND.get();
        if now && before != px && main != 0 {
            apply_hidden_left(windows::Win32::Foundation::HWND(main as *mut _));
        }
    }
    #[cfg(not(windows))]
    let _ = (px, now);
}

/// Przesunięcie o `dx` i zmiana szerokości o `dw` (piksele fizyczne) jednym `SetWindowPos`.
/// `false` - nie udało się (wtedy osobno rozmiar i położenie przez Tauri).
fn move_and_resize(w: &WebviewWindow, dx: i32, dw: i32) -> bool {
    #[cfg(windows)]
    {
        use windows::Win32::Foundation::RECT;
        use windows::Win32::UI::WindowsAndMessaging::{
            GetWindowRect, SWP_NOACTIVATE, SWP_NOZORDER, SetWindowPos,
        };
        let Ok(hwnd) = w.hwnd() else { return false };
        // SAFETY: uchwyt okna z Tauri; tylko odczyt prostokąta i zmiana położenia/rozmiaru.
        unsafe {
            let mut r = RECT::default();
            if GetWindowRect(hwnd, &mut r).is_err() {
                return false;
            }
            LEFT_MOVE.set(dx != 0);
            let ok = SetWindowPos(
                hwnd,
                None,
                r.left + dx,
                r.top,
                (r.right - r.left + dw).max(1),
                r.bottom - r.top,
                SWP_NOZORDER | SWP_NOACTIVATE,
            )
            .is_ok();
            LEFT_MOVE.set(false);
            ok
        }
    }
    #[cfg(not(windows))]
    {
        let _ = (w, dx, dw);
        false
    }
}

/// Okno przyciągnięte do krawędzi (Aero Snap): Windows trzyma wtedy w `rcNormalPosition`
/// prostokąt sprzed przyciągnięcia - inny ROZMIAR niż bieżący. Porównujemy tylko rozmiar
/// (położenie jest we współrzędnych obszaru roboczego, zależnych od paska zadań i monitorów).
fn is_snapped(w: &WebviewWindow) -> bool {
    #[cfg(windows)]
    {
        use windows::Win32::Foundation::RECT;
        use windows::Win32::UI::WindowsAndMessaging::{
            GetWindowPlacement, GetWindowRect, IsIconic, IsZoomed, WINDOWPLACEMENT,
        };
        let Ok(hwnd) = w.hwnd() else { return false };
        // SAFETY: tylko odczyt położenia okna z Tauri.
        unsafe {
            if IsZoomed(hwnd).as_bool() || IsIconic(hwnd).as_bool() {
                return false;
            }
            let mut placement = WINDOWPLACEMENT {
                length: std::mem::size_of::<WINDOWPLACEMENT>() as u32,
                ..Default::default()
            };
            let mut rect = RECT::default();
            if GetWindowPlacement(hwnd, &mut placement).is_err()
                || GetWindowRect(hwnd, &mut rect).is_err()
            {
                return false;
            }
            let n = placement.rcNormalPosition;
            let off = |a: i32, b: i32| (a - b).abs() > 2;
            off(rect.right - rect.left, n.right - n.left)
                || off(rect.bottom - rect.top, n.bottom - n.top)
        }
    }
    #[cfg(not(windows))]
    {
        let _ = w;
        false
    }
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

/// Rozmiar okna przycisku. Kółko zostaje w lewym górnym rogu okna (tam, gdzie było), większe
/// okno rośnie w prawo i w dół; gdy wyszłoby poza obszar roboczy monitora - przesuwa się do
/// środka (bez zapisu pozycji).
fn resize_bubble(app: &AppHandle, w: &WebviewWindow, layout: BubbleLayout) {
    let (width, height) = match layout {
        BubbleLayout::Training => (BUBBLE_TRAINING_W, BUBBLE_TRAINING_H),
        BubbleLayout::Stop => (BUBBLE_SIZE, BUBBLE_STOP_H),
        BubbleLayout::Plain => (BUBBLE_SIZE, BUBBLE_SIZE),
    };
    let _ = w.set_size(LogicalSize::new(width, height));
    let (Ok(k), Ok(p)) = (w.scale_factor(), w.outer_position()) else {
        return;
    };
    let (x, y) = (p.x as f64 / k, p.y as f64 / k);
    let Some(m) = monitor_at_logical(app, x + BUBBLE_SIZE / 2.0, y + BUBBLE_SIZE / 2.0) else {
        return;
    };
    let mk = m.scale_factor();
    let area = m.work_area();
    let (ax, ay) = (area.position.x as f64 / mk, area.position.y as f64 / mk);
    let (aw, ah) = (area.size.width as f64 / mk, area.size.height as f64 / mk);
    let nx = x.min(ax + aw - width).max(ax);
    let ny = y.min(ay + ah - height).max(ay);
    if (nx, ny) != (x, y) {
        let _ = w.set_position(LogicalPosition::new(nx, ny));
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

#[cfg(test)]
mod tests {
    use super::*;

    const FULL: PanelLayout = PanelLayout {
        on: true,
        collapsed: false,
        left: false,
    };

    #[test]
    fn panel_collapse_narrows_window_and_left_panel_moves_it() {
        let bar = PanelLayout {
            collapsed: true,
            ..FULL
        };
        let off = PanelLayout { on: false, ..FULL };
        assert_eq!(
            (FULL.min_window(), bar.min_window(), off.min_window()),
            (720.0, 484.0, 420.0)
        );
        assert_eq!(FULL.resize_to(bar), (-224.0, 0.0)); // z prawej: okno węższe, x bez zmian
        assert_eq!(bar.resize_to(FULL), (224.0, 0.0));
        let left = PanelLayout { left: true, ..FULL };
        let left_bar = PanelLayout { left: true, ..bar };
        assert_eq!(left.resize_to(left_bar), (-224.0, 224.0)); // główna kolumna stoi
        assert_eq!(left_bar.resize_to(left), (224.0, -224.0));
        assert_eq!(FULL.resize_to(off), (-288.0, 0.0));
        assert_eq!(FULL.resize_to(left), (0.0, 0.0)); // zmiana strony - bez zmiany rozmiaru
        assert_eq!(FULL.resize_to(FULL), (0.0, 0.0));
        // WebView wysunięty w lewo tylko przy zwiniętym panelu z lewej
        assert_eq!(left_bar.hidden_left(), 224.0);
        assert_eq!(
            (left.hidden_left(), bar.hidden_left(), off.hidden_left()),
            (0.0, 0.0, 0.0)
        );
        let left_off = PanelLayout {
            on: false,
            ..left_bar
        };
        assert_eq!(left_off.hidden_left(), 0.0);
    }
}
