//! Teksty zależne od stanu, które układa rdzeń (docs/kontrakt.md rozdz. 9) - brzmienie 1:1
//! z 0.5.2 (bieznia/gui/ekrany/*.py), liczby z przecinkiem dziesiętnym.

/// Liczba z jednym miejscem po przecinku, bez zbędnego „,0” (2,5 / 12).
pub fn num(value: f64) -> String {
    let text = format!("{value:.1}");
    let text = text.strip_suffix(".0").unwrap_or(&text);
    text.replace('.', ",")
}

/// Odmiana: 1 osiągnięcie, 2-4 osiągnięcia, 5+ osiągnięć.
pub fn plural<'a>(n: usize, one: &'a str, few: &'a str, many: &'a str) -> &'a str {
    if n == 1 {
        one
    } else if (2..=4).contains(&(n % 10)) && !(12..=14).contains(&(n % 100)) {
        few
    } else {
        many
    }
}

/// Podpowiedź pod kafelkami trybów (render_output_hint).
pub fn output_hint(direction: &str, output: &str, walk: &str, run: &str) -> String {
    match direction {
        "gamepad" => {
            "Wychyl lewą gałkę - im dalej, tym szybciej. Skoki zmienisz w Ustawieniach.".into()
        }
        "keyboard" => format!("Trzymaj {walk}, żeby przyspieszyć, {walk} + {run} - bieg."),
        _ if output == "keys" => format!(
            "Gdy idziesz, aplikacja trzyma {walk} w aktywnym oknie gry, a przy szybszym chodzie \
             {run} + {walk}."
        ),
        _ => "Tryb próbny: aplikacja nie naciska klawiszy w grze. Żeby postać szła, w \
              Ustawieniach > Gry i sterowanie ustaw Wyjście na Klawiatura."
            .into(),
    }
}

/// Podtytuł karty sterowania (render_control_subtitle).
pub fn control_subtitle(
    direction: &str,
    belt_control: bool,
    pad_button: &str,
    walk: &str,
    slower: &str,
    faster: &str,
) -> String {
    let mut start = if belt_control {
        "uruchamia i zatrzymuje taśmę".to_string()
    } else {
        "włącza i wyłącza sterowanie".to_string()
    };
    if !pad_button.is_empty() {
        start = format!("(albo pad: {pad_button}) {start}");
    }
    match direction {
        "training" => format!("{start} · {faster} szybciej, {slower} wolniej"),
        "keyboard" => format!("{start} · {walk} przyspiesza"),
        "gamepad" => format!("{start} · gałka ustawia tempo"),
        _ => format!("{start} · działa też w grze"),
    }
}

/// Opis stanu pada (pad_status_text).
pub fn pad_status(name: Option<&str>, deflection: f64, step: usize) -> String {
    match name {
        None => "Nie widzę pada. Podłącz DualSense albo pada Xbox (kablem albo Bluetooth).".into(),
        Some(name) => format!(
            "{name}: wychylenie {}%, skok {}.",
            (deflection * 100.0).round(),
            step + 1
        ),
    }
}

/// Komunikat za krótkiego opisu zgłoszenia (show_report_error).
pub fn report_too_short(length: usize) -> String {
    let min = walklevel_core::reports::REPORT_MIN_CHARS;
    if length == 0 {
        return "Opisz, co się stało - wystarczy jedno zdanie.".into();
    }
    let missing = min.saturating_sub(length);
    let word = if missing == 1 {
        "znak"
    } else if missing < 5 {
        "znaki"
    } else {
        "znaków"
    };
    format!("Opis jest za krótki - dopisz jeszcze {missing} {word}, np. co poszło nie tak.")
}

/// Nazwa rzadkości do popupu (odznaki.RARITY_NAMES).
pub fn rarity_name(rarity: &str) -> &'static str {
    match rarity {
        "rzadkie" => "Rzadkie",
        "epickie" => "Epickie",
        "heroiczne" => "Heroiczne",
        "legendarne" => "Legendarne",
        _ => "Pospolite",
    }
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

/// Kilometry w popupach (poziomy.fmt_km): od 100 km całe z odstępami, niżej jedno miejsce
/// bez „,0”.
pub fn km(value: f64) -> String {
    if value >= 100.0 {
        int(value)
    } else {
        num(value)
    }
}

/// Liczba całkowita z odstępami tysięcy (poziomy.fmt_int).
pub fn int(value: f64) -> String {
    let n = value.round() as i64;
    let digits = n.abs().to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(' ');
        }
        out.push(c);
    }
    if n < 0 { format!("-{out}") } else { out }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_use_comma_without_trailing_zero() {
        assert_eq!(num(2.5), "2,5");
        assert_eq!(num(12.0), "12");
        assert_eq!(num(0.04), "0");
        assert_eq!(int(12345.0), "12 345");
    }

    #[test]
    fn polish_plurals() {
        assert_eq!(plural(1, "a", "b", "c"), "a");
        assert_eq!(plural(3, "a", "b", "c"), "b");
        assert_eq!(plural(12, "a", "b", "c"), "c");
        assert_eq!(plural(22, "a", "b", "c"), "b");
        assert_eq!(
            report_too_short(3),
            "Opis jest za krótki - dopisz jeszcze 2 znaki, np. co poszło nie tak."
        );
    }
}
