//! Small macOS helpers for the app shell: local time, UI language, OS version, opening links,
//! the Accessibility permission.

use objc2_foundation::{NSLocale, NSProcessInfo};

use crate::cf::*;

fn local_tm(unix: i64) -> Option<libc::tm> {
    let t: libc::time_t = unix;
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    (!unsafe { libc::localtime_r(&t, &mut tm) }.is_null()).then_some(tm)
}

/// Local wall-clock (hour, minute) of a Unix time in seconds.
pub fn local_hour_minute(unix: i64) -> Option<(u16, u16)> {
    local_tm(unix).map(|tm| (tm.tm_hour as u16, tm.tm_min as u16))
}

/// Local wall-clock time, e.g. `2026-11-11 14:03:22.123`.
pub fn local_timestamp() -> String {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
    let Some(tm) = local_tm(now.as_secs() as i64) else { return String::new() };
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}.{:03}",
        tm.tm_year + 1900,
        tm.tm_mon + 1,
        tm.tm_mday,
        tm.tm_hour,
        tm.tm_min,
        tm.tm_sec,
        now.subsec_millis()
    )
}

/// The macOS version, e.g. `macOS 15.1.0`.
pub fn os_version() -> String {
    let v = NSProcessInfo::processInfo().operatingSystemVersion();
    format!("macOS {}.{}.{}", v.majorVersion, v.minorVersion, v.patchVersion)
}

/// Opens a URL in the default browser, or a folder in Finder. False if `open` failed.
pub fn open(target: &str) -> bool {
    std::process::Command::new("/usr/bin/open").arg(target).status().is_ok_and(|s| s.success())
}

/// The first preferred language is Russian.
pub fn ui_language_is_russian() -> bool {
    NSLocale::preferredLanguages().firstObject().is_some_and(|l| l.to_string().starts_with("ru"))
}

/// Keygloss has the Accessibility permission (needed for the keyboard hook, typing and reading
/// the selection).
pub fn accessibility_granted() -> bool {
    unsafe { AXIsProcessTrusted() }
}

/// Shows the system prompt that sends the user to the Accessibility settings (once per launch
/// at most; macOS ignores repeats).
pub fn request_accessibility() {
    let options = unsafe { dict(&[(kAXTrustedCheckOptionPrompt, kCFBooleanTrue)]) };
    unsafe { AXIsProcessTrustedWithOptions(options.0) };
}

/// Opens System Settings at Privacy & Security > Accessibility.
pub fn open_accessibility_settings() -> bool {
    open("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_time_and_version_have_their_shape() {
        let (h, m) = local_hour_minute(1_800_000_000).unwrap();
        assert!(h < 24 && m < 60);
        assert_eq!(local_timestamp().len(), 23);
        assert!(os_version().starts_with("macOS "));
    }
}
