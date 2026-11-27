//! ICU MessageFormat subset of the language packs, the same as src/lib/message-format.ts:
//! text, `{name}`, `{name, plural, [offset:n] =0 {...} one {...} other {...}}` with `#`, and
//! `{name, select, key {...} other {...}}`. Apostrophes follow ICU (DOUBLE_OPTIONAL).

use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq)]
pub enum Part {
    Text(String),
    Pound,
    Arg(Argument),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Argument {
    pub name: String,
    pub kind: Kind,
    pub offset: u32,
    pub options: Vec<(String, Vec<Part>)>,
    /// source text, shown when the value is missing
    pub raw: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Simple,
    Plural,
    Select,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{what} at {at}")]
pub struct MessageError {
    pub what: String,
    pub at: usize,
}

struct Parser<'a> {
    text: &'a str,
    i: usize,
}

impl Parser<'_> {
    fn fail<T>(&self, what: &str) -> Result<T, MessageError> {
        Err(MessageError {
            what: what.to_string(),
            at: self.i,
        })
    }

    fn peek(&self) -> Option<char> {
        self.text[self.i..].chars().next()
    }

    fn peek2(&self) -> Option<char> {
        let mut it = self.text[self.i..].chars();
        it.next();
        it.next()
    }

    fn bump(&mut self) -> Option<char> {
        let c = self.peek()?;
        self.i += c.len_utf8();
        Some(c)
    }

    fn spaces(&mut self) {
        while self.peek().is_some_and(char::is_whitespace) {
            self.bump();
        }
    }

    fn word(&mut self) -> Result<String, MessageError> {
        let start = self.i;
        while self
            .peek()
            .is_some_and(|c| !c.is_whitespace() && !"{},#'".contains(c))
        {
            self.bump();
        }
        if self.i == start {
            return self.fail("name expected");
        }
        Ok(self.text[start..self.i].to_string())
    }

    fn message(&mut self, in_plural: bool, nested: bool) -> Result<Vec<Part>, MessageError> {
        let mut parts = Vec::new();
        let mut text = String::new();
        while let Some(c) = self.peek() {
            match c {
                '\'' => {
                    let next = self.peek2();
                    self.bump();
                    if next == Some('\'') {
                        self.bump();
                        text.push('\'');
                    } else if matches!(next, Some('{' | '}')) || (in_plural && next == Some('#')) {
                        while let Some(q) = self.bump() {
                            if q == '\'' {
                                if self.peek() == Some('\'') {
                                    self.bump();
                                    text.push('\'');
                                    continue;
                                }
                                break;
                            }
                            text.push(q);
                        }
                    } else {
                        text.push('\'');
                    }
                }
                '{' => {
                    if !text.is_empty() {
                        parts.push(Part::Text(std::mem::take(&mut text)));
                    }
                    parts.push(Part::Arg(self.argument()?));
                }
                '}' => {
                    if !nested {
                        return self.fail("unmatched }");
                    }
                    break;
                }
                '#' if in_plural => {
                    if !text.is_empty() {
                        parts.push(Part::Text(std::mem::take(&mut text)));
                    }
                    self.bump();
                    parts.push(Part::Pound);
                }
                c => {
                    self.bump();
                    text.push(c);
                }
            }
        }
        if !text.is_empty() {
            parts.push(Part::Text(text));
        }
        Ok(parts)
    }

    fn argument(&mut self) -> Result<Argument, MessageError> {
        let start = self.i;
        self.bump(); // {
        self.spaces();
        let name = self.word()?;
        self.spaces();
        let mut kind = Kind::Simple;
        let mut offset = 0;
        let mut options: Vec<(String, Vec<Part>)> = Vec::new();
        if self.peek() == Some(',') {
            self.bump();
            self.spaces();
            kind = match self.word()?.as_str() {
                "plural" => Kind::Plural,
                "select" => Kind::Select,
                other => return self.fail(&format!("unsupported argument type {other}")),
            };
            self.spaces();
            if self.peek() != Some(',') {
                return self.fail(", expected");
            }
            self.bump();
            self.spaces();
            if kind == Kind::Plural && self.text[self.i..].starts_with("offset:") {
                self.i += "offset:".len();
                self.spaces();
                let digits: String = self.text[self.i..]
                    .chars()
                    .take_while(char::is_ascii_digit)
                    .collect();
                offset = match digits.parse() {
                    Ok(n) => n,
                    Err(_) => return self.fail("offset number expected"),
                };
                self.i += digits.len();
                self.spaces();
            }
            while self.peek().is_some_and(|c| c != '}') {
                let selector = self.word()?;
                if options.iter().any(|(s, _)| *s == selector) {
                    return self.fail(&format!("repeated option {selector}"));
                }
                self.spaces();
                if self.peek() != Some('{') {
                    return self.fail("{ expected");
                }
                self.bump();
                let sub = self.message(kind == Kind::Plural, true)?;
                if self.peek() != Some('}') {
                    return self.fail("} expected");
                }
                self.bump();
                options.push((selector, sub));
                self.spaces();
            }
            if !options.iter().any(|(s, _)| s == "other") {
                return self.fail(&format!("{name}: option \"other\" is required"));
            }
        }
        if self.peek() != Some('}') {
            return self.fail("} expected");
        }
        self.bump();
        Ok(Argument {
            name,
            kind,
            offset,
            options,
            raw: self.text[start..self.i].to_string(),
        })
    }
}

pub fn parse(text: &str) -> Result<Vec<Part>, MessageError> {
    let mut p = Parser { text, i: 0 };
    let parts = p.message(false, false)?;
    if p.i < text.len() {
        return p.fail("unexpected }");
    }
    Ok(parts)
}

/// Names of all arguments (nested ones too).
pub fn argument_names(parts: &[Part]) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    collect(parts, &mut out);
    out
}

fn collect(parts: &[Part], out: &mut BTreeSet<String>) {
    for part in parts {
        if let Part::Arg(a) = part {
            out.insert(a.name.clone());
            for (_, sub) in &a.options {
                collect(sub, out);
            }
        }
    }
}

/// Language-dependent part of formatting.
pub trait Context {
    /// CLDR category and numeric value of an (already formatted) value, None - not a number
    fn plural(&self, value: &str) -> Option<(&'static str, f64)>;
    /// a number shown as text
    fn number(&self, value: f64) -> String;
}

pub fn format(parts: &[Part], vars: &[(&str, &str)], ctx: &dyn Context) -> String {
    let mut out = String::new();
    write(parts, vars, ctx, None, &mut out);
    out
}

fn write(
    parts: &[Part],
    vars: &[(&str, &str)],
    ctx: &dyn Context,
    pound: Option<&str>,
    out: &mut String,
) {
    for part in parts {
        let a = match part {
            Part::Text(t) => {
                out.push_str(t);
                continue;
            }
            Part::Pound => {
                out.push_str(pound.unwrap_or("#"));
                continue;
            }
            Part::Arg(a) => a,
        };
        let Some(value) = vars.iter().find(|(k, _)| *k == a.name).map(|(_, v)| *v) else {
            out.push_str(&a.raw);
            continue;
        };
        let option = |name: &str| a.options.iter().find(|(s, _)| s == name).map(|(_, p)| p);
        match a.kind {
            Kind::Simple => out.push_str(value),
            Kind::Select => {
                let sub = option(value).or_else(|| option("other"));
                write(sub.map_or(&[][..], Vec::as_slice), vars, ctx, pound, out);
            }
            Kind::Plural => {
                let Some((_, n)) = ctx.plural(value) else {
                    out.push_str(&a.raw);
                    continue;
                };
                let offset = f64::from(a.offset);
                let (category, shown) = if a.offset == 0 {
                    (
                        ctx.plural(value).map_or("other", |c| c.0),
                        value.to_string(),
                    )
                } else {
                    let shown = ctx.number(n - offset);
                    (ctx.plural(&shown).map_or("other", |c| c.0), shown)
                };
                let exact = format!("={}", n);
                let sub = option(&exact)
                    .or_else(|| option(category))
                    .or_else(|| option("other"));
                write(
                    sub.map_or(&[][..], Vec::as_slice),
                    vars,
                    ctx,
                    Some(&shown),
                    out,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct English;
    impl Context for English {
        fn plural(&self, value: &str) -> Option<(&'static str, f64)> {
            let n: f64 = value.parse().ok()?;
            Some((if value == "1" { "one" } else { "other" }, n))
        }
        fn number(&self, value: f64) -> String {
            value.to_string()
        }
    }

    #[test]
    fn plural_select_and_quotes() {
        let m =
            parse("{n, plural, =0 {no steps} one {# step} other {# steps}} '{x}' it's").unwrap();
        assert_eq!(format(&m, &[("n", "1")], &English), "1 step {x} it's");
        assert_eq!(format(&m, &[("n", "0")], &English), "no steps {x} it's");
        assert_eq!(format(&m, &[("n", "7")], &English), "7 steps {x} it's");
        assert_eq!(
            format(&m, &[], &English),
            "{n, plural, =0 {no steps} one {# step} other {# steps}} {x} it's"
        );
        let s = parse("{on, select, yes {on} other {off}}").unwrap();
        assert_eq!(format(&s, &[("on", "yes")], &English), "on");
        assert_eq!(format(&s, &[("on", "x")], &English), "off");
        assert_eq!(
            argument_names(&parse("{a} {b, plural, other {{c}}}").unwrap()),
            ["a", "b", "c"].iter().map(|s| s.to_string()).collect()
        );
        assert!(parse("{n, plural, one {x}}").is_err());
        assert!(parse("a } b").is_err());
        assert!(parse("{n, number}").is_err());
    }
}
