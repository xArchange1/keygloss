import { Check } from "lucide-react";
import { useTranslation } from "react-i18next";
import { getCurrentWindow } from "@tauri-apps/api/window";
import * as ipc from "../lib/ipc";
import { useSettings, useUpdate } from "../store";
import { Brand } from "../components/Brand";
import { HotkeyPicker } from "../components/HotkeyPicker";
import { Button, Card, Row, SectionTitle } from "../components/ui";

export function WelcomeView() {
  const { t } = useTranslation();
  const s = useSettings();
  const update = useUpdate();

  const finish = (thenSettings: boolean) => {
    update({ didShowWelcome: true });
    if (thenSettings) ipc.openSettings().catch(console.error);
    if (ipc.inTauri) getCurrentWindow().close().catch(console.error);
  };

  return (
    // Capped and centered: a tiling window manager may stretch the window to the whole screen.
    <div className="grid h-full place-items-center">
    <div className="flex h-full max-h-[540px] w-full max-w-[860px] overflow-hidden min-[861px]:rounded-2xl min-[861px]:border min-[861px]:border-fg/10">
      {/* Instructions */}
      <div className="kg-in min-w-0 flex-1 overflow-y-auto px-8 py-7">
        <div className="flex items-center gap-3.5">
          <Brand size={44} />
          <h1 className="text-[24px] font-semibold tracking-tight">{t("wel.hi")}</h1>
        </div>
        <p className="mt-4 text-[14px] text-fg/80">{t("wel.tagline")}</p>
        <p className="mt-2 text-fg/65">{t("wel.tray")}</p>

        <SectionTitle>{t("wel.canTitle")}</SectionTitle>
        <ul className="space-y-2 px-1">
          {(["can1", "can2", "can3", "can4"] as const).map((k) => (
            <li key={k} className="flex gap-2.5">
              <Check size={15} strokeWidth={2.5} className="mt-[2px] shrink-0 text-accent" aria-hidden />
              <span>{t(`wel.${k}`)}</span>
            </li>
          ))}
        </ul>
      </div>

      {/* Try-it panel: its own surface, split off from the instructions */}
      <aside className="flex w-[380px] shrink-0 flex-col border-l border-fg/10 bg-fg/[0.05]">
        <div className="flex min-h-0 flex-1 flex-col px-6 pt-2.5 pb-4">
          <SectionTitle>{t("wel.keysTitle")}</SectionTitle>
          <Card>
            <Row title={t("wel.key")} control={
              <HotkeyPicker target="manual" value={s.hotkey} label={t("wel.key")} onChange={(hotkey) => update({ hotkey })} />
            } />
          </Card>
          <p className="mt-2 px-1 text-[12px] text-fg/65">{t("wel.keysNote")}</p>

          <SectionTitle>{t("wel.practiceTitle")}</SectionTitle>
          <textarea spellCheck={false} placeholder={t("wel.practicePh")} aria-label={t("wel.practiceTitle")}
            className="min-h-[72px] w-full flex-1 resize-none rounded-xl border border-fg/20 bg-raised px-3 py-2.5 text-[14px] shadow-[0_1px_3px_rgb(0_0_0/0.12)_inset] placeholder:text-fg/45 focus:border-detail" />
          <p className="mt-1.5 px-1 text-[12px] text-fg/65">{t("wel.practiceHint")}</p>
        </div>
        <footer className="flex justify-end gap-2 border-t border-fg/10 px-6 py-3.5">
          <Button onClick={() => finish(true)}>{t("wel.settings")}</Button>
          <Button variant="primary" className="px-5" onClick={() => finish(false)}>{t("wel.go")}</Button>
        </footer>
      </aside>
    </div>
    </div>
  );
}
