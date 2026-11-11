//! Updates from GitHub releases, installed by `tauri-plugin-updater` (signed NSIS installers).
//! These are the only network requests the app makes.

use keygloss_core::settings::UpdateChannel;
use serde::Deserialize;
use serde_json::json;
use tauri::{AppHandle, Emitter, Manager, Url};
use tauri_plugin_updater::{Error, UpdaterExt};

use crate::state::AppState;
use crate::{ipc, windows};

const REPO: &str = "xArchange1/keygloss";

#[derive(Deserialize)]
struct Release {
    tag_name: String,
}

/// The updater manifest attached to release `tag`, or to the latest stable release.
fn manifest(tag: Option<&str>) -> Url {
    let release = tag.map_or_else(|| "latest/download".into(), |t| format!("download/{t}"));
    format!("https://github.com/{REPO}/releases/{release}/latest.json").parse().expect("valid URL")
}

/// Tag of the newest release, pre-releases included ("latest" on GitHub skips them).
async fn newest_tag() -> Result<Option<String>, reqwest::Error> {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let releases: Vec<Release> = reqwest::Client::builder()
        .user_agent("keygloss-updater")
        .build()?
        .get(format!("https://api.github.com/repos/{REPO}/releases?per_page=1"))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    Ok(releases.into_iter().next().map(|r| r.tag_name))
}

/// Looks for a release newer than this build and remembers it. `Some(version)`: found one.
pub async fn check(app: &AppHandle) -> Result<Option<String>, String> {
    let state = app.state::<AppState>();
    let mut endpoints = Vec::new();
    if state.settings().update_channel == UpdateChannel::Beta
        && let Some(tag) = newest_tag().await.map_err(|e| e.to_string())?
    {
        endpoints.push(manifest(Some(&tag)));
    }
    endpoints.push(manifest(None));
    let handle = app.clone();
    let updater = app
        .updater_builder()
        .endpoints(endpoints)
        .and_then(|b| b.on_before_exit(move || crate::shutdown(&handle)).build())
        .map_err(|e| e.to_string())?;
    let update = match updater.check().await {
        Ok(update) => update,
        // No release carries a manifest yet.
        Err(Error::ReleaseNotFound) => None,
        Err(e) => return Err(e.to_string()),
    };
    let version = update.as_ref().map(|u| u.version.clone());
    state.set_pending_update(update);
    Ok(version)
}

/// Downloads the update found by the last check and runs its installer, which restarts the app.
pub async fn install(app: &AppHandle) -> Result<(), String> {
    let update = app.state::<AppState>().pending_update().ok_or("no update")?;
    update.download_and_install(|_, _| {}, || {}).await.map_err(|e| e.to_string())
}

/// The startup check: installs what it finds, or says so in a toast.
pub fn on_startup(app: &AppHandle) {
    let settings = app.state::<AppState>().settings();
    // Not in dev runs: an update would install over the build being worked on.
    if cfg!(debug_assertions) || !settings.update_check_on_startup {
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        match check(&app).await {
            Ok(Some(_)) if settings.auto_update => {
                if let Err(e) = install(&app).await {
                    crate::log::write(&format!("error update install failed: {e}"));
                }
            }
            Ok(Some(version)) => {
                let toast = json!({ "key": "toast.updateAvailable", "params": { "version": version } });
                let _ = app.emit(ipc::EVENT_TOAST, toast);
                crate::on_main(&app, windows::show_banner);
            }
            Ok(None) => {}
            Err(e) => crate::log::write(&format!("error update check failed: {e}")),
        }
    });
}
