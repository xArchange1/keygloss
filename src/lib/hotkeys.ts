import type { TFunction } from "i18next";
import type { Hotkey, HotkeyTarget, KeyCode, Modifiers } from "./ipc";

const mods = (m: Partial<Modifiers>): Modifiers => ({ ctrl: false, alt: false, shift: false, meta: false, ...m });
const key = (k: KeyCode, m: Partial<Modifiers> = {}): Hotkey => ({ mode: "key", key: k, mods: mods(m) });

/** Ready-made choices per feature. Default first where the feature has one. */
export const PRESETS: Record<HotkeyTarget, Hotkey[]> = {
  manual: [
    key("Pause"),
    key("ScrollLock"),
    { mode: "modkey", key: "ControlRight" },
    { mode: "modkey", key: "AltRight" },
    { mode: "doubletap", key: "ShiftLeft" },
    { mode: "doubletap", key: "ControlLeft" },
    { mode: "combo", mods: mods({ ctrl: true, alt: true }) },
  ],
  caseChange: [
    key("KeyU", { ctrl: true, alt: true }),
    key("KeyU", { ctrl: true, shift: true }),
    key("KeyK", { ctrl: true, alt: true }),
  ],
  plainPaste: [
    key("KeyV", { ctrl: true, shift: true }),
    key("KeyV", { ctrl: true, alt: true }),
    key("KeyV", { alt: true, shift: true }),
  ],
  snippetPick: [
    key("KeyS", { ctrl: true, alt: true }),
    key("KeyE", { ctrl: true, alt: true }),
    key("Space", { ctrl: true, shift: true }),
  ],
};

const MOD_NAMES: Record<string, string> = { Control: "Ctrl", Alt: "Alt", Shift: "Shift", Meta: "Win" };
const SYMBOLS: Record<string, string> = {
  Backquote: "`", Minus: "-", Equal: "=", BracketLeft: "[", BracketRight: "]", Backslash: "\\",
  IntlBackslash: "\\", Semicolon: ";", Quote: "'", Comma: ",", Period: ".", Slash: "/",
  ArrowLeft: "←", ArrowRight: "→", ArrowUp: "↑", ArrowDown: "↓", Delete: "Del", Insert: "Ins",
  Escape: "Esc", PageUp: "PgUp", PageDown: "PgDn", CapsLock: "Caps Lock", NumLock: "Num Lock",
  ScrollLock: "Scroll Lock", PrintScreen: "PrtSc", ContextMenu: "Menu", NumpadAdd: "Num +",
  NumpadSubtract: "Num −", NumpadMultiply: "Num *", NumpadDivide: "Num /", NumpadDecimal: "Num .",
  NumpadEnter: "Num Enter",
};

/** Name of a modifier key without its side: "ControlRight" -> "Ctrl". */
function modName(code: KeyCode) {
  return MOD_NAMES[code.replace(/(Left|Right)$/, "")] ?? code;
}

export function keyName(code: KeyCode, t: TFunction): string {
  if (/^Key[A-Z]$/.test(code)) return code.slice(3);
  if (/^Digit\d$/.test(code)) return code.slice(5);
  if (/^Numpad\d$/.test(code)) return `Num ${code.slice(6)}`;
  if (code === "Space") return t("keys.space");
  if (/(Left|Right)$/.test(code) && MOD_NAMES[code.replace(/(Left|Right)$/, "")]) {
    return t(code.endsWith("Left") ? "keys.left" : "keys.right", { key: modName(code) });
  }
  return SYMBOLS[code] ?? code;
}

function modParts(m: Modifiers) {
  return [m.ctrl && "Ctrl", m.alt && "Alt", m.shift && "Shift", m.meta && "Win"].filter(Boolean) as string[];
}

/** Key-cap labels for a hotkey, in press order. */
export function hotkeyParts(h: Hotkey, t: TFunction): string[] {
  switch (h.mode) {
    case "combo": return modParts(h.mods);
    case "doubletap": return [t("keys.double", { key: modName(h.key) })];
    case "modkey": return [keyName(h.key, t)];
    case "key": return [...modParts(h.mods), keyName(h.key, t)];
  }
}

/** Equality as the engine sees it (a double tap accepts either side). */
export function sameHotkey(a: Hotkey | null, b: Hotkey | null): boolean {
  const norm = (h: Hotkey | null) => {
    if (!h) return "";
    if (h.mode === "combo") return `combo:${modParts(h.mods)}`;
    if (h.mode === "doubletap") return `dt:${modName(h.key)}`;
    if (h.mode === "modkey") return `mk:${h.key}`;
    return `key:${h.key}:${modParts(h.mods)}`;
  };
  return norm(a) === norm(b);
}
