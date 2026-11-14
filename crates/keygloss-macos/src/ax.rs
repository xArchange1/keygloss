//! `FocusedText` via the Accessibility API.

use std::ptr;

use keygloss_core::platform::{FocusedText, SelectionRead};

use crate::cf::*;

/// Messaging timeout, so a hung target app cannot block the caller for long.
const TIMEOUT_S: f32 = 0.5;
const AX_SUCCESS: i32 = 0;

/// An attribute of an AX element (+1), if it has one.
fn attribute(element: CFTypeRef, name: &str) -> Option<Owned> {
    let name = cfstr(name);
    let mut value: CFTypeRef = ptr::null();
    let err = unsafe { AXUIElementCopyAttributeValue(element, name.0, &mut value) };
    if err == AX_SUCCESS { Owned::new(value) } else { None }
}

/// The focused UI element (+1).
fn focused_element() -> Option<Owned> {
    let system = Owned::new(unsafe { AXUIElementCreateSystemWide() })?;
    unsafe { AXUIElementSetMessagingTimeout(system.0, TIMEOUT_S) };
    attribute(system.0, "AXFocusedUIElement")
}

fn is_password(element: &Owned) -> bool {
    (unsafe { IsSecureEventInputEnabled() })
        || attribute(element.0, "AXSubrole").and_then(|s| string(s.0)).as_deref() == Some("AXSecureTextField")
}

/// `FocusedText` implementation. Call it off the hook thread: AX calls can take hundreds of
/// milliseconds, which would get the event tap disabled.
#[derive(Clone, Copy, Debug, Default)]
pub struct MacFocusedText;

impl FocusedText for MacFocusedText {
    fn selected_text(&self) -> SelectionRead {
        let Some(element) = focused_element() else { return SelectionRead::Unavailable };
        if is_password(&element) {
            return SelectionRead::Password;
        }
        match attribute(element.0, "AXSelectedText").and_then(|t| string(t.0)) {
            Some(text) if text.is_empty() => SelectionRead::Empty,
            Some(text) => SelectionRead::Text(text),
            None => SelectionRead::Unavailable,
        }
    }

    fn focused_is_password(&self) -> bool {
        focused_element().is_some_and(|e| is_password(&e)) || unsafe { IsSecureEventInputEnabled() }
    }
}
