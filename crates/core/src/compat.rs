//! Drobne odpowiedniki wbudowanych funkcji Pythona, żeby wyniki były bit w bit takie same.

use serde_json::Value;

/// `round(x, 1)` z Pythona: zaokrąglenie dokładnej wartości do 0,1 (remis do parzystej).
/// Zwykłe `(x * 10).round() / 10` daje inny wynik np. dla 0,15 (to naprawdę 0,1499...).
pub fn round_tenth(x: f64) -> f64 {
    format!("{x:.1}").parse().unwrap_or(x)
}

/// `sum()` z Pythona 3.12+ dla liczb zmiennoprzecinkowych (kompensacja Neumaiera).
pub(crate) fn fsum<'a>(values: impl IntoIterator<Item = &'a f64>) -> f64 {
    let mut total = 0.0_f64;
    let mut c = 0.0_f64;
    for &x in values {
        let t = total + x;
        if total.abs() >= x.abs() {
            c += (total - t) + x;
        } else {
            c += (x - t) + total;
        }
        total = t;
    }
    if c != 0.0 && c.is_finite() {
        total + c
    } else {
        total
    }
}

/// `float(v)` z Pythona dla wartości z JSON (liczba, bool, napis); `None` = błąd.
pub(crate) fn py_float(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n) => n.as_f64(),
        Value::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
        Value::String(s) => s.trim().parse().ok(),
        _ => None,
    }
}

/// `bool(v)` z Pythona dla wartości z JSON.
pub(crate) fn truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().is_some_and(|f| f != 0.0),
        Value::String(s) => !s.is_empty(),
        Value::Array(a) => !a.is_empty(),
        Value::Object(o) => !o.is_empty(),
    }
}

/// Biały znak jak `str.isspace()` w Pythonie: białe znaki Unicode oraz separatory
/// U+001C..U+001F (Python liczy je jako białe, Rust nie).
pub(crate) fn is_space(c: char) -> bool {
    c.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&c)
}

/// `str.strip()` z Pythona.
pub(crate) fn strip(text: &str) -> &str {
    text.trim_matches(is_space)
}

/// `urllib.parse.quote(text, safe="")` z Pythona (fragment ścieżki, temat e-maila).
pub(crate) fn quote(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for &b in text.as_bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b'-' | b'~') {
            out.push(char::from(b));
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// `int(v)` z Pythona dla wartości z JSON (ułamek obcinany do zera); `None` = błąd.
pub(crate) fn py_int(v: &Value) -> Option<i64> {
    match v {
        Value::Number(n) => match n.as_i64() {
            Some(i) => Some(i),
            None => n
                .as_f64()
                .filter(|f| f.is_finite())
                .map(|f| f.trunc() as i64),
        },
        Value::Bool(b) => Some(i64::from(*b)),
        Value::String(s) => s.trim().parse().ok(),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_tenth_matches_python() {
        // wartości policzone w Pythonie: round(x, 1)
        assert_eq!(round_tenth(0.15), 0.1);
        assert_eq!(round_tenth(0.25), 0.2);
        assert_eq!(round_tenth(0.35), 0.3);
        assert_eq!(round_tenth(2.45), 2.5);
        assert_eq!(round_tenth(1.05), 1.1);
        assert_eq!(round_tenth(1.04), 1.0);
        assert_eq!(round_tenth(-0.05), -0.1);
        assert!(round_tenth(f64::INFINITY).is_infinite());
    }

    #[test]
    fn fsum_compensates_like_python() {
        // sum([0.1] * 10) w Pythonie 3.12+ to dokładnie 1.0
        assert_eq!(fsum(&[0.1; 10]), 1.0);
        assert_eq!(fsum(&[]), 0.0);
    }

    #[test]
    fn int_and_float_conversions() {
        assert_eq!(py_int(&serde_json::json!(50.7)), Some(50));
        assert_eq!(py_int(&serde_json::json!(-0.9)), Some(0));
        assert_eq!(py_int(&serde_json::json!("40")), Some(40));
        assert_eq!(py_int(&serde_json::json!("4.0")), None);
        assert_eq!(py_float(&serde_json::json!(" 2.5 ")), Some(2.5));
        assert_eq!(py_float(&serde_json::json!(true)), Some(1.0));
        assert_eq!(py_float(&serde_json::json!(null)), None);
    }
}
