//! `Clipboard` via `NSPasteboard`. Snapshot formats are `"<item index>\t<type>"`, so multi-item
//! content (several files) comes back as it was.

use keygloss_core::platform::{Clipboard, ClipboardSnapshot, PlatformError};
use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2_app_kit::{NSPasteboard, NSPasteboardItem, NSPasteboardTypeString, NSPasteboardWriting};
use objc2_foundation::{NSArray, NSData, NSString};

/// Marks our writes so clipboard managers skip them (nspasteboard.org convention).
const TRANSIENT: &str = "org.nspasteboard.TransientType";

fn write(items: Vec<Retained<NSPasteboardItem>>) -> Result<(), PlatformError> {
    if let Some(first) = items.first() {
        first.setData_forType(&NSData::new(), &NSString::from_str(TRANSIENT));
    }
    let pb = NSPasteboard::generalPasteboard();
    pb.clearContents();
    let objects: Vec<Retained<ProtocolObject<dyn NSPasteboardWriting>>> =
        items.into_iter().map(ProtocolObject::from_retained).collect();
    if pb.writeObjects(&NSArray::from_retained_slice(&objects)) {
        Ok(())
    } else {
        Err(PlatformError("NSPasteboard writeObjects failed".into()))
    }
}

/// `Clipboard` implementation.
#[derive(Clone, Copy, Debug, Default)]
pub struct MacClipboard;

impl Clipboard for MacClipboard {
    fn get_text(&self) -> Option<String> {
        NSPasteboard::generalPasteboard().stringForType(unsafe { NSPasteboardTypeString }).map(|s| s.to_string())
    }

    fn set_text(&self, text: &str) -> Result<(), PlatformError> {
        let item = NSPasteboardItem::new();
        item.setString_forType(&NSString::from_str(text), unsafe { NSPasteboardTypeString });
        write(vec![item])
    }

    fn snapshot(&self) -> ClipboardSnapshot {
        let mut snapshot = ClipboardSnapshot::default();
        let Some(items) = NSPasteboard::generalPasteboard().pasteboardItems() else { return snapshot };
        for (i, item) in items.iter().enumerate() {
            for ty in item.types() {
                if let Some(data) = item.dataForType(&ty) {
                    snapshot.formats.push((format!("{i}\t{ty}"), data.to_vec()));
                }
            }
        }
        snapshot
    }

    fn restore(&self, snapshot: &ClipboardSnapshot) -> Result<(), PlatformError> {
        let mut items: Vec<(usize, Retained<NSPasteboardItem>)> = Vec::new();
        for (format, bytes) in &snapshot.formats {
            let Some((index, ty)) = format.split_once('\t') else { continue };
            let Ok(index) = index.parse::<usize>() else { continue };
            if items.last().is_none_or(|(i, _)| *i != index) {
                items.push((index, NSPasteboardItem::new()));
            }
            let item = &items.last().expect("pushed above").1;
            item.setData_forType(&NSData::with_bytes(bytes), &NSString::from_str(ty));
        }
        write(items.into_iter().map(|(_, item)| item).collect())
    }

    fn change_count(&self) -> u64 {
        NSPasteboard::generalPasteboard().changeCount() as u64
    }
}
