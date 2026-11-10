//! Commands and events shared with the frontend. Mirror: `src/lib/ipc.ts` (kept in sync by hand).
//!
//! Every command is a placeholder that returns static or in-memory values so the frontend can
//! be built against it. The integration phase replaces the bodies, not the signatures.

// Event names and payloads are emitted only once the engine is wired in.
#![allow(dead_code)]

use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use keygloss_core::platform::{AppId, AppInfo};
use keygloss_core::settings::{Hotkey, Settings};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{AppHandle, Emitter, State};
use tauri_plugin_autostart::ManagerExt;

// ---- Event names ----

/// Payload: [`Settings`]. Sent after any settings change.
pub const EVENT_SETTINGS_CHANGED: &str = "settings-changed";
/// Payload: [`EngineStatus`]. Sent when any status field changes.
pub const EVENT_ENGINE_STATUS: &str = "engine-status";
/// Payload: [`Hotkey`]. Sent while recording, when a hotkey was pressed.
pub const EVENT_HOTKEY_CAPTURED: &str = "hotkey-captured";
/// Payload: [`LearnBanner`]. Ask "Keep <word> as is?".
pub const EVENT_LEARN_BANNER: &str = "learn-banner";
/// Payload: [`Toast`]. Short info toast.
pub const EVENT_TOAST: &str = "toast";
/// Payload: [`SnippetPickerOpen`]. The picker window should show these entries.
pub const EVENT_SNIPPET_PICKER_OPEN: &str = "snippet-picker-open";
/// Payload: none. The picker window should hide.
pub const EVENT_SNIPPET_PICKER_CLOSE: &str = "snippet-picker-close";

// ---- Payload and return types ----

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineStatus {
    /// The input hook is installed and the engine processes keys.
    pub running: bool,
    /// Unix seconds; `None` when not paused.
    pub paused_until: Option<f64>,
    /// Automatic correction on (`autoEnabled`).
    pub auto_enabled: bool,
    /// Language code of the foreground layout, e.g. "en", "ru"; `None` if unknown.
    pub layout: Option<String>,
    /// False if the OS dropped the hook.
    pub hook_healthy: bool,
}

/// Which feature a hotkey belongs to (for conflict checks).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HotkeyTarget {
    Manual,
    CaseChange,
    PlainPaste,
    SnippetPick,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HotkeyVerdict {
    /// Free to assign.
    Ok,
    /// Assignable, but shadows a known system or common shortcut.
    Warn,
    /// Refused.
    Blocked,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HotkeyCheck {
    pub verdict: HotkeyVerdict,
    /// i18n key explaining a warn/blocked verdict.
    pub reason: Option<String>,
    /// Our own feature that already uses this hotkey.
    pub conflicts_with: Option<HotkeyTarget>,
}

/// One "who wins" pair; stored as a force-swap entry when not "auto".
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AmbiguousPair {
    pub en: String,
    pub ru: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LearnBanner {
    pub word: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Toast {
    /// i18n key of the message.
    pub key: String,
    /// Interpolation values for the message.
    pub params: std::collections::BTreeMap<String, String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SnippetPickerOpen {
    /// `[name, text]` entries, in picker order.
    pub snippets: Vec<(String, String)>,
}

/// In-memory settings until the store is wired in.
pub type SettingsState = Mutex<Settings>;

fn status(s: &Settings) -> EngineStatus {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0);
    EngineStatus {
        running: false,
        paused_until: (s.paused_until > now).then_some(s.paused_until),
        auto_enabled: s.auto_enabled,
        layout: Some("en".into()),
        hook_healthy: false,
    }
}

// ---- Commands ----

#[tauri::command]
pub fn get_settings(state: State<SettingsState>) -> Settings {
    state.lock().unwrap().clone()
}

/// Shallow-merges `patch` (a partial settings object) and returns the result.
#[tauri::command]
pub fn update_settings(app: AppHandle, state: State<SettingsState>, patch: Value) -> Result<Settings, String> {
    let mut s = state.lock().unwrap();
    let mut merged = serde_json::to_value(&*s).map_err(|e| e.to_string())?;
    let (Value::Object(base), Value::Object(patch)) = (&mut merged, patch) else {
        return Err("patch must be an object".into());
    };
    base.extend(patch);
    *s = serde_json::from_value(merged).map_err(|e| e.to_string())?;
    let _ = app.emit(EVENT_SETTINGS_CHANGED, &*s);
    Ok(s.clone())
}

#[tauri::command]
pub fn get_engine_status(state: State<SettingsState>) -> EngineStatus {
    status(&state.lock().unwrap())
}

/// Pauses correction for `minutes`.
#[tauri::command]
pub fn pause(state: State<SettingsState>, minutes: u32) -> EngineStatus {
    let mut s = state.lock().unwrap();
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0);
    s.pause_minutes = minutes;
    s.paused_until = now + f64::from(minutes) * 60.0;
    status(&s)
}

#[tauri::command]
pub fn resume(state: State<SettingsState>) -> EngineStatus {
    let mut s = state.lock().unwrap();
    s.paused_until = 0.0;
    status(&s)
}

/// Starts routing key presses to `hotkey-captured` instead of the engine.
#[tauri::command]
pub fn start_hotkey_recording() {}

#[tauri::command]
pub fn stop_hotkey_recording() {}

#[tauri::command]
pub fn check_hotkey(target: HotkeyTarget, hotkey: Hotkey) -> HotkeyCheck {
    let _ = (target, hotkey);
    HotkeyCheck { verdict: HotkeyVerdict::Ok, reason: None, conflicts_with: None }
}

/// Running apps for the per-app exceptions picker.
#[tauri::command]
pub fn list_apps() -> Vec<AppInfo> {
    vec![
        AppInfo { id: AppId::from_exe("notepad.exe"), name: "Notepad".into() },
        AppInfo { id: AppId::from_exe("code.exe"), name: "Visual Studio Code".into() },
        AppInfo { id: AppId::from_exe("windowsterminal.exe"), name: "Windows Terminal".into() },
    ]
}

#[tauri::command]
pub fn get_ambiguous_pairs() -> Vec<AmbiguousPair> {
    [("yt", "не"), ("vs", "мы"), ("in", "шт"), ("ru", "кг"), ("key", "лун")]
        .into_iter()
        .map(|(en, ru)| AmbiguousPair { en: en.into(), ru: ru.into() })
        .collect()
}

/// Answer to the learn banner: `keep` adds the word to learned words, otherwise it is declined.
#[tauri::command]
pub fn answer_learn_banner(word: String, keep: bool) {
    let _ = (word, keep);
}

/// Inserts picker entry `index` (0-based) and closes the picker.
#[tauri::command]
pub fn snippet_picker_choose(index: usize) {
    let _ = index;
}

#[tauri::command]
pub fn snippet_picker_close() {}

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
