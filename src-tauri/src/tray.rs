//! Tray icon and menu. The menu is a pure function of (status, settings language, OS language),
//! rebuilt whenever its model changes.

use keygloss_core::engine::EngineStatus;
use keygloss_core::settings::UiLanguage;
use tauri::image::Image;
use tauri::menu::{CheckMenuItem, IsMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::tray::{TrayIcon, TrayIconBuilder};
use tauri::{AppHandle, Manager, Wry};

use crate::state::AppState;

pub const ID_AUTO: &str = "auto";
pub const ID_RESUME: &str = "resume";
pub const ID_SETTINGS: &str = "settings";
pub const ID_QUIT: &str = "quit";
pub const PAUSE_PREFIX: &str = "pause:";
const PAUSE_MINUTES: [u32; 4] = [15, 60, 180, 300];
const TRAY_ID: &str = "main";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lang {
    En,
    Ru,
}

impl Lang {
    pub fn resolve(setting: UiLanguage) -> Lang {
        match setting {
            UiLanguage::Ru => Lang::Ru,
            UiLanguage::En => Lang::En,
            UiLanguage::Auto if keygloss_windows::shell::ui_language_is_russian() => Lang::Ru,
            UiLanguage::Auto => Lang::En,
        }
    }
}

/// Tray strings (wording from the original's `L10n.swift`).
fn tr(lang: Lang, key: &str) -> &'static str {
    let ru = lang == Lang::Ru;
    match key {
        "autoOff" => if ru { "авто выкл" } else { "auto off" },
        "pausedUntil" => if ru { "Не мешаю до {}" } else { "Not disturbing you until {}" },
        "resumeNow" => if ru { "Продолжить сейчас" } else { "Resume now" },
        "engineDown" => if ru { "Движок не запущен" } else { "Engine is not running" },
        "auto" => if ru { "Авто-переключение" } else { "Auto-switch" },
        "dnd" => if ru { "Не мешать" } else { "Do not disturb" },
        "15" => if ru { "15 минут" } else { "15 minutes" },
        "60" => if ru { "1 час" } else { "1 hour" },
        "180" => if ru { "3 часа" } else { "3 hours" },
        "300" => if ru { "5 часов" } else { "5 hours" },
        "settings" => if ru { "Настройки…" } else { "Settings…" },
        "quit" => if ru { "Выйти" } else { "Quit" },
        _ => "",
    }
}

/// Local "HH:mm" of a Unix time.
pub fn hh_mm(unix: f64) -> String {
    match keygloss_windows::shell::local_hour_minute(unix.round() as i64) {
        Some((h, m)) => format!("{h:02}:{m:02}"),
        None => String::new(),
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Entry {
    Disabled(String),
    Action(String, String),
    Check(String, String, bool),
    Submenu(String, Vec<(String, String)>),
    Separator,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Model {
    pub entries: Vec<Entry>,
    pub tooltip: String,
    pub dimmed: bool,
}

pub fn model(status: &EngineStatus, lang: Lang, version: &str, time: impl Fn(f64) -> String) -> Model {
    let t = |k: &str| tr(lang, k).to_string();
    let mut header = format!("Keygloss {version}");
    if !status.auto_enabled {
        header += &format!(" · {}", t("autoOff"));
    }
    let mut entries = vec![Entry::Disabled(header), Entry::Separator];
    let mut state = None;
    if let Some(until) = status.paused_until {
        let row = t("pausedUntil").replace("{}", &time(until));
        entries.push(Entry::Disabled(row.clone()));
        entries.push(Entry::Action(ID_RESUME.into(), t("resumeNow")));
        state = Some(row);
    }
    let down = !status.running || !status.hook_healthy;
    if down {
        entries.push(Entry::Disabled(t("engineDown")));
    }
    entries.push(Entry::Check(ID_AUTO.into(), t("auto"), status.auto_enabled));
    if status.auto_enabled && status.paused_until.is_none() {
        let lengths = PAUSE_MINUTES.iter().map(|m| (format!("{PAUSE_PREFIX}{m}"), t(&m.to_string()))).collect();
        entries.push(Entry::Submenu(t("dnd"), lengths));
    }
    entries.extend([
        Entry::Separator,
        Entry::Action(ID_SETTINGS.into(), t("settings")),
        Entry::Separator,
        Entry::Action(ID_QUIT.into(), t("quit")),
    ]);
    let state = state
        .or_else(|| down.then(|| t("engineDown")))
        .or_else(|| (!status.auto_enabled).then(|| t("autoOff")));
    Model {
        entries,
        tooltip: state.map_or_else(|| "Keygloss".to_string(), |s| format!("Keygloss · {s}")),
        dimmed: status.paused_until.is_some() || !status.auto_enabled,
    }
}

fn build_menu(app: &AppHandle, entries: &[Entry]) -> tauri::Result<Menu<Wry>> {
    let menu = Menu::new(app)?;
    for e in entries {
        let item: Box<dyn IsMenuItem<Wry>> = match e {
            Entry::Disabled(text) => Box::new(MenuItem::new(app, text, false, None::<&str>)?),
            Entry::Action(id, text) => Box::new(MenuItem::with_id(app, id, text, true, None::<&str>)?),
            Entry::Check(id, text, on) => Box::new(CheckMenuItem::with_id(app, id, text, true, *on, None::<&str>)?),
            Entry::Submenu(text, items) => {
                let sub = Submenu::new(app, text, true)?;
                for (id, text) in items {
                    sub.append(&MenuItem::with_id(app, id, text, true, None::<&str>)?)?;
                }
                Box::new(sub)
            }
            Entry::Separator => Box::new(PredefinedMenuItem::separator(app)?),
        };
        menu.append(item.as_ref())?;
    }
    Ok(menu)
}

/// The app icon at reduced opacity.
fn dimmed(icon: &Image<'_>) -> Image<'static> {
    let rgba: Vec<u8> = icon
        .rgba()
        .chunks_exact(4)
        .flat_map(|p| [p[0], p[1], p[2], (u16::from(p[3]) * 2 / 5) as u8])
        .collect();
    Image::new_owned(rgba, icon.width(), icon.height())
}

pub fn create(app: &AppHandle) -> tauri::Result<TrayIcon> {
    let mut builder = TrayIconBuilder::with_id(TRAY_ID)
        .tooltip("Keygloss")
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| crate::on_tray_menu(app, event.id().as_ref()));
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    let tray = builder.build(app)?;
    refresh(app);
    Ok(tray)
}

/// Rebuilds the tray menu, icon and tooltip if their model changed. Call on the main thread.
pub fn refresh(app: &AppHandle) {
    let state = app.state::<AppState>();
    let (Some(tray), Some(icon)) = (app.tray_by_id(TRAY_ID), app.default_window_icon()) else { return };
    let status = state.status();
    let lang = Lang::resolve(state.settings().language);
    let m = model(&status, lang, &app.package_info().version.to_string(), hh_mm);
    {
        let mut last = state.tray_model.lock().unwrap_or_else(|p| p.into_inner());
        if last.as_ref() == Some(&m) {
            return;
        }
        *last = Some(m.clone());
    }
    if let Ok(menu) = build_menu(app, &m.entries) {
        let _ = tray.set_menu(Some(menu));
    }
    let _ = tray.set_icon(Some(if m.dimmed { dimmed(icon) } else { icon.clone() }));
    let _ = tray.set_tooltip(Some(&m.tooltip));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn status(running: bool, paused: Option<f64>, auto: bool) -> EngineStatus {
        EngineStatus { running, paused_until: paused, auto_enabled: auto, layout: None, hook_healthy: running }
    }

    fn texts(m: &Model) -> Vec<String> {
        m.entries
            .iter()
            .map(|e| match e {
                Entry::Disabled(t) => format!("[{t}]"),
                Entry::Action(id, t) => format!("{id}:{t}"),
                Entry::Check(id, t, on) => format!("{id}:{t}:{on}"),
                Entry::Submenu(t, items) => format!("{t}>{}", items.iter().map(|i| i.0.as_str()).collect::<Vec<_>>().join(",")),
                Entry::Separator => "-".into(),
            })
            .collect()
    }

    #[test]
    fn running_and_active() {
        let m = model(&status(true, None, true), Lang::En, "0.1.0", |_| unreachable!());
        assert_eq!(
            texts(&m),
            [
                "[Keygloss 0.1.0]",
                "-",
                "auto:Auto-switch:true",
                "Do not disturb>pause:15,pause:60,pause:180,pause:300",
                "-",
                "settings:Settings…",
                "-",
                "quit:Quit",
            ]
        );
        assert_eq!(m.tooltip, "Keygloss");
        assert!(!m.dimmed);
    }

    #[test]
    fn paused_hides_the_pause_submenu() {
        let m = model(&status(true, Some(1.0), true), Lang::Ru, "0.1.0", |_| "14:30".into());
        assert_eq!(
            texts(&m)[..5],
            ["[Keygloss 0.1.0]", "-", "[Не мешаю до 14:30]", "resume:Продолжить сейчас", "auto:Авто-переключение:true"]
        );
        assert!(!texts(&m).iter().any(|t| t.contains("pause:")));
        assert_eq!(m.tooltip, "Keygloss · Не мешаю до 14:30");
        assert!(m.dimmed);
    }

    #[test]
    fn auto_off_and_engine_down() {
        let m = model(&status(false, None, false), Lang::En, "0.1.0", |_| unreachable!());
        assert_eq!(
            texts(&m)[..4],
            ["[Keygloss 0.1.0 · auto off]", "-", "[Engine is not running]", "auto:Auto-switch:false"]
        );
        assert!(!texts(&m).iter().any(|t| t.contains("pause:")));
        assert_eq!(m.tooltip, "Keygloss · Engine is not running");
        assert!(m.dimmed);
        let unhealthy = EngineStatus { hook_healthy: false, ..status(true, None, true) };
        assert!(texts(&model(&unhealthy, Lang::En, "1", |_| String::new())).contains(&"[Engine is not running]".into()));
    }

    #[test]
    fn dimmed_icon_keeps_color_and_scales_alpha() {
        let icon = Image::new_owned(vec![10, 20, 30, 255, 1, 2, 3, 0], 2, 1);
        assert_eq!(dimmed(&icon).rgba(), &[10, 20, 30, 102, 1, 2, 3, 0]);
    }
}
