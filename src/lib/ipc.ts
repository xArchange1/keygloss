// Commands and events shared with Rust. Mirror: src-tauri/src/ipc.rs and
// crates/keygloss-core/src/{settings,platform}.rs (kept in sync by hand).

import { invoke as tauriInvoke, isTauri } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

/** False in a plain browser (`pnpm dev` without Tauri): commands then fail soft, see below. */
export const inTauri = isTauri();

// ---- Core types (keygloss_core) ----

/** W3C `KeyboardEvent.code` value, e.g. "KeyA", "Pause", "AltRight" (Rust `KeyCode`). */
export type KeyCode = string;

export interface Modifiers {
  ctrl: boolean;
  alt: boolean;
  shift: boolean;
  /** Windows key. */
  meta: boolean;
}

export type Hotkey =
  /** Modifier-only chord, fires on release if nothing else was pressed. */
  | { mode: "combo"; mods: Modifiers }
  /** Two solo taps of a modifier within 300 ms; either side counts. */
  | { mode: "doubletap"; key: KeyCode }
  /** Solo tap of one specific modifier key (left/right distinct). */
  | { mode: "modkey"; key: KeyCode }
  /** A non-modifier key with optional modifiers. */
  | { mode: "key"; key: KeyCode; mods: Modifiers };

export type AppMode = "off" | "soft";
export type Theme = "system" | "light" | "dark";
export type UiLanguage = "auto" | "ru" | "en";
export type Sound = "off" | "beep" | "whoosh" | "custom";
/** "beta" also gets pre-releases. */
export type UpdateChannel = "stable" | "beta";

export interface Settings {
  // Layout switching
  autoEnabled: boolean;
  liveFixEnabled: boolean;
  /** Shortest word (in letters) the automatic fix touches; 1 = no limit. */
  autoMinLetters: number;
  inlineLiveFix: boolean;
  triggerSpace: boolean;
  triggerEnter: boolean;
  triggerTab: boolean;
  enterPreConvert: boolean;
  arrowsCancel: boolean;
  developerMode: boolean;
  dedupeChatter: boolean;
  groupConvert: boolean;
  learnOnUndoEnabled: boolean;
  // Typo fixes
  typoFix: boolean;
  twoCapsFix: boolean;
  // Sound
  sound: Sound;
  /** 0..1 */
  soundVolume: number;
  /** Name of the file picked for "custom"; "" = none. */
  soundFile: string;
  // Hotkeys
  hotkey: Hotkey;
  caseChangeHotkey: Hotkey | null;
  plainPaste: boolean;
  plainPasteHotkey: Hotkey;
  snippetPickHotkey: Hotkey | null;
  // Pause
  pauseMinutes: number;
  /** Unix seconds; 0 = not paused. */
  pausedUntil: number;
  // Exceptions (app keys are lowercase exe names, e.g. "code.exe")
  ignoredWords: string[];
  forceSwapWords: string[];
  learnedWords: string[];
  appExceptionModes: Record<string, AppMode>;
  /** exe name -> "en" | "ru" */
  appForcedLayouts: Record<string, string>;
  seededDefaultApps: string[];
  // Snippets
  /** [trigger, expansion] */
  snippetsOrdered: [string, string][];
  snippetExpandSpace: boolean;
  snippetExpandEnter: boolean;
  snippetExpandTab: boolean;
  /** [name, text] */
  textSnippets: [string, string][];
  // Learning state
  undoDeclinedWords: string[];
  undoStrikeCount: Record<string, number>;
  undoStrikeTime: Record<string, number>;
  typoPersonalWords: Record<string, number>;
  // Layouts
  lastLayoutCyr: string;
  lastLayoutLat: string;
  // General
  language: UiLanguage;
  appTheme: Theme;
  // Updates
  updateCheckOnStartup: boolean;
  /** Install what the startup check finds without asking. */
  autoUpdate: boolean;
  updateChannel: UpdateChannel;
  // App state
  didInitialSetup: boolean;
  didShowWelcome: boolean;
  rescuedCount: number;
}

export interface AppInfo {
  /** Lowercase exe name. */
  id: string;
  name: string;
}

// ---- IPC types (src-tauri/src/ipc.rs) ----

export interface EngineStatus {
  running: boolean;
  /** Unix seconds; null when not paused. */
  pausedUntil: number | null;
  autoEnabled: boolean;
  /** "en", "ru", ... or null if unknown. */
  layout: string | null;
  hookHealthy: boolean;
}

export type HotkeyTarget = "manual" | "caseChange" | "plainPaste" | "snippetPick";
export type HotkeyVerdict = "ok" | "warn" | "blocked";

export interface HotkeyCheck {
  verdict: HotkeyVerdict;
  /** i18n key for warn/blocked. */
  reason: string | null;
  conflictsWith: HotkeyTarget | null;
}

export interface AmbiguousPair {
  en: string;
  ru: string;
}

export interface LearnBanner {
  word: string;
}

export interface Toast {
  /** i18n key. */
  key: string;
  params: Record<string, string>;
}

/** One automatic fix (src-tauri/src/history.rs). */
export interface HistoryEntry {
  /** Unix seconds. */
  at: number;
  from: string;
  to: string;
}

export interface SnippetPickerOpen {
  /** [name, text] */
  snippets: [string, string][];
}

// ---- Browser fallback (dev only: no Tauri, settings live in memory) ----

const NO_MODS: Modifiers = { ctrl: false, alt: false, shift: false, meta: false };
let memSettings: Settings = {
  autoEnabled: true, liveFixEnabled: true, autoMinLetters: 1, inlineLiveFix: true, triggerSpace: true,
  triggerEnter: true, triggerTab: false, enterPreConvert: true, arrowsCancel: true,
  developerMode: false, dedupeChatter: false, groupConvert: true, learnOnUndoEnabled: true,
  typoFix: true, twoCapsFix: true, sound: "beep", soundVolume: 0.6, soundFile: "",
  hotkey: { mode: "key", key: "Pause", mods: NO_MODS }, caseChangeHotkey: null, plainPaste: false,
  plainPasteHotkey: { mode: "key", key: "KeyV", mods: { ...NO_MODS, ctrl: true, shift: true } },
  snippetPickHotkey: null, pauseMinutes: 15, pausedUntil: 0, ignoredWords: ["вк", "тг", "vk"],
  forceSwapWords: [], learnedWords: [], appExceptionModes: {}, appForcedLayouts: {},
  seededDefaultApps: [], snippetsOrdered: [], snippetExpandSpace: true, snippetExpandEnter: true,
  snippetExpandTab: true, textSnippets: [], undoDeclinedWords: [], undoStrikeCount: {},
  undoStrikeTime: {}, typoPersonalWords: {}, lastLayoutCyr: "", lastLayoutLat: "",
  language: "auto", appTheme: "system", didInitialSetup: false, didShowWelcome: false, rescuedCount: 0,
  updateCheckOnStartup: true, autoUpdate: false, updateChannel: "stable",
};

function browserInvoke(cmd: string, args?: Record<string, unknown>): unknown {
  switch (cmd) {
    case "get_settings": return memSettings;
    case "update_settings": return (memSettings = { ...memSettings, ...(args!.patch as object) });
    case "get_engine_status":
      return { running: false, pausedUntil: null, autoEnabled: memSettings.autoEnabled, layout: null, hookHealthy: false };
    case "check_hotkey": return { verdict: "ok", reason: null, conflictsWith: null };
    case "list_apps": return [{ id: "notepad.exe", name: "Notepad" }];
    case "get_ambiguous_pairs": return [{ en: "yt", ru: "не" }, { en: "vs", ru: "мы" }];
    case "get_launch_at_login": return false;
    case "set_launch_at_login": return args!.enabled;
    case "app_version": return "dev";
    case "check_for_update": case "pending_update": case "clear_history": return null;
    case "get_history": return [{ at: Date.now() / 1000 - 60, from: "ghbdtn", to: "привет" }, { at: Date.now() / 1000 - 3600, from: "vbh", to: "мир" }];
  }
}

function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  return inTauri ? tauriInvoke<T>(cmd, args) : Promise.resolve(browserInvoke(cmd, args) as T);
}

// ---- Commands ----

export const getSettings = () => invoke<Settings>("get_settings");
/** Shallow merge; returns the full updated settings. */
export const updateSettings = (patch: Partial<Settings>) =>
  invoke<Settings>("update_settings", { patch });
export const getEngineStatus = () => invoke<EngineStatus>("get_engine_status");
export const pause = (minutes: number) => invoke<EngineStatus>("pause", { minutes });
export const resume = () => invoke<EngineStatus>("resume");
export const startHotkeyRecording = () => invoke<void>("start_hotkey_recording");
export const stopHotkeyRecording = () => invoke<void>("stop_hotkey_recording");
export const checkHotkey = (target: HotkeyTarget, hotkey: Hotkey) =>
  invoke<HotkeyCheck>("check_hotkey", { target, hotkey });
export const listApps = () => invoke<AppInfo[]>("list_apps");
export const getAmbiguousPairs = () => invoke<AmbiguousPair[]>("get_ambiguous_pairs");
export const answerLearnBanner = (word: string, keep: boolean) =>
  invoke<void>("answer_learn_banner", { word, keep });
/** 0-based index into the picker entries. */
export const snippetPickerChoose = (index: number) =>
  invoke<void>("snippet_picker_choose", { index });
export const snippetPickerClose = () => invoke<void>("snippet_picker_close");
export const getLaunchAtLogin = () => invoke<boolean>("get_launch_at_login");
export const setLaunchAtLogin = (enabled: boolean) =>
  invoke<boolean>("set_launch_at_login", { enabled });
export const previewSound = (sound: Sound, volume: number) =>
  invoke<void>("preview_sound", { sound, volume });
/** Opens a file dialog; a picked file becomes the custom sound. False = cancelled; rejects on a file that is not audio. */
export const chooseSoundFile = () => invoke<boolean>("choose_sound_file");
export const appVersion = () => invoke<string>("app_version");
/** Asks GitHub for a newer release on the chosen channel: its version, or null. */
export const checkForUpdate = () => invoke<string | null>("check_for_update");
/** Version found by the last check (the startup one too), or null. */
export const pendingUpdate = () => invoke<string | null>("pending_update");
/** Downloads and installs the update found by the last check; the app restarts. */
export const installUpdate = () => invoke<void>("install_update");
/** Opens the GitHub bug form (`bug`) or feature form in the browser, prefilled with the app version, the Windows version and (bug form) the newest log lines. */
export const reportIssue = (bug: boolean) => invoke<void>("report_issue", { bug });
/** Shows the folder with the log files in Explorer. */
export const openLogs = () => invoke<void>("open_logs");
/** Automatic fixes, newest first. Stored on this computer only. */
export const getHistory = () => invoke<HistoryEntry[]>("get_history");
export const clearHistory = () => invoke<void>("clear_history");
/** Writes a page error to the log file. */
export const logError = (message: string) => invoke<void>("log_error", { message });

// Added by the frontend phase; the Rust side must implement these (see the phase report).
/** Shows (creating if needed) and focuses the settings window. */
export const openSettings = () => invoke<void>("open_settings");
/** Shows (creating if needed) and focuses the welcome window. */
export const openWelcome = () => invoke<void>("open_welcome");
/** The learn banner timed out unanswered: reset the strike count, do not decline the word. */
export const dismissLearnBanner = (word: string) => invoke<void>("dismiss_learn_banner", { word });

// ---- Events ----

interface EventPayloads {
  "settings-changed": Settings;
  "engine-status": EngineStatus;
  "hotkey-captured": Hotkey;
  "learn-banner": LearnBanner;
  toast: Toast;
  "snippet-picker-open": SnippetPickerOpen;
  "snippet-picker-close": null;
  /** Newest first. */
  "history-changed": HistoryEntry[];
}

export type EventName = keyof EventPayloads;

const browserListeners = new Map<string, Set<(p: unknown) => void>>();

export function on<E extends EventName>(
  event: E,
  handler: (payload: EventPayloads[E]) => void,
): Promise<UnlistenFn> {
  if (inTauri) return listen<EventPayloads[E]>(event, (e) => handler(e.payload));
  const set = browserListeners.get(event) ?? new Set();
  browserListeners.set(event, set);
  set.add(handler as (p: unknown) => void);
  return Promise.resolve(() => set.delete(handler as (p: unknown) => void));
}

// Dev only: `__kgEmit("toast", {...})` in the browser console fakes a backend event.
if (!inTauri && import.meta.env.DEV) {
  (window as unknown as Record<string, unknown>).__kgEmit = (event: string, payload: unknown) =>
    browserListeners.get(event)?.forEach((h) => h(payload));
}
