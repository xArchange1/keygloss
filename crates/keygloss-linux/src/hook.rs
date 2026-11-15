//! `InputHook` over evdev, the keyd model: the physical keyboards are grabbed (`EVIOCGRAB`), so
//! the desktop sees none of their events, and every key the handler passes is re-sent through the
//! virtual keyboard ([`crate::vkbd`]). Swallowing a key is not re-sending it. Mice are read
//! without a grab, for button presses only.
//!
//! Needs read access to `/dev/input/event*` and write access to `/dev/uinput` (the packages'
//! udev rule gives both to the logged-in user). Without them [`LinuxHook::start`] still succeeds:
//! the thread retries, and [`InputHook::is_healthy`] stays false until it has a keyboard.
//!
//! The desktop cannot set a grabbed keyboard's LEDs: it sets them on the virtual keyboard, and the
//! hook copies them to the grabbed ones. [`CAPS_LOCK`] comes from there too.
//!
//! Safety nets for the grab: a crash closes the devices, which ends the grab; a watchdog thread
//! ungrabs everything if the hook thread stalls; held keys are let go after [`HOLD_LIMIT`] even
//! if output is still pending.

use std::collections::VecDeque;
use std::fs;
use std::os::fd::{AsRawFd, RawFd};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use evdev::raw_stream::RawDevice;
use evdev::{EventType, InputEvent, KeyCode as EvKey, SynchronizationCode};
use keygloss_core::platform::{HookDecision, InputHandler, InputHook, KeyCode, KeyEvent, Modifiers, PlatformError};

use crate::keymap::key_from_code;
use crate::vkbd::{self, BUTTONS, LED_CAPSL, LEDS, REL_AXES};

/// Device rescan (hotplug) and permission retry interval.
const RESCAN: Duration = Duration::from_secs(2);
/// Poll timeout while keys are held for pending output.
const HELD_RETRY: Duration = Duration::from_millis(5);
/// Poll timeout otherwise: also how often the stop flag and the rescan are checked.
const IDLE_POLL: Duration = Duration::from_millis(500);
/// Held keys are handed on after this long even if output is still pending.
const HOLD_LIMIT: Duration = Duration::from_secs(3);
/// The watchdog ungrabs the keyboards when the hook thread has not looped for this long.
const STALL_LIMIT: Duration = Duration::from_secs(3);

/// Caps Lock as the desktop last set it on the virtual keyboard.
pub static CAPS_LOCK: AtomicBool = AtomicBool::new(false);

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64)
}

struct Dev {
    raw: RawDevice,
    path: PathBuf,
    /// A keyboard to grab; grabbed once no key is down on it (a key down at grab time would
    /// never be released for the desktop).
    keyboard: bool,
    grabbed: bool,
    /// Pointer events of a grabbed device, re-sent at its next `SYN_REPORT`.
    report: Vec<InputEvent>,
}

impl Dev {
    fn set_led(&mut self, code: u16, on: bool) {
        let _ = self.raw.send_events(&[InputEvent::new(EventType::LED.0, code, i32::from(on))]);
    }
}

#[derive(Default)]
struct Shared {
    stop: AtomicBool,
    installed: AtomicBool,
    /// Set by the watchdog: the keyboards were ungrabbed and the hook thread must end.
    failed: AtomicBool,
    heartbeat: AtomicU64,
    /// File descriptors of the grabbed devices, for the watchdog.
    grabbed: Mutex<Vec<RawFd>>,
}

impl Shared {
    fn grabbed(&self) -> std::sync::MutexGuard<'_, Vec<RawFd>> {
        self.grabbed.lock().unwrap_or_else(|p| p.into_inner())
    }
}

struct HookState {
    handler: Box<dyn InputHandler>,
    /// Keys currently down, indexed by `KeyCode as usize`.
    down: [bool; 128],
    /// Evdev key codes whose initial key-down was swallowed; their key-up is swallowed too.
    swallowed: [bool; 256],
    /// Real key events (code, value, Caps Lock) held while output is pending, oldest first.
    held: VecDeque<(u16, i32, bool, Instant)>,
    /// LED states the desktop set on the virtual keyboard (Num, Caps, Scroll Lock).
    leds: [Option<bool>; LEDS.end as usize],
}

impl HookState {
    fn mods_except(&self, key: KeyCode) -> Modifiers {
        let held = |a: KeyCode, b: KeyCode| (a != key && self.down[a as usize]) || (b != key && self.down[b as usize]);
        Modifiers {
            ctrl: held(KeyCode::ControlLeft, KeyCode::ControlRight),
            alt: held(KeyCode::AltLeft, KeyCode::AltRight),
            shift: held(KeyCode::ShiftLeft, KeyCode::ShiftRight),
            meta: held(KeyCode::MetaLeft, KeyCode::MetaRight),
        }
    }

    /// Runs the handler for one key event, then re-sends it unless swallowed.
    fn process(&mut self, code: u16, value: i32, caps_lock: bool) {
        let key = key_from_code(code);
        let (down, repeat) = (value != 0, value == 2);
        let event = KeyEvent { key, down, repeat, mods: self.mods_except(key), caps_lock, at: Instant::now() };
        if key != KeyCode::Unidentified {
            self.down[key as usize] = down;
        }
        let swallow = self.handler.on_key(&event) == HookDecision::Swallow;
        let paired = &mut self.swallowed[usize::from(code)];
        let swallow = if down {
            // Pairing follows the initial press; repeats only follow the handler.
            if !repeat {
                *paired = swallow;
            }
            swallow
        } else {
            std::mem::take(paired) || swallow
        };
        if !swallow {
            let _ = vkbd::with(|k| k.emit(&[InputEvent::new(EventType::KEY.0, code, value)]));
        }
    }

    /// A key event of a grabbed keyboard: held behind pending output, or processed now.
    fn on_key(&mut self, code: u16, value: i32, caps_lock: bool) {
        if !self.held.is_empty() || self.handler.output_pending() {
            self.held.push_back((code, value, caps_lock, Instant::now()));
        } else {
            self.process(code, value, caps_lock);
        }
    }

    /// Hands held events on once no output is pending (or the oldest has waited too long).
    fn release_held(&mut self) {
        while let Some(&(code, value, caps, at)) = self.held.front() {
            if self.handler.output_pending() && at.elapsed() < HOLD_LIMIT {
                return;
            }
            self.held.pop_front();
            self.process(code, value, caps);
        }
    }
}

fn is_keyboard(raw: &RawDevice) -> bool {
    raw.supported_keys().is_some_and(|k| [EvKey::KEY_A, EvKey::KEY_Z, EvKey::KEY_SPACE].iter().all(|&c| k.contains(c)))
}

fn has_buttons(raw: &RawDevice) -> bool {
    raw.supported_keys().is_some_and(|k| k.contains(EvKey::BTN_LEFT))
}

/// Touchpads, touchscreens and tablets: never grabbed (the virtual device has no absolute axes).
fn is_absolute(raw: &RawDevice) -> bool {
    raw.supported_absolute_axes().is_some_and(|a| a.iter().next().is_some())
}

fn set_nonblocking(fd: RawFd) {
    unsafe {
        let flags = libc::fcntl(fd, libc::F_GETFL);
        libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK);
    }
}

/// Opens the event devices not open yet: keyboards and devices with mouse buttons.
fn scan(devs: &mut Vec<Dev>) {
    let Ok(entries) = fs::read_dir("/dev/input") else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if !entry.file_name().to_string_lossy().starts_with("event") || devs.iter().any(|d| d.path == path) {
            continue;
        }
        let Ok(raw) = RawDevice::open(&path) else { continue };
        if raw.name() == Some(vkbd::NAME) {
            continue;
        }
        let keyboard = is_keyboard(&raw) && !is_absolute(&raw);
        if !keyboard && !has_buttons(&raw) {
            continue;
        }
        set_nonblocking(raw.as_raw_fd());
        devs.push(Dev { raw, path, keyboard, grabbed: false, report: Vec::new() });
    }
}

/// Grabs the keyboards that are waiting for their keys to be released, and sets their LEDs.
fn grab_ready(devs: &mut [Dev], leds: &[Option<bool>], shared: &Shared) {
    for d in devs.iter_mut().filter(|d| d.keyboard && !d.grabbed) {
        if d.raw.get_key_state().is_ok_and(|k| k.iter().next().is_none()) && d.raw.grab().is_ok() {
            d.grabbed = true;
            shared.grabbed().push(d.raw.as_raw_fd());
            for (code, on) in LEDS.zip(leds) {
                if let Some(on) = *on {
                    d.set_led(code, on);
                }
            }
        }
    }
}

/// Copies the LEDs the desktop set on the virtual keyboard to the grabbed keyboards.
fn sync_leds(st: &mut HookState, devs: &mut [Dev]) {
    for (code, on) in vkbd::with(|k| k.read_leds()).unwrap_or_default() {
        st.leds[usize::from(code)] = Some(on);
        if code == LED_CAPSL {
            CAPS_LOCK.store(on, Ordering::Relaxed);
        }
        for d in devs.iter_mut().filter(|d| d.grabbed) {
            d.set_led(code, on);
        }
    }
}

fn close(devs: &mut Vec<Dev>, i: usize, shared: &Shared) {
    let fd = devs[i].raw.as_raw_fd();
    shared.grabbed().retain(|&g| g != fd);
    devs.swap_remove(i);
}

/// One event from device `i`.
fn on_event(st: &mut HookState, dev: &mut Dev, e: InputEvent) {
    let (ty, code, value) = (e.event_type(), e.code(), e.value());
    if ty == EventType::KEY && BUTTONS.contains(&code) {
        if value == 1 {
            st.handler.on_mouse_down();
        }
        if dev.grabbed {
            dev.report.push(e);
        }
    } else if ty == EventType::KEY && code < 256 {
        if dev.grabbed {
            st.on_key(code, value, CAPS_LOCK.load(Ordering::Relaxed));
        }
    } else if dev.grabbed && ty == EventType::RELATIVE {
        // Only the axes the virtual device has.
        if REL_AXES.contains(&code) {
            dev.report.push(e);
        }
    } else if ty == EventType::SYNCHRONIZATION && code == SynchronizationCode::SYN_REPORT.0 && !dev.report.is_empty() {
        let report = std::mem::take(&mut dev.report);
        let _ = vkbd::with(|k| k.emit(&report));
    }
}

fn run(handler: Box<dyn InputHandler>, shared: &Shared) {
    let mut st =
        HookState { handler, down: [false; 128], swallowed: [false; 256], held: VecDeque::new(), leds: Default::default() };
    let mut devs: Vec<Dev> = Vec::new();
    let mut last_scan: Option<Instant> = None;
    while !shared.stop.load(Ordering::SeqCst) && !shared.failed.load(Ordering::SeqCst) {
        shared.heartbeat.store(now_ms(), Ordering::SeqCst);
        if last_scan.is_none_or(|t| t.elapsed() >= RESCAN) {
            last_scan = Some(Instant::now());
            if vkbd::create().is_ok() {
                scan(&mut devs);
            }
        }
        grab_ready(&mut devs, &st.leds, shared);
        shared.installed.store(devs.iter().any(|d| d.grabbed), Ordering::SeqCst);

        // The devices, then the virtual keyboard (LEDs).
        let mut fds: Vec<libc::pollfd> = devs
            .iter()
            .map(|d| d.raw.as_raw_fd())
            .chain(vkbd::with(|k| k.fd()).ok())
            .map(|fd| libc::pollfd { fd, events: libc::POLLIN, revents: 0 })
            .collect();
        let timeout = if st.held.is_empty() { IDLE_POLL } else { HELD_RETRY };
        let n = unsafe { libc::poll(fds.as_mut_ptr(), fds.len() as libc::nfds_t, timeout.as_millis() as i32) };
        if n > 0 {
            if fds.len() > devs.len() && fds[devs.len()].revents & libc::POLLIN != 0 {
                sync_leds(&mut st, &mut devs);
            }
            // Backwards: `close` swaps the last device into the removed slot.
            for i in (0..devs.len()).rev() {
                let revents = fds[i].revents;
                if revents & (libc::POLLERR | libc::POLLHUP | libc::POLLNVAL) != 0 {
                    close(&mut devs, i, shared);
                    continue;
                }
                if revents & libc::POLLIN == 0 {
                    continue;
                }
                let fetched = devs[i].raw.fetch_events().map(|events| events.collect::<Vec<InputEvent>>());
                let events = match fetched {
                    Ok(events) => events,
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => continue,
                    Err(_) => {
                        close(&mut devs, i, shared);
                        continue;
                    }
                };
                for e in events {
                    on_event(&mut st, &mut devs[i], e);
                }
            }
        }
        st.release_held();
    }
    shared.installed.store(false, Ordering::SeqCst);
    shared.grabbed().clear();
    // Dropping the devices closes them, which ends the grabs.
    drop(devs);
    vkbd::destroy();
}

/// Ungrabs the keyboards if the hook thread stops looping.
fn watchdog(shared: &Shared) {
    while !shared.stop.load(Ordering::SeqCst) {
        std::thread::sleep(Duration::from_millis(500));
        let last = shared.heartbeat.load(Ordering::SeqCst);
        if last != 0 && now_ms().saturating_sub(last) > STALL_LIMIT.as_millis() as u64 {
            let fds = shared.grabbed();
            for &fd in fds.iter() {
                // EVIOCGRAB with 0: release the grab.
                unsafe { libc::ioctl(fd, 0x4004_4590, 0) };
            }
            shared.failed.store(true, Ordering::SeqCst);
            shared.installed.store(false, Ordering::SeqCst);
            return;
        }
    }
}

/// `InputHook` implementation.
#[derive(Default)]
pub struct LinuxHook {
    threads: Vec<JoinHandle<()>>,
    shared: Arc<Shared>,
}

impl LinuxHook {
    pub fn new() -> Self {
        Self::default()
    }
}

impl InputHook for LinuxHook {
    fn start(&mut self, handler: Box<dyn InputHandler>) -> Result<(), PlatformError> {
        if !self.threads.is_empty() {
            return Err(PlatformError("hook already started".into()));
        }
        let spawn = |name: &str, f: Box<dyn FnOnce() + Send>| {
            std::thread::Builder::new().name(name.into()).spawn(f).map_err(|e| PlatformError(format!("spawn {name}: {e}")))
        };
        let shared = self.shared.clone();
        self.threads.push(spawn("keygloss-hook", Box::new(move || run(handler, &shared)))?);
        let shared = self.shared.clone();
        self.threads.push(spawn("keygloss-hook-watchdog", Box::new(move || watchdog(&shared)))?);
        Ok(())
    }

    fn stop(&mut self) {
        self.shared.stop.store(true, Ordering::SeqCst);
        for t in self.threads.drain(..) {
            let _ = t.join();
        }
    }

    fn is_healthy(&self) -> bool {
        self.shared.installed.load(Ordering::SeqCst) && !self.shared.failed.load(Ordering::SeqCst)
    }
}

impl Drop for LinuxHook {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use evdev::AttributeSet;
    use evdev::uinput::VirtualDevice;

    struct Recorder(Arc<Mutex<Vec<(KeyCode, bool, bool)>>>);

    impl InputHandler for Recorder {
        fn on_key(&mut self, e: &KeyEvent) -> HookDecision {
            self.0.lock().unwrap().push((e.key, e.down, e.caps_lock));
            if e.key == KeyCode::KeyQ { HookDecision::Swallow } else { HookDecision::Pass }
        }
        fn on_mouse_down(&mut self) {}
    }

    fn open_by_name(name: &str) -> RawDevice {
        fs::read_dir("/dev/input")
            .unwrap()
            .flatten()
            .find_map(|e| RawDevice::open(e.path()).ok().filter(|d| d.name() == Some(name)))
            .expect("event device")
    }

    /// A fake keyboard (uinput) stands in for a physical one: its keys reach the handler, the
    /// passed ones come out of the virtual keyboard, and its Caps Lock LED feeds the events.
    /// Needs `/dev/uinput` and `/dev/input` access (e.g. root); grabs every real keyboard too.
    #[test]
    #[ignore]
    fn grabs_keyboard_and_re_sends_passed_keys() {
        let keys: AttributeSet<EvKey> = [EvKey::KEY_A, EvKey::KEY_Q, EvKey::KEY_Z, EvKey::KEY_SPACE].into_iter().collect();
        let mut fake = VirtualDevice::builder().unwrap().name("Keygloss test keyboard").with_keys(&keys).unwrap().build().unwrap();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let mut hook = LinuxHook::new();
        hook.start(Box::new(Recorder(seen.clone()))).unwrap();
        std::thread::sleep(Duration::from_millis(1000));
        assert!(hook.is_healthy());
        let mut out = open_by_name(vkbd::NAME);
        set_nonblocking(out.as_raw_fd());
        out.send_events(&[InputEvent::new(EventType::LED.0, LED_CAPSL, 1)]).unwrap();
        std::thread::sleep(Duration::from_millis(200));
        for code in [EvKey::KEY_Q, EvKey::KEY_A] {
            fake.emit(&[InputEvent::new(EventType::KEY.0, code.0, 1)]).unwrap();
            fake.emit(&[InputEvent::new(EventType::KEY.0, code.0, 0)]).unwrap();
        }
        std::thread::sleep(Duration::from_millis(300));
        assert_eq!(
            *seen.lock().unwrap(),
            [(KeyCode::KeyQ, true, true), (KeyCode::KeyQ, false, true), (KeyCode::KeyA, true, true), (KeyCode::KeyA, false, true)]
        );
        let sent: Vec<(u16, i32)> = out
            .fetch_events()
            .unwrap()
            .filter(|e| e.event_type() == EventType::KEY)
            .map(|e| (e.code(), e.value()))
            .collect();
        hook.stop();
        assert_eq!(sent, [(EvKey::KEY_A.0, 1), (EvKey::KEY_A.0, 0)]);
    }
}
