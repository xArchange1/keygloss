//! Small Win32 helpers for the app shell: local time, UI language, OS version, opening links,
//! window activation.

use windows::Win32::Foundation::{FILETIME, HWND, SYSTEMTIME};
use windows::Win32::Globalization::GetUserDefaultUILanguage;
use windows::Win32::System::SystemInformation::{GetLocalTime, OSVERSIONINFOW};
use windows::Win32::System::Time::{FileTimeToSystemTime, SystemTimeToTzSpecificLocalTime};
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, HWND_TOPMOST, SW_SHOWNORMAL, SWP_NOACTIVATE, SWP_SHOWWINDOW, SetForegroundWindow,
    SetWindowPos,
};
use windows::core::{HSTRING, PCWSTR, w};

/// Local wall-clock (hour, minute) of a Unix time in seconds.
pub fn local_hour_minute(unix: i64) -> Option<(u16, u16)> {
    let ticks = (unix + 11_644_473_600).checked_mul(10_000_000)? as u64;
    let ft = FILETIME { dwLowDateTime: ticks as u32, dwHighDateTime: (ticks >> 32) as u32 };
    let (mut utc, mut local) = (SYSTEMTIME::default(), SYSTEMTIME::default());
    unsafe {
        FileTimeToSystemTime(&ft, &mut utc).ok()?;
        SystemTimeToTzSpecificLocalTime(None, &utc, &mut local).ok()?;
    }
    Some((local.wHour, local.wMinute))
}

/// Local wall-clock time, e.g. `2026-11-11 14:03:22.123`.
pub fn local_timestamp() -> String {
    let t = unsafe { GetLocalTime() };
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}.{:03}",
        t.wYear, t.wMonth, t.wDay, t.wHour, t.wMinute, t.wSecond, t.wMilliseconds
    )
}

#[link(name = "ntdll")]
unsafe extern "system" {
    /// Unlike `GetVersionExW`, reports the real version whatever the app manifest says.
    fn RtlGetVersion(info: *mut OSVERSIONINFOW) -> i32;
}

/// The Windows version, e.g. `Windows 11 (10.0.26100)`.
pub fn os_version() -> String {
    let mut v = OSVERSIONINFOW { dwOSVersionInfoSize: size_of::<OSVERSIONINFOW>() as u32, ..Default::default() };
    unsafe { RtlGetVersion(&mut v) };
    // Windows 11 kept the 10.0 version number: only the build tells them apart.
    let name = if v.dwMajorVersion == 10 && v.dwBuildNumber >= 22000 { 11 } else { v.dwMajorVersion };
    format!("Windows {name} ({}.{}.{})", v.dwMajorVersion, v.dwMinorVersion, v.dwBuildNumber)
}

/// Opens a URL in the default browser, or a folder in Explorer. False if Windows refused.
pub fn open(target: &str) -> bool {
    let result = unsafe {
        ShellExecuteW(None, w!("open"), &HSTRING::from(target), PCWSTR::null(), PCWSTR::null(), SW_SHOWNORMAL)
    };
    // "If the function succeeds, it returns a value greater than 32."
    result.0 as usize > 32
}

/// The Windows display language is Russian.
pub fn ui_language_is_russian() -> bool {
    const LANG_RUSSIAN: u16 = 0x19;
    unsafe { GetUserDefaultUILanguage() & 0x3FF == LANG_RUSSIAN }
}

fn hwnd(h: isize) -> HWND {
    HWND(h as *mut _)
}

/// Shows a window topmost at this physical rectangle without activating it.
pub fn show_no_activate(window: isize, x: i32, y: i32, width: i32, height: i32) {
    unsafe {
        let _ = SetWindowPos(hwnd(window), Some(HWND_TOPMOST), x, y, width, height, SWP_NOACTIVATE | SWP_SHOWWINDOW);
    }
}

pub fn foreground_window() -> isize {
    unsafe { GetForegroundWindow().0 as isize }
}

pub fn set_foreground_window(window: isize) {
    unsafe {
        let _ = SetForegroundWindow(hwnd(window));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_time_is_a_valid_clock_time() {
        let (h, m) = local_hour_minute(1_800_000_000).unwrap();
        assert!(h < 24 && m < 60);
        // Whole-hour time zones keep the minutes; most zones are whole hours.
        assert_eq!(local_hour_minute(1_800_000_000 + 3600).map(|t| t.1), Some(m));
    }

    #[test]
    fn timestamp_and_os_version_have_their_shape() {
        let ts = local_timestamp();
        assert_eq!(ts.len(), 23, "{ts}");
        let os = os_version();
        assert!(os.starts_with("Windows 1") && os.contains("(10.0."), "{os}");
    }
}
