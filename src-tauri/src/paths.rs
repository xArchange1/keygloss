//! Where the app keeps its files. Windows: `%APPDATA%\Keygloss` (settings, history, custom
//! sound) and `%LOCALAPPDATA%\Keygloss` (logs, WebView2 data), or a `keygloss` folder in each
//! XDG base directory whose variable is set (`XDG_CONFIG_HOME`: settings, `XDG_DATA_HOME`:
//! custom sound, `XDG_STATE_HOME`: history and logs, `XDG_CACHE_HOME`: WebView2 data).
//! macOS and Linux keep Tauri's dirs, named after the identifier.

use std::path::PathBuf;
use std::sync::OnceLock;

use tauri::{AppHandle, Manager};

pub struct Dirs {
    /// `settings.json`.
    pub config: PathBuf,
    /// The custom sound.
    pub data: PathBuf,
    /// `history.json`.
    pub state: PathBuf,
    pub logs: PathBuf,
    /// The WebView2 user data folder; `None` keeps Tauri's.
    pub cache: Option<PathBuf>,
}

static DIRS: OnceLock<Dirs> = OnceLock::new();

/// The dirs; [`init`] runs first in `setup`.
pub fn dirs() -> &'static Dirs {
    DIRS.get().expect("paths::init runs first")
}

#[cfg(not(windows))]
pub fn init(app: &AppHandle) -> tauri::Result<()> {
    let config = app.path().app_config_dir()?;
    let logs = app.path().app_log_dir()?;
    let _ = DIRS.set(Dirs { data: config.clone(), state: config.clone(), config, logs, cache: None });
    Ok(())
}

#[cfg(windows)]
pub fn init(app: &AppHandle) -> tauri::Result<()> {
    let xdg = |var| std::env::var_os(var).map(PathBuf::from).filter(|p| p.is_absolute()).map(|p| p.join("keygloss"));
    let roaming = app.path().config_dir()?.join("Keygloss");
    let local = app.path().local_data_dir()?.join("Keygloss");
    let state = xdg("XDG_STATE_HOME");
    let dirs = Dirs {
        config: xdg("XDG_CONFIG_HOME").unwrap_or_else(|| roaming.clone()),
        data: xdg("XDG_DATA_HOME").unwrap_or_else(|| roaming.clone()),
        logs: state.clone().unwrap_or_else(|| local.clone()).join("logs"),
        state: state.unwrap_or(roaming),
        cache: Some(xdg("XDG_CACHE_HOME").unwrap_or(local)),
    };
    migrate(app, &dirs);
    let _ = DIRS.set(dirs);
    Ok(())
}

/// Moves the files from the old dirs, named after the identifier, and deletes the old WebView2
/// data.
#[cfg(windows)]
fn migrate(app: &AppHandle, dirs: &Dirs) {
    use std::fs;
    use std::path::Path;

    fn move_file(from: &Path, to: &Path) {
        if !from.is_file() || to.exists() || to.parent().is_some_and(|p| fs::create_dir_all(p).is_err()) {
            return;
        }
        if fs::rename(from, to).is_err() && fs::copy(from, to).is_ok() {
            let _ = fs::remove_file(from);
        }
    }

    if let Ok(old) = app.path().app_config_dir() {
        move_file(&old.join(crate::persist::FILE), &dirs.config.join(crate::persist::FILE));
        move_file(&old.join(crate::history::FILE), &dirs.state.join(crate::history::FILE));
        move_file(&old.join(crate::sound::CUSTOM_FILE), &dirs.data.join(crate::sound::CUSTOM_FILE));
        let _ = fs::remove_dir(old);
    }
    if let Ok(old) = app.path().app_local_data_dir() {
        let old_logs = old.join("logs");
        for entry in fs::read_dir(&old_logs).into_iter().flatten().flatten() {
            move_file(&entry.path(), &dirs.logs.join(entry.file_name()));
        }
        let _ = fs::remove_dir(old_logs);
        let _ = fs::remove_dir_all(old.join("EBWebView"));
        let _ = fs::remove_dir(old);
    }
}
