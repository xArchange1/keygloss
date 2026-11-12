//! History of automatic fixes, newest first, in `history.json` next to the settings. It holds
//! typed words, so it never leaves this computer: not in the log, not in issue reports.

use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{AppHandle, Manager, Wry};
use tauri_plugin_store::{Store, StoreExt};

use crate::state::unix_now;

pub const FILE: &str = "history.json";
const KEY: &str = "entries";
/// Oldest entries past this are dropped.
const MAX: usize = 200;
const SAVE_DEBOUNCE: Duration = Duration::from_millis(500);
const DAY: f64 = 86_400.0;

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

/// The list, without (and removing from the store) entries older than `keep_days`; 0 keeps all.
pub fn load(store: &Store<Wry>, keep_days: u32) -> Vec<Entry> {
    let mut list: Vec<Entry> = store.get(KEY).and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default();
    if expire(&mut list, keep_days, unix_now()) {
        save(store, &list);
    }
    list
}

/// Adds an entry on top and returns the whole list.
pub fn add(store: &Store<Wry>, entry: Entry, keep_days: u32) -> Vec<Entry> {
    let mut list = load(store, keep_days);
    list.insert(0, entry);
    list.truncate(MAX);
    save(store, &list);
    list
}

fn save(store: &Store<Wry>, list: &[Entry]) {
    store.set(KEY, serde_json::to_value(list).unwrap_or(Value::Null));
}

/// Drops entries older than `keep_days` before `now`; 0 keeps all. True if any were dropped.
fn expire(list: &mut Vec<Entry>, keep_days: u32, now: f64) -> bool {
    if keep_days == 0 {
        return false;
    }
    let cutoff = now - f64::from(keep_days) * DAY;
    let len = list.len();
    list.retain(|e| e.at >= cutoff);
    list.len() != len
}

pub fn clear(store: &Store<Wry>) {
    store.set(KEY, Value::Array(Vec::new()));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(at: f64) -> Entry {
        Entry { at, from: "ghbdtn".into(), to: "привет".into() }
    }

    #[test]
    fn expire_drops_entries_older_than_the_period() {
        let now = 100.0 * DAY;
        let mut list = vec![entry(now - 60.0), entry(now - 3.0 * DAY + 1.0), entry(now - 3.0 * DAY - 1.0), entry(0.0)];
        assert!(expire(&mut list, 3, now));
        assert_eq!(list, vec![entry(now - 60.0), entry(now - 3.0 * DAY + 1.0)]);
        assert!(!expire(&mut list, 3, now));
    }

    #[test]
    fn expire_off_keeps_everything() {
        let mut list = vec![entry(0.0)];
        assert!(!expire(&mut list, 0, 100.0 * DAY));
        assert_eq!(list.len(), 1);
    }
}
