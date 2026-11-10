//! Anti-resonance guard (`AntiResonanceGuard.swift`): a circuit breaker against automatic
//! conversions flipping the same text back and forth. Applies to automatic conversions only
//! (live fix and boundary), never to the manual hotkey.

use std::time::{Duration, Instant};

#[derive(Clone, Debug)]
pub struct AntiResonanceGuard {
    /// Observation window for recent conversions.
    window: Duration,
    /// More conversions than this inside the window is a storm.
    max_flips: usize,
    /// How long automatic conversion stays frozen after a detection.
    freeze_for: Duration,
    recent: Vec<(String, Instant)>,
    frozen_until: Option<Instant>,
}

impl Default for AntiResonanceGuard {
    fn default() -> Self {
        AntiResonanceGuard::new(Duration::from_millis(700), 6, Duration::from_millis(2500))
    }
}

impl AntiResonanceGuard {
    pub fn new(window: Duration, max_flips: usize, freeze_for: Duration) -> AntiResonanceGuard {
        AntiResonanceGuard { window, max_flips, freeze_for, recent: Vec::new(), frozen_until: None }
    }

    /// Asks before converting `word` into `produced`. `false` = resonance: skip the conversion
    /// (and reset the buffer). Oscillation is converting a form we produced within the window.
    pub fn allow(&mut self, word: &str, produced: &str, now: Instant) -> bool {
        if self.is_frozen(now) {
            return false;
        }
        let window = self.window;
        self.recent.retain(|(_, at)| now.saturating_duration_since(*at) <= window);
        let oscillation = self.recent.iter().any(|(p, _)| p == word);
        self.recent.push((produced.to_string(), now));
        if oscillation || self.recent.len() > self.max_flips {
            self.frozen_until = Some(now + self.freeze_for);
            self.recent.clear();
            return false;
        }
        true
    }

    pub fn is_frozen(&self, now: Instant) -> bool {
        self.frozen_until.is_some_and(|until| now < until)
    }

    /// Forgets the history (not the freeze). Called on a context change.
    pub fn reset_history(&mut self) {
        self.recent.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(t: Instant, n: u64) -> Instant {
        t + Duration::from_millis(n)
    }

    #[test]
    fn oscillation_freezes() {
        let t = Instant::now();
        let mut g = AntiResonanceGuard::default();
        assert!(g.allow("ghbdtn", "привет", t));
        assert!(!g.allow("привет", "ghbdtn", ms(t, 100)));
        assert!(g.is_frozen(ms(t, 2000)));
        assert!(!g.allow("abc", "def", ms(t, 2000)));
        assert!(!g.is_frozen(ms(t, 2700)));
        assert!(g.allow("abc", "def", ms(t, 2700)));
    }

    #[test]
    fn storm_freezes_and_window_expires() {
        let t = Instant::now();
        let mut g = AntiResonanceGuard::default();
        for i in 0..6 {
            assert!(g.allow(&format!("w{i}"), &format!("p{i}"), ms(t, i * 10)));
        }
        assert!(!g.allow("w6", "p6", ms(t, 60)));
        let mut g = AntiResonanceGuard::default();
        assert!(g.allow("a", "b", t));
        assert!(g.allow("b", "a", ms(t, 800))); // outside the window: not an oscillation
        g.reset_history();
        assert!(g.allow("a", "b", ms(t, 900)));
    }
}
