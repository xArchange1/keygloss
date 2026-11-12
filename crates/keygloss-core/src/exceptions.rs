//! User word exceptions (`ExceptionStore.swift`) and the "who wins" pair choice
//! (`AmbiguousPairs.swift`). Per-app modes live in [`crate::apps`].

use std::collections::HashSet;

use crate::data::{AmbiguousPair, LangData};
use crate::settings::Settings;

/// The user's word lists, lowercase.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Exceptions {
    /// Never converted.
    pub ignored: HashSet<String>,
    /// Always converted into (a word typed in the other layout that maps here is converted;
    /// the word itself typed as is stays).
    pub force_swap: HashSet<String>,
}

/// Result of [`Exceptions::add_ignored`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AddOutcome {
    Added,
    AlreadyThere,
    /// Added, but the built-in keep list already covers it.
    AlreadyBuiltin,
    /// Empty or contains whitespace: it could never match a typed token.
    Rejected,
}

/// Choice on the "who wins" page for one [`AmbiguousPair`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PairChoice {
    /// The Latin side wins.
    Latin,
    /// Decided by context (default).
    Auto,
    /// The Alt side wins.
    Alt,
}

impl Exceptions {
    pub fn from_settings(s: &Settings) -> Exceptions {
        let set = |v: &[String]| v.iter().cloned().collect();
        Exceptions {
            ignored: set(&s.ignored_words),
            force_swap: set(&s.force_swap_words),
        }
    }

    /// Writes the lists back (sorted, for a stable store).
    pub fn write_to(&self, s: &mut Settings) {
        let sorted = |set: &HashSet<String>| {
            let mut v: Vec<String> = set.iter().cloned().collect();
            v.sort();
            v
        };
        s.ignored_words = sorted(&self.ignored);
        s.force_swap_words = sorted(&self.force_swap);
    }

    /// An entry with whitespace can never match a token, so it is refused.
    pub fn is_reachable(word: &str) -> bool {
        !word.chars().any(char::is_whitespace)
    }

    /// On the user's "Never switch" list.
    pub fn is_kept(&self, w: &str) -> bool {
        self.ignored.contains(w)
    }

    pub fn add_ignored(&mut self, word: &str, data: &LangData) -> AddOutcome {
        let w = word.to_lowercase();
        if w.is_empty() || !Self::is_reachable(&w) {
            return AddOutcome::Rejected;
        }
        let builtin = data.extra.default_keep.contains(&w);
        self.force_swap.remove(&w);
        let had = !self.ignored.insert(w);
        if builtin {
            AddOutcome::AlreadyBuiltin
        } else if had {
            AddOutcome::AlreadyThere
        } else {
            AddOutcome::Added
        }
    }

    pub fn remove_ignored(&mut self, word: &str) {
        self.ignored.remove(&word.to_lowercase());
    }

    /// Replaces the ignored list (settings text field): trimmed, lowercase, empties dropped.
    pub fn set_ignored(&mut self, words: &[String]) {
        self.ignored = clean(words);
    }

    pub fn add_force_swap(&mut self, word: &str) {
        let w = word.to_lowercase();
        if w.is_empty() {
            return;
        }
        self.ignored.remove(&w);
        self.force_swap.insert(w);
    }

    pub fn remove_force_swap(&mut self, word: &str) {
        self.force_swap.remove(&word.to_lowercase());
    }

    /// Learned from undo: goes to "Never switch" (and leaves "Always switch").
    pub fn add_learned(&mut self, word: &str) {
        let w = word.to_lowercase();
        if !w.is_empty() {
            self.force_swap.remove(&w);
            self.ignored.insert(w);
        }
    }

    /// Current choice for a pair: only the user's explicit force-swap entries count.
    pub fn pair_choice(&self, p: &AmbiguousPair) -> PairChoice {
        if self.force_swap.contains(&p.alt) {
            PairChoice::Alt
        } else if self.force_swap.contains(&p.latin) {
            PairChoice::Latin
        } else {
            PairChoice::Auto
        }
    }

    /// Stores a choice: the winner goes to force-swap, the loser is removed.
    pub fn choose_pair(&mut self, p: &AmbiguousPair, choice: PairChoice) {
        self.remove_force_swap(&p.latin);
        self.remove_force_swap(&p.alt);
        match choice {
            PairChoice::Alt => self.add_force_swap(&p.alt),
            PairChoice::Latin => self.add_force_swap(&p.latin),
            PairChoice::Auto => {}
        }
    }
}

fn clean(words: &[String]) -> HashSet<String> {
    words.iter().map(|w| w.trim().to_lowercase()).filter(|w| !w.is_empty()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::warm_up;

    #[test]
    fn add_ignored_outcomes() {
        let d = warm_up();
        let mut e = Exceptions::default();
        e.add_force_swap("Foo");
        assert_eq!(e.add_ignored("FOO", d), AddOutcome::Added);
        assert!(!e.force_swap.contains("foo"));
        assert_eq!(e.add_ignored("foo", d), AddOutcome::AlreadyThere);
        assert_eq!(e.add_ignored("мск", d), AddOutcome::AlreadyBuiltin);
        assert_eq!(e.add_ignored("a\u{00A0}b", d), AddOutcome::Rejected);
        assert_eq!(e.add_ignored("", d), AddOutcome::Rejected);
    }

    #[test]
    fn pair_choice_round_trip() {
        let p = AmbiguousPair { latin: "vs".into(), alt: "мы".into() };
        let mut e = Exceptions::default();
        assert_eq!(e.pair_choice(&p), PairChoice::Auto);
        e.choose_pair(&p, PairChoice::Alt);
        assert_eq!(e.pair_choice(&p), PairChoice::Alt);
        e.choose_pair(&p, PairChoice::Latin);
        assert_eq!(e.pair_choice(&p), PairChoice::Latin);
        assert!(!e.force_swap.contains("мы"));
        e.choose_pair(&p, PairChoice::Auto);
        assert!(e.force_swap.is_empty());
    }

    #[test]
    fn settings_round_trip() {
        let mut s = Settings::default();
        let mut e = Exceptions::from_settings(&s);
        assert!(e.ignored.contains("vk"));
        e.add_learned("Гифки");
        e.write_to(&mut s);
        assert!(s.ignored_words.contains(&"гифки".to_string()));
    }
}
