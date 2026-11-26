//! Teksty dla użytkownika. Jedyne źródło: docs/teksty.md -> src/lib/teksty-slownik.ts
//! (`npm run teksty`), ten sam plik czyta interfejs. Rdzeń bierze z niego teksty po kluczu:
//! `t("glowny.podpowiedz.trening", &[("klawisz_szybciej", "F7"), ("klawisz_wolniej", "F6")])`.
//! Zmienne i odmiana jak w src/lib/teksty.ts: `{nazwa}`, `{nazwa:f1/f2/f5}`.

use std::collections::HashMap;
use std::sync::OnceLock;

static SLOWNIK_TS: &str = include_str!("../../../src/lib/teksty-slownik.ts");

fn slownik() -> &'static HashMap<String, String> {
    static MAPA: OnceLock<HashMap<String, String>> = OnceLock::new();
    MAPA.get_or_init(|| {
        let start = SLOWNIK_TS.find('{').map_or(0, |i| i + 1);
        let end = SLOWNIK_TS.rfind('}').unwrap_or(SLOWNIK_TS.len());
        let body = SLOWNIK_TS[start..end].trim().trim_end_matches(',');
        serde_json::from_str(&format!("{{{body}}}")).expect("src/lib/teksty-slownik.ts")
    })
}

/// Czy słownik ma taki klucz.
pub fn has(key: &str) -> bool {
    slownik().contains_key(key)
}

/// Tekst ze słownika z podstawionymi zmiennymi. Brak zmiennej zostawia `{nazwa}` (widać od
/// razu na ekranie), brak klucza - sam klucz (w testach to błąd).
pub fn t(key: &str, vars: &[(&str, &str)]) -> String {
    let Some(pattern) = slownik().get(key) else {
        debug_assert!(false, "Brak tekstu „{key}” w docs/teksty.md");
        return key.to_string();
    };
    let mut out = String::with_capacity(pattern.len() + 16);
    let mut rest = pattern.as_str();
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        let Some(close) = after.find('}') else {
            out.push_str(&rest[open..]);
            rest = "";
            break;
        };
        let inner = &after[..close];
        let whole = &rest[open..open + close + 2];
        out.push_str(&substitute(inner, vars).unwrap_or_else(|| whole.to_string()));
        rest = &after[close + 1..];
    }
    out.push_str(rest);
    out
}

fn substitute(inner: &str, vars: &[(&str, &str)]) -> Option<String> {
    let value = |name: &str| vars.iter().find(|(k, _)| *k == name).map(|(_, v)| *v);
    match inner.split_once(':') {
        Some((name, forms)) => {
            let f: Vec<&str> = forms.split('/').collect();
            let [f1, f2, f5] = f.as_slice() else {
                return None;
            };
            let n = number(value(name)?)?;
            Some(forms_for(n, f1, f2, f5).to_string())
        }
        None => value(inner).map(str::to_string),
    }
}

/// Wartość liczbowa już sformatowanej liczby („12 345”, „2,5”).
fn number(text: &str) -> Option<f64> {
    let clean: String = text
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '\u{202f}' && *c != '\u{a0}')
        .map(|c| if c == ',' { '.' } else { c })
        .collect();
    clean.parse().ok()
}

/// Odmiana: 1 -> f1; końcówka 2-4 (bez 12-14) -> f2; reszta -> f5; ułamki -> f2.
pub fn forms_for<'a>(n: f64, f1: &'a str, f2: &'a str, f5: &'a str) -> &'a str {
    if n.fract() != 0.0 {
        return f2;
    }
    let a = n.abs() as u64;
    if a == 1 {
        return f1;
    }
    let (d, s) = (a % 10, a % 100);
    if (2..=4).contains(&d) && !(12..=14).contains(&s) {
        f2
    } else {
        f5
    }
}

/// Liczba po polsku (src/lib/format.ts `liczba`): przecinek dziesiętny, tysiące rozdzielone
/// wąską twardą spacją U+202F.
pub fn number_pl(value: f64, places: usize) -> String {
    let text = format!("{:.*}", places, value.abs());
    let (int, frac) = text.split_once('.').unwrap_or((&text, ""));
    let mut out = String::new();
    if value < 0.0 && text.chars().any(|c| c.is_ascii_digit() && c != '0') {
        out.push('-');
    }
    for (i, c) in int.chars().enumerate() {
        if i > 0 && (int.len() - i).is_multiple_of(3) {
            out.push('\u{202f}');
        }
        out.push(c);
    }
    if !frac.is_empty() {
        out.push(',');
        out.push_str(frac);
    }
    out
}

/// `{kmh}` zawsze z jednym miejscem po przecinku (docs/teksty.md 0.4): „2,5”, „6,0”.
pub fn kmh(value: f64) -> String {
    number_pl(value, 1)
}

/// Brzmienie 0.6.0 dla komunikatu z 0.5.2 (fixtury z Pythona w testach zgodności): wiersz
/// docs/teksty.md, którego „Obecny tekst” pasuje do `old` (klucz z jednym z prefiksów), daje
/// „Nowy tekst” z tymi samymi zmiennymi. Bez dopasowania - `old` bez zmian. Tylko dla testów.
#[doc(hidden)]
pub fn from_052(old: &str, prefixes: &[&str]) -> String {
    let md = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/teksty.md"),
    )
    .expect("docs/teksty.md");
    // najdokładniejszy wiersz: najwięcej stałego tekstu (nie „{rola}: {klawisz}”)
    let mut best: Option<(usize, String)> = None;
    for line in md.lines().filter(|l| l.starts_with("| `")) {
        let cells: Vec<&str> = line.split(" | ").collect();
        let (Some(key), Some(before)) = (cells.first(), cells.get(1)) else {
            continue;
        };
        let key = key.trim_start_matches("| ").trim_matches('`');
        if !prefixes.iter().any(|p| key.starts_with(p)) || !has(key) {
            continue;
        }
        // „Obecny tekst” jako wzorzec: {zmienna} i {f1/f2/f5} pasują do dowolnego tekstu
        let mut names = Vec::new();
        let mut pattern = String::from("^");
        let mut rest = *before;
        while let Some(open) = rest.find('{') {
            pattern.push_str(&regex::escape(&rest[..open]));
            let Some(close) = rest[open..].find('}') else {
                break;
            };
            names.push(rest[open + 1..open + close].to_string());
            pattern.push_str("(.+?)");
            rest = &rest[open + close + 1..];
        }
        pattern.push_str(&regex::escape(rest));
        pattern.push('$');
        let Some(caps) = regex::Regex::new(&pattern)
            .ok()
            .and_then(|re| re.captures(old))
        else {
            continue;
        };
        let values: Vec<(String, String)> = names
            .iter()
            .enumerate()
            .map(|(i, n)| {
                // powód (np. „F6 to skrót symulatora”) to też tekst z 0.5.2
                let v = &caps[i + 1];
                let v = if n == "powod" {
                    from_052(v, prefixes)
                } else {
                    v.to_string()
                };
                (n.clone(), v)
            })
            .collect();
        let vars: Vec<(&str, &str)> = values
            .iter()
            .map(|(n, v)| (n.as_str(), v.as_str()))
            .collect();
        let fixed = before.len() - names.iter().map(|n| n.len() + 2).sum::<usize>();
        if best.as_ref().is_none_or(|(f, _)| fixed > *f) {
            best = Some((fixed, t(key, &vars)));
        }
    }
    best.map_or_else(|| old.to_string(), |(_, text)| text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn substitutes_variables_and_plurals() {
        assert_eq!(
            t("glowny.osoby.dzis", &[("kroki", "1\u{202f}234")]),
            "dziś 1\u{202f}234 kroki"
        );
        assert_eq!(t("glowny.osoby.dzis", &[("kroki", "1")]), "dziś 1 krok");
        assert_eq!(t("glowny.osoby.dzis", &[("kroki", "12")]), "dziś 12 kroków");
        // brak zmiennej zostaje widoczny
        assert_eq!(
            t("glowny.osoby.dzis", &[]),
            "dziś {kroki} {kroki:krok/kroki/kroków}"
        );
    }

    #[test]
    fn polish_numbers() {
        assert_eq!(number_pl(12345.0, 0), "12\u{202f}345");
        assert_eq!(number_pl(1234.0, 0), "1\u{202f}234");
        assert_eq!(number_pl(999.0, 0), "999");
        assert_eq!(kmh(6.0), "6,0");
        assert_eq!(kmh(2.54), "2,5");
        assert_eq!(number_pl(-0.04, 1), "0,0");
    }

    /// Każdy klucz użyty w Rust (`t("...")`) jest w słowniku.
    #[test]
    fn every_used_key_exists() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut missing = Vec::new();
        let mut used = 0;
        for dir in ["crates/core/src", "src-tauri/src"] {
            let mut stack = vec![root.join(dir)];
            while let Some(d) = stack.pop() {
                for e in std::fs::read_dir(&d).unwrap().flatten() {
                    let p = e.path();
                    if p.is_dir() {
                        stack.push(p);
                        continue;
                    }
                    if p.extension().is_none_or(|x| x != "rs") {
                        continue;
                    }
                    let src = std::fs::read_to_string(&p).unwrap();
                    for (i, _) in src.match_indices("t(\"") {
                        let before = src[..i].chars().next_back().unwrap_or(' ');
                        if before.is_alphanumeric() || before == '_' {
                            continue;
                        }
                        let rest = &src[i + 3..];
                        let key = &rest[..rest.find('"').unwrap()];
                        let key_chars = key.chars().all(|c| {
                            c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '.'
                        });
                        if !key_chars
                            || !key.contains('.')
                            || !key.starts_with(|c: char| c.is_ascii_lowercase())
                        {
                            continue;
                        }
                        used += 1;
                        if !has(key) {
                            missing.push(format!("{}: {key}", p.display()));
                        }
                    }
                }
            }
        }
        assert!(used > 20, "za mało kluczy: {used}");
        assert!(missing.is_empty(), "brak w docs/teksty.md: {missing:#?}");
    }
}
