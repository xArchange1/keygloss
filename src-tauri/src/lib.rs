mod ipc;
mod persist;
mod sound;
mod state;
mod tray;
mod windows;

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
fn on_main(app: &AppHandle, f: impl FnOnce(&AppHandle) + Send + 'static) {
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || f(&handle));
}

fn emit_ui(app: &AppHandle, ev: &EngineEvent) {
    if let Some((name, payload)) = ipc::ui_event(ev, tray::hh_mm) {
        let _ = app.emit(name, payload);
    }
}

/// Routes an engine event (called on the engine's worker thread).
fn on_engine_event(app: &AppHandle, store: &Store<Wry>, ev: EngineEvent) {
    emit_ui(app, &ev);
    match ev {
        EngineEvent::SettingsChanged(s) => {
            persist::save(store, &s);
            on_main(app, tray::refresh);
        }
        EngineEvent::StatusChanged(_) => on_main(app, tray::refresh),
        EngineEvent::Toast(_) | EngineEvent::LearnBanner { .. } => on_main(app, windows::show_banner),
        EngineEvent::SnippetPickerOpen { snippets } => {
            let rows = snippets.len();
            on_main(app, move |app| windows::show_picker(app, rows));
        }
        EngineEvent::SnippetPickerClose => on_main(app, windows::hide_picker),
        EngineEvent::HotkeyCaptured(_) => {}
        EngineEvent::PlaySound { volume } => sound::play_cue(volume),
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

fn start_engine(app: &AppHandle, store: Arc<Store<Wry>>, settings: Settings) -> Option<EngineHandle> {
    let platform = Platform {
        output: Arc::new(WindowsOutput),
        layouts: Arc::new(WindowsLayouts),
        foreground: Arc::new(WindowsForegroundApp),
        focused: Arc::new(WindowsFocusedText),
        clipboard: Arc::new(WindowsClipboard),
    };
    let handle = app.clone();
    let sink = Arc::new(move |ev| on_engine_event(&handle, &store, ev));
    match engine::start(platform, Box::new(WindowsHook::new()), settings, sink) {
        Ok(engine) => Some(engine),
        Err(e) => {
            eprintln!("engine did not start: {e}");
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

/// Unhooks the keyboard, writes pending settings and exits.
fn quit(app: &AppHandle) {
    let state = app.state::<AppState>();
    if let Some(e) = &state.engine {
        e.shutdown();
        persist::save(&state.store, &e.settings());
    }
    let _ = state.store.save();
    app.exit(0);
}

fn setup(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let store = persist::open(app)?;
    let mut settings = persist::load(&store);

    windows::create_settings(app)?;
    let banner = windows::hwnd(&windows::create_banner(app)?);
    let picker = windows::hwnd(&windows::create_picker(app)?);

    let first_run = !settings.did_show_welcome;
    if first_run {
        settings.did_show_welcome = true;
        settings.did_initial_setup = true;
        persist::save(&store, &settings);
    }

    let engine = start_engine(app, store.clone(), settings.clone());
    app.manage(AppState {
        engine,
        fallback: Mutex::new(settings),
        store,
        picker_return_to: Mutex::new(0),
        tray_model: Mutex::new(None),
        banner_hwnd: banner,
        picker_hwnd: picker,
    });
    tray::create(app)?;
    if first_run {
        windows::open_welcome(app);
    }
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
            ipc::snippet_picker_choose,
            ipc::snippet_picker_close,
            ipc::get_launch_at_login,
            ipc::set_launch_at_login,
            ipc::app_version,
            ipc::open_settings,
            ipc::open_welcome,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Keygloss");
}
