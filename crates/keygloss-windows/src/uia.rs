//! `FocusedText` via UI Automation.

use keygloss_core::platform::{FocusedText, SelectionRead};
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoUninitialize,
};
use windows::Win32::UI::Accessibility::{
    CUIAutomation8, IUIAutomation, IUIAutomation2, IUIAutomationTextPattern, UIA_TextPatternId,
};

/// UIA connection/transaction timeout, so a hung target app cannot block the caller for long.
const TIMEOUT_MS: u32 = 500;

/// Runs `f` with a UIA client on the calling thread. Joins the MTA if the thread has no COM
/// apartment yet (and leaves it afterwards); an existing STA or MTA is used as is.
fn with_uia<T>(f: impl FnOnce(&IUIAutomation) -> Option<T>) -> Option<T> {
    unsafe {
        let joined = CoInitializeEx(None, COINIT_MULTITHREADED).is_ok();
        let result = (|| {
            let uia: IUIAutomation2 = CoCreateInstance(&CUIAutomation8, None, CLSCTX_INPROC_SERVER).ok()?;
            let _ = uia.SetConnectionTimeout(TIMEOUT_MS);
            let _ = uia.SetTransactionTimeout(TIMEOUT_MS);
            f(&uia)
        })();
        if joined {
            CoUninitialize();
        }
        result
    }
}

/// `FocusedText` implementation. Call it off the hook thread: UIA calls can take hundreds of
/// milliseconds, which would get the low-level hook removed.
#[derive(Clone, Copy, Debug, Default)]
pub struct WindowsFocusedText;

impl FocusedText for WindowsFocusedText {
    fn selected_text(&self) -> SelectionRead {
        with_uia(|uia| unsafe {
            let element = uia.GetFocusedElement().ok()?;
            if element.CurrentIsPassword().ok()?.as_bool() {
                return Some(SelectionRead::Password);
            }
            let pattern: IUIAutomationTextPattern = element.GetCurrentPatternAs(UIA_TextPatternId).ok()?;
            let ranges = pattern.GetSelection().ok()?;
            let mut text = String::new();
            for i in 0..ranges.Length().ok()? {
                text.push_str(&ranges.GetElement(i).ok()?.GetText(-1).ok()?.to_string());
            }
            Some(if text.is_empty() { SelectionRead::Empty } else { SelectionRead::Text(text) })
        })
        .unwrap_or(SelectionRead::Unavailable)
    }

    fn focused_is_password(&self) -> bool {
        with_uia(|uia| unsafe { uia.GetFocusedElement().ok()?.CurrentIsPassword().ok() })
            .is_some_and(|b| b.as_bool())
    }
}
