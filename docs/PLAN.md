# Keygloss

Windows-only Tauri 2 rewrite of Keyboop (macOS, Swift, MIT): a RU↔EN wrong-layout auto-corrector (`ghbdtn` → `привет`). The MVP is built and verified. `docs/original-app-analysis.md` inventories the original's features.

## Rules

- **No voice features of any kind**: no dictation, microphone, voice settings or strings.
- No Keyboop name, icon or copy. No network calls (no keyboop.com, Telegram or donation links).
- Logs never contain typed text, only lengths.
- Keep `LICENSE` (MIT, xArchange1) and `THIRD_PARTY.md` (Keyboop and keyswitcher MIT notices).

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

Theme tokens live in `src/styles.css` (`--kg-*`, exposed as Tailwind colors). Gold is an accent only, never light-theme body text. Use the detail color sparingly: status dots, badges, focus rings, links.

## Develop

- Checks: `cargo test --workspace`, `cargo clippy --workspace`, `pnpm build`.
- Run: `pnpm tauri dev`. Release build: `pnpm tauri build`.
- Only one instance runs at a time (single-instance plugin), and its global keyboard hook types into the live session. Live-test one branch at a time.
