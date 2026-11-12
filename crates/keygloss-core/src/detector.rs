//! Wrong-layout detection (`LayoutDetector.swift`) and the pure decision helpers the engine calls
//! around it (`Engine.autoConversionProposal`, `convertGroup`, live-heal, manual and selection
//! direction).

use std::collections::HashSet;
use std::time::Duration;

use crate::data::LangData;
use crate::exceptions::Exceptions;
use crate::keymap::{self, Keymap};
use crate::pair::{Side, has_latin_letter};

/// Trigram score lead the swapped form needs over the typed one.
pub const MARGIN: f64 = 2.0;
/// Plausibility at which the Latin form of a short Alt fragment counts as a real word.
pub const SHORT_EN_SWAP_FLOOR: f64 = -10.0;
/// Live fix: the typed form must score at most this (practically impossible) …
pub const LIVE_IMPOSSIBLE: f64 = -13.0;
/// … and the swapped form must beat it by more than this.
pub const LIVE_MARGIN: f64 = 6.0;
/// Soft mode: a word with an inner pause longer than this was commands, not typing.
pub const SOFT_MAX_INTRA_WORD_GAP: Duration = Duration::from_secs(1);
/// Live fix runs on words of at least this many characters …
pub const LIVE_MIN_LEN: usize = 4;
/// … and the inline (in-hook) path on at most this many.
pub const INLINE_LIVE_MAX_LEN: usize = 16;
/// Clipboard-read selections longer than this (or multi-line) are refused (likely a whole line
/// copied by Ctrl+C with no selection).
pub const CLIPBOARD_SELECTION_MAX_CHARS: usize = 80;

/// What to do with a word.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decision {
    Keep,
    /// Convert into the given layout.
    Convert(Side),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ContextHint {
    None,
    Alt,
    Latin,
}

/// Inputs of [`Detector::propose`] that come from the engine state.
#[derive(Clone, Copy, Debug, Default)]
pub struct ProposalContext<'a> {
    /// Previous word of the phrase as on screen ([`crate::buffer::WordBuffer::context_word`]).
    pub prev: Option<&'a str>,
    /// The word before that ([`crate::buffer::WordBuffer::earlier_context_word`]).
    pub earlier: Option<&'a str>,
    /// First word after the caret was moved by mouse or navigation (already adjusted by the
    /// engine's caret probe).
    pub after_caret_jump: bool,
    /// The front app is in soft mode.
    pub soft: bool,
    /// Longest pause between letters of the word.
    pub intra_word_gap: Duration,
    /// The undo learner protects this word in the current context.
    pub session_protected: bool,
}

/// An automatic conversion to apply.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Proposal {
    /// Replacement text for the whole token (trailing punctuation kept).
    pub text: String,
    pub to: Side,
    /// Mixed-script rescue (not offered to the undo learner).
    pub rescue: bool,
}

/// Result of [`Detector::plan_group`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GroupPlan {
    /// Words and tails, with the converted words replaced. Same length as the group.
    pub text: String,
    /// How many words were converted.
    pub converted: usize,
    /// Layout of the last converted word (select it afterwards).
    pub last_to: Side,
}

/// Result of [`Detector::live_heal`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LiveHeal {
    /// Characters to delete (the raw Latin tail).
    pub delete: usize,
    /// Text to type in their place.
    pub text: String,
    /// The whole word after the heal.
    pub word: String,
}

/// The detector over one pair's data, the active keymap and the user's exceptions.
pub struct Detector<'a> {
    pub data: &'a LangData,
    pub keymap: &'a Keymap,
    pub exceptions: &'a Exceptions,
}

fn count(s: &str) -> usize {
    s.chars().count()
}

/// Collapses runs of 3+ equal letters: `круууто` → [`круто`], `класссно` → [`классно`, `класно`].
/// `None` if nothing is stretched.
pub fn de_elongated(s: &str) -> Option<Vec<String>> {
    let (mut run, mut prev, mut stretched) = (0, None, false);
    let (mut to_two, mut to_one) = (String::new(), String::new());
    for c in s.chars() {
        if Some(c) == prev {
            run += 1;
        } else {
            run = 1;
            prev = Some(c);
        }
        if run >= 3 {
            stretched = true;
        }
        if run <= 2 {
            to_two.push(c);
        }
        if run == 1 {
            to_one.push(c);
        }
    }
    if !stretched {
        return None;
    }
    Some(if to_two == to_one { vec![to_one] } else { vec![to_two, to_one] })
}

/// Western emoticon (`:D`, `:-P`, `;D`, `XD`, `=)`), optionally followed by sentence punctuation.
pub fn is_emoticon(raw: &str) -> bool {
    let s = raw.trim_end_matches(['.', ',', '!', '?', '…']);
    let mut chars: Vec<char> = s.chars().collect();
    if chars.is_empty() {
        return false;
    }
    let eyes = chars.remove(0);
    if chars.len() > 1 && "-~^'".contains(chars[0]) {
        chars.remove(0);
    }
    if chars.len() != 1 {
        return false;
    }
    let mouth = chars[0];
    let mouth_symbol = ")(|/\\*$@[]{}3".contains(mouth);
    match eyes {
        ':' | '=' => mouth.is_alphabetic() || mouth_symbol,
        ';' | '8' => (mouth.is_alphabetic() && mouth.is_uppercase()) || mouth_symbol,
        'X' | 'x' => mouth == 'D' || mouth == 'd',
        _ => false,
    }
}

/// A token of digits and `,.-–/` with at least one digit (`5`, `1,5`, `2024`).
pub fn is_numeric_token(s: Option<&str>) -> bool {
    let Some(s) = s.filter(|s| !s.is_empty()) else { return false };
    let mut has_digit = false;
    for ch in s.chars() {
        if ch.is_numeric() {
            has_digit = true;
        } else if !",.-–/".contains(ch) {
            return false;
        }
    }
    has_digit
}

/// Trailing run of ASCII Latin letters.
fn trailing_latin_run(w: &str) -> &str {
    let start = w
        .char_indices()
        .rev()
        .take_while(|(_, c)| c.is_ascii_alphabetic())
        .last()
        .map_or(w.len(), |(i, _)| i);
    &w[start..]
}

fn trim_chars(s: &str, mut drop: impl FnMut(char) -> bool) -> &str {
    s.trim_start_matches(&mut drop).trim_end_matches(drop)
}

impl<'a> Detector<'a> {
    fn pair(&self) -> &'static crate::pair::LanguagePair {
        self.data.pair
    }

    fn context_of(&self, word: Option<&str>) -> ContextHint {
        match word {
            Some(w) if self.pair().has_alt(w) => ContextHint::Alt,
            Some(w) if has_latin_letter(w) => ContextHint::Latin,
            _ => ContextHint::None,
        }
    }

    /// A letter, or a key whose character in the other layout is a letter (`[` = х, `;` = ж).
    pub fn is_layout_letter(&self, c: char) -> bool {
        let b = &self.data.base;
        c.is_alphabetic()
            || b.map(c, Side::Alt).is_some_and(char::is_alphabetic)
            || b.map(c, Side::Latin).is_some_and(char::is_alphabetic)
    }

    /// Drops leading and trailing characters that are neither layout letters nor digits:
    /// `(tckb` → `tckb`, `„если"` → `если`.
    pub fn letter_core<'s>(&self, raw: &'s str) -> &'s str {
        trim_chars(raw, |c| !self.is_layout_letter(c) && !c.is_numeric())
    }

    /// Shape of a real hyphenated word; `None` for minus signs, ranges, `--flags`, dangling or
    /// double hyphens and identifiers with digits.
    fn hyphen_linguistic_core<'s>(&self, raw: &'s str) -> Option<&'s str> {
        let edge = |c: char| self.is_layout_letter(c) || c.is_numeric();
        let first = raw.find(edge)?;
        let (last, last_ch) = raw.char_indices().rev().find(|&(_, c)| edge(c))?;
        let end = last + last_ch.len_utf8();
        if raw[..first].contains('-') || raw[end..].contains('-') {
            return None;
        }
        let inner = &raw[first..end];
        let (mut segment, mut segments) = (0, 0);
        for c in inner.chars() {
            if c == '-' {
                if segment == 0 {
                    return None;
                }
                segment = 0;
                segments += 1;
            } else {
                if !self.is_layout_letter(c) {
                    return None;
                }
                segment += 1;
            }
        }
        (segments >= 1 && segment > 0).then_some(inner)
    }

    /// The Alt token converts to `swapped` exactly by the pair's base table.
    fn is_base_swap(&self, w: &str, swapped: &str) -> bool {
        let mut out = String::new();
        for ch in w.chars() {
            match self.data.base.map(ch, Side::Latin) {
                Some(m) => out.push(m),
                None => return false,
            }
        }
        out.to_lowercase() == swapped
    }

    fn is_tech_token_from_alt(&self, w: &str, swapped: &str) -> bool {
        self.data.extra.tech_from_alt.contains(swapped) && self.is_base_swap(w, swapped)
    }

    /// A real word before trailing punctuation beats reading the punctuation keys as letters of
    /// the other layout (`it.` is not `шею`).
    fn has_valid_source_before_trailing_punctuation(&self, raw_core: &str, forced_source: &HashSet<String>) -> bool {
        let whole = raw_core.to_lowercase();
        let semantic = keymap::core(&whole);
        if semantic == whole || count(semantic) < 2 {
            return false;
        }
        let Some(side) = self.pair().script(semantic) else { return false };
        let e = &self.data.extra;
        if e.force_swap.contains(semantic) || forced_source.contains(semantic) {
            return true;
        }
        if side == Side::Latin && e.latin_tech_tokens.contains(semantic) {
            return true;
        }
        if side == Side::Latin && e.force_alt_amb.contains(semantic) {
            return false;
        }
        let words = self.data.words(side);
        words.contains(semantic) || de_elongated(semantic).is_some_and(|v| v.iter().any(|x| words.contains(x)))
    }

    /// `w` is a keep-word or a prefix of one (live fix only: do not flip a word the user is still
    /// typing towards an exception).
    fn is_exception_or_prefix(&self, w: &str, alt: bool) -> bool {
        if count(w) < 2 {
            return false;
        }
        let hit = |set: &HashSet<String>| set.contains(w) || set.iter().any(|e| e.len() > w.len() && e.starts_with(w));
        let (exc, e) = (self.exceptions, &self.data.extra);
        if hit(&exc.ignored) || hit(&e.default_keep) {
            return true;
        }
        if alt { hit(&e.alt_prefix_keep) } else { hit(&e.latin_prefix_keep) }
    }

    /// Strict mid-word decision: the typed form must be practically impossible and the swapped
    /// form much better. No dictionary guessing.
    pub fn live_decide(&self, raw: &str) -> Decision {
        let letter_core = self.letter_core(raw);
        let typed = keymap::core(letter_core).to_lowercase();
        let core = trim_chars(letter_core, char::is_numeric);
        let w = core.to_lowercase();
        if typed != w && self.is_exception_or_prefix(&typed, self.pair().has_alt(&typed)) {
            return Decision::Keep;
        }
        if self.has_valid_source_before_trailing_punctuation(core, &self.exceptions.force_swap) {
            return Decision::Keep;
        }
        if count(&w) < 4 || !w.chars().all(|c| self.is_layout_letter(c)) {
            return Decision::Keep;
        }
        let Some(source) = self.pair().script(&w) else { return Decision::Keep };
        let to = source.other();
        let swapped = self.keymap.convert(core, to).to_lowercase();
        if swapped == w || !swapped.chars().all(char::is_alphabetic) {
            return Decision::Keep;
        }
        if self.data.words(source).contains(&w) {
            return Decision::Keep;
        }
        if self.is_exception_or_prefix(&w, source == Side::Alt) {
            return Decision::Keep;
        }
        let orig = self.data.plausibility(&w, source);
        let swap = self.data.plausibility(&swapped, to);
        if orig <= LIVE_IMPOSSIBLE && swap > orig + LIVE_MARGIN {
            return Decision::Convert(to);
        }
        Decision::Keep
    }

    /// A token with both scripts (an artefact of a mid-word flip): convert the whole token if
    /// exactly one direction gives a dictionary word.
    pub fn mixed_rescue(&self, raw: &str) -> Decision {
        let pair = self.pair();
        if !pair.has_alt(raw) || !has_latin_letter(raw) {
            return Decision::Keep;
        }
        let core = self.letter_core(raw);
        if count(core) < 2 || !core.chars().all(|c| self.is_layout_letter(c)) {
            return Decision::Keep;
        }
        let to_alt = self.keymap.convert(core, Side::Alt);
        let to_latin = self.keymap.convert(core, Side::Latin);
        let alt_ok = !has_latin_letter(&to_alt) && self.data.words_alt.contains(&to_alt.to_lowercase());
        let latin_ok = !pair.has_alt(&to_latin) && self.data.words_latin.contains(&to_latin.to_lowercase());
        match (alt_ok, latin_ok) {
            (true, false) => Decision::Convert(Side::Alt),
            (false, true) => Decision::Convert(Side::Latin),
            _ => Decision::Keep,
        }
    }

    /// `… касается iOS, nj`: the bridge word after a Latin abbreviation with a comma continues an
    /// Alt phrase.
    fn label_bridge(&self, word: &str, raw_core: &str, prev: Option<&str>, earlier: Option<&str>) -> bool {
        let bridge = self.pair().label_bridge_word;
        if bridge.is_empty() || word != bridge || raw_core != word {
            return false;
        }
        let (Some(prev), Some(earlier)) = (prev, earlier) else { return false };
        let Some(label) = prev.strip_suffix(',') else { return false };
        if !self.pair().has_alt(earlier) || has_latin_letter(earlier) {
            return false;
        }
        !label.is_empty()
            && label.chars().all(|c| c.is_ascii_alphabetic())
            && label.chars().filter(|c| c.is_uppercase()).count() >= 2
    }

    /// Boundary (automatic) decision for a finished word. `prev`/`earlier` are the previous words
    /// of the phrase as on screen; `after_caret_jump` marks the first word after a click or
    /// navigation.
    pub fn decide(&self, raw: &str, prev: Option<&str>, earlier: Option<&str>, after_caret_jump: bool) -> Decision {
        let (pair, data, e, exc) = (self.pair(), self.data, &self.data.extra, self.exceptions);
        let context = self.context_of(prev);
        let letter_core = self.letter_core(raw);
        let literal = letter_core.to_lowercase();
        let typed = keymap::core(letter_core).to_lowercase();
        let whole = raw.to_lowercase();
        if [&literal, &typed, &whole].iter().any(|k| exc.is_kept(k)) {
            return Decision::Keep;
        }
        if is_emoticon(raw) {
            return Decision::Keep;
        }
        let had_digits = letter_core.chars().any(char::is_numeric);
        let core_raw = if had_digits { trim_chars(letter_core, char::is_numeric) } else { letter_core };
        let w = core_raw.to_lowercase();

        if w.contains('-') {
            return self.decide_hyphenated(raw, &w);
        }
        if count(&w) < if had_digits { 4 } else { 1 } || !w.chars().all(|c| self.is_layout_letter(c)) {
            return Decision::Keep;
        }
        if exc.is_kept(&w) || e.default_keep.contains(&w) {
            return Decision::Keep;
        }
        let Some(source) = pair.script(&w) else { return Decision::Keep };
        let to = source.other();
        let (latin, alt) = (source == Side::Latin, source == Side::Alt);
        let swapped = self.keymap.convert(core_raw, to).to_lowercase();

        // Comma, period and semicolon typed on the Alt layout arrive as letters (б, ю, ж): decide
        // on the word without them, unless the token itself is a real Alt word.
        if alt
            && !data.words_alt.contains(&w)
            && swapped.chars().last().is_some_and(keymap::is_trailing_punctuation)
        {
            let mut trimmed = core_raw;
            while let Some(l) = trimmed.chars().last() {
                let mapped = self.keymap.convert(&l.to_string(), Side::Latin);
                if !mapped.chars().last().is_some_and(keymap::is_trailing_punctuation) {
                    break;
                }
                trimmed = &trimmed[..trimmed.len() - l.len_utf8()];
            }
            if trimmed.is_empty() {
                return Decision::Keep;
            }
            return self.decide(trimmed, prev, earlier, after_caret_jump);
        }
        if swapped == w || !swapped.chars().all(|c| c.is_alphabetic() || c == '\'') {
            return Decision::Keep;
        }

        let is_real = |x: &str| data.in_dictionaries(x);
        let source_is_real_word = self.has_valid_source_before_trailing_punctuation(core_raw, &exc.force_swap)
            || is_real(&w)
            || de_elongated(&w).is_some_and(|v| v.iter().any(|x| is_real(x)));
        if e.force_swap.contains(&swapped) && !source_is_real_word {
            return Decision::Convert(to);
        }
        if alt && !source_is_real_word && !after_caret_jump && self.is_tech_token_from_alt(&w, &swapped) {
            return Decision::Convert(Side::Latin);
        }
        if exc.force_swap.contains(&swapped) {
            return Decision::Convert(to);
        }
        if e.force_swap.contains(&w) || exc.force_swap.contains(&w) {
            return Decision::Keep;
        }
        if latin && e.latin_tech_tokens.contains(&w) {
            return Decision::Keep;
        }
        // Strict gate: a word valid in the language it was typed in is never converted.
        if count(&w) >= 2 && source_is_real_word && !(latin && e.force_alt_amb.contains(&w)) {
            return Decision::Keep;
        }
        // A capital in mid-phrase is a name.
        if prev.is_some() {
            let mut cs = core_raw.chars();
            if cs.next().is_some_and(char::is_uppercase) && cs.any(char::is_lowercase) {
                return Decision::Keep;
            }
        }

        if count(&w) == 1 {
            if after_caret_jump && context == ContextHint::None {
                return Decision::Keep;
            }
            if latin && e.alt_single_letters.contains(&swapped) {
                if let Some(p) = prev.map(str::to_lowercase) {
                    if context == ContextHint::Latin && e.latin_label_classifiers.contains(&p) {
                        return Decision::Keep;
                    }
                    if context == ContextHint::Alt && e.alt_label_classifiers.contains(&p) {
                        return Decision::Keep;
                    }
                }
                return Decision::Convert(Side::Alt);
            }
            if alt && context != ContextHint::Alt && pair.latin_single_letters.contains(&swapped.as_str()) {
                if is_numeric_token(prev) {
                    return Decision::Keep;
                }
                return Decision::Convert(Side::Latin);
            }
            return Decision::Keep;
        }

        let en_swap_not_junk = || {
            if count(&w) == 2 {
                return e.common_latin_two_letter.contains(&swapped);
            }
            count(&w) >= 4 || data.plausibility(&swapped, Side::Latin) > SHORT_EN_SWAP_FLOOR
        };
        if latin {
            if data.words_latin.contains(&w) && !e.force_alt_amb.contains(&w) {
                if context == ContextHint::Alt && data.words_alt.contains(&swapped) && !e.latin_keep_short.contains(&w) {
                    return Decision::Convert(Side::Alt);
                }
                return Decision::Keep;
            }
            if data.words_alt.contains(&swapped) {
                if context == ContextHint::Latin
                    && e.latin_keep_short.contains(&w)
                    && !self.label_bridge(&w, core_raw, prev, earlier)
                {
                    return Decision::Keep;
                }
                return Decision::Convert(Side::Alt);
            }
        } else {
            if e.force_latin_amb.contains(&swapped) && !data.words_alt.contains(&w) {
                return Decision::Convert(Side::Latin);
            }
            if data.words_alt.contains(&w) {
                if context == ContextHint::Latin && data.words_latin.contains(&swapped) {
                    return Decision::Convert(Side::Latin);
                }
                return Decision::Keep;
            }
            if data.words_latin.contains(&swapped) && en_swap_not_junk() {
                return Decision::Convert(Side::Latin);
            }
        }

        if count(&w) < 4 {
            return Decision::Keep;
        }
        // An all-caps Alt token is an abbreviation, unless its spelling is impossible in the Alt
        // language.
        if alt
            && !core_raw.chars().any(char::is_lowercase)
            && core_raw.chars().filter(|c| c.is_alphabetic()).count() >= 2
            && !(pair.is_impossible_alt_spelling)(&w)
        {
            return Decision::Keep;
        }
        let orig = data.plausibility(&w, source);
        let swap = data.plausibility(&swapped, to);
        if swap > orig + MARGIN || (orig <= -19.0 && swap > orig + 1.0) {
            return Decision::Convert(to);
        }
        Decision::Keep
    }

    fn decide_hyphenated(&self, raw: &str, w: &str) -> Decision {
        let (pair, data, e) = (self.pair(), self.data, &self.data.extra);
        let Some(hyphen_raw) = self.hyphen_linguistic_core(raw) else { return Decision::Keep };
        let hw = hyphen_raw.to_lowercase();
        if hw != w {
            return Decision::Keep;
        }
        let Some(source) = pair.script(&hw) else { return Decision::Keep };
        let to = source.other();
        let swapped = self.keymap.convert(hyphen_raw, to).to_lowercase();
        let term = |x: &str| e.latin_hyphen_terms.contains(x) || e.alt_hyphen_terms.contains(x);
        if term(&hw) {
            return Decision::Keep;
        }
        if term(&swapped) {
            return Decision::Convert(to);
        }
        let src: Vec<&str> = hw.split('-').collect();
        let dst: Vec<&str> = swapped.split('-').collect();
        if src.len() == dst.len()
            && src.iter().all(|s| count(s) >= 2)
            && dst.iter().all(|s| count(s) >= 2 && s.chars().all(char::is_alphabetic))
        {
            if src.iter().all(|s| data.words(source).contains(*s)) {
                return Decision::Keep;
            }
            if dst.iter().all(|s| data.words(to).contains(*s)) {
                return Decision::Convert(to);
            }
        }
        Decision::Keep
    }

    /// Is `w` (lowercase) a valid Alt word? The `is_valid_target` of [`Keymap::smart_convert`].
    pub fn is_valid_alt(&self, w: &str) -> bool {
        self.data.words_alt.contains(w)
    }

    fn smart_convert(&self, word: &str, to: Side) -> String {
        self.keymap.smart_convert(word, to, Some(&|w: &str| self.is_valid_alt(w)))
    }

    /// Mid-word conversion text, `None` = wait for the next key (see [`Keymap::live_convert`]).
    pub fn live_convert(&self, word: &str, to: Side) -> Option<String> {
        self.keymap.live_convert(word, to, Some(&|w: &str| self.is_valid_alt(w)))
    }

    /// The automatic decision for a finished word (boundary and pre-Enter paths): session
    /// protection, mixed-script rescue, [`Detector::decide`], the soft-mode filter, then the
    /// replacement text. `None` = leave the word alone. The engine still checks data readiness
    /// and that the current layout belongs to the pair.
    pub fn propose(&self, word: &str, ctx: &ProposalContext) -> Option<Proposal> {
        if ctx.session_protected {
            return None;
        }
        if let Decision::Convert(to) = self.mixed_rescue(word) {
            return Some(Proposal { text: self.keymap.convert(word, to), to, rescue: true });
        }
        let Decision::Convert(to) = self.decide(word, ctx.prev, ctx.earlier, ctx.after_caret_jump) else {
            return None;
        };
        if ctx.soft {
            let core = keymap::core(word).to_lowercase();
            let mut chars = core.chars();
            let first = chars.next();
            if count(&core) <= 2 || chars.all(|c| Some(c) == first) {
                return None;
            }
            if ctx.intra_word_gap > SOFT_MAX_INTRA_WORD_GAP {
                return None;
            }
        }
        let text = self.smart_convert(word, to);
        (text != word).then_some(Proposal { text, to, rescue: false })
    }

    /// Manual multi-word conversion: converts only the words [`Detector::decide`] marks, keeps
    /// the rest. `words` are (word, tail) from [`crate::buffer::WordBuffer::group_for_conversion`].
    /// `None` = nothing to convert (the hotkey is consumed without action).
    pub fn plan_group(&self, words: &[(String, String)]) -> Option<GroupPlan> {
        let mut text = String::new();
        let (mut converted, mut last_to) = (0, Side::Latin);
        let mut prev: Option<String> = None;
        for (word, tail) in words {
            let out = match self.decide(word, prev.as_deref(), None, false) {
                Decision::Convert(to) => {
                    converted += 1;
                    last_to = to;
                    self.smart_convert(word, to)
                }
                Decision::Keep => word.clone(),
            };
            text.push_str(&out);
            text.push_str(tail);
            prev = Some(out);
        }
        let expected: usize = words.iter().map(|(w, t)| count(w) + count(t)).sum();
        (converted > 0 && count(&text) == expected).then_some(GroupPlan { text, converted, last_to })
    }

    /// Self-heal of a mixed word left by a mid-word fix: an Alt prefix we produced (`live_fix_last`)
    /// followed by a raw Latin tail; converts only the tail.
    pub fn live_heal(&self, word: &str, live_fix_last: &str) -> Option<LiveHeal> {
        let pair = self.pair();
        if !pair.has_alt(word) || !has_latin_letter(word) {
            return None;
        }
        let tail = trailing_latin_run(word);
        let prefix = &word[..word.len() - tail.len()];
        if tail.is_empty() || has_latin_letter(prefix) || prefix != live_fix_last {
            return None;
        }
        let conv = self.keymap.convert(tail, Side::Alt);
        if conv == tail || (pair.has_alt(&conv) && has_latin_letter(&conv)) {
            return None;
        }
        Some(LiveHeal { delete: count(tail), word: format!("{prefix}{conv}"), text: conv })
    }

    /// Direction of a manual conversion of a word: Alt script → Latin, Latin letters → Alt,
    /// neither → away from the current layout.
    pub fn manual_direction(&self, word: &str, current_is_alt: bool) -> Side {
        if self.pair().has_alt(word) {
            Side::Latin
        } else if has_latin_letter(word) {
            Side::Alt
        } else if current_is_alt {
            Side::Latin
        } else {
            Side::Alt
        }
    }

    /// Direction of a selection conversion; `None` = refuse (no letters and the symbols are
    /// ambiguous).
    pub fn selection_direction(&self, text: &str) -> Option<Side> {
        if self.pair().has_alt(text) {
            Some(Side::Latin)
        } else if has_latin_letter(text) {
            Some(Side::Alt)
        } else {
            self.keymap.symbol_direction(text)
        }
    }
}

/// A selection read through the clipboard fallback is refused when it is multi-line or longer
/// than [`CLIPBOARD_SELECTION_MAX_CHARS`] (likely a whole line copied with no selection).
pub fn clipboard_selection_refused(text: &str) -> bool {
    text.contains(['\n', '\r']) || count(text) > CLIPBOARD_SELECTION_MAX_CHARS
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::warm_up;
    use crate::pair::RU_EN;
    use std::sync::LazyLock;

    static KEYMAP: LazyLock<Keymap> = LazyLock::new(|| Keymap::fallback(&RU_EN));
    static NO_EXC: LazyLock<Exceptions> = LazyLock::new(Exceptions::default);

    fn det(exc: &Exceptions) -> Detector<'_> {
        Detector { data: warm_up(), keymap: &KEYMAP, exceptions: exc }
    }

    fn decide(word: &str) -> Decision {
        det(&NO_EXC).decide(word, None, None, false)
    }

    fn decide_after(word: &str, prev: &str) -> Decision {
        det(&NO_EXC).decide(word, Some(prev), None, false)
    }

    const ALT: Decision = Decision::Convert(Side::Alt);
    const LAT: Decision = Decision::Convert(Side::Latin);
    const KEEP: Decision = Decision::Keep;

    #[test]
    fn converts_wrong_layout_words() {
        assert_eq!(decide("ghbdtn"), ALT);
        assert_eq!(decide("руддщ"), LAT);
        assert_eq!(decide("ghbdtn."), ALT);
        assert_eq!(decide("(tckb"), ALT);
        assert_eq!(decide("ghbdtn2"), ALT);
    }

    #[test]
    fn keeps_real_words() {
        for w in ["hello", "world", "привет", "работает", "her", "here", "it.", "нож", "муж"] {
            assert_eq!(decide(w), KEEP, "{w}");
        }
        assert_eq!(decide("круууто"), KEEP);
        assert_eq!(decide(":D"), KEEP);
        assert_eq!(decide("gj1"), KEEP);
        assert_eq!(decide("h2o"), KEEP);
    }

    #[test]
    fn forced_terms() {
        // Built-in force-swap: the Alt-layout form converts, the Latin form stays.
        assert_eq!(decide("реезы"), LAT); // https
        assert_eq!(decide("оыщт"), LAT); // json
        assert_eq!(decide("ЫЙД"), LAT); // SQL, all caps but resolved before the abbreviation rule
        assert_eq!(decide("json"), KEEP);
        assert_eq!(decide("http"), KEEP);
        // ...but a real Alt word whose swap is a forced term stays.
        assert_eq!(decide("еды"), KEEP);
        // Latin tech tokens: kept as typed, and the Alt form converts only as a standalone word.
        assert_eq!(decide("xlsx"), KEEP);
        assert_eq!(decide("txt."), KEEP);
        assert_eq!(decide("ече"), LAT); // txt
    }

    #[test]
    fn user_exceptions() {
        let mut exc = Exceptions::default();
        exc.ignored.insert("ghbdtn".into());
        exc.ignored.insert("руддщ".into());
        exc.ignored.insert(":p".into());
        let d = det(&exc);
        assert_eq!(d.decide("ghbdtn", None, None, false), KEEP);
        assert_eq!(d.decide("Ghbdtn.", None, None, false), KEEP);
        assert_eq!(d.decide("руддщ", None, None, false), KEEP);
        assert_eq!(d.live_decide("ghbdt"), KEEP); // prefix of an exception
        // User force-swap: converting into it wins, typing it stays.
        let mut exc = Exceptions::default();
        exc.force_swap.insert("мы".into());
        let d = det(&exc);
        assert_eq!(d.decide("vs", None, None, false), ALT);
        assert_eq!(d.decide("мы", None, None, false), KEEP);
    }

    #[test]
    fn default_keep_and_seeded_ignored() {
        assert_eq!(decide("мск"), KEEP);
        let exc = Exceptions::from_settings(&crate::settings::Settings::default());
        assert_eq!(det(&exc).decide("вк", None, None, false), KEEP);
    }

    #[test]
    fn context_resolves_short_collisions() {
        assert_eq!(decide("yt"), ALT); // forced: не
        assert_eq!(decide_after("yt", "hello"), ALT);
        // `nj` (то) is forced, but kept after a Latin word ...
        assert_eq!(decide("nj"), ALT);
        assert_eq!(decide_after("nj", "hello"), KEEP);
        // ... unless it continues a Russian phrase after a Latin abbreviation.
        let d = det(&NO_EXC);
        assert_eq!(d.decide("nj", Some("iOS,"), Some("касается"), false), ALT);
        assert_eq!(d.decide("nj", Some("New,"), Some("касается"), false), KEEP);
        // The strict gate keeps real words of both sides of a collision.
        assert_eq!(decide_after("vs", "привет"), KEEP);
        assert_eq!(decide_after("шт", "hello"), KEEP);
    }

    #[test]
    fn single_letters() {
        assert_eq!(decide("d"), ALT); // в
        assert_eq!(decide_after("d", "vitamin"), KEEP);
        assert_eq!(decide_after("d", "room"), ALT);
        assert_eq!(decide("г"), LAT); // u
        assert_eq!(decide_after("г", "5"), KEEP);
        assert_eq!(decide_after("г", "привет"), KEEP);
        assert_eq!(det(&NO_EXC).decide("d", None, None, true), KEEP);
    }

    #[test]
    fn names_and_abbreviations() {
        assert_eq!(decide_after("Уфе", "по"), KEEP);
        assert_eq!(decide("УФНС"), KEEP);
        assert_eq!(decide("ЬФСИЩЩЛ"), LAT); // MACBOOK
    }

    #[test]
    fn hyphenated() {
        assert_eq!(decide("xnj-nj"), ALT);
        assert_eq!(decide("dry-run"), KEEP);
        assert_eq!(decide("что-то"), KEEP);
        assert_eq!(decide("ult-nj"), ALT); // где-то
        assert_eq!(decide("--xnj-nj"), KEEP);
        assert_eq!(decide("5-3"), KEEP);
    }

    #[test]
    fn live_decide_is_stricter() {
        let d = det(&NO_EXC);
        assert_eq!(d.live_decide("ghbdtn"), ALT);
        assert_eq!(d.live_decide("руддщ"), LAT);
        assert_eq!(d.live_decide("hello"), KEEP);
        assert_eq!(d.live_decide("ghb"), KEEP); // too short
        // Words `decide` converts but live fix leaves to the boundary.
        let looser: Vec<&str> = ["yt", "d", "ds", "vs"].into_iter().filter(|w| decide(w) != KEEP).collect();
        assert!(!looser.is_empty());
        for w in looser {
            assert_eq!(d.live_decide(w), KEEP, "{w}");
        }
        // The live verdict never converts what the boundary keeps.
        for w in ["hello", "привет", "it.", "json", "xlsx", "УФНС"] {
            assert_eq!(decide(w), KEEP);
            assert_eq!(d.live_decide(w), KEEP, "{w}");
        }
    }

    #[test]
    fn mixed_rescue() {
        let d = det(&NO_EXC);
        assert_eq!(d.mixed_rescue("привtn"), ALT);
        assert_eq!(d.mixed_rescue("ghbdет"), ALT);
        assert_eq!(d.mixed_rescue("helloмир"), KEEP);
        assert_eq!(d.mixed_rescue("привет"), KEEP);
    }

    #[test]
    fn propose_and_soft_filter() {
        let d = det(&NO_EXC);
        let p = d.propose("ghbdtn.", &ProposalContext::default()).unwrap();
        assert_eq!(p, Proposal { text: "привет.".into(), to: Side::Alt, rescue: false });
        assert_eq!(d.propose("привtn", &ProposalContext::default()).unwrap().text, "привет");
        let soft = ProposalContext { soft: true, ..Default::default() };
        assert!(d.propose("yt", &soft).is_none());
        assert!(d.propose("ghbdtn", &soft).is_some());
        let slow = ProposalContext { soft: true, intra_word_gap: Duration::from_millis(1500), ..Default::default() };
        assert!(d.propose("ghbdtn", &slow).is_none());
        let protected = ProposalContext { session_protected: true, ..Default::default() };
        assert!(d.propose("ghbdtn", &protected).is_none());
    }

    #[test]
    fn group_plan_converts_only_wrong_words() {
        let d = det(&NO_EXC);
        let words = vec![("hello".to_string(), " ".to_string()), ("ghbdtn".to_string(), String::new())];
        let plan = d.plan_group(&words).unwrap();
        assert_eq!(plan.text, "hello привет");
        assert_eq!(plan.converted, 1);
        assert_eq!(plan.last_to, Side::Alt);
        let valid = vec![("hello".to_string(), " ".to_string()), ("world".to_string(), String::new())];
        assert!(d.plan_group(&valid).is_none());
    }

    #[test]
    fn live_heal_converts_raw_tail() {
        let d = det(&NO_EXC);
        let h = d.live_heal("привtn", "прив").unwrap();
        assert_eq!(h, LiveHeal { delete: 2, text: "ет".into(), word: "привет".into() });
        assert!(d.live_heal("привtn", "при").is_none());
    }

    #[test]
    fn manual_and_selection_direction() {
        let d = det(&NO_EXC);
        assert_eq!(d.manual_direction("руддщ", false), Side::Latin);
        assert_eq!(d.manual_direction("ghbdtn", true), Side::Alt);
        assert_eq!(d.manual_direction("123", true), Side::Latin);
        assert_eq!(d.selection_direction("hello world"), Some(Side::Alt));
        assert_eq!(d.selection_direction("?"), None); // fallback table: no symbol direction
        assert!(clipboard_selection_refused("a\nb"));
        assert!(!clipboard_selection_refused("short"));
    }

    #[test]
    fn helpers() {
        assert_eq!(de_elongated("круууто"), Some(vec!["крууто".to_string(), "круто".to_string()]));
        assert_eq!(de_elongated("нуууу"), Some(vec!["нуу".to_string(), "ну".to_string()]));
        assert_eq!(de_elongated("ааа"), Some(vec!["аа".to_string(), "а".to_string()]));
        assert_eq!(de_elongated("класссно"), Some(vec!["классно".to_string(), "класно".to_string()]));
        assert_eq!(de_elongated("класс"), None);
        assert!(is_emoticon(":D."));
        assert!(is_emoticon(":-P"));
        assert!(!is_emoticon(";t"));
        assert!(is_numeric_token(Some("1,5")));
        assert!(!is_numeric_token(Some("a1")));
        assert_eq!(trailing_latin_run("привtn"), "tn");
    }
}
