//! The log file: one timestamped line per event, in the app log dir
//! (`%LOCALAPPDATA%\com.keygloss.app\logs\keygloss.log`). Lines never contain typed text, only
//! lengths.

use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use keygloss_core::platform::{KeyCode, KeyTable, LayoutId, LayoutInfo, Layouts, Modifiers, PlatformError, TextOutput};

const FILE: &str = "keygloss.log";
/// The previous file: kept when `FILE` grows past [`MAX_BYTES`].
const OLD_FILE: &str = "keygloss.old.log";
const MAX_BYTES: u64 = 1024 * 1024;

struct Log {
    dir: PathBuf,
    file: File,
    size: u64,
}

static LOG: OnceLock<Mutex<Log>> = OnceLock::new();

/// Opens the log for appending and returns its size. A file past the limit becomes the old file.
fn open(dir: &Path) -> std::io::Result<(File, u64)> {
    let path = dir.join(FILE);
    if fs::metadata(&path).is_ok_and(|m| m.len() > MAX_BYTES) {
        let _ = fs::rename(&path, dir.join(OLD_FILE));
    }
    let file = OpenOptions::new().create(true).append(true).open(&path)?;
    let size = file.metadata()?.len();
    Ok((file, size))
}

/// Starts logging into `dir`, and logs panics. Without a writable `dir` nothing is logged.
pub fn init(dir: PathBuf) {
    let _ = fs::create_dir_all(&dir);
    if let Ok((file, size)) = open(&dir) {
        let _ = LOG.set(Mutex::new(Log { dir, file, size }));
    }
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        write(&format!("panic: {info}"));
        default(info);
    }));
}

fn lock() -> Option<std::sync::MutexGuard<'static, Log>> {
    Some(LOG.get()?.lock().unwrap_or_else(|p| p.into_inner()))
}

pub fn write(line: &str) {
    let Some(mut log) = lock() else { return };
    if log.size > MAX_BYTES
        && let Ok((file, size)) = open(&log.dir)
    {
        (log.file, log.size) = (file, size);
    }
    let line = format!("{} {}\n", keygloss_windows::shell::local_timestamp(), line.replace('\n', " | "));
    if log.file.write_all(line.as_bytes()).is_ok() {
        log.size += line.len() as u64;
    }
}

pub fn dir() -> Option<PathBuf> {
    Some(lock()?.dir.clone())
}

/// The current log file's text ("" if there is none).
pub fn read() -> String {
    let Some(dir) = dir() else { return String::new() };
    String::from_utf8_lossy(&fs::read(dir.join(FILE)).unwrap_or_default()).into_owned()
}

/// A [`TextOutput`] that logs the calls that fail.
pub struct LoggedOutput<T>(pub T);

impl<T: TextOutput> TextOutput for LoggedOutput<T> {
    fn replace_text(&self, backspaces: usize, text: &str) -> Result<(), PlatformError> {
        self.0.replace_text(backspaces, text).inspect_err(|e| {
            write(&format!("error replace_text backspaces={backspaces} len={}: {e}", text.chars().count()));
        })
    }
    fn send_chord(&self, key: KeyCode, mods: Modifiers) -> Result<(), PlatformError> {
        self.0.send_chord(key, mods).inspect_err(|e| write(&format!("error send_chord {key:?}: {e}")))
    }
    fn send_mask_key(&self) -> Result<(), PlatformError> {
        self.0.send_mask_key().inspect_err(|e| write(&format!("error send_mask_key: {e}")))
    }
}

/// [`Layouts`] that logs the layout switches that fail.
pub struct LoggedLayouts<T>(pub T);

impl<T: Layouts> Layouts for LoggedLayouts<T> {
    fn current_layout(&self) -> Option<LayoutId> {
        self.0.current_layout()
    }
    fn switch_layout(&self, id: &LayoutId) -> Result<(), PlatformError> {
        self.0.switch_layout(id).inspect_err(|e| write(&format!("error switch_layout {}: {e}", id.0)))
    }
    fn installed_layouts(&self) -> Vec<LayoutInfo> {
        self.0.installed_layouts()
    }
    fn key_table(&self, id: &LayoutId) -> KeyTable {
        self.0.key_table(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_past_the_limit_becomes_the_old_file() {
        let dir = std::env::temp_dir().join(format!("keygloss-log-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let (mut file, size) = open(&dir).unwrap();
        assert_eq!(size, 0);
        file.write_all(&vec![b'x'; MAX_BYTES as usize + 1]).unwrap();
        // Rotated while the old handle is still open, as `write` does.
        let (_new, size) = open(&dir).unwrap();
        assert_eq!(size, 0);
        assert_eq!(fs::metadata(dir.join(OLD_FILE)).unwrap().len(), MAX_BYTES + 1);
        let _ = fs::remove_dir_all(&dir);
    }
}
