//! `TextOutput` via `CGEventPost`. Every event is tagged with [`MARKER`] in
//! `kCGEventSourceUserData` (the tap ignores tagged events) and carries explicit flags, so held
//! modifiers never turn Backspace into Option+Backspace or text into shortcuts.

use keygloss_core::platform::{KeyCode, Modifiers, PlatformError, TextOutput};

use crate::cf::*;
use crate::keymap::{BACKSPACE, RETURN, TAB, code_for_key};

/// `kCGEventSourceUserData` tag on every event we post: "KGLS".
pub const MARKER: i64 = 0x4B47_4C53;

pub fn is_own_event(event: CGEventRef) -> bool {
    unsafe { CGEventGetIntegerValueField(event, kCGEventSourceUserData) == MARKER }
}

/// Unicode characters per event: the limit is about 20 UTF-16 units.
const CHUNK: usize = 12;

/// One unit of typed output.
#[derive(Debug, PartialEq, Eq)]
enum Unit {
    /// Up to [`CHUNK`] UTF-16 units, never splitting a surrogate pair.
    Text(Vec<u16>),
    /// A real key press (Return, Tab).
    Key(u16),
}

/// Splits text into Unicode chunks; `\n`, `\r\n` and lone `\r` become Return, `\t` becomes Tab.
fn text_units(text: &str) -> Vec<Unit> {
    let mut units = Vec::new();
    let mut chunk: Vec<u16> = Vec::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        let key = match c {
            '\r' if chars.peek() == Some(&'\n') => continue,
            '\n' | '\r' => Some(RETURN),
            '\t' => Some(TAB),
            _ => None,
        };
        if (key.is_some() || chunk.len() + c.len_utf16() > CHUNK) && !chunk.is_empty() {
            units.push(Unit::Text(std::mem::take(&mut chunk)));
        }
        match key {
            Some(k) => units.push(Unit::Key(k)),
            None => chunk.extend(c.encode_utf16(&mut [0; 2]).iter()),
        }
    }
    if !chunk.is_empty() {
        units.push(Unit::Text(chunk));
    }
    units
}

fn flags_of(mods: Modifiers) -> u64 {
    [
        (mods.ctrl, kCGEventFlagMaskControl),
        (mods.alt, kCGEventFlagMaskAlternate),
        (mods.shift, kCGEventFlagMaskShift),
        (mods.meta, kCGEventFlagMaskCommand),
    ]
    .into_iter()
    .filter_map(|(on, f)| on.then_some(f))
    .fold(0, |a, f| a | f)
}

/// Builds tagged events and posts them at the HID level.
struct Poster(Owned);

impl Poster {
    fn new() -> Result<Poster, PlatformError> {
        Owned::new(unsafe { CGEventSourceCreate(kCGEventSourceStatePrivate) })
            .map(Poster)
            .ok_or_else(|| PlatformError("CGEventSourceCreate failed".into()))
    }

    fn event(&self, code: u16, down: bool, flags: u64) -> Result<Owned, PlatformError> {
        let e = Owned::new(unsafe { CGEventCreateKeyboardEvent(self.0.0, code, down) })
            .ok_or_else(|| PlatformError("CGEventCreateKeyboardEvent failed".into()))?;
        unsafe {
            CGEventSetFlags(e.0, flags);
            CGEventSetIntegerValueField(e.0, kCGEventSourceUserData, MARKER);
        }
        Ok(e)
    }

    fn press(&self, code: u16, flags: u64) -> Result<(), PlatformError> {
        for down in [true, false] {
            let e = self.event(code, down, flags)?;
            unsafe { CGEventPost(kCGHIDEventTap, e.0) };
        }
        Ok(())
    }

    fn type_chars(&self, chars: &[u16]) -> Result<(), PlatformError> {
        for down in [true, false] {
            let e = self.event(0, down, 0)?;
            unsafe {
                CGEventKeyboardSetUnicodeString(e.0, chars.len(), chars.as_ptr());
                CGEventPost(kCGHIDEventTap, e.0);
            }
        }
        Ok(())
    }
}

/// `TextOutput` implementation.
#[derive(Clone, Copy, Debug, Default)]
pub struct MacOutput;

impl TextOutput for MacOutput {
    fn replace_text(&self, backspaces: usize, text: &str) -> Result<(), PlatformError> {
        let p = Poster::new()?;
        for _ in 0..backspaces {
            p.press(BACKSPACE, 0)?;
        }
        for unit in text_units(text) {
            match unit {
                Unit::Text(chars) => p.type_chars(&chars)?,
                Unit::Key(code) => p.press(code, 0)?,
            }
        }
        Ok(())
    }

    fn send_chord(&self, key: KeyCode, mods: Modifiers) -> Result<(), PlatformError> {
        let code = code_for_key(key).ok_or_else(|| PlatformError(format!("no mapping for {key:?}")))?;
        Poster::new()?.press(code, flags_of(mods))
    }

    /// Nothing to mask: releasing Option or Command alone opens no menu on macOS.
    fn send_mask_key(&self) -> Result<(), PlatformError> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(s: &str) -> Unit {
        Unit::Text(s.encode_utf16().collect())
    }

    #[test]
    fn splits_text() {
        assert_eq!(text_units("ab"), vec![text("ab")]);
        assert_eq!(text_units("a\nb"), vec![text("a"), Unit::Key(RETURN), text("b")]);
        assert_eq!(text_units("\r\n\r\t"), vec![Unit::Key(RETURN), Unit::Key(RETURN), Unit::Key(TAB)]);
        assert_eq!(text_units("привет мир!!"), vec![text("привет мир!!")]);
        assert_eq!(text_units("привет мир!!!"), vec![text("привет мир!!"), text("!")]);
        // A surrogate pair is never split.
        assert_eq!(text_units("12345678901😀"), vec![text("12345678901"), text("😀")]);
        assert!(text_units("").is_empty());
    }

    #[test]
    fn chord_flags() {
        assert_eq!(flags_of(Modifiers::NONE), 0);
        assert_eq!(flags_of(Modifiers::SHORTCUT), kCGEventFlagMaskCommand);
    }
}
