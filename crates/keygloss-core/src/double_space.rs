//! The "double space inserts a period" tail rule (`DoubleSpacePeriod.swift`).
//!
//! When the OS already turned two quickly typed spaces into `". "`, a delayed boundary
//! conversion must not retype the buffered `"  "`. Whether the OS setting is on is passed in.

use std::time::Duration;

/// The two spaces must be typed within this gap.
pub const MAX_GAP: Duration = Duration::from_millis(450);

/// Rewrites the leading two spaces of `tail` to `". "` when the system rule is on, the converted
/// word ends in a letter or digit, and the spaces came within `max_gap`. Otherwise returns
/// `tail` unchanged. The length never changes.
pub fn rewritten_tail(
    tail: &str,
    converted_word: &str,
    system_enabled: bool,
    double_space_gap: Option<Duration>,
    max_gap: Duration,
) -> String {
    let ends_alnum = converted_word.chars().last().is_some_and(|c| c.is_alphabetic() || c.is_numeric());
    match (system_enabled, tail.strip_prefix("  "), double_space_gap) {
        (true, Some(rest), Some(gap)) if ends_alnum && gap <= max_gap => format!(". {rest}"),
        _ => tail.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rewrites_only_when_every_guard_holds() {
        let fast = Some(Duration::from_millis(200));
        assert_eq!(rewritten_tail("  x", "привет", true, fast, MAX_GAP), ". x");
        assert_eq!(rewritten_tail("  ", "привет", false, fast, MAX_GAP), "  ");
        assert_eq!(rewritten_tail(" ", "привет", true, fast, MAX_GAP), " ");
        assert_eq!(rewritten_tail("  ", "привет!", true, fast, MAX_GAP), "  ");
        assert_eq!(rewritten_tail("  ", "привет", true, None, MAX_GAP), "  ");
        assert_eq!(rewritten_tail("  ", "привет", true, Some(Duration::from_millis(500)), MAX_GAP), "  ");
    }
}
