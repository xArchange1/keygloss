//! Hotkeys: gesture recognition (`EventTap.handleFlags` and the key-mode checks), the recorder's
//! capture (`HotkeyControl.capture`) and the recorder rules (`HotkeyGuard`), for Windows keys.

use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::platform::{KeyCode, KeyEvent, Modifiers};
use crate::settings::{Hotkey, Settings};

/// Two solo taps of a modifier within this window are a double tap.
pub const DOUBLE_TAP_WINDOW: Duration = Duration::from_millis(300);

/// Which feature a hotkey belongs to (for conflict checks).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HotkeyTarget {
    Manual,
    CaseChange,
    PlainPaste,
    SnippetPick,
}

impl HotkeyTarget {
    pub const ALL: [HotkeyTarget; 4] =
        [HotkeyTarget::Manual, HotkeyTarget::CaseChange, HotkeyTarget::PlainPaste, HotkeyTarget::SnippetPick];

    /// The active hotkey of this feature, `None` when the feature is off.
    pub fn hotkey(self, s: &Settings) -> Option<Hotkey> {
        match self {
            HotkeyTarget::Manual => Some(s.hotkey),
            HotkeyTarget::CaseChange => s.case_change_hotkey,
            HotkeyTarget::PlainPaste => s.plain_paste.then_some(s.plain_paste_hotkey),
            HotkeyTarget::SnippetPick => s.snippet_pick_hotkey,
        }
    }

    /// Serialized name (`hkrec.what.<name>` in the UI).
    pub fn name(self) -> &'static str {
        match self {
            HotkeyTarget::Manual => "manual",
            HotkeyTarget::CaseChange => "caseChange",
            HotkeyTarget::PlainPaste => "plainPaste",
            HotkeyTarget::SnippetPick => "snippetPick",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HotkeyVerdict {
    /// Free to assign.
    Ok,
    /// Assignable, but shadows a known system shortcut.
    Warn,
    /// Refused.
    Blocked,
}

/// Result of [`check_hotkey`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HotkeyCheck {
    pub verdict: HotkeyVerdict,
    /// i18n key explaining a warn/blocked verdict (`hkrec.reason.*`).
    pub reason: Option<String>,
    /// Our own feature that already uses this hotkey (verdict is then `Blocked`).
    pub conflicts_with: Option<HotkeyTarget>,
}

const NONE: Modifiers = Modifiers::NONE;

/// The modifier flag of a modifier key (either side).
pub fn modifier_of(key: KeyCode) -> Option<Modifiers> {
    use KeyCode::*;
    Some(match key {
        ShiftLeft | ShiftRight => Modifiers { shift: true, ..NONE },
        ControlLeft | ControlRight => Modifiers { ctrl: true, ..NONE },
        AltLeft | AltRight => Modifiers { alt: true, ..NONE },
        MetaLeft | MetaRight => Modifiers { meta: true, ..NONE },
        _ => return None,
    })
}

fn union(a: Modifiers, b: Modifiers) -> Modifiers {
    Modifiers { ctrl: a.ctrl || b.ctrl, alt: a.alt || b.alt, shift: a.shift || b.shift, meta: a.meta || b.meta }
}

/// `a` holds nothing outside `b`.
fn subset(a: Modifiers, b: Modifiers) -> bool {
    (!a.ctrl || b.ctrl) && (!a.alt || b.alt) && (!a.shift || b.shift) && (!a.meta || b.meta)
}

fn count(m: Modifiers) -> usize {
    [m.ctrl, m.alt, m.shift, m.meta].iter().filter(|&&b| b).count()
}

/// Modifiers held after a modifier key event (`mods` excludes the key itself).
pub(crate) fn held_after(ev: &KeyEvent) -> Modifiers {
    match modifier_of(ev.key) {
        Some(m) if ev.down => union(ev.mods, m),
        _ => ev.mods,
    }
}

/// Keys that are safe as hotkeys without modifiers: nothing types with them.
pub fn is_bare_safe(key: KeyCode) -> bool {
    matches!(key, KeyCode::Pause | KeyCode::ScrollLock) || (KeyCode::F13..=KeyCode::F24).contains(&key)
}

/// Keys that produce text or edit it.
fn is_typing_key(key: KeyCode) -> bool {
    (KeyCode::KeyA..=KeyCode::IntlBackslash).contains(&key)
        || (KeyCode::Numpad0..=KeyCode::NumpadEnter).contains(&key)
        || matches!(key, KeyCode::Space | KeyCode::Enter | KeyCode::Tab | KeyCode::Backspace)
}

/// A key-mode hotkey matches this key-down: same key, exactly the same modifiers. A saved bare key
/// is only honoured for keys that are safe bare (as the original refused to intercept a bare key).
pub(crate) fn key_matches(hk: &Hotkey, ev: &KeyEvent) -> bool {
    match *hk {
        Hotkey::Key { key, mods } => ev.key == key && ev.mods == mods && (!mods.is_empty() || is_bare_safe(key)),
        _ => false,
    }
}

/// A modifier-only hotkey that involves Alt or Win: releasing those after it fires would open the
/// window menu or the Start menu unless a mask key is sent first.
pub(crate) fn needs_mask(hk: &Hotkey) -> bool {
    let alt_win = |m: Modifiers| m.alt || m.meta;
    match *hk {
        Hotkey::Combo { mods } => alt_win(mods),
        Hotkey::ModKey { key } | Hotkey::DoubleTap { key } => modifier_of(key).is_some_and(alt_win),
        Hotkey::Key { .. } => false,
    }
}

/// Modifier-gesture state of one hotkey (`comboArmed`, `modkeyArmed`, `lastModTap`, ...).
#[derive(Clone, Debug, Default)]
pub(crate) struct Gesture {
    combo_armed: bool,
    other_key_during_combo: bool,
    modkey_armed: bool,
    last_tap: Option<Instant>,
    other_key_between_taps: bool,
}

impl Gesture {
    /// A modifier-only chord is held and may still fire.
    pub fn combo_armed(&self) -> bool {
        self.combo_armed
    }

    /// A non-modifier key went down.
    pub fn other_key(&mut self) {
        if self.combo_armed {
            self.other_key_during_combo = true;
        }
        self.modkey_armed = false;
        self.other_key_between_taps = true;
    }

    /// A mouse button went down.
    pub fn mouse(&mut self) {
        self.modkey_armed = false;
        self.other_key_between_taps = true;
        self.other_key_during_combo = true;
    }

    /// A modifier key changed; `held` is the modifier state after the change. True = fire.
    pub fn modifier(&mut self, hk: &Hotkey, key: KeyCode, down: bool, held: Modifiers, now: Instant) -> bool {
        match *hk {
            Hotkey::ModKey { key: target } => {
                let Some(mask) = modifier_of(target) else { return false };
                if key != target {
                    self.modkey_armed = false;
                    return false;
                }
                if down {
                    self.modkey_armed = subset(held, mask);
                    false
                } else {
                    std::mem::take(&mut self.modkey_armed)
                }
            }
            Hotkey::DoubleTap { key: target } => {
                let Some(mask) = modifier_of(target) else { return false };
                if !down || modifier_of(key) != Some(mask) {
                    return false;
                }
                if !subset(held, mask) {
                    self.last_tap = None;
                    self.other_key_between_taps = false;
                    return false;
                }
                let fire = self.last_tap.is_some_and(|t| now.saturating_duration_since(t) < DOUBLE_TAP_WINDOW)
                    && !self.other_key_between_taps;
                self.last_tap = if fire { None } else { Some(now) };
                self.other_key_between_taps = false;
                fire
            }
            Hotkey::Combo { mods } => {
                if mods.is_empty() {
                    return false;
                }
                if held == mods {
                    self.combo_armed = true;
                    self.other_key_during_combo = false;
                    false
                } else if self.combo_armed && held.is_empty() {
                    self.combo_armed = false;
                    !self.other_key_during_combo
                } else {
                    if self.combo_armed && !subset(held, mods) {
                        self.combo_armed = false;
                    }
                    false
                }
            }
            Hotkey::Key { .. } => false,
        }
    }
}

/// Recorder capture: a key with modifiers, a modifier chord released together, a single modifier
/// tapped alone, or the same modifier tapped twice within [`DOUBLE_TAP_WINDOW`].
#[derive(Clone, Debug, Default)]
pub(crate) struct Recorder {
    peak: Modifiers,
    peak_key: Option<KeyCode>,
    /// A key was captured with modifiers held: ignore modifier releases until all are up.
    awaiting_release: bool,
    last_solo: Option<(KeyCode, Instant)>,
}

impl Recorder {
    pub fn key_down(&mut self, ev: &KeyEvent) -> Option<Hotkey> {
        if ev.repeat {
            return None;
        }
        self.peak = NONE;
        self.peak_key = None;
        self.last_solo = None;
        self.awaiting_release = !ev.mods.is_empty();
        Some(Hotkey::Key { key: ev.key, mods: ev.mods })
    }

    pub fn modifier(&mut self, key: KeyCode, down: bool, held: Modifiers, now: Instant) -> Option<Hotkey> {
        if self.awaiting_release {
            if held.is_empty() {
                self.awaiting_release = false;
            }
            return None;
        }
        if down {
            if self.peak.is_empty() && count(held) == 1 {
                self.peak_key = Some(key);
            }
            if count(held) >= count(self.peak) {
                self.peak = held;
            }
            return None;
        }
        if !held.is_empty() {
            return None;
        }
        let peak = std::mem::replace(&mut self.peak, NONE);
        let peak_key = self.peak_key.take();
        match (count(peak), peak_key) {
            (n, _) if n >= 2 => {
                self.last_solo = None;
                Some(Hotkey::Combo { mods: peak })
            }
            (1, Some(k)) => match self.last_solo.take() {
                Some((prev, at))
                    if modifier_of(prev) == modifier_of(k)
                        && now.saturating_duration_since(at) < DOUBLE_TAP_WINDOW =>
                {
                    Some(Hotkey::DoubleTap { key: prev })
                }
                _ => {
                    self.last_solo = Some((k, now));
                    Some(Hotkey::ModKey { key: k })
                }
            },
            _ => None,
        }
    }
}

/// The same physical trigger (`HotkeyGuard.sameTrigger`): a double tap matches either side of its
/// modifier, including a single-modifier hotkey on that modifier.
pub fn same_trigger(a: &Hotkey, b: &Hotkey) -> bool {
    use Hotkey::*;
    match (*a, *b) {
        (DoubleTap { key: x }, DoubleTap { key: y })
        | (DoubleTap { key: x }, ModKey { key: y })
        | (ModKey { key: y }, DoubleTap { key: x }) => modifier_of(x) == modifier_of(y),
        (ModKey { key: x }, ModKey { key: y }) => x == y,
        (Combo { mods: x }, Combo { mods: y }) => x == y,
        (Key { key: k1, mods: m1 }, Key { key: k2, mods: m2 }) => k1 == k2 && m1 == m2,
        _ => false,
    }
}

fn m(ctrl: bool, alt: bool, shift: bool, meta: bool) -> Modifiers {
    Modifiers { ctrl, alt, shift, meta }
}

/// Shortcuts Windows and every app rely on: never taken.
fn is_critical(key: KeyCode, mods: Modifiers) -> bool {
    use KeyCode::*;
    let ctrl = m(true, false, false, false);
    (mods == ctrl && matches!(key, KeyC | KeyV | KeyX | KeyZ | KeyA | KeyS | KeyW | KeyN | KeyO | KeyP | KeyF | KeyY | KeyT))
        || (key == Tab && (mods == m(false, true, false, false) || mods == m(false, true, true, false)))
        || (key == F4 && mods == m(false, true, false, false))
        || (key == KeyL && mods == m(false, false, false, true))
        || (key == Delete && mods == m(true, true, false, false))
        || (key == Escape && mods == m(true, false, true, false))
}

/// Combos Windows uses to switch the input language.
fn is_layout_switch(hk: &Hotkey) -> bool {
    match *hk {
        Hotkey::Combo { mods } => mods == m(false, true, true, false) || mods == m(true, false, true, false),
        Hotkey::Key { key: KeyCode::Space, mods } => mods.meta && !mods.ctrl && !mods.alt,
        _ => false,
    }
}

/// Known system shortcuts: Win key combinations (and the Win key alone opens Start), plus a few
/// window-management keys.
fn is_system(hk: &Hotkey) -> bool {
    use KeyCode::*;
    match *hk {
        Hotkey::Key { key, mods } => {
            mods.meta
                || (key == Escape && mods == m(true, false, false, false))
                || (key == Escape && mods == m(false, true, false, false))
                || (key == Space && mods == m(false, true, false, false))
                || (key == Tab && mods == m(true, true, false, false))
                || (key == PrintScreen && !mods.is_empty())
        }
        Hotkey::Combo { mods } => mods.meta,
        Hotkey::ModKey { key } | Hotkey::DoubleTap { key } => matches!(key, MetaLeft | MetaRight),
    }
}

/// The recorder's verdict on assigning `hotkey` to `target` (`HotkeyGuard.verdict`, `ourBusy`,
/// adapted to Windows): refusals first, then clashes with our other hotkeys, then warnings.
pub fn check_hotkey(target: HotkeyTarget, hotkey: &Hotkey, settings: &Settings) -> HotkeyCheck {
    let verdict = |verdict, reason: Option<&str>| HotkeyCheck {
        verdict,
        reason: reason.map(str::to_string),
        conflicts_with: None,
    };
    let blocked = |reason| verdict(HotkeyVerdict::Blocked, Some(reason));
    match *hotkey {
        Hotkey::Key { key, mods } => {
            if key.is_modifier() || key == KeyCode::Unidentified {
                return blocked("hkrec.reason.bare");
            }
            let shift_only = mods == m(false, false, true, false);
            if (mods.is_empty() || shift_only) && is_typing_key(key) {
                return blocked("hkrec.reason.needMods");
            }
            if mods.is_empty() && !is_bare_safe(key) {
                return blocked("hkrec.reason.bare");
            }
            if is_critical(key, mods) {
                return blocked("hkrec.reason.critical");
            }
        }
        Hotkey::Combo { mods } if mods.is_empty() => return blocked("hkrec.reason.bare"),
        Hotkey::ModKey { key } | Hotkey::DoubleTap { key } if !key.is_modifier() => {
            return blocked("hkrec.reason.bare");
        }
        _ => {}
    }
    for other in HotkeyTarget::ALL {
        if other != target
            && let Some(o) = other.hotkey(settings)
            && same_trigger(hotkey, &o)
        {
            return HotkeyCheck { verdict: HotkeyVerdict::Blocked, reason: None, conflicts_with: Some(other) };
        }
    }
    if is_layout_switch(hotkey) {
        return verdict(HotkeyVerdict::Warn, Some("hkrec.reason.layoutSwitch"));
    }
    if is_system(hotkey) {
        return verdict(HotkeyVerdict::Warn, Some("hkrec.reason.system"));
    }
    verdict(HotkeyVerdict::Ok, None)
}

/// The first pair of our active hotkeys that share a trigger.
pub fn active_clash(settings: &Settings) -> Option<(HotkeyTarget, HotkeyTarget)> {
    let active: Vec<(HotkeyTarget, Hotkey)> =
        HotkeyTarget::ALL.iter().filter_map(|&t| t.hotkey(settings).map(|h| (t, h))).collect();
    for (i, (a, ha)) in active.iter().enumerate() {
        for (b, hb) in &active[i + 1..] {
            if same_trigger(ha, hb) {
                return Some((*a, *b));
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(key: KeyCode, mods: Modifiers) -> Hotkey {
        Hotkey::Key { key, mods }
    }

    fn check(target: HotkeyTarget, hk: Hotkey, s: &Settings) -> (HotkeyVerdict, Option<String>, Option<HotkeyTarget>) {
        let c = check_hotkey(target, &hk, s);
        (c.verdict, c.reason, c.conflicts_with)
    }

    #[test]
    fn verdicts() {
        use HotkeyVerdict::*;
        let s = Settings::default();
        let ctrl = m(true, false, false, false);
        let r = |x: &str| Some(x.to_string());
        assert_eq!(check(HotkeyTarget::CaseChange, key(KeyCode::KeyK, NONE), &s), (Blocked, r("hkrec.reason.needMods"), None));
        assert_eq!(check(HotkeyTarget::CaseChange, key(KeyCode::KeyK, m(false, false, true, false)), &s), (Blocked, r("hkrec.reason.needMods"), None));
        assert_eq!(check(HotkeyTarget::CaseChange, key(KeyCode::F5, NONE), &s), (Blocked, r("hkrec.reason.bare"), None));
        assert_eq!(check(HotkeyTarget::CaseChange, key(KeyCode::F13, NONE), &s).0, Ok);
        assert_eq!(check(HotkeyTarget::Manual, key(KeyCode::Pause, NONE), &s).0, Ok);
        assert_eq!(check(HotkeyTarget::CaseChange, key(KeyCode::KeyC, ctrl), &s), (Blocked, r("hkrec.reason.critical"), None));
        assert_eq!(check(HotkeyTarget::CaseChange, key(KeyCode::Tab, m(false, true, false, false)), &s).1, r("hkrec.reason.critical"));
        assert_eq!(check(HotkeyTarget::CaseChange, key(KeyCode::KeyL, m(false, false, false, true)), &s).1, r("hkrec.reason.critical"));
        assert_eq!(check(HotkeyTarget::CaseChange, key(KeyCode::Delete, m(true, true, false, false)), &s).1, r("hkrec.reason.critical"));
        assert_eq!(check(HotkeyTarget::CaseChange, key(KeyCode::KeyE, m(false, false, false, true)), &s), (Warn, r("hkrec.reason.system"), None));
        assert_eq!(check(HotkeyTarget::CaseChange, key(KeyCode::Space, m(false, false, false, true)), &s), (Warn, r("hkrec.reason.layoutSwitch"), None));
        assert_eq!(check(HotkeyTarget::Manual, Hotkey::Combo { mods: m(false, true, true, false) }, &s).1, r("hkrec.reason.layoutSwitch"));
        assert_eq!(check(HotkeyTarget::Manual, Hotkey::Combo { mods: m(true, false, true, false) }, &s).1, r("hkrec.reason.layoutSwitch"));
        assert_eq!(check(HotkeyTarget::Manual, Hotkey::ModKey { key: KeyCode::MetaLeft }, &s).1, r("hkrec.reason.system"));
        assert_eq!(check(HotkeyTarget::Manual, Hotkey::ModKey { key: KeyCode::KeyA }, &s).0, Blocked);
        assert_eq!(check(HotkeyTarget::Manual, Hotkey::DoubleTap { key: KeyCode::ShiftLeft }, &s).0, Ok);
        assert_eq!(check(HotkeyTarget::CaseChange, key(KeyCode::KeyU, m(true, true, false, false)), &s).0, Ok);
    }

    #[test]
    fn clashes_with_our_hotkeys() {
        let mut s = Settings::default();
        // Default manual hotkey is Pause.
        let c = check_hotkey(HotkeyTarget::CaseChange, &key(KeyCode::Pause, NONE), &s);
        assert_eq!((c.verdict, c.conflicts_with), (HotkeyVerdict::Blocked, Some(HotkeyTarget::Manual)));
        // Re-assigning a feature its own hotkey is not a clash.
        assert_eq!(check_hotkey(HotkeyTarget::Manual, &key(KeyCode::Pause, NONE), &s).verdict, HotkeyVerdict::Ok);
        // Plain paste counts only when enabled.
        let pp = s.plain_paste_hotkey;
        assert_eq!(check_hotkey(HotkeyTarget::CaseChange, &pp, &s).conflicts_with, None);
        s.plain_paste = true;
        assert_eq!(check_hotkey(HotkeyTarget::CaseChange, &pp, &s).conflicts_with, Some(HotkeyTarget::PlainPaste));
        // A double tap covers both sides of its modifier.
        s.hotkey = Hotkey::DoubleTap { key: KeyCode::ShiftLeft };
        let c = check_hotkey(HotkeyTarget::SnippetPick, &Hotkey::ModKey { key: KeyCode::ShiftRight }, &s);
        assert_eq!(c.conflicts_with, Some(HotkeyTarget::Manual));
        let c = check_hotkey(HotkeyTarget::SnippetPick, &Hotkey::ModKey { key: KeyCode::AltRight }, &s);
        assert_eq!(c.conflicts_with, None);
        assert_eq!(active_clash(&s), None);
        s.case_change_hotkey = Some(Hotkey::DoubleTap { key: KeyCode::ShiftRight });
        assert_eq!(active_clash(&s), Some((HotkeyTarget::Manual, HotkeyTarget::CaseChange)));
    }
}
