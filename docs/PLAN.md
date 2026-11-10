# Keygloss

Windows-only Tauri 2 rewrite of Keyboop (macOS, Swift, MIT): a RU↔EN wrong-layout auto-corrector (`ghbdtn` → `привет`). The MVP is built and verified. `docs/original-app-analysis.md` inventories the original's features.

## Rules

- **No voice features of any kind**: no dictation, microphone, voice settings or strings.
- No Keyboop name, icon or copy. No network calls (no keyboop.com, Telegram or donation links).
- Logs never contain typed text, only lengths.
- Keep `LICENSE` (MIT, xArchange1) and `THIRD_PARTY.md` (Keyboop and keyswitcher MIT notices).

## Not built

Translate, instant layout-switch key, clipboard history, Caps Lock LED, auto-update, feedback window, Basics/Everything modes, tray icon styles, macOS/Linux. Add one only when asked.

## Stack and layout

Tauri 2, Rust (MSVC), React + TS + Vite, Tailwind 4, Zustand, i18next, lucide-react, pnpm. No tauri-specta: command and event types are written by hand on both sides. The identifier is `com.keygloss.app`; settings are stored in `%APPDATA%\com.keygloss.app\settings.json`.

- `crates/keygloss-core/`: pure Rust with no OS calls. It holds:
  - platform traits (`platform.rs`) and the settings schema (`settings.rs`);
  - the detector, typo fixes, snippets and other rules;
  - the engine (`engine/`), tested against a fake platform;
  - embedded data (`data/`).
- `crates/keygloss-windows/`: the Windows implementation (low-level hooks, `SendInput`, layouts, UI Automation, clipboard, sound, non-activating windows).
- `src-tauri/`: app shell (engine wiring, tray, windows, persistence, IPC commands in `ipc.rs`).
- `src/`: frontend. The IPC contract is `src/lib/ipc.ts`; strings are in `src/locales/{en,ru}.ts`, and every user-facing string goes in both.

Adding a setting touches `settings.rs`, `ipc.ts` and both locale files. Parallel branches usually conflict there.

## Design

| Token | Dark | Light |
|---|---|---|
| Background gradient | `#2B2D30` → `#0B0C0D` | `#F3EEE7` → `#E4E5E3` |
| Body text | `#F3EEE7` | `#171819` |
| Accent | `#F5C84C` (gold) | `#8A6500` (ochre) |
| Detail color | `#8E9BFF` | `#3D48B8` |

Gold is an accent only, never light-theme body text. Use the detail color sparingly: status dots, badges, focus rings, links.

## Deliberate differences from the original

- Launch at login is off by default, and Quit has no confirmation.
- Snippet picker banks use Shift/Ctrl+digit, because Windows owns Win+digit.
- Exceptions add apps from the running-apps list only. That page also has an "Always switch" list.
- The double-space-period rule never fires, because Windows has no matching system setting.
- Password fields (UI Automation `IsPassword`) are never read. Keygloss does not run elevated, so admin windows are not corrected.

## Develop

- Checks: `cargo test --workspace`, `cargo clippy --workspace`, `pnpm build`.
- Run: `pnpm tauri dev`. Release build: `pnpm tauri build`.
- Only one instance runs at a time (single-instance plugin), and its global keyboard hook types into the live session. Live-test one branch at a time.
