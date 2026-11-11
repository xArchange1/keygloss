import { getCurrentWindow } from "@tauri-apps/api/window";
import { inTauri, type Theme } from "./ipc";

/** Applies the appTheme setting: "system" follows the OS, otherwise forces light or dark. */
export function applyTheme(theme: Theme) {
  const root = document.documentElement;
  if (theme === "system") root.removeAttribute("data-theme");
  else root.setAttribute("data-theme", theme);
  // The native title bar follows the OS theme unless the window is told otherwise.
  if (inTauri) getCurrentWindow().setTheme(theme === "system" ? null : theme).catch(console.error);
}
