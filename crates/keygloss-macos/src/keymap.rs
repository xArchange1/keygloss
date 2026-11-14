//! `KeyCode` <-> macOS virtual key code (`kVK_*`, Carbon `Events.h`). macOS key codes name
//! physical positions, so the mapping is layout-independent in both directions.
//!
//! No Mac key sends Pause, ScrollLock, PrintScreen or F21-F24; they stay unmapped. Insert is
//! the Help key and NumLock the keypad Clear key, at the same positions.

use keygloss_core::platform::KeyCode::{self, *};

pub const KEYS: &[(KeyCode, u16)] = &[
    (KeyA, 0x00), (KeyS, 0x01), (KeyD, 0x02), (KeyF, 0x03), (KeyH, 0x04), (KeyG, 0x05),
    (KeyZ, 0x06), (KeyX, 0x07), (KeyC, 0x08), (KeyV, 0x09), (IntlBackslash, 0x0A), (KeyB, 0x0B),
    (KeyQ, 0x0C), (KeyW, 0x0D), (KeyE, 0x0E), (KeyR, 0x0F), (KeyY, 0x10), (KeyT, 0x11),
    (Digit1, 0x12), (Digit2, 0x13), (Digit3, 0x14), (Digit4, 0x15), (Digit6, 0x16), (Digit5, 0x17),
    (Equal, 0x18), (Digit9, 0x19), (Digit7, 0x1A), (Minus, 0x1B), (Digit8, 0x1C), (Digit0, 0x1D),
    (BracketRight, 0x1E), (KeyO, 0x1F), (KeyU, 0x20), (BracketLeft, 0x21), (KeyI, 0x22), (KeyP, 0x23),
    (Enter, 0x24), (KeyL, 0x25), (KeyJ, 0x26), (Quote, 0x27), (KeyK, 0x28), (Semicolon, 0x29),
    (Backslash, 0x2A), (Comma, 0x2B), (Slash, 0x2C), (KeyN, 0x2D), (KeyM, 0x2E), (Period, 0x2F),
    (Tab, 0x30), (Space, 0x31), (Backquote, 0x32), (Backspace, 0x33), (Escape, 0x35),
    (MetaRight, 0x36), (MetaLeft, 0x37), (ShiftLeft, 0x38), (CapsLock, 0x39), (AltLeft, 0x3A),
    (ControlLeft, 0x3B), (ShiftRight, 0x3C), (AltRight, 0x3D), (ControlRight, 0x3E),
    (F17, 0x40), (NumpadDecimal, 0x41), (NumpadMultiply, 0x43), (NumpadAdd, 0x45), (NumLock, 0x47),
    (NumpadDivide, 0x4B), (NumpadEnter, 0x4C), (NumpadSubtract, 0x4E), (F18, 0x4F), (F19, 0x50),
    (Numpad0, 0x52), (Numpad1, 0x53), (Numpad2, 0x54), (Numpad3, 0x55), (Numpad4, 0x56),
    (Numpad5, 0x57), (Numpad6, 0x58), (Numpad7, 0x59), (F20, 0x5A), (Numpad8, 0x5B), (Numpad9, 0x5C),
    (F5, 0x60), (F6, 0x61), (F7, 0x62), (F3, 0x63), (F8, 0x64), (F9, 0x65), (F11, 0x67), (F13, 0x69),
    (F16, 0x6A), (F14, 0x6B), (F10, 0x6D), (ContextMenu, 0x6E), (F12, 0x6F), (F15, 0x71),
    (Insert, 0x72), (Home, 0x73), (PageUp, 0x74), (Delete, 0x75), (F4, 0x76), (End, 0x77), (F2, 0x78),
    (PageDown, 0x79), (F1, 0x7A), (ArrowLeft, 0x7B), (ArrowRight, 0x7C), (ArrowDown, 0x7D), (ArrowUp, 0x7E),
];

pub const BACKSPACE: u16 = 0x33;
pub const RETURN: u16 = 0x24;
pub const TAB: u16 = 0x30;

pub fn key_from_code(code: u16) -> KeyCode {
    KEYS.iter().find(|e| e.1 == code).map_or(Unidentified, |e| e.0)
}

pub fn code_for_key(key: KeyCode) -> Option<u16> {
    KEYS.iter().find(|e| e.0 == key).map(|e| e.1)
}

/// Keys that type a character (letters, digit row, punctuation).
pub fn is_char_key(key: KeyCode) -> bool {
    (KeyA..=IntlBackslash).contains(&key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_and_unique() {
        let mut codes: Vec<u16> = KEYS.iter().map(|e| e.1).collect();
        let mut keys: Vec<KeyCode> = KEYS.iter().map(|e| e.0).collect();
        codes.sort();
        codes.dedup();
        keys.sort();
        keys.dedup();
        assert_eq!(codes.len(), KEYS.len());
        assert_eq!(keys.len(), KEYS.len());
        for &(key, code) in KEYS {
            assert_eq!(key_from_code(code), key);
            assert_eq!(code_for_key(key), Some(code));
        }
        // Every character key is mapped.
        for &(key, _) in KEYS.iter().filter(|e| is_char_key(e.0)) {
            assert!(code_for_key(key).is_some());
        }
        assert_eq!(KEYS.iter().filter(|e| is_char_key(e.0)).count(), (IntlBackslash as usize) - (KeyA as usize) + 1);
        assert_eq!(key_from_code(0x3F), Unidentified); // Fn
    }
}
