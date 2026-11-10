//! Small Win32 helpers for the app shell: sound, local time, UI language, window activation.

use windows::Win32::Foundation::{FILETIME, HWND, SYSTEMTIME};
use windows::Win32::Globalization::GetUserDefaultUILanguage;
use windows::Win32::Media::Audio::{PlaySoundW, SND_MEMORY, SND_NODEFAULT, SND_SYNC};
use windows::Win32::System::Time::{FileTimeToSystemTime, SystemTimeToTzSpecificLocalTime};
use windows::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, HWND_TOPMOST, SWP_NOACTIVATE, SWP_SHOWWINDOW, SetForegroundWindow, SetWindowPos,
};
use windows::core::PCWSTR;

/// Plays a WAV held in memory on a new thread and returns at once. The thread owns the buffer
/// for the whole (synchronous) playback; a newer sound cuts an older one off.
pub fn play_wav(wav: Vec<u8>) {
    let _ = std::thread::Builder::new().name("keygloss-sound".into()).spawn(move || unsafe {
        let _ = PlaySoundW(PCWSTR(wav.as_ptr().cast()), None, SND_MEMORY | SND_SYNC | SND_NODEFAULT);
    });
}

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
}
