//! Kolory osób i profili gier: 20 par odcieni (ciemny motyw, jasny motyw) - port bieznia/paleta.py.
//!
//! Każdy odcień przechodzi kontrast 4,5:1 z tłem motywu i z tekstem na kolorze.
//! Mięta to dotychczasowy kolor aplikacji (domyślny).

use serde::Serialize;

/// Kolor z palety. W osoby.json zapisywana jest tylko nazwa (`name`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Color {
    pub name: &'static str,
    /// odcień w stopniach, do dopasowania koloru ikony
    pub hue: i32,
    /// odcień na ciemny motyw
    pub dark: &'static str,
    /// odcień na jasny motyw
    pub light: &'static str,
}

impl Color {
    /// Odcień pod motyw ("dark" albo inny = jasny) - do tekstu i akcentów.
    pub fn for_theme(&self, theme_name: &str) -> &'static str {
        if theme_name == "dark" {
            self.dark
        } else {
            self.light
        }
    }

    /// Żywy kolor do kółek palety, awatarów i wykresów - taki sam w obu motywach.
    pub fn swatch(&self) -> &'static str {
        self.dark
    }
}

const fn c(name: &'static str, hue: i32, dark: &'static str, light: &'static str) -> Color {
    Color {
        name,
        hue,
        dark,
        light,
    }
}

/// Paleta 20 kolorów w kolejności wyświetlania.
pub static COLORS: [Color; 20] = [
    c("Czerwień", 0, "#fa3838", "#e70606"),
    c("Koral", 10, "#f34523", "#dc2e0c"),
    c("Pomarańcz", 25, "#f96b06", "#bf5305"),
    c("Morela", 32, "#df8620", "#a56318"),
    c("Banan", 52, "#f9d806", "#847303"),
    c("Limonka", 80, "#a4ec13", "#587f0a"),
    c("Zieleń", 118, "#26df20", "#178613"),
    c("Szmaragd", 145, "#19e66f", "#0f8540"),
    c("Mięta", 158, "#8fc2b0", "#2f5d50"),
    c("Morski", 174, "#13ecd6", "#0b8276"),
    c("Niebo", 196, "#0db5f2", "#097ba5"),
    c("Błękit", 212, "#2384f3", "#0c6fe1"),
    c("Chaber", 226, "#5c7ff2", "#1346ec"),
    c("Indygo", 242, "#7c78f0", "#2019e6"),
    c("Fiolet", 258, "#956df3", "#5413ec"),
    c("Lawenda", 270, "#a16dd5", "#7f39c6"),
    c("Orchidea", 292, "#cf46e5", "#c01fd8"),
    c("Fuksja", 312, "#f312c6", "#d30bab"),
    c("Róż", 330, "#f42f91", "#df0c75"),
    c("Malina", 345, "#f0426e", "#e21246"),
];

/// Nazwa koloru domyślnego.
pub const DEFAULT: &str = "Mięta";
const DEFAULT_INDEX: usize = 8;
const RUN_ALT_INDEX: usize = 19;

/// Tekst na kolorze w ciemnym motywie (ciemny na jasnym odcieniu).
pub const ON_DARK_THEME: &str = "#0f1f1a";
/// Tekst na kolorze w jasnym motywie (biały na ciemnym odcieniu).
pub const ON_LIGHT_THEME: &str = "#ffffff";

/// Kolor domyślny (Mięta).
pub fn default_color() -> &'static Color {
    &COLORS[DEFAULT_INDEX]
}

/// Kolor biegu, gdy kolor profilu jest blisko bursztynu biegu (Malina).
pub fn run_alt() -> &'static Color {
    &COLORS[RUN_ALT_INDEX]
}

/// Kolor kolejnej osoby, gdy jest ich już `count` (po kolei z palety, w kółko).
pub fn next_for(count: usize) -> &'static Color {
    &COLORS[count % COLORS.len()]
}

/// Kolor po nazwie; nieznana nazwa albo brak - Mięta.
pub fn color(name: Option<&str>) -> &'static Color {
    let name = name.unwrap_or("");
    COLORS
        .iter()
        .find(|k| k.name == name)
        .unwrap_or(default_color())
}

/// Nazwy wszystkich kolorów w kolejności palety.
pub fn names() -> Vec<&'static str> {
    COLORS.iter().map(|k| k.name).collect()
}

/// Żółcie i pomarańcze zlewałyby się z bursztynem strefy biegu.
pub fn clashes_with_run(k: &Color) -> bool {
    (10..=70).contains(&k.hue)
}

/// Kolor palety najbliższy odcieniowi (Mięta pomijana - to kolor ikon bez koloru).
pub fn nearest(hue: f64) -> &'static Color {
    let dist = |k: &Color| {
        let d = (f64::from(k.hue) - hue).abs() % 360.0;
        d.min(360.0 - d)
    };
    COLORS
        .iter()
        .filter(|k| k.name != DEFAULT)
        .min_by(|a, b| dist(a).total_cmp(&dist(b)))
        .unwrap_or(default_color())
}

/// colorsys.rgb_to_hls z Pythona (wartości 0..1) -> (odcień, jasność, nasycenie).
fn rgb_to_hls(r: f64, g: f64, b: f64) -> (f64, f64, f64) {
    let maxc = r.max(g).max(b);
    let minc = r.min(g).min(b);
    let sumc = maxc + minc;
    let rangec = maxc - minc;
    let l = sumc / 2.0;
    if minc == maxc {
        return (0.0, l, 0.0);
    }
    let s = if l <= 0.5 {
        rangec / sumc
    } else {
        rangec / (2.0 - maxc - minc)
    };
    let rc = (maxc - r) / rangec;
    let gc = (maxc - g) / rangec;
    let bc = (maxc - b) / rangec;
    let h = if r == maxc {
        bc - gc
    } else if g == maxc {
        2.0 + rc - bc
    } else {
        4.0 + gc - rc
    };
    ((h / 6.0).rem_euclid(1.0), l, s)
}

/// Kolor z pikseli ikony ([r, g, b, alfa]): najczęstszy nasycony odcień -> najbliższy z palety.
///
/// Gdy kolorowych pikseli jest mniej niż 8% (szara, czarno-biała ikona) - Mięta.
pub fn from_rgb_pixels(pixels: &[[u8; 4]]) -> &'static Color {
    let mut bins = [0.0f64; 36];
    let mut opaque = 0u32;
    let mut colored = 0u32;
    for &[r, g, b, a] in pixels {
        if a < 128 {
            continue;
        }
        opaque += 1;
        let (h, light, s) = rgb_to_hls(
            f64::from(r) / 255.0,
            f64::from(g) / 255.0,
            f64::from(b) / 255.0,
        );
        if s < 0.28 || !(0.12..=0.92).contains(&light) {
            continue;
        }
        colored += 1;
        // h w [0, 1), więc indeks mieści się w 0..36
        let idx = ((h * 36.0) as usize) % 36;
        bins[idx] += s * (1.0 - (light - 0.5).abs());
    }
    if opaque == 0 || f64::from(colored) / f64::from(opaque) < 0.08 {
        return default_color();
    }
    // pierwszy największy (jak bins.index(max(bins)) w Pythonie)
    let mut best = 0;
    for (i, v) in bins.iter().enumerate() {
        if *v > bins[best] {
            best = i;
        }
    }
    nearest((best as f64 + 0.5) * 10.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn fixture() -> Value {
        let text = include_str!("../../tests/fixtures/dane/paleta.json");
        serde_json::from_str(text).unwrap()
    }

    // bieznia/gui/theme.py: luminance, contrast i kolory motywów
    fn luminance(hex: &str) -> f64 {
        let h = hex.trim_start_matches('#');
        let ch = |i: usize| {
            let c = f64::from(u8::from_str_radix(&h[i..i + 2], 16).unwrap()) / 255.0;
            if c <= 0.04045 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * ch(0) + 0.7152 * ch(2) + 0.0722 * ch(4)
    }

    fn contrast(a: &str, b: &str) -> f64 {
        let (la, lb) = (luminance(a), luminance(b));
        let (hi, lo) = if la > lb { (la, lb) } else { (lb, la) };
        (hi + 0.05) / (lo + 0.05)
    }

    const DARK_BG: [&str; 2] = ["#151514", "#1e1e1c"];
    const LIGHT_BG: [&str; 2] = ["#fbfaf7", "#ffffff"];

    #[test]
    fn every_colour_keeps_contrast_in_both_themes() {
        for k in &COLORS {
            for bg in DARK_BG {
                assert!(contrast(k.dark, bg) >= 4.5, "{}", k.name);
            }
            assert!(contrast(ON_DARK_THEME, k.dark) >= 4.5, "{}", k.name);
            for bg in LIGHT_BG {
                assert!(contrast(k.light, bg) >= 4.5, "{}", k.name);
            }
            assert!(contrast(ON_LIGHT_THEME, k.light) >= 4.5, "{}", k.name);
        }
    }

    #[test]
    fn palette_has_20_distinct_colours_and_mint_is_app_default() {
        let unique: std::collections::HashSet<&str> = names().into_iter().collect();
        assert_eq!(unique.len(), 20);
        let mint = color(Some("Mięta"));
        assert_eq!((mint.dark, mint.light), ("#8fc2b0", "#2f5d50"));
        assert_eq!(color(Some("nie ma takiego")).name, "Mięta");
        assert_eq!(color(None).name, DEFAULT);
        assert_eq!(run_alt().name, "Malina");
    }

    #[test]
    fn palette_matches_python() {
        let f = fixture();
        let kolory = f["kolory"].as_array().unwrap();
        assert_eq!(kolory.len(), COLORS.len());
        for (k, py) in COLORS.iter().zip(kolory) {
            assert_eq!(k.name, py["name"]);
            assert_eq!(i64::from(k.hue), py["hue"].as_i64().unwrap());
            assert_eq!(k.dark, py["dark"]);
            assert_eq!(k.light, py["light"]);
        }
        assert_eq!(f["domyslny"], DEFAULT);
        let run: Vec<&str> = COLORS
            .iter()
            .filter(|k| clashes_with_run(k))
            .map(|k| k.name)
            .collect();
        let py_run: Vec<&str> = f["z_biegiem"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert_eq!(run, py_run);
    }

    #[test]
    fn icon_colour_matches_python() {
        let f = fixture();
        for case in f["piksele"].as_array().unwrap() {
            let px: Vec<[u8; 4]> = serde_json::from_value(case["piksele"].clone()).unwrap();
            assert_eq!(from_rgb_pixels(&px).name, case["kolor"], "{px:?}");
        }
        for case in f["najblizszy"].as_array().unwrap() {
            let hue = case["hue"].as_f64().unwrap();
            assert_eq!(nearest(hue).name, case["kolor"], "odcień {hue}");
        }
    }

    #[test]
    fn icon_colour_matching() {
        let mut yellow = vec![[250, 215, 0, 255]; 50];
        yellow.extend(vec![[20, 20, 20, 255]; 50]);
        assert_eq!(from_rgb_pixels(&yellow).name, "Banan");
        assert_eq!(
            from_rgb_pixels(&vec![[220, 30, 30, 255]; 60]).name,
            "Czerwień"
        );
        let mut gray = vec![[128, 128, 128, 255]; 100];
        gray.extend(vec![[200, 0, 0, 255]; 3]); // prawie bez koloru
        assert_eq!(from_rgb_pixels(&gray).name, "Mięta");
        assert_eq!(from_rgb_pixels(&[]).name, "Mięta");
    }

    #[test]
    fn yellow_and_orange_profiles_change_run_colour() {
        assert!(clashes_with_run(color(Some("Banan"))));
        assert!(clashes_with_run(color(Some("Pomarańcz"))));
        assert!(!clashes_with_run(color(Some("Błękit"))));
    }
}
