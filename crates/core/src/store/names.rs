//! Losowe nazwy osób: przymiotnik + rzeczownik w zgodnym rodzaju („Szybki Zawodnik”) -
//! port bieznia/nazwy.py.
//!
//! Przymiotniki zapisane parami (męski, żeński), rzeczowniki osobno dla każdego rodzaju -
//! odmiana nie jest zgadywana z końcówek, więc nie wyjdzie „Szybki Biegaczka”.

use std::collections::HashSet;

/// Przymiotniki: (rodzaj męski, rodzaj żeński).
pub const ADJECTIVES: [(&str, &str); 18] = [
    ("Szybki", "Szybka"),
    ("Szalony", "Szalona"),
    ("Podstępny", "Podstępna"),
    ("Dzielny", "Dzielna"),
    ("Wytrwały", "Wytrwała"),
    ("Niezmordowany", "Niezmordowana"),
    ("Zwinny", "Zwinna"),
    ("Nocny", "Nocna"),
    ("Cichy", "Cicha"),
    ("Żelazny", "Żelazna"),
    ("Błyskawiczny", "Błyskawiczna"),
    ("Nieustraszony", "Nieustraszona"),
    ("Tajemniczy", "Tajemnicza"),
    ("Legendarny", "Legendarna"),
    ("Waleczny", "Waleczna"),
    ("Uparty", "Uparta"),
    ("Sprytny", "Sprytna"),
    ("Wesoły", "Wesoła"),
];

/// Rzeczowniki rodzaju męskiego.
pub const NOUNS_M: [&str; 12] = [
    "Zawodnik",
    "Biegacz",
    "Maszerowicz",
    "Piechur",
    "Wędrowiec",
    "Tropiciel",
    "Sprinter",
    "Spacerowicz",
    "Truchtacz",
    "Łazik",
    "Podróżnik",
    "Zdobywca",
];

/// Rzeczowniki rodzaju żeńskiego.
pub const NOUNS_F: [&str; 10] = [
    "Zawodniczka",
    "Biegaczka",
    "Maszerowiczka",
    "Piechurka",
    "Wędrowniczka",
    "Tropicielka",
    "Sprinterka",
    "Spacerowiczka",
    "Podróżniczka",
    "Zdobywczyni",
];

/// Prosty generator liczb losowych (splitmix64) do losowania nazw.
/// `from_seed` daje powtarzalne wyniki (testy), `new` - losowe.
#[derive(Debug, Clone)]
pub struct NameRng(u64);

impl NameRng {
    /// Generator z losowym ziarnem (z UUID v4).
    pub fn new() -> Self {
        let bytes = uuid::Uuid::new_v4().into_bytes();
        let mut seed = [0u8; 8];
        seed.copy_from_slice(&bytes[..8]);
        Self(u64::from_le_bytes(seed))
    }

    /// Generator z podanym ziarnem (powtarzalny).
    pub fn from_seed(seed: u64) -> Self {
        Self(seed)
    }

    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Liczba z przedziału 0..n (n > 0).
    fn below(&mut self, n: usize) -> usize {
        (self.next_u64() % n.max(1) as u64) as usize
    }
}

impl Default for NameRng {
    fn default() -> Self {
        Self::new()
    }
}

/// Wszystkie nazwy (396): najpierw męskie, potem żeńskie.
pub fn all_names() -> Vec<String> {
    let mut names: Vec<String> = ADJECTIVES
        .iter()
        .flat_map(|(m, _)| NOUNS_M.iter().map(move |n| format!("{m} {n}")))
        .collect();
    names.extend(
        ADJECTIVES
            .iter()
            .flat_map(|(_, f)| NOUNS_F.iter().map(move |n| format!("{f} {n}"))),
    );
    names
}

/// Nazwa, której jeszcze nikt nie ma (gdy wszystkie zajęte - z numerem, np. „Cichy Łazik 2”).
pub fn random_name<S: AsRef<str>>(taken: &[S], rng: &mut NameRng) -> String {
    let used: HashSet<&str> = taken.iter().map(AsRef::as_ref).collect();
    let all = all_names();
    let free: Vec<&String> = all.iter().filter(|n| !used.contains(n.as_str())).collect();
    if !free.is_empty() {
        return free[rng.below(free.len())].clone();
    }
    let base = &all[rng.below(all.len())];
    let mut i = 2;
    while used.contains(format!("{base} {i}").as_str()) {
        i += 1;
    }
    format!("{base} {i}")
}

/// `count` różnych wolnych nazw (mniej, gdy wolnych nie starcza).
pub fn suggestions<S: AsRef<str>>(count: usize, taken: &[S], rng: &mut NameRng) -> Vec<String> {
    let used: HashSet<&str> = taken.iter().map(AsRef::as_ref).collect();
    let mut free: Vec<String> = all_names()
        .into_iter()
        .filter(|n| !used.contains(n.as_str()))
        .collect();
    let k = count.min(free.len());
    // częściowe tasowanie Fishera-Yatesa - losowanie bez powtórzeń
    for i in 0..k {
        let j = i + rng.below(free.len() - i);
        free.swap(i, j);
    }
    free.truncate(k);
    free
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_have_matching_gender() {
        let masc: HashSet<&str> = ADJECTIVES.iter().map(|(m, _)| *m).collect();
        let fem: HashSet<&str> = ADJECTIVES.iter().map(|(_, f)| *f).collect();
        let all = all_names();
        for name in &all {
            let (adj, noun) = name.split_once(' ').unwrap();
            if NOUNS_M.contains(&noun) {
                assert!(masc.contains(adj), "{name}");
            } else {
                assert!(NOUNS_F.contains(&noun) && fem.contains(adj), "{name}");
            }
        }
        let unique: HashSet<&String> = all.iter().collect();
        assert_eq!(all.len(), 396);
        assert_eq!(unique.len(), 396);
    }

    #[test]
    fn names_match_python() {
        let text = include_str!("../../tests/fixtures/store/names.json");
        let f: serde_json::Value = serde_json::from_str(text).unwrap();
        let py: Vec<String> = serde_json::from_value(f["wszystkie"].clone()).unwrap();
        assert_eq!(all_names(), py);
    }

    #[test]
    fn random_name_avoids_taken_ones() {
        let mut rng = NameRng::from_seed(1);
        let all = all_names();
        let taken = &all[..all.len() - 1];
        assert_eq!(random_name(taken, &mut rng), all[all.len() - 1]);
        assert!(random_name(&all, &mut rng).ends_with(" 2"));
        let mut more = all.clone();
        let next = random_name(&more, &mut rng);
        more.push(next.clone());
        // następna z numerem nie powtarza zajętej
        for _ in 0..50 {
            let n = random_name(&more, &mut rng);
            assert!(!more.contains(&n));
        }
        let s = suggestions::<&str>(3, &[], &mut rng);
        let unique: HashSet<&String> = s.iter().collect();
        assert_eq!(unique.len(), 3);
        assert_eq!(suggestions(5, &all[..all.len() - 2], &mut rng).len(), 2);
        let mut random = NameRng::new();
        assert!(all.contains(&random_name::<&str>(&[], &mut random)));
    }
}
