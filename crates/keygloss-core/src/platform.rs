//! OS-agnostic contracts between the engine and the operating system.
//!
//! `keygloss-windows` implements these traits; the engine in `src-tauri` drives them.
//! Nothing here depends on OS types.

use std::collections::HashMap;
use std::fmt;
use std::time::Instant;

use serde::{Deserialize, Serialize};

/// Physical key, named by its position on a US-QWERTY keyboard.
///
/// Variant names are the W3C UI Events `KeyboardEvent.code` values and serialize as those
/// strings (`"KeyA"`, `"Pause"`, `"AltRight"`), so the frontend can use `event.code` directly.
///
/// Windows mapping (done in `keygloss-windows`): character keys come from the hook's scan code
/// (set 1, plus `LLKHF_EXTENDED` for the E0 prefix), so they are layout-independent; keys whose
/// scan codes are ambiguous or layout-free (Pause, NumLock, PrintScreen, arrows vs numpad,
/// left/right modifiers) come from `vkCode` (`VK_PAUSE`, `VK_LSHIFT`/`VK_RSHIFT`, ...).
/// The reverse direction for `SendInput` uses `MapVirtualKeyExW`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum KeyCode {
    // Letters
    KeyA, KeyB, KeyC, KeyD, KeyE, KeyF, KeyG, KeyH, KeyI, KeyJ, KeyK, KeyL, KeyM,
    KeyN, KeyO, KeyP, KeyQ, KeyR, KeyS, KeyT, KeyU, KeyV, KeyW, KeyX, KeyY, KeyZ,
    // Digit row
    Digit0, Digit1, Digit2, Digit3, Digit4, Digit5, Digit6, Digit7, Digit8, Digit9,
    // Punctuation keys
    Backquote, Minus, Equal, BracketLeft, BracketRight, Backslash, Semicolon, Quote,
    Comma, Period, Slash, IntlBackslash,
    // Whitespace and editing
    Space, Enter, Tab, Backspace, Delete, Insert, Escape,
    // Navigation
    ArrowLeft, ArrowRight, ArrowUp, ArrowDown, Home, End, PageUp, PageDown,
    // Modifiers (left/right distinct)
    ShiftLeft, ShiftRight, ControlLeft, ControlRight, AltLeft, AltRight, MetaLeft, MetaRight,
    // Locks and system keys
    CapsLock, NumLock, ScrollLock, Pause, PrintScreen, ContextMenu,
    // Function keys
    F1, F2, F3, F4, F5, F6, F7, F8, F9, F10, F11, F12,
    F13, F14, F15, F16, F17, F18, F19, F20, F21, F22, F23, F24,
    // Numpad
    Numpad0, Numpad1, Numpad2, Numpad3, Numpad4, Numpad5, Numpad6, Numpad7, Numpad8, Numpad9,
    NumpadDecimal, NumpadAdd, NumpadSubtract, NumpadMultiply, NumpadDivide, NumpadEnter,
    /// Any key not listed above (media keys, IME keys, ...). Never a hotkey or a character.
    Unidentified,
}

impl KeyCode {
    /// True for the eight Shift/Control/Alt/Meta keys.
    pub fn is_modifier(self) -> bool {
        matches!(
            self,
            KeyCode::ShiftLeft
                | KeyCode::ShiftRight
                | KeyCode::ControlLeft
                | KeyCode::ControlRight
                | KeyCode::AltLeft
                | KeyCode::AltRight
                | KeyCode::MetaLeft
                | KeyCode::MetaRight
        )
    }
}

/// Modifier state, side-insensitive. Side matters only for modifier-tap hotkeys, which name a
/// specific [`KeyCode`] instead.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Modifiers {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    /// Windows key (Command on macOS).
    pub meta: bool,
}

impl Modifiers {
    pub const NONE: Modifiers = Modifiers { ctrl: false, alt: false, shift: false, meta: false };

    /// True when no modifier is held.
    pub fn is_empty(self) -> bool {
        self == Modifiers::NONE
    }
}

/// A real (non-synthetic) key event from the global hook.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyEvent {
    pub key: KeyCode,
    /// `true` for key-down (including auto-repeat), `false` for key-up.
    pub down: bool,
    /// Auto-repeat key-down (the key was already down).
    pub repeat: bool,
    /// Modifiers held when the event arrived, not counting this key itself.
    pub mods: Modifiers,
    /// Caps Lock toggle state when the event arrived.
    pub caps_lock: bool,
    /// When the event arrived on the hook thread.
    pub at: Instant,
}

/// What the hook does with a key event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HookDecision {
    /// Let the event reach the focused application.
    Pass,
    /// Drop the event; the application never sees it.
    Swallow,
}

/// Receives input from [`InputHook`]. Called on the hook thread; every call must return fast
/// (Windows silently removes a low-level hook that blocks too long).
pub trait InputHandler: Send + 'static {
    /// Handles one real key event. A swallowed key-down also swallows its matching key-up
    /// (the hook pairs them), whatever this returns for the key-up.
    fn on_key(&mut self, event: &KeyEvent) -> HookDecision;
    /// Any real mouse button press (resets typing context).
    fn on_mouse_down(&mut self);
}

/// Global keyboard and mouse-button hook. Synthetic events sent through [`TextOutput`] are
/// tagged and never reach the handler.
pub trait InputHook {
    /// Installs the hook on its own thread and starts delivering events to `handler`.
    fn start(&mut self, handler: Box<dyn InputHandler>) -> Result<(), PlatformError>;
    /// Removes the hook and stops its thread.
    fn stop(&mut self);
    /// False if the hook is not installed or the OS has dropped it.
    fn is_healthy(&self) -> bool;
}

/// Synthetic keyboard output into the focused application.
/// All methods are safe to call from inside [`InputHandler::on_key`].
pub trait TextOutput: Send + Sync {
    /// Sends `backspaces` Backspace presses, then types `text` as Unicode characters.
    fn replace_text(&self, backspaces: usize, text: &str) -> Result<(), PlatformError>;
    /// Presses and releases `key` with `mods` held, e.g. Ctrl+C, Ctrl+V, Enter.
    fn send_chord(&self, key: KeyCode, mods: Modifiers) -> Result<(), PlatformError>;
    /// Taps a key that does nothing (Windows: unassigned vk 0xE8), so that releasing Alt or Win
    /// after a modifier-tap hotkey does not open the window menu or the Start menu.
    fn send_mask_key(&self) -> Result<(), PlatformError>;
}

/// Opaque OS identifier of an installed keyboard layout (Windows: the HKL as hex, e.g. `"04190419"`).
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct LayoutId(pub String);

/// One installed keyboard layout.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LayoutInfo {
    pub id: LayoutId,
    /// BCP-47 language tag of the layout, e.g. `"ru-RU"`, `"en-US"`.
    pub lang: String,
    /// Human-readable name, e.g. `"Russian"`.
    pub name: String,
}

/// Characters a layout produces: (key, shift held) → character. Dead keys and keys that
/// produce nothing or more than one character are absent.
pub type KeyTable = HashMap<(KeyCode, bool), char>;

/// Keyboard layout query, switching and key tables.
pub trait Layouts: Send + Sync {
    /// Layout of the foreground window's input thread. Cheap; may be called from
    /// [`InputHandler::on_key`].
    fn current_layout(&self) -> Option<LayoutId>;
    /// Switches the foreground window to `id`. Cheap (asynchronous request); may be called from
    /// [`InputHandler::on_key`].
    fn switch_layout(&self, id: &LayoutId) -> Result<(), PlatformError>;
    /// Installed layouts in the OS switching order.
    fn installed_layouts(&self) -> Vec<LayoutInfo>;
    /// Builds the key table of layout `id` from the OS (no Caps Lock, no AltGr).
    fn key_table(&self, id: &LayoutId) -> KeyTable;
}

/// Identity of an application, by process executable name.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct AppId(pub String);

impl AppId {
    /// Normalizes an executable name to the stored form: lowercase file name, e.g. `"code.exe"`.
    pub fn from_exe(name: &str) -> AppId {
        AppId(name.to_lowercase())
    }
}

/// A running application, for the per-app exceptions picker.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppInfo {
    pub id: AppId,
    /// Display name (file description), falling back to the executable name.
    pub name: String,
}

/// Foreground application tracking.
pub trait ForegroundApp: Send + Sync {
    /// The application owning the foreground window.
    fn foreground_app(&self) -> Option<AppId>;
    /// Calls `callback` (on an OS thread) every time the foreground application changes.
    fn on_foreground_change(&self, callback: Box<dyn Fn(AppId) + Send + Sync>);
    /// Applications that currently have a visible top-level window.
    fn running_apps(&self) -> Vec<AppInfo>;
}

/// Result of reading the focused element's selection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SelectionRead {
    /// The selected text (non-empty).
    Text(String),
    /// The element supports selection, but nothing is selected.
    Empty,
    /// The focused element is a password field; never read or convert it.
    Password,
    /// The element exposes no text selection API; the caller may fall back to Ctrl+C.
    Unavailable,
}

/// Focused-element text access (Windows: UI Automation).
/// Must not be called from [`InputHandler::on_key`]: calls can block for up to ~0.5 s.
pub trait FocusedText: Send + Sync {
    /// Reads the selected text of the focused element.
    fn selected_text(&self) -> SelectionRead;
    /// True if the focused element is a password field.
    fn focused_is_password(&self) -> bool;
}

/// Opaque copy of the whole clipboard (every format), for snapshot and restore.
#[derive(Clone, Debug, Default)]
pub struct ClipboardSnapshot {
    /// (OS format id, raw bytes). Never interpreted by the core.
    pub formats: Vec<(u32, Vec<u8>)>,
}

/// Clipboard access.
pub trait Clipboard: Send + Sync {
    /// Current clipboard text, if any.
    fn get_text(&self) -> Option<String>;
    /// Replaces the clipboard with plain `text`, marked as excluded from clipboard history.
    fn set_text(&self, text: &str) -> Result<(), PlatformError>;
    /// Copies every clipboard format.
    fn snapshot(&self) -> ClipboardSnapshot;
    /// Puts a snapshot back.
    fn restore(&self, snapshot: &ClipboardSnapshot) -> Result<(), PlatformError>;
    /// Counter that changes on every clipboard write (to detect foreign changes).
    fn change_count(&self) -> u64;
}

/// An OS call failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlatformError(pub String);

impl fmt::Display for PlatformError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for PlatformError {}
