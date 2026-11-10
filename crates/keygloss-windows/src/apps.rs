//! `ForegroundApp`: foreground process, change notification, running apps.

use std::cell::RefCell;
use std::collections::HashSet;

use keygloss_core::platform::{AppId, AppInfo, ForegroundApp};
use windows::Win32::Foundation::{CloseHandle, HWND, LPARAM};
use windows::Win32::Storage::FileSystem::{GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW};
use windows::Win32::System::Threading::{
    OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
};
use windows::Win32::UI::Accessibility::{HWINEVENTHOOK, SetWinEventHook};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{BOOL, HSTRING, PWSTR, w};

fn window_pid(hwnd: HWND) -> u32 {
    let mut pid = 0;
    unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
    pid
}

fn process_path(pid: u32) -> Option<String> {
    unsafe {
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buf = [0u16; 1024];
        let mut len = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(process, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut len);
        let _ = CloseHandle(process);
        ok.ok()?;
        Some(String::from_utf16_lossy(&buf[..len as usize]))
    }
}

fn file_name(path: &str) -> &str {
    path.rsplit('\\').next().unwrap_or(path)
}

/// Executable path of the process owning `hwnd`. UWP apps are hosted in
/// `ApplicationFrameHost.exe`; their real process owns a child window, so that one is used.
fn window_exe_path(hwnd: HWND) -> Option<String> {
    let pid = window_pid(hwnd);
    let path = process_path(pid)?;
    if !file_name(&path).eq_ignore_ascii_case("applicationframehost.exe") {
        return Some(path);
    }
    unsafe extern "system" fn find_other(child: HWND, lparam: LPARAM) -> BOOL {
        // SAFETY: lparam is the `(u32, u32)` below.
        let (host, found) = unsafe { &mut *(lparam.0 as *mut (u32, u32)) };
        let pid = window_pid(child);
        if pid != *host {
            *found = pid;
            return false.into();
        }
        true.into()
    }
    let mut search = (pid, 0u32);
    unsafe { let _ = EnumChildWindows(Some(hwnd), Some(find_other), LPARAM(&mut search as *mut _ as isize)); }
    if search.1 != 0 { process_path(search.1) } else { Some(path) }
}

fn app_of_window(hwnd: HWND) -> Option<AppId> {
    if hwnd.is_invalid() {
        return None;
    }
    window_exe_path(hwnd).map(|p| AppId::from_exe(file_name(&p)))
}

/// `FileDescription` from the executable's version resource.
fn file_description(path: &str) -> Option<String> {
    unsafe {
        let path = HSTRING::from(path);
        let size = GetFileVersionInfoSizeW(&path, None);
        if size == 0 {
            return None;
        }
        let mut data = vec![0u8; size as usize];
        GetFileVersionInfoW(&path, None, size, data.as_mut_ptr().cast()).ok()?;
        let mut ptr = std::ptr::null_mut();
        let mut len = 0u32;
        if !VerQueryValueW(data.as_ptr().cast(), w!("\\VarFileInfo\\Translation"), &mut ptr, &mut len).as_bool()
            || len < 4
        {
            return None;
        }
        let lang = *(ptr as *const u16);
        let codepage = *(ptr as *const u16).add(1);
        let sub = HSTRING::from(format!("\\StringFileInfo\\{lang:04x}{codepage:04x}\\FileDescription"));
        if !VerQueryValueW(data.as_ptr().cast(), &sub, &mut ptr, &mut len).as_bool() || len == 0 {
            return None;
        }
        let text = String::from_utf16_lossy(std::slice::from_raw_parts(ptr as *const u16, len as usize));
        let text = text.trim_end_matches('\0').trim();
        (!text.is_empty()).then(|| text.to_string())
    }
}

/// Visible, unowned, non-tool top-level window with a title. Cloaked windows are kept: windows
/// on other virtual desktops are cloaked too.
fn is_app_window(hwnd: HWND) -> bool {
    unsafe {
        IsWindowVisible(hwnd).as_bool()
            && !GetWindow(hwnd, GW_OWNER).is_ok_and(|o| !o.is_invalid())
            && GetWindowLongW(hwnd, GWL_EXSTYLE) as u32 & WS_EX_TOOLWINDOW.0 == 0
            && GetWindowTextLengthW(hwnd) > 0
    }
}

type ForegroundCallback = Box<dyn Fn(AppId) + Send + Sync>;

thread_local! {
    /// Callback and last reported app of this WinEvent thread.
    static LISTENER: RefCell<Option<(ForegroundCallback, Option<AppId>)>> = const { RefCell::new(None) };
}

unsafe extern "system" fn foreground_event(
    _hook: HWINEVENTHOOK,
    _event: u32,
    hwnd: HWND,
    _id_object: i32,
    _id_child: i32,
    _thread: u32,
    _time: u32,
) {
    let Some(app) = app_of_window(hwnd) else { return };
    LISTENER.with(|l| {
        if let Some((callback, last)) = l.borrow_mut().as_mut()
            && last.as_ref() != Some(&app)
        {
            *last = Some(app.clone());
            callback(app);
        }
    });
}

/// `ForegroundApp` implementation.
#[derive(Clone, Copy, Debug, Default)]
pub struct WindowsForegroundApp;

impl ForegroundApp for WindowsForegroundApp {
    fn foreground_app(&self) -> Option<AppId> {
        app_of_window(unsafe { GetForegroundWindow() })
    }

    /// Spawns a thread with a `SetWinEventHook(EVENT_SYSTEM_FOREGROUND)` message loop. It runs
    /// for the life of the process; consecutive switches within one app are not reported.
    fn on_foreground_change(&self, callback: Box<dyn Fn(AppId) + Send + Sync>) {
        let initial = self.foreground_app();
        let _ = std::thread::Builder::new().name("keygloss-foreground".into()).spawn(move || unsafe {
            LISTENER.set(Some((callback, initial)));
            let hook = SetWinEventHook(
                EVENT_SYSTEM_FOREGROUND,
                EVENT_SYSTEM_FOREGROUND,
                None,
                Some(foreground_event),
                0,
                0,
                WINEVENT_OUTOFCONTEXT,
            );
            if hook.is_invalid() {
                return;
            }
            let mut msg = MSG::default();
            while GetMessageW(&mut msg, None, 0, 0).0 > 0 {
                DispatchMessageW(&msg);
            }
        });
    }

    fn running_apps(&self) -> Vec<AppInfo> {
        unsafe extern "system" fn collect(hwnd: HWND, lparam: LPARAM) -> BOOL {
            // SAFETY: lparam is the Vec below.
            let windows = unsafe { &mut *(lparam.0 as *mut Vec<HWND>) };
            windows.push(hwnd);
            true.into()
        }
        let mut windows: Vec<HWND> = Vec::new();
        unsafe { let _ = EnumWindows(Some(collect), LPARAM(&mut windows as *mut _ as isize)); }

        let mut seen = HashSet::new();
        let mut apps: Vec<AppInfo> = windows
            .into_iter()
            .filter(|&h| is_app_window(h))
            .filter_map(window_exe_path)
            .filter_map(|path| {
                let id = AppId::from_exe(file_name(&path));
                if !seen.insert(id.clone()) {
                    return None;
                }
                let name = file_description(&path).unwrap_or_else(|| file_name(&path).to_string());
                Some(AppInfo { id, name })
            })
            .collect();
        apps.sort_by_key(|a| a.name.to_lowercase());
        apps
    }
}
