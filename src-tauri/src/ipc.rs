//! Commands and events shared with the frontend. Mirror: `src/lib/ipc.ts` (kept in sync by hand).
//! Payload types are the core's where they serialize as ipc.ts expects.

use keygloss_core::data;
use keygloss_core::engine::{EngineEvent, EngineStatus, HotkeyCheck, HotkeyTarget, check_hotkey as core_check_hotkey};
use keygloss_core::platform::{AppId, AppInfo};
use keygloss_core::settings::{Hotkey, Settings};
use serde::Serialize;
use serde_json::{Value, json};
use tauri::{AppHandle, State};
use tauri_plugin_autostart::ManagerExt;

use crate::state::{AppState, idle_status, merge, unix_now};
use crate::windows;

// ---- Event names ----

/// Payload: [`Settings`].
pub const EVENT_SETTINGS_CHANGED: &str = "settings-changed";
/// Payload: [`EngineStatus`].
pub const EVENT_ENGINE_STATUS: &str = "engine-status";
/// Payload: [`Hotkey`].
pub const EVENT_HOTKEY_CAPTURED: &str = "hotkey-captured";
/// Payload: `{ word }`.
pub const EVENT_LEARN_BANNER: &str = "learn-banner";
/// Payload: [`keygloss_core::engine::Toast`].
pub const EVENT_TOAST: &str = "toast";
/// Payload: `{ snippets: [name, text][] }`.
pub const EVENT_SNIPPET_PICKER_OPEN: &str = "snippet-picker-open";
/// Payload: null.
pub const EVENT_SNIPPET_PICKER_CLOSE: &str = "snippet-picker-close";

/// One "who wins" pair.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct AmbiguousPair {
    pub en: String,
    pub ru: String,
}

/// The UI event for an engine event: name and JSON payload (`None` for `PlaySound`).
/// `toast.pausedUntil` gets the `time` parameter (local "HH:mm") its string expects.
pub fn ui_event(ev: &EngineEvent, time: impl Fn(f64) -> String) -> Option<(&'static str, Value)> {
    Some(match ev {
        EngineEvent::SettingsChanged(s) => (EVENT_SETTINGS_CHANGED, v(&**s)),
        EngineEvent::StatusChanged(s) => (EVENT_ENGINE_STATUS, v(s)),
        EngineEvent::Toast(t) => {
            let mut t = t.clone();
            if let Some(until) = t.params.get("until").and_then(|u| u.parse::<f64>().ok()) {
                t.params.insert("time".into(), time(until));
            }
            (EVENT_TOAST, v(&t))
        }
        EngineEvent::LearnBanner { word } => (EVENT_LEARN_BANNER, json!({ "word": word })),
        EngineEvent::SnippetPickerOpen { snippets } => (EVENT_SNIPPET_PICKER_OPEN, json!({ "snippets": snippets })),
        EngineEvent::SnippetPickerClose => (EVENT_SNIPPET_PICKER_CLOSE, Value::Null),
        EngineEvent::HotkeyCaptured(h) => (EVENT_HOTKEY_CAPTURED, v(h)),
        EngineEvent::PlaySound { .. } => return None,
    })
}

fn v<T: Serialize>(x: &T) -> Value {
    serde_json::to_value(x).unwrap_or_default()
}

// ---- Commands ----

#[tauri::command]
pub fn get_settings(state: State<AppState>) -> Settings {
    state.settings()
}

/// Shallow-merges `patch` (a partial settings object) and returns the result.
#[tauri::command]
pub fn update_settings(app: AppHandle, state: State<AppState>, patch: Value) -> Result<Settings, String> {
    match &state.engine {
        Some(e) => e.update_settings(patch),
        None => {
            let new = merge(&state.settings(), patch)?;
            let s = state.edit_fallback(|s| *s = new);
            crate::publish_fallback(&app, &s);
            Ok(s)
        }
    }
}

#[tauri::command]
pub fn get_engine_status(state: State<AppState>) -> EngineStatus {
    state.status()
}

#[tauri::command]
pub fn pause(app: AppHandle, state: State<AppState>, minutes: u32) -> EngineStatus {
    match &state.engine {
        Some(e) => e.pause(minutes),
        None => {
            let s = state.edit_fallback(|s| {
                s.pause_minutes = minutes;
                s.paused_until = unix_now() + f64::from(minutes) * 60.0;
            });
            crate::publish_fallback(&app, &s);
            idle_status(&s)
        }
    }
}

#[tauri::command]
pub fn resume(app: AppHandle, state: State<AppState>) -> EngineStatus {
    match &state.engine {
        Some(e) => e.resume(),
        None => {
            let s = state.edit_fallback(|s| s.paused_until = 0.0);
            crate::publish_fallback(&app, &s);
            idle_status(&s)
        }
    }
}

#[tauri::command]
pub fn start_hotkey_recording(state: State<AppState>) {
    if let Some(e) = &state.engine {
        e.start_hotkey_recording();
    }
}

#[tauri::command]
pub fn stop_hotkey_recording(state: State<AppState>) {
    if let Some(e) = &state.engine {
        e.stop_hotkey_recording();
    }
}

#[tauri::command]
pub fn check_hotkey(state: State<AppState>, target: HotkeyTarget, hotkey: Hotkey) -> HotkeyCheck {
    match &state.engine {
        Some(e) => e.check_hotkey(target, &hotkey),
        None => core_check_hotkey(target, &hotkey, &state.settings()),
    }
}

/// Apps with a visible window, for the per-app exceptions picker (without Keygloss itself).
#[tauri::command]
pub async fn list_apps() -> Vec<AppInfo> {
    use keygloss_core::platform::ForegroundApp;
    let own = std::env::current_exe()
        .ok()
        .and_then(|p| p.file_name().map(|n| AppId::from_exe(&n.to_string_lossy())));
    let mut apps = keygloss_windows::WindowsForegroundApp.running_apps();
    apps.retain(|a| Some(&a.id) != own.as_ref());
    apps
}

#[tauri::command]
pub async fn get_ambiguous_pairs() -> Vec<AmbiguousPair> {
    data::warm_up()
        .extra
        .ambiguous_pairs
        .iter()
        .map(|p| AmbiguousPair { en: p.latin.clone(), ru: p.alt.clone() })
        .collect()
}

/// `keep` adds the word to learned words, otherwise it is declined.
#[tauri::command]
pub fn answer_learn_banner(state: State<AppState>, word: String, keep: bool) {
    if let Some(e) = &state.engine {
        e.answer_learn_banner(&word, keep);
    }
}

/// The learn banner timed out unanswered.
#[tauri::command]
pub fn dismiss_learn_banner(state: State<AppState>, word: String) {
    if let Some(e) = &state.engine {
        e.dismiss_learn_banner(&word);
    }
}

/// Inserts picker entry `index` (0-based) and closes the picker.
#[tauri::command]
pub fn snippet_picker_choose(app: AppHandle, state: State<AppState>, index: usize) {
    windows::release_picker_focus(&app);
    if let Some(e) = &state.engine {
        e.snippet_picker_choose(index);
    }
}

#[tauri::command]
pub fn snippet_picker_close(app: AppHandle, state: State<AppState>) {
    match &state.engine {
        Some(e) => e.snippet_picker_close(),
        None => windows::hide_picker(&app),
    }
}

#[tauri::command]
pub fn get_launch_at_login(app: AppHandle) -> Result<bool, String> {
    app.autolaunch().is_enabled().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_launch_at_login(app: AppHandle, enabled: bool) -> Result<bool, String> {
    let al = app.autolaunch();
    if enabled { al.enable() } else { al.disable() }.map_err(|e| e.to_string())?;
    al.is_enabled().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn app_version(app: AppHandle) -> String {
    app.package_info().version.to_string()
}

#[tauri::command]
pub fn open_settings(app: AppHandle) {
    windows::open_settings(&app);
}

#[tauri::command]
pub fn open_welcome(app: AppHandle) {
    windows::open_welcome(&app);
}

#[cfg(test)]
mod tests {
    use super::*;
    use keygloss_core::engine::{HotkeyVerdict, Toast};
    use keygloss_core::platform::{KeyCode, Modifiers};
    use std::collections::BTreeMap;

    fn keys(v: &Value) -> Vec<&str> {
        let mut k: Vec<&str> = v.as_object().unwrap().keys().map(String::as_str).collect();
        k.sort();
        k
    }

    #[test]
    fn engine_status_shape() {
        let st = EngineStatus { running: true, paused_until: None, auto_enabled: true, layout: Some("ru".into()), hook_healthy: true };
        let (name, v) = ui_event(&EngineEvent::StatusChanged(st), |_| unreachable!()).unwrap();
        assert_eq!(name, "engine-status");
        assert_eq!(v, json!({ "running": true, "pausedUntil": null, "autoEnabled": true, "layout": "ru", "hookHealthy": true }));
    }

    #[test]
    fn toast_shape_and_paused_until_time() {
        let t = Toast { key: "toast.pausedUntil".into(), params: BTreeMap::from([("until".into(), "1800000000".into())]) };
        let (name, v) = ui_event(&EngineEvent::Toast(t), |u| format!("{u}!")).unwrap();
        assert_eq!(name, "toast");
        assert_eq!(v, json!({ "key": "toast.pausedUntil", "params": { "until": "1800000000", "time": "1800000000!" } }));
        let t = Toast { key: "toast.resumed".into(), params: BTreeMap::new() };
        assert_eq!(ui_event(&EngineEvent::Toast(t), |_| unreachable!()).unwrap().1, json!({ "key": "toast.resumed", "params": {} }));
    }

    #[test]
    fn other_event_shapes() {
        let none = |_: f64| String::new();
        let ev = |e: EngineEvent| ui_event(&e, none).unwrap();
        assert_eq!(ev(EngineEvent::LearnBanner { word: "ok".into() }), ("learn-banner", json!({ "word": "ok" })));
        assert_eq!(
            ev(EngineEvent::SnippetPickerOpen { snippets: vec![("a".into(), "b".into())] }),
            ("snippet-picker-open", json!({ "snippets": [["a", "b"]] }))
        );
        assert_eq!(ev(EngineEvent::SnippetPickerClose), ("snippet-picker-close", Value::Null));
        assert_eq!(
            ev(EngineEvent::HotkeyCaptured(Hotkey::ModKey { key: KeyCode::AltRight })),
            ("hotkey-captured", json!({ "mode": "modkey", "key": "AltRight" }))
        );
        let (name, v) = ev(EngineEvent::SettingsChanged(Box::default()));
        assert_eq!(name, "settings-changed");
        assert_eq!(v, serde_json::to_value(Settings::default()).unwrap());
        assert!(ui_event(&EngineEvent::PlaySound { volume: 0.5 }, none).is_none());
    }

    #[test]
    fn command_type_shapes() {
        let c = HotkeyCheck { verdict: HotkeyVerdict::Blocked, reason: None, conflicts_with: Some(HotkeyTarget::CaseChange) };
        assert_eq!(serde_json::to_value(c).unwrap(), json!({ "verdict": "blocked", "reason": null, "conflictsWith": "caseChange" }));
        for (t, s) in [(HotkeyTarget::Manual, "manual"), (HotkeyTarget::PlainPaste, "plainPaste"), (HotkeyTarget::SnippetPick, "snippetPick")] {
            assert_eq!(serde_json::from_value::<HotkeyTarget>(json!(s)).unwrap(), t);
        }
        let app = AppInfo { id: AppId::from_exe("Notepad.exe"), name: "Notepad".into() };
        assert_eq!(serde_json::to_value(app).unwrap(), json!({ "id": "notepad.exe", "name": "Notepad" }));
        let p = AmbiguousPair { en: "yt".into(), ru: "не".into() };
        assert_eq!(serde_json::to_value(p).unwrap(), json!({ "en": "yt", "ru": "не" }));
        let hk: Hotkey = serde_json::from_value(json!({ "mode": "combo", "mods": { "ctrl": true, "alt": false, "shift": true, "meta": false } })).unwrap();
        assert_eq!(hk, Hotkey::Combo { mods: Modifiers { ctrl: true, shift: true, ..Modifiers::NONE } });
    }

    #[test]
    fn settings_keys_match_ipc_ts() {
        let v = serde_json::to_value(Settings::default()).unwrap();
        // The `Settings` interface in src/lib/ipc.ts, field for field.
        let mut ts = vec![
            "autoEnabled", "liveFixEnabled", "inlineLiveFix", "triggerSpace", "triggerEnter", "triggerTab",
            "enterPreConvert", "arrowsCancel", "developerMode", "dedupeChatter", "groupConvert",
            "learnOnUndoEnabled", "typoFix", "twoCapsFix", "soundEnabled", "soundVolume", "silentMode", "hotkey",
            "caseChangeHotkey", "plainPaste", "plainPasteHotkey", "snippetPickHotkey", "pauseMinutes",
            "pausedUntil", "ignoredWords", "forceSwapWords", "learnedWords", "appExceptionModes",
            "appForcedLayouts", "seededDefaultApps", "snippetsOrdered", "snippetExpandSpace", "snippetExpandEnter",
            "snippetExpandTab", "textSnippets", "undoDeclinedWords", "undoStrikeCount", "undoStrikeTime",
            "typoPersonalWords", "lastLayoutCyr", "lastLayoutLat", "language", "appTheme", "didInitialSetup",
            "didShowWelcome", "rescuedCount",
        ];
        ts.sort();
        assert_eq!(keys(&v), ts);
    }

    #[test]
    fn ambiguous_pairs_come_from_the_core_data() {
        let pairs = tauri::async_runtime::block_on(get_ambiguous_pairs());
        assert_eq!(pairs.len(), 48);
        assert_eq!(pairs[0], AmbiguousPair { en: "yt".into(), ru: "не".into() });
    }
}
