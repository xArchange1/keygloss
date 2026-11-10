import type { Theme } from "./ipc";

/** Applies the appTheme setting: "system" follows the OS, otherwise forces light or dark. */
export function applyTheme(theme: Theme) {
  const root = document.documentElement;
  if (theme === "system") root.removeAttribute("data-theme");
  else root.setAttribute("data-theme", theme);
}
