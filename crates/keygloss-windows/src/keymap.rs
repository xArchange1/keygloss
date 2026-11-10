//! `KeyCode` <-> Windows scan code / virtual key mapping.
//!
//! Scan codes are set 1 in a 16-bit form: the low byte is the make code and [`EXT`] marks the
//! E0 prefix (`LLKHF_EXTENDED`). This is also the form `MapVirtualKeyExW(MAPVK_VSC_TO_VK_EX)` takes.

use keygloss_core::platform::KeyCode::{self, *};
use windows::Win32::UI::Input::KeyboardAndMouse::*;

/// E0 prefix marker in a 16-bit scan code.
pub const EXT: u16 = 0xE000;

/// Keys identified by virtual key in the hook, because their scan codes are ambiguous
/// (Pause/NumLock share 0x45) or layout-free. The scan code is used when sending them.
pub const VK_KEYS: &[(KeyCode, VIRTUAL_KEY, u16)] = &[
    (Pause, VK_PAUSE, 0x45),
    (NumLock, VK_NUMLOCK, EXT | 0x45),
    (PrintScreen, VK_SNAPSHOT, EXT | 0x37),
    (ShiftLeft, VK_LSHIFT, 0x2A),
    (ShiftRight, VK_RSHIFT, 0x36),
    (ControlLeft, VK_LCONTROL, 0x1D),
    (ControlRight, VK_RCONTROL, EXT | 0x1D),
    (AltLeft, VK_LMENU, 0x38),
    (AltRight, VK_RMENU, EXT | 0x38),
    (MetaLeft, VK_LWIN, EXT | 0x5B),
    (MetaRight, VK_RWIN, EXT | 0x5C),
];

/// Keys identified by scan code (layout-independent physical position).
pub const SCAN_KEYS: &[(KeyCode, u16)] = &[
    (Escape, 0x01),
    (Digit1, 0x02), (Digit2, 0x03), (Digit3, 0x04), (Digit4, 0x05), (Digit5, 0x06),
    (Digit6, 0x07), (Digit7, 0x08), (Digit8, 0x09), (Digit9, 0x0A), (Digit0, 0x0B),
    (Minus, 0x0C), (Equal, 0x0D), (Backspace, 0x0E), (Tab, 0x0F),
    (KeyQ, 0x10), (KeyW, 0x11), (KeyE, 0x12), (KeyR, 0x13), (KeyT, 0x14),
    (KeyY, 0x15), (KeyU, 0x16), (KeyI, 0x17), (KeyO, 0x18), (KeyP, 0x19),
    (BracketLeft, 0x1A), (BracketRight, 0x1B), (Enter, 0x1C),
    (KeyA, 0x1E), (KeyS, 0x1F), (KeyD, 0x20), (KeyF, 0x21), (KeyG, 0x22),
    (KeyH, 0x23), (KeyJ, 0x24), (KeyK, 0x25), (KeyL, 0x26),
    (Semicolon, 0x27), (Quote, 0x28), (Backquote, 0x29), (Backslash, 0x2B),
    (KeyZ, 0x2C), (KeyX, 0x2D), (KeyC, 0x2E), (KeyV, 0x2F), (KeyB, 0x30),
    (KeyN, 0x31), (KeyM, 0x32), (Comma, 0x33), (Period, 0x34), (Slash, 0x35),
    (NumpadMultiply, 0x37), (Space, 0x39), (CapsLock, 0x3A),
    (F1, 0x3B), (F2, 0x3C), (F3, 0x3D), (F4, 0x3E), (F5, 0x3F),
    (F6, 0x40), (F7, 0x41), (F8, 0x42), (F9, 0x43), (F10, 0x44),
    (ScrollLock, 0x46),
    (Numpad7, 0x47), (Numpad8, 0x48), (Numpad9, 0x49), (NumpadSubtract, 0x4A),
    (Numpad4, 0x4B), (Numpad5, 0x4C), (Numpad6, 0x4D), (NumpadAdd, 0x4E),
    (Numpad1, 0x4F), (Numpad2, 0x50), (Numpad3, 0x51), (Numpad0, 0x52), (NumpadDecimal, 0x53),
    (IntlBackslash, 0x56), (F11, 0x57), (F12, 0x58),
    (F13, 0x64), (F14, 0x65), (F15, 0x66), (F16, 0x67), (F17, 0x68), (F18, 0x69),
    (F19, 0x6A), (F20, 0x6B), (F21, 0x6C), (F22, 0x6D), (F23, 0x6E), (F24, 0x76),
    (NumpadEnter, EXT | 0x1C), (NumpadDivide, EXT | 0x35),
    (Home, EXT | 0x47), (ArrowUp, EXT | 0x48), (PageUp, EXT | 0x49), (ArrowLeft, EXT | 0x4B),
    (ArrowRight, EXT | 0x4D), (End, EXT | 0x4F), (ArrowDown, EXT | 0x50), (PageDown, EXT | 0x51),
    (Insert, EXT | 0x52), (Delete, EXT | 0x53), (ContextMenu, EXT | 0x5D),
];

/// Maps a low-level hook event to a `KeyCode`.
pub fn key_from_hook(vk: u32, scan: u32, extended: bool) -> KeyCode {
    // AltGr sends a fake LControl with scan code 0x21D before RMenu; it is not a real Ctrl press.
    if scan & 0x200 != 0 {
        return Unidentified;
    }
    // Ctrl+Pause arrives as VK_CANCEL.
    if vk == u32::from(VK_CANCEL.0) {
        return Pause;
    }
    if let Some(&(key, _, _)) = VK_KEYS.iter().find(|e| u32::from(e.1.0) == vk) {
        return key;
    }
    let code = (scan & 0xFF) as u16 | if extended { EXT } else { 0 };
    SCAN_KEYS.iter().find(|e| e.1 == code).map_or(Unidentified, |e| e.0)
}

/// How to send `key`: a fixed virtual key (if it has one) and its 16-bit scan code.
/// Keys without a fixed virtual key get theirs from the target layout via `MapVirtualKeyExW`.
pub fn key_for_output(key: KeyCode) -> Option<(Option<VIRTUAL_KEY>, u16)> {
    if let Some(&(_, vk, scan)) = VK_KEYS.iter().find(|e| e.0 == key) {
        return Some((Some(vk), scan));
    }
    SCAN_KEYS.iter().find(|e| e.0 == key).map(|e| (None, e.1))
}

/// Keys that type a character (letters, digit row, punctuation).
pub fn is_char_key(key: KeyCode) -> bool {
    (KeyA..=IntlBackslash).contains(&key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scan_keys_round_trip() {
        for &(key, scan) in SCAN_KEYS {
            assert_eq!(key_from_hook(0, u32::from(scan & 0xFF), scan & EXT != 0), key, "{key:?}");
            assert_eq!(key_for_output(key), Some((None, scan)), "{key:?}");
        }
    }

    #[test]
    fn vk_keys_round_trip() {
        for &(key, vk, scan) in VK_KEYS {
            assert_eq!(key_from_hook(u32::from(vk.0), u32::from(scan & 0xFF), scan & EXT != 0), key);
            assert_eq!(key_for_output(key), Some((Some(vk), scan)));
        }
    }

    #[test]
    fn no_duplicates() {
        let mut keys: Vec<KeyCode> =
            SCAN_KEYS.iter().map(|e| e.0).chain(VK_KEYS.iter().map(|e| e.0)).collect();
        let mut scans: Vec<u16> = SCAN_KEYS.iter().map(|e| e.1).collect();
        let n = keys.len();
        keys.sort();
        keys.dedup();
        scans.sort();
        scans.dedup();
        assert_eq!(keys.len(), n);
        assert_eq!(scans.len(), SCAN_KEYS.len());
        // Every key except Unidentified is mapped.
        assert_eq!(n, KeyCode::Unidentified as usize);
    }

    #[test]
    fn special_cases() {
        // Pause vs NumLock share scan 0x45; the vk decides.
        assert_eq!(key_from_hook(u32::from(VK_PAUSE.0), 0x45, false), Pause);
        assert_eq!(key_from_hook(u32::from(VK_NUMLOCK.0), 0x45, true), NumLock);
        assert_eq!(key_from_hook(u32::from(VK_CANCEL.0), 0x46, true), Pause);
        // AltGr's fake LControl.
        assert_eq!(key_from_hook(u32::from(VK_LCONTROL.0), 0x21D, false), Unidentified);
        // Numpad with NumLock off still maps by position; arrows are extended.
        assert_eq!(key_from_hook(u32::from(VK_HOME.0), 0x47, false), Numpad7);
        assert_eq!(key_from_hook(u32::from(VK_HOME.0), 0x47, true), Home);
        assert_eq!(key_from_hook(0xAD, 0x20, true), Unidentified); // volume mute
        assert!(is_char_key(KeyA) && is_char_key(IntlBackslash) && is_char_key(Digit0));
        assert!(!is_char_key(Space) && !is_char_key(Enter));
    }
}
