//! Linux implementation of the `keygloss_core::platform` traits, for KDE Plasma 6 on Wayland: the
//! input side works on any desktop, the clipboard needs Wayland, layouts and the foreground app
//! need KDE.
//!
//! - [`LinuxHook`] (`InputHook`): evdev, keyboards grabbed and re-sent through a uinput device.
//! - [`LinuxOutput`] (`TextOutput`): key presses through that device, in a layout that has the
//!   characters; the clipboard for the rest.
//! - [`LinuxLayouts`] (`Layouts`): KDE's keyboard daemon on D-Bus; libxkbcommon key tables.
//! - [`LinuxForegroundApp`] (`ForegroundApp`): a KWin script that reports window activations.
//! - [`LinuxFocusedText`] (`FocusedText`): not implemented (always the Ctrl+C fallback).
//! - [`LinuxClipboard`] (`Clipboard`): the Wayland data-control protocol.
#![cfg(target_os = "linux")]

mod apps;
mod clipboard;
mod focused;
mod hook;
mod keymap;
mod layouts;
mod output;
pub mod shell;
mod vkbd;

pub use apps::LinuxForegroundApp;
pub use clipboard::LinuxClipboard;
pub use focused::LinuxFocusedText;
pub use hook::LinuxHook;
pub use layouts::LinuxLayouts;
pub use output::LinuxOutput;
