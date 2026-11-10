//! Settings in one JSON file (`tauri-plugin-store`), one top-level entry per setting key.
//! Writes are coalesced by the store's auto-save debounce and flushed on quit.

use std::sync::Arc;
use std::time::Duration;

use keygloss_core::settings::Settings;
use serde_json::Value;
use tauri::{AppHandle, Manager, Wry};
use tauri_plugin_store::{Store, StoreExt};

pub const FILE: &str = "settings.json";
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
    serde_json::from_value(Value::Object(merged)).unwrap_or_default()
}

pub fn load(store: &Store<Wry>) -> Settings {
    settings_from_entries(store.entries())
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
    fn malformed_and_unknown_keys_are_skipped() {
        let s = settings_from_entries(entries(json!({
            "autoEnabled": "yes",
            "hotkey": { "mode": "nope" },
            "learnedWords": ["ок"],
            "someOldKey": 1,
        })));
        assert!(s.auto_enabled);
        assert_eq!(s.hotkey, Settings::default().hotkey);
        assert_eq!(s.learned_words, vec!["ок".to_string()]);
    }

    #[test]
    fn round_trips_through_entries() {
        let mut s = Settings::default();
        s.snippets_ordered.push(("br".into(), "Best regards".into()));
        s.paused_until = 1_800_000_000.0;
        assert_eq!(settings_from_entries(entries(serde_json::to_value(&s).unwrap())), s);
    }
}
