//! `Layouts` via Text Input Sources. TIS is main-thread-only, but the engine asks from the hook
//! and worker threads, so everything is read on the main thread into a cache: at start and on
//! the "selected / enabled input sources changed" notifications. Switching is sent to the main
//! thread.

use std::collections::HashMap;
use std::ffi::c_void;
use std::ptr;
use std::sync::{Mutex, Once};

use keygloss_core::platform::{KeyTable, LayoutId, LayoutInfo, Layouts, PlatformError};

use crate::cf::*;
use crate::keymap::{KEYS, is_char_key};

#[derive(Default)]
struct Cache {
    current: Option<LayoutId>,
    installed: Vec<LayoutInfo>,
    tables: HashMap<LayoutId, KeyTable>,
}

static CACHE: Mutex<Option<Cache>> = Mutex::new(None);
static OBSERVE: Once = Once::new();

fn cache<T>(f: impl FnOnce(&mut Cache) -> T) -> T {
    let mut c = CACHE.lock().unwrap_or_else(|p| p.into_inner());
    f(c.get_or_insert_with(Cache::default))
}

fn source_id(source: TISInputSourceRef) -> Option<LayoutId> {
    string(unsafe { TISGetInputSourceProperty(source, kTISPropertyInputSourceID) }).map(LayoutId)
}

/// Enabled, selectable keyboard layouts (+1 array of sources), in the system order.
fn enabled_sources() -> Option<Owned> {
    let filter = unsafe {
        dict(&[
            (kTISPropertyInputSourceCategory, kTISCategoryKeyboardInputSource),
            (kTISPropertyInputSourceIsSelectCapable, kCFBooleanTrue),
        ])
    };
    Owned::new(unsafe { TISCreateInputSourceList(filter.0, false) })
}

fn is_layout(source: TISInputSourceRef) -> bool {
    let ty = string(unsafe { TISGetInputSourceProperty(source, kTISPropertyInputSourceType) });
    ty.is_some() && ty == string(unsafe { kTISTypeKeyboardLayout })
}

/// Characters of each character key, without and with Shift, from the layout's `uchr` data.
fn key_table(source: TISInputSourceRef) -> KeyTable {
    let mut table = KeyTable::new();
    let data = unsafe { TISGetInputSourceProperty(source, kTISPropertyUnicodeKeyLayoutData) };
    if data.is_null() {
        return table;
    }
    let layout = unsafe { CFDataGetBytePtr(data) };
    let kbd_type = u32::from(unsafe { LMGetKbdType() });
    const SHIFT: u32 = 0x0200 >> 8; // (shiftKey >> 8) & 0xFF
    const NO_DEAD_KEYS: u32 = 1; // kUCKeyTranslateNoDeadKeysMask
    for &(key, code) in KEYS.iter().filter(|e| is_char_key(e.0)) {
        for shift in [false, true] {
            let (mut dead, mut len, mut buf) = (0u32, 0usize, [0u16; 4]);
            let status = unsafe {
                UCKeyTranslate(
                    layout,
                    code,
                    0, // kUCKeyActionDown
                    if shift { SHIFT } else { 0 },
                    kbd_type,
                    NO_DEAD_KEYS,
                    &mut dead,
                    buf.len(),
                    &mut len,
                    buf.as_mut_ptr(),
                )
            };
            if status == 0
                && len == 1
                && let Some(c) = char::from_u32(u32::from(buf[0]))
                && !c.is_control()
            {
                table.insert((key, shift), c);
            }
        }
    }
    table
}

/// Re-reads the current layout. Main thread only.
fn refresh_current() {
    let current = Owned::new(unsafe { TISCopyCurrentKeyboardInputSource() }).and_then(|s| source_id(s.0));
    cache(|c| c.current = current);
}

/// Re-reads the installed layouts and their key tables. Main thread only.
fn refresh_installed() {
    let Some(list) = enabled_sources() else { return };
    let mut installed = Vec::new();
    let mut tables = HashMap::new();
    for source in array(list.0).into_iter().filter(|&s| is_layout(s)) {
        let Some(id) = source_id(source) else { continue };
        let name = string(unsafe { TISGetInputSourceProperty(source, kTISPropertyLocalizedName) }).unwrap_or_default();
        let langs = unsafe { TISGetInputSourceProperty(source, kTISPropertyInputSourceLanguages) };
        let lang = array(langs).first().and_then(|&l| string(l)).unwrap_or_default();
        tables.insert(id.clone(), key_table(source));
        installed.push(LayoutInfo { id, lang, name });
    }
    cache(|c| {
        c.installed = installed;
        c.tables = tables;
    });
}

extern "C" fn on_selected_changed(_: *const c_void, _: *mut c_void, _: CFStringRef, _: *const c_void, _: CFDictionaryRef) {
    refresh_current();
}

extern "C" fn on_enabled_changed(_: *const c_void, _: *mut c_void, _: CFStringRef, _: *const c_void, _: CFDictionaryRef) {
    refresh_installed();
    refresh_current();
}

/// `Layouts` implementation.
#[derive(Clone, Copy, Debug, Default)]
pub struct MacLayouts;

impl MacLayouts {
    /// Fills the cache and subscribes to input source changes (on the main thread; at once if
    /// called there).
    pub fn new() -> Self {
        on_main(|| {
            OBSERVE.call_once(|| unsafe {
                let center = CFNotificationCenterGetDistributedCenter();
                for (name, callback) in [
                    (kTISNotifySelectedKeyboardInputSourceChanged, on_selected_changed as _),
                    (kTISNotifyEnabledKeyboardInputSourcesChanged, on_enabled_changed as _),
                ] {
                    CFNotificationCenterAddObserver(
                        center,
                        ptr::null(),
                        callback,
                        name,
                        ptr::null(),
                        CFNotificationSuspensionBehaviorDeliverImmediately,
                    );
                }
            });
            refresh_installed();
            refresh_current();
        });
        MacLayouts
    }
}

impl Layouts for MacLayouts {
    fn current_layout(&self) -> Option<LayoutId> {
        cache(|c| c.current.clone())
    }

    /// Records the switch at once (the engine reads it back right away) and asks the main
    /// thread to select the source; the notification then confirms it.
    fn switch_layout(&self, id: &LayoutId) -> Result<(), PlatformError> {
        if !cache(|c| c.installed.iter().any(|l| &l.id == id)) {
            return Err(PlatformError(format!("unknown layout {:?}", id.0)));
        }
        cache(|c| c.current = Some(id.clone()));
        let id = id.clone();
        on_main(move || {
            let Some(list) = enabled_sources() else { return };
            if let Some(&source) = array(list.0).iter().find(|&&s| source_id(s).as_ref() == Some(&id)) {
                unsafe { TISSelectInputSource(source) };
            }
            refresh_current();
        });
        Ok(())
    }

    fn installed_layouts(&self) -> Vec<LayoutInfo> {
        cache(|c| c.installed.clone())
    }

    fn key_table(&self, id: &LayoutId) -> KeyTable {
        cache(|c| c.tables.get(id).cloned().unwrap_or_default())
    }
}
