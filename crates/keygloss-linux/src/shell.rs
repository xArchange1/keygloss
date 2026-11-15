//! Small Linux helpers for the app shell: local time, UI language, OS version, opening links.

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

/// The distribution and session, e.g. `Linux: Fedora Linux 41 (KDE Plasma), wayland, KDE`.
pub fn os_version() -> String {
    let release = std::fs::read_to_string("/etc/os-release").unwrap_or_default();
    let name = release
        .lines()
        .find_map(|l| l.strip_prefix("PRETTY_NAME="))
        .map_or("unknown".into(), |n| n.trim_matches('"').to_string());
    let env = |k: &str| std::env::var(k).unwrap_or_else(|_| "?".into());
    format!("Linux: {name}, {}, {}", env("XDG_SESSION_TYPE"), env("XDG_CURRENT_DESKTOP"))
}

/// Opens a URL in the default browser, or a folder in the file manager. False if `xdg-open` failed.
pub fn open(target: &str) -> bool {
    std::process::Command::new("xdg-open").arg(target).status().is_ok_and(|s| s.success())
}

/// The UI language (the first of `LANGUAGE`, `LC_ALL`, `LC_MESSAGES`, `LANG` that is set) is Russian.
pub fn ui_language_is_russian() -> bool {
    ["LANGUAGE", "LC_ALL", "LC_MESSAGES", "LANG"]
        .iter()
        .find_map(|k| std::env::var(k).ok().filter(|v| !v.is_empty()))
        .is_some_and(|v| v.starts_with("ru"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_time_and_version_have_their_shape() {
        let (h, m) = local_hour_minute(1_800_000_000).unwrap();
        assert!(h < 24 && m < 60);
        assert_eq!(local_timestamp().len(), 23);
        assert!(os_version().starts_with("Linux: "));
    }
}
