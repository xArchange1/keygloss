//! The virtual input device (uinput). The hook grabs the physical keyboards, so every key that
//! reaches the desktop comes from here: the keys the hook passes on and Keygloss's own output.
//! The hook skips this device by name, which is how synthetic input stays out of the handler.
//!
//! The device has LEDs: the desktop sets Caps Lock and Num Lock on it, as on any keyboard, and the
//! hook reads them back ([`Vkbd::read_leds`]) and mirrors them to the grabbed keyboards, whose own
//! LEDs the desktop can no longer set. Created with raw ioctls, since `evdev`'s builder has no LEDs.

use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};
use std::os::fd::{AsRawFd, RawFd};
use std::os::unix::fs::OpenOptionsExt;
use std::sync::Mutex;

use evdev::{EventType, InputEvent};
use keygloss_core::platform::PlatformError;

pub const NAME: &str = "Keygloss virtual keyboard";

/// Shift, Ctrl, Alt and Meta, left and right.
const MODIFIERS: [u16; 8] = [42, 54, 29, 97, 56, 100, 125, 126];

/// Mouse buttons (`BTN_LEFT`..`BTN_TASK`), forwarded from keyboards that also have a pointer.
pub const BUTTONS: std::ops::RangeInclusive<u16> = 0x110..=0x117;

/// `REL_X`, `REL_Y`, `REL_HWHEEL`, `REL_WHEEL`, `REL_WHEEL_HI_RES`, `REL_HWHEEL_HI_RES`.
pub const REL_AXES: [u16; 6] = [0, 1, 6, 8, 11, 12];

/// `LED_NUML`, `LED_CAPSL`, `LED_SCROLLL`.
pub const LEDS: std::ops::Range<u16> = 0..3;
pub const LED_CAPSL: u16 = 1;

// <linux/uinput.h>, `_IOW('U', nr, size)` and `_IO('U', nr)`.
const UI_SET_EVBIT: libc::c_ulong = 0x4004_5564;
const UI_SET_KEYBIT: libc::c_ulong = 0x4004_5565;
const UI_SET_RELBIT: libc::c_ulong = 0x4004_5566;
const UI_SET_LEDBIT: libc::c_ulong = 0x4004_5569;
const UI_DEV_SETUP: libc::c_ulong = 0x405c_5503;
const UI_DEV_CREATE: libc::c_ulong = 0x5501;

/// `struct uinput_setup`.
#[repr(C)]
struct Setup {
    bustype: u16,
    vendor: u16,
    product: u16,
    version: u16,
    name: [u8; 80],
    ff_effects_max: u32,
}

pub struct Vkbd {
    file: File,
    /// Key codes below 256 currently down on this device.
    down: [bool; 256],
}

static VKBD: Mutex<Option<Vkbd>> = Mutex::new(None);

fn lock() -> std::sync::MutexGuard<'static, Option<Vkbd>> {
    VKBD.lock().unwrap_or_else(|p| p.into_inner())
}

fn ioctl(fd: RawFd, request: libc::c_ulong, arg: libc::c_ulong) -> io::Result<()> {
    if unsafe { libc::ioctl(fd, request as _, arg) } < 0 { Err(io::Error::last_os_error()) } else { Ok(()) }
}

/// Creates the device (once).
pub fn create() -> io::Result<()> {
    let mut slot = lock();
    if slot.is_some() {
        return Ok(());
    }
    let file = OpenOptions::new().read(true).write(true).custom_flags(libc::O_NONBLOCK).open("/dev/uinput")?;
    let fd = file.as_raw_fd();
    let caps: [(EventType, libc::c_ulong, Vec<u16>); 3] = [
        (EventType::KEY, UI_SET_KEYBIT, (1..256).chain(BUTTONS).collect()),
        (EventType::RELATIVE, UI_SET_RELBIT, REL_AXES.to_vec()),
        (EventType::LED, UI_SET_LEDBIT, LEDS.collect()),
    ];
    for (ty, request, codes) in caps {
        ioctl(fd, UI_SET_EVBIT, ty.0.into())?;
        for code in codes {
            ioctl(fd, request, code.into())?;
        }
    }
    let mut setup = Setup { bustype: 0x06, vendor: 0x1234, product: 0x5678, version: 1, name: [0; 80], ff_effects_max: 0 };
    setup.name[..NAME.len()].copy_from_slice(NAME.as_bytes());
    ioctl(fd, UI_DEV_SETUP, &raw const setup as libc::c_ulong)?;
    ioctl(fd, UI_DEV_CREATE, 0)?;
    *slot = Some(Vkbd { file, down: [false; 256] });
    Ok(())
}

/// Releases the keys still down and removes the device (closing it destroys it).
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

fn raw(ty: u16, code: u16, value: i32) -> libc::input_event {
    libc::input_event { time: libc::timeval { tv_sec: 0, tv_usec: 0 }, type_: ty, code, value }
}

impl Vkbd {
    /// The device file, which becomes readable when the desktop sets an LED.
    pub fn fd(&self) -> RawFd {
        self.file.as_raw_fd()
    }

    /// Sends events as one report (a `SYN_REPORT` follows them).
    pub fn emit(&mut self, events: &[InputEvent]) -> io::Result<()> {
        let mut out: Vec<libc::input_event> = Vec::with_capacity(events.len() + 1);
        for e in events {
            if e.event_type() == EventType::KEY && e.code() < 256 {
                self.down[usize::from(e.code())] = e.value() != 0;
            }
            out.push(raw(e.event_type().0, e.code(), e.value()));
        }
        out.push(raw(EventType::SYNCHRONIZATION.0, 0, 0));
        let bytes = unsafe {
            std::slice::from_raw_parts(out.as_ptr().cast::<u8>(), std::mem::size_of_val(out.as_slice()))
        };
        self.file.write_all(bytes)
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

    /// The LED changes (code, on) the desktop has sent since the last call.
    pub fn read_leds(&mut self) -> Vec<(u16, bool)> {
        let mut leds = Vec::new();
        let mut buf = [0u8; size_of::<libc::input_event>() * 16];
        while let Ok(n) = self.file.read(&mut buf) {
            if n == 0 {
                break;
            }
            for chunk in buf[..n].as_chunks::<{ size_of::<libc::input_event>() }>().0 {
                let e: libc::input_event = unsafe { std::ptr::read_unaligned(chunk.as_ptr().cast()) };
                if e.type_ == EventType::LED.0 && LEDS.contains(&e.code) {
                    leds.push((e.code, e.value != 0));
                }
            }
        }
        leds
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use evdev::raw_stream::RawDevice;

    /// Needs write access to `/dev/uinput` and read access to `/dev/input` (e.g. root).
    #[test]
    #[ignore]
    fn creates_device_and_reads_leds_back() {
        create().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(500));
        let path = std::fs::read_dir("/dev/input")
            .unwrap()
            .flatten()
            .map(|e| e.path())
            .find(|p| RawDevice::open(p).is_ok_and(|d| d.name() == Some(NAME)))
            .expect("event device");
        let mut dev = RawDevice::open(&path).unwrap();
        assert!(dev.supported_leds().is_some_and(|l| l.iter().count() == 3));
        assert!(dev.supported_keys().is_some_and(|k| k.contains(evdev::KeyCode::KEY_A)));
        dev.send_events(&[InputEvent::new(EventType::LED.0, LED_CAPSL, 1)]).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(100));
        assert_eq!(with(|k| k.read_leds()).unwrap(), vec![(LED_CAPSL, true)]);
        with(|k| k.tap(30)).unwrap().unwrap();
        let keys: Vec<(u16, i32)> = dev
            .fetch_events()
            .unwrap()
            .filter(|e| e.event_type() == EventType::KEY)
            .map(|e| (e.code(), e.value()))
            .collect();
        assert_eq!(keys, vec![(30, 1), (30, 0)]);
        destroy();
    }
}
