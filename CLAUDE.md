# Keygloss

Windows, macOS and (experimental) Linux Tauri 2 rewrite of Keyboop (macOS-only, Swift, MIT): a RU↔EN wrong-layout auto-corrector (`ghbdtn` → `привет`). The MVP is built and verified.

- The original is cloned at `C:\dev\keygloss\reference\original-app` (https://github.com/iffuno/keyboop). It is gitignored, so it is missing from worktrees: read it at that absolute path. Never edit it.
- `docs/original-app-analysis.md` (68 KB) is a reference for the original's behaviour: do not read it whole, grep its section headings only when porting or checking parity with an original feature.
- After filing a PR, queue it with `gh pr merge --auto --merge` and move on: do not wait for CI. Run the checks under Develop locally first.
- Admins can bypass the `main` ruleset. Push straight to `main` only when asked to; otherwise file a PR.

## Rules

- **No voice features of any kind**: no dictation, microphone, voice settings or strings.
- No Keyboop name, icon or copy, and none of Keyboop's endpoints or links (keyboop.com, its Telegram or donation pages). The app makes no network calls except the updater's requests to GitHub (`src-tauri/src/update.rs`). Links open in the browser: the two GitHub issue forms (`report.rs`) and the README's Sponsor section (Boosty, T-Bank), all on the About page.
- Logs never contain typed text, only lengths. The log is `%LOCALAPPDATA%\Keygloss\logs\keygloss.log` (macOS: `~/Library/Logs/com.keygloss.app/keygloss.log`; `src-tauri/src/log.rs`); the engine writes to it through `EngineEvent::Log`. The bug form gets its newest lines in the URL, so they become public once submitted.
- The only typed text kept is the History page (off by default, `historyEnabled`): the last 200 automatic fixes in `%APPDATA%\Keygloss\history.json` (macOS: `~/Library/Application Support/com.keygloss.app/`) (`src-tauri/src/history.rs`, fed by `EngineEvent::Converted`). It stays local: never log it or put it in a report.
- The original is MIT licensed: keep its copyright notice and license text in this project's licensing/attribution. Keep `LICENSE` (MIT, xArchange1) and `THIRD_PARTY.md` (Keyboop and keyswitcher MIT notices).

## Layout

No tauri-specta: command and event types are written by hand on both sides. The identifier is `com.keygloss.app`; settings are stored in `%APPDATA%\Keygloss\settings.json`. On Windows every app dir comes from `src-tauri/src/paths.rs`: a set `XDG_CONFIG_HOME`, `XDG_DATA_HOME`, `XDG_STATE_HOME` or `XDG_CACHE_HOME` moves its files to `<dir>\keygloss`, and the first start moves files from the old `com.keygloss.app` dirs. The NSIS uninstaller's "Delete the application data" box deletes them all (`src-tauri/windows/hooks.nsh`).

- `crates/keygloss-core/`: pure Rust with no OS calls. It holds:
  - platform traits (`platform.rs`) and the settings schema (`settings.rs`);
  - the detector, typo fixes, snippets and other rules;
  - the engine (`engine/`), tested against a fake platform;
  - embedded data (`data/`).
- `crates/keygloss-windows/`: the Windows implementation (low-level hooks, `SendInput`, layouts, UI Automation, clipboard, non-activating windows).
- `crates/keygloss-macos/`: the macOS implementation (`CGEventTap` on its own run-loop thread, `CGEventPost`, Text Input Sources cached on the main thread, Accessibility API, `NSPasteboard`, `NSWorkspace`). The hook needs the Accessibility permission: without it the engine runs with `hookHealthy: false` and Settings shows a button to the permission pane. Copy and paste chords use `Modifiers::SHORTCUT` (Command on macOS). App ids are lowercase bundle ids there. Its code cross-checks from Windows with `cargo clippy -p keygloss-macos --target aarch64-apple-darwin` (after `rustup target add aarch64-apple-darwin`); the app crate only builds on a Mac or the CI's macOS job.
- `crates/keygloss-linux/`: the Linux implementation, for KDE Plasma 6 on Wayland. The hook grabs the keyboards with evdev and re-sends their keys through a uinput virtual keyboard (`vkbd.rs`), which also types the output and whose LEDs (Caps Lock) the hook mirrors to the grabbed keyboards; a watchdog ungrabs them if the hook thread stalls. Layouts come from KDE's `org.kde.keyboard` D-Bus service with libxkbcommon key tables, the foreground app from a KWin script that calls back over D-Bus, the clipboard from wlr-data-control (`wl-clipboard-rs`). No AT-SPI: `FocusedText` is unavailable. Access to `/dev/input` and `/dev/uinput` comes from the package's udev `uaccess` rule (`src-tauri/linux/`). It builds and tests in WSL (Ubuntu, with the Tauri Linux prerequisites and `libxkbcommon-dev`), but has no input devices there.
- `src-tauri/src/os.rs` picks the platform crate; the banner and the picker are non-activating `NSPanel`s on macOS (`tauri-nspanel`). On Linux the app runs under XWayland (`GDK_BACKEND=x11`), so those windows can be placed and do not take focus.
- `src-tauri/`: app shell (engine wiring, tray, windows, persistence, sound, IPC commands in `ipc.rs`).
- `src/`: frontend. The IPC contract is `src/lib/ipc.ts`; strings are in `src/locales/{en,ru}.ts`, and every user-facing string goes in both.

Adding a setting touches `settings.rs`, `ipc.ts` and both locale files. Parallel branches usually conflict there.

## Design

Theme tokens live in `src/styles.css` (`--kg-*`, exposed as Tailwind colors). Gold is an accent only, never light-theme body text. Use the detail color sparingly: status dots, badges, focus rings, links.

## Develop

- Checks: `cargo test --workspace`, `cargo clippy --workspace`, `pnpm build`.
- Before running the app from the main checkout (`C:\dev\keygloss`), run `git pull --ff-only` there: merged PRs don't update it.
- Run: `pnpm tauri dev`. Verify changes with this debug build: it rebuilds in seconds. For a standalone exe, `pnpm tauri build --debug --no-bundle` (about 20 s). `pnpm tauri build` takes about 100 s (LTO, one codegen unit): run it only for installer, updater or release-profile work.
- Waiting for the dev app: grep the dev log for `Running.*keygloss|error` and stop after 60 s (3 min for the first build in a checkout). Do not match `Running ` with a space: cargo colours its output, so an ANSI code follows the word and the pattern never matches.
- Release: set the new version in `Cargo.toml`, `package.json` and `src-tauri/tauri.conf.json`, add its notes to `src/changelog.ts` (What's new, shown once after an update; the release workflow also turns them into the release body, which the update banner's What's new dialog shows), then push a tag `v<version>`. `.github/workflows/release.yml` builds the NSIS and MSI installers with their updater signatures (pre-releases: NSIS only, since WiX rejects non-numeric pre-release versions), then a universal macOS `.app`/`.dmg` (`tauri.release.macos.conf.json`) and Linux `.deb`/`.rpm` (`tauri.release.linux.conf.json`; pre-releases: `.deb` only), and publishes the GitHub release the in-app updater reads. A version with a suffix (`0.2.0-beta.1`) becomes a pre-release, which only the beta channel offers. The updater signing key is the repository secret `TAURI_SIGNING_PRIVATE_KEY`; its public half is in `tauri.conf.json`. Losing it means installed copies can no longer update.
- macOS signing: with the secrets `APPLE_CERTIFICATE` (base64 .p12 of a Developer ID Application certificate), `APPLE_CERTIFICATE_PASSWORD`, `APPLE_SIGNING_IDENTITY`, `APPLE_ID`, `APPLE_PASSWORD` (app-specific password) and `APPLE_TEAM_ID`, the release job signs and notarizes the app. Without them it is signed with the self-signed certificate in `MACOS_CERTIFICATE` (base64 .p12, common name `Keygloss Self-Signed`) and `MACOS_CERTIFICATE_PASSWORD`: Gatekeeper blocks the first launch of a download (System Settings > Privacy & Security > Open Anyway), but the Accessibility grant survives updates as long as the certificate stays the same. A new certificate, or ad-hoc signing without one, makes users grant it again. The official homebrew-cask tap only takes signed, notarized apps.
- Linux packages do not self-update: the app checks the GitHub API and its install button opens the release page. `packaging/linux/install.sh` (the README's curl command) installs or updates from the latest stable release.
- Homebrew: `packaging/homebrew/keygloss.rb` is the cask for a tap repository (`xArchange1/homebrew-tap`, file `Casks/keygloss.rb`). Each stable release pushes it there with the version and the .dmg's sha256 set, through the write deploy key whose private half is the secret `HOMEBREW_TAP_DEPLOY_KEY`: edit the cask here, not in the tap.
- Only one instance runs at a time (single-instance plugin), and its global keyboard hook types into the live session. Live-test one branch at a time.
