//! `TextOutput` through the virtual keyboard. A uinput device sends key codes, not characters,
//! so text is typed as the keys that produce it in a layout: the current one where it can, else
//! another installed layout (switched to for those characters, then back). Characters no layout
//! has are pasted through the clipboard. Held modifiers are released while typing, so they never
//! turn text into shortcuts.

use std::time::Duration;

use keygloss_core::platform::{Clipboard, KeyCode, KeyTable, LayoutId, Modifiers, PlatformError, TextOutput};

use crate::clipboard::LinuxClipboard;
use crate::keymap::{BACKSPACE, ENTER, LEFT_CTRL, TAB, code_for_key};
use crate::{hook, layouts, vkbd};

/// Pause after each key press, so slow clients keep up.
const KEY_DELAY: Duration = Duration::from_millis(2);
/// Pause around a layout switch, so no key lands in the wrong layout.
const SWITCH_SETTLE: Duration = Duration::from_millis(30);
/// How long the clipboard keeps pasted text before the previous content comes back.
const PASTE_RESTORE: Duration = Duration::from_millis(500);

/// One step of typed output.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Step {
    /// A key press; `cased` keys are inverted by Caps Lock.
    Key { code: u16, shift: bool, cased: bool },
    Switch(LayoutId),
    Paste(String),
}

/// The key that types `c` in `table`, preferring no Shift.
fn find(table: &KeyTable, c: char) -> Option<(u16, bool)> {
    let mut best: Option<(KeyCode, bool)> = None;
    for (&(key, shift), &ch) in table {
        if ch == c && best.is_none_or(|(k, s)| (shift, key) < (s, k)) {
            best = Some((key, shift));
        }
    }
    best.and_then(|(key, shift)| Some((code_for_key(key)?, shift)))
}

/// Plans typing `text` starting in layout `current`; ends back in `current`.
fn plan(text: &str, current: Option<&LayoutId>, layouts: &[(LayoutId, KeyTable)]) -> Vec<Step> {
    let table = |id: Option<&LayoutId>| id.and_then(|id| layouts.iter().find(|l| &l.0 == id)).map(|l| &l.1);
    let mut steps = Vec::new();
    let mut active = current.cloned();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        let special = match c {
            '\r' if chars.peek() == Some(&'\n') => continue,
            '\n' | '\r' => Some(ENTER),
            '\t' => Some(TAB),
            _ => None,
        };
        if let Some(code) = special {
            steps.push(Step::Key { code, shift: false, cased: false });
            continue;
        }
        let cased = c.to_lowercase().ne(c.to_uppercase());
        let here = table(active.as_ref()).and_then(|t| find(t, c));
        let elsewhere = || layouts.iter().find_map(|(id, t)| find(t, c).map(|k| (id, k)));
        match (here, elsewhere()) {
            (Some((code, shift)), _) => steps.push(Step::Key { code, shift, cased }),
            (None, Some((id, (code, shift)))) => {
                steps.push(Step::Switch(id.clone()));
                active = Some(id.clone());
                steps.push(Step::Key { code, shift, cased });
            }
            (None, None) => match steps.last_mut() {
                Some(Step::Paste(s)) => s.push(c),
                _ => steps.push(Step::Paste(c.into())),
            },
        }
    }
    if let Some(current) = current
        && active.as_ref() != Some(current)
    {
        steps.push(Step::Switch(current.clone()));
    }
    steps
}

fn io(e: std::io::Error) -> PlatformError {
    PlatformError(format!("uinput: {e}"))
}

/// Presses one key (with Shift if asked), then pauses outside the device lock.
fn press(code: u16, shift: bool) -> Result<(), PlatformError> {
    vkbd::with(|k| {
        if shift {
            k.key(42, true)?;
        }
        k.tap(code)?;
        if shift {
            k.key(42, false)?;
        }
        Ok(())
    })?
    .map_err(io)?;
    std::thread::sleep(KEY_DELAY);
    Ok(())
}

fn chord(code: u16, mods: Modifiers) -> Result<(), PlatformError> {
    let codes: Vec<u16> =
        [(mods.ctrl, 29), (mods.alt, 56), (mods.shift, 42), (mods.meta, 125)].into_iter().filter_map(|(on, c)| on.then_some(c)).collect();
    vkbd::with(|k| {
        for &c in &codes {
            k.key(c, true)?;
        }
        k.tap(code)?;
        for &c in codes.iter().rev() {
            k.key(c, false)?;
        }
        Ok(())
    })?
    .map_err(io)
}

/// Pastes `text` with Ctrl+V and puts the previous clipboard back a moment later.
fn paste(text: &str) -> Result<(), PlatformError> {
    let cb = LinuxClipboard;
    let previous = cb.snapshot();
    cb.set_text(text)?;
    chord(code_for_key(KeyCode::KeyV).expect("mapped"), Modifiers::SHORTCUT)?;
    std::thread::spawn(move || {
        std::thread::sleep(PASTE_RESTORE);
        let _ = LinuxClipboard.restore(&previous);
    });
    std::thread::sleep(KEY_DELAY);
    Ok(())
}

fn run(steps: &[Step]) -> Result<(), PlatformError> {
    let caps = hook::CAPS_LOCK.load(std::sync::atomic::Ordering::Relaxed);
    for step in steps {
        match step {
            &Step::Key { code, shift, cased } => press(code, shift != (caps && cased))?,
            Step::Switch(id) => {
                // The keys typed so far reach the desktop on another path than the switch.
                std::thread::sleep(SWITCH_SETTLE);
                layouts::switch(id)?;
                std::thread::sleep(SWITCH_SETTLE);
            }
            Step::Paste(text) => paste(text)?,
        }
    }
    Ok(())
}

/// `TextOutput` implementation.
#[derive(Clone, Copy, Debug, Default)]
pub struct LinuxOutput;

impl TextOutput for LinuxOutput {
    fn replace_text(&self, backspaces: usize, text: &str) -> Result<(), PlatformError> {
        let (current, tables) = layouts::snapshot();
        let steps = plan(text, current.as_ref(), &tables);
        let held = vkbd::with(|k| k.release_modifiers())?.map_err(io)?;
        let mut result = (0..backspaces).try_for_each(|_| press(BACKSPACE, false));
        if result.is_ok() {
            result = run(&steps);
        }
        vkbd::with(|k| held.iter().try_for_each(|&c| k.key(c, true)))?.map_err(io)?;
        result
    }

    fn send_chord(&self, key: KeyCode, mods: Modifiers) -> Result<(), PlatformError> {
        let code = code_for_key(key).ok_or_else(|| PlatformError(format!("no mapping for {key:?}")))?;
        let held = vkbd::with(|k| k.release_modifiers())?.map_err(io)?;
        let result = chord(code, mods);
        vkbd::with(|k| held.iter().try_for_each(|&c| k.key(c, true)))?.map_err(io)?;
        result
    }

    /// A Ctrl tap: with it, releasing Meta alone does not open the application launcher.
    fn send_mask_key(&self) -> Result<(), PlatformError> {
        vkbd::with(|k| k.tap(LEFT_CTRL))?.map_err(io)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table(pairs: &[(KeyCode, bool, char)]) -> KeyTable {
        pairs.iter().map(|&(k, s, c)| ((k, s), c)).collect()
    }

    #[test]
    fn plans_keys_switches_and_pastes() {
        let us = LayoutId("us".into());
        let ru = LayoutId("ru".into());
        let layouts = vec![
            (us.clone(), table(&[(KeyCode::KeyA, false, 'a'), (KeyCode::KeyA, true, 'A'), (KeyCode::Digit1, false, '1')])),
            (ru.clone(), table(&[(KeyCode::KeyF, false, 'а'), (KeyCode::KeyF, true, 'А'), (KeyCode::Digit1, false, '1')])),
        ];
        let a = Step::Key { code: 30, shift: false, cased: true };
        let ru_a = Step::Key { code: 33, shift: false, cased: true };
        assert_eq!(plan("aA", Some(&us), &layouts), vec![a.clone(), Step::Key { code: 30, shift: true, cased: true }]);
        assert_eq!(
            plan("а1\n", Some(&us), &layouts),
            vec![
                Step::Switch(ru.clone()),
                ru_a,
                Step::Key { code: 2, shift: false, cased: false },
                Step::Key { code: ENTER, shift: false, cased: false },
                Step::Switch(us.clone()),
            ]
        );
        assert_eq!(plan("😀😀a", Some(&us), &layouts), vec![Step::Paste("😀😀".into()), a]);
        assert_eq!(plan("\r\n\t", None, &[]), vec![
            Step::Key { code: ENTER, shift: false, cased: false },
            Step::Key { code: TAB, shift: false, cased: false },
        ]);
    }
}
