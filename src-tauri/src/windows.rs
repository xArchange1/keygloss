//! The four windows. `banner` and `picker` never take focus: they are created non-focusable
//! (`WS_EX_NOACTIVATE`) and shown with `SetWindowPos(SWP_NOACTIVATE | SWP_SHOWWINDOW)`.
//! Everything here runs on the main thread.

use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

use crate::state::AppState;

pub const SETTINGS: &str = "settings";
pub const WELCOME: &str = "welcome";
pub const BANNER: &str = "banner";
pub const PICKER: &str = "picker";

/// Logical size of the banner window (BannerView: 12 px padding around a card of up to ~140 px).
const BANNER_SIZE: (f64, f64) = (380.0, 170.0);
/// Gap between the banner and the work area's corner.
const BANNER_MARGIN: f64 = 8.0;
const PICKER_WIDTH: f64 = 620.0;
/// PickerView: 28 px rows plus padding, list padding and the tip row.
const PICKER_ROW: f64 = 28.0;
const PICKER_CHROME: f64 = 60.0;
const PICKER_MAX_ROWS: usize = 12;

fn builder<'a>(app: &'a AppHandle, label: &'a str) -> WebviewWindowBuilder<'a, tauri::Wry, AppHandle> {
    WebviewWindowBuilder::new(app, label, WebviewUrl::default()).title("Keygloss")
}

/// Turns off WebView2's browser shortcuts (reload, print, find, back/forward, ...), its zoom
/// keys and its default context menu. Editing shortcuts (copy, paste, undo) keep working.
fn lock_down(w: WebviewWindow) -> tauri::Result<WebviewWindow> {
    w.with_webview(|wv| unsafe {
        use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2Settings3;
        use windows_core::Interface;
        let Ok(settings) = wv.controller().CoreWebView2().and_then(|c| c.Settings()) else { return };
        let _ = settings.SetAreDefaultContextMenusEnabled(false);
        let _ = settings.SetIsZoomControlEnabled(false);
        if let Ok(settings) = settings.cast::<ICoreWebView2Settings3>() {
            let _ = settings.SetAreBrowserAcceleratorKeysEnabled(false);
        }
    })?;
    Ok(w)
}

pub fn create_settings(app: &AppHandle) -> tauri::Result<WebviewWindow> {
    builder(app, SETTINGS).inner_size(680.0, 560.0).min_inner_size(680.0, 420.0).visible(false).center().build().and_then(lock_down)
}

/// A transparent, undecorated, topmost, non-activating, hidden window.
fn create_floating(app: &AppHandle, label: &str, (w, h): (f64, f64)) -> tauri::Result<WebviewWindow> {
    builder(app, label)
        .inner_size(w, h)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .focused(false)
        .focusable(false)
        .visible(false)
        .build()
        .and_then(lock_down)
}

pub fn create_banner(app: &AppHandle) -> tauri::Result<WebviewWindow> {
    create_floating(app, BANNER, BANNER_SIZE)
}

pub fn create_picker(app: &AppHandle) -> tauri::Result<WebviewWindow> {
    create_floating(app, PICKER, (PICKER_WIDTH, PICKER_CHROME + PICKER_ROW * 9.0))
}

pub fn hwnd(w: &WebviewWindow) -> isize {
    w.hwnd().map(|h| h.0 as isize).unwrap_or_default()
}

fn show_and_focus(w: &WebviewWindow) {
    let _ = w.unminimize();
    let _ = w.show();
    let _ = w.set_focus();
}

pub fn open_settings(app: &AppHandle) {
    if let Some(w) = app.get_webview_window(SETTINGS) {
        show_and_focus(&w);
    }
}

pub fn open_welcome(app: &AppHandle) {
    if let Some(w) = app.get_webview_window(WELCOME) {
        show_and_focus(&w);
        return;
    }
    let built = builder(app, WELCOME)
        .inner_size(820.0, 460.0)
        .min_inner_size(720.0, 380.0)
        .maximizable(false)
        .center()
        .build()
        .and_then(lock_down);
    if let Err(e) = built {
        crate::log::write(&format!("error welcome window: {e}"));
    }
}

/// Physical rectangle of a window of logical size `w`×`h` on the primary monitor's work area;
/// `place` gets (work x, y, width, height) and returns the logical top-left offset in it.
fn on_primary(app: &AppHandle, w: f64, h: f64, place: impl Fn(f64, f64) -> (f64, f64)) -> Option<(i32, i32, i32, i32)> {
    let m = app.primary_monitor().ok()??;
    let s = m.scale_factor();
    let wa = m.work_area();
    let (dx, dy) = place(f64::from(wa.size.width) / s, f64::from(wa.size.height) / s);
    Some((
        wa.position.x + (dx * s).round() as i32,
        wa.position.y + (dy * s).round() as i32,
        (w * s).round() as i32,
        (h * s).round() as i32,
    ))
}

/// Shows a floating window at `rect` without activating it, then lets Tauri record it as
/// visible (a plain show of an already visible window changes nothing), so the view's own
/// `hide()` works.
fn show_floating(app: &AppHandle, label: &str, hwnd: isize, rect: Option<(i32, i32, i32, i32)>) {
    let Some(w) = app.get_webview_window(label) else { return };
    let Some((x, y, width, height)) = rect else { return };
    keygloss_windows::shell::show_no_activate(hwnd, x, y, width, height);
    let _ = w.show();
}

/// Top-right corner of the primary monitor's work area.
pub fn show_banner(app: &AppHandle) {
    let (w, h) = BANNER_SIZE;
    let rect = on_primary(app, w, h, |aw, _| (aw - w - BANNER_MARGIN, BANNER_MARGIN));
    show_floating(app, BANNER, app.state::<AppState>().banner_hwnd, rect);
}

/// Sized to `rows` entries, centred horizontally, centred on two thirds of the work area's height.
pub fn show_picker(app: &AppHandle, rows: usize) {
    let state = app.state::<AppState>();
    let foreground = keygloss_windows::shell::foreground_window();
    if foreground != state.picker_hwnd {
        state.set_picker_return_to(foreground);
    }
    let (w, h) = (PICKER_WIDTH, PICKER_CHROME + PICKER_ROW * rows.clamp(1, PICKER_MAX_ROWS) as f64);
    let rect = on_primary(app, w, h, |aw, ah| ((aw - w) / 2.0, (ah * 2.0 / 3.0 - h / 2.0).min(ah - h).max(0.0)));
    show_floating(app, PICKER, state.picker_hwnd, rect);
}

pub fn hide_picker(app: &AppHandle) {
    if let Some(w) = app.get_webview_window(PICKER) {
        let _ = w.hide();
    }
}

/// Safety net for a click on a picker row: if the click activated the picker after all, hand
/// the focus back to the app the snippet is meant for before it is typed.
pub fn release_picker_focus(app: &AppHandle) {
    let state = app.state::<AppState>();
    let activated = keygloss_windows::shell::foreground_window() == state.picker_hwnd;
    hide_picker(app);
    let back = state.picker_return_to();
    if activated && back != 0 {
        keygloss_windows::shell::set_foreground_window(back);
    }
}
