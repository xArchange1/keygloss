//! Learning from undo (`UndoLearner.swift`).
//!
//! After an automatic conversion W→C, the user undoing it within [`UNDO_WINDOW_SECS`] counts as
//! a strike on W: either by a manual hotkey flip C→W, or by erasing C and retyping W exactly.
//! A live fix converts a word's start, so the flip may cover the rest of the word typed after it.
//! Every undo protects W for the current typing context; after [`STRIKE_THRESHOLD`] strikes
//! (decaying after [`STRIKE_DECAY_SECS`]) the engine asks "Keep W as is?".
//!
//! Times are Unix seconds (`f64`), passed in; strike times are persisted in [`Settings`].

use std::collections::{BTreeMap, BTreeSet, HashSet};

use crate::exceptions::Exceptions;
use crate::settings::Settings;

/// An undo must follow the conversion within this many seconds.
pub const UNDO_WINDOW_SECS: f64 = 4.0;
/// Undos of one word before the "keep as is?" question.
pub const STRIKE_THRESHOLD: u32 = 2;
/// A strike older than this resets the count.
pub const STRIKE_DECAY_SECS: f64 = 7.0 * 24.0 * 3600.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Stage {
    Fresh,
    Deleting,
    Retyping,
    /// The word went on past our output: only a hotkey flip can still undo it.
    TypedOn,
}

#[derive(Clone, Debug)]
struct Candidate {
    original: String,
    converted: String,
    created_at: f64,
    stage: Stage,
}

/// A confirmed undo.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UndoEvent {
    /// The restored original (lowercase).
    pub original: String,
    /// Show the "Keep … as is?" question for this word.
    pub ask: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct UndoLearner {
    /// `learnOnUndoEnabled`. Session protection works even when off.
    pub enabled: bool,
    strike_count: BTreeMap<String, u32>,
    strike_time: BTreeMap<String, f64>,
    declined: BTreeSet<String>,
    pending: HashSet<String>,
    session_protected: HashSet<String>,
    candidate: Option<Candidate>,
    /// Our last automatic conversion `(converted, original, at)`, lowercase, tracked even when
    /// learning is off or the word cannot be learned.
    last_auto: Option<(String, String, f64)>,
}

impl UndoLearner {
    /// Loads the persisted state. Counts that already reached the threshold (an old model that
    /// asked without remembering it) are dropped.
    pub fn from_settings(s: &Settings) -> UndoLearner {
        let mut u = UndoLearner {
            enabled: s.learn_on_undo_enabled,
            strike_count: s.undo_strike_count.clone(),
            strike_time: s.undo_strike_time.clone(),
            declined: s.undo_declined_words.iter().cloned().collect(),
            ..Default::default()
        };
        let done: Vec<String> =
            u.strike_count.iter().filter(|(_, n)| **n >= STRIKE_THRESHOLD).map(|(w, _)| w.clone()).collect();
        for w in done {
            u.clear_strikes(&w);
        }
        u
    }

    pub fn write_to(&self, s: &mut Settings) {
        s.undo_strike_count = self.strike_count.clone();
        s.undo_strike_time = self.strike_time.clone();
        s.undo_declined_words = self.declined.iter().cloned().collect();
    }

    pub fn strike_count(&self, word: &str) -> u32 {
        self.strike_count.get(&word.to_lowercase()).copied().unwrap_or(0)
    }

    fn register_strike(&mut self, w: &str, now: f64) -> bool {
        if self.strike_time.get(w).is_some_and(|&last| now - last > STRIKE_DECAY_SECS) {
            self.strike_count.insert(w.to_string(), 0);
        }
        let c = self.strike_count.get(w).copied().unwrap_or(0) + 1;
        self.strike_count.insert(w.to_string(), c);
        self.strike_time.insert(w.to_string(), now);
        c >= STRIKE_THRESHOLD
    }

    fn clear_strikes(&mut self, w: &str) {
        self.strike_count.remove(w);
        self.strike_time.remove(w);
    }

    fn suggest_learn(&mut self, w: &str, learned: &HashSet<String>) -> Option<String> {
        if !self.enabled || learned.contains(w) || self.declined.contains(w) || self.pending.contains(w) {
            return None;
        }
        self.pending.insert(w.to_string());
        Some(w.to_string())
    }

    fn record_relevant_undo(&mut self, w: &str, now: f64, learned: &HashSet<String>) -> Option<String> {
        self.session_protected.insert(w.to_string());
        if self.pending.contains(w) || !self.register_strike(w, now) {
            return None;
        }
        let ask = self.suggest_learn(w, learned);
        if ask.is_some() {
            self.clear_strikes(w);
        }
        ask
    }

    fn live_candidate(&mut self, now: f64) -> Option<Candidate> {
        let c = self.candidate.as_ref()?;
        if now - c.created_at > UNDO_WINDOW_SECS {
            self.candidate = None;
            return None;
        }
        Some(c.clone())
    }

    /// We converted `original` into `converted` automatically (or fixed a typo).
    pub fn note_conversion(&mut self, original: &str, converted: &str, now: f64, learned: &HashSet<String>) {
        self.candidate = None;
        let (o, c) = (original.to_lowercase(), converted.to_lowercase());
        self.last_auto = Some((c.clone(), o.clone(), now));
        if !self.enabled || o == c || o.is_empty() || c.is_empty() || learned.contains(&o) || self.declined.contains(&o) {
            return;
        }
        self.candidate = Some(Candidate { original: o, converted: c, created_at: now, stage: Stage::Fresh });
    }

    /// `from`→`to` by hotkey reverses our recent automatic conversion (an undo, not a switch).
    pub fn reverses_last_conversion(&self, from: &str, to: &str, now: f64) -> bool {
        self.last_auto.as_ref().is_some_and(|(c, o, at)| {
            now - at <= UNDO_WINDOW_SECS && from.to_lowercase().starts_with(c) && to.to_lowercase().starts_with(o)
        })
    }

    /// The user flipped `from` into `to` by hotkey. A reversal of our recent conversion is an
    /// undo of `to`, also when the word went on past a live fix (`from` starts with our output).
    pub fn note_manual_convert(&mut self, from: &str, to: &str, now: f64, learned: &HashSet<String>) -> Option<UndoEvent> {
        if !self.enabled {
            return None;
        }
        let cand = self.live_candidate(now)?;
        let (from, to) = (from.to_lowercase(), to.to_lowercase());
        if !from.starts_with(&cand.converted) || !to.starts_with(&cand.original) {
            return None;
        }
        self.candidate = None;
        let ask = self.record_relevant_undo(&to, now, learned);
        Some(UndoEvent { original: to, ask })
    }

    /// Call after every printable key and Backspace with the buffer's current word.
    /// `Some` when the user erased our output and retyped the original exactly.
    pub fn observe(&mut self, current: &str, now: f64, learned: &HashSet<String>) -> Option<UndoEvent> {
        if !self.enabled {
            return None;
        }
        let mut cand = self.live_candidate(now)?;
        let cur = current.to_lowercase();
        match cand.stage {
            Stage::Fresh | Stage::Deleting => {
                if cur.is_empty() {
                    cand.stage = Stage::Retyping;
                    self.candidate = Some(cand);
                } else if cand.converted.starts_with(&cur) && cur.chars().count() < cand.converted.chars().count() {
                    cand.stage = Stage::Deleting;
                    self.candidate = Some(cand);
                } else if cand.stage == Stage::Fresh && cur.starts_with(&cand.converted) {
                    cand.stage = Stage::TypedOn;
                    self.candidate = Some(cand);
                } else {
                    self.candidate = None;
                }
                None
            }
            Stage::TypedOn => {
                if !cur.starts_with(&cand.converted) {
                    self.candidate = None;
                }
                None
            }
            Stage::Retyping => {
                if cur == cand.original {
                    self.candidate = None;
                    let ask = self.record_relevant_undo(&cand.original, now, learned);
                    Some(UndoEvent { original: cand.original, ask })
                } else {
                    if !cand.original.starts_with(&cur) {
                        self.candidate = None;
                    }
                    None
                }
            }
        }
    }

    /// The user is retyping the original: hold automatic conversion of `current`.
    pub fn should_suppress(&mut self, current: &str, now: f64) -> bool {
        if !self.enabled {
            return false;
        }
        let Some(cand) = self.live_candidate(now) else { return false };
        let cur = current.to_lowercase();
        cand.stage == Stage::Retyping && !cur.is_empty() && cand.original.starts_with(&cur) && cur != cand.original
    }

    pub fn is_session_protected(&self, word: &str) -> bool {
        self.session_protected.contains(&word.to_lowercase())
    }

    /// Protects a word the user just set by hand from immediate automatic re-conversion.
    pub fn protect(&mut self, word: &str) {
        let w = word.to_lowercase();
        if !w.is_empty() {
            self.session_protected.insert(w);
        }
    }

    /// Typing context changed (click, navigation, app switch).
    pub fn reset_context(&mut self) {
        self.candidate = None;
        self.session_protected.clear();
    }

    /// "Add": the word becomes a learned exception. Returns false if it already was.
    pub fn confirm_learn(&mut self, word: &str, exceptions: &mut Exceptions) -> bool {
        let w = word.to_lowercase();
        self.pending.remove(&w);
        if exceptions.ignored.contains(&w) {
            return false;
        }
        exceptions.add_learned(&w);
        self.clear_strikes(&w);
        true
    }

    /// "No thanks": never ask about this word again.
    pub fn decline_learn(&mut self, word: &str) {
        let w = word.to_lowercase();
        self.pending.remove(&w);
        self.clear_strikes(&w);
        self.declined.insert(w);
    }

    /// The question was dismissed without an answer: forget this round only.
    pub fn dismiss_learn(&mut self, word: &str) {
        let w = word.to_lowercase();
        self.pending.remove(&w);
        self.clear_strikes(&w);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn learner() -> UndoLearner {
        UndoLearner { enabled: true, ..Default::default() }
    }

    /// Converts and undoes by erasing and retyping, starting at `t`.
    fn retype_undo(u: &mut UndoLearner, t: f64) -> Option<UndoEvent> {
        let none = HashSet::new();
        u.note_conversion("гифки", "ubarb", t, &none);
        for cur in ["ubar", "uba", "ub", "u", ""] {
            assert!(u.observe(cur, t + 0.5, &none).is_none());
        }
        for cur in ["г", "ги", "гиф", "гифк"] {
            assert!(u.observe(cur, t + 1.0, &none).is_none());
            assert!(u.should_suppress(cur, t + 1.0));
        }
        u.observe("гифки", t + 1.5, &none)
    }

    #[test]
    fn two_strikes_ask_once() {
        let mut u = learner();
        let e1 = retype_undo(&mut u, 0.0).unwrap();
        assert_eq!(e1, UndoEvent { original: "гифки".into(), ask: None });
        assert!(u.is_session_protected("Гифки"));
        assert_eq!(u.strike_count("гифки"), 1);
        let e2 = retype_undo(&mut u, 20.0).unwrap();
        assert_eq!(e2.ask.as_deref(), Some("гифки"));
        assert_eq!(u.strike_count("гифки"), 0); // the strikes were spent on the question
        // While the question is pending, undos do not count.
        assert!(retype_undo(&mut u, 30.0).unwrap().ask.is_none());
        assert_eq!(u.strike_count("гифки"), 0);
        let mut exc = Exceptions::default();
        assert!(u.confirm_learn("гифки", &mut exc));
        assert!(exc.ignored.contains("гифки"));
        // A learned word is no longer a candidate.
        u.note_conversion("гифки", "ubarb", 40.0, &exc.ignored);
        assert!(u.note_manual_convert("ubarb", "гифки", 41.0, &exc.ignored).is_none());
    }

    #[test]
    fn manual_flip_counts_within_window() {
        let none = HashSet::new();
        let mut u = learner();
        u.note_conversion("ghbdtn", "привет", 0.0, &none);
        assert!(u.note_manual_convert("привет", "ghbdtn", 5.0, &none).is_none()); // window passed
        u.note_conversion("ghbdtn", "привет", 10.0, &none);
        assert!(u.note_manual_convert("привет", "nope", 11.0, &none).is_none());
        let ev = u.note_manual_convert("Привет", "Ghbdtn", 12.0, &none).unwrap();
        assert_eq!(ev.original, "ghbdtn");
        u.reset_context();
        assert!(!u.is_session_protected("ghbdtn"));
    }

    #[test]
    fn manual_flip_after_typing_on_past_a_live_fix() {
        let none = HashSet::new();
        let mut u = learner();
        u.note_conversion("ghbd", "прив", 0.0, &none);
        for cur in ["приве", "привет"] {
            assert!(u.observe(cur, 0.5, &none).is_none());
        }
        let ev = u.note_manual_convert("Привет", "Ghbdtn", 1.0, &none).unwrap();
        assert_eq!(ev.original, "ghbdtn");
        assert_eq!(u.strike_count("ghbdtn"), 1);
        assert!(u.is_session_protected("ghbdtn"));
    }

    #[test]
    fn strikes_decay_after_a_week() {
        let none = HashSet::new();
        let mut u = learner();
        u.note_conversion("a1", "b1", 0.0, &none);
        u.note_manual_convert("b1", "a1", 1.0, &none).unwrap();
        assert_eq!(u.strike_count("a1"), 1);
        let later = 1.0 + STRIKE_DECAY_SECS + 1.0;
        u.note_conversion("a1", "b1", later, &none);
        let ev = u.note_manual_convert("b1", "a1", later + 1.0, &none).unwrap();
        assert!(ev.ask.is_none());
        assert_eq!(u.strike_count("a1"), 1);
    }

    #[test]
    fn decline_and_persistence() {
        let none = HashSet::new();
        let mut u = learner();
        u.decline_learn("x1");
        u.note_conversion("x1", "y1", 0.0, &none);
        assert!(u.note_manual_convert("y1", "x1", 1.0, &none).is_none());
        let mut s = Settings::default();
        s.undo_strike_count.insert("old".into(), 2);
        s.undo_strike_count.insert("w".into(), 1);
        let mut u = UndoLearner::from_settings(&s);
        assert_eq!(u.strike_count("old"), 0);
        assert_eq!(u.strike_count("w"), 1);
        u.decline_learn("z");
        u.write_to(&mut s);
        assert_eq!(s.undo_declined_words, vec!["z".to_string()]);
        assert!(!s.undo_strike_count.contains_key("old"));
    }

    #[test]
    fn typing_on_accepts_conversion() {
        let none = HashSet::new();
        let mut u = learner();
        u.note_conversion("ghbdtn", "привет", 0.0, &none);
        assert!(u.observe("приветы", 0.5, &none).is_none());
        assert!(u.observe("", 0.6, &none).is_none());
        assert!(u.observe("ghbdtn", 0.7, &none).is_none()); // candidate was dropped
    }
}
