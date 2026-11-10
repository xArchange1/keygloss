//! Character conversion between the two layouts of a pair (`Keymap.swift`, `DynamicKeymap.swift`).
//!
//! A [`Keymap`] is either the pair's fixed fallback table or a live table built from the two
//! installed layouts' [`KeyTable`]s. Conversion is char-for-char; unknown characters pass through.

use std::collections::HashMap;

use crate::pair::{LanguagePair, Side};
use crate::platform::{KeyCode, KeyTable, LayoutId};

/// Sentence punctuation that is the same in both layouts (on different keys), so it is kept as
/// typed at the end of a word.
pub const TRAILING_PUNCTUATION: &[char] = &['.', ',', '!', '?', ';', ':', '…'];

pub fn is_trailing_punctuation(c: char) -> bool {
    TRAILING_PUNCTUATION.contains(&c)
}

/// The word without trailing [`TRAILING_PUNCTUATION`].
pub fn core(word: &str) -> &str {
    word.trim_end_matches(TRAILING_PUNCTUATION)
}

/// Typographic twin → straight character (’ ‘ ´ ′ → `'`, “ ” ‟ → `"`). Dashes and guillemets are
/// real characters and stay.
pub fn straighten(c: char) -> char {
    match c {
        '\u{2019}' | '\u{2018}' | '\u{00B4}' | '\u{2032}' => '\'',
        '\u{201C}' | '\u{201D}' | '\u{201F}' => '"',
        other => other,
    }
}

const TYPOGRAPHIC_TWINS: &[(char, &[char])] = &[
    ('\'', &['\u{2019}', '\u{2018}', '\u{00B4}', '\u{2032}']),
    ('"', &['\u{201C}', '\u{201D}', '\u{201F}']),
];

fn add_typographic_aliases(map: &mut HashMap<char, char>) {
    for (straight, curly) in TYPOGRAPHIC_TWINS {
        let Some(&target) = map.get(straight) else { continue };
        for &c in *curly {
            map.entry(c).or_insert(target);
        }
    }
}

fn is_ascii_digit(c: char) -> bool {
    c.is_ascii_digit()
}

/// Printable keys in the order the original enumerated macOS key codes 0…50 (the first pair
/// found for a character wins). The ISO key next to left Shift goes last, so it never steals a
/// pair from a main-block key.
const KEY_ORDER: &[KeyCode] = &[
    KeyCode::KeyA, KeyCode::KeyS, KeyCode::KeyD, KeyCode::KeyF, KeyCode::KeyH, KeyCode::KeyG,
    KeyCode::KeyZ, KeyCode::KeyX, KeyCode::KeyC, KeyCode::KeyV, KeyCode::KeyB, KeyCode::KeyQ,
    KeyCode::KeyW, KeyCode::KeyE, KeyCode::KeyR, KeyCode::KeyY, KeyCode::KeyT, KeyCode::Digit1,
    KeyCode::Digit2, KeyCode::Digit3, KeyCode::Digit4, KeyCode::Digit6, KeyCode::Digit5,
    KeyCode::Equal, KeyCode::Digit9, KeyCode::Digit7, KeyCode::Minus, KeyCode::Digit8,
    KeyCode::Digit0, KeyCode::BracketRight, KeyCode::KeyO, KeyCode::KeyU, KeyCode::BracketLeft,
    KeyCode::KeyI, KeyCode::KeyP, KeyCode::KeyL, KeyCode::KeyJ, KeyCode::Quote, KeyCode::KeyK,
    KeyCode::Semicolon, KeyCode::Backslash, KeyCode::Comma, KeyCode::Slash, KeyCode::KeyN,
    KeyCode::KeyM, KeyCode::Period, KeyCode::Space, KeyCode::Backquote, KeyCode::IntlBackslash,
];

/// Conversion tables for one pair, in both directions.
#[derive(Clone, Debug)]
pub struct Keymap {
    to_alt: HashMap<char, char>,
    to_latin: HashMap<char, char>,
    live: bool,
}

impl Keymap {
    /// The pair's fixed table (`Keymap.basePairs`): letters also get their uppercase pair.
    /// [`Keymap::convert`] maps typographic quotes like the straight ones; [`Keymap::map`] does
    /// not (the detector's "is this key a letter" test relies on that).
    pub fn fallback(pair: &LanguagePair) -> Keymap {
        let mut to_alt = HashMap::new();
        let mut to_latin = HashMap::new();
        for &(l, a) in pair.base_pairs {
            to_alt.insert(l, a);
            to_latin.insert(a, l);
            if l.is_alphabetic() {
                let (lu, au) = (upper(l), upper(a));
                to_alt.insert(lu, au);
                to_latin.insert(au, lu);
            }
        }
        Keymap { to_alt, to_latin, live: false }
    }

    /// Live table from the user's two installed layouts (`DynamicKeymap.buildTables`): for every
    /// printable key, unshifted and shifted, the Latin and Alt characters become a pair when they
    /// differ. `None` if no pair was found.
    pub fn from_key_tables(latin: &KeyTable, alt: &KeyTable) -> Option<Keymap> {
        let mut to_alt = HashMap::new();
        let mut to_latin = HashMap::new();
        for &key in KEY_ORDER {
            for shift in [false, true] {
                let (Some(&l), Some(&a)) = (latin.get(&(key, shift)), alt.get(&(key, shift))) else {
                    continue;
                };
                if l == a {
                    continue;
                }
                to_alt.entry(l).or_insert(a);
                to_latin.entry(a).or_insert(l);
            }
        }
        if to_alt.is_empty() {
            return None;
        }
        add_typographic_aliases(&mut to_alt);
        Some(Keymap { to_alt, to_latin, live: true })
    }

    /// True for a table built from the real layouts, false for the fallback.
    pub fn is_live(&self) -> bool {
        self.live
    }

    /// The character this key gives in the `to` layout, if the table knows it.
    pub fn map(&self, c: char, to: Side) -> Option<char> {
        match to {
            Side::Alt => self.to_alt.get(&c).copied(),
            Side::Latin => self.to_latin.get(&c).copied(),
        }
    }

    /// Converts `text` to the `to` layout. A comma or period between two ASCII digits is a
    /// number separator, not a key: towards Alt it stays as typed; towards Latin a comma becomes
    /// `.` only when it is the number's single separator (`5,5` → `5.5`, `1,000,000` stays).
    pub fn convert(&self, text: &str, to: Side) -> String {
        let chars: Vec<char> = text.chars().collect();
        let mut out = String::with_capacity(text.len());
        let mut run_has_separator = false;
        for (i, &ch) in chars.iter().enumerate() {
            let between_digits = (ch == ',' || ch == '.')
                && i > 0
                && is_ascii_digit(chars[i - 1])
                && chars.get(i + 1).is_some_and(|&n| is_ascii_digit(n));
            if between_digits {
                out.push(number_separator(ch, to, run_has_separator, &chars[i + 2..]));
                run_has_separator = true;
            } else {
                let mapped = self.map(ch, to).or_else(|| if self.live { None } else { self.map(straighten(ch), to) });
                out.push(mapped.unwrap_or(ch));
                if !is_ascii_digit(ch) {
                    run_has_separator = false;
                }
            }
        }
        out
    }

    /// Converts a word but keeps trailing sentence punctuation: `ghbdtn.` → `привет.`. Towards
    /// Alt, if the whole token converts to a valid word (`is_valid_target`, lowercase), the
    /// punctuation keys were letters: `yj;` → `нож`.
    pub fn smart_convert(&self, word: &str, to: Side, is_valid_target: Option<&dyn Fn(&str) -> bool>) -> String {
        if to == Side::Alt
            && let Some(valid) = is_valid_target
        {
            let full = self.convert(word, Side::Alt);
            if valid(&full.to_lowercase()) {
                return full;
            }
        }
        let mut core: Vec<char> = word.chars().collect();
        let mut trailing: Vec<char> = Vec::new();
        while let Some(&last) = core.last() {
            if !is_trailing_punctuation(last) {
                break;
            }
            trailing.insert(0, self.peeled_mark(last, to));
            core.pop();
        }
        if core.is_empty() {
            return word.to_string();
        }
        let core: String = core.into_iter().collect();
        self.convert(&core, to) + &trailing.into_iter().collect::<String>()
    }

    /// A peeled trailing mark changes only towards Alt, and only when this table maps it to
    /// another trailing mark (`Lf?` → `Да,` on Russian-PC-like layouts).
    fn peeled_mark(&self, p: char, to: Side) -> char {
        if to != Side::Alt {
            return p;
        }
        let mapped = self.convert(&p.to_string(), Side::Alt);
        let mut it = mapped.chars();
        match (it.next(), it.next()) {
            (Some(m), None) if m != p && is_trailing_punctuation(m) => m,
            _ => p,
        }
    }

    /// Conversion for mid-word (live) fixing. `None` means "wait for the next key": the word
    /// ends in a punctuation key that is a letter in the target layout (`gthtrk.` is an
    /// unfinished `переклю…`, not `перекл.`).
    pub fn live_convert(&self, word: &str, to: Side, is_valid_target: Option<&dyn Fn(&str) -> bool>) -> Option<String> {
        let smart = self.smart_convert(word, to, is_valid_target);
        if smart == word {
            return Some(smart);
        }
        let full = self.convert(word, to);
        if smart == full {
            return Some(smart);
        }
        if smart.chars().zip(full.chars()).any(|(s, f)| s != f && f.is_alphabetic()) {
            return None;
        }
        Some(smart)
    }

    /// Direction for converting a run of symbols with no letters (manual conversion only): a
    /// symbol that exists on only one side tells which layout it was typed in. Only on a live
    /// table; `None` if empty, has letters, all symbols ambiguous, or they pull both ways.
    pub fn symbol_direction(&self, text: &str) -> Option<Side> {
        if !self.live {
            return None;
        }
        let (mut to_alt, mut to_latin) = (false, false);
        for ch in text.chars() {
            if ch.is_alphabetic() {
                return None;
            }
            if ch.is_whitespace() || ch.is_numeric() {
                continue;
            }
            let c = straighten(ch);
            let (latin, alt) = (self.to_alt.contains_key(&c), self.to_latin.contains_key(&c));
            if latin && !alt {
                to_alt = true;
            } else if alt && !latin {
                to_latin = true;
            }
        }
        if to_alt == to_latin {
            return None;
        }
        Some(if to_alt { Side::Alt } else { Side::Latin })
    }
}

fn upper(c: char) -> char {
    let mut u = c.to_uppercase();
    match (u.next(), u.next()) {
        (Some(x), None) => x,
        _ => c,
    }
}

/// `rest` starts right after the digit that follows the separator.
fn number_separator(ch: char, to: Side, left_has_separator: bool, rest: &[char]) -> char {
    if to == Side::Alt || ch == '.' || left_has_separator {
        return ch;
    }
    let mut i = 0;
    while let Some(&c) = rest.get(i) {
        i += 1;
        if is_ascii_digit(c) {
            continue;
        }
        if (c == ',' || c == '.') && rest.get(i).is_some_and(|&a| is_ascii_digit(a)) {
            return ch;
        }
        break;
    }
    '.'
}

/// Which side a layout belongs to, judged by the unshifted `KeyA` character.
pub fn classify_layout(pair: &LanguagePair, table: &KeyTable) -> Option<Side> {
    let c = *table.get(&(KeyCode::KeyA, false))?;
    if (pair.is_alt_char)(c) {
        Some(Side::Alt)
    } else if c.is_ascii_alphabetic() {
        Some(Side::Latin)
    } else {
        None
    }
}

/// Picks the (Latin, Alt) layouts to pair from installed layouts in OS order, preferring the
/// ones the user last used (`lastLayoutLat` / `lastLayoutCyr`; empty = no preference).
/// Same tie-breaking as the original: without a preference the last candidate of a side wins,
/// with an unmet preference the first one does.
pub fn choose_layouts(
    candidates: &[(LayoutId, Side)],
    prefer_latin: &str,
    prefer_alt: &str,
) -> Option<(LayoutId, LayoutId)> {
    let pick = |side: Side, want: &str| -> Option<LayoutId> {
        let mut exact = None;
        let mut fallback: Option<&LayoutId> = None;
        for (id, _) in candidates.iter().filter(|(_, s)| *s == side) {
            if id.0 == want {
                exact = Some(id);
            } else if exact.is_none() && (want.is_empty() || fallback.is_none()) {
                fallback = Some(id);
            }
        }
        exact.or(fallback).cloned()
    };
    Some((pick(Side::Latin, prefer_latin)?, pick(Side::Alt, prefer_alt)?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pair::RU_EN;

    fn fb() -> Keymap {
        Keymap::fallback(&RU_EN)
    }

    #[test]
    fn converts_both_ways() {
        assert_eq!(fb().convert("ghbdtn", Side::Alt), "привет");
        assert_eq!(fb().convert("руддщ", Side::Latin), "hello");
        assert_eq!(fb().convert("Ghbdtn", Side::Alt), "Привет");
        assert_eq!(fb().convert("№1", Side::Latin), "№1");
    }

    #[test]
    fn typographic_quotes_alias_straight() {
        assert_eq!(fb().convert("\u{2019}nj", Side::Alt), "это");
        assert_eq!(fb().convert("'nj", Side::Alt), "это");
    }

    #[test]
    fn number_separators() {
        let k = fb();
        assert_eq!(k.convert("5,5", Side::Latin), "5.5");
        assert_eq!(k.convert("5.5", Side::Alt), "5.5");
        assert_eq!(k.convert("17.2", Side::Alt), "17.2");
        assert_eq!(k.convert("1,000,000", Side::Latin), "1,000,000");
        assert_eq!(k.convert("10,5,7", Side::Latin), "10,5,7");
        assert_eq!(k.convert("1.000,50", Side::Latin), "1.000,50");
        assert_eq!(k.convert("25.09.2026", Side::Latin), "25.09.2026");
        // Not between digits: an ordinary key.
        assert_eq!(k.convert("5,", Side::Alt), "5б");
    }

    #[test]
    fn smart_convert_keeps_trailing_punctuation() {
        let k = fb();
        let valid = |w: &str| w == "нож" || w == "хлеб";
        assert_eq!(k.smart_convert("ghbdtn.", Side::Alt, Some(&valid)), "привет.");
        assert_eq!(k.smart_convert("yj;", Side::Alt, Some(&valid)), "нож");
        assert_eq!(k.smart_convert("[kt,", Side::Alt, Some(&valid)), "хлеб");
        assert_eq!(k.smart_convert("...", Side::Alt, Some(&valid)), "...");
    }

    #[test]
    fn live_convert_waits_on_letter_punctuation() {
        let k = fb();
        assert_eq!(k.live_convert("gthtrk.", Side::Alt, None), None);
        assert_eq!(k.live_convert("gthtrk.x", Side::Alt, None).as_deref(), Some("переключ"));
        assert_eq!(k.live_convert("ghbdtn!", Side::Alt, None).as_deref(), Some("привет!"));
    }

    fn table(pairs: &[(KeyCode, bool, char)]) -> KeyTable {
        pairs.iter().map(|&(k, s, c)| ((k, s), c)).collect()
    }

    #[test]
    fn live_table_from_key_tables() {
        let lat = table(&[
            (KeyCode::KeyA, false, 'a'), (KeyCode::KeyA, true, 'A'), (KeyCode::Quote, false, '\''),
            (KeyCode::Slash, false, '/'), (KeyCode::Slash, true, '?'), (KeyCode::Digit1, false, '1'),
        ]);
        let ru = table(&[
            (KeyCode::KeyA, false, 'ф'), (KeyCode::KeyA, true, 'Ф'), (KeyCode::Quote, false, 'э'),
            (KeyCode::Slash, false, '.'), (KeyCode::Slash, true, ','), (KeyCode::Digit1, false, '1'),
        ]);
        let k = Keymap::from_key_tables(&lat, &ru).unwrap();
        assert!(k.is_live());
        assert_eq!(k.convert("Aa'", Side::Alt), "Ффэ");
        assert_eq!(k.convert("\u{2018}", Side::Alt), "э");
        assert_eq!(k.convert("фэ", Side::Latin), "a'");
        assert_eq!(classify_layout(&RU_EN, &ru), Some(Side::Alt));
        assert_eq!(classify_layout(&RU_EN, &lat), Some(Side::Latin));
        // `?` exists only on the Latin side; `.` is typed on both.
        assert_eq!(k.symbol_direction("?"), Some(Side::Alt));
        assert_eq!(k.symbol_direction("a?"), None);
        assert_eq!(fb().symbol_direction("?"), None);
        // Russian-PC: trailing `?` is a comma on the Alt side.
        assert_eq!(k.smart_convert("a?", Side::Alt, None), "ф,");
        assert!(Keymap::from_key_tables(&lat, &lat).is_none());
    }

    #[test]
    fn choose_layouts_prefers_last_used() {
        let id = |s: &str| LayoutId(s.into());
        let c = vec![(id("en1"), Side::Latin), (id("ru1"), Side::Alt), (id("en2"), Side::Latin), (id("ru2"), Side::Alt)];
        assert_eq!(choose_layouts(&c, "en1", "ru2"), Some((id("en1"), id("ru2"))));
        assert_eq!(choose_layouts(&c, "", ""), Some((id("en2"), id("ru2"))));
        assert_eq!(choose_layouts(&c, "x", "y"), Some((id("en1"), id("ru1"))));
        assert_eq!(choose_layouts(&c[..1], "", ""), None);
    }
}
