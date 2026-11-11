//! Links to the GitHub issue forms (`.github/ISSUE_TEMPLATE`) with the app version, the Windows
//! version and the newest log lines filled in. The browser opens them; the app sends nothing.

const NEW_ISSUE: &str = "https://github.com/xArchange1/keygloss/issues/new";
/// GitHub refuses longer URLs (414, at about 8 KB).
const MAX_URL: usize = 7000;

/// Percent-encodes everything but the unreserved characters.
fn encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// The new-issue URL: the bug form (with as many of the newest lines of `log` as fit) or the
/// feature form. The query keys are the `id`s of the form fields.
pub fn url(bug: bool, version: &str, os: &str, log: &str) -> String {
    let template = if bug { "bug_report.yml" } else { "feature_request.yml" };
    let mut url = format!("{NEW_ISSUE}?template={template}&version={}&os={}", encode(version), encode(os));
    if bug {
        url.push_str("&logs=");
        let mut left = MAX_URL.saturating_sub(url.len());
        let mut lines = Vec::new();
        for line in log.lines().rev() {
            let line = encode(line) + "%0A";
            if line.len() > left {
                break;
            }
            left -= line.len();
            lines.push(line);
        }
        lines.reverse();
        url.push_str(&lines.concat());
    }
    url
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bug_url_carries_version_os_and_the_newest_log_lines() {
        let log: String = (0..500).map(|i| format!("2026-11-11 10:00:00.000 fix kind=auto len={i}->{i}\n")).collect();
        let u = url(true, "0.1.0", "Windows 11 (10.0.26100)", &log);
        assert!(u.starts_with(
            "https://github.com/xArchange1/keygloss/issues/new?template=bug_report.yml&version=0.1.0&os=Windows%2011%20%2810.0.26100%29&logs="
        ));
        assert!(u.len() <= MAX_URL && u.len() > MAX_URL - 100, "{}", u.len());
        // Whole lines only, the newest last.
        assert!(u.ends_with("len%3D499-%3E499%0A"));
        assert!(u.contains("&logs=2026-11-11%2010%3A00%3A00.000%20fix"));
        assert!(!u.contains("len%3D0-"));
    }

    #[test]
    fn feature_url_has_no_log() {
        let u = url(false, "0.1.0", "Windows 10 (10.0.19045)", "secret line\n");
        assert_eq!(
            u,
            "https://github.com/xArchange1/keygloss/issues/new?template=feature_request.yml&version=0.1.0&os=Windows%2010%20%2810.0.19045%29"
        );
    }

    #[test]
    fn encode_escapes_non_ascii_bytes() {
        assert_eq!(encode("a b&c=я"), "a%20b%26c%3D%D1%8F");
    }
}
