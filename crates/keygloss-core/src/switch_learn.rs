//! Learning from manual switches: flipping the same word by hotkey [`STRIKE_THRESHOLD`] times
//! (strikes decay after [`STRIKE_DECAY_SECS`]) asks "Always switch it?". "Add" puts the
//! converted word into "Always switch" (`force_swap`). Undos of our own conversions do not count.

use std::collections::{BTreeMap, BTreeSet, HashSet};

use crate::exceptions::Exceptions;
use crate::settings::Settings;
use crate::undo::{STRIKE_DECAY_SECS, STRIKE_THRESHOLD};

#[derive(Clone, Debug, Default)]
pub struct SwitchLearner {
    /// `learnOnSwitchEnabled`.
    pub enabled: bool,
    strike_count: BTreeMap<String, u32>,
    strike_time: BTreeMap<String, f64>,
    declined: BTreeSet<String>,
    pending: HashSet<String>,
}

impl SwitchLearner {
    pub fn from_settings(s: &Settings) -> SwitchLearner {
        SwitchLearner {
            enabled: s.learn_on_switch_enabled,
            strike_count: s.switch_strike_count.clone(),
            strike_time: s.switch_strike_time.clone(),
            declined: s.switch_declined_words.iter().cloned().collect(),
            ..Default::default()
        }
    }

    pub fn write_to(&self, s: &mut Settings) {
        s.switch_strike_count = self.strike_count.clone();
        s.switch_strike_time = self.strike_time.clone();
        s.switch_declined_words = self.declined.iter().cloned().collect();
    }

    fn clear_strikes(&mut self, w: &str) {
        self.strike_count.remove(w);
        self.strike_time.remove(w);
    }

    /// The user flipped `from` into `to` by hotkey. Returns `to` (lowercase) when it is time to ask.
    pub fn note_switch(&mut self, from: &str, to: &str, now: f64, exc: &Exceptions) -> Option<String> {
        let (f, w) = (from.to_lowercase(), to.to_lowercase());
        if !self.enabled
            || w.is_empty()
            || !Exceptions::is_reachable(&w)
            || exc.force_swap.contains(&w)
            || exc.is_kept(&f)
            || exc.is_kept(&w)
            || self.declined.contains(&w)
            || self.pending.contains(&w)
        {
            return None;
        }
        if self.strike_time.get(&w).is_some_and(|&last| now - last > STRIKE_DECAY_SECS) {
            self.strike_count.remove(&w);
        }
        let c = self.strike_count.get(&w).copied().unwrap_or(0) + 1;
        self.strike_count.insert(w.clone(), c);
        self.strike_time.insert(w.clone(), now);
        if c < STRIKE_THRESHOLD {
            return None;
        }
        self.clear_strikes(&w);
        self.pending.insert(w.clone());
        Some(w)
    }

    /// "Add": the word goes to "Always switch". Returns false if it already was there.
    pub fn confirm(&mut self, word: &str, exceptions: &mut Exceptions) -> bool {
        let w = word.to_lowercase();
        self.pending.remove(&w);
        if exceptions.force_swap.contains(&w) {
            return false;
        }
        exceptions.add_force_swap(&w);
        true
    }

    /// "No thanks": never ask about this word again.
    pub fn decline(&mut self, word: &str) {
        let w = word.to_lowercase();
        self.pending.remove(&w);
        self.clear_strikes(&w);
        self.declined.insert(w);
    }

    /// The question was dismissed without an answer: forget this round only.
    pub fn dismiss(&mut self, word: &str) {
        self.pending.remove(&word.to_lowercase());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_switches_ask_once_and_add_to_force_swap() {
        let mut exc = Exceptions::default();
        let mut l = SwitchLearner { enabled: true, ..Default::default() };
        assert!(l.note_switch("vs", "мы", 0.0, &exc).is_none());
        assert_eq!(l.note_switch("Vs", "Мы", 10.0, &exc).as_deref(), Some("мы"));
        assert!(l.note_switch("vs", "мы", 20.0, &exc).is_none()); // pending
        assert!(l.confirm("мы", &mut exc));
        assert!(exc.force_swap.contains("мы"));
        assert!(l.note_switch("vs", "мы", 30.0, &exc).is_none());
    }

    #[test]
    fn decline_decay_and_persistence() {
        let exc = Exceptions::default();
        let mut l = SwitchLearner { enabled: true, ..Default::default() };
        l.note_switch("a", "ф", 0.0, &exc);
        assert!(l.note_switch("a", "ф", STRIKE_DECAY_SECS + 1.0, &exc).is_none()); // decayed
        l.decline("ф");
        assert!(l.note_switch("a", "ф", STRIKE_DECAY_SECS + 2.0, &exc).is_none());
        let mut s = Settings::default();
        l.write_to(&mut s);
        assert_eq!(s.switch_declined_words, vec!["ф".to_string()]);
        let mut l = SwitchLearner::from_settings(&s);
        assert!(l.note_switch("a", "ф", 0.0, &exc).is_none());
    }
}
