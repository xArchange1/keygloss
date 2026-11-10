//! Built-in per-app policy (`Engine.defaultOffApps`, `defaultSoftApps`, `devApps`,
//! `chromiumFamily`, `builtinAppMode`; `ExceptionStore.seedDefaultApps`), translated to Windows
//! process names ([`AppId`]: lowercase executable file names). macOS-only apps are dropped.

use crate::platform::AppId;
use crate::settings::{AppMode, Settings};

/// Terminals.
const TERMINALS: &[&str] = &[
    "windowsterminal.exe", "openconsole.exe", "conhost.exe", "cmd.exe", "powershell.exe",
    "pwsh.exe", "mintty.exe", "alacritty.exe", "wezterm-gui.exe", "tabby.exe", "hyper.exe",
];

/// Video/3D editors where Space plays and Backspace deletes clips, plus system settings.
const PRO_MEDIA_AND_SYSTEM: &[&str] = &[
    "blender.exe", "resolve.exe", "adobe premiere pro.exe", "afterfx.exe", "systemsettings.exe",
];

/// Code editors (not JetBrains).
const EDITORS: &[&str] = &[
    "code.exe", "code - insiders.exe", "cursor.exe", "windsurf.exe", "zed.exe",
    "sublime_text.exe", "gvim.exe", "atom.exe",
];

/// JetBrains IDEs (the original matched the `com.jetbrains` bundle prefix).
const JETBRAINS: &[&str] = &[
    "idea64.exe", "pycharm64.exe", "webstorm64.exe", "rider64.exe", "clion64.exe",
    "goland64.exe", "rustrover64.exe", "phpstorm64.exe", "rubymine64.exe", "datagrip64.exe",
];

/// Chromium/Electron apps: Unicode injection there is unreliable mid-word, so live fix skips
/// them (boundary conversion still runs).
const CHROMIUM_FAMILY: &[&str] = &[
    "chrome.exe", "msedge.exe", "brave.exe", "vivaldi.exe", "opera.exe", "browser.exe",
    "code.exe", "code - insiders.exe", "cursor.exe", "windsurf.exe", "slack.exe",
    "notion.exe", "obsidian.exe", "figma.exe", "spotify.exe", "discord.exe", "signal.exe",
];

fn is(app: &AppId, list: &[&str]) -> bool {
    list.contains(&app.0.as_str())
}

/// The built-in mode for an app: terminals and pro media apps off, code editors soft.
pub fn builtin_app_mode(app: &AppId) -> Option<AppMode> {
    if is(app, TERMINALS) || is(app, PRO_MEDIA_AND_SYSTEM) {
        Some(AppMode::Off)
    } else if is(app, EDITORS) || is(app, JETBRAINS) {
        Some(AppMode::Soft)
    } else {
        None
    }
}

/// IDEs and terminals: automatic correction is fully off there in developer mode.
pub fn is_dev_app(app: &AppId) -> bool {
    is(app, TERMINALS) || is(app, EDITORS) || is(app, JETBRAINS)
}

/// Live fix (mid-word) is skipped in these apps.
pub fn is_chromium_family(app: &AppId) -> bool {
    is(app, CHROMIUM_FAMILY)
}

/// The app's mode from the user's list (`appExceptionModes`); built-in defaults reach it only
/// through [`seed_default_apps`], so removing an entry really turns correction back on.
pub fn app_mode(settings: &Settings, app: &AppId) -> Option<AppMode> {
    settings.app_exception_modes.get(&app.0).copied()
}

/// Gives each app its built-in mode once (first sight), without overriding a mode the user set
/// and without bringing back an app the user removed. Returns true if settings changed.
pub fn seed_default_apps(settings: &mut Settings, apps: &[AppId]) -> bool {
    let mut changed = false;
    for app in apps {
        let Some(mode) = builtin_app_mode(app) else { continue };
        if settings.seeded_default_apps.contains(&app.0) {
            continue;
        }
        settings.seeded_default_apps.push(app.0.clone());
        settings.app_exception_modes.entry(app.0.clone()).or_insert(mode);
        changed = true;
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(s: &str) -> AppId {
        AppId::from_exe(s)
    }

    #[test]
    fn builtin_modes() {
        assert_eq!(builtin_app_mode(&id("WindowsTerminal.exe")), Some(AppMode::Off));
        assert_eq!(builtin_app_mode(&id("pwsh.exe")), Some(AppMode::Off));
        assert_eq!(builtin_app_mode(&id("Resolve.exe")), Some(AppMode::Off));
        assert_eq!(builtin_app_mode(&id("Code.exe")), Some(AppMode::Soft));
        assert_eq!(builtin_app_mode(&id("rustrover64.exe")), Some(AppMode::Soft));
        assert_eq!(builtin_app_mode(&id("notepad.exe")), None);
        assert!(is_dev_app(&id("cmd.exe")) && is_dev_app(&id("idea64.exe")));
        assert!(!is_dev_app(&id("chrome.exe")));
        assert!(is_chromium_family(&id("msedge.exe")) && is_chromium_family(&id("discord.exe")));
        assert!(!is_chromium_family(&id("telegram.exe")));
    }

    #[test]
    fn seeding_runs_once_and_respects_user() {
        let mut s = Settings::default();
        s.app_exception_modes.insert("code.exe".into(), AppMode::Off);
        assert!(seed_default_apps(&mut s, &[id("code.exe"), id("cmd.exe"), id("notepad.exe")]));
        assert_eq!(app_mode(&s, &id("code.exe")), Some(AppMode::Off));
        assert_eq!(app_mode(&s, &id("cmd.exe")), Some(AppMode::Off));
        assert_eq!(app_mode(&s, &id("notepad.exe")), None);
        s.app_exception_modes.remove("cmd.exe");
        assert!(!seed_default_apps(&mut s, &[id("cmd.exe")]));
        assert_eq!(app_mode(&s, &id("cmd.exe")), None);
    }
}
