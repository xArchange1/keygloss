//! The virtual input device (uinput). The hook grabs the physical keyboards, so every key that
//! reaches the desktop comes from here: the keys the hook passes on and Keygloss's own output.
//! The hook skips this device by name, which is how synthetic input stays out of the handler.

use std::io;
use std::sync::Mutex;

use evdev::uinput::VirtualDevice;
use evdev::{AttributeSet, EventType, InputEvent, KeyCode, RelativeAxisCode};
use keygloss_core::platform::PlatformError;

pub const NAME: &str = "Keygloss virtual keyboard";

/// Shift, Ctrl, Alt and Meta, left and right.
const MODIFIERS: [u16; 8] = [42, 54, 29, 97, 56, 100, 125, 126];

/// Mouse buttons (`BTN_LEFT`..`BTN_TASK`), forwarded from keyboards that also have a pointer.
pub const BUTTONS: std::ops::RangeInclusive<u16> = 0x110..=0x117;

pub struct Vkbd {
    dev: VirtualDevice,
    /// Key codes below 256 currently down on this device.
    down: [bool; 256],
}

static VKBD: Mutex<Option<Vkbd>> = Mutex::new(None);

fn lock() -> std::sync::MutexGuard<'static, Option<Vkbd>> {
    VKBD.lock().unwrap_or_else(|p| p.into_inner())
}

/// Creates the device (once).
pub fn create() -> io::Result<()> {
    let mut slot = lock();
    if slot.is_some() {
        return Ok(());
    }
    let mut keys = AttributeSet::<KeyCode>::new();
    for code in (1..256).chain(BUTTONS) {
        keys.insert(KeyCode::new(code));
    }
    let mut axes = AttributeSet::<RelativeAxisCode>::new();
    for axis in [
        RelativeAxisCode::REL_X,
        RelativeAxisCode::REL_Y,
        RelativeAxisCode::REL_WHEEL,
        RelativeAxisCode::REL_HWHEEL,
        RelativeAxisCode::REL_WHEEL_HI_RES,
        RelativeAxisCode::REL_HWHEEL_HI_RES,
    ] {
        axes.insert(axis);
    }
    let dev = VirtualDevice::builder()?.name(NAME).with_keys(&keys)?.with_relative_axes(&axes)?.build()?;
    *slot = Some(Vkbd { dev, down: [false; 256] });
    Ok(())
}

/// Releases the keys still down and removes the device.
pub fn destroy() {
    if let Some(mut k) = lock().take() {
        for code in 0..256u16 {
            if k.down[usize::from(code)] {
                let _ = k.key(code, false);
            }
        }
    }
}

/// Runs `f` on the device; an error if the hook has not created it.
pub fn with<T>(f: impl FnOnce(&mut Vkbd) -> T) -> Result<T, PlatformError> {
    lock().as_mut().map(f).ok_or_else(|| PlatformError("no virtual keyboard: the keyboard hook is not running".into()))
}

impl Vkbd {
    /// Sends events as one report (a `SYN_REPORT` follows them).
    pub fn emit(&mut self, events: &[InputEvent]) -> io::Result<()> {
        for e in events {
            if e.event_type() == EventType::KEY && e.code() < 256 {
                self.down[usize::from(e.code())] = e.value() != 0;
            }
        }
        self.dev.emit(events)
    }

    pub fn key(&mut self, code: u16, down: bool) -> io::Result<()> {
        self.emit(&[InputEvent::new(EventType::KEY.0, code, i32::from(down))])
    }

    pub fn tap(&mut self, code: u16) -> io::Result<()> {
        self.key(code, true)?;
        self.key(code, false)
    }

    /// Releases the modifiers held on this device; returns them, to press again later.
    pub fn release_modifiers(&mut self) -> io::Result<Vec<u16>> {
        let held: Vec<u16> = MODIFIERS.into_iter().filter(|&c| self.down[usize::from(c)]).collect();
        for &c in &held {
            self.key(c, false)?;
        }
        Ok(held)
    }
}
