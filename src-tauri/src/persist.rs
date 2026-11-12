//! Settings in one JSON file (`tauri-plugin-store`), one top-level entry per setting key.
//! Writes are coalesced by the store's auto-save debounce and flushed on quit.

use std::sync::Arc;
use std::time::Duration;

use keygloss_core::settings::{Settings, UiLanguage};
use serde_json::Value;
use tauri::{AppHandle, Manager, Wry};
use tauri_plugin_store::{Store, StoreExt};

pub const FILE: &str = "settings.json";
/// Old key: words learned from undo, now kept in `ignoredWords` ("Never switch").
const OLD_LEARNED: &str = "learnedWords";
/// Changes within this window are written to disk once.
const SAVE_DEBOUNCE: Duration = Duration::from_millis(500);

/// Opens the store in the app config dir. A missing or corrupt file yields an empty store.
pub fn open(app: &AppHandle) -> Result<Arc<Store<Wry>>, String> {
    let dir = app.path().app_config_dir().map_err(|e| e.to_string())?;
    app.store_builder(dir.join(FILE)).auto_save(SAVE_DEBOUNCE).build().map_err(|e| e.to_string())
}

/// Defaults overlaid with every stored key that is valid on its own; unknown and malformed
/// keys are ignored.
pub fn settings_from_entries(entries: impl IntoIterator<Item = (String, Value)>) -> Settings {
    let Ok(Value::Object(mut merged)) = serde_json::to_value(Settings::default()) else {
        return Settings::default();
    };
    let entries: Vec<_> = entries.into_iter().collect();
    // Files from before the `sound` choice: either of the two old switches had the sound off.
    let stored = |key: &str, value: bool| entries.iter().any(|(k, v)| k == key && *v == Value::Bool(value));
    if stored("soundEnabled", false) || stored("silentMode", true) {
        merged.insert("sound".into(), "off".into());
    }
    let old_learned = entries.iter().find(|(k, _)| k == OLD_LEARNED).map(|(_, v)| v.clone());
    for (key, value) in entries {
        if !merged.contains_key(&key) {
            continue;
        }
        let previous = merged.insert(key.clone(), value);
        if serde_json::from_value::<Settings>(Value::Object(merged.clone())).is_err()
            && let Some(previous) = previous
        {
            merged.insert(key, previous);
        }
    }
    let mut s: Settings = serde_json::from_value(Value::Object(merged)).unwrap_or_default();
    if let Some(Value::Array(old)) = old_learned {
        for w in old.iter().filter_map(Value::as_str) {
            if !s.ignored_words.iter().any(|x| x == w) {
                s.ignored_words.push(w.to_string());
            }
        }
    }
    s
}

pub fn load(store: &Store<Wry>) -> Settings {
    let mut s = settings_from_entries(store.entries());
    let mut changed = store.delete(OLD_LEARNED);
    // On install (or over the old "auto"), the interface takes the OS language if it is
    // translated, else English.
    if store.get("language").and_then(|v| serde_json::from_value::<UiLanguage>(v).ok()).is_none() {
        s.language = if keygloss_windows::shell::ui_language_is_russian() { UiLanguage::Ru } else { UiLanguage::En };
        changed = true;
    }
    if changed {
        save(store, &s);
    }
    s
}

/// Puts every key into the store; the store's debounce writes the file.
pub fn save(store: &Store<Wry>, settings: &Settings) {
    if let Ok(Value::Object(map)) = serde_json::to_value(settings) {
        for (k, v) in map {
            if store.get(&k).as_ref() != Some(&v) {
                store.set(k, v);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use keygloss_core::settings::Sound;
    use serde_json::json;

    fn entries(v: Value) -> Vec<(String, Value)> {
        v.as_object().unwrap().clone().into_iter().collect()
    }

    #[test]
    fn missing_keys_take_defaults() {
        let s = settings_from_entries(entries(json!({ "autoEnabled": false, "soundVolume": 0.25 })));
        assert!(!s.auto_enabled);
        assert_eq!(s.sound_volume, 0.25);
        assert_eq!(s.ignored_words, Settings::default().ignored_words);
        assert_eq!(settings_from_entries(Vec::new()), Settings::default());
    }

    #[test]
    fn sound_switched_off_in_an_old_file_stays_off() {
        let load = |v| settings_from_entries(entries(v)).sound;
        assert_eq!(load(json!({ "soundEnabled": true, "silentMode": false })), Sound::Beep);
        assert_eq!(load(json!({ "soundEnabled": false, "silentMode": false })), Sound::Off);
        assert_eq!(load(json!({ "soundEnabled": true, "silentMode": true })), Sound::Off);
        // A choice made since then wins over the stale keys.
        assert_eq!(load(json!({ "soundEnabled": false, "sound": "whoosh" })), Sound::Whoosh);
    }

    #[test]
    fn malformed_and_unknown_keys_are_skipped() {
        let s = settings_from_entries(entries(json!({
            "autoEnabled": "yes",
            "hotkey": { "mode": "nope" },
            "someOldKey": 1,
        })));
        assert!(s.auto_enabled);
        assert_eq!(s.hotkey, Settings::default().hotkey);
    }

    #[test]
    fn old_auto_language_is_not_a_language() {
        assert!(serde_json::from_value::<UiLanguage>(json!("auto")).is_err());
        assert_eq!(serde_json::from_value::<UiLanguage>(json!("ru")).unwrap(), UiLanguage::Ru);
    }

    #[test]
    fn old_learned_words_move_to_ignored() {
        let s = settings_from_entries(entries(json!({ "ignoredWords": ["vk"], "learnedWords": ["ок", "vk"] })));
        assert_eq!(s.ignored_words, vec!["vk".to_string(), "ок".to_string()]);
    }

    #[test]
    fn round_trips_through_entries() {
        let mut s = Settings::default();
        s.snippets_ordered.push(("br".into(), "Best regards".into()));
        s.paused_until = 1_800_000_000.0;
        assert_eq!(settings_from_entries(entries(serde_json::to_value(&s).unwrap())), s);
    }
}
