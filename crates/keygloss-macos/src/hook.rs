//! `InputHook` via an active `CGEventTap` on a dedicated thread with its own run loop (never the
//! main thread: a busy main run loop would get the tap disabled by timeout, freezing input).
//!
//! Creating the tap needs the Accessibility permission. Without it [`MacHook::start`] still
//! succeeds: the thread asks for the permission once and retries until it is granted, and
//! [`InputHook::is_healthy`] stays false until then.

// The CoreGraphics constants keep their C names, also in match patterns.
#![allow(non_upper_case_globals)]

use std::collections::VecDeque;
use std::ffi::c_void;
use std::ptr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicPtr, Ordering};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use keygloss_core::platform::{
    HookDecision, InputHandler, InputHook, KeyCode, KeyEvent, Modifiers, PlatformError,
};

use crate::cf::*;
use crate::keymap::key_from_code;
use crate::output::{MARKER, is_own_event};

/// How often held keys are retried while output is pending, in seconds.
const HELD_RETRY: f64 = 0.005;
/// How often tap creation is retried while the Accessibility permission is missing.
const PERMISSION_RETRY: Duration = Duration::from_secs(2);

/// Device-dependent flag bit (`NX_DEVICE*KEYMASK`) and general flag of each modifier key.
const MOD_BITS: [(KeyCode, u64, u64); 8] = [
    (KeyCode::ControlLeft, 0x0001, kCGEventFlagMaskControl),
    (KeyCode::ShiftLeft, 0x0002, kCGEventFlagMaskShift),
    (KeyCode::ShiftRight, 0x0004, kCGEventFlagMaskShift),
    (KeyCode::MetaLeft, 0x0008, kCGEventFlagMaskCommand),
    (KeyCode::MetaRight, 0x0010, kCGEventFlagMaskCommand),
    (KeyCode::AltLeft, 0x0020, kCGEventFlagMaskAlternate),
    (KeyCode::AltRight, 0x0040, kCGEventFlagMaskAlternate),
    (KeyCode::ControlRight, 0x2000, kCGEventFlagMaskControl),
];

/// Whether modifier `key` is down according to `flags`. Events without side bits (posted by
/// other apps) count as the left key.
fn modifier_down(key: KeyCode, flags: u64) -> bool {
    let Some(&(_, bit, general)) = MOD_BITS.iter().find(|m| m.0 == key) else { return false };
    if flags & bit != 0 {
        return true;
    }
    let any_side = MOD_BITS.iter().filter(|m| m.2 == general).any(|m| flags & m.1 != 0);
    !any_side && flags & general != 0 && matches!(key, KeyCode::ControlLeft | KeyCode::ShiftLeft | KeyCode::MetaLeft | KeyCode::AltLeft)
}

/// State of the tap thread; the tap callback and the held-key timer get a pointer to it.
struct TapState {
    handler: Box<dyn InputHandler>,
    tap: CFMachPortRef,
    /// Keys currently down, indexed by `KeyCode as usize`.
    down: [bool; 128],
    /// macOS key codes whose initial key-down was swallowed; their key-up is swallowed too.
    swallowed: [bool; 128],
    /// Real events held while output is pending (copies), oldest first.
    held: VecDeque<(u32, Owned)>,
    /// Retry timer for `held`, null when not armed.
    timer: CFRunLoopTimerRef,
}

impl TapState {
    fn mods_except(&self, key: KeyCode) -> Modifiers {
        let held = |a: KeyCode, b: KeyCode| (a != key && self.down[a as usize]) || (b != key && self.down[b as usize]);
        Modifiers {
            ctrl: held(KeyCode::ControlLeft, KeyCode::ControlRight),
            alt: held(KeyCode::AltLeft, KeyCode::AltRight),
            shift: held(KeyCode::ShiftLeft, KeyCode::ShiftRight),
            meta: held(KeyCode::MetaLeft, KeyCode::MetaRight),
        }
    }

    /// Runs the handler for one key transition. Returns true to swallow.
    fn on_key(&mut self, key: KeyCode, code: u16, down: bool, repeat: bool, flags: u64, caps_lock: bool) -> bool {
        // The event flags are the truth about the other modifiers (no missed key-ups).
        for &(k, ..) in &MOD_BITS {
            if k != key {
                self.down[k as usize] = modifier_down(k, flags);
            }
        }
        let event = KeyEvent { key, down, repeat, mods: self.mods_except(key), caps_lock, at: Instant::now() };
        if key != KeyCode::Unidentified {
            self.down[key as usize] = down;
        }
        let swallow = self.handler.on_key(&event) == HookDecision::Swallow;
        let paired = &mut self.swallowed[usize::from(code & 0x7F)];
        if down {
            // Pairing follows the initial press; repeats only follow the handler.
            if !repeat {
                *paired = swallow;
            }
            swallow
        } else {
            std::mem::take(paired) || swallow
        }
    }

    /// Handles a key or modifier event. Returns true to swallow.
    fn process(&mut self, ty: u32, event: CGEventRef) -> bool {
        let code = unsafe { CGEventGetIntegerValueField(event, kCGKeyboardEventKeycode) } as u16;
        let flags = unsafe { CGEventGetFlags(event) };
        let caps = flags & kCGEventFlagMaskAlphaShift != 0;
        let key = key_from_code(code);
        if ty != kCGEventFlagsChanged {
            let down = ty == kCGEventKeyDown;
            let repeat = down && unsafe { CGEventGetIntegerValueField(event, kCGKeyboardEventAutorepeat) } != 0;
            return self.on_key(key, code, down, repeat, flags, caps);
        }
        match key {
            // Caps Lock reports one event per toggle and no release: report a full press, with
            // the state before the toggle on the key-down (as Windows does).
            KeyCode::CapsLock => {
                let swallow = self.on_key(key, code, true, false, flags, !caps);
                self.on_key(key, code, false, false, flags, caps) || swallow
            }
            k if k.is_modifier() => self.on_key(key, code, modifier_down(key, flags), false, flags, caps),
            // Fn and other flag keys: reported (they cancel modifier gestures), never tracked.
            _ => self.on_key(KeyCode::Unidentified, code, flags & kCGEventFlagMaskSecondaryFn != 0, false, flags, caps),
        }
    }

    fn arm_timer(&mut self) {
        let mut ctx = CFRunLoopTimerContext {
            version: 0,
            info: (self as *mut TapState).cast(),
            retain: ptr::null(),
            release: ptr::null(),
            copy_description: ptr::null(),
        };
        unsafe {
            let first = CFAbsoluteTimeGetCurrent() + HELD_RETRY;
            self.timer = CFRunLoopTimerCreate(ptr::null(), first, HELD_RETRY, 0, 0, release_held, &mut ctx);
            CFRunLoopAddTimer(CFRunLoopGetCurrent(), self.timer, kCFRunLoopCommonModes);
        }
    }

    fn disarm_timer(&mut self) {
        if !self.timer.is_null() {
            unsafe {
                CFRunLoopTimerInvalidate(self.timer);
                CFRelease(self.timer);
            }
            self.timer = ptr::null();
        }
    }
}

/// Hands the held events to the handler once no output is pending, re-posting the passed ones.
extern "C" fn release_held(_timer: CFRunLoopTimerRef, info: *mut c_void) {
    // SAFETY: `info` is the tap thread's `TapState`; the timer runs on that thread.
    let st = unsafe { &mut *(info as *mut TapState) };
    while !st.handler.output_pending() {
        let Some((ty, event)) = st.held.pop_front() else {
            st.disarm_timer();
            return;
        };
        if !st.process(ty, event.0) {
            unsafe {
                CGEventSetIntegerValueField(event.0, kCGEventSourceUserData, MARKER);
                CGEventPost(kCGHIDEventTap, event.0);
            }
        }
    }
}

extern "C" fn tap_callback(_proxy: CGEventTapProxy, ty: u32, event: CGEventRef, info: *mut c_void) -> CGEventRef {
    // SAFETY: `info` is the tap thread's `TapState`; the callback runs on that thread.
    let st = unsafe { &mut *(info as *mut TapState) };
    match ty {
        kCGEventTapDisabledByTimeout | kCGEventTapDisabledByUserInput => {
            unsafe { CGEventTapEnable(st.tap, true) };
            // Releases went by while the tap was off.
            st.swallowed = [false; 128];
            return event;
        }
        kCGEventLeftMouseDown | kCGEventRightMouseDown | kCGEventOtherMouseDown => {
            if !is_own_event(event) {
                st.handler.on_mouse_down();
            }
            return event;
        }
        kCGEventKeyDown | kCGEventKeyUp | kCGEventFlagsChanged => {}
        _ => return event,
    }
    if is_own_event(event) {
        return event;
    }
    // Hold the event while output is pending, and behind events already held (order).
    if !st.held.is_empty() || st.handler.output_pending() {
        if let Some(copy) = Owned::new(unsafe { CGEventCreateCopy(event) }) {
            st.held.push_back((ty, copy));
        }
        if st.timer.is_null() {
            st.arm_timer();
        }
        return ptr::null();
    }
    if st.process(ty, event) { ptr::null() } else { event }
}

#[derive(Default)]
struct Shared {
    stop: AtomicBool,
    installed: AtomicBool,
    run_loop: AtomicPtr<c_void>,
}

/// `InputHook` implementation.
#[derive(Default)]
pub struct MacHook {
    thread: Option<JoinHandle<()>>,
    shared: Arc<Shared>,
}

impl MacHook {
    pub fn new() -> Self {
        Self::default()
    }
}

fn mask(types: &[u32]) -> u64 {
    types.iter().fold(0, |m, t| m | 1 << t)
}

fn run_tap_thread(handler: Box<dyn InputHandler>, shared: &Shared) {
    let state = Box::into_raw(Box::new(TapState {
        handler,
        tap: ptr::null(),
        down: [false; 128],
        swallowed: [false; 128],
        held: VecDeque::new(),
        timer: ptr::null(),
    }));
    let events = mask(&[
        kCGEventKeyDown,
        kCGEventKeyUp,
        kCGEventFlagsChanged,
        kCGEventLeftMouseDown,
        kCGEventRightMouseDown,
        kCGEventOtherMouseDown,
    ]);
    let mut prompted = false;
    let tap = loop {
        if shared.stop.load(Ordering::SeqCst) {
            drop(unsafe { Box::from_raw(state) });
            return;
        }
        let tap = unsafe {
            CGEventTapCreate(kCGSessionEventTap, kCGHeadInsertEventTap, kCGEventTapOptionDefault, events, tap_callback, state.cast())
        };
        if !tap.is_null() {
            break tap;
        }
        if !prompted {
            prompted = true;
            crate::shell::request_accessibility();
        }
        let until = Instant::now() + PERMISSION_RETRY;
        while Instant::now() < until && !shared.stop.load(Ordering::SeqCst) {
            std::thread::sleep(Duration::from_millis(100));
        }
    };
    unsafe {
        (*state).tap = tap;
        let source = CFMachPortCreateRunLoopSource(ptr::null(), tap, 0);
        let run_loop = CFRunLoopGetCurrent();
        CFRunLoopAddSource(run_loop, source, kCFRunLoopCommonModes);
        CGEventTapEnable(tap, true);
        shared.run_loop.store(run_loop as *mut c_void, Ordering::SeqCst);
        shared.installed.store(true, Ordering::SeqCst);
        // A stop that came before the run loop started is kept by the run loop and ends it at once.
        if !shared.stop.load(Ordering::SeqCst) {
            CFRunLoopRun();
        }
        shared.installed.store(false, Ordering::SeqCst);
        CGEventTapEnable(tap, false);
        CFMachPortInvalidate(tap);
        CFRelease(source);
        CFRelease(tap);
        (*state).disarm_timer();
        drop(Box::from_raw(state));
    }
}

impl InputHook for MacHook {
    fn start(&mut self, handler: Box<dyn InputHandler>) -> Result<(), PlatformError> {
        if self.thread.is_some() {
            return Err(PlatformError("hook already started".into()));
        }
        let shared = self.shared.clone();
        let handle = std::thread::Builder::new()
            .name("keygloss-hook".into())
            .spawn(move || run_tap_thread(handler, &shared))
            .map_err(|e| PlatformError(format!("spawn hook thread: {e}")))?;
        self.thread = Some(handle);
        Ok(())
    }

    fn stop(&mut self) {
        if let Some(handle) = self.thread.take() {
            self.shared.stop.store(true, Ordering::SeqCst);
            let run_loop = self.shared.run_loop.load(Ordering::SeqCst);
            if !run_loop.is_null() {
                unsafe { CFRunLoopStop(run_loop) };
            }
            let _ = handle.join();
        }
    }

    fn is_healthy(&self) -> bool {
        self.shared.installed.load(Ordering::SeqCst)
    }
}

impl Drop for MacHook {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modifier_sides() {
        let shift_right = kCGEventFlagMaskShift | 0x0004;
        assert!(modifier_down(KeyCode::ShiftRight, shift_right));
        assert!(!modifier_down(KeyCode::ShiftLeft, shift_right));
        // No side bits: the left key.
        assert!(modifier_down(KeyCode::MetaLeft, kCGEventFlagMaskCommand));
        assert!(!modifier_down(KeyCode::MetaRight, kCGEventFlagMaskCommand));
        assert!(!modifier_down(KeyCode::AltLeft, 0));
    }
}
