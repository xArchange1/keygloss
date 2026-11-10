//! Case toggle of the selection (`Engine.changeSelectionCase`).

/// Selections longer than this are refused (likely an accidental select-all).
pub const MAX_CHARS: usize = 5000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CaseToggleError {
    TooLong,
    /// Nothing changes (no letters).
    NoLetters,
}

/// Uppercase if the text has any lowercase letter, otherwise lowercase.
pub fn toggle_case(text: &str) -> Result<String, CaseToggleError> {
    if text.chars().count() > MAX_CHARS {
        return Err(CaseToggleError::TooLong);
    }
    let changed = if text.chars().any(char::is_lowercase) { text.to_uppercase() } else { text.to_lowercase() };
    if changed == text {
        return Err(CaseToggleError::NoLetters);
    }
    Ok(changed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toggles() {
        assert_eq!(toggle_case("phone").unwrap(), "PHONE");
        assert_eq!(toggle_case("PHONE").unwrap(), "phone");
        assert_eq!(toggle_case("Привет").unwrap(), "ПРИВЕТ");
        assert_eq!(toggle_case("123 !"), Err(CaseToggleError::NoLetters));
        assert_eq!(toggle_case(&"a".repeat(5001)), Err(CaseToggleError::TooLong));
        assert!(toggle_case(&"a".repeat(5000)).is_ok());
    }
}
