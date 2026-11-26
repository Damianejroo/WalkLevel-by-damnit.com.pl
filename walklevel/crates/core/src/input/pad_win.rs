//! Odczyt padów w Windows: XInput, winmm (joyGetPosEx) i pady Sony wprost przez HID.
//! winmm, SetupAPI, hid.dll i CreateFileW/ReadFile/CancelIoEx są zadeklarowane tutaj
//! (raw-dylib), bo crate `windows` nie ma włączonych tych funkcji.

use std::collections::BTreeSet;
use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::UI::Input::XboxController::{XINPUT_STATE, XInputGetState};

use super::pad::{PadSource, PadState, axis, parse_sony, parse_sony_buttons, xinput_buttons};

const SONY: u16 = 0x054C;
const MICROSOFT: u16 = 0x045E;
const BT_HID: &str = "00001124-0000-1000-8000-00805f9b34fb"; // usługa HID Bluetooth w ścieżce

fn sony_model(pid: u16) -> Option<&'static str> {
    match pid {
        0x0CE6 => Some("DualSense"),
        0x0DF2 => Some("DualSense Edge"),
        0x05C4 | 0x09CC => Some("DualShock 4"),
        _ => None,
    }
}

fn is_dualsense(pid: u16) -> bool {
    matches!(pid, 0x0CE6 | 0x0DF2)
}

// --- deklaracje Win32 -------------------------------------------------------------

#[repr(C)]
#[derive(Default)]
struct JoyInfoEx {
    size: u32,
    flags: u32,
    x: u32,
    y: u32,
    z: u32,
    r: u32,
    u: u32,
    v: u32,
    buttons: u32,
    button_number: u32,
    pov: u32,
    reserved1: u32,
    reserved2: u32,
}

#[repr(C)]
struct JoyCapsW {
    mid: u16,
    pid: u16,
    pname: [u16; 32],
    xmin: u32,
    xmax: u32,
    ymin: u32,
    ymax: u32,
    zmin: u32,
    zmax: u32,
    num_buttons: u32,
    period_min: u32,
    period_max: u32,
    rmin: u32,
    rmax: u32,
    umin: u32,
    umax: u32,
    vmin: u32,
    vmax: u32,
    caps: u32,
    max_axes: u32,
    num_axes: u32,
    max_buttons: u32,
    reg_key: [u16; 32],
    oem_vxd: [u16; 260],
}

#[repr(C)]
#[derive(Default, Clone, Copy)]
struct Guid {
    d1: u32,
    d2: u16,
    d3: u16,
    d4: [u8; 8],
}

#[repr(C)]
#[derive(Default)]
struct SpDeviceInterfaceData {
    size: u32,
    guid: Guid,
    flags: u32,
    reserved: usize,
}

#[repr(C)]
#[derive(Default)]
struct HiddAttributes {
    size: u32,
    vendor: u16,
    product: u16,
    version: u16,
}

const JOY_RETURNX: u32 = 0x01;
const JOY_RETURNY: u32 = 0x02;
const JOY_RETURNBUTTONS: u32 = 0x80;
const GENERIC_READ: u32 = 0x8000_0000;
const SHARE_READ_WRITE: u32 = 0x3; // Steam i gra czytają pada równolegle - nikomu go nie zabieramy
const OPEN_EXISTING: u32 = 3;
const DIGCF_PRESENT_INTERFACE: u32 = 0x12;
const INVALID: isize = -1;

#[link(name = "winmm", kind = "raw-dylib")]
unsafe extern "system" {
    fn joyGetNumDevs() -> u32;
    fn joyGetPosEx(id: u32, info: *mut JoyInfoEx) -> u32;
    fn joyGetDevCapsW(id: usize, caps: *mut JoyCapsW, size: u32) -> u32;
}

#[link(name = "setupapi", kind = "raw-dylib")]
unsafe extern "system" {
    fn SetupDiGetClassDevsW(
        guid: *const Guid,
        enumerator: *const u16,
        parent: isize,
        flags: u32,
    ) -> isize;
    fn SetupDiEnumDeviceInterfaces(
        info: isize,
        devinfo: *const c_void,
        guid: *const Guid,
        index: u32,
        data: *mut SpDeviceInterfaceData,
    ) -> i32;
    fn SetupDiGetDeviceInterfaceDetailW(
        info: isize,
        data: *const SpDeviceInterfaceData,
        detail: *mut c_void,
        size: u32,
        required: *mut u32,
        devinfo: *mut c_void,
    ) -> i32;
    fn SetupDiDestroyDeviceInfoList(info: isize) -> i32;
}

#[link(name = "hid", kind = "raw-dylib")]
unsafe extern "system" {
    fn HidD_GetHidGuid(guid: *mut Guid);
    fn HidD_GetAttributes(handle: isize, attrs: *mut HiddAttributes) -> u8;
}

#[link(name = "kernel32", kind = "raw-dylib")]
unsafe extern "system" {
    #[link_name = "CreateFileW"]
    fn create_file_w(
        name: *const u16,
        access: u32,
        share: u32,
        security: *const c_void,
        disposition: u32,
        flags: u32,
        template: isize,
    ) -> isize;
    #[link_name = "ReadFile"]
    fn read_file(
        handle: isize,
        buf: *mut u8,
        len: u32,
        read: *mut u32,
        overlapped: *mut c_void,
    ) -> i32;
    #[link_name = "CancelIoEx"]
    fn cancel_io_ex(handle: isize, overlapped: *mut c_void) -> i32;
}

fn close(handle: isize) {
    // SAFETY: uchwyt pochodzi z CreateFileW i jest zamykany dokładnie raz.
    let _ = unsafe { CloseHandle(HANDLE(handle as *mut c_void)) };
}

fn open(path: &str, access: u32) -> Option<isize> {
    let wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
    // SAFETY: ścieżka zakończona zerem żyje do końca wywołania, pozostałe argumenty są stałe.
    let handle = unsafe {
        create_file_w(
            wide.as_ptr(),
            access,
            SHARE_READ_WRITE,
            std::ptr::null(),
            OPEN_EXISTING,
            0,
            0,
        )
    };
    (handle != 0 && handle != INVALID).then_some(handle)
}

fn wide_to_string(chars: &[u16]) -> String {
    let end = chars.iter().position(|&c| c == 0).unwrap_or(chars.len());
    String::from_utf16_lossy(&chars[..end])
}

// --- XInput ----------------------------------------------------------------------------

struct XInputSource {
    slot: u32,
    name: String,
}

impl PadSource for XInputSource {
    fn name(&self) -> &str {
        &self.name
    }

    fn read(&mut self) -> Option<PadState> {
        let mut state = XINPUT_STATE::default();
        // SAFETY: state to poprawny wskaźnik na lokalną strukturę XINPUT_STATE.
        if unsafe { XInputGetState(self.slot, &mut state) } != 0 {
            return None;
        }
        let g = state.Gamepad;
        Some(PadState {
            name: self.name.clone(),
            x: f64::from(g.sThumbLX) / 32767.0,
            y: f64::from(g.sThumbLY) / 32767.0,
            buttons: xinput_buttons(g.wButtons.0, g.bLeftTrigger, g.bRightTrigger),
        })
    }
}

// --- winmm -----------------------------------------------------------------------------

struct WinmmSource {
    joy_id: u32,
    range: (u32, u32, u32, u32),
    vendor: u16,
    name: String,
}

impl WinmmSource {
    fn new(joy_id: u32, caps: &JoyCapsW) -> WinmmSource {
        let name = if caps.mid == SONY {
            sony_model(caps.pid)
                .unwrap_or("Pad PlayStation")
                .to_string()
        } else {
            let pname = wide_to_string(&caps.pname);
            if pname.trim().is_empty() {
                crate::teksty::t("pad.nazwa.ogolna", &[("liczba", &(joy_id + 1).to_string())])
            } else {
                pname.trim().to_string()
            }
        };
        WinmmSource {
            joy_id,
            range: (caps.xmin, caps.xmax, caps.ymin, caps.ymax),
            vendor: caps.mid,
            name,
        }
    }
}

impl PadSource for WinmmSource {
    fn name(&self) -> &str {
        &self.name
    }

    fn read(&mut self) -> Option<PadState> {
        let mut info = JoyInfoEx {
            size: std::mem::size_of::<JoyInfoEx>() as u32,
            flags: JOY_RETURNX | JOY_RETURNY | JOY_RETURNBUTTONS,
            ..JoyInfoEx::default()
        };
        // SAFETY: info ma ustawiony rozmiar i jest poprawnym wskaźnikiem na lokalną strukturę.
        if unsafe { joyGetPosEx(self.joy_id, &mut info) } != 0 {
            return None;
        }
        let (xmin, xmax, ymin, ymax) = self.range;
        let x = axis(info.x, xmin, xmax);
        let y = -axis(info.y, ymin, ymax); // w DirectInput oś Y rośnie w dół
        let buttons = (0..32)
            .filter(|i| info.buttons & (1 << i) != 0)
            .map(|i| format!("b{}", i + 1))
            .collect();
        Some(PadState {
            name: self.name.clone(),
            x,
            y,
            buttons,
        })
    }
}

// --- pady Sony przez HID ------------------------------------------------------------------

/// Ścieżki urządzeń HID firmy Sony (pady) i ich PID.
fn sony_hid_paths() -> Vec<(String, u16)> {
    let mut guid = Guid::default();
    // SAFETY: guid to poprawny wskaźnik na lokalną strukturę GUID.
    unsafe { HidD_GetHidGuid(&mut guid) };
    // SAFETY: guid żyje do końca wywołania, pozostałe argumenty są puste albo stałe.
    let info = unsafe { SetupDiGetClassDevsW(&guid, std::ptr::null(), 0, DIGCF_PRESENT_INTERFACE) };
    if info == 0 || info == INVALID {
        return Vec::new();
    }
    let mut found = Vec::new();
    for index in 0.. {
        let mut data = SpDeviceInterfaceData {
            size: std::mem::size_of::<SpDeviceInterfaceData>() as u32,
            ..Default::default()
        };
        // SAFETY: info z SetupDiGetClassDevsW, data ma ustawiony rozmiar.
        if unsafe { SetupDiEnumDeviceInterfaces(info, std::ptr::null(), &guid, index, &mut data) }
            == 0
        {
            break;
        }
        if let Some(path) = interface_path(info, &data)
            && let Some(pid) = sony_pad_pid(&path)
        {
            found.push((path, pid));
        }
    }
    // SAFETY: info z SetupDiGetClassDevsW, zwalniane raz.
    unsafe { SetupDiDestroyDeviceInfoList(info) };
    found
}

/// Ścieżka urządzenia z SP_DEVICE_INTERFACE_DETAIL_DATA_W (DWORD cbSize + WCHAR[]).
fn interface_path(info: isize, data: &SpDeviceInterfaceData) -> Option<String> {
    let mut need = 0u32;
    // SAFETY: pytanie o rozmiar - bez bufora, need to poprawny wskaźnik.
    unsafe {
        SetupDiGetDeviceInterfaceDetailW(
            info,
            data,
            std::ptr::null_mut(),
            0,
            &mut need,
            std::ptr::null_mut(),
        )
    };
    if need <= 4 {
        return None;
    }
    // bufor z wyrównaniem do DWORD; cbSize struktury: 8 w 64 bitach, 6 w 32 bitach
    let mut buf = vec![0u32; (need as usize).div_ceil(4) + 1];
    buf[0] = if cfg!(target_pointer_width = "64") {
        8
    } else {
        6
    };
    // SAFETY: bufor ma co najmniej `need` bajtów i ustawione cbSize.
    let ok = unsafe {
        SetupDiGetDeviceInterfaceDetailW(
            info,
            data,
            buf.as_mut_ptr().cast(),
            need,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    if ok == 0 {
        return None;
    }
    let bytes: Vec<u8> = buf.iter().flat_map(|w| w.to_ne_bytes()).collect();
    let wide: Vec<u16> = bytes[4..]
        .as_chunks::<2>()
        .0
        .iter()
        .map(|c| u16::from_ne_bytes(*c))
        .collect();
    Some(wide_to_string(&wide))
}

/// PID pada Sony pod ścieżką albo None (cudzych urządzeń nie otwieramy).
fn sony_pad_pid(path: &str) -> Option<u16> {
    let lower = path.to_lowercase();
    if !lower.contains("vid_054c") && !lower.contains("vid&0002054c") {
        return None;
    }
    let handle = open(path, 0)?;
    let mut attrs = HiddAttributes {
        size: std::mem::size_of::<HiddAttributes>() as u32,
        ..Default::default()
    };
    // SAFETY: uchwyt z CreateFileW, attrs ma ustawiony rozmiar.
    let ok = unsafe { HidD_GetAttributes(handle, &mut attrs) } != 0;
    close(handle);
    (ok && attrs.vendor == SONY && sony_model(attrs.product).is_some()).then_some(attrs.product)
}

#[derive(Default)]
struct HidShared {
    xy: Option<(f64, f64)>,
    buttons: BTreeSet<String>,
    at: Option<Instant>,
}

/// Pad Sony czytany w osobnym wątku (pad sam wysyła raporty setki razy na sekundę).
struct HidSource {
    path: String,
    name: String,
    dualsense: bool,
    shared: Arc<Mutex<HidShared>>,
    /// uchwyt otwarty przez wątek odczytu; None = zamknięty
    handle: Arc<Mutex<Option<isize>>>,
    stop: Arc<AtomicBool>,
}

impl HidSource {
    const STALE: Duration = Duration::from_secs(1);
    const FIRST_REPORT: Duration = Duration::from_millis(300);

    fn new(path: String, pid: u16) -> HidSource {
        let model = sony_model(pid).unwrap_or("Pad PlayStation");
        let link = if path.to_lowercase().contains(BT_HID) {
            "Bluetooth"
        } else {
            "USB"
        };
        HidSource {
            name: format!("{model} ({link})"),
            path,
            dualsense: is_dualsense(pid),
            shared: Arc::default(),
            handle: Arc::default(),
            stop: Arc::default(),
        }
    }

    fn start(&mut self) -> bool {
        let Some(handle) = open(&self.path, GENERIC_READ) else {
            log::info!(
                "Nie da się czytać pada {} (zajęty na wyłączność?)",
                self.name
            );
            return false;
        };
        if let Ok(mut h) = self.handle.lock() {
            *h = Some(handle);
        }
        let (tx, rx) = mpsc::channel::<()>();
        let (shared, slot, stop, dualsense) = (
            self.shared.clone(),
            self.handle.clone(),
            self.stop.clone(),
            self.dualsense,
        );
        let spawned = std::thread::Builder::new()
            .name("pad-hid".into())
            .spawn(move || read_loop(handle, dualsense, &shared, &stop, &tx, &slot));
        if spawned.is_err() {
            self.close_handle();
            return false;
        }
        if rx.recv_timeout(Self::FIRST_REPORT).is_ok() {
            return true;
        }
        PadSource::stop(self);
        false
    }

    fn close_handle(&self) {
        if let Ok(mut h) = self.handle.lock()
            && let Some(handle) = h.take()
        {
            close(handle);
        }
    }
}

fn read_loop(
    handle: isize,
    dualsense: bool,
    shared: &Mutex<HidShared>,
    stop: &AtomicBool,
    first: &mpsc::Sender<()>,
    slot: &Mutex<Option<isize>>,
) {
    let mut buf = vec![0u8; 1024];
    while !stop.load(Ordering::SeqCst) {
        let mut got = 0u32;
        // SAFETY: uchwyt otwarty do odczytu, bufor ma podaną długość, odczyt synchroniczny.
        let ok = unsafe {
            read_file(
                handle,
                buf.as_mut_ptr(),
                buf.len() as u32,
                &mut got,
                std::ptr::null_mut(),
            )
        };
        if ok == 0 {
            break; // pad odłączony albo odczyt przerwany
        }
        let report = &buf[..(got as usize).min(buf.len())];
        let Some(xy) = parse_sony(report) else {
            continue;
        };
        let buttons = parse_sony_buttons(report, dualsense);
        if let Ok(mut s) = shared.lock() {
            *s = HidShared {
                xy: Some(xy),
                buttons,
                at: Some(Instant::now()),
            };
        }
        let _ = first.send(());
    }
    if let Ok(mut h) = slot.lock()
        && let Some(handle) = h.take()
    {
        close(handle);
    }
    if let Ok(mut s) = shared.lock() {
        s.at = None;
    }
}

impl PadSource for HidSource {
    fn name(&self) -> &str {
        &self.name
    }

    fn read(&mut self) -> Option<PadState> {
        let s = self.shared.lock().ok()?;
        let (xy, at) = (s.xy?, s.at?);
        if at.elapsed() > Self::STALE {
            return None;
        }
        Some(PadState {
            name: self.name.clone(),
            x: xy.0,
            y: xy.1,
            buttons: s.buttons.clone(),
        })
    }

    fn stop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Ok(h) = self.handle.lock()
            && let Some(handle) = *h
        {
            // SAFETY: uchwyt jest otwarty, dopóki trzymamy blokadę (wątek zamyka go pod nią).
            unsafe { cancel_io_ex(handle, std::ptr::null_mut()) };
        }
    }
}

impl Drop for HidSource {
    fn drop(&mut self) {
        PadSource::stop(self);
    }
}

/// Podłączone pady: Sony przez HID, potem XInput, potem pozostałe przez winmm.
pub fn discover() -> Vec<Box<dyn PadSource>> {
    let mut found: Vec<Box<dyn PadSource>> = Vec::new();
    for (path, pid) in sony_hid_paths() {
        let mut source = HidSource::new(path, pid);
        if source.start() {
            found.push(Box::new(source));
        }
    }
    let sony_hid = !found.is_empty();
    for slot in 0..4 {
        let mut source = XInputSource {
            slot,
            name: crate::teksty::t("pad.nazwa.xbox", &[("liczba", &(slot + 1).to_string())]),
        };
        if source.read().is_some() {
            found.push(Box::new(source));
        }
    }
    // SAFETY: joyGetNumDevs nie przyjmuje argumentów.
    let count = unsafe { joyGetNumDevs() }.min(16);
    for joy_id in 0..count {
        // SAFETY: JOYCAPSW to same liczby i tablice - same zera są poprawną wartością.
        let mut caps: JoyCapsW = unsafe { std::mem::zeroed() };
        // SAFETY: caps to poprawny wskaźnik na strukturę o podanym rozmiarze.
        if unsafe {
            joyGetDevCapsW(
                joy_id as usize,
                &mut caps,
                std::mem::size_of::<JoyCapsW>() as u32,
            )
        } != 0
        {
            continue;
        }
        let mut source = WinmmSource::new(joy_id, &caps);
        if !found.is_empty() && source.vendor == MICROSOFT {
            continue; // ten sam pad Xbox już jest przez XInput
        }
        if sony_hid && source.vendor == SONY {
            continue; // ten sam pad Sony już jest przez HID (pewniej, także przez BT)
        }
        if source.read().is_some() {
            found.push(Box::new(source));
        }
    }
    found
}
