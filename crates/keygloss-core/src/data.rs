//! Language data of a pair: dictionaries, trigram models, typo table and extra word lists
//! (`LayoutData.swift`, `ExtraWords.swift`, `AmbiguousPairs.swift`).
//!
//! Parsing the embedded JSON takes a noticeable moment, so the app calls [`warm_up`] once on a
//! background thread and treats [`ready`] returning `None` as "data not loaded yet: do nothing".

use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

use crate::keymap::Keymap;
use crate::pair::{LanguagePair, RU_EN, Side};

/// A Latin token and an Alt word typed with the same keys, both real words (`vs` / `мы`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AmbiguousPair {
    pub latin: String,
    pub alt: String,
}

/// Extra word lists, by role (see [`crate::pair::ExtraKeys`]).
#[derive(Debug, Default)]
pub struct ExtraSets {
    pub latin_prefix_keep: HashSet<String>,
    pub alt_prefix_keep: HashSet<String>,
    pub force_alt_amb: HashSet<String>,
    pub force_latin_amb: HashSet<String>,
    pub latin_tech_tokens: HashSet<String>,
    pub tech_from_alt: HashSet<String>,
    pub default_keep: HashSet<String>,
    pub latin_hyphen_terms: HashSet<String>,
    pub alt_hyphen_terms: HashSet<String>,
    pub latin_label_classifiers: HashSet<String>,
    pub alt_label_classifiers: HashSet<String>,
    pub latin_keep_short: HashSet<String>,
    pub force_swap: HashSet<String>,
    pub common_latin_two_letter: HashSet<String>,
    pub alt_single_letters: HashSet<String>,
    /// In display order (most useful first).
    pub ambiguous_pairs: Vec<AmbiguousPair>,
}

/// Loaded data of one [`LanguagePair`].
pub struct LangData {
    pub pair: &'static LanguagePair,
    /// The pair's fixed table, for checks that the original ran on the static table.
    pub base: Keymap,
    pub words_latin: HashSet<String>,
    pub words_alt: HashSet<String>,
    trigrams_latin: HashMap<String, f64>,
    trigrams_alt: HashMap<String, f64>,
    pub typos_latin: HashMap<String, String>,
    pub typos_alt: HashMap<String, String>,
    pub extra: ExtraSets,
}

impl LangData {
    /// Parses the pair's embedded data. Panics only if the embedded files are malformed, which
    /// the tests rule out.
    pub fn load(pair: &'static LanguagePair) -> LangData {
        let f = &pair.files;
        let extra: HashMap<String, serde_json::Value> = parse(f.extra);
        let k = &pair.extra;
        let set = |keys: &[&str]| -> HashSet<String> {
            let mut out = HashSet::new();
            for key in keys {
                let list = extra.get(*key).and_then(|v| v.as_array()).expect("extra word list exists");
                out.extend(list.iter().filter_map(|w| w.as_str()).map(str::to_string));
            }
            out
        };
        let mut words_latin: HashSet<String> = parse::<Vec<String>>(f.words_latin).into_iter().collect();
        words_latin.extend(set(k.latin_words));
        let mut words_alt: HashSet<String> = parse::<Vec<String>>(f.words_alt).into_iter().collect();
        words_alt.extend(set(k.alt_words));
        let mut typos: HashMap<String, HashMap<String, String>> = parse(f.typo_rules);
        let ambiguous_pairs = extra
            .get(k.ambiguous_pairs)
            .and_then(|v| serde_json::from_value::<Vec<(String, String)>>(v.clone()).ok())
            .expect("ambiguous pairs list exists")
            .into_iter()
            .map(|(latin, alt)| AmbiguousPair { latin, alt })
            .collect();
        let extra = ExtraSets {
            latin_prefix_keep: set(k.latin_prefix_keep),
            alt_prefix_keep: set(k.alt_prefix_keep),
            force_alt_amb: set(&[k.force_alt_amb]),
            force_latin_amb: set(&[k.force_latin_amb]),
            latin_tech_tokens: set(&[k.latin_tech_tokens]),
            tech_from_alt: set(&[k.tech_from_alt]),
            default_keep: set(&[k.default_keep]),
            latin_hyphen_terms: set(&[k.latin_hyphen_terms]),
            alt_hyphen_terms: set(&[k.alt_hyphen_terms]),
            latin_label_classifiers: set(&[k.latin_label_classifiers]),
            alt_label_classifiers: set(&[k.alt_label_classifiers]),
            latin_keep_short: set(&[k.latin_keep_short]),
            force_swap: set(&[k.force_swap]),
            common_latin_two_letter: set(&[k.common_latin_two_letter]),
            alt_single_letters: set(&[k.alt_single_letters]),
            ambiguous_pairs,
        };
        LangData {
            pair,
            base: Keymap::fallback(pair),
            words_latin,
            words_alt,
            trigrams_latin: parse(f.trigrams_latin),
            trigrams_alt: parse(f.trigrams_alt),
            typos_latin: typos.remove(pair.latin_lang).unwrap_or_default(),
            typos_alt: typos.remove(pair.alt_lang).unwrap_or_default(),
            extra,
        }
    }

    /// The dictionary of one side (base dictionary plus the extra word lists).
    pub fn words(&self, side: Side) -> &HashSet<String> {
        match side {
            Side::Latin => &self.words_latin,
            Side::Alt => &self.words_alt,
        }
    }

    /// The word is in either dictionary (exact, case-sensitive; dictionaries are lowercase).
    pub fn in_dictionaries(&self, w: &str) -> bool {
        self.words_alt.contains(w) || self.words_latin.contains(w)
    }

    /// Mean trigram log-probability of `" " + word.lowercase() + " "` in the `side` language;
    /// a missing trigram scores −20.
    pub fn plausibility(&self, word: &str, side: Side) -> f64 {
        let table = match side {
            Side::Latin => &self.trigrams_latin,
            Side::Alt => &self.trigrams_alt,
        };
        let chars: Vec<char> = format!(" {} ", word.to_lowercase()).chars().collect();
        if chars.len() < 3 {
            return f64::NEG_INFINITY;
        }
        let sum: f64 = chars
            .windows(3)
            .map(|t| table.get(&t.iter().collect::<String>()).copied().unwrap_or(-20.0))
            .sum();
        sum / (chars.len() - 2) as f64
    }
}

fn parse<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> T {
    serde_json::from_slice(bytes).expect("embedded language data is valid JSON")
}

static RU_EN_DATA: OnceLock<LangData> = OnceLock::new();

/// Loads the RU↔EN data once (blocking) and returns it. Call it from a background thread at
/// startup; later calls return immediately.
pub fn warm_up() -> &'static LangData {
    RU_EN_DATA.get_or_init(|| LangData::load(&RU_EN))
}

/// The loaded data, or `None` while [`warm_up`] has not finished.
pub fn ready() -> Option<&'static LangData> {
    RU_EN_DATA.get()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_all_lists_with_source_counts() {
        let d = warm_up();
        assert!(ready().is_some());
        let e = &d.extra;
        // Unique entries of the original's literal sets (duplicates in the source collapse).
        assert_eq!(e.force_swap.len(), 74);
        assert_eq!(e.force_alt_amb.len(), 22);
        assert_eq!(e.force_latin_amb.len(), 58);
        assert_eq!(e.latin_tech_tokens.len(), 52);
        assert_eq!(e.tech_from_alt.len(), 20);
        assert_eq!(e.default_keep.len(), 19);
        assert_eq!(e.latin_keep_short.len(), 65);
        assert_eq!(e.common_latin_two_letter.len(), 30);
        assert_eq!(e.alt_single_letters.len(), 8);
        assert_eq!(e.ambiguous_pairs.len(), 48);
        assert_eq!(e.ambiguous_pairs[0], AmbiguousPair { latin: "yt".into(), alt: "не".into() });
        assert!(d.words_alt.contains("привет"));
        assert!(d.words_alt.contains("багфикс")); // ruDev merged
        assert!(d.words_latin.contains("hello"));
        assert!(d.words_latin.contains("rizz")); // en extra merged
        assert!(!d.typos_alt.is_empty() && !d.typos_latin.is_empty());
    }

    #[test]
    fn plausibility_prefers_real_words() {
        let d = warm_up();
        assert!(d.plausibility("привет", Side::Alt) > d.plausibility("ghbdtn", Side::Latin));
        assert!(d.plausibility("ghbdtn", Side::Latin) <= -10.0);
    }
}
