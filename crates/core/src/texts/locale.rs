//! Numbers and plural categories of a language with ICU4X (CLDR data compiled in), the Rust
//! counterpart of `Intl.NumberFormat` / `Intl.PluralRules` in src/lib/locale-format.ts. The
//! test `cldr_samples_match_intl` compares both on samples written by `npm run texts`.

use std::fmt;
use std::str::FromStr;

use icu_decimal::input::Decimal;
use icu_decimal::options::{DecimalFormatterOptions, GroupingStrategy};
use icu_decimal::{DecimalFormatter, parts};
use icu_locale_core::Locale;
use icu_plurals::{PluralCategory, PluralRules};
use serde::{Deserialize, Serialize};
use writeable::{Part, PartsWrite, Writeable};

/// Exceptions to CLDR number formatting (`number` in locales/languages.json).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NumberStyle {
    /// "always": group 4-digit numbers too
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grouping: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group_separator: Option<String>,
}

pub struct LocaleFormat {
    pub code: String,
    numbers: DecimalFormatter,
    plurals: PluralRules,
    decimal: String,
    group: String,
    style: NumberStyle,
}

impl fmt::Debug for LocaleFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LocaleFormat")
            .field("code", &self.code)
            .finish()
    }
}

/// Collects a formatted number: replaces the group separator and, when ICU4X does not group
/// (its `Always` is the same as `Auto`, so CLDR pl leaves 4 digits ungrouped), groups the
/// integer digits by three.
#[derive(Default)]
struct Sink<'a> {
    out: String,
    group: Option<&'a str>,
    /// group the integer part by three with this separator
    regroup: Option<&'a str>,
    in_group: bool,
    decimal: String,
    in_decimal: bool,
    in_integer: bool,
    integer: Vec<char>,
}

impl fmt::Write for Sink<'_> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        if self.in_decimal {
            self.decimal.push_str(s);
        }
        if self.in_integer && self.regroup.is_some() {
            if !self.in_group {
                self.integer.extend(s.chars());
            }
            return Ok(());
        }
        match (self.in_group, self.group) {
            (true, Some(g)) => self.out.push_str(g),
            _ => self.out.push_str(s),
        }
        Ok(())
    }
}

impl PartsWrite for Sink<'_> {
    type SubPartsWrite = Self;
    fn with_part(
        &mut self,
        part: Part,
        mut f: impl FnMut(&mut Self) -> fmt::Result,
    ) -> fmt::Result {
        let (group, decimal, integer) = (self.in_group, self.in_decimal, self.in_integer);
        self.in_group |= part == parts::GROUP;
        self.in_decimal |= part == parts::DECIMAL;
        self.in_integer |= part == parts::INTEGER;
        let r = f(self);
        if !integer
            && self.in_integer
            && let Some(sep) = self.regroup
        {
            let digits = std::mem::take(&mut self.integer);
            for (i, c) in digits.iter().enumerate() {
                if i > 0 && (digits.len() - i).is_multiple_of(3) {
                    self.out.push_str(sep);
                }
                self.out.push(*c);
            }
        }
        self.in_group = group;
        self.in_decimal = decimal;
        self.in_integer = integer;
        r
    }
}

impl LocaleFormat {
    /// Formats for a BCP 47 tag; an unknown tag gets CLDR root data (like `Intl`).
    pub fn new(code: &str, style: NumberStyle) -> Self {
        let locale = Locale::try_from_str(code).unwrap_or(Locale::UNKNOWN);
        let mut options = DecimalFormatterOptions::default();
        if style.grouping.as_deref() == Some("always") {
            options.grouping_strategy = Some(GroupingStrategy::Never);
        }
        let numbers = DecimalFormatter::try_new((&locale).into(), options)
            .or_else(|_| DecimalFormatter::try_new(Default::default(), options))
            .expect("ICU4X compiled decimal data");
        let plurals = PluralRules::try_new_cardinal((&locale).into())
            .or_else(|_| PluralRules::try_new_cardinal(Default::default()))
            .expect("ICU4X compiled plural data");
        let mut probe = Sink::default();
        let _ = numbers
            .format(&Decimal::from_str("1.5").expect("decimal"))
            .write_to_parts(&mut probe);
        let decimal = if probe.decimal.is_empty() {
            ".".to_string()
        } else {
            probe.decimal
        };
        let mut format = Self {
            code: code.to_string(),
            numbers,
            plurals,
            decimal,
            group: String::new(),
            style,
        };
        format.group = format.style.group_separator.clone().unwrap_or_else(|| {
            let mut probe = Sink::default();
            let always = DecimalFormatter::try_new((&locale).into(), Default::default());
            if let Ok(f) = always {
                let _ = f
                    .format(&Decimal::from_str("1000000").expect("decimal"))
                    .write_to_parts(&mut probe);
            }
            probe
                .out
                .chars()
                .find(|c| !c.is_numeric())
                .map(String::from)
                .unwrap_or_default()
        });
        format
    }

    /// `places` digits after the decimal separator, rounded; -0.04 with one place is "0.0".
    pub fn number(&self, value: f64, places: usize) -> String {
        if !value.is_finite() {
            return "–".to_string();
        }
        let mut text = format!("{value:.places$}");
        if text.starts_with('-') && text[1..].chars().all(|c| c == '0' || c == '.') {
            text.remove(0);
        }
        let Ok(decimal) = Decimal::from_str(&text) else {
            return text;
        };
        let always = self.style.grouping.as_deref() == Some("always");
        let mut sink = Sink {
            group: self.style.group_separator.as_deref(),
            regroup: always.then_some(self.group.as_str()),
            ..Sink::default()
        };
        let _ = self.numbers.format(&decimal).write_to_parts(&mut sink);
        sink.out
    }

    /// A number written by [`LocaleFormat::number`] back as a decimal with its visible digits.
    pub fn parse(&self, text: &str) -> Option<Decimal> {
        let mut clean = String::new();
        let mut rest = text.trim();
        while let Some(c) = rest.chars().next() {
            if !self.decimal.is_empty() && rest.starts_with(self.decimal.as_str()) {
                clean.push('.');
                rest = &rest[self.decimal.len()..];
                continue;
            }
            if !self.group.is_empty() && rest.starts_with(self.group.as_str()) {
                rest = &rest[self.group.len()..];
                continue;
            }
            match c {
                c if c.is_whitespace() || c == '\u{202f}' || c == '\u{a0}' => {}
                '\u{2212}' => clean.push('-'),
                c => clean.push(c),
            }
            rest = &rest[c.len_utf8()..];
        }
        let valid = clean.strip_prefix('-').unwrap_or(&clean);
        let mut halves = valid.split('.');
        let digits = |s: Option<&str>| {
            s.is_some_and(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()))
        };
        let first = halves.next();
        let second = halves.next();
        if !digits(first) || (second.is_some() && !digits(second)) || halves.next().is_some() {
            return None;
        }
        Decimal::from_str(&clean).ok()
    }

    /// CLDR cardinal category ("one", "few", ...) of a decimal with its visible digits.
    pub fn plural(&self, value: &Decimal) -> &'static str {
        category_name(self.plurals.category_for(value))
    }
}

fn category_name(c: PluralCategory) -> &'static str {
    match c {
        PluralCategory::Zero => "zero",
        PluralCategory::One => "one",
        PluralCategory::Two => "two",
        PluralCategory::Few => "few",
        PluralCategory::Many => "many",
        PluralCategory::Other => "other",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Same CLDR answers as Intl in Node (samples from `npm run texts`).
    #[test]
    fn cldr_samples_match_intl() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/texts/cldr-samples.json");
        let samples: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        let manifest = super::super::packs::built_in();
        let mut checked = 0;
        for (code, data) in samples.as_object().unwrap() {
            let style = manifest
                .language(code)
                .and_then(|l| l.number.clone())
                .unwrap_or_default();
            let f = LocaleFormat::new(code, style);
            for s in data["plural"].as_array().unwrap() {
                let n = s[0].as_str().unwrap();
                let got = f.plural(&Decimal::from_str(n).unwrap());
                assert_eq!(got, s[1].as_str().unwrap(), "{code} plural {n}");
                checked += 1;
            }
            for s in data["number"].as_array().unwrap() {
                let (v, places) = (s[0].as_f64().unwrap(), s[1].as_u64().unwrap() as usize);
                assert_eq!(
                    f.number(v, places),
                    s[2].as_str().unwrap(),
                    "{code} number {v}"
                );
                checked += 1;
            }
        }
        assert!(checked > 100);
    }

    #[test]
    fn parses_own_numbers() {
        let pl = LocaleFormat::new(
            "pl",
            NumberStyle {
                grouping: Some("always".into()),
                group_separator: Some("\u{202f}".into()),
            },
        );
        assert_eq!(pl.parse("12\u{202f}345").unwrap().to_string(), "12345");
        assert_eq!(pl.parse("2,5").unwrap().to_string(), "2.5");
        assert_eq!(pl.parse("6,0").unwrap().to_string(), "6.0");
        assert!(pl.parse("Ola").is_none());
        let en = LocaleFormat::new("en", NumberStyle::default());
        assert_eq!(en.parse("12,345").unwrap().to_string(), "12345");
        assert_eq!(en.parse("2.5").unwrap().to_string(), "2.5");
    }
}
