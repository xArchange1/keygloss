//! Prints what the Windows backend sees. Never sends keystrokes; never prints typed characters.
//!
//! `probe`            layouts, key tables, foreground/running apps, clipboard count, password focus
//! `probe --hook`     also installs the hook for 3 s and logs key codes and down/up only
//! `probe --clipboard` also snapshots and restores the clipboard (contents unchanged)

use std::time::{Duration, Instant};

use keygloss_core::platform::*;
use keygloss_windows::*;

struct LogHandler(Instant);

impl InputHandler for LogHandler {
    fn on_key(&mut self, e: &KeyEvent) -> HookDecision {
        let dir = if e.down { "down" } else { "up" };
        let rep = if e.repeat { " (repeat)" } else { "" };
        println!("  +{:>4}ms {:?} {dir}{rep} mods={:?}", self.0.elapsed().as_millis(), e.key, e.mods);
        HookDecision::Pass
    }
    fn on_mouse_down(&mut self) {
        println!("  +{:>4}ms mouse down", self.0.elapsed().as_millis());
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let layouts = WindowsLayouts;

    println!("Installed layouts:");
    let installed = layouts.installed_layouts();
    for l in &installed {
        println!("  {} {} {}", l.id.0, l.lang, l.name);
    }
    println!("Current layout: {:?}", layouts.current_layout().map(|l| l.0));

    for l in &installed {
        let table = layouts.key_table(&l.id);
        print!("Key table {} ({} entries):", l.id.0, table.len());
        for key in [KeyCode::KeyQ, KeyCode::KeyG, KeyCode::Semicolon, KeyCode::Backquote, KeyCode::Digit2] {
            for shift in [false, true] {
                print!(" {:?}{}={:?}", key, if shift { "+Shift" } else { "" }, table.get(&(key, shift)));
            }
        }
        println!();
    }

    let fg = WindowsForegroundApp;
    println!("Foreground app: {:?}", fg.foreground_app().map(|a| a.0));
    let apps = fg.running_apps();
    println!("Running apps ({}):", apps.len());
    for a in &apps {
        println!("  {} ({})", a.id.0, a.name);
    }

    let clipboard = WindowsClipboard;
    println!("Clipboard change count: {}", clipboard.change_count());
    println!("Focused is password: {}", WindowsFocusedText.focused_is_password());
    let selection = match WindowsFocusedText.selected_text() {
        SelectionRead::Text(t) => format!("Text (len {})", t.chars().count()),
        other => format!("{other:?}"),
    };
    println!("Selected text: {selection}");

    if args.iter().any(|a| a == "--clipboard") {
        let before = clipboard.get_text().map(|t| t.chars().count());
        let snapshot = clipboard.snapshot();
        let bytes: usize = snapshot.formats.iter().map(|f| f.1.len()).sum();
        let ids: Vec<u32> = snapshot.formats.iter().map(|f| f.0).collect();
        println!("Clipboard snapshot: {} formats {ids:?}, {bytes} bytes, text len {before:?}", ids.len());
        let result = clipboard.restore(&snapshot);
        let after = clipboard.get_text().map(|t| t.chars().count());
        println!("Restore: {result:?}; text len after {after:?}; change count {}", clipboard.change_count());
    }

    if args.iter().any(|a| a == "--hook") {
        let mut hook = WindowsHook::new();
        let start = hook.start(Box::new(LogHandler(Instant::now())));
        println!("Hook start: {start:?}, healthy: {}", hook.is_healthy());
        std::thread::sleep(Duration::from_secs(3));
        hook.stop();
        println!("Hook stopped, healthy: {}", hook.is_healthy());
    }
}
