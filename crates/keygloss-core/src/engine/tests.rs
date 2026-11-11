//! Scenario tests against an in-memory OS: a text field with a caret and selection, two layouts
//! (US QWERTY, Russian ЙЦУКЕН) the app types with, a clipboard, a foreground app and a
//! password flag. Time is a fake clock; the worker is driven by [`H::advance`].

use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use super::runtime::{EventSink, HookHandler, Shared, pump, start_with_clock};
use super::*;
use crate::platform::{
    AppInfo, Clipboard, FocusedText, ForegroundApp, InputHandler, InputHook, PlatformError, SelectionRead,
};

const EN: &str = "00000409";
const RU: &str = "00000419";

const ROWS: [&[KeyCode]; 4] = {
    use KeyCode::*;
    [
        &[Backquote, Digit1, Digit2, Digit3, Digit4, Digit5, Digit6, Digit7, Digit8, Digit9, Digit0, Minus, Equal],
        &[KeyQ, KeyW, KeyE, KeyR, KeyT, KeyY, KeyU, KeyI, KeyO, KeyP, BracketLeft, BracketRight, Backslash],
        &[KeyA, KeyS, KeyD, KeyF, KeyG, KeyH, KeyJ, KeyK, KeyL, Semicolon, Quote],
        &[KeyZ, KeyX, KeyC, KeyV, KeyB, KeyN, KeyM, Comma, Period, Slash],
    ]
};

fn table(plain: [&str; 4], shifted: [&str; 4]) -> KeyTable {
    let mut t = KeyTable::new();
    for (row, keys) in ROWS.iter().enumerate() {
        for (shift, chars) in [(false, plain[row]), (true, shifted[row])] {
            for (k, c) in keys.iter().zip(chars.chars()) {
                t.insert((*k, shift), c);
            }
        }
    }
    t
}

fn us() -> KeyTable {
    table(
        ["`1234567890-=", "qwertyuiop[]\\", "asdfghjkl;'", "zxcvbnm,./"],
        ["~!@#$%^&*()_+", "QWERTYUIOP{}|", "ASDFGHJKL:\"", "ZXCVBNM<>?"],
    )
}

fn ru() -> KeyTable {
    table(
        ["ё1234567890-=", "йцукенгшщзхъ\\", "фывапролджэ", "ячсмитьбю."],
        ["Ё!\"№;%:?*()_+", "ЙЦУКЕНГШЩЗХЪ/", "ФЫВАПРОЛДЖЭ", "ЯЧСМИТЬБЮ,"],
    )
}

fn key_for(t: &KeyTable, c: char) -> (KeyCode, bool) {
    match c {
        ' ' => (KeyCode::Space, false),
        '\n' => (KeyCode::Enter, false),
        '\t' => (KeyCode::Tab, false),
        _ => *t.iter().find(|(_, v)| **v == c).unwrap_or_else(|| panic!("no key for {c:?}")).0,
    }
}

// ---- Fake OS ----

#[derive(Default)]
struct Field {
    text: Vec<char>,
    caret: usize,
    /// Selected range; the caret is at its end.
    sel: Option<(usize, usize)>,
}

impl Field {
    fn delete_selection(&mut self) -> bool {
        let Some((a, b)) = self.sel.take() else { return false };
        self.text.drain(a..b);
        self.caret = a;
        true
    }
    fn backspace(&mut self) {
        if !self.delete_selection() && self.caret > 0 {
            self.caret -= 1;
            self.text.remove(self.caret);
        }
    }
    fn insert(&mut self, c: char) {
        self.delete_selection();
        self.text.insert(self.caret, c);
        self.caret += 1;
    }
    fn selected(&self) -> Option<String> {
        self.sel.map(|(a, b)| self.text[a..b].iter().collect())
    }
}

struct Os {
    field: Field,
    layout: LayoutId,
    tables: HashMap<LayoutId, KeyTable>,
    app: AppId,
    password: bool,
    /// The focused element exposes a selection API (UI Automation).
    uia: bool,
    /// Ctrl+C with nothing selected copies the whole line (like many editors).
    copy_line: bool,
    clipboard: String,
    /// The clipboard also holds a rich format.
    rich: bool,
    clip_count: u64,
    on_foreground: Option<Box<dyn Fn(AppId) + Send + Sync>>,
    outputs: Vec<(usize, String)>,
    chords: Vec<(KeyCode, Modifiers)>,
    masks: usize,
    selection_reads: usize,
}

struct FakeOs(Mutex<Os>);

impl FakeOs {
    fn new() -> Arc<FakeOs> {
        let tables = HashMap::from([(LayoutId(EN.into()), us()), (LayoutId(RU.into()), ru())]);
        Arc::new(FakeOs(Mutex::new(Os {
            field: Field::default(),
            layout: LayoutId(EN.into()),
            tables,
            app: AppId::from_exe("notepad.exe"),
            password: false,
            uia: true,
            copy_line: false,
            clipboard: String::new(),
            rich: false,
            clip_count: 0,
            on_foreground: None,
            outputs: Vec::new(),
            chords: Vec::new(),
            masks: 0,
            selection_reads: 0,
        })))
    }

    fn os(&self) -> std::sync::MutexGuard<'_, Os> {
        self.0.lock().unwrap()
    }

    fn paste(os: &mut Os) {
        let text = os.clipboard.clone();
        for c in text.chars() {
            os.field.insert(c);
        }
    }

    fn copy(os: &mut Os) {
        let copied = match os.field.selected() {
            Some(s) => s,
            None if os.copy_line => os.field.text.iter().collect::<String>() + "\n",
            None => return,
        };
        os.clipboard = copied;
        os.rich = false;
        os.clip_count += 1;
    }

    /// The app receives a real key-down that the hook passed.
    fn app_key(&self, ev: &KeyEvent) {
        use KeyCode::*;
        let mut os = self.os();
        if ev.mods.ctrl {
            match ev.key {
                KeyV => Self::paste(&mut os),
                KeyC => Self::copy(&mut os),
                _ => {}
            }
            return;
        }
        if ev.mods.alt || ev.mods.meta {
            return;
        }
        let f = &mut os.field;
        match ev.key {
            Backspace => f.backspace(),
            Space => f.insert(' '),
            Enter | NumpadEnter => f.insert('\n'),
            Tab => f.insert('\t'),
            ArrowLeft => {
                f.sel = None;
                f.caret = f.caret.saturating_sub(1);
            }
            ArrowRight => {
                f.sel = None;
                f.caret = (f.caret + 1).min(f.text.len());
            }
            _ => {
                let layout = os.layout.clone();
                if let Some(&c) = os.tables[&layout].get(&(ev.key, ev.mods.shift)) {
                    let c = if ev.caps_lock && c.is_alphabetic() { flip_case(c) } else { c };
                    os.field.insert(c);
                }
            }
        }
    }
}

impl TextOutput for FakeOs {
    fn replace_text(&self, backspaces: usize, text: &str) -> Result<(), PlatformError> {
        let mut os = self.os();
        for _ in 0..backspaces {
            os.field.backspace();
        }
        for c in text.chars() {
            os.field.insert(c);
        }
        os.outputs.push((backspaces, text.to_string()));
        Ok(())
    }
    fn send_chord(&self, key: KeyCode, mods: Modifiers) -> Result<(), PlatformError> {
        let mut os = self.os();
        os.chords.push((key, mods));
        if mods.ctrl && key == KeyCode::KeyC {
            Self::copy(&mut os);
        } else if mods.ctrl && key == KeyCode::KeyV {
            Self::paste(&mut os);
        }
        Ok(())
    }
    fn send_mask_key(&self) -> Result<(), PlatformError> {
        self.os().masks += 1;
        Ok(())
    }
}

impl Layouts for FakeOs {
    fn current_layout(&self) -> Option<LayoutId> {
        Some(self.os().layout.clone())
    }
    fn switch_layout(&self, id: &LayoutId) -> Result<(), PlatformError> {
        self.os().layout = id.clone();
        Ok(())
    }
    fn installed_layouts(&self) -> Vec<LayoutInfo> {
        vec![
            LayoutInfo { id: LayoutId(EN.into()), lang: "en-US".into(), name: "English".into() },
            LayoutInfo { id: LayoutId(RU.into()), lang: "ru-RU".into(), name: "Russian".into() },
        ]
    }
    fn key_table(&self, id: &LayoutId) -> KeyTable {
        self.os().tables.get(id).cloned().unwrap_or_default()
    }
}

impl ForegroundApp for FakeOs {
    fn foreground_app(&self) -> Option<AppId> {
        Some(self.os().app.clone())
    }
    fn on_foreground_change(&self, callback: Box<dyn Fn(AppId) + Send + Sync>) {
        self.os().on_foreground = Some(callback);
    }
    fn running_apps(&self) -> Vec<AppInfo> {
        let id = self.os().app.clone();
        vec![AppInfo { name: id.0.clone(), id }]
    }
}

impl FocusedText for FakeOs {
    fn selected_text(&self) -> SelectionRead {
        let mut os = self.os();
        if os.password {
            return SelectionRead::Password;
        }
        if !os.uia {
            return SelectionRead::Unavailable;
        }
        os.selection_reads += 1;
        match os.field.selected() {
            Some(s) if !s.is_empty() => SelectionRead::Text(s),
            _ => SelectionRead::Empty,
        }
    }
    fn focused_is_password(&self) -> bool {
        self.os().password
    }
}

impl Clipboard for FakeOs {
    fn get_text(&self) -> Option<String> {
        Some(self.os().clipboard.clone())
    }
    fn set_text(&self, text: &str) -> Result<(), PlatformError> {
        let mut os = self.os();
        os.clipboard = text.to_string();
        os.rich = false;
        os.clip_count += 1;
        Ok(())
    }
    fn snapshot(&self) -> ClipboardSnapshot {
        let os = self.os();
        let mut formats = vec![(13, os.clipboard.as_bytes().to_vec())];
        if os.rich {
            formats.push((49_000, b"rich".to_vec()));
        }
        ClipboardSnapshot { formats }
    }
    fn restore(&self, snapshot: &ClipboardSnapshot) -> Result<(), PlatformError> {
        let mut os = self.os();
        os.clipboard = String::from_utf8(snapshot.formats[0].1.clone()).unwrap();
        os.rich = snapshot.formats.len() > 1;
        os.clip_count += 1;
        Ok(())
    }
    fn change_count(&self) -> u64 {
        self.os().clip_count
    }
}

struct FakeClock {
    base: Instant,
    offset: Mutex<Duration>,
}

impl Clock for FakeClock {
    fn now(&self) -> Instant {
        self.base + *self.offset.lock().unwrap()
    }
    fn unix_now(&self) -> f64 {
        1_800_000_000.0 + self.offset.lock().unwrap().as_secs_f64()
    }
    fn sleep(&self, d: Duration) {
        *self.offset.lock().unwrap() += d;
    }
}

impl FakeClock {
    fn set(&self, t: Instant) {
        let mut o = self.offset.lock().unwrap();
        *o = (*o).max(t - self.base);
    }
}

// ---- Harness ----

/// Gap between typed keys.
const KEY_GAP: u64 = 40;

struct H {
    os: Arc<FakeOs>,
    clock: Arc<FakeClock>,
    shared: Arc<Shared>,
    platform: Platform,
    handler: HookHandler,
    sink: EventSink,
    events: Arc<Mutex<Vec<EngineEvent>>>,
    held: Vec<KeyCode>,
    caps: bool,
}

fn mods_of(held: &[KeyCode], except: KeyCode) -> Modifiers {
    let on = |a: KeyCode, b: KeyCode| held.iter().any(|&k| k != except && (k == a || k == b));
    use KeyCode::*;
    Modifiers {
        ctrl: on(ControlLeft, ControlRight),
        alt: on(AltLeft, AltRight),
        shift: on(ShiftLeft, ShiftRight),
        meta: on(MetaLeft, MetaRight),
    }
}

const CTRL_ALT: Modifiers = Modifiers { ctrl: true, alt: true, shift: false, meta: false };

impl H {
    fn new() -> H {
        H::with(|_| {})
    }

    /// Default settings with live fix off (so boundary tests see only the boundary path).
    fn boundary() -> H {
        H::with(|s| s.live_fix_enabled = false)
    }

    fn with(edit: impl FnOnce(&mut Settings)) -> H {
        data::warm_up();
        let mut settings = Settings::default();
        edit(&mut settings);
        let os = FakeOs::new();
        let clock = Arc::new(FakeClock { base: Instant::now(), offset: Mutex::new(Duration::ZERO) });
        let platform = Platform {
            output: os.clone(),
            layouts: os.clone(),
            foreground: os.clone(),
            focused: os.clone(),
            clipboard: os.clone(),
        };
        let shared = Shared::new(settings, &platform, &*clock);
        {
            let (shared, clock) = (shared.clone(), clock.clone());
            platform.foreground.on_foreground_change(Box::new(move |app| {
                let now = clock.now();
                shared.with(|e| e.on_foreground_change(app, now));
            }));
        }
        shared.with(|e| e.set_hook_state(true, true));
        let events: Arc<Mutex<Vec<EngineEvent>>> = Arc::default();
        let sink: EventSink = {
            let events = events.clone();
            Arc::new(move |ev| events.lock().unwrap().push(ev))
        };
        let handler = HookHandler { shared: shared.clone(), clock: clock.clone() };
        let h = H { os, clock, shared, platform, handler, sink, events, held: Vec::new(), caps: false };
        h.pump();
        h
    }

    fn pump(&self) {
        pump(&self.shared, &self.platform, &*self.clock, &self.sink);
    }

    fn now(&self) -> Instant {
        self.clock.now()
    }

    /// Advances time by `ms`, firing every timer at its deadline.
    fn advance(&self, ms: u64) {
        let target = self.now() + Duration::from_millis(ms);
        loop {
            let next = runtime::lock(&self.shared.engine).next_deadline();
            match next {
                Some(d) if d <= target => {
                    self.clock.set(d);
                    self.pump();
                }
                _ => break,
            }
        }
        self.clock.set(target);
        self.pump();
    }

    fn ev(&mut self, key: KeyCode, down: bool) -> HookDecision {
        let repeat = down && self.held.contains(&key);
        let event = KeyEvent { key, down, repeat, mods: mods_of(&self.held, key), caps_lock: self.caps, at: self.now() };
        if key == KeyCode::CapsLock && down && !repeat {
            self.caps = !self.caps;
        }
        if key.is_modifier() {
            self.held.retain(|&k| k != key);
            if down {
                self.held.push(key);
            }
        }
        let d = self.handler.on_key(&event);
        if d == HookDecision::Pass && down {
            self.os.app_key(&event);
        }
        self.pump();
        d
    }

    fn press(&mut self, key: KeyCode) -> HookDecision {
        let d = self.ev(key, true);
        self.ev(key, false);
        d
    }

    fn chord(&mut self, mods: &[KeyCode], key: KeyCode) -> HookDecision {
        for &m in mods {
            self.ev(m, true);
        }
        let d = self.press(key);
        for &m in mods.iter().rev() {
            self.ev(m, false);
        }
        d
    }

    /// Taps a modifier alone.
    fn tap(&mut self, key: KeyCode) {
        self.ev(key, true);
        self.ev(key, false);
    }

    fn type_with(&mut self, table: &KeyTable, s: &str) {
        for c in s.chars() {
            let (key, shift) = key_for(table, c);
            if shift {
                self.ev(KeyCode::ShiftLeft, true);
            }
            self.press(key);
            if shift {
                self.ev(KeyCode::ShiftLeft, false);
            }
            self.advance(KEY_GAP);
        }
    }

    /// Types physical keys, named by what they type on a US keyboard.
    fn keys(&mut self, us_chars: &str) {
        self.type_with(&us(), us_chars);
    }

    fn text(&self) -> String {
        self.os.os().field.text.iter().collect()
    }

    fn set_text(&self, s: &str, selected: bool) {
        let mut os = self.os.os();
        os.field.text = s.chars().collect();
        os.field.caret = os.field.text.len();
        os.field.sel = selected.then_some((0, os.field.text.len()));
    }

    fn lang(&self) -> &'static str {
        if self.os.os().layout.0 == RU { "ru" } else { "en" }
    }

    fn set_layout(&self, id: &str) {
        self.os.os().layout = LayoutId(id.into());
    }

    fn set_app(&self, exe: &str) {
        let app = AppId::from_exe(exe);
        let cb = {
            let mut os = self.os.os();
            os.app = app.clone();
            os.on_foreground.take()
        };
        if let Some(cb) = cb {
            cb(app);
            self.os.os().on_foreground = Some(cb);
        }
        self.pump();
    }

    fn click(&self) {
        let now = self.now();
        self.shared.with(|e| e.on_mouse_down(now));
        self.pump();
    }

    fn take_events(&self) -> Vec<EngineEvent> {
        std::mem::take(&mut self.events.lock().unwrap())
    }

    fn toasts(&self) -> Vec<String> {
        self.take_events()
            .into_iter()
            .filter_map(|e| match e {
                EngineEvent::Toast(t) => Some(t.key),
                _ => None,
            })
            .collect()
    }

    fn settings(&self) -> Settings {
        runtime::lock(&self.shared.engine).settings().clone()
    }

    fn engine<T>(&self, f: impl FnOnce(&mut Engine, Instant) -> T) -> T {
        let now = self.now();
        let out = self.shared.with(|e| f(e, now));
        self.pump();
        out
    }

    /// Presses the manual hotkey (Pause) and waits out the output flight.
    fn manual(&mut self) -> HookDecision {
        let d = self.press(KeyCode::Pause);
        self.advance(100);
        d
    }
}

fn hotkey(key: KeyCode, mods: Modifiers) -> Hotkey {
    Hotkey::Key { key, mods }
}

// ---- Automatic conversion ----

#[test]
fn logs_each_fix_with_lengths_and_never_the_text() {
    let mut h = H::boundary();
    h.keys("ghbdtn ");
    h.advance(100);
    assert_eq!(h.text(), "привет ");
    let logs: Vec<String> = h
        .take_events()
        .into_iter()
        .filter_map(|e| match e {
            EngineEvent::Log(line) => Some(line),
            _ => None,
        })
        .collect();
    assert!(logs.iter().any(|l| l.starts_with("fix kind=auto len=6->6 to=ru app=")), "{logs:?}");
    assert!(logs.iter().any(|l| l.starts_with("layouts [")), "{logs:?}");
    assert!(!logs.iter().any(|l| l.contains("ghbdtn") || l.contains("привет")), "{logs:?}");
}

#[test]
fn converts_wrong_layout_word_at_space() {
    let mut h = H::boundary();
    h.keys("ghbdtn ");
    h.advance(100);
    assert_eq!(h.text(), "привет ");
    assert_eq!(h.lang(), "ru");
    let events = h.take_events();
    assert!(events.iter().any(|e| matches!(e, EngineEvent::PlaySound { sound: Sound::Beep, volume } if (*volume - 0.6).abs() < 1e-9)));
    assert!(events.iter().any(|e| matches!(e, EngineEvent::SettingsChanged(s) if s.rescued_count == 1)));
    // Typing continues in Russian.
    h.keys("vbh ");
    h.advance(100);
    assert_eq!(h.text(), "привет мир ");
}

#[test]
fn sound_off_plays_nothing_and_another_sound_is_passed_on() {
    let convert_with = |sound| {
        let mut h = H::boundary();
        h.shared.with(|e| {
            let mut s = e.settings().clone();
            s.sound = sound;
            e.set_settings(s, Instant::now());
        });
        h.keys("ghbdtn ");
        h.advance(100);
        assert_eq!(h.text(), "привет ");
        h.take_events()
    };
    assert!(!convert_with(Sound::Off).iter().any(|e| matches!(e, EngineEvent::PlaySound { .. })));
    assert!(convert_with(Sound::Custom).iter().any(|e| matches!(e, EngineEvent::PlaySound { sound: Sound::Custom, .. })));
}

#[test]
fn converts_russian_layout_gibberish_to_english() {
    let mut h = H::boundary();
    h.set_layout(RU);
    h.keys("hello ");
    h.advance(100);
    assert_eq!(h.text(), "hello ");
    assert_eq!(h.lang(), "en");
}

#[test]
fn leaves_real_and_ignored_words_alone() {
    let mut h = H::with(|s| {
        s.live_fix_enabled = false;
        s.ignored_words.push("ghbdtn".into());
    });
    h.keys("hello world ghbdtn ");
    h.advance(100);
    assert_eq!(h.text(), "hello world ghbdtn ");
    assert_eq!(h.lang(), "en");
}

#[test]
fn enter_converts_before_the_enter_reaches_the_app() {
    let mut h = H::boundary();
    h.keys("ghbdtn");
    assert_eq!(h.press(KeyCode::Enter), HookDecision::Swallow);
    assert_eq!(h.text(), "привет\n");
    assert_eq!(h.os.os().outputs.last().unwrap(), &(6, "привет\n".to_string()));
    h.advance(100);
    assert_eq!(h.text(), "привет\n");
    // With enterPreConvert off, Enter passes and the boundary path converts afterwards.
    let mut h = H::with(|s| {
        s.live_fix_enabled = false;
        s.enter_pre_convert = false;
    });
    h.keys("ghbdtn");
    assert_eq!(h.press(KeyCode::Enter), HookDecision::Pass);
    h.advance(100);
    assert_eq!(h.text(), "привет\n");
}

#[test]
fn shortcuts_are_not_boundaries() {
    let mut h = H::boundary();
    h.keys("ghbdtn");
    h.chord(&[KeyCode::AltLeft], KeyCode::Tab);
    h.advance(100);
    assert_eq!(h.text(), "ghbdtn");
}

#[test]
fn arrows_cancel_and_backspace_edits() {
    let mut h = H::boundary();
    h.keys("ghbdtn");
    h.press(KeyCode::ArrowLeft);
    h.press(KeyCode::ArrowRight);
    h.keys(" ");
    h.advance(100);
    assert_eq!(h.text(), "ghbdtn ");
    // A typo fixed with Backspace is still converted at the boundary.
    let mut h = H::boundary();
    h.keys("ghbdtx");
    h.press(KeyCode::Backspace);
    h.keys("n ");
    h.advance(100);
    assert_eq!(h.text(), "привет ");
}

#[test]
fn caps_lock_and_shift_case() {
    let mut h = H::boundary();
    h.keys("Ghbdtn ");
    h.advance(100);
    assert_eq!(h.text(), "Привет ");
    let mut h = H::boundary();
    h.press(KeyCode::CapsLock);
    h.keys("ghbdtn ");
    h.advance(100);
    assert_eq!(h.text(), "ПРИВЕТ ");
}

// ---- Live fix ----

#[test]
fn live_fix_mid_word_keeps_typing_in_the_new_layout() {
    let mut h = H::new();
    let word = "ghjuhfvvf"; // программа
    h.keys(word);
    assert_eq!(h.text(), "программа");
    assert_eq!(h.lang(), "ru");
    let outputs = h.os.os().outputs.clone();
    assert_eq!(outputs.len(), 1, "one inline fix");
    // The fix replaced a prefix; the keys after it were typed by the app in the Russian layout.
    assert!(outputs[0].0 < word.len() - 1, "fixed mid-word: {outputs:?}");
    h.keys(" ");
    h.advance(200);
    assert_eq!(h.text(), "программа ");
}

#[test]
fn pause_fix_after_a_mid_word_stop() {
    let mut h = H::with(|s| s.inline_live_fix = false);
    h.keys("ghjuhfvv");
    assert!(h.os.os().outputs.is_empty());
    h.advance(200);
    assert_eq!(h.text(), "программ");
    assert_eq!(h.lang(), "ru");
    h.keys("f ");
    h.advance(100);
    assert_eq!(h.text(), "программа ");
}

#[test]
fn live_fix_skips_chromium_apps_but_the_boundary_still_converts() {
    let mut h = H::new();
    h.set_app("chrome.exe");
    h.keys("ghjuhfvvf");
    h.advance(300);
    assert_eq!(h.text(), "ghjuhfvvf");
    h.keys(" ");
    h.advance(100);
    assert_eq!(h.text(), "программа ");
}

#[test]
fn min_letters_gates_automatic_fixes_only() {
    let short = |s: &mut Settings| {
        s.live_fix_enabled = false;
        s.auto_min_letters = 3;
    };
    // Default (1): a two-letter word is switched.
    let mut h = H::boundary();
    h.keys("yt ");
    h.advance(100);
    assert_eq!(h.text(), "не ");
    // Below the threshold: left alone, on Space and on Enter.
    for boundary in [" ", "\n"] {
        let mut h = H::with(short);
        h.keys(&format!("yt{boundary}"));
        h.advance(100);
        assert_eq!(h.text(), format!("yt{boundary}"));
        assert_eq!(h.lang(), "en");
    }
    // At the threshold: switched.
    let mut h = H::with(short);
    h.keys("vbh ");
    h.advance(100);
    assert_eq!(h.text(), "мир ");
    // The manual hotkey still switches a short word.
    let mut h = H::with(short);
    h.keys("yt");
    assert_eq!(h.manual(), HookDecision::Swallow);
    assert_eq!(h.text(), "не");
}

#[test]
fn min_letters_delays_the_live_fix() {
    let first_fix = |min: u32, inline: bool| {
        let mut h = H::with(|s| {
            s.auto_min_letters = min;
            s.inline_live_fix = inline;
        });
        if inline {
            h.keys("ghjuhfvvf");
        } else {
            h.keys("ghjuh");
            h.advance(200);
            let fixed = h.text() != "ghjuh";
            h.keys("fvvf");
            h.advance(200);
            assert_eq!(h.text().chars().count(), 9);
            return if fixed { 5 } else { 9 };
        }
        assert_eq!(h.text(), "программа");
        // Letters on screen after the first fix.
        h.os.os().outputs[0].1.chars().count()
    };
    assert!(first_fix(1, true) < 7);
    assert!(first_fix(7, true) >= 7);
    assert_eq!(first_fix(1, false), 5);
    assert_eq!(first_fix(7, false), 9);
}

// ---- Manual hotkey ----

#[test]
fn manual_converts_last_word_then_undoes() {
    let mut h = H::with(|s| s.auto_enabled = false);
    h.keys("ghbdtn");
    assert_eq!(h.manual(), HookDecision::Swallow);
    assert_eq!(h.text(), "привет");
    assert_eq!(h.lang(), "ru");
    h.manual();
    assert_eq!(h.text(), "ghbdtn");
    assert_eq!(h.lang(), "en");
}

#[test]
fn manual_undoes_an_automatic_conversion() {
    let mut h = H::boundary();
    h.keys("ghbdtn ");
    h.advance(100);
    assert_eq!(h.text(), "привет ");
    h.manual();
    assert_eq!(h.text(), "ghbdtn ");
    assert_eq!(h.lang(), "en");
}

#[test]
fn manual_converts_selection_via_uia_and_clipboard() {
    let mut h = H::new();
    h.set_text("ghbdtn vbh", true);
    h.click();
    h.manual();
    assert_eq!(h.text(), "привет мир");
    assert_eq!(h.lang(), "ru");
    // No selection API: Ctrl+C fallback, clipboard restored.
    let mut h = H::new();
    h.os.os().uia = false;
    h.os.os().clipboard = "keep me".into();
    h.os.os().rich = true;
    h.set_text("руддщ", true);
    h.click();
    h.manual();
    assert_eq!(h.text(), "hello");
    let os = h.os.os();
    assert_eq!((os.clipboard.as_str(), os.rich), ("keep me", true));
}

#[test]
fn manual_refuses_an_auto_copied_line() {
    let mut h = H::new();
    {
        let mut os = h.os.os();
        os.uia = false;
        os.copy_line = true;
    }
    h.set_text("ghbdtn", false);
    h.click();
    h.manual();
    assert_eq!(h.text(), "ghbdtn");
    assert_eq!(h.lang(), "en", "a refused selection does not cycle the layout");
}

#[test]
fn manual_with_nothing_cycles_the_layout() {
    let mut h = H::new();
    h.manual();
    assert_eq!(h.lang(), "ru");
    h.manual();
    assert_eq!(h.lang(), "en");
    assert_eq!(h.text(), "");
}

#[test]
fn manual_group_conversion_when_auto_is_off() {
    let mut h = H::with(|s| s.auto_enabled = false);
    h.keys("ghbdtn vbh");
    h.manual();
    assert_eq!(h.text(), "привет мир");
}

#[test]
fn manual_is_queued_while_output_is_in_flight() {
    let mut h = H::boundary();
    h.keys("ghbdtn ");
    h.advance(31); // boundary conversion just ran: output in flight for 80 ms
    assert_eq!(h.text(), "привет ");
    h.press(KeyCode::Pause);
    assert_eq!(h.text(), "привет ", "queued");
    h.advance(200);
    assert_eq!(h.text(), "ghbdtn ", "replayed after the flight");
}

// ---- Hotkey modes ----

/// Fires the manual hotkey on an empty buffer: visible as a layout cycle.
fn fired(h: &H, before: &str) -> bool {
    h.lang() != before
}

#[test]
fn combo_fires_on_release_unless_another_key_was_pressed() {
    let mut h = H::with(|s| s.hotkey = Hotkey::Combo { mods: CTRL_ALT });
    h.ev(KeyCode::ControlLeft, true);
    h.ev(KeyCode::AltLeft, true);
    h.ev(KeyCode::AltLeft, false);
    assert!(!fired(&h, "en"), "not until every modifier is up");
    h.ev(KeyCode::ControlLeft, false);
    h.advance(10);
    assert!(fired(&h, "en"));
    h.chord(&[KeyCode::ControlLeft, KeyCode::AltLeft], KeyCode::KeyK);
    h.advance(10);
    assert!(!fired(&h, "ru"), "chord broken by K");
    // An extra modifier disarms it (until the held set equals the chord again).
    h.ev(KeyCode::ControlLeft, true);
    h.ev(KeyCode::AltLeft, true);
    h.ev(KeyCode::ShiftLeft, true);
    h.ev(KeyCode::AltLeft, false);
    h.ev(KeyCode::ControlLeft, false);
    h.ev(KeyCode::ShiftLeft, false);
    h.advance(10);
    assert!(!fired(&h, "ru"));
}

#[test]
fn double_tap_within_300ms_either_side() {
    let mut h = H::with(|s| s.hotkey = Hotkey::DoubleTap { key: KeyCode::ShiftLeft });
    h.tap(KeyCode::ShiftLeft);
    h.advance(100);
    h.tap(KeyCode::ShiftRight);
    h.advance(10);
    assert!(fired(&h, "en"));
    h.tap(KeyCode::ShiftLeft);
    h.advance(400);
    h.tap(KeyCode::ShiftLeft);
    h.advance(10);
    assert!(!fired(&h, "ru"), "too slow");
    h.advance(400);
    h.tap(KeyCode::ShiftLeft);
    h.press(KeyCode::KeyA);
    h.tap(KeyCode::ShiftLeft);
    h.advance(10);
    assert!(!fired(&h, "ru"), "a key between the taps");
}

#[test]
fn modkey_is_side_specific_and_solo() {
    let mut h = H::with(|s| s.hotkey = Hotkey::ModKey { key: KeyCode::AltRight });
    h.tap(KeyCode::AltLeft);
    h.advance(10);
    assert!(!fired(&h, "en"), "wrong side");
    h.ev(KeyCode::ShiftLeft, true);
    h.tap(KeyCode::AltRight);
    h.ev(KeyCode::ShiftLeft, false);
    h.advance(10);
    assert!(!fired(&h, "en"), "not solo");
    h.ev(KeyCode::AltRight, true);
    h.press(KeyCode::KeyA);
    h.ev(KeyCode::AltRight, false);
    h.advance(10);
    assert!(!fired(&h, "en"), "used as a modifier");
    h.tap(KeyCode::AltRight);
    h.advance(10);
    assert!(fired(&h, "en"));
}

#[test]
fn alt_and_win_modifier_taps_send_a_mask_key() {
    use KeyCode::*;
    let masks = |h: &H| h.os.os().masks;
    let mut h = H::with(|s| s.hotkey = Hotkey::ModKey { key: AltRight });
    h.tap(AltRight);
    assert_eq!(masks(&h), 1, "Alt modkey");
    h.tap(AltLeft);
    assert_eq!(masks(&h), 1, "did not fire");

    let mut h = H::with(|s| s.hotkey = Hotkey::DoubleTap { key: MetaLeft });
    h.tap(MetaLeft);
    h.advance(100);
    h.ev(MetaLeft, true);
    assert_eq!(masks(&h), 1, "Win double tap: masked while the key is still down");
    h.ev(MetaLeft, false);

    let mut h = H::with(|s| s.hotkey = Hotkey::Combo { mods: CTRL_ALT });
    h.ev(ControlLeft, true);
    h.ev(AltLeft, true);
    h.ev(AltLeft, false);
    assert_eq!(masks(&h), 1, "Alt released while the chord is armed");
    h.ev(ControlLeft, false);
    assert_eq!(masks(&h), 2, "the chord fired");

    for hk in [
        Hotkey::ModKey { key: ShiftLeft },
        Hotkey::DoubleTap { key: ControlLeft },
        Hotkey::Combo { mods: Modifiers { ctrl: true, shift: true, ..Modifiers::NONE } },
    ] {
        let mut h = H::with(|s| s.hotkey = hk);
        h.ev(ControlLeft, true);
        h.ev(ShiftLeft, true);
        h.ev(ShiftLeft, false);
        h.ev(ControlLeft, false);
        h.tap(ShiftLeft);
        h.tap(ControlLeft);
        h.advance(50);
        h.tap(ControlLeft);
        assert_eq!(masks(&h), 0, "{hk:?}");
    }
}

#[test]
fn key_hotkey_needs_exact_modifiers_and_is_swallowed() {
    let mut h = H::with(|s| s.hotkey = hotkey(KeyCode::KeyK, CTRL_ALT));
    assert_eq!(h.chord(&[KeyCode::ControlLeft], KeyCode::KeyK), HookDecision::Pass);
    h.advance(10);
    assert!(!fired(&h, "en"));
    assert_eq!(h.chord(&[KeyCode::ControlLeft, KeyCode::AltLeft, KeyCode::ShiftLeft], KeyCode::KeyK), HookDecision::Pass);
    h.advance(10);
    assert!(!fired(&h, "en"));
    assert_eq!(h.chord(&[KeyCode::ControlLeft, KeyCode::AltRight], KeyCode::KeyK), HookDecision::Swallow);
    h.advance(10);
    assert!(fired(&h, "en"));
    // A bare non-safe key saved by hand is not intercepted.
    let mut h = H::with(|s| s.hotkey = hotkey(KeyCode::KeyK, Modifiers::NONE));
    assert_eq!(h.press(KeyCode::KeyK), HookDecision::Pass);
    assert_eq!(h.text(), "k");
}

// ---- Per-app policy ----

#[test]
fn app_off_mode_and_developer_mode() {
    let mut h = H::with(|s| {
        s.live_fix_enabled = false;
        s.app_exception_modes.insert("notepad.exe".into(), AppMode::Off);
    });
    h.set_app("notepad.exe");
    h.keys("ghbdtn ");
    h.advance(100);
    assert_eq!(h.text(), "ghbdtn ");

    let mut h = H::with(|s| {
        s.live_fix_enabled = false;
        s.developer_mode = true;
    });
    h.set_app("idea64.exe");
    h.keys("ghbdtn ");
    h.advance(100);
    assert_eq!(h.text(), "ghbdtn ");
}

#[test]
fn soft_mode_skips_short_and_slow_words() {
    let mut h = H::boundary();
    h.set_app("code.exe"); // seeded as soft on first sight
    assert_eq!(h.settings().app_exception_modes.get("code.exe"), Some(&AppMode::Soft));
    assert!(h.settings().seeded_default_apps.contains(&"code.exe".to_string()));
    h.keys("ghbdtn ");
    h.advance(100);
    assert_eq!(h.text(), "привет ");
    let mut h = H::boundary();
    h.set_app("code.exe");
    h.keys("ghb");
    h.advance(1500); // an inner pause: commands, not typing
    h.keys("dtn ");
    h.advance(100);
    assert_eq!(h.text(), "ghbdtn ");
}

#[test]
fn seeding_marks_terminals_off() {
    let h = H::boundary();
    h.set_app("WindowsTerminal.exe");
    let s = h.settings();
    assert_eq!(s.app_exception_modes.get("windowsterminal.exe"), Some(&AppMode::Off));
    assert!(h.take_events().iter().any(|e| matches!(e, EngineEvent::SettingsChanged(s) if s.seeded_default_apps.contains(&"windowsterminal.exe".to_string()))));
}

#[test]
fn forced_layout_on_app_activation() {
    let h = H::with(|s| {
        s.app_forced_layouts.insert("winword.exe".into(), "ru".into());
    });
    assert_eq!(h.lang(), "en");
    h.set_app("winword.exe");
    assert_eq!(h.lang(), "ru");
    // Once per switch: a manual change inside the app sticks.
    h.advance(100);
    h.set_layout(EN);
    h.advance(1000);
    assert_eq!(h.lang(), "en");
    h.set_app("notepad.exe");
    h.set_app("winword.exe");
    assert_eq!(h.lang(), "ru");
}

// ---- Pause, auto off ----

#[test]
fn pause_and_auto_off_stop_conversion() {
    let mut h = H::boundary();
    let status = h.engine(|e, now| {
        e.pause(15, now);
        e.status(now)
    });
    assert!(status.paused_until.is_some());
    assert_eq!(h.toasts(), vec!["toast.pausedUntil"]);
    h.keys("ghbdtn ");
    h.advance(100);
    assert_eq!(h.text(), "ghbdtn ");
    h.engine(|e, now| e.resume(now));
    assert_eq!(h.toasts(), vec!["toast.resumed"]);
    h.keys("ghbdtn ");
    h.advance(100);
    assert_eq!(h.text(), "ghbdtn привет ");

    h.engine(|e, _| e.set_auto(false));
    assert_eq!(h.toasts(), vec!["toast.autoOff"]);
    h.keys("hello "); // the layout is Russian now
    h.advance(100);
    assert_eq!(h.text(), "ghbdtn привет руддщ ");
    h.engine(|e, _| e.set_auto(true));
    h.keys("hello ");
    h.advance(100);
    assert_eq!(h.text(), "ghbdtn привет руддщ hello ");
}

#[test]
fn pause_expires_by_itself() {
    let h = H::boundary();
    h.engine(|e, now| e.pause(15, now));
    h.take_events();
    h.advance(15 * 60 * 1000 + 600);
    let events = h.take_events();
    assert!(events.iter().any(|e| matches!(e, EngineEvent::StatusChanged(s) if s.paused_until.is_none())));
}

// ---- Password fields ----

#[test]
fn password_field_is_never_touched() {
    let mut h = H::with(|s| s.snippets_ordered.push(("!br".into(), "Best regards".into())));
    h.os.os().password = true;
    h.click();
    h.keys("ghjuhfvvf ghbdtn !br ");
    h.advance(300);
    assert_eq!(h.text(), "ghjuhfvvf ghbdtn !br ");
    h.set_text("ghbdtn", true);
    h.manual();
    assert_eq!(h.text(), "ghbdtn");
    assert_eq!(h.os.os().selection_reads, 0);
    assert!(h.os.os().chords.is_empty(), "no Ctrl+C");
}

#[test]
fn password_flag_is_rechecked_before_a_deferred_conversion() {
    let mut h = H::boundary();
    h.keys("ghbdtn");
    h.press(KeyCode::Space);
    h.os.os().password = true; // focus moved to a password field before the boundary timer
    h.advance(100);
    assert_eq!(h.text(), "ghbdtn ");
}

// ---- Snippets ----

#[test]
fn snippet_expands_in_either_layout() {
    let mut h = H::boundary();
    h.shared.with(|e| {
        let mut s = e.settings().clone();
        s.snippets_ordered.push(("!br".into(), "Best regards".into()));
        e.set_settings(s, Instant::now());
    });
    assert_eq!(h.text(), "");
    h.keys("!br ");
    assert_eq!(h.text(), "Best regards ");
    h.set_layout(RU);
    h.keys("!br\n");
    assert_eq!(h.text(), "Best regards Best regards\n");
}

#[test]
fn snippet_picker_digits_escape_and_other_keys() {
    let mut h = H::with(|s| {
        s.text_snippets = vec![("a".into(), "Alpha".into()), ("b".into(), "Beta".into())];
        s.snippet_pick_hotkey = Some(hotkey(KeyCode::KeyJ, CTRL_ALT));
    });
    let open = |h: &mut H| h.chord(&[KeyCode::ControlLeft, KeyCode::AltLeft], KeyCode::KeyJ);
    assert_eq!(open(&mut h), HookDecision::Swallow);
    assert!(h.take_events().iter().any(|e| matches!(e, EngineEvent::SnippetPickerOpen { snippets } if snippets.len() == 2)));
    assert_eq!(h.press(KeyCode::Digit2), HookDecision::Swallow);
    assert_eq!(h.text(), "Beta");
    assert!(h.take_events().contains(&EngineEvent::SnippetPickerClose));
    open(&mut h);
    assert_eq!(h.press(KeyCode::Escape), HookDecision::Swallow);
    assert!(h.take_events().contains(&EngineEvent::SnippetPickerClose));
    assert_eq!(h.text(), "Beta");
    open(&mut h);
    assert_eq!(h.press(KeyCode::Digit9), HookDecision::Swallow, "past the list: swallowed, stays open");
    assert_eq!(h.press(KeyCode::KeyX), HookDecision::Pass);
    assert!(h.take_events().contains(&EngineEvent::SnippetPickerClose));
    assert_eq!(h.text(), "Betax");
    // Chosen from the UI.
    open(&mut h);
    h.engine(|e, now| e.picker_choose(0, now));
    assert_eq!(h.text(), "BetaxAlpha");
    // Empty list: toast, key not swallowed.
    let mut h = H::with(|s| s.snippet_pick_hotkey = Some(hotkey(KeyCode::KeyJ, CTRL_ALT)));
    assert_eq!(open(&mut h), HookDecision::Pass);
    assert_eq!(h.toasts(), vec!["toast.snippetsEmpty"]);
}

// ---- Typo fixes ----

#[test]
fn typo_fix_and_its_undo() {
    // On by default.
    let mut h = H::boundary();
    h.keys("adress ");
    h.advance(100);
    assert_eq!(h.text(), "address ");
    h.manual();
    assert_eq!(h.text(), "adress ");
    // Off: left alone.
    let mut h = H::with(|s| {
        s.live_fix_enabled = false;
        s.typo_fix = false;
    });
    h.keys("adress ");
    h.advance(100);
    assert_eq!(h.text(), "adress ");
}

#[test]
fn two_caps_fix() {
    // On by default.
    let mut h = H::boundary();
    h.keys("WHen ");
    h.advance(100);
    assert_eq!(h.text(), "When ");
    let mut h = H::with(|s| {
        s.live_fix_enabled = false;
        s.two_caps_fix = false;
    });
    h.keys("WHen ");
    h.advance(100);
    assert_eq!(h.text(), "WHen ");
}

// ---- Learn from undo ----

#[test]
fn three_undos_raise_the_banner_and_accepting_learns_the_word() {
    let mut h = H::boundary();
    for round in 0..3 {
        h.set_text("", false);
        h.set_layout(EN);
        h.click();
        h.keys("ghbdtn ");
        h.advance(100);
        assert_eq!(h.text(), "привет ", "round {round}");
        h.manual();
        assert_eq!(h.text(), "ghbdtn ");
        h.advance(1000);
    }
    let events = h.take_events();
    assert!(events.contains(&EngineEvent::LearnBanner { word: "ghbdtn".into() }));
    h.engine(|e, _| e.answer_learn("ghbdtn", true));
    let events = h.take_events();
    assert!(events.iter().any(|e| matches!(e, EngineEvent::SettingsChanged(s) if s.learned_words == vec!["ghbdtn".to_string()])));
    assert!(events.iter().any(|e| matches!(e, EngineEvent::Toast(t) if t.key == "toast.learned")));
    h.set_text("", false);
    h.click();
    h.keys("ghbdtn ");
    h.advance(100);
    assert_eq!(h.text(), "ghbdtn ");
}

// ---- Plain paste, case toggle ----

#[test]
fn plain_paste_restores_the_clipboard() {
    let mut h = H::with(|s| s.plain_paste = true);
    {
        let mut os = h.os.os();
        os.clipboard = "rich text".into();
        os.rich = true;
    }
    let ctrl_shift = [KeyCode::ControlLeft, KeyCode::ShiftLeft];
    assert_eq!(h.chord(&ctrl_shift, KeyCode::KeyV), HookDecision::Swallow);
    assert_eq!(h.text(), "rich text");
    assert!(!h.os.os().rich, "pasted as plain text");
    h.advance(350);
    assert!(h.os.os().rich, "restored");
    // The clipboard changed meanwhile: not restored.
    h.chord(&ctrl_shift, KeyCode::KeyV);
    h.os.os().clip_count += 1;
    h.advance(350);
    assert!(!h.os.os().rich);
}

#[test]
fn case_toggle_and_its_limit() {
    let mut h = H::with(|s| s.case_change_hotkey = Some(hotkey(KeyCode::KeyU, CTRL_ALT)));
    let toggle = |h: &mut H| {
        h.chord(&[KeyCode::ControlLeft, KeyCode::AltLeft], KeyCode::KeyU);
        h.advance(10);
    };
    h.set_text("phone", true);
    toggle(&mut h);
    assert_eq!(h.text(), "PHONE");
    h.advance(400);
    h.set_text("PHONE", true);
    toggle(&mut h);
    assert_eq!(h.text(), "phone");
    h.advance(400);
    h.take_events();
    let long = "a".repeat(5001);
    h.set_text(&long, true);
    toggle(&mut h);
    assert_eq!(h.text(), long);
    let events = h.take_events();
    assert!(events.iter().any(|e| matches!(e, EngineEvent::Toast(t) if t.key == "toast.caseTooLong" && t.params["max"] == "5000")));
    h.set_text("x", false);
    toggle(&mut h);
    assert_eq!(h.toasts(), vec!["toast.noSelection"]);
}

// ---- Anti-resonance ----

#[test]
fn oscillating_conversion_is_refused() {
    let mut h = H::with(|s| {
        s.live_fix_enabled = false;
        s.force_swap_words = vec!["привет".into(), "ghbdtn".into()];
    });
    h.keys("ghbdtn ");
    h.advance(40);
    assert_eq!(h.text(), "привет ");
    // The same keys again within 0.7 s now type "привет", which force-swap would flip back.
    h.keys("ghbdtn ");
    h.advance(100);
    assert_eq!(h.text(), "привет привет ");
}

// ---- Recording ----

#[test]
fn recording_captures_every_kind_and_swallows_keys() {
    let mut h = H::new();
    h.engine(|e, _| e.set_recording(true));
    let captured = |h: &H| -> Vec<Hotkey> {
        h.take_events()
            .into_iter()
            .filter_map(|e| match e {
                EngineEvent::HotkeyCaptured(hk) => Some(hk),
                _ => None,
            })
            .collect()
    };
    assert_eq!(h.chord(&[KeyCode::ControlLeft, KeyCode::AltLeft], KeyCode::KeyK), HookDecision::Swallow);
    assert_eq!(captured(&h), vec![hotkey(KeyCode::KeyK, CTRL_ALT)]);
    h.tap(KeyCode::AltRight);
    assert_eq!(captured(&h), vec![Hotkey::ModKey { key: KeyCode::AltRight }]);
    h.advance(500);
    h.tap(KeyCode::ShiftLeft);
    h.advance(100);
    h.tap(KeyCode::ShiftLeft);
    assert_eq!(
        captured(&h),
        vec![Hotkey::ModKey { key: KeyCode::ShiftLeft }, Hotkey::DoubleTap { key: KeyCode::ShiftLeft }]
    );
    h.ev(KeyCode::ControlLeft, true);
    h.ev(KeyCode::AltLeft, true);
    h.ev(KeyCode::AltLeft, false);
    h.ev(KeyCode::ControlLeft, false);
    assert_eq!(captured(&h), vec![Hotkey::Combo { mods: CTRL_ALT }]);
    assert_eq!(h.press(KeyCode::Escape), HookDecision::Swallow);
    assert_eq!(captured(&h), vec![hotkey(KeyCode::Escape, Modifiers::NONE)]);
    assert_eq!(h.press(KeyCode::KeyA), HookDecision::Swallow);
    assert_eq!(h.text(), "");
    h.engine(|e, _| e.set_recording(false));
    h.press(KeyCode::KeyA);
    assert_eq!(h.text(), "a");
}

// ---- Output in flight ----

#[test]
fn keys_during_output_do_not_corrupt_the_buffer() {
    let mut h = H::boundary();
    h.keys("ghbdtn");
    // Space, then the next word starts before the boundary conversion fires.
    h.press(KeyCode::Space);
    h.advance(5);
    h.press(KeyCode::KeyR);
    h.advance(40); // conversion fires: "привет r"
    h.keys("fr ");
    h.advance(100);
    assert_eq!(h.text(), "привет как ");
}

#[test]
fn output_waits_while_a_real_key_is_in_the_hook() {
    let mut h = H::boundary();
    h.keys("ghbdtn");
    h.press(KeyCode::Space);
    h.shared.hook_busy.store(1, Ordering::SeqCst);
    h.advance(100);
    assert_eq!(h.text(), "ghbdtn ");
    h.shared.hook_busy.store(0, Ordering::SeqCst);
    h.advance(10);
    assert_eq!(h.text(), "привет ");
}

#[test]
fn hotkey_clash_toast_and_check() {
    let h = H::with(|s| s.case_change_hotkey = Some(hotkey(KeyCode::Pause, Modifiers::NONE)));
    h.engine(|e, _| e.toast_clash());
    let events = h.take_events();
    assert!(events.iter().any(|e| matches!(e, EngineEvent::Toast(t) if t.key == "toast.hotkeyClash" && t.params["a"] == "manual" && t.params["b"] == "caseChange")));
    let c = check_hotkey(HotkeyTarget::SnippetPick, &hotkey(KeyCode::Pause, Modifiers::NONE), &h.settings());
    assert_eq!(c.conflicts_with, Some(HotkeyTarget::Manual));
}

// ---- Runtime ----

struct FakeHook {
    handler: Arc<Mutex<Option<Box<dyn InputHandler>>>>,
}

impl InputHook for FakeHook {
    fn start(&mut self, handler: Box<dyn InputHandler>) -> Result<(), PlatformError> {
        *self.handler.lock().unwrap() = Some(handler);
        Ok(())
    }
    fn stop(&mut self) {
        self.handler.lock().unwrap().take();
    }
    fn is_healthy(&self) -> bool {
        self.handler.lock().unwrap().is_some()
    }
}

#[test]
fn runtime_end_to_end() {
    fn handle_is_shareable<T: Clone + Send + Sync + 'static>() {}
    handle_is_shareable::<EngineHandle>();
    data::warm_up();
    let os = FakeOs::new();
    let platform = Platform {
        output: os.clone(),
        layouts: os.clone(),
        foreground: os.clone(),
        focused: os.clone(),
        clipboard: os.clone(),
    };
    let handler: Arc<Mutex<Option<Box<dyn InputHandler>>>> = Arc::default();
    let events: Arc<Mutex<Vec<EngineEvent>>> = Arc::default();
    let sink = {
        let events = events.clone();
        Arc::new(move |e| events.lock().unwrap().push(e))
    };
    let settings = Settings { live_fix_enabled: false, ..Settings::default() };
    let handle = start_with_clock(
        platform,
        Box::new(FakeHook { handler: handler.clone() }),
        settings,
        sink,
        Arc::new(SystemClock),
    )
    .unwrap();
    let wait = |cond: &dyn Fn() -> bool| {
        let t = Instant::now();
        while !cond() && t.elapsed() < Duration::from_secs(5) {
            std::thread::sleep(Duration::from_millis(5));
        }
    };
    // Layouts are loaded by the worker.
    wait(&|| handle.status().layout.is_some());
    assert!(handle.status().running && handle.status().hook_healthy);
    let t = us();
    for c in "ghbdtn ".chars() {
        let (key, _) = key_for(&t, c);
        for down in [true, false] {
            let ev = KeyEvent { key, down, repeat: false, mods: Modifiers::NONE, caps_lock: false, at: Instant::now() };
            let d = handler.lock().unwrap().as_mut().unwrap().on_key(&ev);
            if d == HookDecision::Pass && down {
                os.app_key(&ev);
            }
        }
    }
    let text = || os.os().field.text.iter().collect::<String>();
    wait(&|| text() == "привет ");
    assert_eq!(text(), "привет ");
    wait(&|| events.lock().unwrap().iter().any(|e| matches!(e, EngineEvent::SettingsChanged(s) if s.rescued_count == 1)));
    assert_eq!(handle.settings().rescued_count, 1);
    let st = handle.pause(15);
    assert!(st.paused_until.is_some());
    let s = handle.update_settings(serde_json::json!({ "typoFix": true })).unwrap();
    assert!(s.typo_fix && s.rescued_count == 1);
    assert!(handle.update_settings(serde_json::json!([1])).is_err());
    handle.shutdown();
    assert!(!handle.status().running);
    assert!(handler.lock().unwrap().is_none());
}
