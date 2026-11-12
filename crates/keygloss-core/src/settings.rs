//! Persisted settings. Field names serialize to the original app's setting keys (camelCase).
//!
//! Differences from the original's storage:
//! - Hotkeys are one [`Hotkey`] value per feature instead of the original's separate
//!   `*Mode` / `*KeyCode` / `*Mods` / `*KeyLabel` keys (those hold macOS keycodes and flags).
//!   Labels are derived from [`KeyCode`] by the UI.
//! - Ignored words are seeded through the default value instead of `didSeedExc*` flags.
//! - `soundName`, `soundEnabled` and `silentMode` are one [`Sound`] choice (`sound`): off, a
//!   synthesized sound or the user's own file.
//! - Launch at login is not stored here (the autostart plugin owns it).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::platform::{KeyCode, Modifiers};

/// A global hotkey. Serialized with a `mode` tag using the original's mode names.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "lowercase")]
pub enum Hotkey {
    /// Modifier-only chord, fires on release if no other key was pressed while held.
    Combo { mods: Modifiers },
    /// Two solo taps of a modifier within 300 ms. `key` names the modifier; either side counts.
    DoubleTap { key: KeyCode },
    /// Solo tap of one specific modifier key; left and right are distinct.
    ModKey { key: KeyCode },
    /// A non-modifier key with optional modifiers.
    Key { key: KeyCode, mods: Modifiers },
}

/// Per-app correction mode (`appExceptionModes` values).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AppMode {
    /// No automatic correction in this app.
    Off,
    /// Only obvious words are corrected.
    Soft,
}

/// What a correction plays (`sound`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Sound {
    Off,
    Beep,
    Whoosh,
    /// The user's own audio file.
    Custom,
}

/// Interface theme (`appTheme`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    System,
    Light,
    Dark,
}

/// Interface language (`language`). Picked from the OS language on install.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UiLanguage {
    Ru,
    En,
}

/// Which GitHub releases the updater offers (`updateChannel`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UpdateChannel {
    Stable,
    /// Pre-releases too.
    Beta,
}

/// Every persisted setting and engine-learned list. Missing keys load as defaults.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    // Layout switching
    pub auto_enabled: bool,
    pub live_fix_enabled: bool,
    /// Automatic fixes skip words whose fixed form has fewer letters. 1 = no limit.
    pub auto_min_letters: u32,
    /// Hidden kill switch: replace inside the hook callback (atomic live fix).
    pub inline_live_fix: bool,
    pub trigger_space: bool,
    pub trigger_enter: bool,
    pub trigger_tab: bool,
    /// Hidden: convert the pending word before Enter reaches the app.
    pub enter_pre_convert: bool,
    pub arrows_cancel: bool,
    pub developer_mode: bool,
    pub dedupe_chatter: bool,
    pub group_convert: bool,
    pub learn_on_undo_enabled: bool,
    pub learn_on_switch_enabled: bool,

    // Typo fixes
    pub typo_fix: bool,
    pub two_caps_fix: bool,

    // Sound
    pub sound: Sound,
    /// 0.0..=1.0; 0 means do not play.
    pub sound_volume: f64,
    /// Name of the file picked for [`Sound::Custom`] (the app keeps its own copy); empty = none.
    pub sound_file: String,

    // Hotkeys
    /// Manual convert / undo. Default: Pause/Break.
    pub hotkey: Hotkey,
    /// Case toggle of the selection; `None` = off.
    pub case_change_hotkey: Option<Hotkey>,
    /// Paste without formatting, active when `plain_paste` is on.
    pub plain_paste: bool,
    pub plain_paste_hotkey: Hotkey,
    /// Opens the snippet picker; `None` = off.
    pub snippet_pick_hotkey: Option<Hotkey>,

    // Pause
    /// Last chosen pause length in minutes (15, 60, 180, 300).
    pub pause_minutes: u32,
    /// Paused until this Unix time in seconds; 0 = not paused.
    pub paused_until: f64,

    // Exceptions (app keys are lowercase executable names, e.g. "code.exe")
    pub ignored_words: Vec<String>,
    pub force_swap_words: Vec<String>,
    pub app_exception_modes: BTreeMap<String, AppMode>,
    /// Layout language forced when the app comes to the foreground: "en" or "ru".
    pub app_forced_layouts: BTreeMap<String, String>,
    /// Apps that already received their built-in default mode once.
    pub seeded_default_apps: Vec<String>,

    // Snippets
    /// Autoreplace pairs `[trigger, expansion]`, in order.
    pub snippets_ordered: Vec<(String, String)>,
    pub snippet_expand_space: bool,
    pub snippet_expand_enter: bool,
    pub snippet_expand_tab: bool,
    /// Picker entries `[name, text]`, in order.
    pub text_snippets: Vec<(String, String)>,

    // Learning state
    pub undo_declined_words: Vec<String>,
    pub undo_strike_count: BTreeMap<String, u32>,
    /// Unix seconds of the last strike per word.
    pub undo_strike_time: BTreeMap<String, f64>,
    pub switch_declined_words: Vec<String>,
    pub switch_strike_count: BTreeMap<String, u32>,
    /// Unix seconds of the last manual switch per word.
    pub switch_strike_time: BTreeMap<String, f64>,
    /// How many times each unknown word was typed.
    pub typo_personal_words: BTreeMap<String, u32>,

    // Layouts remembered per script (LayoutId strings)
    pub last_layout_cyr: String,
    pub last_layout_lat: String,

    // General
    pub language: UiLanguage,
    pub app_theme: Theme,

    // Updates
    pub update_check_on_startup: bool,
    /// Install what the startup check finds without asking.
    pub auto_update: bool,
    pub update_channel: UpdateChannel,
    /// Show the release notes on the first start after an update.
    pub show_whats_new: bool,

    // History
    /// History entries older than this many days are removed; 0 = keep them (up to the 200 newest).
    pub history_keep_days: u32,

    // App state
    pub did_initial_setup: bool,
    pub did_show_welcome: bool,
    /// Words fixed so far (shown in About).
    pub rescued_count: u64,
    /// App version of the last start; a different one means the app was updated.
    pub last_seen_version: String,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            auto_enabled: true,
            live_fix_enabled: true,
            auto_min_letters: 1,
            inline_live_fix: true,
            trigger_space: true,
            trigger_enter: true,
            trigger_tab: false,
            enter_pre_convert: true,
            arrows_cancel: true,
            developer_mode: false,
            dedupe_chatter: false,
            group_convert: true,
            learn_on_undo_enabled: true,
            learn_on_switch_enabled: true,

            typo_fix: true,
            two_caps_fix: true,

            sound: Sound::Beep,
            sound_volume: 0.6,
            sound_file: String::new(),

            hotkey: Hotkey::Key { key: KeyCode::Pause, mods: Modifiers::NONE },
            case_change_hotkey: None,
            plain_paste: false,
            plain_paste_hotkey: Hotkey::Key {
                key: KeyCode::KeyV,
                mods: Modifiers { ctrl: true, shift: true, ..Modifiers::NONE },
            },
            snippet_pick_hotkey: None,

            pause_minutes: 15,
            paused_until: 0.0,

            ignored_words: vec!["вк".into(), "тг".into(), "vk".into()],
            force_swap_words: Vec::new(),
            app_exception_modes: BTreeMap::new(),
            app_forced_layouts: BTreeMap::new(),
            seeded_default_apps: Vec::new(),

            snippets_ordered: Vec::new(),
            snippet_expand_space: false,
            snippet_expand_enter: false,
            snippet_expand_tab: true,
            text_snippets: Vec::new(),

            undo_declined_words: Vec::new(),
            undo_strike_count: BTreeMap::new(),
            undo_strike_time: BTreeMap::new(),
            switch_declined_words: Vec::new(),
            switch_strike_count: BTreeMap::new(),
            switch_strike_time: BTreeMap::new(),
            typo_personal_words: BTreeMap::new(),

            last_layout_cyr: String::new(),
            last_layout_lat: String::new(),

            language: UiLanguage::En,
            app_theme: Theme::System,

            update_check_on_startup: true,
            auto_update: false,
            update_channel: UpdateChannel::Stable,
            show_whats_new: true,

            history_keep_days: 0,

            did_initial_setup: false,
            did_show_welcome: false,
            rescued_count: 0,
            last_seen_version: String::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_round_trip() {
        let s = Settings::default();
        let json = serde_json::to_string(&s).unwrap();
        let back: Settings = serde_json::from_str(&json).unwrap();
        assert_eq!(s, back);
    }

    #[test]
    fn empty_object_loads_defaults() {
        let s: Settings = serde_json::from_str("{}").unwrap();
        assert_eq!(s, Settings::default());
    }

    #[test]
    fn uses_original_key_names_and_shapes() {
        let mut s = Settings::default();
        s.snippets_ordered.push(("br".into(), "Best regards".into()));
        s.app_exception_modes.insert("code.exe".into(), AppMode::Soft);
        let v = serde_json::to_value(&s).unwrap();
        assert_eq!(v["autoEnabled"], true);
        assert_eq!(v["triggerTab"], false);
        assert_eq!(v["sound"], "beep");
        assert_eq!(v["soundVolume"], 0.6);
        assert_eq!(v["snippetsOrdered"][0], serde_json::json!(["br", "Best regards"]));
        assert_eq!(v["appExceptionModes"]["code.exe"], "soft");
        assert_eq!(v["appTheme"], "system");
        assert_eq!(
            v["hotkey"],
            serde_json::json!({
                "mode": "key",
                "key": "Pause",
                "mods": { "ctrl": false, "alt": false, "shift": false, "meta": false }
            })
        );
        assert_eq!(v["caseChangeHotkey"], serde_json::Value::Null);
    }

    #[test]
    fn hotkey_modes_round_trip() {
        let all = [
            Hotkey::Combo { mods: Modifiers { alt: true, shift: true, ..Modifiers::NONE } },
            Hotkey::DoubleTap { key: KeyCode::ShiftLeft },
            Hotkey::ModKey { key: KeyCode::AltRight },
            Hotkey::Key { key: KeyCode::KeyT, mods: Modifiers { ctrl: true, ..Modifiers::NONE } },
        ];
        let modes = ["combo", "doubletap", "modkey", "key"];
        for (h, mode) in all.iter().zip(modes) {
            let v = serde_json::to_value(h).unwrap();
            assert_eq!(v["mode"], mode);
            assert_eq!(serde_json::from_value::<Hotkey>(v).unwrap(), *h);
        }
    }
}
