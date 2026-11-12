//! App state: the engine (if it started) or, without it, the settings in memory.

use std::sync::atomic::{AtomicBool, AtomicIsize};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use keygloss_core::engine::{EngineHandle, EngineStatus};
use keygloss_core::settings::Settings;
use serde_json::Value;
use tauri::Wry;
use tauri_plugin_store::Store;
use tauri_plugin_updater::Update;

use crate::tray;

pub struct AppState {
    /// `None` if the engine failed to start: the app still runs (tray, settings).
    pub engine: Option<EngineHandle>,
    /// Settings while the engine is not running.
    pub fallback: Mutex<Settings>,
    pub store: Arc<Store<Wry>>,
    /// `history.json`, see [`crate::history`].
    pub history: Arc<Store<Wry>>,
    /// Foreground window when the snippet picker opened.
    pub picker_return_to: Mutex<isize>,
    /// Last tray model applied.
    pub tray_model: Mutex<Option<tray::Model>>,
    /// Physical HWNDs of the non-activating windows, set once they exist.
    pub banner_hwnd: AtomicIsize,
    pub picker_hwnd: AtomicIsize,
    /// Update found by the last check.
    pub update: Mutex<Option<Update>>,
    /// The app was updated: Settings shows the release notes once (`take_whats_new`).
    pub whats_new: AtomicBool,
}

pub fn unix_now() -> f64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0)
}

/// Status without an engine.
pub fn idle_status(s: &Settings) -> EngineStatus {
    EngineStatus {
        running: false,
        paused_until: (s.paused_until > unix_now()).then_some(s.paused_until),
        auto_enabled: s.auto_enabled,
        layout: None,
        hook_healthy: false,
    }
}

/// Shallow merge of a partial settings object (top-level keys), as the engine does it.
pub fn merge(settings: &Settings, patch: Value) -> Result<Settings, String> {
    let mut merged = serde_json::to_value(settings).map_err(|e| e.to_string())?;
    let (Value::Object(base), Value::Object(patch)) = (&mut merged, patch) else {
        return Err("patch must be an object".into());
    };
    base.extend(patch);
    serde_json::from_value(merged).map_err(|e| e.to_string())
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

impl AppState {
    pub fn settings(&self) -> Settings {
        match &self.engine {
            Some(e) => e.settings(),
            None => lock(&self.fallback).clone(),
        }
    }

    pub fn status(&self) -> EngineStatus {
        match &self.engine {
            Some(e) => e.status(),
            None => idle_status(&lock(&self.fallback)),
        }
    }

    /// Changes the in-memory settings (no engine) and returns the result.
    pub fn edit_fallback(&self, f: impl FnOnce(&mut Settings)) -> Settings {
        let mut s = lock(&self.fallback);
        f(&mut s);
        s.clone()
    }

    pub fn pending_update(&self) -> Option<Update> {
        lock(&self.update).clone()
    }

    pub fn set_pending_update(&self, update: Option<Update>) {
        *lock(&self.update) = update;
    }

    pub fn set_picker_return_to(&self, hwnd: isize) {
        *lock(&self.picker_return_to) = hwnd;
    }

    pub fn picker_return_to(&self) -> isize {
        *lock(&self.picker_return_to)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn merge_is_shallow_and_validated() {
        let s = merge(&Settings::default(), json!({ "autoEnabled": false, "pauseMinutes": 60 })).unwrap();
        assert!(!s.auto_enabled);
        assert_eq!(s.pause_minutes, 60);
        assert!(merge(&Settings::default(), json!({ "autoEnabled": 3 })).is_err());
        assert!(merge(&Settings::default(), json!([1])).is_err());
    }

    #[test]
    fn idle_status_reports_not_running() {
        let mut s = Settings::default();
        assert_eq!(idle_status(&s).paused_until, None);
        s.paused_until = unix_now() + 60.0;
        let st = idle_status(&s);
        assert!(!st.running && !st.hook_healthy);
        assert_eq!(st.paused_until, Some(s.paused_until));
    }
}
