//! `Clipboard` via the Wayland data-control protocol (`wl-clipboard-rs`), which reads and writes
//! the clipboard without a focused window. Snapshot formats are MIME types. Our writes carry
//! KDE's password-manager hint, so Klipper keeps them out of its history.
//!
//! Wayland has no change counter: [`LinuxClipboard::change_count`] is a hash of the offered MIME
//! types and the text, so writing the same text again does not count as a change.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::io::Read;

use keygloss_core::platform::{Clipboard, ClipboardSnapshot, PlatformError};
use wl_clipboard_rs::{copy, paste};

fn read(mime: paste::MimeType<'_>) -> Option<Vec<u8>> {
    let (mut pipe, _) = paste::get_contents(paste::ClipboardType::Regular, paste::Seat::Unspecified, mime).ok()?;
    let mut bytes = Vec::new();
    pipe.read_to_end(&mut bytes).ok()?;
    Some(bytes)
}

fn mime_types() -> Vec<String> {
    paste::get_mime_types_ordered(paste::ClipboardType::Regular, paste::Seat::Unspecified).unwrap_or_default()
}

fn err(e: copy::Error) -> PlatformError {
    PlatformError(format!("clipboard: {e}"))
}

/// `Clipboard` implementation.
#[derive(Clone, Copy, Debug, Default)]
pub struct LinuxClipboard;

impl Clipboard for LinuxClipboard {
    fn get_text(&self) -> Option<String> {
        read(paste::MimeType::Text).and_then(|b| String::from_utf8(b).ok())
    }

    fn set_text(&self, text: &str) -> Result<(), PlatformError> {
        let mut options = copy::Options::new();
        options.sensitive(true);
        options.copy(copy::Source::Bytes(text.as_bytes().into()), copy::MimeType::Text).map_err(err)
    }

    fn snapshot(&self) -> ClipboardSnapshot {
        let formats = mime_types().into_iter().filter_map(|m| read(paste::MimeType::Specific(&m)).map(|b| (m, b))).collect();
        ClipboardSnapshot { formats }
    }

    fn restore(&self, snapshot: &ClipboardSnapshot) -> Result<(), PlatformError> {
        if snapshot.formats.is_empty() {
            return copy::clear(copy::ClipboardType::Regular, copy::Seat::All).map_err(err);
        }
        let sources = snapshot
            .formats
            .iter()
            .map(|(mime, bytes)| copy::MimeSource {
                source: copy::Source::Bytes(bytes.clone().into()),
                mime_type: copy::MimeType::Specific(mime.clone()),
            })
            .collect();
        let mut options = copy::Options::new();
        options.omit_additional_text_mime_types(true);
        options.copy_multi(sources).map_err(err)
    }

    fn change_count(&self) -> u64 {
        let mut h = DefaultHasher::new();
        mime_types().hash(&mut h);
        self.get_text().hash(&mut h);
        h.finish()
    }
}
