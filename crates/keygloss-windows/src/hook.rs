//! `InputHook` via `WH_KEYBOARD_LL` + `WH_MOUSE_LL` on a dedicated message-loop thread.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::thread::JoinHandle;
use std::time::Instant;

use keygloss_core::platform::{
    HookDecision, InputHandler, InputHook, KeyCode, KeyEvent, Modifiers, PlatformError,
};
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Input::KeyboardAndMouse::{GetKeyState, VK_CAPITAL};
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::keymap::key_from_hook;
use crate::output::{is_own_event, replay_key};

const _: () = assert!((KeyCode::Unidentified as usize) < 128);

/// Per-hook-thread state. Lives in a thread-local because hook procs get no context pointer.
struct HookState {
    handler: Box<dyn InputHandler>,
    /// Keys currently down, indexed by `KeyCode as usize`.
    down: [bool; 128],
    /// Virtual keys whose initial key-down was swallowed; their key-up is swallowed too.
    swallowed: [bool; 256],
    caps_lock: bool,
}

/// A held key event: vk, scan, extended, down.
type Held = (u32, u32, bool, bool);

/// How often held keys are retried while output is pending.
const HELD_RETRY_MS: u32 = 5;

thread_local! {
    static STATE: RefCell<Option<HookState>> = const { RefCell::new(None) };
    /// Real key events held while output from another thread is pending, oldest first.
    static HELD: RefCell<VecDeque<Held>> = const { RefCell::new(VecDeque::new()) };
}

/// Hands the held keys to the handler once no output is pending, re-sending the passed ones.
unsafe extern "system" fn release_held(_: HWND, _: u32, timer: usize, _: u32) {
    loop {
        let pending = STATE.with(|s| s.try_borrow().map_or(true, |s| s.as_ref().is_some_and(|s| s.handler.output_pending())));
        if pending {
            return;
        }
        let Some((vk, scan, extended, down)) = HELD.with(|h| h.borrow_mut().pop_front()) else {
            unsafe {
                let _ = KillTimer(None, timer);
            }
            return;
        };
        let swallow = STATE.with(|s| match s.try_borrow_mut() {
            Ok(mut s) => s.as_mut().is_some_and(|s| s.on_key(vk, scan, extended, down)),
            Err(_) => false,
        });
        if !swallow {
            replay_key(vk, scan, extended, !down);
        }
    }
}

impl HookState {
    fn mods_except(&self, key: KeyCode) -> Modifiers {
        let held = |a: KeyCode, b: KeyCode| {
            (a != key && self.down[a as usize]) || (b != key && self.down[b as usize])
        };
        Modifiers {
            ctrl: held(KeyCode::ControlLeft, KeyCode::ControlRight),
            alt: held(KeyCode::AltLeft, KeyCode::AltRight),
            shift: held(KeyCode::ShiftLeft, KeyCode::ShiftRight),
            meta: held(KeyCode::MetaLeft, KeyCode::MetaRight),
        }
    }

    /// Returns true to swallow.
    fn on_key(&mut self, vk: u32, scan: u32, extended: bool, down: bool) -> bool {
        let key = key_from_hook(vk, scan, extended);
        let tracked = key != KeyCode::Unidentified;
        let repeat = down && tracked && self.down[key as usize];
        let event = KeyEvent {
            key,
            down,
            repeat,
            mods: self.mods_except(key),
            caps_lock: self.caps_lock,
            at: Instant::now(),
        };
        if tracked {
            self.down[key as usize] = down;
        }
        if key == KeyCode::CapsLock && down && !repeat {
            self.caps_lock = !self.caps_lock;
        }

        let swallow = self.handler.on_key(&event) == HookDecision::Swallow;
        let paired = &mut self.swallowed[(vk & 0xFF) as usize];
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
}

unsafe extern "system" fn keyboard_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 {
        // SAFETY: for HC_ACTION, lparam points to a KBDLLHOOKSTRUCT.
        let kb = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };
        if !is_own_event(kb.dwExtraInfo) {
            let down = matches!(wparam.0 as u32, WM_KEYDOWN | WM_SYSKEYDOWN);
            let extended = kb.flags.contains(LLKHF_EXTENDED);
            // Hold the key while output is pending, and behind keys already held (order).
            let hold = HELD.with(|h| !h.borrow().is_empty())
                || STATE.with(|s| s.try_borrow().is_ok_and(|s| s.as_ref().is_some_and(|s| s.handler.output_pending())));
            if hold {
                let first = HELD.with(|h| {
                    let mut h = h.borrow_mut();
                    h.push_back((kb.vkCode, kb.scanCode, extended, down));
                    h.len() == 1
                });
                if first {
                    unsafe { SetTimer(None, 0, HELD_RETRY_MS, Some(release_held)) };
                }
                return LRESULT(1);
            }
            // try_borrow_mut: never panic if the handler somehow re-enters the hook.
            let swallow = STATE.with(|s| match s.try_borrow_mut() {
                Ok(mut s) => s.as_mut().is_some_and(|s| s.on_key(kb.vkCode, kb.scanCode, extended, down)),
                Err(_) => false,
            });
            if swallow {
                return LRESULT(1);
            }
        }
    }
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

unsafe extern "system" fn mouse_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32
        && matches!(wparam.0 as u32, WM_LBUTTONDOWN | WM_RBUTTONDOWN | WM_MBUTTONDOWN | WM_XBUTTONDOWN)
    {
        // SAFETY: for HC_ACTION, lparam points to an MSLLHOOKSTRUCT.
        let ms = unsafe { &*(lparam.0 as *const MSLLHOOKSTRUCT) };
        if !is_own_event(ms.dwExtraInfo) {
            STATE.with(|s| {
                if let Ok(mut s) = s.try_borrow_mut()
                    && let Some(s) = s.as_mut()
                {
                    s.handler.on_mouse_down();
                }
            });
        }
    }
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

/// `InputHook` implementation.
#[derive(Default)]
pub struct WindowsHook {
    thread: Option<(u32, JoinHandle<()>)>,
    installed: Arc<AtomicBool>,
}

impl WindowsHook {
    pub fn new() -> Self {
        Self::default()
    }
}

fn run_hook_thread(
    handler: Box<dyn InputHandler>,
    installed: &AtomicBool,
    ready: mpsc::Sender<Result<u32, PlatformError>>,
) {
    unsafe {
        let mut msg = MSG::default();
        // Create the thread's message queue before anyone posts WM_QUIT to it.
        let _ = PeekMessageW(&mut msg, None, 0, 0, PM_NOREMOVE);
        let caps_lock = GetKeyState(i32::from(VK_CAPITAL.0)) & 1 != 0;
        STATE.set(Some(HookState { handler, down: [false; 128], swallowed: [false; 256], caps_lock }));

        let module = GetModuleHandleW(None).ok().map(HINSTANCE::from);
        let kb = SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_proc), module, 0);
        let ms = SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_proc), module, 0);
        let (kb, ms) = match (kb, ms) {
            (Ok(kb), Ok(ms)) => (kb, ms),
            (kb, ms) => {
                let err = kb.as_ref().err().or(ms.as_ref().err()).map(|e| e.message()).unwrap_or_default();
                for h in [kb, ms].into_iter().flatten() {
                    let _ = UnhookWindowsHookEx(h);
                }
                STATE.set(None);
                let _ = ready.send(Err(PlatformError(format!("SetWindowsHookExW failed: {err}"))));
                return;
            }
        };
        installed.store(true, Ordering::SeqCst);
        let _ = ready.send(Ok(GetCurrentThreadId()));

        while GetMessageW(&mut msg, None, 0, 0).0 > 0 {
            DispatchMessageW(&msg);
        }

        let _ = UnhookWindowsHookEx(kb);
        let _ = UnhookWindowsHookEx(ms);
        installed.store(false, Ordering::SeqCst);
        STATE.set(None);
    }
}

impl InputHook for WindowsHook {
    fn start(&mut self, handler: Box<dyn InputHandler>) -> Result<(), PlatformError> {
        if self.thread.is_some() {
            return Err(PlatformError("hook already started".into()));
        }
        let (tx, rx) = mpsc::channel();
        let installed = self.installed.clone();
        let handle = std::thread::Builder::new()
            .name("keygloss-hook".into())
            .spawn(move || run_hook_thread(handler, &installed, tx))
            .map_err(|e| PlatformError(format!("spawn hook thread: {e}")))?;
        match rx.recv() {
            Ok(Ok(tid)) => {
                self.thread = Some((tid, handle));
                Ok(())
            }
            Ok(Err(e)) => {
                let _ = handle.join();
                Err(e)
            }
            Err(_) => {
                let _ = handle.join();
                Err(PlatformError("hook thread exited".into()))
            }
        }
    }

    fn stop(&mut self) {
        if let Some((tid, handle)) = self.thread.take() {
            unsafe {
                let _ = PostThreadMessageW(tid, WM_QUIT, WPARAM(0), LPARAM(0));
            }
            let _ = handle.join();
        }
    }

    fn is_healthy(&self) -> bool {
        self.installed.load(Ordering::SeqCst)
    }
}

impl Drop for WindowsHook {
    fn drop(&mut self) {
        self.stop();
    }
}
