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
    <div className="mx-auto flex h-full w-full max-w-[480px] flex-col">
      <div className="kg-in flex-1 overflow-y-auto px-8 pt-9 pb-4">
        <Brand size={56} />
        <h1 className="mt-5 text-[26px] font-semibold tracking-tight">{t("wel.hi")}</h1>
        <p className="mt-2 text-[14px] text-fg/80">{t("wel.tagline")}</p>
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

        <SectionTitle>{t("wel.keysTitle")}</SectionTitle>
        <Card>
          <Row title={t("wel.key")} control={
            <HotkeyPicker target="manual" value={s.hotkey} label={t("wel.key")} onChange={(hotkey) => update({ hotkey })} />
          } />
        </Card>
        <p className="mt-2 px-1 text-[12px] text-fg/65">{t("wel.keysNote")}</p>

        <SectionTitle>{t("wel.practiceTitle")}</SectionTitle>
        <textarea rows={3} spellCheck={false} placeholder={t("wel.practicePh")} aria-label={t("wel.practiceTitle")}
          className="w-full resize-none rounded-xl border border-fg/15 bg-raised px-3 py-2.5 text-[14px] placeholder:text-fg/45 focus:border-detail" />
        <p className="mt-1.5 px-1 text-[12px] text-fg/65">{t("wel.practiceHint")}</p>
      </div>
      <footer className="flex justify-end gap-2 border-t border-fg/[0.08] px-8 py-4">
        <Button onClick={() => finish(true)}>{t("wel.settings")}</Button>
        <Button variant="primary" className="px-5" onClick={() => finish(false)}>{t("wel.go")}</Button>
      </footer>
    </div>
  );
}
