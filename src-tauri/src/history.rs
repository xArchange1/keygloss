//! History of automatic fixes, newest first, in `history.json` next to the settings. It holds
//! typed words, so it never leaves this computer: not in the log, not in issue reports.

use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{AppHandle, Manager, Wry};
use tauri_plugin_store::{Store, StoreExt};

pub const FILE: &str = "history.json";
const KEY: &str = "entries";
/// Oldest entries past this are dropped.
const MAX: usize = 200;
const SAVE_DEBOUNCE: Duration = Duration::from_millis(500);

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Entry {
    /// Unix seconds.
    pub at: f64,
    pub from: String,
    pub to: String,
}

pub fn open(app: &AppHandle) -> Result<Arc<Store<Wry>>, String> {
    let dir = app.path().app_config_dir().map_err(|e| e.to_string())?;
    app.store_builder(dir.join(FILE)).auto_save(SAVE_DEBOUNCE).build().map_err(|e| e.to_string())
}

pub fn load(store: &Store<Wry>) -> Vec<Entry> {
    store.get(KEY).and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default()
}

/// Adds an entry on top and returns the whole list.
pub fn add(store: &Store<Wry>, entry: Entry) -> Vec<Entry> {
    let mut list = load(store);
    list.insert(0, entry);
    list.truncate(MAX);
    store.set(KEY, serde_json::to_value(&list).unwrap_or(Value::Null));
    list
}

pub fn clear(store: &Store<Wry>) {
    store.set(KEY, Value::Array(Vec::new()));
}
