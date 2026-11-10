// Commands and events shared with Rust. Mirror: src-tauri/src/ipc.rs and
// crates/keygloss-core/src/{settings,platform}.rs (kept in sync by hand).

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

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

export interface Settings {
  // Layout switching
  autoEnabled: boolean;
  liveFixEnabled: boolean;
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
  soundEnabled: boolean;
  /** 0..1 */
  soundVolume: number;
  silentMode: boolean;
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

export interface SnippetPickerOpen {
  /** [name, text] */
  snippets: [string, string][];
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
export const appVersion = () => invoke<string>("app_version");

// ---- Events ----

interface EventPayloads {
  "settings-changed": Settings;
  "engine-status": EngineStatus;
  "hotkey-captured": Hotkey;
  "learn-banner": LearnBanner;
  toast: Toast;
  "snippet-picker-open": SnippetPickerOpen;
  "snippet-picker-close": null;
}

export type EventName = keyof EventPayloads;

export function on<E extends EventName>(
  event: E,
  handler: (payload: EventPayloads[E]) => void,
): Promise<UnlistenFn> {
  return listen<EventPayloads[E]>(event, (e) => handler(e.payload));
}
