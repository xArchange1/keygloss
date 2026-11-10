# Keyboop (`reference/original-app`): inventory for the Tauri rewrite

## 1. Purpose and overview

**Purpose.** Keyboop watches what you type. When a word was obviously typed in the wrong layout (`ghbdtn` instead of `привет`), it deletes and retypes the word in the right layout and switches the OS input source. It works both ways (RU↔EN).

Other features:
- Manual hotkey fixes and a hotkey undo.
- Typo fixes, snippets, a selection translator, a plain-paste hotkey, a case toggle, and instant layout switching.
- A clipboard history that is stored together with voice history.
- It never uses the clipboard to fix words; it synthesizes key events.

**Interaction.**
- It is a menu-bar agent with `LSUIElement=true` and `app.setActivationPolicy(.accessory)` (`main.swift`).
- The menu-bar item is a left-click menu and a right-click quick action.
- Windows: Settings, Welcome/onboarding, Feedback, a dictation-history window (voice, excluded), and floating panels (snippet picker, banner/toast).
- **A Dock icon appears while Settings or History is open.** `DockPresence.swift` switches the activation policy to `.regular` for that time and back to `.accessory` afterwards. History gets a "History" badge on the Dock icon.

**Lifecycle.**
- **Single instance.** `AppDelegate.acquireSingleInstanceLock` (`AppDelegate.swift:1150`) takes an `flock` on `~/Library/Application Support/Keyboop/singleton.lock`, with 10 retries at 200 ms.
  - A second launch posts a distributed notification `ru.keyboop.app.openSettings` and exits after 0.15 s.
  - The first instance opens Settings only if it has been up for more than 15 s (login-race guard).
- **First-run initial setup** (`AppDelegate.swift:295`):
  - `didInitialSetup=true`
  - `launchAtLogin=true`
  - `voiceHoldMode="toggle"`
  - `simpleMode=true`
- **Separate one-time login seed** (`didLoginSeed`): enables launch-at-login once if it was off.
- **Welcome window.** Shown once (`didShowWelcome`), 0.8 s after launch.
- **Engine start.** `tryStart()` runs immediately, then on a 0.5 s retry timer until the event tap is created. It also retries on the `com.apple.accessibility.api` distributed notification.
- **Engine watchdog.** A 2 s timer checks the tap.
  - Revival back-off is `2 << revivals` seconds.
  - Maximum 5 consecutive revivals, maximum 12 per session.
  - The back-off resets after 15 healthy ticks (30 s).
  - After the ceiling it shows the "needs permission" state.
- **Quit.**
  - Menu "Quit" shows a confirm alert ("Quit Keyboop?", buttons "Stay" as default and "Quit").
  - SIGTERM and SIGINT are handled: they remove the Caps remap, shut down the Caps LED, finish the helper, and `exit(0)`.
  - `applicationWillTerminate` does the same cleanup.
- **Reopen** (`applicationShouldHandleReopen`): opens Settings, or History if it is frontmost.
- **Startup checks:**
  - Punto Switcher conflict alert (2.5 s after launch). It matches any running app whose bundle ID or name contains "punto". Buttons are "Quit Punto", "Login items" and "Later".
  - A hotkey-clash banner (2 s after launch).
- **Translocation / wrong install location.** If the bundle path contains `/AppTranslocation/`, a "move to Applications" alert is shown once, and the engine does not start.
- **Relaunch offer.** If Accessibility is trusted but the tap still fails after 6 retries, it offers a relaunch.

**Dev/test environment hooks (drop in the rewrite).** `AppDelegate.applicationDidFinishLaunching` handles about 56 `KEYBOOP_*` environment variables (screenshot dumpers, fake secure input, and so on), plus `KEYBOOP_DATA_DIR` and `--settings*` CLI args.

---

## 2. Feature list

Defaults come from `AppSettings.swift:72` (`register(defaults:)`) or from the accessor fallbacks. A boolean not listed as registered defaults to false.

**A. Automatic layout correction (Settings → "Layout switching")**

| Label | Key | Default | Notes |
|---|---|---|---|
| "Automatic layout correction" | `autoEnabled` | true | "Two-way, RU ↔ EN. The word is fixed once you finish it". Also the menu item "Auto-switch" (key equivalent `a`). |
| "Fix as you type" | `liveFixEnabled` | true | Fixes mid-word. See 3.3. |
| (hidden) | `inlineLiveFix` | true | Atomic replacement in the tap callback. Falls back to a non-atomic path when off. |
| Triggers "After a word": Space, Enter, Tab | `triggerSpace`, `triggerEnter`, `triggerTab` | true, true, **false** | Boundary keys that fire auto-conversion. |
| (no UI label found) | `enterPreConvert` | true | Convert before Enter is sent. |
| "Arrow keys cancel the switch" | `arrowsCancel` | true | |
| "Mode for developers" | `developerMode` | false | Auto fully off in IDEs/terminals; protects short tokens. |
| "Swallow key chatter" | `dedupeChatter` | false | Drops the same key repeating within 30 ms. |
| "Fix typos" | `typoFix` | false | See 3.7. |
| "Two leading capitals" | `twoCapsFix` | false | "WHen" becomes "When". |
| "Convert multiple words" | `groupConvert` | true | Group convert via manual hotkey. Only when auto is off ("Available when auto-switch is off"). |
| "Learn from undo" | `learnOnUndoEnabled` | true | See 3.6. |
| "Sound on switch" / "Sound" / "Volume" | `soundEnabled`, `soundName`, `soundVolume` | true, "Pop", 0.6 | See 3.8. |

**B. Manual hotkey (fix last word or selection, or undo)**
- Label: "Manual switch and undo". Sub-label: "Switch the last word, or undo an automatic switch".
- Keys: `hotkeyMode`, `hotkeyKeyCode` (-1), `hotkeyModifiers`, `hotkeyKeyLabel`.
- Default mode is `"combo"`: modifier-only chord ⌥+⇧, tap and release with no other key (default mods = `maskAlternate | maskShift`).
- Modes:
  - `combo`: modifier chord, fires on release if no other key was pressed during the chord.
  - `doubletap`: double-tap of a modifier within 0.30 s.
  - `modkey`: a single modifier, including right-side keys, fn/globe (63) and Caps (57).
  - `key`: key plus modifiers, requires at least one modifier.
- Presets in the picker (`UIControls.swift:594`):
  - ⌥⇧ combo
  - ⌃⌥ combo
  - Right ⌥
  - Left ⌥
  - Right ⌘
  - Left ⌃
  - 2×⇧
  - 2×⌥
  - 🌐 Globe/Fn
  - ⇪ Caps Lock
  - "Custom…" (opens the recorder panel)
- Hint text: "Press a combo… (Esc to cancel)".

**C. Instant layout switch (cycles input sources, does not touch text)**
- Keys: `instantSwitchEnabled` (false), `instantSwitchMode` ("globe"), `instantSwitchKeyCode` (63), `instantSwitchMods`, `instantSwitchKeyLabel`.
- Modes: `globe`, `key`, `modkey`.
- Presets (`UIControls.swift:1484`): 🌐 Globe/Fn, ⌘Space, ⌃Space, ⇪ Caps Lock, ⌥Space, Left ⌃, Right ⌃.
- Shift is rejected (message `is.noShift`).
- Migration: if saved as keyCode 56 or 60 (Shift), it is disabled at launch.
- Caps and Globe variants need the macOS-only machinery in section 11.

**D. Caps Lock LED as a language indicator**
- Key: `capsLEDIndicator`, default false.
- Label: "Caps Lock light shows the language". Sub-label: "Lit means Russian, dark means English".
- Lit = Russian, dark = English. Needs Input Monitoring and writes the HID LED element (`CapsLED.swift`).

**E. Exceptions**
- Ignored words (`ignoredWords`): never converted. Pre-seeded with "вк", "тг", "vk" (flags `didSeedExcExamples`, `didSeedExcVkLatin`).
- Force-swap words (`forceSwapWords`).
- Learned words (`learnedWords`).
- Per-app mode (`appExceptionModes`): "off" or "soft". Default value is "" (normal).
- Per-app forced layout (`appForcedLayouts`): "en" or "ru". It is applied when the app becomes frontmost, once per app switch.
- "Soft" mode: only obvious words are converted. A word with a core of 2 characters or less, or all identical characters, is skipped. It is also skipped if an intra-word pause exceeds 1.0 s (`softMaxIntraWordGap`).
- Settings UI: "App exceptions" list with "Add app…", "From running…", Off/Soft per row, and a forced-layout dash/EN/RU.
- **Seeding of built-in app modes** (`Engine.seedDefaultExceptions`, `Engine.builtinAppMode`). On first sight of an installed app (scanning /Applications, ~/Applications, /System/Applications and Spotlight.app), it assigns:
  - "off" for `defaultOffApps` (`Engine.swift:2412`): Final Cut, Motion, Compressor, Logic, GarageBand, iMovie, Ableton Live, Pro Tools, Cinema 4D, Blender, Terminal, iTerm2, Console, Alacritty, kitty, Ghostty, Hyper, Tabby, WezTerm, System Preferences. Also bundle prefixes for Premiere Pro, After Effects, Audition, Warp and DaVinci Resolve.
  - "soft" for `defaultSoftApps` (VS Code and Insiders, Xcode, Cursor, Windsurf, Nova, Zed, Sublime Text 3/4, MacVim, Atom) and any `com.jetbrains*` bundle.
  - `seededDefaultApps` records the ones already applied, so user changes are not overwritten.
- **Dev-app set** (`Engine.devApps`): in `developerMode`, auto is off in these apps and `com.jetbrains*`.
- **"Who wins" (ambiguous pairs) subpage:**
  - 49 hard-coded EN/RU pairs in `AmbiguousPairs.list`, for example `yt`/`не`, `vs`/`мы`, `in`/`шт`, `ru`/`кг`, `key`/`лун`.
  - Each pair is stored as a force-swap entry. The choices are EN, auto or RU.
  - It is reachable via a button, not the sidebar.

**F. Snippets**
- Autoreplace (`snippetsOrdered` = array of `[trigger, expansion]`). Legacy key `snippets` (a dictionary) is migrated.
  - Triggers match case-insensitively and layout-insensitively, because the typed word is passed through `Keymap.convert(toCyrillic:false).lowercased()`.
  - Example from the hint: `!test = !TEST = !еуые`.
- Expand-on-key toggles (`snippetExpandSpace`, `snippetExpandEnter`, `snippetExpandTab`, all default true), shown as "Expand on key". If all three are off: "Snippets are off — no expansion key selected."
- Editor: columns "Replace" and "With", placeholders "e.g. br" and "e.g. Best regards, Alex". It always keeps one trailing empty row. Editing uses hand-built NSStackView rows with a drag-reorder ghost animation (0.15–0.16 s).
- Separate "Snippets to insert" list (`textSnippets` = `[[name, text]]`). Placeholders: "name" and "text to insert". "Copy from autoreplace" merges entries by name.
- Snippet picker hotkey (`snippetPickKeyCode` -1 = off, `snippetPickModifiers`).
  - It opens a borderless non-activating panel (width 620, row height 28).
  - Digits 1–9 insert directly.
  - ⇧digit inserts items 10–18 and ⌘digit inserts items 19–27 (banks by modifier).
  - A "zeroth" row `0` inserts the last dictation (excluded voice entanglement).
  - Esc closes. Any other key closes the panel.
  - Tip text: "a digit inserts · Esc closes".
- Expansion is sanitized: control characters other than `\n` and `\t` are dropped (`Engine.sanitizeSnippet`).

**G. Translate selection**
- Keys: `translateEnabled` (true), `translateHotkeyKeyCode` (17 = T), `translateHotkeyMods` (⌃⌥), `translateHotkeyKeyLabel` ("T").
- Presets: ⌃⌥T, ⌥T, ⌃⌥E, ⇧⌘T.
- Direction is auto-detected: Cyrillic means ru→en, otherwise en→ru (`TranslateDirection.of`).
- Uses Apple's Translation framework offline, **macOS 15+ only**, with an 8 s timeout. Language packs are downloaded via `translationTask`.
- Translate sound: `translateSoundEnabled` (true), `translateSoundName` ("keyboop"), `translateSoundVolume` (0.6).
- It replaces the selection in place, via AX write or synthetic typing.

**H. Case toggle**
- Keys: `caseChangeKeyCode` (-1 = off), `caseChangeMods`, `caseChangeKeyLabel`.
- Text:
  - Title: "Change case of the selection".
  - Sub: «phone» becomes «PHONE», pressing again brings it back.
- Behavior: if the selection contains any lowercase, make it uppercase; otherwise lowercase. Maximum 5000 characters (`caseChangeMaxChars`, `Engine.swift:1529`), refused above that.

**I. Plain paste**
- Keys: `plainPaste` (false), `plainPasteKeyCode` (9 = V), `plainPasteMods` (⌘⇧), `plainPasteKeyLabel` ("⇧⌘V").
- Label: "Paste without formatting". It swaps the clipboard to plain text, sends ⌘V, waits 0.30 s, then restores the original contents. It does not restore if the clipboard changed meanwhile.
- It adds an `org.nspasteboard.TransientType` marker.

**J. Pause ("Do not disturb")**
- Lengths: 15 min, 1 h, 3 h, 5 h (`Pause.lengths = [15, 60, 180, 300]`).
- Keys: `pauseMinutes` (15), `pausedUntil` (epoch seconds).
- It disables auto-conversion, live-fix and snippets, via `Pause.active` checks.
- Toasts: "Not disturbing you. Repeat the quick action to resume", "Back on duty", "Not disturbing you until %@". Menu item "Resume now".

**K. Menu-bar appearance**
- `menuBarStyle`: "brand" (default), "keyboard", "flag", "hidden". Legacy "letter" maps to brand and "layout" maps to hidden.
- `menuBarShowLanguage` (default true) shows the two-letter code (RU, EN) next to the icon.
- With "hidden" and no language, the status item is invisible. Relaunching from Applications opens Settings.
- Flag mode uses an emoji flag per language, for 30 languages (`flagByLang`, `MenuBarController.swift`) rendered to an image. It falls back to the SF Symbol "keyboard".
- Status-icon states (`AppHealth.IconState`): ok, tapSuspended, engineDown, secureInput, paused, autoOff. Dimmed for tapSuspended, engineDown and paused. A "⚠︎" suffix is shown when permissions are needed.

**L. Quick actions (right click on the menu-bar icon)**
- `quickAction` default "copyVoice". Options: copyVoice, pause, dictate, history, settings.
- ⌥-click toggles hidden call-recording (voice side, excluded).
- A "slap the MacBook" gesture exists but is **compiled off** (`ReleaseFeatures.slap = false`). See section 12.

**M. General**
- Appearance `appTheme`: system, light, dark.
- Interface language `language`: auto, ru, en. It syncs to `AppleLanguages` for Sparkle dialogs.
- `launchAtLogin` uses `SMAppService.mainApp` (not stored in defaults).
- "Sounds" master mute `silentMode` (false).
  - Description: "Turn it off and the app goes completely silent: switching, translation, dictation and system beeps. Volumes are remembered".
  - It uses `mutedVol.<key>` backups.
- "Remember copied text" (`clipboardHistoryEnabled`, false). It requires voice history to be enabled. See 3.10.
- Modes: "Basics" (`simpleMode`, a root screen of about 6 rows with no sidebar) versus "Everything" (full sidebar).

**N. Updates (Sparkle)**
- "Check for updates" (Sparkle automatic checks).
- "Install right away, no questions" = `silentAutoUpdate`.
- "Install beta versions" = `betaChannel`.
- "Check now" button.
- Banner "Keyboop %@ is ready" with buttons "Update" and "Update automatically".

**O. About / Feedback / Welcome**
- About: version (with codename), license, counters "Gibberish un-garbled" (`rescuedCount`) and "Dictated by voice". Buttons: "Email the developer", "What's new", "Welcome tour", "Telegram channel @keyboop", "Support the project ₽".
- Easter egg: 5 clicks on the version within 1.5 s each play a synthesized meow (`CueSynth.versionTap`).
- Feedback window ("Write to the developer"). See 7 and section 11.
- Welcome window. See 5.

**P. Privacy section** ("Doesn't phone home"): static text. It names four network uses: voice-model download, opening the site or Telegram, feedback, and the update check.

**Q. Menu title and codenames.** `Changelog.codenames` maps minor versions to animal names: 0.3 Pika, 0.4 Axolotl, 0.5 Kinkajou, 0.6 Binturong, 0.7 Kakapo, 0.8 Olinguito, 0.9 Aye-aye, and so on. The menu header is "Keyboop 0.4.10 · Axolotl", with " · auto off" appended when auto is off.

---

## 3. Core behavior, precisely

### 3.1 Input capture (`EventTap.swift`)
- **API.** `CGEvent.tapCreate(tap: .cgSessionEventTap, place: .headInsertEventTap, options: .defaultTap, ...)` (active, can swallow events), on the main run loop in `.commonModes`.
  - The mask is `keyDown | keyUp | flagsChanged | system-defined (type 14)`.
  - Mouse: `NSEvent.addGlobalMonitorForEvents(matching: [.leftMouseDown, .rightMouseDown, .otherMouseDown])` (`EventTap.swift:305`). A click resets context: clears the buffer, disarms modifier gestures, and probes via AX whether the character left of the caret is a letter.
- **Tap health.**
  - If the system disables the tap (`tapDisabledByTimeout` or `tapDisabledByUserInput`), it re-enables it.
  - 3 timeouts within 60 s (`tapStormLimit`) cause a suspension of 300 s (`suspendCooldown`).
  - Maximum 3 suspensions per session (`maxSuspensions`); after that it is off permanently.
  - A timeout also disables inline fix for 180 s (`Engine.disableInlineAfterTapTimeout`).
  - After 3 such timeouts, inline fix is off for the session.
- **Event age log.** Events older than 20 ms, or a callback body over 10 ms, are logged. It auto-detects whether `event.timestamp` is in nanoseconds or Mach ticks.
- **Synthetic-event marking.**
  - Every event the app posts carries `eventSourceUserData = kbSyntheticMarker` (random non-zero Int64 per launch).
  - Marked key events pass straight through the tap.
  - A second random marker, `kbPauseFixMarker`, tags the "pause-fix" probe event (virtual key 255). Its arrival triggers the inline fix inside the tap, so `post` can use the tap proxy.
- **Per-key-down order.**
  1. Chatter filter, if `dedupeChatter`: drop the same non-autorepeat keycode within 0.030 s (`chatterWindow`, `EventTap.swift:188`), unless ⌘/⌃/⌥ are held.
  2. If a hotkey is being recorded (`HotkeyRecording.active`), pass the event through.
  3. Voice hotkeys and Esc cancel for dictation (excluded).
  4. Instant-switch key (key mode), then Caps-as-LANG1 handling: keyCode 104 is LANG1 after the Caps remap. ⇧+Caps toggles real caps lock via a synthetic event (`RealCapsLock.toggle`).
  5. Snippet-picker handling: Esc, 0, digits with banks, other key closes.
  6. Open-picker hotkey, plain-paste, case-change, paste-last-dictation (voice entanglement), translate (macOS 15+), manual-convert `key` mode.
  7. Characters are computed from the cached keyboard layout (`KeyboardLayoutCache.characters` via `UCKeyTranslate`, with dead-key state). It falls back to `CGEvent.keyboardGetUnicodeString`. Then `Engine.handleKeyDown(...)`. Returning true swallows the key.
  - A hotkey match swallows the down event and remembers the keycode in `swallowedDownKeyCodes`. The matching up event is swallowed too (`EventTap.swift:399`).
- **Modifier gestures** (`handleFlags`). Relevant mods are only ⌥, ⇧, ⌘, ⌃.
  - `combo`: arms when the held mods equal the target exactly. It fires on release of all mods if no other key was pressed in between.
  - `doubletap`: two solo taps of the mod within 0.30 s (`EventTap.swift:1136`) with no other key between them. It accepts either left or right key (`HotkeyGuard.maskCovers`).
  - `modkey`: a solo tap of one specific modifier keycode; any other key or modifier disarms it.
  - A fn/globe tap is swallowed whenever the globe is our hotkey.
  - A system-defined event (type 14, media/brightness keys) disarms pending modifier gestures.
  - Secure Input on: modifier gestures are disarmed, because ordinary keys are hidden.
- **Not captured:** the code never looks at keyUp for typing, only to swallow hotkey/inline partners. It does not synthesize or track key-repeat except to ignore it for hotkeys. There is no mouse use other than reset-context clicks.

### 3.2 Word buffer (`KeystrokeBuffer.swift`)
- State: `currentWord`, `lastWord`, `lastTail` (the whitespace after it), `sessionWords` (array of `(word, tail)`).
  - Typing chars append. Backspace removes the last char, and when the word empties it clears `lastWord`/`lastTail`.
  - Backspace on an empty word with a tail removes tail characters. With neither, it pulls `lastWord` back into `currentWord` (editing the previous word).
  - A boundary key (space, tab, return) moves `currentWord` into `lastWord` and `sessionWords`.
  - Double-space gap timing is tracked (`lastTailDoubleSpaceGap`) for the system's "." substitution rule.
- Gap tracking: `currentWordGap` and `lastWordGap` (the maximum idle between characters of a word), used by the soft filter.
- Group conversion eligibility (`groupForConversion`):
  - Idle at most 8 s (`groupMaxIdle`).
  - At least 2 words.
  - Total characters at most 200 (`groupMaxChars`, line 24).
  - No `\n` or `\t` tails.
  - No multi-scalar graphemes.
- Context words: `contextWord(forCurrent:)` and `earlierContextWord` give the previous 1–2 words, but not across line breaks.
- Printable test in `Engine`: first scalar value ≥ 0x20 and ≠ 0x7F, with ⌘/⌃ not held.
- Backspace modifiers: ⌥⌫ or ⌘⌫ clears the buffer. Arrows follow `arrowsCancel` (and always clear with ⇧). Esc, Delete-forward, Home, End, PgUp and PgDn clear the buffer. ⌘/⌃ combos clear the buffer.
- Context reset on mouse click, frontmost-app change, Spotlight open/close, and secure input. A click yields a full clear on the next key, but a soft reset if a real key was typed after the click. A soft reset clears completed words only. An app switch sets `pendingSoftReset`.

### 3.3 Auto and live conversion paths (`Engine.swift`)
- **Boundary conversion** (`handleKeyDown`, boundary case 49/48/36):
  1. Snippet expansion check first (see F).
  2. For Return only: `convertBeforeReturn`. It pre-converts the pending word and the Enter is swallowed and re-sent as a synthetic key after the replacement (`thenReturn`).
  3. `buffer.boundary(...)`.
  4. After 30 ms (`asyncAfter 0.03`), it reconciles the layout opinion with the OS (if they diverged, it drops the word). If a real key arrived less than 25 ms earlier, it delays another 40 ms.
  5. `convertFromBuffer(manual:false, soft:)`.
- **Mid-word pause fix.** After every printable key, `pauseFixTick` is scheduled at +150 ms. If no key arrived for at least 140 ms, a dummy key event (virtual key 255, marked) is posted to the HID tap. When it comes back through the tap, `handlePauseFixMarker` runs the inline fix atomically.
  - Gating includes: word length 4–16, not the same as `liveFixLast`, front-app mode empty, not Chromium/Electron family, no ⇧/⌥/⌘/⌃ held, not Secure Input, the data warm, and `muted` false.
  - If the word is mixed Cyrillic+Latin with a pure Latin tail, only the tail is converted ("live-heal").
- **Inline fix on keypress** (`tryInlineLiveFix`, line 1111). On the key that would complete the candidate (`currentWord + pendingChar`), if `LayoutDetector.liveDecide` says convert, it:
  - builds one event batch: N×(backspace down+up) then the converted text in 12-UTF-16-unit unicode chunks (down+up each),
  - posts them all via the tap proxy (`tapPostEvent`),
  - swallows the original key (and its later keyUp, via `inlineSwallowedKeyCode`, valid 2.0 s).
  - It only works when `inlineHealthy`, `settings.inlineLiveFix`, `Warm.isReady`, no modifiers, not Chromium/Electron family, and not Secure Input.
- **Chromium family.** `Engine.chromiumFamily` (line 853) is a list including Chrome variants, Brave, VS Code, Slack, Notion, Obsidian, Figma, Spotify, Discord, Superhuman, Linear, Signal, Vivaldi, Arc, Opera and Yandex Browser, plus any bundle prefix `com.electron`, `com.tinyspeck`, `com.microsoft.edgemac` or `org.chromium`. Live-fix is off in these. Electron is detected by the presence of `Contents/Frameworks/Electron Framework.framework`.
- **Anti-resonance guard** (`AntiResonanceGuard.swift`). Defaults: `window=0.7 s`, `maxFlips=6`, `freezeFor=2.5 s`. A conversion is refused (auto frozen for 2.5 s) if the word being converted equals something we produced within the last 0.7 s (oscillation), or if more than 6 conversions fit in the window (storm).
- **Muting during synthetic output** (`muted`):
  - Set while a synthetic replacement is in flight.
  - `inFlightRealKeys` counts real keys that arrive during the flight; if any arrive, the buffer is cleared at the end.
  - The flight ends `muteDrain = 0.08 s` (line 185) after the synth completion callback.
  - A stuck `muted` is self-healed after 1.2 s (`mutedStuckCheck`).
- **Manual path** (`convertFromBuffer(manual:true)`, line 1727):
  1. Undo last text-fix (typo or caps) if the word still equals the fixed text.
  2. If the buffer has no word, try converting the **selection** (`convertSelection`, line 1540).
  3. If group-convert is on and auto is off, try `convertGroup` (line 1676).
  4. Otherwise convert the current or last word.
  5. If the buffer is empty and no selection is available, it **cycles the input source** (`layout.cycleLayout`).
  - Direction is by script: Cyrillic becomes Latin, Latin letters become Cyrillic, otherwise it flips the current layout.
  - If a hotkey is pressed while `muted`, it is queued for up to 2.0 s (`pendingManualTTL`) and replayed when the flight ends.
- **Selection conversion** (`convertSelection`):
  - It reads the selection via AX (`kAXSelectedTextAttribute`), falling back to a synthetic ⌘C with clipboard snapshot and restore.
  - It rejects clipboard fallbacks with a newline or more than 80 characters (likely an auto-copied line).
  - For Figma canvas objects, `lastCopyLooksLikeCanvasNode` with a single-line body of 300 characters or less uses `replaceCanvasObjectText` (Return, then 60 ms, type, 20 ms, Esc).
  - For Gecko browsers (Firefox and others, a hard-coded bundle list plus `Contents/MacOS/XUL`), the write path is typing, and selections containing newlines or U+FFFC are refused.
- **Mixed-script rescue** (`LayoutDetector.mixedRescue`): a word containing both Cyrillic and Latin letters is converted entirely if exactly one direction yields a dictionary word.
- **Double-space-period rule** (`DoubleSpacePeriod.swift`). If the macOS "double space inserts a period" setting is on (`NSSpellChecker.isAutomaticPeriodSubstitutionEnabled`) and the typed tail starts with two spaces typed within 0.45 s, and the converted word ends in a letter or digit, the tail is rewritten to ". " + rest.
- **Layout-phantom guard.** Within 1.0 s of our own layout selection, if AX shows the caret is already preceded by the converted text, the replacement is skipped.
- **Digit-aware conversion.** A comma or period between two ASCII digits is treated as a number separator: converting to Cyrillic keeps it; converting to Latin turns `,` into `.` unless it is the first of a repeating separator pattern (`Keymap.numberSeparator`).

### 3.4 Detection (`LayoutDetector.swift`, `LayoutData.swift`)
- Data: `words_en.json` (59,276 entries), `words_ru.json` (162,760), `trigrams_en.json` (8,110), `trigrams_ru.json` (11,458), merged with the in-code sets in `ExtraWords.swift`: `ru`, `ruDev`, `ruAbbr`, `ruShort`, `ruCommonForms`, `ruLoanNames`, `en`, `forceRuAmb`, `forceEnAmb`, `techLatinTokens`, `techFromCyrillic`, `defaultKeep`, `hyphenTerms`, `ruHyphenTerms`, `labelClassifiers`, `ruLabelClassifiers`, `enKeepShort`.
- `plausibility(word, cyrillic:)` is the average trigram log-probability over `" " + word.lowercased() + " "`. A missing trigram scores -20.0.
- Constants: `margin = 2.0`, `shortEnSwapFloor = -10.0`, `liveImpossible = -13.0`, `liveMargin = 6.0`.
- `decide(...)` (line 395) returns `.keep` or `.convert(toCyrillic:)`. Order of rules (abridged):
  1. Learned, ignored or whole-word exceptions keep the word.
  2. Emoticons keep it.
  3. Hyphenated tokens get segment-wise handling.
  4. A core under 1 character (or 4 characters when digits were present) keeps it.
  5. Mixed script keeps it.
  6. If the layout swap of the word is a built-in forced term (`forceSwap`: http, json, sql, api, …) and the source is not a real word, it converts.
  7. Source real word (EN or RU dictionary, including de-elongated forms like "heeeey") keeps it.
  8. A capitalized word that is not first keeps it.
  9. One-letter handling: Latin to the Russian single letters а в и к о с у я; Cyrillic to `i`, `u`, `a`; with context hints and a caret-jump suppression.
  10. Dictionary-membership decisions in each direction, with ambiguity sets (`forceRuAmb`, `forceEnAmb`, `enKeepShort`) and context from the previous word.
  11. Otherwise word length of 4 or more, with trigram scoring:
      - convert if `swapScore > origScore + 2.0`, or
      - convert if `origScore <= -19.0 && swapScore > origScore + 1.0`.
  - Special rules: an all-caps Cyrillic word is kept unless it is an impossible Russian spelling; if a Cyrillic word's swap ends in trailing punctuation and the word is not a dictionary word, it re-decides on the word with punctuation trimmed.
- `liveDecide` (mid-word) is much stricter: the original must score `<= -13.0` (impossible), the swap must beat it by more than 6.0, the word must be at least 4 letters, no digits at the edges, no exception, no valid source word.
- `Keymap.liveConvert` additionally refuses (returns nil, "wait for the next letter") if converting would turn a trailing punctuation key into a letter.
- `LayoutData` also reads from `KEYBOOP_DATA_DIR` if set.
- Detection operates only on RU/EN. The layout bridge is a hard-coded JCUKEN base table (`Keymap.basePairs`, `Keymap.swift:8`), but the live table is generated from the user's actual installed layouts (`DynamicKeymap.rebuild`, section 3.5).

### 3.5 Key mapping tables
- **Fallback table** `Keymap.basePairs`: ` `→ё; qwertyuiop[]→йцукенгшщзхъ; asdfghjkl;'→фывапролджэ; zxcvbnm,./→ячсмитьбю.
- **Dynamic table** (`DynamicKeymap.swift`):
  - It enumerates select-capable keyboard input sources with a `UnicodeKeyLayoutData`, picks one Latin and one Cyrillic layout (preferring the last-used IDs `lastLayoutLat`/`lastLayoutCyr`), and builds `enToRu`/`ruToEn` by `UCKeyTranslate` for keyCodes 0–50 × {no shift, shift}.
  - It skips the ISO section-key (keyCode 10) on ANSI/JIS hardware (via `KBGetLayoutType` loaded with dlsym).
  - It adds typographic-quote aliases (’ ‘ ´ ′ map like `'`; “ ” ‟ map like `"`).
  - It rebuilds on `kTISNotifyEnabledKeyboardInputSourcesChanged` and `kTISNotifySelectedKeyboardInputSourceChanged`.
  - `DynamicKeymap.latinKeyCodes` is also used to place hidden Cyrillic "twin" key equivalents in menus.

### 3.6 Learning from undo (`UndoLearner.swift`)
- A candidate is the last conversion (original, converted, time), valid for 4 s (`undoWindow`).
- Stage machine: fresh/deleting (the user erases our output char by char) to retyping (output fully erased) to the original retyped exactly. That counts as an undo and puts the word under session protection.
- A manual hotkey press right after a conversion that maps converted back to original also counts as an undo (`noteManualConvert`).
- 3 strikes (`strikeThreshold`, line 31) on one word within a 7-day decay window (`strikeDecay`) trigger a banner: "Keep "%@" as is?" with "Add" and "No thanks". Adding puts it in `learnedWords`. Declining stores the word in `undoDeclinedWords`. The banner auto-dismisses at 20 s.
- Keys: `undoStrikeCount`, `undoStrikeTime`, `undoDeclinedWords`.

### 3.7 Typo fixes (`TypoFix.swift`, `NumericTypoRule.swift`)
- Table `typo_rules.json`: `{"en": {...155 entries}, "ru": {...4874}}`.
- Order at the boundary after no layout conversion: two-caps fix (if on), caps-lock-word fix, then typos.
  - Caps-lock word: at least 5 letters, first lowercase, the rest all uppercase. It becomes Capitalized, or a brand spelling from `Engine.brandSpelling`: iOS, iPadOS, macOS, watchOS, tvOS, visionOS, iPhone, iPad, iMac, iCloud, iTunes, iMessage, eBay, eSIM, IoT. Applies only if `typoFix` is on.
  - Two caps: `twoCapsKeep` set exempts iphone, ipad, ipod, imac, icloud, itunes, imessage, ibooks, iwork, ebay.
- `suggest`: numeric rule, then a curated table lookup, then a "mechanical" suggestion. Mechanical means a neighbouring-letter swap or a doubled-letter removal that yields exactly one dictionary word. It needs at least 5 characters, and a Russian doubled "н" is exempt.
- Numeric rule: a Russian-layout `ю` between two ASCII digits becomes `.` (for example "1ю8" to "1.8").
- Guards: the word must be absent from the dictionaries, not personal, not protected (ignored, learned, or session-protected).
- Personal dictionary: `typoPersonalWords` counts how often an unknown word is typed. At 2 (`personalThreshold`) it is considered "yours" and is never corrected.
- Leading capital is preserved. Surrounding punctuation is peeled.

### 3.6b Replacement mechanism (`TextReplacer.swift`) — critical for the port
- Output is always synthetic key events: virtual key 51 (backspace) down and up, then a Unicode string posted as keyboardEvent with `keyboardSetUnicodeString`.
- Chunks are 12 UTF-16 units (`chunkSize`, line 551), each down+up, with 800 µs between chunks.
- Delays: the first-key settle is 9 ms (`settleMicros`; snippets use 45 ms); 1.8 ms between backspaces; 0.9 ms down→up hold.
- A forward-delete is added first if Spotlight is open or there is an AX selection ("kill suggestion"). After the clipboard read there is a "sacrifice" keypress (keyCode 79 / F18) on Chromium/Electron apps to prime the field.
- Posted to the `cghidEventTap`, or `cgAnnotatedSessionEventTap` if another app's key tap is detected (`RivalTapWatch`).
- Events use `CGEventSource(stateID: .privateState)` and `flags = []`.
- It first tries a direct `NSTextView` replacement if Keyboop's own window is active, and checks secure-input policy before writing (`SecureInputPolicy.canWrite`).

### 3.8 Sounds (the only audio output in the non-voice app)
- `Sounds.swift` is the single gate. `Sounds.allMuted` equals `silentMode`.
  - `Sounds.play(NSSound?, volume:, as:key)` skips when volume is 0 (zero volume means do not play, not play silently).
  - It keeps one retained `NSSound` per key, stops the previous one, and plays asynchronously.
  - `Sounds.beep()` wraps `NSSound.beep()`, and is the only path for beeps.
- Conversion cue (`Engine.playSound`, line 2369): plays when `soundEnabled` is on and `soundName` is non-empty.
  - `"keyboop"` plays the synthesized `CueSynth.switchData`.
  - Anything else is a system sound by name (`NSSound(named:)`).
  - The picker lists "No sound", "Keyboop", then every `*.aiff` in `/System/Library/Sounds`, sorted. Selecting one previews it at the current volume.
- Translate cue: `translateSoundName` "keyboop" gives `CueSynth.translateData`, "" gives none, else a system sound.
- `CueSynth.swift` synthesizes mono 16-bit 44100 Hz WAV (RIFF header written in code):
  - Switch cue: two 34 ms bursts of 440 Hz (exponential decay `exp(-t/(0.034*0.38))`, 7 ms linear attack, amplitude 0.38) with 22 ms silence between.
  - Translate cue: tri-tone 330, 415, 523 Hz (about C-E-G), each 50 ms, with 18 ms gaps (`gapDur*0.6`); the decay constant is `0.05*0.5` per tone, attack 7 ms, amplitude 0.38.
  - Also defined: `startData` (330 then 440 Hz), `stopData` (440 then 587 Hz), `failData` (300 then 225 Hz). These are the voice start/stop/fail cues and are not used by non-voice paths. `baseHz = 330`, `ratio = 440/330`.
  - Meow: 0.42 s, a pitch glide 480 Hz to 760 Hz (first 45%) to 500 Hz, 6 Hz vibrato at 3%, harmonics [1.0, 0.5, 0.28, 0.12]; used by the easter egg and the (disabled) slap gesture.
- System beeps (via `Sounds.beep`): manual hotkey when the conversion output equals the input; translate and case-toggle failures (no selection, secure input, over 5000 characters, Gecko newline refusal).
- No sample files ship; there is no audio engine other than `NSSound`.

### 3.9 Layout management (`LayoutManager.swift`, `LayoutCycle.swift`)
- OS layout is read via Carbon TIS. It also reads `AppleSelectedInputSources` from `com.apple.HIToolbox` prefs (more current than TIS for the active layout) and maps names to language codes.
- Keeps an "opinion" of the current layout and reconciles it with the OS before a conversion (more than 0.8 s after our own select); a mismatch means the word is skipped because it was decoded in the wrong layout.
- `selectLayout(cyrillic:)` selects the last-used ID for that script if still enabled, else the first `ru` source, or the first `en`/ASCII-capable Latin source. It verifies after 60 ms and re-issues once if the OS did not apply it.
- `cycleLayout()` orders enabled sources by `AppleEnabledInputSources`, picks the next with a fresh-opinion anchor (0.15 s) logic, and verifies after 60 ms. It remembers the last Cyrillic and Latin IDs (`lastLayoutCyr`, `lastLayoutLat`).
- The secure-input timer polls every 2.5 s: `IsSecureEventInputEnabled()`, reconciling layout opinion. When it turns on, a background `ioreg -l -w0` parse finds the holder PID (`kCGSSessionSecureInputPID`) for the UI message ("Hidden input: %@").

### 3.10 Clipboard history (non-voice feature stored in voice's file)
- `ClipboardWatcher`: polls `NSPasteboard.changeCount` every 0.5 s. A change is accepted on the next tick if the count did not move again (debounce).
- Active only when `voiceHistoryEnabled && clipboardHistoryEnabled`.
- Skipped when: our own operation, Secure Input, concealed type (`org.nspasteboard.ConcealedType`, `.TransientType`, `.AutoGeneratedType`, `com.agilebits.onepassword`, `de.petermaurer.TransientPasteboardType`), non-text, whitespace only, over 20,000 characters, or a duplicate of the last stored text.
- Stored via `VoiceHistory.shared.addClipboard(...)` with the frontmost app name. Cap is 100 clipboard entries (`HistoryPolicy.maxClipboard`).
- `PasteboardOwnership` tracks the last 32 change counts of our own writes, and "owned windows" around multi-step operations.
- Retention is shared with dictation (see Persistence).

### 3.11 Other timing constants (all in code)
- Spotlight detection (`SpotlightWatch`): probe at most every 0.25 s on key down via CGWindowList. Hosts: `com.apple.Spotlight` (any size) and `com.apple.campo` (width at least 400, layer above 0). It matters because AX focus is unreliable there.
- `RivalTapWatch`: scans `CGGetEventTapList` every 5 s (cached) for any other enabled default key tap not in system paths.
- `IdleMonitor`: default 300 s threshold, 20 s poll, uses `CGEventSource.secondsSinceLastEventType` (used by the update-install logic).
- Logging: capped at 1 MiB, trimmed to the last 512 KiB, size checked every 128 writes.

---

## 4. Sound assets

**None exist.**
- No sound files. No packs, themes or manifests. No custom import.
- Bundled resources are exactly: `menubar-mark.png`, `keyboop-logo-anim.mp4`, `AppIcon.icns`, an asset catalog containing only a coral accent color, the five JSON language/typo data files, and the Sparkle framework (vendored).
- The only audio is runtime-synthesized WAV (section 3.8) and macOS system sounds read from `/System/Library/Sounds/*.aiff` (not bundled).

---

## 5. UI

**Design system `DS`** (`SettingsWindow.swift:7`):
- Sidebar width 220, row height 32, row gap 2.
- Content width 600 (fixed), margin 24.
- Simple-mode content width 560.
- Brand icon 44, titlebar 32.
- Coral accent `#FF7A59` (`srgb 1.0, 0.478, 0.349`; same in the asset catalog `AccentColor`).
- Graphite `#1C1B1A`.
- Pill radius 6, section gap 18, item gap 10, field max width 360.
- Minimum window width = 220 + 24 + 600 + 28.

**Menu-bar item and menu** (`MenuBarController.swift:588`): left click opens the menu, right click runs the quick action. Menu contents, in order:
1. Disabled header "Keyboop <version · codename>" (dev builds add a time stamp; " · auto off" when off).
2. If paused: "Not disturbing you until HH:mm" and "Resume now".
3. If a permission is missing: orange-triangle item "Needs access: Input Monitoring…" or "Needs access: Accessibility…", plus "…and move Keyboop to Applications" if not in /Applications.
4. A health problem row (orange triangle): "Accessibility permission not granted", "Engine is not running", "Interception is off: the system kept killing it, so I stopped", or "Another app hid the keyboard" (clickable, opens the Privacy section).
5. "Auto-switch" toggle (checkmark, key equivalent `a`, no modifier).
6. "Do not disturb" submenu: 15 minutes, 1 hour, 3 hours, 5 hours (only when auto is on and not paused).
7. Separator, then the voice items: "Microphone" submenu (excluded), "Dictation and clipboard history…" (key `h`), "Copy last dictation" (key `c`, only when one exists).
8. "Settings…" (⌘,).
9. "Check for Updates…" (key `u`), "Report a problem…" (key `r`).
10. Separator, then "Quit" (key `q`, behind a confirm alert).
- Hidden Cyrillic twin key equivalents are added for the single-letter shortcuts (`addLayoutTwins`), so they work on a Russian layout.
- A hidden main menu is installed so Cmd-V/C/X/A/Z work in text fields: Quit ⌘Q, Close Window ⌘W, Edit (Undo, Redo ⇧⌘Z, Cut, Copy, Paste, Select All).

**Settings window** (`SettingsWindowController`): resizable titled window, fullSizeContentView, transparent titlebar, `NSVisualEffectView` sidebar.
- Two modes. "Basics" is a root screen of about six rows (manual hotkey, dictation key, translate, counters "Unjinxed %@ · dictated %@"); it has no sidebar. "Everything" is the sidebar below, with a "‹ Essentials" back button.
- Sidebar order and SF Symbols: Layout switching (keyboard), Exceptions (tag), [hidden subpage "Who wins" (arrow.left.arrow.right)], Voice input (mic; excluded), Snippets (wand.and.stars), Translate (character.bubble), General (gearshape), Updates (arrow.triangle.2.circlepath), Privacy (lock.shield), About (info.circle).
- The window height is persisted (`proWindowHeight`). Horizontal resize is locked in Pro mode.

**Hotkey recorder panel** (`HotkeyRecorderPanel.swift`): a small panel titled "New shortcut", "For: %@", a live key display, the hint "Hold modifiers and press a key. Or just press and release a single modifier.", and Assign/Cancel.
- Hard-blocked ("blocked") combos: ⌘C ⌘V ⌘X ⌘Z ⌘A ⌘S ⌘Q ⌘W ⌘N ⌘O ⌘P ⌘F ⌘H ⌘M ⌘R ⌘D ⌘Tab ⌘⌫ (`HotkeyGuard.cmdCritical`).
- "Warn" (assignable but flagged) for system hotkeys read from `com.apple.symbolichotkeys`. Known IDs: 32 Mission Control, 33 App windows, 60 and 61 input-source switching, 64 Spotlight, 65 Finder search, 160 Launchpad, 163 Notification Center, 179 Emoji panel.
- Other messages: a key with no modifiers is refused ("A key with no modifiers gets taken from every app at once…"); a clash with Keyboop's own function is refused ("We already use this one for %@…"). A single central registry covers all six hotkey features.
- `HotkeyGuard.sameKey` treats keyCodes 10 and 50 (§ and `` ` ``) as the same key.

**Snippet picker panel.** Borderless `nonactivatingPanel` at `.statusBar` level, width 620, row 28, vertical padding 12, tip row 22. It never takes focus.

**AppBanner.** Custom top-right toast and banner (not system notifications). Dark background with coral buttons; the `nonactivatingPanel` floats at `.floating`.
- Corner radius 14 (compact) or 18; buttons radius 8.
- Slide-in: fade 0.22 s plus 8 pt rise; fade-out 0.18 s.
- Info toast (no buttons) auto-hides; question banner waits.

**Welcome window.** 480×640, min 480×430, max 480×1200. Starts with an intro overlay (the logo video plays once and holds the last frame, a 0.5 s crossfade to content). It then shows:
- "Hi. I'm Keyboop." with the tagline.
- A Permissions card (Accessibility "Allow", Microphone, translate language pack card).
- A "What I do" list of four items.
- Hotkey pickers for the two main keys ("Dictation" and "Switch layout").
- A "Sandbox" text field to try `ghbdtn`.
- Voice model download (excluded).
- Buttons "Settings" and "Let's go".
- Footer: "From the boop family. We boop keys."

**Logo animation** (`AnimatedLogoView`): `keyboop-logo-anim.mp4` in an `AVPlayerLayer` with `plusLighter` compositing so black drops out; plays once and freezes on the last frame; falls back to the static app icon.

**Menu-bar mark.** `menubar-mark.png` (128×128 RGBA template image) scaled to 17 pt in an 18 pt-tall image. A vector fallback `KeyboopMark.draw` renders a rounded-square keycap outline with a bold "K" (inset 0.06, corner radius 0.23, stroke 0.075).

**Feedback window** ("Write to the developer"): multiline text, optional contact field ("Telegram or email — optional"), "Attach diagnostics" checkbox with "see what's sent", "Attach a screenshot or video…" (PNG/JPEG at most 10 MB, MOV/MP4 at most 25 MB), and Send.
- On failure it offers "Send by email" (mailto `hi@keyboop.com`) or "Via Telegram" (saves a report file to Downloads and opens `t.me/keyboop_bot?start=report`).
- A success screen "Off it goes".

**Other:** `FeedbackWindow` uses a hit-test-transparent placeholder label to avoid stealing clicks.

---

## 6. Persistence

All in `UserDefaults.standard` (bundle domain `ru.keyboop.app`, or `ru.keyboop.app.dev` for dev builds), except the listed files. Hotkey `*Mods` values are CGEventFlags raw bit patterns stored as Int; `*KeyCode` is a macOS virtual keycode.

**Core settings, types and defaults** (all other registered or computed defaults are listed in section 2):
- Bool: `autoEnabled` (true), `liveFixEnabled` (true), `inlineLiveFix` (true), `soundEnabled` (true), `triggerSpace` (true), `triggerEnter` (true), `triggerTab` (false), `enterPreConvert` (true), `arrowsCancel` (true), `groupConvert` (true), `learnOnUndoEnabled` (true), `dedupeChatter`, `twoCapsFix`, `typoFix`, `developerMode`, `silentMode`, `plainPaste`, `capsLEDIndicator`, `capsRemapApplied`, `translateEnabled` (true), `translateSoundEnabled` (true), `snippetExpandSpace/Enter/Tab` (true each), `clipboardHistoryEnabled`, `simpleMode` (set true on first run), `silentAutoUpdate`, `betaChannel`, `didInitialSetup`, `didLoginSeed`, `didShowWelcome`, `instantSwitchEnabled` (false; falls back to legacy `globeSwitchesLayout`), `menuBarShowLanguage` (true).
- Double: `soundVolume` (0.6), `translateSoundVolume` (0.6), `proWindowHeight`, `pausedUntil` (epoch seconds), `mutedVol.<key>` (backup, -1 if absent).
- String: `soundName` ("Pop"), `translateSoundName` ("keyboop"), `hotkeyMode` ("combo"), `hotkeyKeyLabel`, `language` ("auto"), `instantSwitchMode` ("globe"), `instantSwitchKeyLabel`, `menuBarStyle` ("brand"), `appTheme` ("system"), `quickAction` ("copyVoice"), `lastRunVersion`, `lastLayoutCyr`, `lastLayoutLat`, `translateHotkeyKeyLabel` ("T"), `plainPasteKeyLabel` ("⇧⌘V"), `caseChangeKeyLabel`.
- Int:
  - `hotkeyKeyCode` (-1)
  - `hotkeyModifiers` (⌥⇧)
  - `instantSwitchKeyCode` (63)
  - `instantSwitchMods`
  - `translateHotkeyKeyCode` (17)
  - `translateHotkeyMods` (⌃⌥)
  - `plainPasteKeyCode` (9)
  - `plainPasteMods` (⌘⇧)
  - `caseChangeKeyCode` (-1)
  - `caseChangeMods`
  - `snippetPickKeyCode` (-1)
  - `snippetPickModifiers`
  - `pauseMinutes` (15)
  - `rescuedCount`
  - `globePrevFnUsage` (-1; stores the previous AppleFnUsageType)
  - `NSInitialToolTipDelay` (120 ms)
- Collections: `ignoredWords`, `forceSwapWords`, `learnedWords` (arrays), `appExceptionModes`, `appForcedLayouts` (dictionaries by bundle ID), `seededDefaultApps` (array), `snippetsOrdered`, `textSnippets` (arrays of `[a, b]`), `undoDeclinedWords`, `undoStrikeCount`, `undoStrikeTime`, `typoPersonalWords` (word to Int). Flags `didSeedExcExamples`, `didSeedExcVkLatin`, `didUnseedExcSpotlight`, `warnedClash:<a>|<b>`.
- Excluded voice keys exist as well (`voice*` family, 30 or more).
- Slap gesture keys: `slapEnabled` (forced false), `slapAction`, `slapSensitivity`, `slapSoundEnabled`, `slapSoundID`.

**Files**
- `~/Library/Application Support/Keyboop/singleton.lock` (mode 0600).
- `~/Library/Application Support/Keyboop/history.enc` (AES-GCM encrypted JSON) holds dictation and clipboard history. The key is in Keychain, migrated from a `.histkey` file next to it. Optional history password: `.histgate` (PBKDF2, 210,000 iterations, mode 0600). Entries: dictation up to 3000, clipboard up to 100, imported/call up to 20 each.
- Retention choices in minutes (`HistoryPolicy.retentionChoices`): 30, 60, 480, 7 days, 30 days, 0 (keep all). Default is `voiceHistoryMinutes` = 60 (migrated from the old `voiceHistoryDays`).
- `~/Library/Logs/Keyboop.log` (dev: `Keyboop-dev.log`): capped at 1 MiB, mode 0600. The log intentionally never contains typed content, only lengths.
- Resource-guard files (globe receipt and helper state), also in Application Support. These are macOS-only.
- The Caps remap is stored in the OS (`hidutil`), tracked by `capsRemapApplied`.
- `NSInitialToolTipDelay` is a global tooltip setting written into the app domain.
- Sparkle's own keys (e.g. `SUScheduledCheckInterval`) are managed by Sparkle.

**Counters shown in About:** `rescuedCount` (incremented per fixed word, per fixed group, per selection word count), `voiceChars`, `voiceWords`.

---

## 7. OS integration

**Permissions**
- **Accessibility** (`AXIsProcessTrusted`, prompt via `AXIsProcessTrustedWithOptions`). Required for the tap and for AX reads and writes.
- **Input Monitoring** (`IOHIDCheckAccess` / `IOHIDRequestAccess`, `kIOHIDRequestTypeListenEvent`). Required for the Caps LED and checked after tap start; requested once, polled up to 60 s.
- **Microphone** and audio-capture usage strings exist in the plist (voice, excluded).
- **Flow.**
  1. The welcome window handles first-run grants.
  2. The Accessibility alert ("Allow"/"Later") appears once if the tap cannot start. After an update it shows a "re-grant" variant instead.
  3. The status menu shows permission items.
  4. Links: `x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility`, `...Privacy_ListenEvent`, `...Privacy_Microphone`, `x-apple.systempreferences:com.apple.LoginItems-Settings.extension`, `...Keyboard-Settings.extension`.
- No notifications permission. The app uses its own banner windows rather than `UNUserNotification`.

**Other integration**
- Launch at login: `SMAppService.mainApp` register/unregister.
- Global hotkeys: all implemented inside the event tap, not through Carbon `RegisterEventHotKey`.
- URL schemes: none registered. `mailto:hi@keyboop.com` and `https://t.me/keyboop` are opened outward. Distributed notification `ru.keyboop.app.openSettings` is used between instances.
- Auto-update via Sparkle. `SUFeedURL=https://keyboop.com/appcast.xml`, `SUPublicEDKey=JHgcY6qatoAU6Tdo02B7mHgfceMyfdPWXqwWQiMESmY=`, `SUEnableAutomaticChecks=true`, `SUAutomaticallyUpdate=true`, `SUEnableSystemProfiling=false`, `SUVerifyUpdateBeforeExtraction=true`, `SURequireSignedFeed=true`, `SUScheduledCheckInterval=7200` (2 h).
  - Behavior (`UpdaterController.swift`):
    - It downloads in the background, then intercepts `willInstallUpdateOnQuit`.
    - It shows its own banner "Keyboop %@ is ready" with "Update" and "Update automatically".
    - Silent install only when `silentAutoUpdate` is on and the user is idle: no dictation, no model download, no window open.
    - A beta channel via `allowedChannels` returns `["beta"]` when enabled; otherwise stable (items with no channel).
    - Pending install rights are released after 10 minutes without a decision.
    - Dev builds disable Sparkle unless `KEYBOOP_UPDATER=1`.
    - `feedParametersForUpdater` is not implemented, so it sends no extra data.
- Sandbox: **not sandboxed**.
  - `Keyboop.entitlements` has `app-sandbox=false`, `device.audio-input=true`, `device.microphone=true`. The dev variant adds `cs.disable-library-validation`.
- Info.plist (generated by `build-app.sh`):

| Key | Value |
|---|---|
| `LSUIElement` | true |
| `CFBundleIdentifier` | `ru.keyboop.app` (dev: `.dev`) |
| `CFBundleVersion` / `CFBundleShortVersionString` | 0.4.10 |
| `LSMinimumSystemVersion` | 13.0 |
| `LSMinimumSystemVersionByArchitecture` | arm64 14.0, x86_64 13.0 |
| `NSPrincipalClass` | NSApplication |
| `CFBundleAllowMixedLocalizations` | true |
| `NSAccentColorName` | AccentColor |
| `NSPrefersDisplaySafeAreaCompatibilityMode` | false (notch) |
| `MDItemKeywords` | `лунищщз, keyboop, раскладка, punto, switcher` (Spotlight aliases; release only) |
| `NSHumanReadableCopyright` | "Keyboop — free & open source" |
| `KeyboopBuildStamp` | date-time of the build |

- Network endpoints: Sparkle feed (above), `https://keyboop.com/api/feedback` (JSON `{text, version, kind:"feedback", contact?, diag?, attachment?:{type,size}}`, 15 s timeout, expects HTTP 200), `https://keyboop.com/api/feedback/attachment` (file upload with `X-Keyboop-Upload` token header, 120 s timeout), and the Parakeet model mirror (voice).
- Crash reporting: `CrashNote` reads `~/Library/Logs/DiagnosticReports/Keyboop*.ips` at launch and writes a path-scrubbed summary to the log.
- Telemetry: none.

---

## 8. Architecture

**Third-party dependencies** (none vendored in the repo; `vendor/` is gitignored):

| Dependency | License | Role | Version |
|---|---|---|---|
| Sparkle | MIT | Updates | not stated |
| whisper.cpp | MIT | Voice (excluded) | not stated |
| FluidAudio | Apache-2.0 | Voice (excluded) | not stated |

- The Swift toolchain is 6.2 (`Package.swift`), built in Swift 5 language mode.
- There are no Swift package dependencies in `Package.swift`.

**Threading model**
- Main thread: event tap callback (run loop source on main), all AppKit, timers.
- The tap callback must stay under a few ms. Heavy work (layout selection, UI) is bounced with `DispatchQueue.main.async`.
- Synthetic keystrokes run on a serial queue `ru.keyboop.synth` (`.userInteractive`), except the inline path, which posts inside the tap callback through the tap proxy.
- Other background queues: `ru.keyboop.log` (file writes), `ru.keyboop.spotlight` (window probe), global `.utility` for ioreg, seeding, and caret probe `.userInitiated`.
- `Warm.prime()` loads `LayoutData` (about 5 MB of JSON, 162k-word Russian set) and `TypoFix` rules on a background queue. Until ready (`Warm.isReady`), auto-conversion does nothing.

**File map (non-voice).** Roughly 70 files; the excluded voice set is listed in section 13.

| File | Responsibility |
|---|---|
| `main.swift` | Entry. Globe-guard helper dispatch, creates NSApplication with accessory policy. |
| `AppDelegate.swift` | Lifecycle, single instance, permissions, watchdog, alerts, Punto detection, signal handlers, dev hooks. |
| `AppSettings.swift` | All UserDefaults accessors and defaults. |
| `EventTap.swift` | CGEventTap, gestures, hotkey routing, chatter filter. |
| `Engine.swift` | Core controller (buffer to detector to replacer to layout), exceptions seeding, secure-input timer, per-app policies. |
| `KeystrokeBuffer.swift` | Word, tail and group state. |
| `LayoutDetector.swift`, `LayoutData.swift`, `ExtraWords.swift`, `AmbiguousPairs.swift` | Detection and data. |
| `Keymap.swift`, `DynamicKeymap.swift`, `KeyboardLayoutCache.swift`, `KeyLabels.swift` | Layout translation and labelling. |
| `LayoutManager.swift`, `LayoutCycle.swift` | OS input-source control. |
| `TextReplacer.swift` | Synthetic key output. |
| `TypoFix.swift`, `NumericTypoRule.swift`, `DoubleSpacePeriod.swift`, `UndoLearner.swift`, `AntiResonanceGuard.swift`, `ExceptionStore.swift` | Corrections and learning. |
| `SelectionText.swift`, `AXScreenCheck.swift`, `CaretLocator.swift` | AX read/write helpers. `CaretLocator` was not read. |
| `SecureInputPolicy.swift`, `SecureInputProbe.swift` | Secure-input checks and a dev probe. |
| `SpotlightWatch.swift`, `RivalTapWatch.swift` | Environment detection. |
| `SnippetStore.swift`, `TextSnippetStore.swift`, `SnippetPicker.swift`, `SnippetsEditor.swift` | Snippets. |
| `PlainPaste.swift`, `ClipboardWatcher.swift`, `ClipboardHistoryCore.swift`, `HistoryGate.swift` | Paste and clipboard history. |
| `TranslationEngine.swift` | Apple Translation host. |
| `Pause.swift`, `AppHealth.swift`, `IdleMonitor.swift`, `InputLatency.swift` | State and timing. |
| `Sounds.swift`, `CueSynth.swift` | Sound gate and synthesis. |
| `MenuBarController.swift`, `SettingsWindow.swift`, `UIControls.swift`, `HotkeyRecorderPanel.swift`, `WelcomeWindow.swift`, `AppBanner.swift`, `AnimatedLogoView.swift`, `KeyboopMark.swift`, `DockPresence.swift`, `FeedbackWindow.swift`, `FeedbackAttachmentCore.swift` | UI. |
| `L10n.swift`, `Changelog.swift` | Localization (about 800 keys, ru and en) and release notes. |
| `Permissions.swift`, `UpdaterController.swift`, `Logger.swift`, `CrashNote.swift`, `SystemHotkeys.swift` | System glue. |
| `CapsRemap.swift`, `CapsLED.swift`, `GlobeKey.swift`, `GlobeGuard.swift`, `PersistentResourceGuard.swift`, `ResourceGuard.swift`, `ResourceGuardProtocol.swift` | Mac-only key and hardware machinery. |
| `SlapDetector.swift`, `SlapSPUDriver.swift`, `SlapSPUEventCounter.swift`, `SlapTypes.swift` | Disabled slap gesture. |
| `Warm.swift` | Background data warm-up flag. |

**Key relationships.** `AppDelegate` owns `Engine`, `MenuBarController` and the window controllers. `Engine` owns `LayoutManager`, `KeystrokeBuffer`, `EventTap` and `AntiResonanceGuard`, and reads singletons `AppSettings`, `ExceptionStore`, `UndoLearner`, `TypoFix`, `LayoutData` and `SnippetStore`. `EventTap` calls back through the `EventTapHandler` protocol.

**Helper process.** The same binary is launched as `Keyboop --globe-guard <pid>` or `--resource-guard-v3` (`main.swift` handles it before any UI). It is a sleeping watchdog that restores the 🌐 key setting if the app dies. It is macOS-only.

---

## 9. Build and distribution

- **Build.** The real build is `build-app.sh` (a ~32 KB bash script that calls `swiftc` directly on all sources; SwiftPM is called "broken in this CLT" in its comment). `Package.swift` also exists (executable target, `.macOS(.v13)`, links IOKit, Swift 5 mode) but is not what ships.
  - The compile uses `-target arm64-apple-macos14.0`. An optional `KEYBOOP_UNIVERSAL=1` builds an additional x86_64 slice targeting `macos13.0` without FluidAudio, then `lipo`s them.
  - Frameworks linked: AppKit, Carbon, ServiceManagement, ApplicationServices, IOKit, AVFoundation, CoreAudio, AudioToolbox, Metal, MetalKit, Accelerate, CoreML, SwiftUI, Translation (weak), Sparkle.
  - It also links whisper/ggml static libs and `libFluidAudio.a` (voice).
  - It checks `L10n.swift` for duplicate keys, which would otherwise crash at startup.
- **Resources copied:** the `*.json` files, `menubar-mark.png`, `keyboop-logo-anim.mp4`, a compiled asset catalog (`actool`), and `AppIcon.icns`.
- **Signing.** Dev build: self-signed "Keyboop Dev" with `--timestamp=none`, `Keyboop-dev.entitlements`, bundle ID `ru.keyboop.app.dev`, with `SUFeedURL` and `MDItemKeywords` deleted. Release (`KEYBOOP_RELEASE=1`): Developer ID, hardened runtime (`--options runtime --timestamp`), `Keyboop.entitlements`. Notarization and DMG scripts are not in the repo.
- Install helper `install-local.sh` builds, replaces `/Applications/Keyboop.app`, verifies `codesign --deep --strict`, and launches.
- **Bundle:** ID `ru.keyboop.app`, version 0.4.10 (HEAD commit `fb9bdde "Keyboop 0.4.10"`). Min OS is macOS 13 (Intel) or 14 (Apple Silicon).
- **CI:** none. The project is distributed as a notarized DMG from keyboop.com, with a Sparkle appcast.
- The README says the original author used Claude as co-author.

---

## 10. License and attribution

`LICENSE` (full text): MIT License, "Copyright (c) 2026 Keyboop contributors". The standard MIT permission and warranty-disclaimer text follows. The CLAUDE.md requirement to carry this notice into the rewrite applies directly.

`THIRD_PARTY.md` lists:
1. **Sparkle**, MIT (Copyright (c) 2006-2013 Andy Matuschak, 2009-2013 Elgato Systems GmbH, 2011-2014 Kornel Lesiński, and bundled components).
2. **whisper.cpp**, MIT (Copyright (c) 2023-2026 The ggml authors). The Whisper model weights are OpenAI, MIT, and are downloaded by the user.
3. **FluidAudio**, Apache-2.0.
4. **NVIDIA Parakeet TDT**, CoreML port by FluidInference, **CC-BY-4.0** (attribution required, and shown in About). It is downloaded from `keyboop.com/models/parakeet/parakeet-v3.tar.gz`. A future Nemotron model would need OpenMDW-1.1.
5. **Language data** (`trigrams_ru/en.json`, `words_ru/en.json`) comes from **keyswitcher** (github.com/graninilya/keyswitcher), **MIT, Copyright (c) 2026 Ilya Granin**. This is the one third-party asset relevant to the rewrite: its MIT notice must be preserved if you reuse the files.
   - The file also says you could generate your own trigram model from wordfreq or FrequencyWords to drop this dependency.
6. Apple system frameworks (no attribution needed).

**Unknown or unattributed:**
- `typo_rules.json` (4,874 ru and 155 en rules) and the in-code `ExtraWords` lists have no origin stated in `THIRD_PARTY.md`.
- `keyboop-logo-anim.mp4`, `menubar-mark.png`, `AppIcon.icns` and the mascot/brand ("boop family", the coral palette) are the original author's brand assets. No license is stated for them separately. Treat them as not reusable, and do not carry the Keyboop name or brand over.

---

## 11. macOS-specific pieces needing a cross-platform equivalent (non-voice)

1. **Global key capture and swallow.**
   - Uses CGEventTap (active, session level) with event swallow and keyUp pairing.
   - Windows: `WH_KEYBOARD_LL` hook (swallow by returning non-zero).
2. **Key injection.** `CGEvent.post` of backspaces and Unicode strings (12 UTF-16 units per event) with source markers. Windows: `SendInput` with `KEYEVENTF_UNICODE`. The synthetic-marker scheme needs an equivalent (Windows `dwExtraInfo`).
3. **Layout detection and switching.**
   - TIS (`TISSelectInputSource`, `TISCopyCurrentKeyboardInputSource`, `UCKeyTranslate`) plus `com.apple.HIToolbox` prefs.
   - Windows: `GetKeyboardLayout`/`ActivateKeyboardLayout`, `ToUnicodeEx` and `MapVirtualKeyEx` per HKL.
   - The dynamic key table is built from live layouts and cannot be hard-coded, but the JCUKEN fallback table is fine to ship.
4. **Accessibility text APIs** (focused element, selected text, selected range, string-for-range, write selected text, secure-text-field subrole, messaging timeouts). Windows: UI Automation (`TextPattern`) with fallback. Many apps expose nothing; the clipboard fallback exists for that.
5. **Secure Input detection** (`IsSecureEventInputEnabled`, `ioreg` holder lookup). No clean equivalent; Windows password fields are detectable only via UIA `IsPassword`.
6. **Frontmost-app identity** (`NSWorkspace` bundle ID, `didActivateApplicationNotification`, `Electron Framework.framework` detection, `XUL` Gecko detection). Replace with process or executable names.
7. **Spotlight/overlay detection** (CGWindowList). Only relevant if you want a Spotlight-like exclusion on other OSes.
8. **Pasteboard** (`NSPasteboard` changeCount polling, snapshot and restore of all types, transient-type markers). Windows: `GetClipboardSequenceNumber`, clipboard format listener, and `ExcludeClipboardContentFromMonitorProcessing` / `CF_CLIPBOARD_VIEWER_IGNORE`.
9. **Menu-bar status item with template icons, SF Symbols and emoji flags.** Tauri tray: icon, menu, tooltip. Dimming and badges need separate assets. Dark/light template tinting is not automatic on Windows.
10. **Window chrome**: sidebar with vibrancy (`NSVisualEffectView`), transparent titlebar, non-activating floating panels (`NSPanel`, `.nonactivatingPanel`, `.statusBar` level) that never steal focus. The snippet picker and banner depend on not stealing focus. Tauri needs per-OS "no-focus always-on-top" window flags.
11. **Dock-icon toggling** by activation policy. Not applicable on Windows; macOS-only polish.
12. **Launch at login** (`SMAppService`). Windows: Run key or Startup folder.
13. **Single-instance** (`flock` + distributed notification). The Tauri single-instance plugin covers it.
14. **Auto-update** (Sparkle with EdDSA). Tauri updater plugin (minisign).
15. **System shortcut conflict list** (`com.apple.symbolichotkeys`). No generic equivalent; drop or write per-OS lists.
16. **Modifier-only gestures and left/right modifier distinction, Fn/Globe, Caps-as-key.**
    - Right/left modifier keycodes: 54/55 (⌘), 56/60 (⇧), 58/61 (⌥), 59/62 (⌃), 63 (fn), 57 (caps).
    - Caps-as-hotkey uses `hidutil` remap to LANG1 (HID usage 0x700000039 to 0x700000090); Caps LED uses `IOHIDManager` (usage page 0x08, usage 0x02). Windows needs different remapping/LED mechanisms, or a decision to drop these.
17. **Globe-key system setting** (`AppleFnUsageType` in HIToolbox prefs, with a crash-safe guard helper). Mac-only; drop.
18. **Translation** (Apple on-device Translation, macOS 15+). Replace with an offline engine (for example Argos or Bergamot) or cloud, or drop. Note the current design depends on a system language-pack download dialog.
19. **Keychain** (history encryption key), `CFPreferences`, `UserDefaults`. Replace with the OS credential store plus a config file. These only matter if you keep clipboard history.
20. **Crash report parsing** (`.ips`), `proc_pidpath`, `CGGetEventTapList` rival-tap scan. Mac-only; drop.
21. **Localization:** the custom `L10n` dictionary and the `AppleLanguages` sync. Port the ru/en string table.
22. **Fonts and typographic assumptions** (SF Symbols names used for icons) need replacements.

---

## 12. Oddities (keep or drop deliberately)

- **Product framing.** The README and strings are Russian first. The product name, mascot ("boop family"), codenames (Pika, Axolotl…), tone and brand assets are the author's and should not be carried over.
- **Disabled but present code.**
  - The slap gesture (`ReleaseFeatures.slap = false`) uses the Apple SPU accelerometer via private-ish HID commands, with a dedicated crash-safe helper (about 2,000 lines across `SlapDetector`, `SlapSPUDriver`, `SlapSPUEventCounter`, `SlapTypes`, `PersistentResourceGuard`). Cut it.
  - `SlapSound` lists meow, ouch, ouchMale and ouchFemale, but only `.meow` is available (`SlapSound.available = [.meow]`).
  - `voiceLiveDraft` and `voiceStreaming` getters hard-return false.
- **Heavy defensive engineering to keep or simplify.** All of this reflects a bug history in the comments:
  - the synthetic marker scheme, the `muted` state with its 1.2 s self-heal, and `endSyntheticFlight` real-key insurance;
  - the 30 ms / 40 ms boundary scheduling and the pause-fix dummy event (virtual key 255);
  - the layout opinion vs reality reconciliation;
  - tap-storm suspension and the watchdog.
  - They are all workarounds for macOS timing, and many of these races will not exist, or will differ, on Windows/Linux.
- **Per-app hacks with named apps:** Figma canvas (Return/Esc dance), Gecko newline refusal, Chromium/Electron "sacrifice key" (keyCode 79 / F18) after a clipboard read, Spotlight forward-delete.
- **Data format traps.**
  - `history.enc` fields must stay Optional (comment warns about breaking old files).
  - `L10n.swift` duplicate keys crash at launch; the build script checks for this.
  - Several `L10n` keys (for example `menu.perm`) are intentionally left unused.
- **Seed side effects.** The "Unseed Spotlight" migration (`didUnseedExcSpotlight`) removes a previously seeded "off" mode for Spotlight. The default exceptions seed is a one-time scan of installed apps.
- **Typed-content privacy rule.** The log must never contain typed text, only lengths (comments call this "principle №2"). Keep it.
- **`EventTap` quirks:**
  - A system-defined event (type 14) disarms modifier gestures.
  - The globe event is swallowed even when not configured.
  - Digit banks in the snippet picker use `⇧` and `⌘` as bank selectors.
  - `HotkeyGuard` treats keyCodes 10 and 50 as the same key.
- **Settings UI is the largest file** (5,391 lines) but mostly layout code. Its behavior is fully described by the string table and the setting keys above. `Changelog.swift` (1,715 lines, 330 KB) is release-note text only.
- **Possible bugs and inconsistencies noticed:**
  - The detector sets `forceSwap` twice in the word list (`"jpg"` and `"sdk"` appear twice). Harmless.
  - `soundName` is read from defaults with a fallback of "Pop", and the picker's default selection when `soundName` is "" is "No sound". Whether the default UI shows "Pop" or "Keyboop" depends on stored state. A comment says "Our sound first and by default (25.07)" while the registered default is `"Pop"`. I did not resolve which wins on a fresh install.
  - I did not verify the `settings.simpleMode` root-screen contents beyond the `root.*` strings.
  - I could not read the ordering and visual layout of every Settings row.
- **Platform-assumption traps.** The whole word list and trigram model are RU/EN only. A cross-platform rewrite that wants more language pairs will need the pair-specific pieces generalized: the base keymap, `isImpossibleRussianSpelling`, the Russian single-letter set, the numeric `ю` rule, `ExtraWords` and the ambiguous-pair list.
