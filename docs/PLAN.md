# Keygloss

Windows-only Tauri 2 rewrite of Keyboop (macOS, Swift, MIT): a RU↔EN wrong-layout auto-corrector (`ghbdtn` → `привет`). The MVP is built and verified. `docs/original-app-analysis.md` (68 KB) is a reference for the original's behaviour: do not read it whole, grep its section headings only when porting or checking parity with an original feature.

## Rules

- **No voice features of any kind**: no dictation, microphone, voice settings or strings.
- No Keyboop name, icon or copy, and none of Keyboop's endpoints or links (keyboop.com, its Telegram or donation pages). The app makes no network calls except the updater's requests to GitHub (`src-tauri/src/update.rs`). Links open in the browser: the two GitHub issue forms (`report.rs`) and Keygloss's own donation link, all on the About page.
- Logs never contain typed text, only lengths. The log is `%LOCALAPPDATA%\com.keygloss.app\logs\keygloss.log` (`src-tauri/src/log.rs`); the engine writes to it through `EngineEvent::Log`. The bug form gets its newest lines in the URL, so they become public once submitted.
- The only typed text kept is the History page: the last 200 automatic fixes in `%APPDATA%\com.keygloss.app\history.json` (`src-tauri/src/history.rs`, fed by `EngineEvent::Converted`). It stays local: never log it or put it in a report.
- Keep `LICENSE` (MIT, xArchange1) and `THIRD_PARTY.md` (Keyboop and keyswitcher MIT notices).

## Stack and layout

Tauri 2, Rust (MSVC), React + TS + Vite, Tailwind 4, Zustand, i18next, lucide-react, pnpm. No tauri-specta: command and event types are written by hand on both sides. The identifier is `com.keygloss.app`; settings are stored in `%APPDATA%\com.keygloss.app\settings.json`.

- `crates/keygloss-core/`: pure Rust with no OS calls. It holds:
  - platform traits (`platform.rs`) and the settings schema (`settings.rs`);
  - the detector, typo fixes, snippets and other rules;
  - the engine (`engine/`), tested against a fake platform;
  - embedded data (`data/`).
- `crates/keygloss-windows/`: the Windows implementation (low-level hooks, `SendInput`, layouts, UI Automation, clipboard, non-activating windows).
- `src-tauri/`: app shell (engine wiring, tray, windows, persistence, sound, IPC commands in `ipc.rs`).
- `src/`: frontend. The IPC contract is `src/lib/ipc.ts`; strings are in `src/locales/{en,ru}.ts`, and every user-facing string goes in both.

Adding a setting touches `settings.rs`, `ipc.ts` and both locale files. Parallel branches usually conflict there.

## Design

Theme tokens live in `src/styles.css` (`--kg-*`, exposed as Tailwind colors). Gold is an accent only, never light-theme body text. Use the detail color sparingly: status dots, badges, focus rings, links.

## Develop

- Checks: `cargo test --workspace`, `cargo clippy --workspace`, `pnpm build`.
- Run: `pnpm tauri dev`. Release build: `pnpm tauri build`.
- Release: set the new version in `Cargo.toml`, `package.json` and `src-tauri/tauri.conf.json`, then push a tag `v<version>`. `.github/workflows/release.yml` builds the signed NSIS installer and publishes the GitHub release the in-app updater reads. A version with a suffix (`0.2.0-beta.1`) becomes a pre-release, which only the beta channel offers. The updater signing key is the repository secret `TAURI_SIGNING_PRIVATE_KEY`; its public half is in `tauri.conf.json`. Losing it means installed copies can no longer update.
- Only one instance runs at a time (single-instance plugin), and its global keyboard hook types into the live session. Live-test one branch at a time.
