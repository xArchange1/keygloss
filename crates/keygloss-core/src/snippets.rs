//! Snippets: autoreplace trigger matching (`SnippetStore.swift`), expansion
//! (`Engine.expandSnippet`, `Engine.sanitizeSnippet`) and the picker's digit mapping
//! (`EventTap` digit banks, `SnippetPicker`).

use std::collections::HashMap;

use crate::keymap::Keymap;
use crate::pair::Side;
use crate::platform::{KeyCode, Modifiers};

/// Picker rows addressable by keyboard: 9 digits × 3 banks.
pub const PICKER_KEYBOARD_ROWS: usize = 27;

/// Layout- and case-independent form of a trigger or typed word: converted to the Latin layout
/// key by key, then lowercased. `!test`, `!TEST` and `!еуые` are the same.
pub fn canonical(keymap: &Keymap, s: &str) -> String {
    keymap.convert(s, Side::Latin).to_lowercase()
}

/// Trigger lookup for typed words. Rebuild it when the pairs or the keymap change.
#[derive(Clone, Debug, Default)]
pub struct SnippetIndex {
    by_canonical: HashMap<String, String>,
}

impl SnippetIndex {
    pub fn new(keymap: &Keymap, pairs: &[(String, String)]) -> SnippetIndex {
        let mut by_canonical = HashMap::new();
        for (t, e) in pairs {
            let c = canonical(keymap, t);
            if !c.is_empty() {
                by_canonical.insert(c, e.clone());
            }
        }
        SnippetIndex { by_canonical }
    }

    /// The expansion for a typed word, ignoring layout and case.
    pub fn expansion_for_typed(&self, keymap: &Keymap, word: &str) -> Option<&str> {
        self.by_canonical.get(&canonical(keymap, word)).map(String::as_str)
    }
}

/// Cleans the editor's pairs before saving: triggers trimmed, empty triggers dropped, duplicate
/// triggers (layout-independent) merged at the first position with the last value.
pub fn normalize_pairs(keymap: &Keymap, pairs: &[(String, String)]) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    for (t, e) in pairs {
        let trig = t.trim();
        if trig.is_empty() {
            continue;
        }
        let c = canonical(keymap, trig);
        match out.iter_mut().find(|(o, _)| canonical(keymap, o) == c) {
            Some(existing) => existing.1 = e.clone(),
            None => out.push((trig.to_string(), e.clone())),
        }
    }
    out
}

/// Drops control characters from an expansion, except `\n` and `\t`.
pub fn sanitize(expansion: &str) -> String {
    expansion.chars().filter(|&c| c == '\n' || c == '\t' || c as u32 >= 0x20).collect()
}

/// What to send for an expansion whose boundary key was swallowed: delete the trigger, type the
/// sanitized expansion plus the boundary whitespace (not doubled if the expansion already ends
/// with it). Returns (characters to delete, text to type).
pub fn expansion_output(trigger: &str, expansion: &str, whitespace: &str) -> (usize, String) {
    let body = sanitize(expansion);
    let glue = match body.chars().last() {
        Some(c @ (' ' | '\t')) if whitespace.chars().eq([c]) => "",
        _ => whitespace,
    };
    (trigger.chars().count(), format!("{body}{glue}"))
}

const DIGITS: [KeyCode; 9] = [
    KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3, KeyCode::Digit4, KeyCode::Digit5,
    KeyCode::Digit6, KeyCode::Digit7, KeyCode::Digit8, KeyCode::Digit9,
];

/// Modifiers of the three digit banks: none (rows 1–9), Shift (10–18), Ctrl (19–27; the
/// original used Command, Windows has no free equivalent).
const BANKS: [Modifiers; 3] = [
    Modifiers::NONE,
    Modifiers { shift: true, ctrl: false, alt: false, meta: false },
    Modifiers { ctrl: true, shift: false, alt: false, meta: false },
];

/// Row index (0-based) chosen by a top-row digit key with modifiers while the picker is open.
/// `None` = not a picker key (the picker closes and the key goes to the app). Numpad digits are
/// deliberately not used.
pub fn picker_index(key: KeyCode, mods: Modifiers) -> Option<usize> {
    let digit = DIGITS.iter().position(|&k| k == key)?;
    let bank = BANKS.iter().position(|&b| b == mods)?;
    Some(bank * 9 + digit)
}

/// The key that picks row `index` (0-based), for the row label; `None` past row 27.
pub fn picker_shortcut(index: usize) -> Option<(KeyCode, Modifiers)> {
    (index < PICKER_KEYBOARD_ROWS).then(|| (DIGITS[index % 9], BANKS[index / 9]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pair::RU_EN;

    #[test]
    fn triggers_match_across_layouts_and_case() {
        let k = Keymap::fallback(&RU_EN);
        let idx = SnippetIndex::new(&k, &[("!test".into(), "Tested".into()), ("".into(), "x".into())]);
        for typed in ["!test", "!TEST", "!еуые", "!ЕУЫЕ"] {
            assert_eq!(idx.expansion_for_typed(&k, typed), Some("Tested"), "{typed}");
        }
        assert_eq!(idx.expansion_for_typed(&k, "test"), None);
    }

    #[test]
    fn normalize_merges_duplicates() {
        let k = Keymap::fallback(&RU_EN);
        let pairs = vec![
            (" br ".to_string(), "one".to_string()),
            ("  ".to_string(), "empty".to_string()),
            ("ИК".to_string(), "two".to_string()), // same keys as "br"
            ("x".to_string(), "three".to_string()),
        ];
        assert_eq!(normalize_pairs(&k, &pairs), vec![("br".into(), "two".into()), ("x".into(), "three".into())]);
    }

    #[test]
    fn expansion_is_sanitized_and_glued() {
        assert_eq!(sanitize("a\u{7}b\nc\td"), "ab\nc\td");
        assert_eq!(expansion_output("!br", "Best regards", " "), (3, "Best regards ".into()));
        assert_eq!(expansion_output("!br", "Best regards ", " "), (3, "Best regards ".into()));
        assert_eq!(expansion_output("!br", "Best regards ", "\n"), (3, "Best regards \n".into()));
    }

    #[test]
    fn picker_banks() {
        assert_eq!(picker_index(KeyCode::Digit1, Modifiers::NONE), Some(0));
        assert_eq!(picker_index(KeyCode::Digit9, Modifiers::NONE), Some(8));
        assert_eq!(picker_index(KeyCode::Digit1, BANKS[1]), Some(9));
        assert_eq!(picker_index(KeyCode::Digit9, BANKS[2]), Some(26));
        assert_eq!(picker_index(KeyCode::Digit0, Modifiers::NONE), None);
        assert_eq!(picker_index(KeyCode::Numpad1, Modifiers::NONE), None);
        assert_eq!(picker_index(KeyCode::Digit1, Modifiers { alt: true, ..Modifiers::NONE }), None);
        for i in 0..PICKER_KEYBOARD_ROWS {
            let (k, m) = picker_shortcut(i).unwrap();
            assert_eq!(picker_index(k, m), Some(i));
        }
        assert_eq!(picker_shortcut(27), None);
    }
}
