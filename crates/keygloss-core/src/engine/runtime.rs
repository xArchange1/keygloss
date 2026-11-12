//! Runtime glue (std only): the worker thread that runs [`Job`]s and fires timers, the
//! [`InputHandler`] installed on the hook, and the [`EngineHandle`] the app holds.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::data;
use crate::platform::{
    Clipboard, FocusedText, ForegroundApp, HookDecision, InputHandler, InputHook, KeyCode, KeyEvent, Layouts,
    Modifiers, PlatformError, SelectionRead, TextOutput,
};
use crate::settings::{Hotkey, Settings};

use super::{Engine, EngineEvent, EngineStatus, HotkeyCheck, HotkeyTarget, Job, JobResult, Selection, check_hotkey};

/// The worker wakes at least this often (hook health polling).
const IDLE_WAKE: Duration = Duration::from_secs(2);
/// Ctrl+C fallback: poll the clipboard this many times …
const COPY_POLLS: u32 = 25;
/// … this far apart.
const COPY_POLL_EVERY: Duration = Duration::from_millis(10);

/// Time source. The engine itself never reads it: the runtime passes times in.
pub trait Clock: Send + Sync {
    fn now(&self) -> Instant;
    /// Wall time, Unix seconds.
    fn unix_now(&self) -> f64;
    fn sleep(&self, d: Duration);
}

/// The real clock.
#[derive(Clone, Copy, Debug, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> Instant {
        Instant::now()
    }
    fn unix_now(&self) -> f64 {
        SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0)
    }
    fn sleep(&self, d: Duration) {
        std::thread::sleep(d);
    }
}

/// The OS, as the engine sees it.
#[derive(Clone)]
pub struct Platform {
    pub output: Arc<dyn TextOutput>,
    pub layouts: Arc<dyn Layouts>,
    pub foreground: Arc<dyn ForegroundApp>,
    pub focused: Arc<dyn FocusedText>,
    pub clipboard: Arc<dyn Clipboard>,
}

pub(crate) type EventSink = Arc<dyn Fn(EngineEvent) + Send + Sync>;

/// One [`TextOutput`] call held back until the engine lock is released.
enum OutputCall {
    Replace(usize, String),
    Chord(KeyCode, Modifiers),
    Mask,
}

/// The engine's [`TextOutput`]. Output from the hook thread is sent at once; output from any
/// other thread is queued and sent by [`Shared::with`] / [`pump`] after the engine lock is
/// released. `SendInput` waits for every low-level hook to handle each injected event, ours
/// included, so sending under the lock froze input for the hook timeout (seconds) whenever a real
/// key reached the hook mid-send and blocked on the lock.
struct QueuedOutput {
    inner: Arc<dyn TextOutput>,
    /// Set (under the engine lock) while the hook thread runs the engine.
    direct: AtomicBool,
    queue: Mutex<Vec<OutputCall>>,
    /// Calls are queued or being sent ([`InputHandler::output_pending`]).
    pending: AtomicBool,
}

impl QueuedOutput {
    fn call(&self, c: OutputCall) -> Result<(), PlatformError> {
        if !self.direct.load(Ordering::SeqCst) {
            self.queue.lock().unwrap_or_else(|p| p.into_inner()).push(c);
            self.pending.store(true, Ordering::SeqCst);
            return Ok(());
        }
        match c {
            OutputCall::Replace(n, text) => self.inner.replace_text(n, &text),
            OutputCall::Chord(key, mods) => self.inner.send_chord(key, mods),
            OutputCall::Mask => self.inner.send_mask_key(),
        }
    }

    /// Sends the queued calls. Call without the engine lock. Failures are the inner output's to
    /// report (the app logs them).
    fn flush(&self) {
        let queue = || self.queue.lock().unwrap_or_else(|p| p.into_inner());
        let calls = std::mem::take(&mut *queue());
        for c in calls {
            let _ = match c {
                OutputCall::Replace(n, text) => self.inner.replace_text(n, &text),
                OutputCall::Chord(key, mods) => self.inner.send_chord(key, mods),
                OutputCall::Mask => self.inner.send_mask_key(),
            };
        }
        let q = queue();
        if q.is_empty() {
            self.pending.store(false, Ordering::SeqCst);
        }
    }
}

impl TextOutput for QueuedOutput {
    fn replace_text(&self, backspaces: usize, text: &str) -> Result<(), PlatformError> {
        self.call(OutputCall::Replace(backspaces, text.to_string()))
    }
    fn send_chord(&self, key: KeyCode, mods: Modifiers) -> Result<(), PlatformError> {
        self.call(OutputCall::Chord(key, mods))
    }
    fn send_mask_key(&self) -> Result<(), PlatformError> {
        self.call(OutputCall::Mask)
    }
}

pub(crate) struct Shared {
    pub(crate) engine: Mutex<Engine>,
    output: Arc<QueuedOutput>,
    wake: Condvar,
    /// Real key events currently inside [`InputHandler::on_key`].
    pub(crate) hook_busy: Arc<AtomicUsize>,
    stop: AtomicBool,
}

pub(crate) fn lock(m: &Mutex<Engine>) -> MutexGuard<'_, Engine> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

impl Shared {
    pub(crate) fn new(settings: Settings, platform: &Platform, clock: &dyn Clock) -> Arc<Shared> {
        let hook_busy = Arc::new(AtomicUsize::new(0));
        let output = Arc::new(QueuedOutput {
            inner: platform.output.clone(),
            direct: AtomicBool::new(false),
            queue: Mutex::new(Vec::new()),
            pending: AtomicBool::new(false),
        });
        let engine = Engine::new(
            settings,
            output.clone(),
            platform.layouts.clone(),
            hook_busy.clone(),
            clock.now(),
            clock.unix_now(),
        );
        Arc::new(Shared {
            engine: Mutex::new(engine),
            output,
            wake: Condvar::new(),
            hook_busy,
            stop: AtomicBool::new(false),
        })
    }

    /// Runs `f` on the engine and wakes the worker if it queued something.
    pub(crate) fn with<T>(&self, f: impl FnOnce(&mut Engine) -> T) -> T {
        let mut e = lock(&self.engine);
        let out = f(&mut e);
        if e.wake() {
            self.wake.notify_all();
        }
        drop(e);
        self.output.flush();
        out
    }
}

/// The engine as the hook's [`InputHandler`].
pub(crate) struct HookHandler {
    pub(crate) shared: Arc<Shared>,
    pub(crate) clock: Arc<dyn Clock>,
}

impl InputHandler for HookHandler {
    fn on_key(&mut self, event: &KeyEvent) -> HookDecision {
        self.shared.hook_busy.fetch_add(1, Ordering::SeqCst);
        let out = &self.shared.output;
        let decision = self.shared.with(|e| {
            // On the hook thread output cannot deadlock; send what another thread left queued
            // first so the order holds.
            out.flush();
            out.direct.store(true, Ordering::SeqCst);
            let d = e.on_key(event);
            out.direct.store(false, Ordering::SeqCst);
            d
        });
        self.shared.hook_busy.fetch_sub(1, Ordering::SeqCst);
        decision
    }

    fn on_mouse_down(&mut self) {
        let now = self.clock.now();
        self.shared.with(|e| e.on_mouse_down(now));
    }

    fn output_pending(&self) -> bool {
        self.shared.output.pending.load(Ordering::SeqCst)
    }
}

/// Reads the selection: UI Automation first, then Ctrl+C with clipboard snapshot and restore
/// when the element exposes no selection API (`SelectionText.read`).
fn read_selection(p: &Platform, clock: &dyn Clock) -> Selection {
    match p.focused.selected_text() {
        SelectionRead::Text(text) => Selection::Text { text, via_clipboard: false },
        SelectionRead::Empty => Selection::Nothing,
        SelectionRead::Password => Selection::Password,
        SelectionRead::Unavailable => match copy_selection(p, clock) {
            Some(text) => Selection::Text { text, via_clipboard: true },
            None => Selection::Nothing,
        },
    }
}

/// `readViaClipboard`.
fn copy_selection(p: &Platform, clock: &dyn Clock) -> Option<String> {
    let cb = &p.clipboard;
    let snapshot = cb.snapshot();
    let before = cb.change_count();
    p.output.send_chord(KeyCode::KeyC, Modifiers { ctrl: true, ..Modifiers::NONE }).ok()?;
    let mut copied = None;
    for _ in 0..COPY_POLLS {
        clock.sleep(COPY_POLL_EVERY);
        if cb.change_count() != before {
            copied = cb.get_text();
            break;
        }
    }
    if cb.change_count() != before {
        let _ = cb.restore(&snapshot);
    }
    copied.filter(|t| !t.is_empty())
}

/// `PlainPaste.paste`, up to the Ctrl+V; the restore is a timer.
fn plain_paste(p: &Platform) -> Option<(crate::platform::ClipboardSnapshot, u64)> {
    let cb = &p.clipboard;
    let text = cb.get_text().filter(|t| !t.is_empty())?;
    let snapshot = cb.snapshot();
    cb.set_text(&text).ok()?;
    let ours = cb.change_count();
    let _ = p.output.send_chord(KeyCode::KeyV, Modifiers { ctrl: true, ..Modifiers::NONE });
    Some((snapshot, ours))
}

fn run_job(job: Job, p: &Platform, clock: &dyn Clock) -> JobResult {
    match job {
        Job::RefreshLayouts => JobResult::Layouts(
            p.layouts.installed_layouts().into_iter().map(|l| { let t = p.layouts.key_table(&l.id); (l, t) }).collect(),
        ),
        Job::ForegroundApp => JobResult::Foreground(p.foreground.foreground_app()),
        Job::RunningApps => JobResult::RunningApps(p.foreground.running_apps().into_iter().map(|a| a.id).collect()),
        Job::CheckPassword(then) => JobResult::Password(p.focused.focused_is_password(), then),
        Job::ReadSelection(purpose) => JobResult::Selection(purpose, read_selection(p, clock)),
        Job::PlainPaste => JobResult::PlainPasted(plain_paste(p)),
        Job::RestoreClipboard(snapshot, ours) => {
            if p.clipboard.change_count() == ours {
                let _ = p.clipboard.restore(&snapshot);
            }
            JobResult::Done
        }
    }
}

/// Fires due timers and runs queued jobs until there is nothing left to do now.
pub(crate) fn pump(shared: &Shared, p: &Platform, clock: &dyn Clock, sink: &EventSink) {
    loop {
        let (job, events) = {
            let mut e = lock(&shared.engine);
            e.tick(clock.now());
            (e.take_job(), e.take_events())
        };
        shared.output.flush();
        for ev in events {
            sink(ev);
        }
        let Some(job) = job else { return };
        let result = run_job(job, p, clock);
        shared.with(|e| e.on_job_result(result, clock.now()));
    }
}

fn worker(shared: Arc<Shared>, p: Platform, clock: Arc<dyn Clock>, sink: EventSink, hook: Arc<Mutex<Box<dyn InputHook + Send>>>) {
    loop {
        if shared.stop.load(Ordering::SeqCst) {
            return;
        }
        let healthy = hook.lock().unwrap_or_else(|p| p.into_inner()).is_healthy();
        shared.with(|e| e.set_hook_state(true, healthy));
        pump(&shared, &p, &*clock, &sink);
        let mut e = lock(&shared.engine);
        if shared.stop.load(Ordering::SeqCst) {
            return;
        }
        if e.wake() {
            e.clear_wake();
            continue;
        }
        let now = clock.now();
        let timeout = e.next_deadline().map_or(IDLE_WAKE, |d| d.saturating_duration_since(now)).min(IDLE_WAKE);
        let (mut e, _) = shared.wake.wait_timeout(e, timeout).unwrap_or_else(|p| p.into_inner());
        e.clear_wake();
    }
}

struct Inner {
    shared: Arc<Shared>,
    clock: Arc<dyn Clock>,
    hook: Arc<Mutex<Box<dyn InputHook + Send>>>,
    worker: Mutex<Option<JoinHandle<()>>>,
}

/// The running engine. Cheap to clone; every method is safe from any thread.
#[derive(Clone)]
pub struct EngineHandle {
    inner: Arc<Inner>,
}

/// Starts the engine: loads the language data in the background, installs `hook` with the engine
/// as its handler, subscribes to foreground changes and starts the worker thread. `on_event` is
/// called on the worker thread.
pub fn start(
    platform: Platform,
    hook: Box<dyn InputHook + Send>,
    settings: Settings,
    on_event: Arc<dyn Fn(EngineEvent) + Send + Sync>,
) -> Result<EngineHandle, PlatformError> {
    let _ = std::thread::Builder::new().name("keygloss-warmup".into()).spawn(|| {
        data::warm_up();
    });
    start_with_clock(platform, hook, settings, on_event, Arc::new(SystemClock))
}

pub(crate) fn start_with_clock(
    platform: Platform,
    mut hook: Box<dyn InputHook + Send>,
    settings: Settings,
    on_event: EventSink,
    clock: Arc<dyn Clock>,
) -> Result<EngineHandle, PlatformError> {
    let shared = Shared::new(settings, &platform, &*clock);
    {
        let (shared, clock) = (shared.clone(), clock.clone());
        platform.foreground.on_foreground_change(Box::new(move |app| {
            if !shared.stop.load(Ordering::SeqCst) {
                let now = clock.now();
                shared.with(|e| e.on_foreground_change(app, now));
            }
        }));
    }
    if let Err(e) = hook.start(Box::new(HookHandler { shared: shared.clone(), clock: clock.clone() })) {
        shared.stop.store(true, Ordering::SeqCst);
        return Err(e);
    }
    let healthy = hook.is_healthy();
    shared.with(|e| {
        e.set_hook_state(true, healthy);
        e.toast_clash();
    });
    let hook = Arc::new(Mutex::new(hook));
    let worker = {
        let (shared, platform, clock, hook) = (shared.clone(), platform.clone(), clock.clone(), hook.clone());
        std::thread::Builder::new()
            .name("keygloss-engine".into())
            .spawn(move || worker(shared, platform, clock, on_event, hook))
            .map_err(|e| PlatformError(format!("spawn engine worker: {e}")))?
    };
    Ok(EngineHandle { inner: Arc::new(Inner { shared, clock, hook, worker: Mutex::new(Some(worker)) }) })
}

impl EngineHandle {
    fn with<T>(&self, f: impl FnOnce(&mut Engine, Instant) -> T) -> T {
        let now = self.inner.clock.now();
        self.inner.shared.with(|e| f(e, now))
    }

    /// Current settings (including everything the engine learned).
    pub fn settings(&self) -> Settings {
        self.with(|e, _| e.settings().clone())
    }

    /// Shallow-merges a partial settings object (top-level keys, as the UI sends them) into the
    /// current settings and returns the result. A `SettingsChanged` event follows.
    pub fn update_settings(&self, patch: serde_json::Value) -> Result<Settings, String> {
        self.with(|e, now| {
            let mut merged = serde_json::to_value(e.settings()).map_err(|err| err.to_string())?;
            let (serde_json::Value::Object(base), serde_json::Value::Object(patch)) = (&mut merged, patch) else {
                return Err("patch must be an object".into());
            };
            base.extend(patch);
            let new: Settings = serde_json::from_value(merged).map_err(|err| err.to_string())?;
            e.set_settings(new.clone(), now);
            Ok(new)
        })
    }

    /// Replaces all settings.
    pub fn replace_settings(&self, settings: Settings) {
        self.with(|e, now| e.set_settings(settings, now));
    }

    pub fn status(&self) -> EngineStatus {
        self.with(|e, now| e.status(now))
    }

    /// Pauses automatic correction, live fix and snippets for `minutes`.
    pub fn pause(&self, minutes: u32) -> EngineStatus {
        self.with(|e, now| {
            e.pause(minutes, now);
            e.status(now)
        })
    }

    pub fn resume(&self) -> EngineStatus {
        self.with(|e, now| {
            e.resume(now);
            e.status(now)
        })
    }

    /// Automatic correction on/off (`autoEnabled`).
    pub fn set_auto(&self, on: bool) -> EngineStatus {
        self.with(|e, now| {
            e.set_auto(on);
            e.status(now)
        })
    }

    /// Routes key presses to `HotkeyCaptured` events (swallowed) until stopped.
    pub fn start_hotkey_recording(&self) {
        self.with(|e, _| e.set_recording(true));
    }

    pub fn stop_hotkey_recording(&self) {
        self.with(|e, _| e.set_recording(false));
    }

    /// The recorder's verdict for assigning `hotkey` to `target`, against the current settings.
    pub fn check_hotkey(&self, target: HotkeyTarget, hotkey: &Hotkey) -> HotkeyCheck {
        self.with(|e, _| check_hotkey(target, hotkey, e.settings()))
    }

    /// "Keep <word> as is?": `keep` adds it to "Never switch", otherwise it is declined for good.
    pub fn answer_learn_banner(&self, word: &str, keep: bool) {
        self.with(|e, _| e.answer_learn(word, keep));
    }

    /// The banner timed out unanswered.
    pub fn dismiss_learn_banner(&self, word: &str) {
        self.with(|e, _| e.dismiss_learn(word));
    }

    /// "Always switch to <word>?": `add` puts it into "Always switch", otherwise it is declined for good.
    pub fn answer_switch_banner(&self, word: &str, add: bool) {
        self.with(|e, _| e.answer_switch(word, add));
    }

    /// The switch banner timed out unanswered.
    pub fn dismiss_switch_banner(&self, word: &str) {
        self.with(|e, _| e.dismiss_switch(word));
    }

    /// Inserts picker entry `index` (0-based) and closes the picker.
    pub fn snippet_picker_choose(&self, index: usize) {
        self.with(|e, now| e.picker_choose(index, now));
    }

    pub fn snippet_picker_close(&self) {
        self.with(|e, _| e.close_picker());
    }

    /// Removes the hook and stops the worker. The handle is inert afterwards.
    pub fn shutdown(&self) {
        let inner = &self.inner;
        inner.shared.stop.store(true, Ordering::SeqCst);
        inner.shared.wake.notify_all();
        if let Some(w) = inner.worker.lock().unwrap_or_else(|p| p.into_inner()).take() {
            let _ = w.join();
        }
        inner.hook.lock().unwrap_or_else(|p| p.into_inner()).stop();
        inner.shared.with(|e| e.set_hook_state(false, false));
    }
}
