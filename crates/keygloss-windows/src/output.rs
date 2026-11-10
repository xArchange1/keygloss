//! `TextOutput` via `SendInput`.

use keygloss_core::platform::{KeyCode, Modifiers, PlatformError, TextOutput};
use windows::Win32::UI::Input::KeyboardAndMouse::*;

use crate::keymap::{EXT, key_for_output};
use crate::layouts::foreground_hkl;

/// `dwExtraInfo` tag on every event we inject; the hook ignores events carrying it.
pub const MARKER: usize = 0x4B47_4C53; // "KGLS"

pub fn is_own_event(extra_info: usize) -> bool {
    extra_info == MARKER
}

/// One unit of typed output.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Unit {
    /// A UTF-16 code unit sent with `KEYEVENTF_UNICODE`.
    Char(u16),
    /// A real key press (Enter, Tab), with its scan code.
    Key(VIRTUAL_KEY, u16),
}

/// Splits text into UTF-16 units; `\n`, `\r\n` and lone `\r` become Enter, `\t` becomes Tab.
fn text_units(text: &str) -> Vec<Unit> {
    let mut units = Vec::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\r' if chars.peek() == Some(&'\n') => {}
            '\n' | '\r' => units.push(Unit::Key(VK_RETURN, 0x1C)),
            '\t' => units.push(Unit::Key(VK_TAB, 0x0F)),
            _ => units.extend(c.encode_utf16(&mut [0; 2]).iter().map(|&u| Unit::Char(u))),
        }
    }
    units
}

fn key_input(vk: VIRTUAL_KEY, scan: u16, up: bool) -> INPUT {
    let mut flags = if up { KEYEVENTF_KEYUP } else { KEYBD_EVENT_FLAGS(0) };
    if scan & EXT != 0 {
        flags |= KEYEVENTF_EXTENDEDKEY;
    }
    input(vk, scan & 0xFF, flags)
}

fn input(vk: VIRTUAL_KEY, scan: u16, flags: KEYBD_EVENT_FLAGS) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT { wVk: vk, wScan: scan, dwFlags: flags, time: 0, dwExtraInfo: MARKER },
        },
    }
}

fn press(out: &mut Vec<INPUT>, vk: VIRTUAL_KEY, scan: u16) {
    out.push(key_input(vk, scan, false));
    out.push(key_input(vk, scan, true));
}

const SHIFT: [(VIRTUAL_KEY, u16); 2] = [(VK_LSHIFT, 0x2A), (VK_RSHIFT, 0x36)];
const CTRL: [(VIRTUAL_KEY, u16); 2] = [(VK_LCONTROL, 0x1D), (VK_RCONTROL, EXT | 0x1D)];
const ALT_WIN: [(VIRTUAL_KEY, u16); 4] =
    [(VK_LMENU, 0x38), (VK_RMENU, EXT | 0x38), (VK_LWIN, EXT | 0x5B), (VK_RWIN, EXT | 0x5C)];

/// Sends `body` with every physically held modifier released first, so held keys do not turn
/// Backspace into Ctrl+Backspace etc. Shift and Ctrl are pressed again afterwards (the user is
/// still holding them). Alt and Win stay released: re-pressing them would make the later
/// physical release open the menu bar / Start menu. An unassigned key (vk 0xE8) is tapped before
/// releasing them for the same reason.
fn send_clean(body: Vec<INPUT>) -> Result<(), PlatformError> {
    let held = |keys: &[(VIRTUAL_KEY, u16)]| -> Vec<(VIRTUAL_KEY, u16)> {
        keys.iter().copied().filter(|k| unsafe { GetAsyncKeyState(i32::from(k.0.0)) } < 0).collect()
    };
    let restore = [held(&SHIFT), held(&CTRL)].concat();
    let alt_win = held(&ALT_WIN);

    let mut inputs = Vec::with_capacity(body.len() + 16);
    if !alt_win.is_empty() {
        press(&mut inputs, VIRTUAL_KEY(0xE8), 0);
    }
    for &(vk, scan) in alt_win.iter().chain(&restore) {
        inputs.push(key_input(vk, scan, true));
    }
    inputs.extend(body);
    for &(vk, scan) in &restore {
        inputs.push(key_input(vk, scan, false));
    }

    let sent = unsafe { SendInput(&inputs, size_of::<INPUT>() as i32) } as usize;
    if sent == inputs.len() {
        Ok(())
    } else {
        // Typically UIPI: the foreground window belongs to a more elevated process.
        Err(PlatformError(format!("SendInput sent {sent} of {} events", inputs.len())))
    }
}

/// `TextOutput` implementation. Every event is tagged with [`MARKER`].
#[derive(Clone, Copy, Debug, Default)]
pub struct WindowsOutput;

impl TextOutput for WindowsOutput {
    fn replace_text(&self, backspaces: usize, text: &str) -> Result<(), PlatformError> {
        let mut body = Vec::with_capacity(backspaces * 2 + text.len() * 2);
        for _ in 0..backspaces {
            press(&mut body, VK_BACK, 0x0E);
        }
        for unit in text_units(text) {
            match unit {
                Unit::Char(u) => {
                    body.push(input(VIRTUAL_KEY(0), u, KEYEVENTF_UNICODE));
                    body.push(input(VIRTUAL_KEY(0), u, KEYEVENTF_UNICODE | KEYEVENTF_KEYUP));
                }
                Unit::Key(vk, scan) => press(&mut body, vk, scan),
            }
        }
        send_clean(body)
    }

    fn send_chord(&self, key: KeyCode, mods: Modifiers) -> Result<(), PlatformError> {
        let (vk, scan) =
            key_for_output(key).ok_or_else(|| PlatformError(format!("no mapping for {key:?}")))?;
        let vk = match vk {
            Some(vk) => vk,
            None => {
                let vk = unsafe { MapVirtualKeyExW(u32::from(scan), MAPVK_VSC_TO_VK_EX, Some(foreground_hkl())) };
                if vk == 0 {
                    return Err(PlatformError(format!("no virtual key for {key:?}")));
                }
                VIRTUAL_KEY(vk as u16)
            }
        };
        let wanted: Vec<(VIRTUAL_KEY, u16)> = [
            (mods.ctrl, CTRL[0]),
            (mods.alt, ALT_WIN[0]),
            (mods.shift, SHIFT[0]),
            (mods.meta, ALT_WIN[2]),
        ]
        .into_iter()
        .filter_map(|(on, k)| on.then_some(k))
        .collect();

        let mut body = Vec::with_capacity(wanted.len() * 2 + 2);
        for &(vk, scan) in &wanted {
            body.push(key_input(vk, scan, false));
        }
        press(&mut body, vk, scan);
        for &(vk, scan) in wanted.iter().rev() {
            body.push(key_input(vk, scan, true));
        }
        send_clean(body)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_text() {
        let enter = Unit::Key(VK_RETURN, 0x1C);
        let tab = Unit::Key(VK_TAB, 0x0F);
        assert_eq!(text_units("ab"), vec![Unit::Char(0x61), Unit::Char(0x62)]);
        assert_eq!(text_units("п"), vec![Unit::Char(0x43F)]);
        assert_eq!(text_units("😀"), vec![Unit::Char(0xD83D), Unit::Char(0xDE00)]);
        assert_eq!(text_units("a\nb"), vec![Unit::Char(0x61), enter, Unit::Char(0x62)]);
        assert_eq!(text_units("\r\n\r\t"), vec![enter, enter, tab]);
        assert!(text_units("").is_empty());
    }

    #[test]
    fn marker_filter() {
        assert!(is_own_event(MARKER));
        assert!(!is_own_event(0));
        assert!(!is_own_event(MARKER + 1));
    }
}
