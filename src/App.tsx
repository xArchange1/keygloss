import { useEffect } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { inTauri } from "./lib/ipc";
import { applyTheme } from "./lib/theme";
import { applyLanguage } from "./i18n";
import { initStore, useStore } from "./store";
import { SettingsView } from "./views/settings/SettingsView";
import { WelcomeView } from "./views/WelcomeView";
import { BannerView } from "./views/BannerView";
import { PickerView } from "./views/PickerView";

const VIEWS = ["settings", "welcome", "banner", "picker"] as const;
type View = (typeof VIEWS)[number];

/** The Tauri window label picks the view; `?view=` stands in for it in a plain browser. */
const view: View = (() => {
  const name = inTauri ? getCurrentWindow().label : new URLSearchParams(location.search).get("view");
  return (VIEWS as readonly string[]).includes(name ?? "") ? (name as View) : "settings";
})();
document.documentElement.dataset.view = view;
initStore();

export default function App() {
  const settings = useStore((s) => s.settings);
  useEffect(() => {
    if (!settings) return;
    applyTheme(settings.appTheme);
    applyLanguage(settings.language);
  }, [settings?.appTheme, settings?.language]);

  if (view === "banner") return <BannerView />;
  if (view === "picker") return <PickerView />;
  if (!settings) return null;
  return view === "welcome" ? <WelcomeView /> : <SettingsView />;
}
