mod ipc;

use std::sync::Mutex;

use keygloss_core::settings::Settings;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(w) = app.get_webview_window("settings") {
                let _ = w.show();
                let _ = w.set_focus();
            }
        }))
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_autostart::Builder::new().build())
        .manage(Mutex::new(Settings::default()))
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
            ipc::snippet_picker_choose,
            ipc::snippet_picker_close,
            ipc::get_launch_at_login,
            ipc::set_launch_at_login,
            ipc::app_version,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Keygloss");
}
