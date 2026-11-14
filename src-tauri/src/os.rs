//! This OS's implementation of the platform traits and shell helpers.

#[cfg(windows)]
pub use keygloss_windows::{
    WindowsClipboard as OsClipboard, WindowsFocusedText as OsFocusedText, WindowsForegroundApp as OsForegroundApp,
    WindowsHook as OsHook, WindowsLayouts as OsLayouts, WindowsOutput as OsOutput, shell,
};

#[cfg(target_os = "macos")]
pub use keygloss_macos::{
    MacClipboard as OsClipboard, MacFocusedText as OsFocusedText, MacForegroundApp as OsForegroundApp,
    MacHook as OsHook, MacLayouts as OsLayouts, MacOutput as OsOutput, shell,
};
