//! Updates from GitHub releases, installed by `tauri-plugin-updater` (signed NSIS installers).
//! Linux packages do not update themselves: a check reads the release from the GitHub API and
//! "install" opens its page. These are the only network requests the app makes.

use keygloss_core::settings::UpdateChannel;
use serde::Deserialize;
use serde_json::json;
#[cfg(not(target_os = "linux"))]
use tauri::Url;
use tauri::{AppHandle, Emitter, Manager};
#[cfg(not(target_os = "linux"))]
use tauri_plugin_updater::{Error, UpdaterExt};

use crate::state::AppState;
use crate::{ipc, windows};

const REPO: &str = "xArchange1/keygloss";

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    body: Option<String>,
}

/// A newer release (Linux), shaped like the updater plugin's `Update`.
#[cfg(target_os = "linux")]
#[derive(Clone, Debug)]
pub struct Update {
    pub version: String,
    pub body: Option<String>,
}

#[cfg(not(target_os = "linux"))]
/// The updater manifest attached to release `tag`, or to the latest stable release.
fn manifest(tag: Option<&str>) -> Url {
    let release = tag.map_or_else(|| "latest/download".into(), |t| format!("download/{t}"));
    format!("https://github.com/{REPO}/releases/{release}/latest.json").parse().expect("valid URL")
}

/// Newest release, pre-releases included ("latest" on GitHub skips them).
async fn newest_release() -> Result<Option<Release>, reqwest::Error> {
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
    Ok(releases.into_iter().next())
}

/// Newest stable release.
#[cfg(target_os = "linux")]
async fn latest_release() -> Result<Release, reqwest::Error> {
    let _ = rustls::crypto::ring::default_provider().install_default();
    reqwest::Client::builder()
        .user_agent("keygloss-updater")
        .build()?
        .get(format!("https://api.github.com/repos/{REPO}/releases/latest"))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await
}

/// Looks for a release newer than this build and remembers it. `Some(version)`: found one.
#[cfg(target_os = "linux")]
pub async fn check(app: &AppHandle) -> Result<Option<String>, String> {
    let state = app.state::<AppState>();
    let release = if state.settings().update_channel == UpdateChannel::Beta {
        newest_release().await
    } else {
        latest_release().await.map(Some)
    }
    .map_err(|e| e.to_string())?;
    let current = &app.package_info().version;
    let update = release.and_then(|r| {
        let version = r.tag_name.trim_start_matches('v').to_string();
        (semver::Version::parse(&version).ok()? > *current).then_some(Update { version, body: r.body })
    });
    let version = update.as_ref().map(|u| u.version.clone());
    state.set_pending_update(update);
    Ok(version)
}

/// Opens the page of the update found by the last check, with its packages.
#[cfg(target_os = "linux")]
pub async fn install(app: &AppHandle) -> Result<(), String> {
    let update = app.state::<AppState>().pending_update().ok_or("no update")?;
    let url = format!("https://github.com/{REPO}/releases/tag/v{}", update.version);
    crate::os::shell::open(&url).then_some(()).ok_or("could not open the browser".into())
}

/// Looks for a release newer than this build and remembers it. `Some(version)`: found one.
#[cfg(not(target_os = "linux"))]
pub async fn check(app: &AppHandle) -> Result<Option<String>, String> {
    let state = app.state::<AppState>();
    let mut endpoints = Vec::new();
    if state.settings().update_channel == UpdateChannel::Beta {
        // A failed lookup (e.g. the API's rate limit) still leaves the stable manifest.
        match newest_release().await {
            Ok(Some(release)) => endpoints.push(manifest(Some(&release.tag_name))),
            Ok(None) => {}
            Err(e) => crate::log::write(&format!("error beta release lookup failed: {e}")),
        }
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
#[cfg(not(target_os = "linux"))]
pub async fn install(app: &AppHandle) -> Result<(), String> {
    let update = app.state::<AppState>().pending_update().ok_or("no update")?;
    update.download_and_install(|_, _| {}, || {}).await.map_err(|e| e.to_string())
}

/// The startup check: installs what it finds, or offers it in the banner.
pub fn on_startup(app: &AppHandle) {
    let settings = app.state::<AppState>().settings();
    // Not in dev runs: an update would install over the build being worked on.
    if cfg!(debug_assertions) || !settings.update_check_on_startup {
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        match check(&app).await {
            // Linux packages do not update themselves: always the banner.
            Ok(Some(_)) if settings.auto_update && cfg!(not(target_os = "linux")) => {
                if let Err(e) = install(&app).await {
                    crate::log::write(&format!("error update install failed: {e}"));
                }
            }
            Ok(Some(version)) => {
                let _ = app.emit(ipc::EVENT_UPDATE_BANNER, json!({ "version": version }));
                crate::on_main(&app, windows::show_banner);
            }
            Ok(None) => {}
            Err(e) => crate::log::write(&format!("error update check failed: {e}")),
        }
    });
}
