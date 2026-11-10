//! Windows implementation of the `keygloss_core::platform` traits:
//!
//! - `InputHook`: `WH_KEYBOARD_LL` + `WH_MOUSE_LL` on a dedicated thread with a message loop;
//!   swallow by return value; key-up pairing for swallowed key-downs; events tagged by
//!   `TextOutput` (via `dwExtraInfo`) are ignored.
//! - `TextOutput`: `SendInput` with Backspaces and `KEYEVENTF_UNICODE`, tagged through `dwExtraInfo`.
//! - `Layouts`: foreground thread's HKL, switching, enumeration, key tables via `ToUnicodeEx`.
//! - `ForegroundApp`: foreground process executable name and change notification.
//! - `FocusedText`: UI Automation selected text and `IsPassword`.
//! - `Clipboard`: text read/write, snapshot and restore, sequence number.
