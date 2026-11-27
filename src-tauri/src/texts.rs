//! Teksty zależne od stanu, które układa rdzeń (docs/contract.md rozdz. 9). Brzmienie ze
//! słownika docs/texts.md (walklevel_core::texts::t), liczby z przecinkiem dziesiętnym.

pub use walklevel_core::texts::{kmh, t};

/// Liczba całkowita z odstępami tysięcy (12 345).
pub fn int(value: f64) -> String {
    walklevel_core::texts::number_pl(value.round(), 0)
}

/// Kilometry w powiadomieniach: od 100 km całe z odstępami, niżej jedno miejsce bez „,0”.
pub fn km(value: f64) -> String {
    if value >= 100.0 {
        int(value)
    } else {
        let text = walklevel_core::texts::number_pl(value, 1);
        text.strip_suffix(",0").map_or(text.clone(), str::to_string)
    }
}

/// Karta stanu bieżni (docs/texts.md 3): tytuł, podtytuł i napis przycisku stanu
/// `glowny.status.<state>.*`. Brakujący podtytuł albo napis (czekam, ponawiam) - jak w stanie
/// „połączono”.
pub fn status_texts(state: &str, vars: &[(&str, &str)]) -> (String, String, String) {
    let part = |name: &str| {
        let key = format!("glowny.status.{state}.{name}");
        let key = if walklevel_core::texts::has(&key) {
            key
        } else {
            format!("glowny.status.polaczono.{name}")
        };
        t(&key, vars)
    };
    (part("tytul"), part("podtytul"), part("akcja"))
}

/// Podpowiedź pod kafelkami trybów (docs/texts.md 8.2, glowny.podpowiedz.*).
pub fn output_hint(
    direction: &str,
    output: &str,
    key_stop: bool,
    walk: &str,
    run: &str,
    slower: &str,
    faster: &str,
) -> String {
    let keys = [
        ("klawisz_chodu", walk),
        ("klawisz_biegu", run),
        ("klawisz_wolniej", slower),
        ("klawisz_szybciej", faster),
    ];
    let key = match direction {
        "gamepad" => "glowny.podpowiedz.pad",
        "keyboard" if key_stop => "glowny.podpowiedz.klawisze_stop",
        "keyboard" => "glowny.podpowiedz.klawisze",
        "training" => "glowny.podpowiedz.trening",
        _ if output == "keys" => "glowny.podpowiedz.chodzenie",
        _ => "glowny.podpowiedz.chodzenie_proba",
    };
    t(key, &keys)
}

/// Podtytuł karty sterowania (docs/texts.md 8, glowny.sterowanie.opis.*).
pub fn control_subtitle(
    direction: &str,
    belt_control: bool,
    pad_button: &str,
    walk: &str,
    slower: &str,
    faster: &str,
) -> String {
    let mut start = t(
        if belt_control {
            "glowny.sterowanie.opis.tasma"
        } else {
            "glowny.sterowanie.opis.bez_tasmy"
        },
        &[],
    );
    if !pad_button.is_empty() {
        start = t(
            "glowny.sterowanie.opis.z_padem",
            &[("opis", &start), ("przycisk_pada", pad_button)],
        );
    }
    let key = match direction {
        "training" => "glowny.sterowanie.opis.trening",
        "keyboard" => "glowny.sterowanie.opis.klawisze",
        "gamepad" => "glowny.sterowanie.opis.pad",
        _ => "glowny.sterowanie.opis.chodzenie",
    };
    t(
        key,
        &[
            ("opis", &start),
            ("klawisz_chodu", walk),
            ("klawisz_wolniej", slower),
            ("klawisz_szybciej", faster),
        ],
    )
}

/// Opis stanu pada (ustawienia.pad.brak / ustawienia.pad.stan).
pub fn pad_status(name: Option<&str>, deflection: f64, step: usize) -> String {
    match name {
        None => t("ustawienia.pad.brak", &[]),
        Some(name) => t(
            "ustawienia.pad.stan",
            &[
                ("pad", name),
                ("procent", &format!("{}", (deflection * 100.0).round())),
                ("stopien", &(step + 1).to_string()),
            ],
        ),
    }
}

/// Nazwa rzadkości do powiadomienia (rzadkosc.*).
pub fn rarity_name(rarity: &str) -> String {
    let key = match rarity {
        "rzadkie" | "epickie" | "heroiczne" | "legendarne" => rarity,
        _ => "pospolite",
    };
    t(&format!("rzadkosc.{key}"), &[])
}

/// Kolor rzadkości (odznaki.RARITY_COLORS).
pub fn rarity_color(rarity: &str) -> &'static str {
    match rarity {
        "rzadkie" => "#4aa3ff",
        "epickie" => "#b26bff",
        "heroiczne" => "#ff5470",
        "legendarne" => "#ffa133",
        _ => "#9aa5b1",
    }
}

/// Rzadkość odznaki poziomu (odznaki.rank_rarity).
pub fn rank_rarity(level: u32) -> &'static str {
    match level {
        0..10 => "pospolite",
        10..25 => "rzadkie",
        25..50 => "epickie",
        50..80 => "heroiczne",
        _ => "legendarne",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEYS: [&str; 4] = ["W", "Shift", "F6", "F7"];

    fn hint(direction: &str, output: &str) -> String {
        output_hint(direction, output, false, KEYS[0], KEYS[1], KEYS[2], KEYS[3])
    }

    #[test]
    fn output_hint_for_every_mode() {
        assert_eq!(
            hint("training", "dry"),
            "Kliknij poziom albo użyj F7 i F6. Gra nie dostaje klawiszy."
        );
        assert_eq!(hint("training", "keys"), hint("training", "dry"));
        assert_eq!(
            hint("treadmill", "keys"),
            "Idź po bieżni, a postać pójdzie razem z Tobą: gra dostaje W, a przy szybszym tempie \
             Shift + W. Okno gry musi być na wierzchu."
        );
        assert_eq!(
            hint("treadmill", "dry"),
            "Tryb próbny: gra nie dostaje klawiszy, więc postać stoi. Żeby szła, włącz \
             Ustawienia > Gry i sterowanie > Sterowanie > Klawisze w grze."
        );
        assert_eq!(
            hint("keyboard", "dry"),
            "Trzymaj W, a taśma przyspieszy. W + Shift to bieg. Puść klawisz, a taśma zwolni."
        );
        assert_eq!(
            hint("gamepad", "dry"),
            "Wychyl lewą gałkę: im dalej, tym szybciej jedzie taśma. Stopnie ustawisz w \
             Ustawienia > Gry i sterowanie > Sterowanie."
        );
        for d in ["training", "treadmill", "keyboard", "gamepad"] {
            assert!(!hint(d, "dry").contains('{'), "{d}");
        }
        assert_eq!(
            output_hint("keyboard", "dry", true, KEYS[0], KEYS[1], KEYS[2], KEYS[3]),
            "Trzymaj W, a taśma ruszy. Puść, a stanie."
        );
    }

    #[test]
    fn control_subtitle_with_pad_and_modes() {
        assert_eq!(
            control_subtitle("training", true, "Panel dotykowy", "W", "F6", "F7"),
            "uruchamia i zatrzymuje taśmę · na padzie: Panel dotykowy · F7 szybciej, F6 wolniej"
        );
        assert_eq!(
            control_subtitle("keyboard", false, "", "W", "F6", "F7"),
            "włącza i wyłącza sterowanie · W przyspiesza"
        );
        assert_eq!(
            control_subtitle("gamepad", false, "", "W", "F6", "F7"),
            "włącza i wyłącza sterowanie · gałka ustawia tempo"
        );
        assert_eq!(
            control_subtitle("treadmill", false, "", "W", "F6", "F7"),
            "włącza i wyłącza sterowanie · działa też w grze"
        );
    }

    #[test]
    fn status_card_for_every_state() {
        let vars = [
            ("model", "KingSmith X21"),
            ("klawisz_wolniej", "F6"),
            ("klawisz_szybciej", "F7"),
            ("min_kmh", "0,5"),
            ("max_kmh", "6,0"),
        ];
        let states = [
            "brak",
            "rozlaczona",
            "skanowanie",
            "szukanie",
            "laczenie",
            "polaczono",
            "czekam",
            "ponawiam",
            "bt_wylaczony",
            "nieobslugiwane",
            "symulator",
        ];
        for state in states {
            let (title, sub, label) = status_texts(state, &vars);
            for text in [&title, &sub, &label] {
                assert!(!text.is_empty() && !text.contains('{'), "{state}: {text}");
            }
        }
        assert_eq!(
            status_texts("polaczono", &vars),
            (
                "Połączono z KingSmith X21".to_string(),
                "Taśma 0,5–6,0 km/h".to_string(),
                "Rozłącz".to_string()
            )
        );
        assert_eq!(status_texts("rozlaczona", &vars).2, "Połącz");
        assert_eq!(status_texts("skanowanie", &vars).2, "Anuluj");
        assert_eq!(status_texts("ponawiam", &vars).2, "Przerwij");
        assert_eq!(
            status_texts("symulator", &vars).1,
            "F7 przyspiesza, F6 zwalnia. Sesje nie trafiają na konto online."
        );
    }

    #[test]
    fn pad_status_uses_steps() {
        assert_eq!(
            pad_status(Some("DualSense"), 0.42, 1),
            "DualSense: wychylenie 42%, stopień 2."
        );
        assert!(pad_status(None, 0.0, 0).starts_with("Nie widać pada."));
    }

    #[test]
    fn numbers() {
        assert_eq!(km(2.5), "2,5");
        assert_eq!(km(12.0), "12");
        assert_eq!(km(1234.4), "1\u{202f}234");
        assert_eq!(int(12345.0), "12\u{202f}345");
        assert_eq!(rarity_name("epickie"), "Epickie");
        assert_eq!(rarity_name("?"), "Pospolite");
    }
}
