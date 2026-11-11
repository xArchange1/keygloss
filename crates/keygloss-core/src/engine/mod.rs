//! The engine: the state machine between the input hook and the OS (`Engine.swift`, the
//! non-voice parts of `EventTap.swift`, `LayoutManager.swift`, `Pause.swift`, `PlainPaste.swift`,
//! `SelectionText.swift`, the snippet picker's key handling).
//!
//! [`Engine`] is pure: it reads no clock (times are passed in), never sleeps and never calls a
//! slow OS function. It talks to the OS only through the [`crate::platform`] traits:
//! - in [`Engine::on_key`] (hook thread) it may call [`TextOutput`] and the cheap [`Layouts`]
//!   methods (`current_layout`, `switch_layout`);
//! - everything slow (UI Automation, clipboard, layout enumeration, waits) is queued as a [`Job`]
//!   that the runtime's worker thread runs ([`runtime`]) and feeds back as a [`JobResult`];
//! - deferred work (boundary conversion, pause fix, clipboard restore, ...) is a timer fired by
//!   [`Engine::tick`].
//!
//! Output sent outside the hook thread (timers, job results, UI commands) first checks that no
//! real key is inside the hook right now (`hook_busy`), and is retried a few milliseconds later
//! otherwise: a key held up in the hook would reach the app before our Backspaces.

mod hotkeys;
mod runtime;
#[cfg(test)]
mod tests;

pub use hotkeys::{HotkeyCheck, HotkeyTarget, HotkeyVerdict, active_clash, check_hotkey, is_bare_safe, same_trigger};
pub use runtime::{Clock, EngineHandle, Platform, SystemClock, start};

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::apps;
use crate::buffer::{WordBuffer, WordItem};
use crate::data;
use crate::detector::{Decision, Detector, Proposal, ProposalContext, clipboard_selection_refused};
use crate::double_space;
use crate::exceptions::Exceptions;
use crate::keymap::{Keymap, choose_layouts, classify_layout};
use crate::pair::{LanguagePair, RU_EN, Side, has_latin_letter};
use crate::platform::{
    AppId, ClipboardSnapshot, HookDecision, KeyCode, KeyEvent, KeyTable, LayoutId, LayoutInfo, Layouts, Modifiers,
    TextOutput,
};
use crate::resonance::AntiResonanceGuard;
use crate::settings::{AppMode, Hotkey, Settings, Sound};
use crate::snippets::{self, SnippetIndex};
use crate::text_case::{self, CaseToggleError};
use crate::typo::{self, TypoFixer};
use crate::undo::{UndoEvent, UndoLearner};

use hotkeys::{Gesture, Recorder, held_after, key_matches, needs_mask};

/// The language pair the engine corrects.
const PAIR: &LanguagePair = &RU_EN;

/// Boundary conversion runs this long after the boundary key …
const BOUNDARY_DELAY: Duration = Duration::from_millis(30);
/// … and once more this much later if a real key arrived within [`BOUNDARY_RECENT_KEY`].
const BOUNDARY_RETRY: Duration = Duration::from_millis(40);
const BOUNDARY_RECENT_KEY: Duration = Duration::from_millis(25);
/// The mid-word pause fix is checked this long after each printable key …
const PAUSE_FIX_DELAY: Duration = Duration::from_millis(150);
/// … and runs only if no key came for at least this long.
const PAUSE_FIX_IDLE: Duration = Duration::from_millis(140);
/// Automatic and manual output keep the engine "in flight" this long (`muteDrain`).
const MUTE_DRAIN: Duration = Duration::from_millis(80);
/// Selection rewrite and case toggle keep it in flight this long.
const SELECTION_MUTE: Duration = Duration::from_millis(300);
/// A manual hotkey queued during output is dropped after this.
const PENDING_MANUAL_TTL: Duration = Duration::from_secs(2);
/// After our own layout switch the OS may still report the old layout this long.
const OWN_SWITCH_GRACE: Duration = Duration::from_millis(800);
/// Our layout switch is verified (and re-sent once) this long after.
const LAYOUT_VERIFY: Duration = Duration::from_millis(60);
/// `dedupeChatter`: the same key again within this is chatter.
const CHATTER_WINDOW: Duration = Duration::from_millis(30);
/// Plain paste restores the clipboard after this.
const PLAIN_PASTE_SETTLE: Duration = Duration::from_millis(300);
/// The snippet picker closes itself after this.
const PICKER_IDLE: Duration = Duration::from_secs(30);
/// A click closes the picker after this, so a click on a picker row can still choose it.
const PICKER_CLICK_CLOSE: Duration = Duration::from_millis(250);
/// The cached password-field flag is re-read at a word start when older than this.
const PASSWORD_STALE: Duration = Duration::from_secs(1);
/// Output from outside the hook thread waits this long while a real key is in the hook.
const BUSY_RETRY: Duration = Duration::from_millis(5);
/// Live fix runs on words of at most this many characters.
const LIVE_MAX_LEN: usize = crate::detector::INLINE_LIVE_MAX_LEN;
/// Windows has no system-wide "double space inserts a period" for hardware keyboards.
const SYSTEM_DOUBLE_SPACE_PERIOD: bool = false;

/// Engine state for the UI (tray, settings).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineStatus {
    /// The input hook is installed and the engine processes keys.
    pub running: bool,
    /// Unix seconds; `None` when not paused.
    pub paused_until: Option<f64>,
    /// Automatic correction on (`autoEnabled`).
    pub auto_enabled: bool,
    /// Language code of the foreground layout, e.g. "en", "ru"; `None` if unknown.
    pub layout: Option<String>,
    /// False if the OS dropped the hook.
    pub hook_healthy: bool,
}

/// A short info toast.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Toast {
    /// i18n key, e.g. `toast.resumed`.
    pub key: String,
    /// Interpolation values.
    pub params: BTreeMap<String, String>,
}

/// What the engine tells the host. Delivered on the worker thread.
#[derive(Clone, Debug, PartialEq)]
pub enum EngineEvent {
    /// Settings changed (by the engine or a UI patch): persist this snapshot.
    SettingsChanged(Box<Settings>),
    StatusChanged(EngineStatus),
    /// `toast.pausedUntil` carries `until` (Unix seconds, as a string): the host formats it into
    /// the `time` parameter the UI string expects.
    Toast(Toast),
    /// Ask "Keep <word> as is?".
    LearnBanner { word: String },
    /// Show the picker with these `[name, text]` entries.
    SnippetPickerOpen { snippets: Vec<(String, String)> },
    SnippetPickerClose,
    /// While recording: a hotkey was pressed (plain Esc arrives as `Key { Escape, no mods }`).
    HotkeyCaptured(Hotkey),
    /// Play this conversion sound (never `Off`) at this volume (0..=1).
    PlaySound { sound: Sound, volume: f64 },
}

/// Slow work for the worker thread.
#[derive(Clone, Debug)]
pub(crate) enum Job {
    RefreshLayouts,
    ForegroundApp,
    RunningApps,
    CheckPassword(Then),
    ReadSelection(SelFor),
    PlainPaste,
    RestoreClipboard(ClipboardSnapshot, u64),
}

/// What to do after a password-field check.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Then {
    Nothing,
    Boundary { soft: bool },
    PauseFix,
    Manual,
}

/// Who asked for the selection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SelFor {
    Manual,
    Case,
}

/// A selection read (UI Automation, then the Ctrl+C fallback).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Selection {
    Text { text: String, via_clipboard: bool },
    Nothing,
    Password,
}

#[derive(Debug)]
pub(crate) enum JobResult {
    Layouts(Vec<(LayoutInfo, KeyTable)>),
    Foreground(Option<AppId>),
    RunningApps(Vec<AppId>),
    Password(bool, Then),
    Selection(SelFor, Selection),
    PlainPasted(Option<(ClipboardSnapshot, u64)>),
    Done,
}

#[derive(Clone, Debug)]
enum Timer {
    Boundary { soft: bool, delayed: bool },
    PauseFix,
    MutedEnd,
    LayoutVerify(LayoutId),
    RestoreClipboard(ClipboardSnapshot, u64),
    PickerClose,
    PauseExpiry,
    Retry(Deferred),
}

/// An action that sends output from outside the hook thread (retried while the hook is busy).
#[derive(Clone, Debug)]
enum Deferred {
    Boundary { soft: bool },
    PauseFix,
    Manual,
    Selection(SelFor, Selection),
    PickerChoose(usize),
}

fn len(s: &str) -> usize {
    s.chars().count()
}

fn is_mixed(s: &str) -> bool {
    PAIR.has_alt(s) && has_latin_letter(s)
}

fn flip_case(c: char) -> char {
    let mut it: Box<dyn Iterator<Item = char>> =
        if c.is_lowercase() { Box::new(c.to_uppercase()) } else { Box::new(c.to_lowercase()) };
    match (it.next(), it.next()) {
        (Some(x), None) => x,
        _ => c,
    }
}

fn side_of_lang(code: &str) -> Option<Side> {
    if code == PAIR.latin_lang {
        Some(Side::Latin)
    } else if code == PAIR.alt_lang {
        Some(Side::Alt)
    } else {
        None
    }
}

pub(crate) struct Engine {
    settings: Settings,
    exceptions: Exceptions,
    undo: UndoLearner,
    typo: TypoFixer,
    snippets: SnippetIndex,
    keymap: Keymap,
    buffer: WordBuffer,
    guard: AntiResonanceGuard,
    gestures: [Gesture; 4],
    recorder: Option<Recorder>,

    // Layouts
    layouts: Vec<LayoutInfo>,
    tables: HashMap<LayoutId, KeyTable>,
    sides: HashMap<LayoutId, Side>,
    /// (Latin, Alt) layouts the keymap is built from; `select_layout` switches to these.
    pair: Option<(LayoutId, LayoutId)>,
    /// The layout we believe the foreground window has.
    belief: Option<LayoutId>,
    own_switch_at: Option<Instant>,
    layouts_requested: bool,

    // Foreground app
    app: Option<AppId>,
    app_mode: Option<AppMode>,
    app_is_dev: bool,
    app_is_chromium: bool,
    forced_layout_app: Option<AppId>,

    // Password field (the Windows counterpart of macOS Secure Input)
    password: bool,
    password_checked_at: Option<Instant>,
    password_check_queued: bool,

    // Typing state
    live_fix_last: String,
    word_edited: bool,
    last_real_key_at: Option<Instant>,
    held: Modifiers,
    chatter: Option<(KeyCode, Instant)>,
    pending_context_clear: bool,
    context_reset_at: Option<Instant>,
    pending_soft_reset: bool,
    caret_jumped_since_clear: bool,
    backspace_since_jump: bool,

    // Output in flight (`muted`)
    muted_until: Option<Instant>,
    job_in_flight: bool,
    in_flight_real_keys: usize,
    pending_manual: Option<Instant>,
    last_text_fix: Option<(String, String)>,
    selection_refused: bool,
    picker: Option<Vec<(String, String)>>,

    // Plumbing
    output: Arc<dyn TextOutput>,
    layouts_api: Arc<dyn Layouts>,
    hook_busy: Arc<AtomicUsize>,
    timers: Vec<(Instant, Timer)>,
    jobs: VecDeque<Job>,
    events: Vec<EngineEvent>,
    /// Something for the worker: a job, a timer or an event.
    wake: bool,
    settings_dirty: bool,
    last_status: Option<EngineStatus>,
    running: bool,
    hook_healthy: bool,
    /// (instant, Unix seconds) at construction, to turn instants into wall time.
    anchor: (Instant, f64),
}

impl Engine {
    pub(crate) fn new(
        settings: Settings,
        output: Arc<dyn TextOutput>,
        layouts_api: Arc<dyn Layouts>,
        hook_busy: Arc<AtomicUsize>,
        now: Instant,
        unix_now: f64,
    ) -> Engine {
        let keymap = Keymap::fallback(PAIR);
        let mut e = Engine {
            exceptions: Exceptions::from_settings(&settings),
            undo: UndoLearner::from_settings(&settings),
            typo: TypoFixer::new(settings.typo_personal_words.clone()),
            snippets: SnippetIndex::new(&keymap, &settings.snippets_ordered),
            keymap,
            buffer: WordBuffer::new(),
            guard: AntiResonanceGuard::default(),
            gestures: Default::default(),
            recorder: None,
            layouts: Vec::new(),
            tables: HashMap::new(),
            sides: HashMap::new(),
            pair: None,
            belief: None,
            own_switch_at: None,
            layouts_requested: true,
            app: None,
            app_mode: None,
            app_is_dev: false,
            app_is_chromium: false,
            forced_layout_app: None,
            password: false,
            password_checked_at: None,
            password_check_queued: true,
            live_fix_last: String::new(),
            word_edited: false,
            last_real_key_at: None,
            held: Modifiers::NONE,
            chatter: None,
            pending_context_clear: false,
            context_reset_at: None,
            pending_soft_reset: false,
            caret_jumped_since_clear: false,
            backspace_since_jump: false,
            muted_until: None,
            job_in_flight: false,
            in_flight_real_keys: 0,
            pending_manual: None,
            last_text_fix: None,
            selection_refused: false,
            picker: None,
            output,
            layouts_api,
            hook_busy,
            timers: Vec::new(),
            jobs: VecDeque::from([
                Job::RefreshLayouts,
                Job::ForegroundApp,
                Job::RunningApps,
                Job::CheckPassword(Then::Nothing),
            ]),
            events: Vec::new(),
            wake: true,
            settings_dirty: false,
            last_status: None,
            running: false,
            hook_healthy: false,
            anchor: (now, unix_now),
            settings,
        };
        e.schedule_pause_expiry(now);
        e
    }

    // ---- Plumbing ----

    fn unix(&self, now: Instant) -> f64 {
        let (at, unix) = self.anchor;
        if now >= at { unix + (now - at).as_secs_f64() } else { unix - (at - now).as_secs_f64() }
    }

    fn is_paused(&self, now: Instant) -> bool {
        self.settings.paused_until > self.unix(now)
    }

    fn push_timer(&mut self, at: Instant, timer: Timer) {
        self.timers.push((at, timer));
        self.wake = true;
    }

    fn remove_timers(&mut self, pred: impl Fn(&Timer) -> bool) {
        self.timers.retain(|(_, t)| !pred(t));
    }

    fn push_job(&mut self, job: Job) {
        self.jobs.push_back(job);
        self.wake = true;
    }

    pub(crate) fn take_job(&mut self) -> Option<Job> {
        self.jobs.pop_front()
    }

    fn emit(&mut self, ev: EngineEvent) {
        self.events.push(ev);
        self.wake = true;
    }

    fn toast(&mut self, key: &str, params: &[(&str, String)]) {
        let params = params.iter().map(|(k, v)| (k.to_string(), v.clone())).collect();
        self.emit(EngineEvent::Toast(Toast { key: key.to_string(), params }));
    }

    fn mark_settings(&mut self) {
        self.settings_dirty = true;
        self.wake = true;
    }

    fn play_sound(&mut self) {
        let s = &self.settings;
        if s.sound != Sound::Off && s.sound_volume > 0.0 {
            let (sound, volume) = (s.sound, s.sound_volume.min(1.0));
            self.emit(EngineEvent::PlaySound { sound, volume });
        }
    }

    fn count_rescued(&mut self, n: u64) {
        self.settings.rescued_count += n;
        self.mark_settings();
    }

    /// Output in flight until `now + d` (`muted`).
    fn mute(&mut self, now: Instant, d: Duration) {
        let until = now + d;
        if self.muted_until.is_none_or(|t| t < until) {
            self.muted_until = Some(until);
            self.push_timer(until, Timer::MutedEnd);
        }
    }

    fn muted(&self, now: Instant) -> bool {
        self.job_in_flight || self.muted_until.is_some_and(|t| now < t)
    }

    pub(crate) fn take_events(&mut self) -> Vec<EngineEvent> {
        std::mem::take(&mut self.events)
    }

    pub(crate) fn wake(&self) -> bool {
        self.wake
    }

    pub(crate) fn clear_wake(&mut self) {
        self.wake = false;
    }

    pub(crate) fn next_deadline(&self) -> Option<Instant> {
        self.timers.iter().map(|(at, _)| *at).min()
    }

    pub(crate) fn settings(&self) -> &Settings {
        &self.settings
    }

    pub(crate) fn status(&self, now: Instant) -> EngineStatus {
        let paused = self.is_paused(now);
        EngineStatus {
            running: self.running,
            paused_until: paused.then_some(self.settings.paused_until),
            auto_enabled: self.settings.auto_enabled,
            layout: self
                .belief
                .as_ref()
                .and_then(|id| self.layouts.iter().find(|l| &l.id == id))
                .map(|l| l.lang.split('-').next().unwrap_or_default().to_lowercase()),
            hook_healthy: self.hook_healthy,
        }
    }

    pub(crate) fn set_hook_state(&mut self, running: bool, healthy: bool) {
        if (self.running, self.hook_healthy) != (running, healthy) {
            self.running = running;
            self.hook_healthy = healthy;
            self.wake = true;
        }
    }

    /// Emits pending SettingsChanged / StatusChanged.
    fn flush(&mut self, now: Instant) {
        if std::mem::take(&mut self.settings_dirty) {
            self.emit(EngineEvent::SettingsChanged(Box::new(self.settings.clone())));
        }
        let status = self.status(now);
        if self.last_status.as_ref() != Some(&status) {
            self.last_status = Some(status.clone());
            self.emit(EngineEvent::StatusChanged(status));
        }
    }

    fn worker_may_send(&self) -> bool {
        self.hook_busy.load(Ordering::SeqCst) == 0
    }

    fn detector(&self) -> Option<Detector<'_>> {
        Some(Detector { data: data::ready()?, keymap: &self.keymap, exceptions: &self.exceptions })
    }

    // ---- Layouts (`LayoutManager.swift`) ----

    fn within_own_grace(&self, now: Instant) -> bool {
        self.own_switch_at.is_some_and(|t| now.saturating_duration_since(t) < OWN_SWITCH_GRACE)
    }

    /// The layout we decode keys with: the OS's, unless our own switch is still settling.
    fn current_layout(&mut self, now: Instant) -> Option<LayoutId> {
        if !self.within_own_grace(now)
            && let Some(real) = self.layouts_api.current_layout()
            && self.belief.as_ref() != Some(&real)
        {
            self.accept_layout(real, true);
        }
        self.belief.clone()
    }

    /// The OS layout differs from our belief: take it. `user_choice`: the user switched it (not a
    /// window change), so remember it as the preferred layout of its script.
    fn accept_layout(&mut self, id: LayoutId, user_choice: bool) {
        if !self.tables.contains_key(&id) {
            self.request_layouts();
        }
        if user_choice && let Some(&side) = self.sides.get(&id) {
            self.remember(&id, side);
        }
        self.belief = Some(id);
        self.wake = true;
    }

    fn remember(&mut self, id: &LayoutId, side: Side) {
        let slot = match side {
            Side::Latin => &mut self.settings.last_layout_lat,
            Side::Alt => &mut self.settings.last_layout_cyr,
        };
        if *slot != id.0 {
            *slot = id.0.clone();
            self.mark_settings();
            self.rebuild_keymap();
        }
    }

    /// `reconcileWithReality`: true if the OS layout differed from our belief (now accepted).
    fn reconcile(&mut self, now: Instant) -> bool {
        if self.within_own_grace(now) {
            return false;
        }
        match self.layouts_api.current_layout() {
            Some(real) if self.belief.as_ref() != Some(&real) => {
                self.accept_layout(real, true);
                true
            }
            _ => false,
        }
    }

    fn request_layouts(&mut self) {
        if !self.layouts_requested {
            self.layouts_requested = true;
            self.push_job(Job::RefreshLayouts);
        }
    }

    fn on_layouts(&mut self, list: Vec<(LayoutInfo, KeyTable)>) {
        self.layouts_requested = false;
        self.layouts.clear();
        self.tables.clear();
        self.sides.clear();
        for (info, table) in list {
            if let Some(side) = classify_layout(PAIR, &table) {
                self.sides.insert(info.id.clone(), side);
            }
            self.tables.insert(info.id.clone(), table);
            self.layouts.push(info);
        }
        if self.belief.is_none() {
            self.belief = self.layouts_api.current_layout();
        }
        self.rebuild_keymap();
        self.wake = true;
    }

    /// Rebuilds the keymap (and the snippet index that depends on it) from the user's layouts,
    /// preferring the last-used layout of each script.
    fn rebuild_keymap(&mut self) {
        let candidates: Vec<(LayoutId, Side)> =
            self.layouts.iter().filter_map(|l| self.sides.get(&l.id).map(|s| (l.id.clone(), *s))).collect();
        self.pair = choose_layouts(&candidates, &self.settings.last_layout_lat, &self.settings.last_layout_cyr);
        self.keymap = self
            .pair
            .as_ref()
            .and_then(|(l, a)| Keymap::from_key_tables(self.tables.get(l)?, self.tables.get(a)?))
            .unwrap_or_else(|| Keymap::fallback(PAIR));
        self.snippets = SnippetIndex::new(&self.keymap, &self.settings.snippets_ordered);
    }

    fn current_side(&self) -> Option<Side> {
        self.belief.as_ref().and_then(|id| self.sides.get(id).copied())
    }

    fn switched(&mut self, id: LayoutId, now: Instant) {
        self.belief = Some(id.clone());
        self.own_switch_at = Some(now);
        self.push_timer(now + LAYOUT_VERIFY, Timer::LayoutVerify(id));
    }

    /// `selectLayout(cyrillic:)`: the paired layout of that script.
    fn select_layout(&mut self, side: Side, now: Instant) {
        let id = match (&self.pair, side) {
            (Some((l, _)), Side::Latin) => l.clone(),
            (Some((_, a)), Side::Alt) => a.clone(),
            (None, _) => match self.layouts.iter().find(|l| self.sides.get(&l.id) == Some(&side)) {
                Some(l) => l.id.clone(),
                None => return,
            },
        };
        let _ = self.layouts_api.switch_layout(&id);
        self.switched(id, now);
    }

    /// `cycleLayout`: the next installed layout in the OS order.
    fn cycle_layout(&mut self, now: Instant) -> bool {
        let ids: Vec<LayoutId> = self.layouts.iter().map(|l| l.id.clone()).collect();
        if ids.len() < 2 {
            return false;
        }
        let next = match self.belief.as_ref().and_then(|b| ids.iter().position(|i| i == b)) {
            Some(i) => ids[(i + 1) % ids.len()].clone(),
            None => ids[0].clone(),
        };
        if self.layouts_api.switch_layout(&next).is_err() {
            return false;
        }
        if let Some(&side) = self.sides.get(&next) {
            self.remember(&next, side);
        }
        self.switched(next, now);
        true
    }

    /// The character a key types in the layout we believe is active (no AltGr; Caps Lock flips
    /// the case of letters).
    fn char_for(&mut self, ev: &KeyEvent, now: Instant) -> Option<char> {
        let id = self.current_layout(now)?;
        let mut c = *self.tables.get(&id)?.get(&(ev.key, ev.mods.shift))?;
        if ev.caps_lock && c.is_alphabetic() {
            c = flip_case(c);
        }
        Some(c)
    }

    // ---- Foreground app and password field ----

    fn refresh_app_cache(&mut self) {
        let app = self.app.as_ref();
        self.app_mode = app.and_then(|a| apps::app_mode(&self.settings, a));
        self.app_is_dev = app.is_some_and(apps::is_dev_app);
        self.app_is_chromium = app.is_some_and(apps::is_chromium_family);
    }

    pub(crate) fn on_foreground_change(&mut self, app: AppId, now: Instant) {
        self.pending_soft_reset = true;
        // Every window has its own layout on Windows: re-read it.
        self.own_switch_at = None;
        if let Some(real) = self.layouts_api.current_layout()
            && self.belief.as_ref() != Some(&real)
        {
            self.accept_layout(real, false);
        }
        if apps::seed_default_apps(&mut self.settings, std::slice::from_ref(&app)) {
            self.mark_settings();
        }
        self.app = Some(app.clone());
        self.refresh_app_cache();
        // Forced layout, once per app switch.
        if self.forced_layout_app.as_ref() != Some(&app) {
            self.forced_layout_app = Some(app.clone());
            let want = self.settings.app_forced_layouts.get(&app.0).and_then(|code| side_of_lang(code));
            if let Some(side) = want
                && self.current_side() != Some(side)
            {
                self.select_layout(side, now);
            }
        }
        self.request_password_check();
        self.request_layouts();
    }

    fn request_password_check(&mut self) {
        if !self.password_check_queued {
            self.password_check_queued = true;
            self.push_job(Job::CheckPassword(Then::Nothing));
        }
    }

    fn password_stale(&self, now: Instant) -> bool {
        self.password_checked_at.is_none_or(|t| now.saturating_duration_since(t) > PASSWORD_STALE)
    }

    fn set_password(&mut self, password: bool, now: Instant) {
        self.password = password;
        self.password_checked_at = Some(now);
        self.password_check_queued = false;
        if password {
            self.buffer.clear();
            self.live_fix_last.clear();
        }
    }

    // ---- Key events (`EventTap.handle`) ----

    pub(crate) fn on_key(&mut self, ev: &KeyEvent) -> HookDecision {
        let now = ev.at;
        if ev.key.is_modifier() {
            let held = held_after(ev);
            self.held = held;
            if let Some(rec) = self.recorder.as_mut() {
                if !ev.repeat
                    && let Some(hk) = rec.modifier(ev.key, ev.down, held, now)
                {
                    self.emit(EngineEvent::HotkeyCaptured(hk));
                }
                return if ev.down { HookDecision::Swallow } else { HookDecision::Pass };
            }
            if !ev.repeat {
                self.modifier_gestures(ev.key, ev.down, held, now);
            }
            return HookDecision::Pass;
        }
        // Key-ups pass; the hook pairs them with swallowed key-downs.
        if !ev.down {
            return HookDecision::Pass;
        }
        if let Some(rec) = self.recorder.as_mut() {
            if let Some(hk) = rec.key_down(ev) {
                self.emit(EngineEvent::HotkeyCaptured(hk));
            }
            return HookDecision::Swallow;
        }
        self.key_down(ev, now)
    }

    /// Runs the modifier gestures. Called before the event is passed on, so a mask key sent here
    /// reaches the app before the Alt/Win key-up: when a gesture on Alt/Win fires, and when Alt/Win
    /// is released while such a chord is armed (it may fire on a later release).
    fn modifier_gestures(&mut self, key: KeyCode, down: bool, held: Modifiers, now: Instant) {
        let alt_win_up = !down && hotkeys::modifier_of(key).is_some_and(|m| m.alt || m.meta);
        let mut mask = false;
        let mut fired = Vec::new();
        for (i, target) in HotkeyTarget::ALL.into_iter().enumerate() {
            let Some(hk) = target.hotkey(&self.settings) else { continue };
            let armed = self.gestures[i].combo_armed();
            let fire = self.gestures[i].modifier(&hk, key, down, held, now);
            mask |= needs_mask(&hk) && (fire || (armed && alt_win_up));
            if fire {
                fired.push(target);
            }
        }
        if mask {
            let _ = self.output.send_mask_key();
        }
        for target in fired {
            self.fire(target, now);
        }
    }

    fn key_down(&mut self, ev: &KeyEvent, now: Instant) -> HookDecision {
        let shortcut = ev.mods.ctrl || ev.mods.alt || ev.mods.meta;
        if self.settings.dedupe_chatter && !ev.repeat && !shortcut {
            let chatter =
                self.chatter.is_some_and(|(k, t)| k == ev.key && now.saturating_duration_since(t) < CHATTER_WINDOW);
            self.chatter = Some((ev.key, now));
            if chatter {
                return HookDecision::Swallow;
            }
        }
        for g in &mut self.gestures {
            g.other_key();
        }
        // Snippet picker, then the key-mode hotkeys, in the original's order.
        if self.picker.is_some() {
            if ev.key == KeyCode::Escape {
                self.close_picker();
                return HookDecision::Swallow;
            }
            if let Some(index) = snippets::picker_index(ev.key, ev.mods) {
                self.pick_snippet(index);
                return HookDecision::Swallow;
            }
            self.close_picker();
        } else if let Some(hk) = self.settings.snippet_pick_hotkey
            && key_matches(&hk, ev)
            && self.open_picker(now)
        {
            return HookDecision::Swallow;
        }
        if self.settings.plain_paste && key_matches(&self.settings.plain_paste_hotkey, ev) {
            self.fire(HotkeyTarget::PlainPaste, now);
            return HookDecision::Swallow;
        }
        if let Some(hk) = self.settings.case_change_hotkey
            && key_matches(&hk, ev)
        {
            if !ev.repeat {
                self.fire(HotkeyTarget::CaseChange, now);
            }
            return HookDecision::Swallow;
        }
        if key_matches(&self.settings.hotkey, ev) {
            self.fire(HotkeyTarget::Manual, now);
            return HookDecision::Swallow;
        }
        self.typing_key(ev, now)
    }

    fn fire(&mut self, target: HotkeyTarget, now: Instant) {
        match target {
            HotkeyTarget::Manual => self.push_job(Job::CheckPassword(Then::Manual)),
            HotkeyTarget::CaseChange => {
                if self.muted(now) {
                    return;
                }
                self.live_fix_last.clear();
                self.start_selection_read(SelFor::Case);
            }
            HotkeyTarget::PlainPaste => self.push_job(Job::PlainPaste),
            HotkeyTarget::SnippetPick => {
                self.open_picker(now);
            }
        }
    }

    fn start_selection_read(&mut self, purpose: SelFor) {
        self.job_in_flight = true;
        self.in_flight_real_keys = 0;
        self.push_job(Job::ReadSelection(purpose));
    }

    fn clear_typing(&mut self) {
        self.live_fix_last.clear();
        self.word_edited = false;
        self.buffer.clear();
    }

    /// `Engine.handleKeyDown`: one non-modifier key-down that is not a hotkey.
    fn typing_key(&mut self, ev: &KeyEvent, now: Instant) -> HookDecision {
        use KeyCode::*;
        if self.job_in_flight {
            self.in_flight_real_keys += 1;
        }
        if self.password {
            self.live_fix_last.clear();
            self.buffer.clear();
            self.pending_context_clear = false;
            if self.password_stale(now) {
                self.request_password_check();
            }
            return HookDecision::Pass;
        }
        if !self.apply_pending_context_clear() && self.pending_soft_reset {
            self.buffer.soft_context_reset();
            self.guard.reset_history();
            self.pending_soft_reset = false;
        }
        let shortcut = ev.mods.ctrl || ev.mods.alt || ev.mods.meta;
        match ev.key {
            Backspace => {
                self.live_fix_last.clear();
                if shortcut {
                    // Ctrl+Backspace deletes a word: the context is gone.
                    self.buffer.clear();
                    self.word_edited = false;
                    self.undo.reset_context();
                } else {
                    self.buffer.backspace(now);
                    self.word_edited = true;
                    if self.caret_jumped_since_clear {
                        self.backspace_since_jump = true;
                    }
                    self.undo_observe(now);
                }
            }
            Space | Tab | Enter | NumpadEnter => {
                if shortcut {
                    // Alt+Tab, Ctrl+Enter, Win+Space: a shortcut, not a word boundary.
                    self.clear_typing();
                    return HookDecision::Pass;
                }
                return self.boundary_key(ev, now);
            }
            ArrowLeft | ArrowRight | ArrowUp | ArrowDown => {
                self.live_fix_last.clear();
                self.word_edited = false;
                self.buffer.invalidate_group_history();
                if ev.mods.shift || self.settings.arrows_cancel {
                    self.buffer.clear();
                    self.undo.reset_context();
                }
            }
            Escape | Delete | Insert | Home | End | PageUp | PageDown => {
                self.clear_typing();
                self.undo.reset_context();
            }
            _ => {
                if shortcut {
                    self.clear_typing();
                    return HookDecision::Pass;
                }
                let Some(ch) = self.char_for(ev, now) else {
                    // Numpad keys type something we cannot model: the word on screen changed.
                    if (Numpad0..=NumpadDivide).contains(&ev.key) {
                        self.clear_typing();
                    }
                    return HookDecision::Pass;
                };
                if (ch as u32) < 0x20 || ch == '\u{7f}' {
                    return HookDecision::Pass;
                }
                self.last_real_key_at = Some(now);
                if self.try_inline_live_fix(ch, ev, now) {
                    return HookDecision::Swallow;
                }
                let word_start = self.buffer.current_word().is_empty();
                self.buffer.append(&ch.to_string(), now);
                self.undo_observe(now);
                let s = &self.settings;
                if s.live_fix_enabled && s.auto_enabled && !self.is_paused(now) {
                    self.remove_timers(|t| matches!(t, Timer::PauseFix));
                    self.push_timer(now + PAUSE_FIX_DELAY, Timer::PauseFix);
                }
                if word_start && self.password_stale(now) {
                    self.request_password_check();
                }
            }
        }
        HookDecision::Pass
    }

    /// Space, Tab or Enter (`handleKeyDown`, boundary case).
    fn boundary_key(&mut self, ev: &KeyEvent, now: Instant) -> HookDecision {
        self.live_fix_last.clear();
        self.word_edited = false;
        self.last_real_key_at = Some(now);
        let (ws, space, tab, enter) = match ev.key {
            KeyCode::Space => (" ", true, false, false),
            KeyCode::Tab => ("\t", false, true, false),
            _ => ("\n", false, false, true),
        };
        let s = &self.settings;
        let snip_key = (space && s.snippet_expand_space) || (enter && s.snippet_expand_enter) || (tab && s.snippet_expand_tab);
        let dev_off = s.developer_mode && self.app_is_dev;
        let snip_allowed = self.app_mode != Some(AppMode::Off) && !dev_off;
        if snip_key && snip_allowed && !self.muted(now) && !self.is_paused(now) && !self.buffer.current_word().is_empty() {
            let typed = self.buffer.current_word().to_string();
            if let Some(expansion) = self.snippets.expansion_for_typed(&self.keymap, &typed).map(str::to_string)
                && self.expand_snippet(&typed, &expansion, ws, now)
            {
                return HookDecision::Swallow;
            }
        }
        if enter && self.convert_before_return(ev, now) {
            return HookDecision::Swallow;
        }
        self.buffer.boundary(ws, now);
        let s = &self.settings;
        let auto = ((space && s.trigger_space) || (enter && s.trigger_enter) || (tab && s.trigger_tab))
            && !dev_off
            && self.app_mode != Some(AppMode::Off);
        if auto {
            let soft = self.app_mode == Some(AppMode::Soft);
            self.push_timer(now + BOUNDARY_DELAY, Timer::Boundary { soft, delayed: false });
        }
        if tab || enter {
            self.request_password_check();
        }
        HookDecision::Pass
    }

    /// `expandSnippet`: replaces the trigger with the expansion plus the swallowed boundary.
    fn expand_snippet(&mut self, trigger: &str, expansion: &str, ws: &str, now: Instant) -> bool {
        let (delete, text) = snippets::expansion_output(trigger, expansion, ws);
        if self.output.replace_text(delete, &text).is_err() {
            return false;
        }
        self.buffer.commit_snippet(expansion, ws, now);
        self.play_sound();
        true
    }

    /// `applyPendingContextClear`: the first key after a click starts a fresh context.
    fn apply_pending_context_clear(&mut self) -> bool {
        if !self.pending_context_clear {
            return false;
        }
        if self.last_real_key_at.is_some() && self.last_real_key_at > self.context_reset_at {
            self.pending_context_clear = false;
            self.pending_soft_reset = true;
            return false;
        }
        self.live_fix_last.clear();
        self.caret_jumped_since_clear = true;
        self.backspace_since_jump = false;
        self.buffer.clear();
        self.undo.reset_context();
        self.guard.reset_history();
        self.pending_context_clear = false;
        self.pending_soft_reset = false;
        true
    }

    fn undo_observe(&mut self, now: Instant) {
        let unix = self.unix(now);
        if let Some(ev) = self.undo.observe(self.buffer.current_word(), unix, &self.exceptions.learned) {
            self.on_undo_event(ev);
        }
    }

    fn on_undo_event(&mut self, ev: UndoEvent) {
        if let Some(word) = ev.ask {
            self.emit(EngineEvent::LearnBanner { word });
        }
        self.undo.write_to(&mut self.settings);
        self.mark_settings();
    }

    pub(crate) fn on_mouse_down(&mut self, now: Instant) {
        for g in &mut self.gestures {
            g.mouse();
        }
        if self.picker.is_some() {
            self.remove_timers(|t| matches!(t, Timer::PickerClose));
            self.push_timer(now + PICKER_CLICK_CLOSE, Timer::PickerClose);
        }
        self.context_reset_at = Some(now);
        self.buffer.invalidate_group_history();
        self.pending_context_clear = true;
        self.request_password_check();
    }

    // ---- Live fix ----

    /// Gates shared by both live-fix paths (`tryInlineLiveFix`, `handlePauseFixMarker`).
    fn live_fix_allowed(&self, now: Instant) -> bool {
        let s = &self.settings;
        s.live_fix_enabled
            && s.auto_enabled
            && !self.is_paused(now)
            && data::ready().is_some()
            && !self.muted(now)
            && !self.word_edited
            && !(s.developer_mode && self.app_is_dev)
            && self.app_mode.is_none()
            && !self.app_is_chromium
            && !self.password
    }

    /// `tryInlineLiveFix`: on the key that completes an impossible word, replace it atomically
    /// inside the hook and swallow the key.
    fn try_inline_live_fix(&mut self, ch: char, ev: &KeyEvent, now: Instant) -> bool {
        if !self.settings.inline_live_fix || !self.live_fix_allowed(now) || !ev.mods.is_empty() {
            return false;
        }
        let on_screen = self.buffer.current_word().to_string();
        let candidate = format!("{on_screen}{ch}");
        let n = len(&candidate);
        if !(crate::detector::LIVE_MIN_LEN..=LIVE_MAX_LEN).contains(&n) || candidate == self.live_fix_last || is_mixed(&candidate) {
            return false;
        }
        let Some(det) = self.detector() else { return false };
        let Decision::Convert(to) = det.live_decide(&candidate) else { return false };
        let Some(converted) = det.live_convert(&candidate, to) else { return false };
        if converted == candidate || !self.long_enough(&converted) {
            return false;
        }
        if !self.guard.allow(&candidate, &converted, now) {
            self.live_fix_last.clear();
            self.buffer.clear();
            return false;
        }
        let unix = self.unix(now);
        if self.undo.should_suppress(&candidate, unix) || self.undo.is_session_protected(&candidate) {
            return false;
        }
        if self.output.replace_text(len(&on_screen), &converted).is_err() {
            return false;
        }
        self.buffer.append(&ch.to_string(), now);
        self.buffer.apply_conversion(&converted);
        self.live_fix_last = converted.clone();
        self.select_layout(to, now);
        self.undo.note_conversion(&candidate, &converted, unix, &self.exceptions.learned);
        self.count_rescued(1);
        self.play_sound();
        true
    }

    /// The pause-fix gates that do not depend on a fresh password check.
    fn pause_fix_word(&self, now: Instant) -> Option<String> {
        if !self.live_fix_allowed(now) || !self.held.is_empty() {
            return None;
        }
        if self.last_real_key_at.is_none_or(|t| now.saturating_duration_since(t) < PAUSE_FIX_IDLE) {
            return None;
        }
        let word = self.buffer.current_word();
        let n = len(word);
        ((crate::detector::LIVE_MIN_LEN..=LIVE_MAX_LEN).contains(&n) && word != self.live_fix_last)
            .then(|| word.to_string())
    }

    /// `handlePauseFixMarker`: the user stopped mid-word; fix an impossible word, or heal the
    /// raw Latin tail of a word we fixed earlier.
    fn pause_fix(&mut self, now: Instant) {
        let Some(word) = self.pause_fix_word(now) else { return };
        let Some(det) = self.detector() else { return };
        if is_mixed(&word) {
            let Some(heal) = det.live_heal(&word, &self.live_fix_last) else { return };
            if !self.guard.allow(&word, &heal.word, now) {
                self.live_fix_last.clear();
                self.buffer.clear();
                return;
            }
            if self.output.replace_text(heal.delete, &heal.text).is_err() {
                return;
            }
            self.buffer.apply_conversion(&heal.word);
            self.live_fix_last = heal.word;
            self.select_layout(Side::Alt, now);
            self.mute(now, MUTE_DRAIN);
            self.play_sound();
            return;
        }
        let Decision::Convert(to) = det.live_decide(&word) else { return };
        let Some(converted) = det.live_convert(&word, to) else { return };
        if converted == word || !self.long_enough(&converted) {
            return;
        }
        if !self.guard.allow(&word, &converted, now) {
            self.live_fix_last.clear();
            self.buffer.clear();
            return;
        }
        let unix = self.unix(now);
        if self.undo.should_suppress(&word, unix) || self.undo.is_session_protected(&word) {
            return;
        }
        if self.output.replace_text(len(&word), &converted).is_err() {
            return;
        }
        self.buffer.apply_conversion(&converted);
        self.live_fix_last = converted.clone();
        self.select_layout(to, now);
        self.undo.note_conversion(&word, &converted, unix, &self.exceptions.learned);
        self.count_rescued(1);
        self.mute(now, MUTE_DRAIN);
        self.play_sound();
    }

    // ---- Automatic conversion ----

    /// `autoMinLetters`: an automatic fix needs at least this many letters in the fixed word.
    fn long_enough(&self, fixed: &str) -> bool {
        fixed.chars().filter(|c| c.is_alphabetic()).count() >= self.settings.auto_min_letters as usize
    }

    /// `autoConversionProposal`.
    fn auto_proposal(&mut self, word: &str, soft: bool, completed: bool) -> Option<Proposal> {
        data::ready()?;
        self.current_side()?;
        if self.undo.is_session_protected(word) {
            return None;
        }
        let for_current = !completed && !self.buffer.current_word().is_empty();
        let mut after_jump = std::mem::take(&mut self.caret_jumped_since_clear);
        // No caret probe on Windows: a single letter after a click is "start of input" unless the
        // user erased something first.
        if after_jump && len(word) == 1 && !self.backspace_since_jump {
            after_jump = false;
        }
        let gap = if completed || self.buffer.current_word().is_empty() {
            self.buffer.last_word_gap()
        } else {
            self.buffer.current_word_gap()
        };
        let det = self.detector()?;
        let ctx = ProposalContext {
            prev: self.buffer.context_word(for_current),
            earlier: self.buffer.earlier_context_word(for_current),
            after_caret_jump: after_jump,
            soft,
            intra_word_gap: gap,
            session_protected: false,
        };
        let prop = det.propose(word, &ctx)?;
        self.long_enough(&prop.text).then_some(prop)
    }

    /// `convertFromBuffer(manual: false)`: the finished word at a boundary.
    fn convert_auto(&mut self, soft: bool, now: Instant) {
        if self.muted(now) || self.password {
            return;
        }
        let Some(item) = self.buffer.word_for_conversion(true) else { return };
        let word = item.word.clone();
        let Some(prop) = self.auto_proposal(&word, soft, true) else {
            self.typo_fixes(&word, &item, now);
            return;
        };
        let converted = prop.text;
        if converted == word {
            return;
        }
        if !self.guard.allow(&word, &converted, now) {
            self.live_fix_last.clear();
            self.buffer.clear();
            return;
        }
        let gap = self.buffer.last_tail_double_space_gap();
        let rewrite = |tail: &str| {
            double_space::rewritten_tail(tail, &converted, SYSTEM_DOUBLE_SPACE_PERIOD, gap, double_space::MAX_GAP)
        };
        let tail = rewrite(&item.tail);
        let completed_tail = rewrite(self.buffer.last_tail());
        if self.output.replace_text(item.delete_count, &format!("{converted}{tail}")).is_err() {
            return;
        }
        self.buffer.apply_completed_conversion(&converted);
        if tail != item.tail {
            self.buffer.apply_completed_tail(&completed_tail);
        }
        if !prop.rescue {
            let unix = self.unix(now);
            self.undo.note_conversion(&word, &converted, unix, &self.exceptions.learned);
        }
        self.count_rescued(1);
        self.select_layout(prop.to, now);
        self.mute(now, MUTE_DRAIN);
        self.play_sound();
    }

    /// Two capitals, Caps Lock word, typo table/mechanical (`fixTwoLeadingCaps`, `fixTypo`).
    fn typo_fixes(&mut self, word: &str, item: &WordItem, now: Instant) {
        let Some(data) = data::ready() else { return };
        if self.settings.two_caps_fix
            && let Some(fixed) = typo::fix_two_leading_caps(PAIR, word, &self.exceptions)
            && self.output.replace_text(item.delete_count, &format!("{fixed}{}", item.tail)).is_ok()
        {
            self.buffer.apply_completed_conversion(&fixed);
            self.mute(now, MUTE_DRAIN);
        }
        if self.settings.typo_fix
            && let Some(fixed) = typo::fix_caps_lock_word(PAIR, word, &self.exceptions)
        {
            self.apply_text_fix(word, &fixed, item, now);
            return;
        }
        self.typo.note_typed(data, word);
        if self.typo.personal != self.settings.typo_personal_words {
            self.settings.typo_personal_words = self.typo.personal.clone();
            self.mark_settings();
        }
        if !self.settings.typo_fix {
            return;
        }
        let (exc, undo) = (&self.exceptions, &self.undo);
        let protected = |w: &str| exc.is_kept(w) || undo.is_session_protected(w);
        if let Some(fixed) = self.typo.suggest(data, word, &protected) {
            self.apply_text_fix(word, &fixed, item, now);
        }
    }

    fn apply_text_fix(&mut self, word: &str, fixed: &str, item: &WordItem, now: Instant) {
        if self.output.replace_text(item.delete_count, &format!("{fixed}{}", item.tail)).is_err() {
            return;
        }
        let unix = self.unix(now);
        self.undo.note_conversion(word, fixed, unix, &self.exceptions.learned);
        self.last_text_fix = Some((word.to_string(), fixed.to_string()));
        self.buffer.apply_completed_conversion(fixed);
        self.mute(now, MUTE_DRAIN);
    }

    /// `convertBeforeReturn`: convert (or typo-fix) the pending word, then send Enter ourselves.
    fn convert_before_return(&mut self, ev: &KeyEvent, now: Instant) -> bool {
        let Some(data) = data::ready() else { return false };
        let s = &self.settings;
        if !(s.enter_pre_convert && s.auto_enabled && s.trigger_enter) || self.muted(now) || self.is_paused(now) {
            return false;
        }
        if !ev.mods.is_empty() {
            return false;
        }
        let word = self.buffer.current_word().to_string();
        if word.is_empty() {
            return false;
        }
        if self.reconcile(now) {
            self.live_fix_last.clear();
            self.buffer.clear();
            return false;
        }
        let s = &self.settings;
        if (s.developer_mode && self.app_is_dev) || self.app_mode == Some(AppMode::Off) {
            return false;
        }
        let typo_on = s.typo_fix;
        let soft = self.app_mode == Some(AppMode::Soft);
        let protected_fix = |e: &Engine, numeric: bool| -> Option<String> {
            let protected = |w: &str| e.exceptions.is_kept(w) || e.undo.is_session_protected(w);
            if numeric {
                e.typo.numeric_suggestion(data, &word, &protected)
            } else {
                e.typo.curated_suggestion(data, &word, &protected)
            }
        };
        if typo_on && let Some(fixed) = protected_fix(self, true) {
            return self.typo_before_return(&word, &fixed, now);
        }
        let prop = self.auto_proposal(&word, soft, false);
        if prop.is_none()
            && typo_on
            && let Some(fixed) = protected_fix(self, false)
        {
            return self.typo_before_return(&word, &fixed, now);
        }
        let Some(prop) = prop else { return false };
        if !self.guard.allow(&word, &prop.text, now) {
            self.live_fix_last.clear();
            self.buffer.clear();
            return false;
        }
        if self.output.replace_text(len(&word), &format!("{}\n", prop.text)).is_err() {
            return false;
        }
        self.buffer.apply_conversion(&prop.text);
        self.buffer.boundary("\n", now);
        if !prop.rescue {
            let unix = self.unix(now);
            self.undo.note_conversion(&word, &prop.text, unix, &self.exceptions.learned);
        }
        self.select_layout(prop.to, now);
        self.count_rescued(1);
        self.play_sound();
        self.request_password_check();
        true
    }

    fn typo_before_return(&mut self, word: &str, fixed: &str, now: Instant) -> bool {
        if self.output.replace_text(len(word), &format!("{fixed}\n")).is_err() {
            return false;
        }
        self.last_text_fix = None;
        self.buffer.apply_conversion(fixed);
        self.buffer.boundary("\n", now);
        self.request_password_check();
        true
    }

    // ---- Manual hotkey (`convertFromBuffer(manual: true)`) ----

    /// After the password check: undo a text fix, else read the selection if there is no word,
    /// else convert the word.
    fn manual_start(&mut self, now: Instant) {
        if self.muted(now) {
            self.pending_manual = Some(now);
            return;
        }
        self.apply_pending_context_clear();
        self.live_fix_last.clear();
        if self.undo_last_text_fix(now) {
            return;
        }
        self.selection_refused = false;
        if self.buffer.word_for_conversion(false).is_none() && !self.password {
            self.job_in_flight = true;
            self.in_flight_real_keys = 0;
            self.jobs.push_front(Job::ReadSelection(SelFor::Manual));
            self.wake = true;
            return;
        }
        self.manual_word(now);
    }

    fn manual_selection(&mut self, sel: Selection, now: Instant) {
        if self.convert_selection(sel, now) {
            return;
        }
        self.manual_word(now);
    }

    /// `convertSelection`: true when the selection was handled.
    fn convert_selection(&mut self, sel: Selection, now: Instant) -> bool {
        let Selection::Text { text, via_clipboard } = sel else { return false };
        if via_clipboard && clipboard_selection_refused(&text) {
            self.selection_refused = true;
            return false;
        }
        let to = if PAIR.has_alt(&text) {
            Side::Latin
        } else if has_latin_letter(&text) {
            Side::Alt
        } else {
            match self.keymap.symbol_direction(&text) {
                Some(side) => side,
                None => return false,
            }
        };
        let converted = self.keymap.convert(&text, to);
        if converted == text {
            return false;
        }
        if self.output.replace_text(0, &converted).is_err() {
            return true;
        }
        self.select_layout(to, now);
        self.count_rescued(text.split(' ').filter(|w| !w.is_empty()).count().max(1) as u64);
        self.buffer.clear();
        self.mute(now, SELECTION_MUTE);
        self.play_sound();
        true
    }

    /// Group, then the current or last word, else cycle the layout.
    fn manual_word(&mut self, now: Instant) {
        if self.settings.group_convert && !self.settings.auto_enabled && self.convert_group(now) {
            return;
        }
        let Some(item) = self.buffer.word_for_conversion(false) else {
            // A refused selection existed: touch nothing, not even the layout.
            if std::mem::take(&mut self.selection_refused) {
                return;
            }
            if !self.cycle_layout(now) {
                self.toast("toast.nothingToConvert", &[]);
            }
            return;
        };
        let word = item.word.clone();
        let to = if PAIR.has_alt(&word) {
            Side::Latin
        } else if has_latin_letter(&word) {
            Side::Alt
        } else if self.current_side() == Some(Side::Alt) {
            Side::Latin
        } else {
            Side::Alt
        };
        let converted = self.keymap.convert(&word, to);
        if converted == word {
            self.toast("toast.nothingToConvert", &[]);
            return;
        }
        if self.output.replace_text(item.delete_count, &format!("{converted}{}", item.tail)).is_err() {
            return;
        }
        self.buffer.apply_conversion(&converted);
        let unix = self.unix(now);
        if let Some(ev) = self.undo.note_manual_convert(&word, &converted, unix, &self.exceptions.learned) {
            self.on_undo_event(ev);
        }
        self.undo.protect(&converted);
        self.count_rescued(1);
        self.select_layout(to, now);
        self.mute(now, MUTE_DRAIN);
        self.play_sound();
    }

    /// `convertGroup`: false only when there is no group.
    fn convert_group(&mut self, now: Instant) -> bool {
        let Some(group) = self.buffer.group_for_conversion(now) else { return false };
        let Some(plan) = self.detector().and_then(|d| d.plan_group(&group.words)) else { return true };
        if self.output.replace_text(group.delete_count, &plan.text).is_err() {
            return true;
        }
        self.count_rescued(plan.converted as u64);
        self.buffer.clear();
        self.select_layout(plan.last_to, now);
        self.mute(now, MUTE_DRAIN);
        self.play_sound();
        true
    }

    /// `undoLastTextFix`: the manual hotkey right after a typo/caps fix puts the original back.
    fn undo_last_text_fix(&mut self, now: Instant) -> bool {
        let Some((original, fixed)) = self.last_text_fix.clone() else { return false };
        let Some(item) = self.buffer.word_for_conversion(false) else { return false };
        if item.word != fixed {
            self.last_text_fix = None;
            return false;
        }
        if self.output.replace_text(item.delete_count, &format!("{original}{}", item.tail)).is_err() {
            return false;
        }
        let unix = self.unix(now);
        if let Some(ev) = self.undo.note_manual_convert(&fixed, &original, unix, &self.exceptions.learned) {
            self.on_undo_event(ev);
        }
        self.undo.protect(&original);
        self.last_text_fix = None;
        self.buffer.apply_conversion(&original);
        self.mute(now, MUTE_DRAIN);
        self.play_sound();
        true
    }

    /// `changeSelectionCase`.
    fn case_toggle(&mut self, sel: Selection, now: Instant) {
        let Selection::Text { text, .. } = sel else {
            self.toast("toast.noSelection", &[]);
            return;
        };
        match text_case::toggle_case(&text) {
            Err(CaseToggleError::TooLong) => {
                self.toast("toast.caseTooLong", &[("max", text_case::MAX_CHARS.to_string())]);
            }
            Err(CaseToggleError::NoLetters) => self.toast("toast.nothingToConvert", &[]),
            Ok(changed) => {
                if self.output.replace_text(0, &changed).is_ok() {
                    self.buffer.clear();
                    self.mute(now, SELECTION_MUTE);
                    self.play_sound();
                }
            }
        }
    }

    // ---- Snippet picker ----

    fn open_picker(&mut self, now: Instant) -> bool {
        if self.settings.text_snippets.is_empty() {
            self.toast("toast.snippetsEmpty", &[]);
            return false;
        }
        let list = self.settings.text_snippets.clone();
        self.picker = Some(list.clone());
        self.remove_timers(|t| matches!(t, Timer::PickerClose));
        self.push_timer(now + PICKER_IDLE, Timer::PickerClose);
        self.emit(EngineEvent::SnippetPickerOpen { snippets: list });
        true
    }

    pub(crate) fn close_picker(&mut self) {
        self.remove_timers(|t| matches!(t, Timer::PickerClose));
        if self.picker.take().is_some() {
            self.emit(EngineEvent::SnippetPickerClose);
        }
    }

    /// Inserts entry `index` and closes the picker; an index past the list does nothing.
    fn pick_snippet(&mut self, index: usize) {
        let Some(text) = self.picker.as_ref().and_then(|p| p.get(index)).map(|e| e.1.clone()) else { return };
        self.close_picker();
        if text.is_empty() {
            return;
        }
        self.buffer.clear();
        self.live_fix_last.clear();
        let _ = self.output.replace_text(0, &text);
    }

    // ---- Timers and jobs ----

    /// Fires due timers and emits pending SettingsChanged / StatusChanged.
    pub(crate) fn tick(&mut self, now: Instant) {
        while let Some(i) = self
            .timers
            .iter()
            .enumerate()
            .filter(|(_, (at, _))| *at <= now)
            .min_by_key(|(_, (at, _))| *at)
            .map(|(i, _)| i)
        {
            let (_, timer) = self.timers.swap_remove(i);
            self.fire_timer(timer, now);
        }
        if let Some(at) = self.pending_manual
            && !self.muted(now)
        {
            self.pending_manual = None;
            if now.saturating_duration_since(at) <= PENDING_MANUAL_TTL {
                self.push_job(Job::CheckPassword(Then::Manual));
            }
        }
        self.flush(now);
    }

    fn fire_timer(&mut self, timer: Timer, now: Instant) {
        match timer {
            Timer::Boundary { soft, delayed } => {
                if self.reconcile(now) {
                    // The word was decoded with the wrong layout.
                    self.live_fix_last.clear();
                    self.buffer.clear();
                    return;
                }
                if !self.settings.auto_enabled || self.is_paused(now) {
                    return;
                }
                if !delayed && self.last_real_key_at.is_some_and(|t| now.saturating_duration_since(t) < BOUNDARY_RECENT_KEY) {
                    self.push_timer(now + BOUNDARY_RETRY, Timer::Boundary { soft, delayed: true });
                    return;
                }
                self.push_job(Job::CheckPassword(Then::Boundary { soft }));
            }
            Timer::PauseFix => {
                if self.pause_fix_word(now).is_some() {
                    self.push_job(Job::CheckPassword(Then::PauseFix));
                }
            }
            Timer::MutedEnd | Timer::PauseExpiry => {}
            Timer::LayoutVerify(id) => {
                if self.belief.as_ref() == Some(&id) && self.layouts_api.current_layout().as_ref() != Some(&id) {
                    let _ = self.layouts_api.switch_layout(&id);
                }
            }
            Timer::RestoreClipboard(snapshot, ours) => self.push_job(Job::RestoreClipboard(snapshot, ours)),
            Timer::PickerClose => self.close_picker(),
            Timer::Retry(d) => self.run_deferred(d, now),
        }
    }

    pub(crate) fn on_job_result(&mut self, result: JobResult, now: Instant) {
        match result {
            JobResult::Layouts(list) => self.on_layouts(list),
            JobResult::Foreground(app) => {
                if let Some(app) = app {
                    self.on_foreground_change(app, now);
                }
            }
            JobResult::RunningApps(apps) => {
                if apps::seed_default_apps(&mut self.settings, &apps) {
                    self.refresh_app_cache();
                    self.mark_settings();
                }
            }
            JobResult::Password(password, then) => {
                self.set_password(password, now);
                let d = match then {
                    Then::Nothing => return,
                    Then::Boundary { soft } => Deferred::Boundary { soft },
                    Then::PauseFix => Deferred::PauseFix,
                    Then::Manual => Deferred::Manual,
                };
                self.run_deferred(d, now);
            }
            JobResult::Selection(purpose, sel) => {
                self.job_in_flight = false;
                // Keys typed while we read the selection replaced it: do not write over them.
                if std::mem::take(&mut self.in_flight_real_keys) > 0 {
                    return;
                }
                self.run_deferred(Deferred::Selection(purpose, sel), now);
            }
            JobResult::PlainPasted(Some((snapshot, ours))) => {
                self.push_timer(now + PLAIN_PASTE_SETTLE, Timer::RestoreClipboard(snapshot, ours));
            }
            JobResult::PlainPasted(None) | JobResult::Done => {}
        }
        self.wake = true;
    }

    /// Runs an action that sends output from outside the hook thread, or retries it shortly if a
    /// real key is inside the hook right now.
    fn run_deferred(&mut self, d: Deferred, now: Instant) {
        if !self.worker_may_send() {
            if let Deferred::Selection(..) = d {
                self.job_in_flight = true;
            }
            self.push_timer(now + BUSY_RETRY, Timer::Retry(d));
            return;
        }
        match d {
            Deferred::Boundary { soft } => self.convert_auto(soft, now),
            Deferred::PauseFix => {
                if !self.password {
                    self.pause_fix(now);
                }
            }
            Deferred::Manual => self.manual_start(now),
            Deferred::Selection(purpose, sel) => {
                self.job_in_flight = false;
                match purpose {
                    SelFor::Manual => self.manual_selection(sel, now),
                    SelFor::Case => self.case_toggle(sel, now),
                }
            }
            Deferred::PickerChoose(index) => self.pick_snippet(index),
        }
    }

    // ---- Commands from the host ----

    fn schedule_pause_expiry(&mut self, now: Instant) {
        self.remove_timers(|t| matches!(t, Timer::PauseExpiry));
        let left = self.settings.paused_until - self.unix(now);
        if left > 0.0 {
            self.push_timer(now + Duration::from_secs_f64(left + 0.5), Timer::PauseExpiry);
        }
    }

    pub(crate) fn pause(&mut self, minutes: u32, now: Instant) {
        self.settings.pause_minutes = minutes;
        self.settings.paused_until = self.unix(now) + f64::from(minutes) * 60.0;
        self.schedule_pause_expiry(now);
        self.mark_settings();
        let until = format!("{}", self.settings.paused_until.round() as i64);
        self.toast("toast.pausedUntil", &[("until", until)]);
    }

    pub(crate) fn resume(&mut self, now: Instant) {
        if self.settings.paused_until == 0.0 {
            return;
        }
        self.settings.paused_until = 0.0;
        self.schedule_pause_expiry(now);
        self.mark_settings();
        self.toast("toast.resumed", &[]);
    }

    pub(crate) fn set_auto(&mut self, on: bool) {
        self.settings.auto_enabled = on;
        self.mark_settings();
        self.toast(if on { "toast.autoOn" } else { "toast.autoOff" }, &[]);
    }

    pub(crate) fn set_recording(&mut self, on: bool) {
        self.recorder = on.then(Recorder::default);
    }

    pub(crate) fn answer_learn(&mut self, word: &str, keep: bool) {
        if keep {
            if self.undo.confirm_learn(word, &mut self.exceptions) {
                self.exceptions.write_to(&mut self.settings);
                self.toast("toast.learned", &[("word", word.to_lowercase())]);
            }
        } else {
            self.undo.decline_learn(word);
        }
        self.undo.write_to(&mut self.settings);
        self.mark_settings();
    }

    pub(crate) fn dismiss_learn(&mut self, word: &str) {
        self.undo.dismiss_learn(word);
        self.undo.write_to(&mut self.settings);
        self.mark_settings();
    }

    pub(crate) fn picker_choose(&mut self, index: usize, now: Instant) {
        self.run_deferred(Deferred::PickerChoose(index), now);
    }

    /// Replaces the settings (a UI patch already merged) and rebuilds what depends on them.
    pub(crate) fn set_settings(&mut self, new: Settings, now: Instant) {
        let old = std::mem::replace(&mut self.settings, new);
        let s = &self.settings;
        self.exceptions = Exceptions::from_settings(s);
        if old.undo_strike_count != s.undo_strike_count
            || old.undo_strike_time != s.undo_strike_time
            || old.undo_declined_words != s.undo_declined_words
        {
            self.undo = UndoLearner::from_settings(s);
        }
        self.undo.enabled = s.learn_on_undo_enabled;
        if old.typo_personal_words != s.typo_personal_words {
            self.typo = TypoFixer::new(s.typo_personal_words.clone());
        }
        self.gestures = Default::default();
        self.rebuild_keymap();
        self.refresh_app_cache();
        if old.paused_until != self.settings.paused_until {
            self.schedule_pause_expiry(now);
        }
        self.mark_settings();
    }

    pub(crate) fn toast_clash(&mut self) {
        if let Some((a, b)) = active_clash(&self.settings) {
            self.toast("toast.hotkeyClash", &[("a", a.name().to_string()), ("b", b.name().to_string())]);
        }
    }
}
