//! Language pair: everything specific to one pair of keyboard layouts.
//!
//! The generic code (keymap, detector, typo fixes, snippets) never names a language; it asks the
//! [`LanguagePair`] instead. One side of a pair is always the ASCII Latin layout ([`Side::Latin`]),
//! the other is the pair's own script ([`Side::Alt`]). [`RU_EN`] is the only instance.

/// One side of a [`LanguagePair`]. Also used as a conversion target ("convert to `Alt`").
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Side {
    /// The ASCII Latin layout (EN in RU↔EN).
    Latin,
    /// The pair's other script (RU in RU↔EN).
    Alt,
}

impl Side {
    pub fn other(self) -> Side {
        match self {
            Side::Latin => Side::Alt,
            Side::Alt => Side::Latin,
        }
    }
}

/// Embedded data files of a pair (JSON, parsed by [`crate::data::LangData::load`]).
pub struct PairFiles {
    /// `["word", ...]`
    pub words_latin: &'static [u8],
    pub words_alt: &'static [u8],
    /// `{"abc": log-probability, ...}`
    pub trigrams_latin: &'static [u8],
    pub trigrams_alt: &'static [u8],
    /// `{"<latin_lang>": {"typo": "fix"}, "<alt_lang>": {...}}`
    pub typo_rules: &'static [u8],
    /// Named word lists extracted from the original's source (`extra_words.json`).
    pub extra: &'static [u8],
}

/// Which lists of [`PairFiles::extra`] play which role in the generic code.
/// Each role is the union of the named lists.
pub struct ExtraKeys {
    /// Merged into the Latin dictionary.
    pub latin_words: &'static [&'static str],
    /// Merged into the Alt dictionary.
    pub alt_words: &'static [&'static str],
    /// Live fix keeps these (and their prefixes) for a Latin-script word.
    pub latin_prefix_keep: &'static [&'static str],
    /// Live fix keeps these (and their prefixes) for an Alt-script word.
    pub alt_prefix_keep: &'static [&'static str],
    /// Latin spellings that are dictionary words but almost always a mistyped Alt word.
    pub force_alt_amb: &'static str,
    /// Latin slang/contractions missing from the dictionary; their Alt form is gibberish.
    pub force_latin_amb: &'static str,
    /// Latin technical tokens kept as typed.
    pub latin_tech_tokens: &'static str,
    /// Latin tokens converted from their Alt-layout form when typed as a standalone word.
    pub tech_from_alt: &'static str,
    /// Never converted, in any context.
    pub default_keep: &'static str,
    /// Canonical hyphenated terms of each side.
    pub latin_hyphen_terms: &'static str,
    pub alt_hyphen_terms: &'static str,
    /// Words after which a single Latin letter is a marker ("vitamin d"), per context side.
    pub latin_label_classifiers: &'static str,
    pub alt_label_classifiers: &'static str,
    /// Short Latin tokens kept even in an Alt-language context.
    pub latin_keep_short: &'static str,
    /// Built-in force-swap abbreviations (Latin form).
    pub force_swap: &'static str,
    /// The only two-letter Latin words accepted as a conversion result.
    pub common_latin_two_letter: &'static str,
    /// One-letter Alt words that a single Latin letter converts to.
    pub alt_single_letters: &'static str,
    /// `[[latin, alt], ...]` pairs where both sides are real words ("who wins" page).
    pub ambiguous_pairs: &'static str,
}

/// Everything pair-specific. The generic code consults these fields; it hard-codes no language.
pub struct LanguagePair {
    /// Language code of the Latin side (key in `typo_rules`), e.g. `"en"`.
    pub latin_lang: &'static str,
    /// Language code of the Alt side, e.g. `"ru"`.
    pub alt_lang: &'static str,
    /// Fallback key table: (Latin char, Alt char) for each key, lowercase, unshifted.
    pub base_pairs: &'static [(char, char)],
    /// True for a character of the Alt script.
    pub is_alt_char: fn(char) -> bool,
    /// One-letter Latin words that a single Alt letter converts to (`ш` → `i`).
    pub latin_single_letters: &'static [&'static str],
    /// Alt-script spelling that cannot exist in the Alt language (so an all-caps token with it
    /// is not an abbreviation).
    pub is_impossible_alt_spelling: fn(&str) -> bool,
    /// Alt letter on the decimal-point key; between two ASCII digits it is a typo for `.`.
    pub decimal_alias: Option<char>,
    /// Letters whose doubling is grammar, not a typo (never de-doubled by the typo fixer).
    pub typo_keep_doubled: &'static [char],
    /// Latin short word that continues an Alt phrase after a Latin abbreviation with a comma
    /// (`… касается iOS, nj`). Empty disables the rule.
    pub label_bridge_word: &'static str,
    pub files: PairFiles,
    pub extra: ExtraKeys,
}

impl LanguagePair {
    /// Text contains a character of the Alt script.
    pub fn has_alt(&self, s: &str) -> bool {
        s.chars().any(self.is_alt_char)
    }

    /// The single script of `s`: `None` if it has both scripts or neither.
    pub fn script(&self, s: &str) -> Option<Side> {
        match (self.has_alt(s), has_latin_letter(s)) {
            (true, false) => Some(Side::Alt),
            (false, true) => Some(Side::Latin),
            _ => None,
        }
    }
}

/// Text contains an ASCII Latin letter.
pub fn has_latin_letter(s: &str) -> bool {
    s.chars().any(|c| c.is_ascii_alphabetic())
}

fn is_cyrillic(c: char) -> bool {
    ('\u{0400}'..='\u{04FF}').contains(&c)
}

/// Orthographic impossibilities of Russian, no statistics: starts with `ь`/`ы`; `ьь`, `ыы`, `йй`,
/// `щщ`; `ь`/`ы` right after a vowel. A token of one repeated letter (`ЫЫЫЫ`) does not count.
fn is_impossible_russian_spelling(w: &str) -> bool {
    let chars: Vec<char> = w.to_lowercase().chars().collect();
    if chars.len() < 2 || chars.iter().all(|&c| c == chars[0]) {
        return false;
    }
    if chars[0] == 'ь' || chars[0] == 'ы' {
        return true;
    }
    const VOWELS: &str = "аеёиоуыэюя";
    chars.windows(2).any(|p| {
        let (a, b) = (p[0], p[1]);
        (a == b && "ьыйщ".contains(a)) || (VOWELS.contains(a) && (b == 'ь' || b == 'ы'))
    })
}

/// RU↔EN: Russian JCUKEN against US QWERTY.
pub static RU_EN: LanguagePair = LanguagePair {
    latin_lang: "en",
    alt_lang: "ru",
    base_pairs: &[
        ('`', 'ё'), ('q', 'й'), ('w', 'ц'), ('e', 'у'), ('r', 'к'), ('t', 'е'), ('y', 'н'),
        ('u', 'г'), ('i', 'ш'), ('o', 'щ'), ('p', 'з'), ('[', 'х'), (']', 'ъ'),
        ('a', 'ф'), ('s', 'ы'), ('d', 'в'), ('f', 'а'), ('g', 'п'), ('h', 'р'), ('j', 'о'),
        ('k', 'л'), ('l', 'д'), (';', 'ж'), ('\'', 'э'),
        ('z', 'я'), ('x', 'ч'), ('c', 'с'), ('v', 'м'), ('b', 'и'), ('n', 'т'), ('m', 'ь'),
        (',', 'б'), ('.', 'ю'), ('/', '.'),
    ],
    is_alt_char: is_cyrillic,
    latin_single_letters: &["i", "u", "a"],
    is_impossible_alt_spelling: is_impossible_russian_spelling,
    decimal_alias: Some('ю'),
    typo_keep_doubled: &['н'],
    label_bridge_word: "nj",
    files: PairFiles {
        words_latin: include_bytes!("../data/words_en.json"),
        words_alt: include_bytes!("../data/words_ru.json"),
        trigrams_latin: include_bytes!("../data/trigrams_en.json"),
        trigrams_alt: include_bytes!("../data/trigrams_ru.json"),
        typo_rules: include_bytes!("../data/typo_rules.json"),
        extra: include_bytes!("../data/extra_words.json"),
    },
    extra: ExtraKeys {
        latin_words: &["en"],
        alt_words: &["ru", "ruDev", "ruAbbr", "ruShort", "ruCommonForms", "ruLoanNames"],
        latin_prefix_keep: &["en", "enKeepShort"],
        alt_prefix_keep: &["ru", "ruAbbr", "ruShort"],
        force_alt_amb: "forceRuAmb",
        force_latin_amb: "forceEnAmb",
        latin_tech_tokens: "techLatinTokens",
        tech_from_alt: "techFromCyrillic",
        default_keep: "defaultKeep",
        latin_hyphen_terms: "hyphenTerms",
        alt_hyphen_terms: "ruHyphenTerms",
        latin_label_classifiers: "labelClassifiers",
        alt_label_classifiers: "ruLabelClassifiers",
        latin_keep_short: "enKeepShort",
        force_swap: "forceSwap",
        common_latin_two_letter: "commonEnTwoLetter",
        alt_single_letters: "ruSingleLetter",
        ambiguous_pairs: "ambiguousPairs",
    },
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn script_detection() {
        assert_eq!(RU_EN.script("привет"), Some(Side::Alt));
        assert_eq!(RU_EN.script("hello"), Some(Side::Latin));
        assert_eq!(RU_EN.script("приdет"), None);
        assert_eq!(RU_EN.script("123"), None);
    }

    #[test]
    fn impossible_russian_spelling() {
        let f = RU_EN.is_impossible_alt_spelling;
        assert!(f("ЬФСИЩЩЛ"));
        assert!(f("аы"));
        assert!(!f("УФНС"));
        assert!(!f("ЫЫЫЫ"));
        assert!(!f("ГИБДД"));
    }
}
