//! Typo fixes (`TypoFix.swift`, `NumericTypoRule.swift`, `Engine.fixTwoLeadingCaps`,
//! `Engine.fixCapsLockWord`).
//!
//! All functions are the pure decision: the engine checks the `typoFix` / `twoCapsFix` settings,
//! applies the replacement and remembers it for undo. Order at a boundary where the layout was
//! not converted: two capitals, then Caps Lock word, then [`TypoFixer::note_typed`] and
//! [`TypoFixer::suggest`].

use std::collections::BTreeMap;

use crate::data::LangData;
use crate::exceptions::Exceptions;
use crate::pair::{LanguagePair, has_latin_letter};

/// An unknown word typed this many times is the user's own and never corrected.
pub const PERSONAL_THRESHOLD: u32 = 2;

const TWO_CAPS_KEEP: &[&str] = &["iphone", "ipad", "ipod", "imac", "icloud", "itunes", "imessage", "ibooks", "iwork", "ebay"];
const BRAND_SPELLING: &[(&str, &str)] = &[("ios", "iOS"), ("ipados", "iPadOS"), ("macos", "macOS"), ("watchos", "watchOS"), ("tvos", "tvOS"), ("visionos", "visionOS"), ("iphone", "iPhone"), ("ipad", "iPad"), ("imac", "iMac"), ("icloud", "iCloud"), ("itunes", "iTunes"), ("imessage", "iMessage"), ("ebay", "eBay"), ("esim", "eSIM"), ("iot", "IoT")];

fn count(s: &str) -> usize {
    s.chars().count()
}

fn all_letters(s: &str) -> bool {
    s.chars().all(char::is_alphabetic)
}

fn one_script(pair: &LanguagePair, s: &str) -> bool {
    pair.has_alt(s) != has_latin_letter(s)
}

/// The pair's decimal-point letter between two ASCII digits becomes `.` (`1ю8` → `1.8`).
pub fn numeric_typo(pair: &LanguagePair, text: &str) -> Option<String> {
    let alias = pair.decimal_alias?;
    let mut chars: Vec<char> = text.chars().collect();
    if chars.len() < 3 {
        return None;
    }
    let mut changed = false;
    for i in 1..chars.len() - 1 {
        if chars[i] == alias && chars[i - 1].is_ascii_digit() && chars[i + 1].is_ascii_digit() {
            chars[i] = '.';
            changed = true;
        }
    }
    changed.then(|| chars.into_iter().collect())
}

/// Two leading capitals then lowercase (`КОгда` → `Когда`). `None` = leave it.
pub fn fix_two_leading_caps(pair: &LanguagePair, word: &str, exceptions: &Exceptions) -> Option<String> {
    let ch: Vec<char> = word.chars().collect();
    if ch.len() < 3 || !all_letters(word) {
        return None;
    }
    if !(ch[0].is_uppercase() && ch[1].is_uppercase() && ch[2].is_lowercase()) || !one_script(pair, word) {
        return None;
    }
    let fixed: String = std::iter::once(ch[0]).chain(ch[1].to_lowercase()).chain(ch[2..].iter().copied()).collect();
    let lower = word.to_lowercase();
    if fixed == word || exceptions.ignored.contains(&lower) || TWO_CAPS_KEEP.contains(&lower.as_str()) {
        return None;
    }
    Some(fixed)
}

/// A word typed with Caps Lock stuck and Shift on the first letter (`пРИВЕТ` → `Привет`, `mACOS`
/// → `macOS`): at least 5 letters, first lowercase, the rest uppercase.
pub fn fix_caps_lock_word(pair: &LanguagePair, word: &str, exceptions: &Exceptions) -> Option<String> {
    let ch: Vec<char> = word.chars().collect();
    if ch.len() < 5 || !all_letters(word) {
        return None;
    }
    if !ch[0].is_lowercase() || !ch[1..].iter().all(|c| c.is_uppercase()) || !one_script(pair, word) {
        return None;
    }
    let lower = word.to_lowercase();
    if exceptions.ignored.contains(&lower) {
        return None;
    }
    let fixed = match BRAND_SPELLING.iter().find(|(k, _)| *k == lower) {
        Some((_, canonical)) => canonical.to_string(),
        None => ch[0].to_uppercase().chain(ch[1..].iter().flat_map(|c| c.to_lowercase())).collect(),
    };
    (fixed != word).then_some(fixed)
}

fn restoring_leading_case(original: &str, fixed: &str) -> String {
    if original.chars().next().is_some_and(char::is_uppercase) {
        let mut cs = fixed.chars();
        cs.next().map(|f| f.to_uppercase().chain(cs).collect()).unwrap_or_default()
    } else {
        fixed.to_string()
    }
}

/// Unicode general category P* (punctuation), approximated without a Unicode table: ASCII
/// punctuation that is not a symbol, Latin-1 punctuation, and the General Punctuation block.
fn is_punctuation(c: char) -> bool {
    matches!(c, '!' | '"' | '#' | '%' | '&' | '\'' | '(' | ')' | '*' | ',' | '-' | '.' | '/' | ':' | ';'
        | '?' | '@' | '[' | '\\' | ']' | '_' | '{' | '}' | '¡' | '§' | '«' | '¶' | '·' | '»' | '¿')
        || ('\u{2010}'..='\u{2027}').contains(&c)
        || (('\u{2030}'..='\u{205E}').contains(&c) && !matches!(c, '\u{2044}' | '\u{2052}'))
}

/// (prefix, core, suffix): outer punctuation peeled.
fn punctuation_wrapped_core(word: &str) -> Option<(&str, &str, &str)> {
    let start = word.find(|c| !is_punctuation(c))?;
    let end = word.rfind(|c| !is_punctuation(c)).map(|i| i + word[i..].chars().next().map_or(1, char::len_utf8))?;
    Some((&word[..start], &word[start..end], &word[end..]))
}

/// The typo table and the personal dictionary.
#[derive(Clone, Debug, Default)]
pub struct TypoFixer {
    /// Unknown word → times typed (`typoPersonalWords`), capped at [`PERSONAL_THRESHOLD`].
    pub personal: BTreeMap<String, u32>,
}

impl TypoFixer {
    pub fn new(personal: BTreeMap<String, u32>) -> TypoFixer {
        TypoFixer { personal }
    }

    fn table_candidate(data: &LangData, w: &str) -> Option<String> {
        let table = if data.pair.has_alt(w) { &data.typos_alt } else { &data.typos_latin };
        table.get(w).cloned()
    }

    /// Mechanical typo that yields exactly one dictionary word, in tiers; the first tier with a
    /// candidate decides, and two different candidates in it mean "leave it":
    /// 1. adjacent transposition (not in the last two positions) or a letter typed twice (not at
    ///    either end);
    /// 2. one of the pair's commonly confused letters swapped (`превет` → `привет`), not in the
    ///    last three positions (endings);
    /// 3. one missing letter (`tlephone` → `telephone`), not first and not in the last two
    ///    positions; words of 6+ letters.
    ///
    /// Words of 5+ letters only.
    fn mechanical(data: &LangData, w: &str) -> Option<String> {
        let chars: Vec<char> = w.chars().collect();
        let n = chars.len();
        if n < 5 {
            return None;
        }
        let alt = data.pair.has_alt(w);
        let dict = if alt { &data.words_alt } else { &data.words_latin };
        // None: no candidate in the tier. Some(None): two different ones (ambiguous).
        let pick = |cands: &mut dyn Iterator<Item = Vec<char>>| -> Option<Option<String>> {
            let mut found: Option<String> = None;
            for c in cands {
                let s: String = c.into_iter().collect();
                if !dict.contains(&s) {
                    continue;
                }
                match &found {
                    Some(f) if *f != s => return Some(None),
                    _ => found = Some(s),
                }
            }
            found.map(Some)
        };
        let edited = |f: &dyn Fn(&mut Vec<char>)| {
            let mut c = chars.clone();
            f(&mut c);
            c
        };
        let mut swapped_or_doubled = (0..n - 2)
            .map(|i| edited(&|c| c.swap(i, i + 1)))
            .chain((1..n - 2).filter(|&i| chars[i] == chars[i + 1] && !data.pair.typo_keep_doubled.contains(&chars[i])).map(|i| edited(&|c| _ = c.remove(i))));
        let mut confused = (0..n - 3).filter_map(|i| {
            let other = data.pair.typo_confused_letters.iter().find_map(|&(a, b)| match chars[i] {
                x if x == a => Some(b),
                x if x == b => Some(a),
                _ => None,
            })?;
            Some(edited(&|c| c[i] = other))
        });
        let alphabet: Vec<char> = data.pair.base_pairs.iter().map(|&(l, a)| if alt { a } else { l }).filter(|c| c.is_alphabetic()).collect();
        let mut missing = (1..n - 1).filter(|_| n >= 6).flat_map(|i| alphabet.iter().map(move |&l| (i, l))).map(|(i, l)| edited(&|c| c.insert(i, l)));
        pick(&mut swapped_or_doubled).or_else(|| pick(&mut confused)).or_else(|| pick(&mut missing)).flatten()
    }

    /// The known fix of a lowercase word, ignoring settings and protections.
    fn candidate(data: &LangData, w: &str) -> Option<String> {
        Self::table_candidate(data, w).or_else(|| if all_letters(w) { Self::mechanical(data, w) } else { None })
    }

    /// A word went into the text: count it if it is unknown and we cannot fix it. Call it at
    /// every boundary with no conversion, even with typo fixing off.
    pub fn note_typed(&mut self, data: &LangData, word: &str) {
        let w = word.to_lowercase();
        if count(&w) < 4 || data.in_dictionaries(&w) || !all_letters(&w) || Self::candidate(data, &w).is_some() {
            return;
        }
        let n = self.personal.get(&w).copied().unwrap_or(0) + 1;
        if n <= PERSONAL_THRESHOLD {
            self.personal.insert(w, n);
        }
    }

    pub fn is_personal(&self, word: &str) -> bool {
        self.personal.get(&word.to_lowercase()).is_some_and(|&n| n >= PERSONAL_THRESHOLD)
    }

    /// Drops personal entries that have a known fix or are not all letters (call after loading).
    /// Returns how many were removed.
    pub fn forget_wrongfully_learned(&mut self, data: &LangData) -> usize {
        let before = self.personal.len();
        self.personal.retain(|w, _| all_letters(w) && Self::candidate(data, w).is_none());
        before - self.personal.len()
    }

    /// `1ю8` → `1.8`, unless the word is protected. `protected` gets a lowercase word and
    /// answers ignored, learned or session-protected.
    pub fn numeric_suggestion(&self, data: &LangData, word: &str, protected: &dyn Fn(&str) -> bool) -> Option<String> {
        let fixed = numeric_typo(data.pair, word)?;
        (!protected(&word.to_lowercase())).then_some(fixed)
    }

    /// Exact curated-table fix (outer punctuation peeled and restored). Cheap enough for the
    /// pre-Enter path.
    pub fn curated_suggestion(&self, data: &LangData, word: &str, protected: &dyn Fn(&str) -> bool) -> Option<String> {
        let (prefix, core, suffix) = punctuation_wrapped_core(word)?;
        let w = core.to_lowercase();
        if count(&w) < 4 || !one_script(data.pair, core) {
            return None;
        }
        let whole = word.to_lowercase();
        if data.in_dictionaries(&w) || self.is_personal(&w) || self.is_personal(&whole) || protected(&w) || protected(&whole) {
            return None;
        }
        let fixed = Self::table_candidate(data, &w)?;
        Some(format!("{prefix}{}{suffix}", restoring_leading_case(core, &fixed)))
    }

    /// Full suggestion: numeric rule, curated table, then mechanical. Leading capital kept.
    pub fn suggest(&self, data: &LangData, word: &str, protected: &dyn Fn(&str) -> bool) -> Option<String> {
        if let Some(f) = self.numeric_suggestion(data, word, protected) {
            return Some(f);
        }
        if let Some(f) = self.curated_suggestion(data, word, protected) {
            return Some(f);
        }
        let w = word.to_lowercase();
        if count(&w) < 4 || !all_letters(&w) || !one_script(data.pair, word) {
            return None;
        }
        if data.in_dictionaries(&w) || self.is_personal(&w) || protected(&w) {
            return None;
        }
        Self::mechanical(data, &w).map(|f| restoring_leading_case(word, &f))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::warm_up;
    use crate::pair::RU_EN;

    const NONE: &dyn Fn(&str) -> bool = &|_| false;

    #[test]
    fn table_hit_and_case() {
        let d = warm_up();
        let t = TypoFixer::default();
        assert_eq!(d.typos_latin.len(), 155);
        assert_eq!(d.typos_alt.len(), 4874);
        assert_eq!(t.suggest(d, "adress", NONE).as_deref(), Some("address"));
        assert_eq!(t.suggest(d, "Adress,", NONE).as_deref(), Some("Address,"));
        assert_eq!(t.suggest(d, "address", NONE), None);
        assert_eq!(t.suggest(d, "adress", &|w| w == "adress"), None);
    }

    #[test]
    fn mechanical_swap_and_double() {
        let d = warm_up();
        let t = TypoFixer::default();
        assert_eq!(t.suggest(d, "рпивет", NONE).as_deref(), Some("привет"));
        assert_eq!(t.suggest(d, "Рпивет", NONE).as_deref(), Some("Привет"));
        assert_eq!(t.suggest(d, "приивет", NONE).as_deref(), Some("привет"));
        // Doubled letters at the ends and the doubled `н` are never touched.
        assert_eq!(TypoFixer::mechanical(d, "ввода"), None);
        assert_eq!(TypoFixer::mechanical(d, "ценна"), None);
        assert_eq!(t.suggest(d, "кеш", NONE), None);
    }

    #[test]
    fn mechanical_confused_and_missing() {
        let d = warm_up();
        let t = TypoFixer::default();
        assert_eq!(t.suggest(d, "превет", NONE).as_deref(), Some("привет"));
        assert_eq!(t.suggest(d, "каторый", NONE).as_deref(), Some("который"));
        assert_eq!(t.suggest(d, "tlephone", NONE).as_deref(), Some("telephone"));
        assert_eq!(t.suggest(d, "Tlephone", NONE).as_deref(), Some("Telephone"));
        assert_eq!(t.suggest(d, "пожалуста", NONE).as_deref(), Some("пожалуйста"));
        // Endings are never touched: a missing last letter or a confused vowel near the end.
        assert_eq!(TypoFixer::mechanical(d, "telephon"), None);
        assert_eq!(TypoFixer::mechanical(d, "сделаеш"), None);
    }

    #[test]
    fn personal_dictionary() {
        let d = warm_up();
        let mut t = TypoFixer::default();
        t.note_typed(d, "зыбрик");
        assert!(!t.is_personal("зыбрик"));
        t.note_typed(d, "Зыбрик");
        assert!(t.is_personal("зыбрик"));
        t.note_typed(d, "зыбрик");
        assert_eq!(t.personal["зыбрик"], PERSONAL_THRESHOLD);
        // A fixable typo is never learned.
        t.note_typed(d, "рпивет");
        assert!(!t.personal.contains_key("рпивет"));
        t.personal.insert("рпивет".into(), 2);
        t.personal.insert("сейчас?".into(), 2);
        assert_eq!(t.forget_wrongfully_learned(d), 2);
    }

    #[test]
    fn numeric_rule() {
        let d = warm_up();
        assert_eq!(numeric_typo(&RU_EN, "1ю8").as_deref(), Some("1.8"));
        assert_eq!(numeric_typo(&RU_EN, "ю8"), None);
        assert_eq!(numeric_typo(&RU_EN, "1Ю8"), None);
        assert_eq!(TypoFixer::default().suggest(d, "1ю8", NONE).as_deref(), Some("1.8"));
    }

    #[test]
    fn two_caps() {
        let e = Exceptions::default();
        assert_eq!(fix_two_leading_caps(&RU_EN, "КОгда", &e).as_deref(), Some("Когда"));
        assert_eq!(fix_two_leading_caps(&RU_EN, "WHen", &e).as_deref(), Some("When"));
        assert_eq!(fix_two_leading_caps(&RU_EN, "IPhone", &e), None);
        assert_eq!(fix_two_leading_caps(&RU_EN, "ГОСТ", &e), None);
        assert_eq!(fix_two_leading_caps(&RU_EN, "ДА", &e), None);
        assert_eq!(fix_two_leading_caps(&RU_EN, "RGb2", &e), None);
        let mut e = Exceptions::default();
        e.ignored.insert("фбр".into());
        assert_eq!(fix_two_leading_caps(&RU_EN, "ФБр", &e), None);
    }

    #[test]
    fn caps_lock_word() {
        let e = Exceptions::default();
        assert_eq!(fix_caps_lock_word(&RU_EN, "пРИВЕТ", &e).as_deref(), Some("Привет"));
        assert_eq!(fix_caps_lock_word(&RU_EN, "mACOS", &e).as_deref(), Some("macOS"));
        assert_eq!(fix_caps_lock_word(&RU_EN, "iPHONE", &e).as_deref(), Some("iPhone"));
        assert_eq!(fix_caps_lock_word(&RU_EN, "iOS", &e), None);
        assert_eq!(fix_caps_lock_word(&RU_EN, "hELLo", &e), None);
    }
}
