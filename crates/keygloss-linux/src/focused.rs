//! `FocusedText`: not implemented on Linux yet (AT-SPI would be the API). The engine falls back
//! to copying the selection with Ctrl+C, and cannot tell password fields apart.

use keygloss_core::platform::{FocusedText, SelectionRead};

/// `FocusedText` implementation.
#[derive(Clone, Copy, Debug, Default)]
pub struct LinuxFocusedText;

impl FocusedText for LinuxFocusedText {
    fn selected_text(&self) -> SelectionRead {
        SelectionRead::Unavailable
    }

    fn focused_is_password(&self) -> bool {
        false
    }
}
