# Keygloss MVP rewrite plan

Keygloss is a Tauri 2 rewrite of Keyboop (`reference/original-app`, macOS, Swift, MIT): a RU↔EN wrong-layout auto-corrector (`ghbdtn` → `привет`).

Read `docs/original-app-analysis.md` for the full inventory of the original. For exact algorithms, constants and rule order, read the Swift sources in `reference/original-app/Sources/Keyboop/`; the analysis abridges some of them.

## Hard rules

- **No voice features of any kind.** No dictation, microphone, audio input, call recording, voice history, voice hotkeys, voice settings, voice strings or voice permissions. Where original non-voice code touches voice code (section 13 of the analysis), cut the voice part out. The snippet picker has no "last dictation" row.
- Never edit anything under `reference/`.
- Logs never contain typed text, only lengths.
- Do not carry over the Keyboop name, icon, logo animation, mascot copy or version codenames.
- Do not call `keyboop.com`, its feedback endpoints, Telegram or donation links.
- Simplest code that meets the plan. No speculative features or abstractions.
- Implementers do not commit. The orchestrator commits after verifying each phase.

## Scope

Windows only. RU↔EN is the only shipped pair, but the engine is pair-agnostic.

**In**
- Automatic correction at a word boundary (Space, Enter, Tab triggers) and as-you-type live fix.
- Manual convert/undo hotkey. Default: **Pause/Break**. Modes: modifier chord, double-tap modifier, single modifier, key plus modifiers.
- Selection conversion and multi-word group conversion.
- Exceptions: ignored words, force-swap words, learned words, the "who wins" ambiguous pairs page, per-app off/soft modes, per-app forced layout, developer mode.
- Learn-from-undo, with the "Keep as is?" banner.
- Pause ("Do not disturb"): 15 min, 1 h, 3 h, 5 h.
- Conversion sound: "No sound" or the synthesized cue, with volume, plus a master mute.
- Snippets: autoreplace triggers and the hotkey-opened numbered picker.
- Typo fixes: typo table, mechanical swaps, two leading capitals, cAPS-lock word, personal dictionary.
- Case toggle hotkey. Paste-without-formatting hotkey.
- Welcome window, RU + EN interface (auto/ru/en), system/light/dark theme.
- Static tray icon with paused and auto-off variants; tray menu with the original's non-voice items.
- Launch at login, single instance, hotkey recorder with conflict checks.

**Out**
- Voice (see hard rules), translate, instant layout-switch key, clipboard history, Caps Lock LED, auto-update.
- Feedback window, Basics/Everything modes, tray icon styles, codenames, slap gesture, Globe/Fn and Caps remap machinery.
- macOS and Linux backends.
- macOS timing workarounds (tap watchdog and storm suspension, Spotlight, Figma and Gecko hacks, rival-tap scan). Port one only if Windows shows the same problem.

## Decisions

- Built-in per-app lists are translated to Windows process names: terminals off, IDEs soft, Chromium/Electron apps excluded from live fix.
- The macOS system-sound list is dropped.
- Keygloss does not run elevated, so it cannot correct text in admin windows.
- Password fields (UI Automation `IsPassword`) are never read or converted.
- Settings live in a JSON store (`tauri-plugin-store`), using the original's key names for the in-scope settings.

## Stack

- Tauri 2, Rust stable (MSVC).
- React + TypeScript + Vite, Tailwind 4, Zustand, i18next + react-i18next, lucide-react.
- pnpm.
- No tauri-specta. Commands and events get hand-written TypeScript types.
- Identifier `com.keygloss.app`. Product name `Keygloss`.

## Layout

```
Cargo.toml                  workspace
crates/keygloss-core/       pure Rust, no OS calls, unit-tested
crates/keygloss-windows/    Windows implementation of the platform traits
src-tauri/                  app: engine orchestration, tray, commands, persistence
src/                        frontend
docs/
LICENSE, THIRD_PARTY.md
```

**`keygloss-core`**
- `platform` module: the OS-agnostic traits and types the engine needs (key events and hook decisions, text injection, layout query/switch and key table, foreground app, selection access, clipboard).
- `LanguagePair`: everything pair-specific (base key table, dictionaries, trigram models, single-letter sets, ambiguity sets, pair-specific rules). RU↔EN is the only instance.
- Keymap, word buffer, detector, typo fix, undo learner, anti-resonance guard, double-space-period rule, snippet matching, exception store logic.
- Data files are embedded with `include_bytes!` from `crates/keygloss-core/data/` and parsed in a background warm-up.

**`keygloss-windows`**
- `WH_KEYBOARD_LL` hook on its own thread with a message loop; swallow by return value; key-up pairing for swallowed keys.
- `SendInput` with `KEYEVENTF_UNICODE` and backspaces, tagged through `dwExtraInfo` so the hook ignores its own events.
- Layout: read the foreground thread's layout, switch it, enumerate installed layouts, build the live key table with `ToUnicodeEx`.
- Foreground app tracking (process executable name) with change notification.
- UI Automation: selected text and password-field detection. Clipboard read/write with snapshot and restore for the copy fallback.

**`src-tauri`**
- Engine: buffer → detector → replacer; boundary, live-fix and manual paths; hotkeys and modifier gestures; pause; per-app policy; sound cue; snippets; case toggle; plain paste.
- Tray, settings persistence, autostart, single instance, commands and events for the frontend.

**Frontend**
- Settings window with the original's sidebar structure: Layout switching, Exceptions (with the "who wins" subpage), Snippets, General, Privacy, About.
- Welcome window, toast/banner window, snippet picker window (must not take focus), hotkey recorder.

## Design

Original layout (220 px sidebar, 600 px content, 32 px rows), new brand, placeholder icon.

| Token | Dark | Light |
|---|---|---|
| Background gradient | `#2B2D30` → `#0B0C0D` | `#F3EEE7` → `#E4E5E3` |
| Body text | `#F3EEE7` | `#171819` |
| Accent | `#F5C84C` (gold) | `#8A6500` (ochre) |
| Detail color | `#8E9BFF` | `#3D48B8` |

Gold is an accent only, never light-theme body text. The detail color is used sparingly: status dots, badges, focus rings, links.

## Data and attribution

- Reuse `words_en.json`, `words_ru.json`, `trigrams_en.json`, `trigrams_ru.json`, `typo_rules.json` and the in-code word lists from the original.
- `LICENSE`: MIT, `Copyright (c) 2026 xArchange1`.
- `THIRD_PARTY.md`: the full Keyboop MIT notice (`Copyright (c) 2026 Keyboop contributors`) and the keyswitcher MIT notice for the language data (`Copyright (c) 2026 Ilya Granin`).

## Execution

Implementers are Opus 5.5 subagents at medium effort. The orchestrator (Fable 5.2) writes briefs, reviews and verifies each phase, fixes small findings itself, and commits each verified phase on `main`.

1. **Scaffold** (1 agent): workspace, Tauri app, frontend skeleton, theme tokens, licensing, data files, and the shared contracts (`keygloss-core::platform`, settings schema, command and event types on both sides).
2. **Parallel** (3 agents, disjoint directories):
   - `crates/keygloss-core/`
   - `crates/keygloss-windows/`
   - `src/`
3. **Integration** (1 agent): `src-tauri/` engine orchestration, tray, persistence, wiring to the frontend.
4. **Verification** (orchestrator): `cargo test`, frontend build, `tauri build`, run the app.

## Status

- [x] Phase 1: scaffold
- [x] Phase 2: core, windows, frontend
- [x] Phase 3: integration
- [ ] Phase 4: verification
