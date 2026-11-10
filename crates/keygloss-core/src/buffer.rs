//! Typed-word buffer (`KeystrokeBuffer.swift`): the current word, the last finished word with
//! its whitespace tail, and the session's finished words for group conversion.
//! Times are passed in, so the buffer reads no clock.
//!
//! Lengths count Unicode scalar values (`char`s). The original counted grapheme clusters; the
//! two differ only for combining sequences, which group conversion refuses anyway.

use std::time::{Duration, Instant};

/// Group conversion expires after this much inactivity.
pub const GROUP_MAX_IDLE: Duration = Duration::from_secs(8);
/// Group conversion refuses more characters than this.
pub const GROUP_MAX_CHARS: usize = 200;

/// A finished word and the whitespace typed after it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionWord {
    pub word: String,
    pub tail: String,
}

/// What a conversion targets: the word, how many characters to delete, and the tail to retype.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WordItem {
    pub word: String,
    pub delete_count: usize,
    pub tail: String,
}

/// Words for group conversion, with the total characters to delete.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Group {
    /// (word, tail); the last entry may be the current word with an empty tail.
    pub words: Vec<(String, String)>,
    pub delete_count: usize,
}

#[derive(Clone, Debug, Default)]
pub struct WordBuffer {
    current_word: String,
    last_word: String,
    last_tail: String,
    /// Gap between the first two spaces of the last tail (system "double space → period").
    last_tail_double_space_gap: Option<Duration>,
    last_tail_first_space_at: Option<Instant>,
    session_words: Vec<SessionWord>,
    last_activity: Option<Instant>,
    /// Longest pause between letters of the current word.
    current_word_gap: Duration,
    /// The same for the last finished word.
    last_word_gap: Duration,
}

fn len(s: &str) -> usize {
    s.chars().count()
}

/// A character that joins the previous one into one grapheme (combining marks, joiners,
/// variation selectors, emoji modifiers, tags).
fn is_grapheme_extender(c: char) -> bool {
    matches!(c as u32,
        0x0300..=0x036F | 0x0483..=0x0489 | 0x0591..=0x05BD | 0x0610..=0x061A | 0x064B..=0x065F
        | 0x1AB0..=0x1AFF | 0x1DC0..=0x1DFF | 0x200C | 0x200D | 0x20D0..=0x20FF
        | 0xFE00..=0xFE0F | 0xFE20..=0xFE2F | 0x1F3FB..=0x1F3FF | 0x1F1E6..=0x1F1FF
        | 0xE0020..=0xE007F | 0xE0100..=0xE01EF)
}

impl WordBuffer {
    pub fn new() -> WordBuffer {
        WordBuffer::default()
    }

    pub fn current_word(&self) -> &str {
        &self.current_word
    }
    pub fn last_word(&self) -> &str {
        &self.last_word
    }
    pub fn last_tail(&self) -> &str {
        &self.last_tail
    }
    pub fn session_words(&self) -> &[SessionWord] {
        &self.session_words
    }
    pub fn current_word_gap(&self) -> Duration {
        self.current_word_gap
    }
    pub fn last_word_gap(&self) -> Duration {
        self.last_word_gap
    }
    /// Gap between the two leading spaces of the last tail, if they were typed as a pair.
    pub fn last_tail_double_space_gap(&self) -> Option<Duration> {
        self.last_tail_double_space_gap
    }

    fn reset_double_space(&mut self) {
        self.last_tail_first_space_at = None;
        self.last_tail_double_space_gap = None;
    }

    /// A typed character (or string).
    pub fn append(&mut self, s: &str, now: Instant) {
        if !self.current_word.is_empty()
            && let Some(last) = self.last_activity
        {
            self.current_word_gap = self.current_word_gap.max(now.saturating_duration_since(last));
        }
        self.current_word.push_str(s);
        self.last_activity = Some(now);
    }

    /// Plain Backspace.
    pub fn backspace(&mut self, now: Instant) {
        self.last_activity = Some(now);
        if !self.current_word.is_empty() {
            self.current_word.pop();
            if self.current_word.is_empty() {
                // The word is gone: the previous context is no longer reliable.
                self.last_word.clear();
                self.last_tail.clear();
                self.reset_double_space();
                self.session_words.clear();
                self.current_word_gap = Duration::ZERO;
            }
        } else if !self.last_tail.is_empty() {
            self.last_tail.pop();
            if let Some(last) = self.session_words.last_mut() {
                last.tail = self.last_tail.clone();
            }
            if !self.last_tail.starts_with("  ") {
                self.reset_double_space();
            }
        } else if !self.last_word.is_empty() {
            // Backspace into the finished word: it becomes the current word again.
            self.current_word = std::mem::take(&mut self.last_word);
            self.current_word_gap = self.last_word_gap;
            self.session_words.pop();
            if let Some(last) = self.session_words.last() {
                self.last_word = last.word.clone();
                self.last_tail = last.tail.clone();
            } else {
                self.last_tail.clear();
            }
            self.reset_double_space();
            self.current_word.pop();
            if self.current_word.is_empty() {
                self.last_word.clear();
                self.last_tail.clear();
                self.session_words.clear();
                self.current_word_gap = Duration::ZERO;
                self.reset_double_space();
            }
        } else {
            self.clear();
        }
    }

    /// A word boundary key: `" "`, `"\t"` or `"\n"`.
    pub fn boundary(&mut self, whitespace: &str, now: Instant) {
        self.last_activity = Some(now);
        if !self.current_word.is_empty() {
            let word = std::mem::take(&mut self.current_word);
            self.session_words.push(SessionWord { word: word.clone(), tail: whitespace.to_string() });
            self.last_word = word;
            self.last_tail = whitespace.to_string();
            self.last_tail_first_space_at = (whitespace == " ").then_some(now);
            self.last_tail_double_space_gap = None;
            self.last_word_gap = self.current_word_gap;
            self.current_word_gap = Duration::ZERO;
        } else if !self.last_word.is_empty() {
            let previous_tail = std::mem::take(&mut self.last_tail);
            self.last_tail = format!("{previous_tail}{whitespace}");
            match self.last_tail_first_space_at {
                Some(first) if previous_tail == " " && whitespace == " " => {
                    self.last_tail_double_space_gap = Some(now.saturating_duration_since(first));
                }
                // A third space, Tab or Enter keeps the leading pair already proven.
                _ if previous_tail.starts_with("  ") && self.last_tail_double_space_gap.is_some() => {}
                _ if previous_tail.is_empty() && whitespace == " " => {
                    self.last_tail_first_space_at = Some(now);
                    self.last_tail_double_space_gap = None;
                }
                _ => self.reset_double_space(),
            }
            if let Some(last) = self.session_words.last_mut() {
                last.tail.push_str(whitespace);
            }
        }
    }

    /// Full reset (navigation, click, app switch).
    pub fn clear(&mut self) {
        self.current_word.clear();
        self.last_word.clear();
        self.last_tail.clear();
        self.reset_double_space();
        self.session_words.clear();
        self.current_word_gap = Duration::ZERO;
        self.last_word_gap = Duration::ZERO;
    }

    /// Soft reset: forgets finished words and the group, keeps the word being typed.
    pub fn soft_context_reset(&mut self) {
        self.last_word.clear();
        self.last_tail.clear();
        self.reset_double_space();
        self.session_words.clear();
    }

    /// Drops only the group history (the screen may no longer match it).
    pub fn invalidate_group_history(&mut self) {
        self.session_words.clear();
    }

    /// Finished session words plus the current word, if the group is usable: active within
    /// [`GROUP_MAX_IDLE`], at least two words, no multi-scalar graphemes, no Tab/Enter tails, at
    /// most [`GROUP_MAX_CHARS`] characters.
    pub fn group_for_conversion(&self, now: Instant) -> Option<Group> {
        let last = self.last_activity?;
        if now.saturating_duration_since(last) > GROUP_MAX_IDLE {
            return None;
        }
        let mut words: Vec<(String, String)> =
            self.session_words.iter().map(|s| (s.word.clone(), s.tail.clone())).collect();
        if !self.current_word.is_empty() {
            words.push((self.current_word.clone(), String::new()));
        }
        if words.len() < 2 {
            return None;
        }
        for (w, t) in &words {
            if w.chars().any(is_grapheme_extender) || t.contains(['\n', '\t']) {
                return None;
            }
        }
        let total: usize = words.iter().map(|(w, t)| len(w) + len(t)).sum();
        if total == 0 || total > GROUP_MAX_CHARS {
            return None;
        }
        Some(Group { words, delete_count: total })
    }

    /// The current word (no tail), else the last word with its tail. With `completed_only` (the
    /// automatic boundary path) always the last finished word; a started next word joins the
    /// tail so both are retyped.
    pub fn word_for_conversion(&self, completed_only: bool) -> Option<WordItem> {
        if completed_only {
            if self.last_word.is_empty() {
                return None;
            }
            return Some(WordItem {
                word: self.last_word.clone(),
                delete_count: len(&self.last_word) + len(&self.last_tail) + len(&self.current_word),
                tail: format!("{}{}", self.last_tail, self.current_word),
            });
        }
        if !self.current_word.is_empty() {
            return Some(WordItem {
                word: self.current_word.clone(),
                delete_count: len(&self.current_word),
                tail: String::new(),
            });
        }
        if !self.last_word.is_empty() {
            return Some(WordItem {
                word: self.last_word.clone(),
                delete_count: len(&self.last_word) + len(&self.last_tail),
                tail: self.last_tail.clone(),
            });
        }
        None
    }

    /// After a `completed_only` conversion: updates the finished word only.
    pub fn apply_completed_conversion(&mut self, converted: &str) {
        if self.last_word.is_empty() {
            return;
        }
        self.last_word = converted.to_string();
        if let Some(last) = self.session_words.last_mut() {
            last.word = converted.to_string();
        }
    }

    /// The boundary conversion retyped a same-length tail (`"  "` → `". "`).
    pub fn apply_completed_tail(&mut self, tail: &str) {
        if self.last_word.is_empty() {
            return;
        }
        self.last_tail = tail.to_string();
        if let Some(last) = self.session_words.last_mut() {
            last.tail = tail.to_string();
        }
        self.reset_double_space();
    }

    /// A snippet replaced the current word (trigger), then the swallowed boundary was typed.
    /// The group is invalidated too (the screen changed by a different length).
    pub fn commit_snippet(&mut self, expansion: &str, whitespace: &str, now: Instant) {
        self.current_word = expansion.to_string();
        self.boundary(whitespace, now);
        self.invalidate_group_history();
    }

    /// After a conversion of [`WordBuffer::word_for_conversion`] (`completed_only == false`).
    pub fn apply_conversion(&mut self, converted: &str) {
        if !self.current_word.is_empty() {
            self.current_word = converted.to_string();
        } else if !self.last_word.is_empty() {
            self.last_word = converted.to_string();
            if let Some(last) = self.session_words.last_mut() {
                last.word = converted.to_string();
            }
        }
    }

    fn context_index(&self, for_current: bool) -> isize {
        self.session_words.len() as isize - if for_current { 1 } else { 2 }
    }

    fn breaks_line(tail: &str) -> bool {
        tail.contains(['\n', '\r'])
    }

    /// The word before the one being decided, as on screen; never across a line break.
    /// `for_current`: deciding the current (unfinished) word; otherwise the last finished one.
    pub fn context_word(&self, for_current: bool) -> Option<&str> {
        let i = self.context_index(for_current);
        let w = self.session_words.get(usize::try_from(i).ok()?)?;
        (!Self::breaks_line(&w.tail)).then_some(w.word.as_str())
    }

    /// One more word to the left of [`WordBuffer::context_word`], same line only.
    pub fn earlier_context_word(&self, for_current: bool) -> Option<&str> {
        let immediate = usize::try_from(self.context_index(for_current)).ok()?;
        let earlier = immediate.checked_sub(1)?;
        let (a, b) = (&self.session_words[immediate], &self.session_words[earlier]);
        (!Self::breaks_line(&a.tail) && !Self::breaks_line(&b.tail)).then_some(b.word.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn typed(b: &mut WordBuffer, s: &str, t: Instant) {
        for c in s.chars() {
            b.append(&c.to_string(), t);
        }
    }

    #[test]
    fn boundary_moves_word_and_tracks_tail() {
        let t = Instant::now();
        let mut b = WordBuffer::new();
        typed(&mut b, "ghbdtn", t);
        b.boundary(" ", t);
        assert_eq!(b.current_word(), "");
        assert_eq!(b.last_word(), "ghbdtn");
        assert_eq!(b.last_tail(), " ");
        assert_eq!(
            b.word_for_conversion(true),
            Some(WordItem { word: "ghbdtn".into(), delete_count: 7, tail: " ".into() })
        );
        typed(&mut b, "x", t);
        assert_eq!(
            b.word_for_conversion(true),
            Some(WordItem { word: "ghbdtn".into(), delete_count: 8, tail: " x".into() })
        );
        assert_eq!(b.word_for_conversion(false).unwrap().word, "x");
    }

    #[test]
    fn backspace_shrinks_tail_then_reopens_word() {
        let t = Instant::now();
        let mut b = WordBuffer::new();
        typed(&mut b, "one", t);
        b.boundary(" ", t);
        typed(&mut b, "two", t);
        b.boundary(" ", t);
        b.backspace(t); // tail
        assert_eq!(b.last_tail(), "");
        assert_eq!(b.last_word(), "two");
        b.backspace(t); // into "two"
        assert_eq!(b.current_word(), "tw");
        assert_eq!(b.last_word(), "one");
        assert_eq!(b.last_tail(), " ");
        b.backspace(t);
        b.backspace(t); // word erased: context dropped
        assert_eq!(b.current_word(), "");
        assert_eq!(b.last_word(), "");
        assert!(b.session_words().is_empty());
        b.backspace(t); // nothing known: clear
        assert!(b.word_for_conversion(false).is_none());
    }

    #[test]
    fn gaps_and_double_space() {
        let t = Instant::now();
        let mut b = WordBuffer::new();
        b.append("a", t);
        b.append("b", t + Duration::from_millis(300));
        b.append("c", t + Duration::from_millis(400));
        assert_eq!(b.current_word_gap(), Duration::from_millis(300));
        b.boundary(" ", t + Duration::from_millis(500));
        assert_eq!(b.last_word_gap(), Duration::from_millis(300));
        assert_eq!(b.current_word_gap(), Duration::ZERO);
        b.boundary(" ", t + Duration::from_millis(700));
        assert_eq!(b.last_tail_double_space_gap(), Some(Duration::from_millis(200)));
        b.boundary(" ", t + Duration::from_millis(900)); // a third space keeps the pair
        assert_eq!(b.last_tail_double_space_gap(), Some(Duration::from_millis(200)));
        b.backspace(t);
        assert_eq!(b.last_tail(), "  ");
        assert!(b.last_tail_double_space_gap().is_some());
        b.backspace(t);
        assert_eq!(b.last_tail_double_space_gap(), None);
    }

    #[test]
    fn group_rules() {
        let t = Instant::now();
        let mut b = WordBuffer::new();
        typed(&mut b, "hello", t);
        assert!(b.group_for_conversion(t).is_none()); // one word
        b.boundary(" ", t);
        typed(&mut b, "ghbdtn", t);
        let g = b.group_for_conversion(t).unwrap();
        assert_eq!(g.words, vec![("hello".into(), " ".into()), ("ghbdtn".into(), String::new())]);
        assert_eq!(g.delete_count, 12);
        assert!(b.group_for_conversion(t + Duration::from_secs(9)).is_none()); // idle
        b.boundary("\n", t);
        typed(&mut b, "x", t);
        assert!(b.group_for_conversion(t).is_none()); // newline tail
        let mut c = WordBuffer::new();
        typed(&mut c, "e\u{0301}", t);
        c.boundary(" ", t);
        typed(&mut c, "x", t);
        assert!(c.group_for_conversion(t).is_none()); // combining sequence
        let mut d = WordBuffer::new();
        for _ in 0..3 {
            typed(&mut d, &"a".repeat(80), t);
            d.boundary(" ", t);
        }
        assert!(d.group_for_conversion(t).is_none()); // over 200 chars
    }

    #[test]
    fn context_words_stop_at_line_breaks() {
        let t = Instant::now();
        let mut b = WordBuffer::new();
        for w in ["a", "b"] {
            typed(&mut b, w, t);
            b.boundary(" ", t);
        }
        typed(&mut b, "c", t);
        assert_eq!(b.context_word(true), Some("b"));
        assert_eq!(b.earlier_context_word(true), Some("a"));
        b.boundary(" ", t);
        assert_eq!(b.context_word(false), Some("b"));
        assert_eq!(b.context_word(true), Some("c"));
        b.boundary("\n", t);
        assert_eq!(b.context_word(true), None);
    }

    #[test]
    fn resets_and_updates() {
        let t = Instant::now();
        let mut b = WordBuffer::new();
        typed(&mut b, "one", t);
        b.boundary(" ", t);
        typed(&mut b, "tw", t);
        b.soft_context_reset();
        assert_eq!(b.current_word(), "tw");
        assert_eq!(b.last_word(), "");
        b.apply_conversion("ец");
        assert_eq!(b.current_word(), "ец");
        b.boundary(" ", t);
        b.apply_completed_conversion("TW");
        assert_eq!(b.session_words().last().unwrap().word, "TW");
        b.apply_completed_tail("_");
        assert_eq!(b.last_tail(), "_");
        typed(&mut b, "!br", t);
        b.commit_snippet("Best regards", " ", t);
        assert_eq!(b.last_word(), "Best regards");
        assert!(b.session_words().is_empty());
        b.clear();
        assert!(b.word_for_conversion(false).is_none());
    }
}
