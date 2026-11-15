//! `Layouts` via KDE Plasma's keyboard daemon on D-Bus (`org.kde.keyboard /Layouts`), which
//! works the same on Wayland and X11. Other desktops have no layouts here: the engine then only
//! fixes text it can read, without switching.
//!
//! Layout ids are XKB names, `us` or `ru(phonetic)` (variants come from `~/.config/kxkbrc`).
//! Key tables are built with libxkbcommon from those names. Everything is cached, refreshed on
//! the daemon's `layoutChanged` and `layoutListChanged` signals.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use keygloss_core::platform::{KeyTable, LayoutId, LayoutInfo, Layouts, PlatformError};
use xkbcommon::xkb;

use crate::keymap::{KEYS, is_char_key};

#[zbus::proxy(interface = "org.kde.KeyboardLayouts", default_service = "org.kde.keyboard", default_path = "/Layouts")]
trait KeyboardLayouts {
    #[zbus(name = "getLayout")]
    fn get_layout(&self) -> zbus::Result<u32>;
    #[zbus(name = "setLayout")]
    fn set_layout(&self, index: u32) -> zbus::Result<bool>;
    #[zbus(name = "getLayoutsList")]
    fn get_layouts_list(&self) -> zbus::Result<Vec<(String, String, String)>>;
    #[zbus(signal, name = "layoutChanged")]
    fn layout_changed(&self, index: u32) -> zbus::Result<()>;
    #[zbus(signal, name = "layoutListChanged")]
    fn layout_list_changed(&self) -> zbus::Result<()>;
}

#[derive(Default)]
struct Cache {
    /// Index of the current layout in `installed`.
    current: Option<usize>,
    installed: Vec<LayoutInfo>,
    tables: HashMap<LayoutId, KeyTable>,
}

static CACHE: Mutex<Option<Cache>> = Mutex::new(None);
static PROXY: OnceLock<Option<KeyboardLayoutsProxyBlocking<'static>>> = OnceLock::new();

fn cache<T>(f: impl FnOnce(&mut Cache) -> T) -> T {
    let mut c = CACHE.lock().unwrap_or_else(|p| p.into_inner());
    f(c.get_or_insert_with(Cache::default))
}

/// Connects (once), fills the cache and starts the signal listeners. `None` off KDE.
fn proxy() -> Option<&'static KeyboardLayoutsProxyBlocking<'static>> {
    PROXY
        .get_or_init(|| {
            let conn = zbus::blocking::Connection::session().ok()?;
            let proxy = KeyboardLayoutsProxyBlocking::new(&conn).ok()?;
            proxy.get_layout().ok()?;
            refresh(&proxy);
            if let Ok(changes) = proxy.receive_layout_changed() {
                std::thread::spawn(move || {
                    for signal in changes {
                        if let Ok(args) = signal.args() {
                            cache(|c| c.current = Some(args.index as usize));
                        }
                    }
                });
            }
            let p = proxy.clone();
            if let Ok(changes) = proxy.receive_layout_list_changed() {
                std::thread::spawn(move || {
                    for _ in changes {
                        refresh(&p);
                    }
                });
            }
            Some(proxy)
        })
        .as_ref()
}

/// `(layout, variant)` pairs from `~/.config/kxkbrc`, if it lists any.
fn kxkbrc_layouts() -> Vec<(String, String)> {
    let Some(home) = std::env::var_os("XDG_CONFIG_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| std::path::Path::new(&h).join(".config")))
    else {
        return Vec::new();
    };
    let Ok(text) = std::fs::read_to_string(home.join("kxkbrc")) else { return Vec::new() };
    parse_kxkbrc(&text)
}

fn parse_kxkbrc(text: &str) -> Vec<(String, String)> {
    let value = |key: &str| {
        text.lines().find_map(|l| l.strip_prefix(key)?.strip_prefix('=')).map(|v| v.split(',').map(str::trim).map(String::from).collect::<Vec<_>>())
    };
    let layouts = value("LayoutList").unwrap_or_default();
    let variants = value("VariantList").unwrap_or_default();
    layouts
        .into_iter()
        .filter(|l| !l.is_empty())
        .enumerate()
        .map(|(i, l)| (l, variants.get(i).cloned().unwrap_or_default()))
        .collect()
}

fn layout_id(layout: &str, variant: &str) -> LayoutId {
    LayoutId(if variant.is_empty() { layout.into() } else { format!("{layout}({variant})") })
}

/// Language of an XKB layout name: the country codes of the common layouts, the name otherwise.
fn lang_of(layout: &str) -> String {
    match layout {
        "us" | "gb" | "au" | "ie" | "nz" | "za" => "en",
        "ua" => "uk",
        "by" => "be",
        "kz" => "kk",
        "ge" => "ka",
        "am" => "hy",
        "il" => "he",
        other => other,
    }
    .into()
}

/// Characters of each character key of one XKB layout, without and with Shift.
fn key_table(layout: &str, variant: &str) -> KeyTable {
    let mut table = KeyTable::new();
    let ctx = xkb::Context::new(xkb::CONTEXT_NO_FLAGS);
    let Some(keymap) = xkb::Keymap::new_from_names(&ctx, "evdev", "pc105", layout, variant, None, xkb::KEYMAP_COMPILE_NO_FLAGS)
    else {
        return table;
    };
    let shift = 1 << keymap.mod_get_index(xkb::MOD_NAME_SHIFT);
    let mut state = xkb::State::new(&keymap);
    for shifted in [false, true] {
        state.update_mask(if shifted { shift } else { 0 }, 0, 0, 0, 0, 0);
        for &(key, code) in KEYS.iter().filter(|e| is_char_key(e.0)) {
            if let Some(c) = char::from_u32(state.key_get_utf32(xkb::Keycode::new(u32::from(code) + 8)))
                && c != '\0'
                && !c.is_control()
            {
                table.insert((key, shifted), c);
            }
        }
    }
    table
}

/// Re-reads the layout list, the current layout and the key tables.
fn refresh(proxy: &KeyboardLayoutsProxyBlocking<'_>) {
    let Ok(list) = proxy.get_layouts_list() else { return };
    let configured = kxkbrc_layouts();
    let mut installed = Vec::new();
    let mut tables = HashMap::new();
    for (i, (short, _display, long)) in list.iter().enumerate() {
        // The daemon gives no variants: take them from the config when it lists the same layouts.
        let (layout, variant) = match configured.get(i) {
            Some((l, v)) if configured.len() == list.len() => (l.clone(), v.clone()),
            _ => (short.clone(), String::new()),
        };
        let id = layout_id(&layout, &variant);
        tables.insert(id.clone(), key_table(&layout, &variant));
        installed.push(LayoutInfo { id, lang: lang_of(&layout), name: long.clone() });
    }
    let current = proxy.get_layout().ok().map(|i| i as usize);
    cache(|c| {
        c.installed = installed;
        c.tables = tables;
        c.current = current;
    });
}

/// The current layout and every layout's key table, for typing.
pub(crate) fn snapshot() -> (Option<LayoutId>, Vec<(LayoutId, KeyTable)>) {
    proxy();
    cache(|c| {
        let current = c.current.and_then(|i| c.installed.get(i)).map(|l| l.id.clone());
        let tables = c.installed.iter().map(|l| (l.id.clone(), c.tables.get(&l.id).cloned().unwrap_or_default())).collect();
        (current, tables)
    })
}

/// Switches and waits for the daemon's answer: the desktop applies the layout before it handles
/// the keys typed after this returns.
pub(crate) fn switch(id: &LayoutId) -> Result<(), PlatformError> {
    let proxy = proxy().ok_or_else(|| PlatformError("no layout service (KDE Plasma only)".into()))?;
    let index = cache(|c| c.installed.iter().position(|l| &l.id == id))
        .ok_or_else(|| PlatformError(format!("unknown layout {:?}", id.0)))?;
    match proxy.set_layout(index as u32) {
        Ok(true) => {
            cache(|c| c.current = Some(index));
            Ok(())
        }
        Ok(false) => Err(PlatformError(format!("layout {:?} refused", id.0))),
        Err(e) => Err(PlatformError(format!("setLayout: {e}"))),
    }
}

/// `Layouts` implementation.
#[derive(Clone, Copy, Debug, Default)]
pub struct LinuxLayouts;

impl LinuxLayouts {
    /// Connects to the layout service and fills the cache.
    pub fn new() -> Self {
        proxy();
        LinuxLayouts
    }
}

impl Layouts for LinuxLayouts {
    fn current_layout(&self) -> Option<LayoutId> {
        snapshot().0
    }

    fn switch_layout(&self, id: &LayoutId) -> Result<(), PlatformError> {
        switch(id)
    }

    fn installed_layouts(&self) -> Vec<LayoutInfo> {
        proxy();
        cache(|c| c.installed.clone())
    }

    fn key_table(&self, id: &LayoutId) -> KeyTable {
        proxy();
        cache(|c| c.tables.get(id).cloned().unwrap_or_default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use keygloss_core::platform::KeyCode;

    #[test]
    fn reads_kxkbrc() {
        let text = "[Layout]\nDisplayNames=,\nLayoutList=us,ru\nUse=true\nVariantList=,phonetic\n";
        assert_eq!(parse_kxkbrc(text), vec![("us".into(), String::new()), ("ru".into(), "phonetic".into())]);
        assert!(parse_kxkbrc("[Layout]\nUse=false\n").is_empty());
        assert_eq!(layout_id("ru", "phonetic").0, "ru(phonetic)");
        assert_eq!(layout_id("us", "").0, "us");
    }

    #[test]
    fn builds_key_tables() {
        let us = key_table("us", "");
        assert_eq!(us.get(&(KeyCode::KeyQ, false)), Some(&'q'));
        assert_eq!(us.get(&(KeyCode::Digit2, true)), Some(&'@'));
        let ru = key_table("ru", "");
        assert_eq!(ru.get(&(KeyCode::KeyQ, false)), Some(&'й'));
        assert_eq!(ru.get(&(KeyCode::KeyQ, true)), Some(&'Й'));
        assert_eq!(ru.get(&(KeyCode::Backquote, false)), Some(&'ё'));
        assert_eq!(ru.get(&(KeyCode::Slash, true)), Some(&','));
    }
}
