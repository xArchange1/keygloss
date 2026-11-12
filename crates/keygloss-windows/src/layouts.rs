//! `Layouts`: foreground layout, switching, enumeration and key tables.

use keygloss_core::platform::{KeyTable, LayoutId, LayoutInfo, Layouts, PlatformError};
use windows::Win32::Foundation::{LPARAM, WPARAM};
use windows::Win32::Globalization::{GetLocaleInfoEx, LCIDToLocaleName, LOCALE_SLOCALIZEDLANGUAGENAME};
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::WindowsAndMessaging::{
    GUITHREADINFO, GetForegroundWindow, GetGUIThreadInfo, GetWindowThreadProcessId, PostMessageW,
    WM_INPUTLANGCHANGEREQUEST,
};
use windows::core::PCWSTR;

use crate::keymap::{SCAN_KEYS, is_char_key};

/// Layout of the thread that owns the keyboard focus in the foreground window (null HKL if there
/// is no foreground window). The focus can sit in a child window of another thread or process,
/// such as a WebView2 page (Keygloss's own windows): that thread translates the keys and gets the
/// layout switches, while the top-level window's thread keeps its old layout.
pub fn foreground_hkl() -> HKL {
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.is_invalid() {
            return HKL::default();
        }
        let thread = GetWindowThreadProcessId(hwnd, None);
        let mut info = GUITHREADINFO { cbSize: size_of::<GUITHREADINFO>() as u32, ..Default::default() };
        let focus = if GetGUIThreadInfo(thread, &mut info).is_ok() && !info.hwndFocus.is_invalid() {
            GetWindowThreadProcessId(info.hwndFocus, None)
        } else {
            thread
        };
        GetKeyboardLayout(focus)
    }
}

/// HKLs are 32-bit values sign-extended on 64-bit Windows; the id is the low 32 bits in hex.
fn layout_id(hkl: HKL) -> LayoutId {
    LayoutId(format!("{:08X}", hkl.0 as usize as u32))
}

fn parse_hkl(id: &LayoutId) -> Option<HKL> {
    let v = u32::from_str_radix(&id.0, 16).ok()?;
    Some(HKL(v as i32 as isize as *mut _))
}

fn utf16_to_string(buf: &[u16]) -> String {
    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..end])
}

/// `Layouts` implementation.
#[derive(Clone, Copy, Debug, Default)]
pub struct WindowsLayouts;

impl Layouts for WindowsLayouts {
    fn current_layout(&self) -> Option<LayoutId> {
        let hkl = foreground_hkl();
        (!hkl.is_invalid()).then(|| layout_id(hkl))
    }

    /// Asks the foreground window to switch; `DefWindowProc` activates the layout for that
    /// window's thread. The window may refuse, and UIPI drops it for elevated windows.
    fn switch_layout(&self, id: &LayoutId) -> Result<(), PlatformError> {
        let hkl = parse_hkl(id).ok_or_else(|| PlatformError(format!("bad layout id {:?}", id.0)))?;
        unsafe {
            let hwnd = GetForegroundWindow();
            if hwnd.is_invalid() {
                return Err(PlatformError("no foreground window".into()));
            }
            PostMessageW(Some(hwnd), WM_INPUTLANGCHANGEREQUEST, WPARAM(0), LPARAM(hkl.0 as isize))
                .map_err(|e| PlatformError(format!("PostMessageW: {}", e.message())))
        }
    }

    fn installed_layouts(&self) -> Vec<LayoutInfo> {
        unsafe {
            let n = GetKeyboardLayoutList(None);
            let mut hkls = vec![HKL::default(); n.max(0) as usize];
            let n = GetKeyboardLayoutList(Some(&mut hkls));
            hkls.truncate(n.max(0) as usize);
            hkls.into_iter()
                .map(|hkl| {
                    let langid = hkl.0 as usize as u32 & 0xFFFF;
                    let mut locale = [0u16; 85];
                    LCIDToLocaleName(langid, Some(&mut locale), 0);
                    let mut name = [0u16; 128];
                    GetLocaleInfoEx(PCWSTR(locale.as_ptr()), LOCALE_SLOCALIZEDLANGUAGENAME, Some(&mut name));
                    LayoutInfo { id: layout_id(hkl), lang: utf16_to_string(&locale), name: utf16_to_string(&name) }
                })
                .collect()
        }
    }

    fn key_table(&self, id: &LayoutId) -> KeyTable {
        let mut table = KeyTable::new();
        let Some(hkl) = parse_hkl(id) else { return table };
        let mut state = [0u8; 256];
        let mut buf = [0u16; 8];
        for &(key, scan) in SCAN_KEYS.iter().filter(|e| is_char_key(e.0)) {
            let vk = unsafe { MapVirtualKeyExW(u32::from(scan), MAPVK_VSC_TO_VK_EX, Some(hkl)) };
            if vk == 0 {
                continue;
            }
            for shift in [false, true] {
                state[usize::from(VK_SHIFT.0)] = if shift { 0x80 } else { 0 };
                // Flag bit 2: do not change kernel keyboard state (no dead-key side effects).
                let n = unsafe { ToUnicodeEx(vk, u32::from(scan), &state, &mut buf, 1 << 2, Some(hkl)) };
                if n == 1
                    && let Some(c) = char::from_u32(u32::from(buf[0]))
                    && !c.is_control()
                {
                    table.insert((key, shift), c);
                }
            }
        }
        table
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_id_round_trip() {
        for raw in [0x0409_0409u32, 0x0419_0419, 0xF0C0_0409] {
            let hkl = HKL(raw as i32 as isize as *mut _);
            let id = layout_id(hkl);
            assert_eq!(id.0.len(), 8);
            assert_eq!(parse_hkl(&id), Some(hkl));
        }
        assert_eq!(layout_id(HKL(0x0419_0419 as *mut _)).0, "04190419");
    }
}
