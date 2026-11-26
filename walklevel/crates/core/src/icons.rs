//! Ikona i nazwa profilu z gry - port bieznia/ikony.py.
//!
//! Folder (albo .exe, albo skrót .lnk) -> główny .exe -> ikona PNG 128 px w dane/ikony.
//! Tylko odczyt plików gry. Ikona z pliku .exe działa tylko na Windows (Win32 GDI);
//! wybór pliku gry, nazwa profilu i skróty .lnk działają wszędzie.

use std::fs;
use std::io::{Cursor, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;
use serde::Serialize;

use crate::store::palette::{self, Color};

/// Jak głęboko szukać pliku gry w folderze.
pub const MAX_DEPTH: usize = 4;
/// Bok ikony profilu w pikselach.
pub const ICON_PX: u32 = 128;

/// Pliki i foldery, które na pewno nie są grą.
static SKIP: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::new(
        r"(?i)unins|setup|install|redist|vc_?redist|dxsetup|prereq|crash|report|easyanticheat|eac_|battleye|be(service|launcher)|helper|updater|dotnet|cefprocess|webhelper|unitycrashhandler|quicksfv|oalinst|physx|touchup|cleanup|benchmark|editor|server|dedicated",
    )
    .ok()
});

/// Foldery techniczne, z których nazwa profilu nie powinna się brać.
const GENERIC_DIRS: &[&str] = &[
    "bin", "x64", "x86", "win64", "win32", "binaries", "game", "_retail_", "release",
];

/// Znaleziony plik gry i proponowana nazwa profilu (z nazwy folderu gry).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FoundGame {
    pub exe: PathBuf,
    pub name: String,
}

fn skipped(name: &str) -> bool {
    SKIP.as_ref().is_some_and(|re| re.is_match(name))
}

fn norm(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .filter(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        .collect()
}

/// Najbardziej prawdopodobny plik gry w folderze: z ikoną, nazwą jak folder, płytko, duży.
pub fn pick_exe(folder: &Path) -> Option<PathBuf> {
    pick_exe_with(folder, MAX_DEPTH, &|p| icon_count(p) > 0)
}

fn pick_exe_with(
    folder: &Path,
    max_depth: usize,
    has_icon: &dyn Fn(&Path) -> bool,
) -> Option<PathBuf> {
    let base = norm(
        &folder
            .file_name()
            .map(|n| n.to_string_lossy())
            .unwrap_or_default(),
    );
    let mut best: Option<(f64, PathBuf)> = None;
    walk(folder, 0, max_depth, &mut |path, depth| {
        let Some(file) = path.file_name().map(|n| n.to_string_lossy().into_owned()) else {
            return;
        };
        if !file.to_lowercase().ends_with(".exe") || skipped(&file) || !has_icon(path) {
            return;
        }
        let Ok(meta) = fs::metadata(path) else { return };
        let size_mb = meta.len() as f64 / 1e6;
        let stem = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let name = norm(&stem.replace("-Win64-Shipping", "").replace("_Win64", ""));
        let mut score = 0.0;
        if !name.is_empty() && (base.contains(&name) || name.contains(&base)) {
            score += 6.0;
        } else if !name.is_empty()
            && !base.is_empty()
            && name.chars().take(4).eq(base.chars().take(4))
        {
            score += 2.0;
        }
        score -= depth as f64 * 1.2;
        score += size_mb.min(200.0) / 50.0;
        if stem.to_lowercase().contains("launcher") {
            score -= 1.5;
        }
        if best.as_ref().is_none_or(|(s, _)| score > *s) {
            best = Some((score, path.to_path_buf()));
        }
    });
    best.map(|(_, p)| p)
}

/// Przejście folderów jak os.walk (najpierw pliki folderu, potem podfoldery w kolejności),
/// bez folderów pominiętych i ukrytych (z kropką), bez wchodzenia w dowiązania.
fn walk(dir: &Path, depth: usize, max_depth: usize, visit: &mut dyn FnMut(&Path, usize)) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut dirs = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_dir() {
            dirs.push(path);
        } else if path.is_file() {
            visit(&path, depth);
        }
    }
    if depth >= max_depth {
        return;
    }
    for d in dirs {
        let name = d
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        if !skipped(&name) && !name.starts_with('.') {
            walk(&d, depth + 1, max_depth, visit);
        }
    }
}

/// Nazwa profilu z folderu gry (bez folderów technicznych typu bin, x64).
pub fn game_name(path: &Path) -> String {
    let mut folder = if path.is_dir() {
        path
    } else {
        path.parent().unwrap_or(path)
    };
    while let Some(parent) = folder.parent() {
        let name = folder
            .file_name()
            .map(|n| n.to_string_lossy().to_lowercase());
        if !name.is_some_and(|n| GENERIC_DIRS.contains(&n.as_str())) {
            break;
        }
        folder = parent;
    }
    match folder.file_name() {
        Some(n) if !n.is_empty() => n.to_string_lossy().into_owned(),
        _ => path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default(),
    }
}

/// Folder, plik .exe albo skrót .lnk -> plik gry i proponowana nazwa (`None` - nie znaleziono).
pub fn resolve(path: &Path) -> Option<FoundGame> {
    resolve_with(path, &|p| icon_count(p) > 0)
}

fn resolve_with(path: &Path, has_icon: &dyn Fn(&Path) -> bool) -> Option<FoundGame> {
    let ext = |p: &Path| p.extension().map(|e| e.to_string_lossy().to_lowercase());
    let target;
    let path = if ext(path).as_deref() == Some("lnk") {
        target = lnk_target(path)?;
        target.as_path()
    } else {
        path
    };
    if path.is_dir() {
        let exe = pick_exe_with(path, MAX_DEPTH, has_icon)?;
        return Some(FoundGame {
            exe,
            name: game_name(path),
        });
    }
    if ext(path).as_deref() == Some("exe") && path.exists() {
        return Some(FoundGame {
            exe: path.to_path_buf(),
            name: game_name(path),
        });
    }
    None
}

// --- skróty .lnk (format MS-SHLLINK, bez COM) -------------------------------------------------

/// Cel skrótu .lnk: ścieżka lokalna z LinkInfo, a gdy jej nie ma albo jest w kodowaniu ANSI
/// z polskimi znakami - ścieżka względna z sekcji StringData.
fn lnk_target(path: &Path) -> Option<PathBuf> {
    let data = fs::read(path).ok()?;
    let u16_at = |o: usize| data.get(o..o + 2).map(|b| u16::from_le_bytes([b[0], b[1]]));
    let u32_at = |o: usize| {
        data.get(o..o + 4)
            .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    };
    if u32_at(0)? != 0x4C {
        return None;
    }
    let flags = u32_at(0x14)?;
    let unicode = flags & 0x80 != 0;
    let mut pos = 0x4C;
    if flags & 0x1 != 0 {
        pos += 2 + usize::from(u16_at(pos)?); // LinkTargetIDList
    }
    let mut local: Option<(String, bool)> = None; // (ścieżka, pewna)
    if flags & 0x2 != 0 {
        let li = pos;
        let size = u32_at(li)? as usize;
        let header = u32_at(li + 4)? as usize;
        if u32_at(li + 8)? & 0x1 != 0 {
            let at = |o: usize| u32_at(li + o).map(|v| li + v as usize);
            local = if header >= 0x24 {
                let base = wide_cstr(&data, at(0x1C)?)?;
                let suffix = at(0x20)
                    .and_then(|o| wide_cstr(&data, o))
                    .unwrap_or_default();
                Some((base + &suffix, true))
            } else {
                let (base, ascii1) = ansi_cstr(&data, at(0x10)?)?;
                let (suffix, ascii2) = at(0x18)
                    .and_then(|o| ansi_cstr(&data, o))
                    .unwrap_or_default();
                Some((base + &suffix, ascii1 && ascii2))
            };
        }
        pos += size;
    }
    if let Some((p, true)) = &local {
        return Some(PathBuf::from(p));
    }
    // StringData: NAME, RELATIVE_PATH, WORKING_DIR... (każde: liczba znaków + znaki)
    let mut relative = None;
    for (bit, is_relative) in [(0x4, false), (0x8, true)] {
        if flags & bit == 0 {
            continue;
        }
        let count = usize::from(u16_at(pos)?);
        pos += 2;
        let len = if unicode { count * 2 } else { count };
        let bytes = data.get(pos..pos + len)?;
        if is_relative {
            relative = Some(if unicode {
                let wide: Vec<u16> = bytes
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|c| u16::from_le_bytes(*c))
                    .collect();
                String::from_utf16_lossy(&wide)
            } else {
                bytes.iter().map(|&b| char::from(b)).collect()
            });
        }
        pos += len;
    }
    if let Some(rel) = relative.filter(|r| !r.is_empty()) {
        let dir = path.parent().unwrap_or(Path::new("."));
        return Some(dir.join(rel));
    }
    local.map(|(p, _)| PathBuf::from(p))
}

fn wide_cstr(data: &[u8], start: usize) -> Option<String> {
    let rest = data.get(start..)?;
    let wide: Vec<u16> = rest
        .as_chunks::<2>()
        .0
        .iter()
        .map(|c| u16::from_le_bytes(*c))
        .take_while(|&c| c != 0)
        .collect();
    Some(String::from_utf16_lossy(&wide))
}

/// Tekst ANSI zakończony zerem; drugi element: czy same znaki ASCII (pewne odczytanie).
fn ansi_cstr(data: &[u8], start: usize) -> Option<(String, bool)> {
    let rest = data.get(start..)?;
    let bytes: Vec<u8> = rest.iter().copied().take_while(|&b| b != 0).collect();
    let ascii = bytes.is_ascii();
    Some((bytes.iter().map(|&b| char::from(b)).collect(), ascii))
}

// --- ikona z pliku .exe -------------------------------------------------------------------

/// Obraz RGBA 8 bit, wiersze od góry.
#[derive(Debug, Clone, PartialEq)]
struct Rgba {
    w: usize,
    h: usize,
    px: Vec<u8>,
}

impl Rgba {
    fn alpha(&self, x: usize, y: usize) -> u8 {
        self.px[(y * self.w + x) * 4 + 3]
    }
}

/// Ikona z pliku .exe -> PNG `ICON_PX` px w `out_png` (puste brzegi przycięte, pikselowe
/// ikony zostają ostre). Zwraca kolor profilu dobrany z ikony, `None` - ikony nie ma
/// albo nie da się jej zapisać.
pub fn extract_icon(exe: &Path, out_png: &Path) -> Option<&'static Color> {
    let img = exe_icon_rgba(exe, 256)?;
    let cropped = crop_square(&img)?;
    let size = ICON_PX as usize;
    let out = scale(&cropped, size, cropped.w > 64);
    let png = encode_png(&out);
    let saved = out_png
        .parent()
        .filter(|d| !d.as_os_str().is_empty())
        .map_or(Ok(()), fs::create_dir_all)
        .and_then(|()| fs::write(out_png, png));
    if let Err(e) = saved {
        log::warn!("Nie da się zapisać ikony {}: {e}", out_png.display());
        return None;
    }
    Some(icon_color_rgba(out.w, out.h, &out.px))
}

/// Kolor profilu z obrazu RGBA (co drugi piksel w obu osiach, jak z zapisanej ikony w 0.5.x):
/// najczęstszy nasycony odcień -> najbliższy z palety.
pub fn icon_color_rgba(width: usize, height: usize, rgba: &[u8]) -> &'static Color {
    let mut pixels = Vec::with_capacity(width.div_ceil(2) * height.div_ceil(2));
    for y in (0..height).step_by(2) {
        for x in (0..width).step_by(2) {
            let i = (y * width + x) * 4;
            if let Some(p) = rgba.get(i..i + 4) {
                pixels.push([p[0], p[1], p[2], p[3]]);
            }
        }
    }
    palette::from_rgb_pixels(&pixels)
}

/// Kwadrat wokół nieprzezroczystej części (alfa > 16), poza obrazem - przezroczysty.
fn crop_square(img: &Rgba) -> Option<Rgba> {
    let (mut x0, mut y0, mut x1, mut y1) = (usize::MAX, usize::MAX, 0, 0);
    for y in 0..img.h {
        for x in 0..img.w {
            if img.alpha(x, y) > 16 {
                x0 = x0.min(x);
                y0 = y0.min(y);
                x1 = x1.max(x);
                y1 = y1.max(y);
            }
        }
    }
    if x0 == usize::MAX {
        return None;
    }
    let side = (x1 - x0).max(y1 - y0) + 1;
    // wyśrodkowanie bez gubienia skrajnej kolumny (0.5.x przy parzystej szerokości ją ucinał)
    let left = x0 as isize - ((side - (x1 - x0 + 1)) / 2) as isize;
    let top = y0 as isize - ((side - (y1 - y0 + 1)) / 2) as isize;
    let mut px = vec![0u8; side * side * 4];
    for y in 0..side {
        for x in 0..side {
            let (sx, sy) = (left + x as isize, top + y as isize);
            if sx >= 0 && sy >= 0 && (sx as usize) < img.w && (sy as usize) < img.h {
                let s = (sy as usize * img.w + sx as usize) * 4;
                let d = (y * side + x) * 4;
                px[d..d + 4].copy_from_slice(&img.px[s..s + 4]);
            }
        }
    }
    Some(Rgba {
        w: side,
        h: side,
        px,
    })
}

/// Skalowanie kwadratu do `size`: najbliższy piksel (ostre pikselowe ikony) albo gładko
/// (średnia z obszaru przy zmniejszaniu, dwuliniowo przy powiększaniu), z alfą
/// przemnożoną, żeby przezroczyste brzegi nie ciemniały.
fn scale(img: &Rgba, size: usize, smooth: bool) -> Rgba {
    let side = img.w;
    let mut px = vec![0u8; size * size * 4];
    if side == 0 {
        return Rgba {
            w: size,
            h: size,
            px,
        };
    }
    let premul = |x: usize, y: usize| -> [f64; 4] {
        let i = (y * side + x) * 4;
        let a = f64::from(img.px[i + 3]) / 255.0;
        [
            f64::from(img.px[i]) * a,
            f64::from(img.px[i + 1]) * a,
            f64::from(img.px[i + 2]) * a,
            f64::from(img.px[i + 3]),
        ]
    };
    let ratio = side as f64 / size as f64;
    for ty in 0..size {
        for tx in 0..size {
            let d = (ty * size + tx) * 4;
            if !smooth || side == size {
                let s = ((ty * side / size) * side + tx * side / size) * 4;
                px[d..d + 4].copy_from_slice(&img.px[s..s + 4]);
                continue;
            }
            let acc = if side > size {
                area_sample(tx, ty, ratio, side, &premul)
            } else {
                bilinear_sample(tx, ty, ratio, side, &premul)
            };
            let a = acc[3];
            let un = |v: f64| {
                if a > 0.0 {
                    (v * 255.0 / a).round().clamp(0.0, 255.0) as u8
                } else {
                    0
                }
            };
            px[d] = un(acc[0]);
            px[d + 1] = un(acc[1]);
            px[d + 2] = un(acc[2]);
            px[d + 3] = a.round().clamp(0.0, 255.0) as u8;
        }
    }
    Rgba {
        w: size,
        h: size,
        px,
    }
}

fn area_sample(
    tx: usize,
    ty: usize,
    ratio: f64,
    side: usize,
    premul: &dyn Fn(usize, usize) -> [f64; 4],
) -> [f64; 4] {
    let (fx0, fx1) = (tx as f64 * ratio, (tx + 1) as f64 * ratio);
    let (fy0, fy1) = (ty as f64 * ratio, (ty + 1) as f64 * ratio);
    let mut acc = [0.0; 4];
    let mut total = 0.0;
    for sy in (fy0.floor() as usize)..(fy1.ceil() as usize).min(side) {
        let wy = (fy1.min(sy as f64 + 1.0) - fy0.max(sy as f64)).max(0.0);
        for sx in (fx0.floor() as usize)..(fx1.ceil() as usize).min(side) {
            let wx = (fx1.min(sx as f64 + 1.0) - fx0.max(sx as f64)).max(0.0);
            let w = wx * wy;
            let p = premul(sx, sy);
            for (a, v) in acc.iter_mut().zip(p) {
                *a += v * w;
            }
            total += w;
        }
    }
    if total > 0.0 {
        acc.iter_mut().for_each(|a| *a /= total);
    }
    acc
}

fn bilinear_sample(
    tx: usize,
    ty: usize,
    ratio: f64,
    side: usize,
    premul: &dyn Fn(usize, usize) -> [f64; 4],
) -> [f64; 4] {
    let max = (side - 1) as f64;
    let sx = ((tx as f64 + 0.5) * ratio - 0.5).clamp(0.0, max);
    let sy = ((ty as f64 + 0.5) * ratio - 0.5).clamp(0.0, max);
    let (x0, y0) = (sx.floor() as usize, sy.floor() as usize);
    let (x1, y1) = ((x0 + 1).min(side - 1), (y0 + 1).min(side - 1));
    let (fx, fy) = (sx - x0 as f64, sy - y0 as f64);
    let (p00, p10, p01, p11) = (
        premul(x0, y0),
        premul(x1, y0),
        premul(x0, y1),
        premul(x1, y1),
    );
    let mut acc = [0.0; 4];
    for i in 0..4 {
        let top = p00[i] * (1.0 - fx) + p10[i] * fx;
        let bottom = p01[i] * (1.0 - fx) + p11[i] * fx;
        acc[i] = top * (1.0 - fy) + bottom * fy;
    }
    acc
}

// --- zapis PNG ------------------------------------------------------------------------------

fn crc32(chunks: &[&[u8]]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for data in chunks {
        for &b in *data {
            crc ^= u32::from(b);
            for _ in 0..8 {
                crc = if crc & 1 != 0 {
                    (crc >> 1) ^ 0xEDB8_8320
                } else {
                    crc >> 1
                };
            }
        }
    }
    !crc
}

fn adler32(data: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for &x in data {
        a = (a + u32::from(x)) % 65521;
        b = (b + a) % 65521;
    }
    (b << 16) | a
}

/// Surowy strumień deflate. Biblioteki deflate nie mamy wprost - kompresuje ją zip
/// (wpis Deflated w archiwum w pamięci, odczyt surowych danych); awaryjnie bloki bez kompresji.
fn deflate(data: &[u8]) -> Vec<u8> {
    let compressed = (|| -> Option<Vec<u8>> {
        let mut w = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let opts = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        w.start_file("d", opts).ok()?;
        w.write_all(data).ok()?;
        let mut archive = zip::ZipArchive::new(w.finish().ok()?).ok()?;
        let mut file = archive.by_index_raw(0).ok()?;
        let mut out = Vec::new();
        file.read_to_end(&mut out).ok()?;
        Some(out)
    })();
    compressed.unwrap_or_else(|| stored_blocks(data))
}

fn stored_blocks(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len() + data.len() / 65535 * 5 + 5);
    let chunks: Vec<&[u8]> = if data.is_empty() {
        vec![&[]]
    } else {
        data.chunks(65535).collect()
    };
    let last = chunks.len() - 1;
    for (i, chunk) in chunks.iter().enumerate() {
        out.push(u8::from(i == last));
        let len = chunk.len() as u16;
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(&(!len).to_le_bytes());
        out.extend_from_slice(chunk);
    }
    out
}

fn encode_png(img: &Rgba) -> Vec<u8> {
    let mut raw = Vec::with_capacity((img.w * 4 + 1) * img.h);
    for row in img.px.chunks(img.w * 4) {
        raw.push(0); // filtr: brak
        raw.extend_from_slice(row);
    }
    let mut zlib = vec![0x78, 0x9C];
    zlib.extend(deflate(&raw));
    zlib.extend_from_slice(&adler32(&raw).to_be_bytes());

    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&(img.w as u32).to_be_bytes());
    ihdr.extend_from_slice(&(img.h as u32).to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]); // 8 bit, RGBA

    let mut png = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    for (kind, body) in [
        (b"IHDR", &ihdr[..]),
        (b"IDAT", &zlib[..]),
        (b"IEND", &[][..]),
    ] {
        png.extend_from_slice(&(body.len() as u32).to_be_bytes());
        png.extend_from_slice(kind);
        png.extend_from_slice(body);
        png.extend_from_slice(&crc32(&[kind, body]).to_be_bytes());
    }
    png
}

// --- Win32: liczba ikon i piksele ikony ----------------------------------------------------

#[cfg(windows)]
fn wide_path(path: &Path) -> Option<[u16; 260]> {
    use std::os::windows::ffi::OsStrExt;
    let wide: Vec<u16> = path.as_os_str().encode_wide().collect();
    if wide.len() >= 260 || wide.contains(&0) {
        return None;
    }
    let mut buf = [0u16; 260];
    buf[..wide.len()].copy_from_slice(&wide);
    Some(buf)
}

/// Liczba ikon w pliku (0 - bez ikony albo błąd). Poza Windows zawsze 1.
#[cfg(windows)]
fn icon_count(path: &Path) -> u32 {
    use windows::Win32::UI::WindowsAndMessaging::PrivateExtractIconsW;
    let Some(name) = wide_path(path) else {
        return 0;
    };
    // SAFETY: `name` to bufor MAX_PATH zakończony zerem; bez tablicy uchwytów funkcja
    // tylko liczy ikony i niczego nie zapisuje.
    let n = unsafe { PrivateExtractIconsW(&name, 0, 0, 0, None, None, 0) };
    if n == u32::MAX { 0 } else { n }
}

#[cfg(not(windows))]
fn icon_count(_path: &Path) -> u32 {
    1
}

#[cfg(not(windows))]
fn exe_icon_rgba(_exe: &Path, _size: i32) -> Option<Rgba> {
    None
}

/// Pierwsza ikona pliku w rozmiarze `size` jako RGBA.
#[cfg(windows)]
fn exe_icon_rgba(exe: &Path, size: i32) -> Option<Rgba> {
    use windows::Win32::UI::WindowsAndMessaging::{DestroyIcon, HICON, PrivateExtractIconsW};
    let name = wide_path(exe)?;
    let mut icons = [HICON::default()];
    // SAFETY: `name` zakończony zerem, tablica na jeden uchwyt; funkcja wpisze co najwyżej
    // tyle uchwytów, ile ma miejsca.
    let n = unsafe { PrivateExtractIconsW(&name, 0, size, size, Some(&mut icons), None, 0) };
    if n == 0 || n == u32::MAX || icons[0].is_invalid() {
        return None;
    }
    let img = hicon_rgba(icons[0]);
    // SAFETY: uchwyt z PrivateExtractIconsW, zwalniany dokładnie raz.
    if let Err(e) = unsafe { DestroyIcon(icons[0]) } {
        log::debug!("DestroyIcon: {e}");
    }
    img
}

#[cfg(windows)]
fn hicon_rgba(icon: windows::Win32::UI::WindowsAndMessaging::HICON) -> Option<Rgba> {
    use windows::Win32::Graphics::Gdi::DeleteObject;
    use windows::Win32::UI::WindowsAndMessaging::{GetIconInfo, ICONINFO};
    let mut info = ICONINFO::default();
    // SAFETY: poprawny uchwyt ikony i wskaźnik na lokalną strukturę.
    unsafe { GetIconInfo(icon, &mut info) }.ok()?;
    let img = (|| {
        if info.hbmColor.is_invalid() {
            return None; // ikona monochromatyczna - pomijamy
        }
        let (w, h, mut px) = bitmap_bgra(info.hbmColor)?;
        if px.as_chunks::<4>().0.iter().all(|p| p[3] == 0) {
            // stara ikona bez kanału alfa: przezroczystość z maski (biały = przezroczysty)
            let (mw, mh, mask) = bitmap_bgra(info.hbmMask)?;
            if mw == w && mh >= h {
                for (p, m) in px
                    .as_chunks_mut::<4>()
                    .0
                    .iter_mut()
                    .zip(mask.as_chunks::<4>().0)
                {
                    p[3] = if m[0] == 0 { 255 } else { 0 };
                }
            }
        }
        for p in px.as_chunks_mut::<4>().0 {
            p.swap(0, 2); // BGRA -> RGBA
        }
        Some(Rgba { w, h, px })
    })();
    // SAFETY: bitmapy z GetIconInfo należą do nas i trzeba je usunąć.
    unsafe {
        let _ = DeleteObject(info.hbmColor.into());
        let _ = DeleteObject(info.hbmMask.into());
    }
    img
}

/// Piksele bitmapy jako 32 bit BGRA, wiersze od góry.
#[cfg(windows)]
fn bitmap_bgra(bmp: windows::Win32::Graphics::Gdi::HBITMAP) -> Option<(usize, usize, Vec<u8>)> {
    use windows::Win32::Graphics::Gdi::{
        BI_RGB, BITMAP, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleDC, DIB_RGB_COLORS, DeleteDC,
        GetDIBits, GetObjectW,
    };
    let mut bm = BITMAP::default();
    // SAFETY: wskaźnik na lokalną strukturę BITMAP o podanym rozmiarze.
    let got = unsafe {
        GetObjectW(
            bmp.into(),
            std::mem::size_of::<BITMAP>() as i32,
            Some((&mut bm as *mut BITMAP).cast()),
        )
    };
    if got == 0 || bm.bmWidth <= 0 || bm.bmHeight <= 0 || bm.bmWidth > 1024 || bm.bmHeight > 2048 {
        return None;
    }
    let (w, h) = (bm.bmWidth as usize, bm.bmHeight as usize);
    let mut bmi = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: bm.bmWidth,
            biHeight: -bm.bmHeight, // wiersze od góry
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut buf = vec![0u8; w * h * 4];
    // SAFETY: kontekst urządzenia tylko do odczytu bitmapy, zwalniany niżej.
    let dc = unsafe { CreateCompatibleDC(None) };
    if dc.is_invalid() {
        return None;
    }
    // SAFETY: bufor ma dokładnie w*h*4 bajtów dla 32-bitowego DIB o wymiarach z GetObjectW.
    let lines = unsafe {
        GetDIBits(
            dc,
            bmp,
            0,
            h as u32,
            Some(buf.as_mut_ptr().cast()),
            &mut bmi,
            DIB_RGB_COLORS,
        )
    };
    // SAFETY: dc z CreateCompatibleDC, zwalniany raz.
    let _ = unsafe { DeleteDC(dc) };
    if lines <= 0 { None } else { Some((w, h, buf)) }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sized_file(path: &Path, len: u64) {
        fs::File::create(path).unwrap().set_len(len).unwrap();
    }

    fn make_game(root: &Path) -> PathBuf {
        let game = root.join("Baldurs Gate 3");
        fs::create_dir_all(game.join("bin")).unwrap();
        fs::create_dir_all(game.join("Redist")).unwrap();
        fs::create_dir_all(game.join("Launcher")).unwrap();
        sized_file(&game.join("bin").join("bg3.exe"), 5_000_000);
        sized_file(&game.join("bin").join("CrashReporter.exe"), 9_000_000);
        sized_file(&game.join("Redist").join("vc_redist.x64.exe"), 1);
        sized_file(&game.join("unins000.exe"), 1);
        sized_file(&game.join("Launcher").join("LariLauncher.exe"), 1000);
        game
    }

    #[test]
    fn pick_exe_skips_installers_and_crash_reporters() {
        let dir = tempfile::tempdir().unwrap();
        let game = make_game(dir.path());
        let any = |_: &Path| true;
        let found = resolve_with(&game, &any).unwrap();
        assert_eq!(found.exe.file_name().unwrap(), "bg3.exe");
        assert_eq!(found.name, "Baldurs Gate 3");
        // wskazany sam plik w bin - nazwa i tak z folderu gry
        let direct = resolve_with(&game.join("bin").join("bg3.exe"), &any).unwrap();
        assert_eq!(direct.name, "Baldurs Gate 3");
        assert!(resolve_with(&dir.path().join("nie ma"), &any).is_none());
        // bez ikon nic nie pasuje
        assert!(resolve_with(&game, &|_: &Path| false).is_none());
    }

    #[test]
    fn deep_and_hidden_folders_are_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let game = dir.path().join("Gra");
        let deep = game.join("a").join("b").join("c").join("d").join("e");
        fs::create_dir_all(&deep).unwrap();
        fs::create_dir_all(game.join(".ukryty")).unwrap();
        sized_file(&deep.join("Gra.exe"), 10);
        sized_file(&game.join(".ukryty").join("Gra.exe"), 10);
        assert!(pick_exe_with(&game, MAX_DEPTH, &|_: &Path| true).is_none());
        let level4 = game.join("a").join("b").join("c").join("d");
        sized_file(&level4.join("Gra.exe"), 10);
        assert_eq!(
            pick_exe_with(&game, MAX_DEPTH, &|_: &Path| true),
            Some(level4.join("Gra.exe"))
        );
    }

    #[test]
    fn game_name_skips_technical_folders() {
        let dir = tempfile::tempdir().unwrap();
        let exe_dir = dir.path().join("Cyberpunk 2077").join("bin").join("x64");
        fs::create_dir_all(&exe_dir).unwrap();
        let exe = exe_dir.join("Cyberpunk2077.exe");
        sized_file(&exe, 1);
        assert_eq!(game_name(&exe), "Cyberpunk 2077");
        assert_eq!(game_name(&exe_dir), "Cyberpunk 2077");
    }

    fn lnk(local: Option<&str>, relative: Option<&str>) -> Vec<u8> {
        let mut flags = 0x80u32; // IsUnicode
        let mut body = Vec::new();
        if let Some(p) = local {
            flags |= 0x2;
            let header = 0x1Cu32;
            let base_off = header;
            let suffix_off = base_off + p.len() as u32 + 1;
            let size = suffix_off + 1;
            for v in [size, header, 1, 0, base_off, 0, suffix_off] {
                body.extend_from_slice(&v.to_le_bytes());
            }
            body.extend_from_slice(p.as_bytes());
            body.extend_from_slice(&[0, 0]);
        }
        if let Some(r) = relative {
            flags |= 0x8;
            let wide: Vec<u16> = r.encode_utf16().collect();
            body.extend_from_slice(&(wide.len() as u16).to_le_bytes());
            for c in wide {
                body.extend_from_slice(&c.to_le_bytes());
            }
        }
        let mut data = vec![0u8; 0x4C];
        data[0..4].copy_from_slice(&0x4Cu32.to_le_bytes());
        data[0x14..0x18].copy_from_slice(&flags.to_le_bytes());
        data.extend(body);
        data
    }

    #[test]
    fn shortcut_targets_are_read_without_com() {
        let dir = tempfile::tempdir().unwrap();
        let game = make_game(dir.path());
        let exe = game.join("bin").join("bg3.exe");
        let any = |_: &Path| true;

        let ascii = dir.path().join("ascii.lnk");
        fs::write(&ascii, lnk(Some(&exe.to_string_lossy()), None)).unwrap();
        if exe.to_string_lossy().is_ascii() {
            assert_eq!(lnk_target(&ascii), Some(exe.clone()));
            assert_eq!(resolve_with(&ascii, &any).unwrap().exe, exe);
        }
        // ścieżka względna (Unicode) - działa też z polskimi znakami
        let rel = dir.path().join("względny.lnk");
        fs::write(
            &rel,
            lnk(Some("C:\\zła ścieżka ANSI"), Some(".\\Baldurs Gate 3")),
        )
        .unwrap();
        let found = resolve_with(&rel, &any).unwrap();
        assert_eq!(found.exe.file_name().unwrap(), "bg3.exe");
        assert_eq!(found.name, "Baldurs Gate 3");

        let broken = dir.path().join("zepsuty.lnk");
        fs::write(&broken, b"nie skrot").unwrap();
        assert!(resolve_with(&broken, &any).is_none());
    }

    fn image(w: usize, h: usize, f: impl Fn(usize, usize) -> [u8; 4]) -> Rgba {
        let mut px = Vec::with_capacity(w * h * 4);
        for y in 0..h {
            for x in 0..w {
                px.extend_from_slice(&f(x, y));
            }
        }
        Rgba { w, h, px }
    }

    #[test]
    fn crop_keeps_opaque_part_centered_in_square() {
        // 10x10, nieprzezroczysty prostokąt x 2..=5, y 3..=4
        let img = image(10, 10, |x, y| {
            if (2..=5).contains(&x) && (3..=4).contains(&y) {
                [255, 0, 0, 255]
            } else {
                [0, 0, 0, 0]
            }
        });
        let c = crop_square(&img).unwrap();
        assert_eq!((c.w, c.h), (4, 4));
        let opaque: usize = c.px.chunks(4).filter(|p| p[3] == 255).count();
        assert_eq!(opaque, 8);
        assert!(crop_square(&image(4, 4, |_, _| [9, 9, 9, 10])).is_none());
    }

    #[test]
    fn scaling_keeps_pixel_art_sharp_and_smooths_big_icons() {
        let checker = image(2, 2, |x, y| {
            if (x + y) % 2 == 0 {
                [255, 255, 255, 255]
            } else {
                [0, 0, 0, 255]
            }
        });
        let sharp = scale(&checker, 4, false);
        assert_eq!(&sharp.px[0..4], &[255, 255, 255, 255]);
        assert_eq!(&sharp.px[4..8], &[255, 255, 255, 255]);
        assert_eq!(&sharp.px[8..12], &[0, 0, 0, 255]);
        let big = image(256, 256, |x, _| {
            if x % 2 == 0 {
                [200, 0, 0, 255]
            } else {
                [0, 0, 200, 255]
            }
        });
        let small = scale(&big, 128, true);
        assert_eq!(&small.px[0..4], &[100, 0, 100, 255]); // średnia z obszaru
        // przezroczyste piksele nie przyciemniają koloru
        let half = image(256, 256, |x, _| {
            if x % 2 == 0 {
                [255, 0, 0, 255]
            } else {
                [0, 0, 0, 0]
            }
        });
        assert_eq!(&scale(&half, 128, true).px[0..4], &[255, 0, 0, 128]);
        let up = scale(&image(100, 100, |_, _| [10, 20, 30, 255]), 128, true);
        assert!(up.px.chunks(4).all(|p| p == [10, 20, 30, 255]));
    }

    #[test]
    fn png_is_valid() {
        let img = image(3, 2, |x, y| [x as u8 * 80, y as u8 * 100, 7, 255]);
        let png = encode_png(&img);
        assert_eq!(&png[..8], &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]);
        // każdy fragment ma poprawną sumę CRC
        let mut pos = 8;
        let mut kinds = Vec::new();
        while pos < png.len() {
            let len = u32::from_be_bytes(png[pos..pos + 4].try_into().unwrap()) as usize;
            let kind = &png[pos + 4..pos + 8];
            let body = &png[pos + 8..pos + 8 + len];
            let crc = u32::from_be_bytes(png[pos + 8 + len..pos + 12 + len].try_into().unwrap());
            assert_eq!(crc, crc32(&[kind, body]));
            kinds.push(String::from_utf8(kind.to_vec()).unwrap());
            pos += 12 + len;
        }
        assert_eq!(kinds, ["IHDR", "IDAT", "IEND"]);
        assert_eq!(crc32(&[b"123456789"]), 0xCBF4_3926);
        assert_eq!(adler32(b"Wikipedia"), 0x11E6_0398);
        // bloki bez kompresji: nagłówek + dane
        let stored = stored_blocks(b"abc");
        assert_eq!(stored, [1, 3, 0, 0xFC, 0xFF, b'a', b'b', b'c']);
    }

    #[test]
    fn icon_colour_from_rgba() {
        let yellow = image(8, 8, |_, _| [250, 215, 0, 255]);
        assert_eq!(icon_color_rgba(8, 8, &yellow.px).name, "Banan");
        assert_eq!(icon_color_rgba(0, 0, &[]).name, "Mięta");
    }

    #[cfg(windows)]
    #[test]
    fn icon_from_real_windows_exe() {
        let windir = std::env::var("WINDIR").unwrap_or_else(|_| r"C:\Windows".into());
        let exe = Path::new(&windir).join("explorer.exe");
        assert!(icon_count(&exe) > 0);
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("ikony").join("ikona.png");
        let color = extract_icon(&exe, &out).unwrap();
        assert!(out.exists() && fs::metadata(&out).unwrap().len() > 100);
        assert!(palette::names().contains(&color.name));
        assert!(extract_icon(&dir.path().join("brak.exe"), &out).is_none());
        assert_eq!(icon_count(&dir.path().join("brak.exe")), 0);
    }
}
