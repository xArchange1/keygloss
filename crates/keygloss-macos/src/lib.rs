//! macOS implementation of the `keygloss_core::platform` traits:
//!
//! - [`MacHook`] (`InputHook`): an active `CGEventTap` on a dedicated run-loop thread; swallow by
//!   returning no event; key-up pairing for swallowed key-downs; events tagged by `TextOutput`
//!   (via `kCGEventSourceUserData`) are ignored. Needs the Accessibility permission.
//! - [`MacOutput`] (`TextOutput`): `CGEventPost` with Backspaces and Unicode strings.
//! - [`MacLayouts`] (`Layouts`): Text Input Sources, cached on the main thread; `UCKeyTranslate`
//!   key tables.
//! - [`MacForegroundApp`] (`ForegroundApp`): `NSWorkspace` frontmost app, by bundle id.
//! - [`MacFocusedText`] (`FocusedText`): Accessibility `AXSelectedText` and secure text fields.
//! - [`MacClipboard`] (`Clipboard`): `NSPasteboard` text, snapshot and restore, change count.
#![cfg(target_os = "macos")]

mod apps;
mod ax;
mod cf;
mod clipboard;
mod hook;
mod keymap;
mod layouts;
mod output;
pub mod shell;

pub use apps::MacForegroundApp;
pub use ax::MacFocusedText;
pub use clipboard::MacClipboard;
pub use hook::MacHook;
pub use layouts::MacLayouts;
pub use output::MacOutput;
