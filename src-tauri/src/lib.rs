mod history;
mod ipc;
mod log;
mod persist;
mod report;
mod sound;
mod state;
mod tray;
mod update;
mod windows;

use std::sync::atomic::Ordering::Relaxed;
use std::sync::{Arc, Mutex};

use keygloss_core::engine::{self, EngineEvent, EngineHandle, Platform};
use keygloss_core::settings::Settings;
use keygloss_windows::{
    WindowsClipboard, WindowsFocusedText, WindowsForegroundApp, WindowsHook, WindowsLayouts, WindowsOutput,
};
use tauri::{AppHandle, DeviceEventFilter, Emitter, Manager, WindowEvent, Wry};
use tauri_plugin_store::Store;

use state::{AppState, idle_status};

/// Runs `f` on the main thread without waiting (the engine's event thread must never block on it).
pub(crate) fn on_main(app: &AppHandle, f: impl FnOnce(&AppHandle) + Send + 'static) {
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || f(&handle));
}

fn emit_ui(app: &AppHandle, ev: &EngineEvent) {
    if let Some((name, payload)) = ipc::ui_event(ev, tray::hh_mm) {
        let _ = app.emit(name, payload);
    }
}

/// Routes an engine event (called on the engine's worker thread).
fn on_engine_event(app: &AppHandle, store: &Store<Wry>, history: &Store<Wry>, ev: EngineEvent) {
    emit_ui(app, &ev);
    match ev {
        EngineEvent::SettingsChanged(s) => {
            persist::save(store, &s);
            on_main(app, tray::refresh);
        }
        EngineEvent::StatusChanged(s) => {
            log::write(&format!(
                "status running={} hook={} auto={} paused={} layout={}",
                s.running,
                s.hook_healthy,
                s.auto_enabled,
                s.paused_until.is_some(),
                s.layout.as_deref().unwrap_or("?")
            ));
            on_main(app, tray::refresh);
        }
        EngineEvent::Toast(_) | EngineEvent::LearnBanner { .. } | EngineEvent::SwitchBanner { .. } => on_main(app, windows::show_banner),
        EngineEvent::SnippetPickerOpen { snippets } => {
            let rows = snippets.len();
            on_main(app, move |app| windows::show_picker(app, rows));
        }
        EngineEvent::SnippetPickerClose => on_main(app, windows::hide_picker),
        EngineEvent::HotkeyCaptured(_) => {}
        EngineEvent::PlaySound { sound, volume } => sound::play(app, sound, volume),
        EngineEvent::Log(line) => log::write(&line),
        EngineEvent::Converted { from, to } => {
            let keep_days = app.try_state::<AppState>().map_or(0, |s| s.settings().history_keep_days);
            let list = history::add(history, history::Entry { at: state::unix_now(), from, to }, keep_days);
            let _ = app.emit(ipc::EVENT_HISTORY_CHANGED, list);
        }
    }
}

/// Without an engine: persist and announce in-memory settings (and the status they imply).
pub(crate) fn publish_fallback(app: &AppHandle, s: &Settings) {
    let state = app.state::<AppState>();
    persist::save(&state.store, s);
    emit_ui(app, &EngineEvent::SettingsChanged(Box::new(s.clone())));
    emit_ui(app, &EngineEvent::StatusChanged(idle_status(s)));
    on_main(app, tray::refresh);
}

fn start_engine(
    app: &AppHandle,
    store: Arc<Store<Wry>>,
    history: Arc<Store<Wry>>,
    settings: Settings,
) -> Option<EngineHandle> {
    let platform = Platform {
        output: Arc::new(log::LoggedOutput(WindowsOutput)),
        layouts: Arc::new(log::LoggedLayouts(WindowsLayouts)),
        foreground: Arc::new(WindowsForegroundApp),
        focused: Arc::new(WindowsFocusedText),
        clipboard: Arc::new(WindowsClipboard),
    };
    let handle = app.clone();
    let sink = Arc::new(move |ev| on_engine_event(&handle, &store, &history, ev));
    match engine::start(platform, Box::new(WindowsHook::new()), settings, sink) {
        Ok(engine) => Some(engine),
        Err(e) => {
            log::write(&format!("error engine did not start: {e}"));
            None
        }
    }
}

pub(crate) fn on_tray_menu(app: &AppHandle, id: &str) {
    let state = app.state::<AppState>();
    match id {
        tray::ID_SETTINGS => windows::open_settings(app),
        tray::ID_QUIT => quit(app),
        tray::ID_AUTO => {
            let on = !state.status().auto_enabled;
            match &state.engine {
                Some(e) => {
                    e.set_auto(on);
                }
                None => publish_fallback(app, &state.edit_fallback(|s| s.auto_enabled = on)),
            }
        }
        tray::ID_RESUME => {
            ipc::resume(app.clone(), state);
        }
        _ => {
            if let Some(minutes) = id.strip_prefix(tray::PAUSE_PREFIX).and_then(|m| m.parse().ok()) {
                ipc::pause(app.clone(), state, minutes);
            }
        }
    }
}

/// Unhooks the keyboard and writes pending settings.
pub(crate) fn shutdown(app: &AppHandle) {
    let state = app.state::<AppState>();
    if let Some(e) = &state.engine {
        e.shutdown();
        persist::save(&state.store, &e.settings());
    }
    let _ = state.store.save();
    let _ = state.history.save();
}

fn quit(app: &AppHandle) {
    shutdown(app);
    log::write("quit");
    app.exit(0);
}

fn setup(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    if let Ok(dir) = app.path().app_log_dir() {
        log::init(dir);
    }
    log::write(&format!("start version={} os={}", app.package_info().version, keygloss_windows::shell::os_version()));
    let store = persist::open(app)?;
    let history = history::open(app)?;
    let mut settings = persist::load(&store);
    history::load(&history, settings.history_keep_days);

    let first_run = !settings.did_show_welcome;
    if first_run {
        settings.did_show_welcome = true;
        settings.did_initial_setup = true;
        persist::save(&store, &settings);
    }
    let version = app.package_info().version.to_string();
    let updated = settings.last_seen_version != version;
    let whats_new = updated && !first_run && settings.show_whats_new;
    if updated {
        settings.last_seen_version = version;
        persist::save(&store, &settings);
    }

    let engine = start_engine(app, store.clone(), history.clone(), settings.clone());
    app.manage(AppState {
        engine,
        fallback: Mutex::new(settings),
        store,
        history,
        picker_return_to: Mutex::new(0),
        tray_model: Mutex::new(None),
        banner_hwnd: 0.into(),
        picker_hwnd: 0.into(),
        update: Mutex::new(None),
        whats_new: whats_new.into(),
    });
    // After the state exists: building a window pumps messages, so the page's first
    // commands can run before `setup` returns and would fail without it.
    let state = app.state::<AppState>();
    state.banner_hwnd.store(windows::hwnd(&windows::create_banner(app)?), Relaxed);
    state.picker_hwnd.store(windows::hwnd(&windows::create_picker(app)?), Relaxed);
    windows::create_settings(app)?;
    tray::create(app)?;
    if first_run {
        windows::open_welcome(app);
    }
    if whats_new {
        windows::open_settings(app);
    }
    update::on_startup(app);
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // No raw input: while a process registers it, Windows skips that process's own
        // low-level keyboard hook whenever one of its windows has focus (the welcome sandbox).
        .device_event_filter(DeviceEventFilter::Always)
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| windows::open_settings(app)))
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_autostart::Builder::new().build())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| setup(app.handle()))
        .on_window_event(|window, event| {
            // The app lives in the tray: closing Settings only hides it.
            if let WindowEvent::CloseRequested { api, .. } = event
                && window.label() == windows::SETTINGS
            {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            ipc::get_settings,
            ipc::update_settings,
            ipc::get_engine_status,
            ipc::pause,
            ipc::resume,
            ipc::start_hotkey_recording,
            ipc::stop_hotkey_recording,
            ipc::check_hotkey,
            ipc::list_apps,
            ipc::get_ambiguous_pairs,
            ipc::answer_learn_banner,
            ipc::dismiss_learn_banner,
            ipc::answer_switch_banner,
            ipc::swap_layout,
            ipc::dismiss_switch_banner,
            ipc::snippet_picker_choose,
            ipc::snippet_picker_close,
            ipc::get_launch_at_login,
            ipc::set_launch_at_login,
            ipc::preview_sound,
            ipc::choose_sound_file,
            ipc::app_version,
            ipc::check_for_update,
            ipc::pending_update,
            ipc::install_update,
            ipc::open_update,
            ipc::take_whats_new,
            ipc::report_issue,
            ipc::open_logs,
            ipc::log_error,
            ipc::open_settings,
            ipc::open_welcome,
            ipc::get_history,
            ipc::clear_history,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Keygloss");
}
