//! `Clipboard` via the Win32 clipboard API.

use std::time::Duration;

use keygloss_core::platform::{Clipboard, ClipboardSnapshot, PlatformError};
use windows::Win32::Foundation::{GlobalFree, HANDLE, HGLOBAL};
use windows::Win32::System::DataExchange::*;
use windows::Win32::System::Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock};
use windows::core::w;

const CF_UNICODETEXT: u32 = 13;

/// Formats whose data is a GDI/other handle rather than global memory, so their bytes cannot be
/// copied: CF_BITMAP, CF_METAFILEPICT, CF_PALETTE, CF_ENHMETAFILE, CF_OWNERDISPLAY,
/// CF_DSPBITMAP, CF_DSPMETAFILEPICT, CF_DSPENHMETAFILE, plus the private (0x200-0x2FF) and
/// GDI-object (0x300-0x3FF) ranges. Images still survive a snapshot through CF_DIB/CF_DIBV5,
/// which Windows lists alongside CF_BITMAP and which are global memory.
fn is_handle_format(format: u32) -> bool {
    matches!(format, 2 | 3 | 9 | 14 | 0x80 | 0x82 | 0x83 | 0x8E | 0x200..=0x3FF)
}

/// Open clipboard; closed on drop.
struct Open;

impl Open {
    /// Retries for ~200 ms while another process holds the clipboard.
    fn new() -> Result<Open, PlatformError> {
        for _ in 0..20 {
            if unsafe { OpenClipboard(None) }.is_ok() {
                return Ok(Open);
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        Err(PlatformError("clipboard is locked by another process".into()))
    }
}

impl Drop for Open {
    fn drop(&mut self) {
        let _ = unsafe { CloseClipboard() };
    }
}

/// Copies the global-memory data of `format`, if present.
fn read_format(format: u32) -> Option<Vec<u8>> {
    unsafe {
        let handle = GetClipboardData(format).ok()?;
        let mem = HGLOBAL(handle.0);
        let size = GlobalSize(mem);
        let ptr = GlobalLock(mem) as *const u8;
        if ptr.is_null() {
            return None;
        }
        let bytes = std::slice::from_raw_parts(ptr, size).to_vec();
        let _ = GlobalUnlock(mem);
        Some(bytes)
    }
}

/// Puts `bytes` on the (open, emptied) clipboard as `format`.
fn write_format(format: u32, bytes: &[u8]) -> Result<(), PlatformError> {
    unsafe {
        let mem = GlobalAlloc(GMEM_MOVEABLE, bytes.len().max(1))
            .map_err(|e| PlatformError(format!("GlobalAlloc: {}", e.message())))?;
        let ptr = GlobalLock(mem) as *mut u8;
        if ptr.is_null() {
            let _ = GlobalFree(Some(mem));
            return Err(PlatformError("GlobalLock failed".into()));
        }
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), ptr, bytes.len());
        let _ = GlobalUnlock(mem);
        // On success the clipboard owns the memory.
        if let Err(e) = SetClipboardData(format, Some(HANDLE(mem.0))) {
            let _ = GlobalFree(Some(mem));
            return Err(PlatformError(format!("SetClipboardData: {}", e.message())));
        }
        Ok(())
    }
}

/// Marks the clipboard content as ours so clipboard-history tools and Win+V skip it.
fn write_exclusion_formats() {
    unsafe {
        let exclude = RegisterClipboardFormatW(w!("ExcludeClipboardContentFromMonitorProcessing"));
        let history = RegisterClipboardFormatW(w!("CanIncludeInClipboardHistory"));
        let _ = write_format(exclude, &[0]);
        let _ = write_format(history, &0u32.to_le_bytes());
    }
}

/// `Clipboard` implementation.
#[derive(Clone, Copy, Debug, Default)]
pub struct WindowsClipboard;

impl Clipboard for WindowsClipboard {
    fn get_text(&self) -> Option<String> {
        let _open = Open::new().ok()?;
        let bytes = read_format(CF_UNICODETEXT)?;
        let units: Vec<u16> = bytes.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
        let end = units.iter().position(|&u| u == 0).unwrap_or(units.len());
        Some(String::from_utf16_lossy(&units[..end]))
    }

    fn set_text(&self, text: &str) -> Result<(), PlatformError> {
        let bytes: Vec<u8> = text.encode_utf16().chain([0]).flat_map(u16::to_le_bytes).collect();
        let _open = Open::new()?;
        unsafe { EmptyClipboard() }.map_err(|e| PlatformError(format!("EmptyClipboard: {}", e.message())))?;
        write_format(CF_UNICODETEXT, &bytes)?;
        write_exclusion_formats();
        Ok(())
    }

    fn snapshot(&self) -> ClipboardSnapshot {
        let mut snapshot = ClipboardSnapshot::default();
        let Ok(_open) = Open::new() else { return snapshot };
        let mut format = 0;
        loop {
            format = unsafe { EnumClipboardFormats(format) };
            if format == 0 {
                break;
            }
            if !is_handle_format(format)
                && let Some(bytes) = read_format(format)
            {
                snapshot.formats.push((format, bytes));
            }
        }
        snapshot
    }

    /// Restores the snapshot, marked as excluded from clipboard history (it was recorded there
    /// when it was first copied).
    fn restore(&self, snapshot: &ClipboardSnapshot) -> Result<(), PlatformError> {
        let _open = Open::new()?;
        unsafe { EmptyClipboard() }.map_err(|e| PlatformError(format!("EmptyClipboard: {}", e.message())))?;
        for (format, bytes) in &snapshot.formats {
            // A format may refuse (e.g. one synthesized from another); keep restoring the rest.
            let _ = write_format(*format, bytes);
        }
        write_exclusion_formats();
        Ok(())
    }

    fn change_count(&self) -> u64 {
        u64::from(unsafe { GetClipboardSequenceNumber() })
    }
}
