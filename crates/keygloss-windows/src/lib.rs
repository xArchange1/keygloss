//! Windows implementation of the `keygloss_core::platform` traits:
//!
//! - [`WindowsHook`] (`InputHook`): `WH_KEYBOARD_LL` + `WH_MOUSE_LL` on a dedicated thread with a
//!   message loop; swallow by return value; key-up pairing for swallowed key-downs; events tagged
//!   by `TextOutput` (via `dwExtraInfo`) are ignored.
//! - [`WindowsOutput`] (`TextOutput`): `SendInput` with Backspaces and `KEYEVENTF_UNICODE`.
//! - [`WindowsLayouts`] (`Layouts`): foreground thread's HKL, switching, enumeration, key tables.
//! - [`WindowsForegroundApp`] (`ForegroundApp`): foreground process and change notification.
//! - [`WindowsFocusedText`] (`FocusedText`): UI Automation selected text and `IsPassword`.
//! - [`WindowsClipboard`] (`Clipboard`): text, snapshot and restore, sequence number.
#![cfg(windows)]

mod apps;
mod clipboard;
mod hook;
mod keymap;
mod layouts;
mod output;
mod uia;

pub use apps::WindowsForegroundApp;
pub use clipboard::WindowsClipboard;
pub use hook::WindowsHook;
pub use layouts::WindowsLayouts;
pub use output::WindowsOutput;
pub use uia::WindowsFocusedText;
