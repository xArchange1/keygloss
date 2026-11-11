import { useEffect, useId, useState } from "react";
import { Bug, EyeOff, FolderOpen, Heart, KeyRound, HardDrive, Lightbulb, WifiOff, Sparkles } from "lucide-react";
import { useTranslation } from "react-i18next";
import * as ipc from "../../lib/ipc";
import type { Theme } from "../../lib/ipc";
import { useSettings, useUpdate } from "../../store";
import { BlockTitle, Button, Card, Hint, PageTitle, Row, Segmented } from "../../components/ui";
import { Brand } from "../../components/Brand";

export function PrivacyPage() {
  const { t } = useTranslation();
  const items = [
    [WifiOff, "priv.net"],
    [EyeOff, "priv.keys"],
    [KeyRound, "priv.passwords"],
    [HardDrive, "priv.storage"],
  ] as const;
  return (
    <>
      <PageTitle title={t("priv.title")} />
      <Card>
        {items.map(([Icon, key]) => (
          <div key={key} className="flex gap-3 px-3.5 py-3">
            <Icon size={17} className="mt-px shrink-0 text-detail" aria-hidden />
            <p>{t(key)}</p>
          </div>
        ))}
      </Card>
    </>
  );
}

export function AboutPage() {
  const { t, i18n } = useTranslation();
  const s = useSettings();
  const update = useUpdate();
  const themeId = useId();
  const [version, setVersion] = useState("");
  useEffect(() => { ipc.appVersion().then(setVersion).catch(console.error); }, []);

  return (
    <>
      <div className="mb-[18px] flex items-center gap-4">
        <Brand size={52} />
        <div>
          <h1 className="text-[22px] font-semibold tracking-tight">Keygloss</h1>
          <p className="text-fg/70">{t("about.tagline")}</p>
        </div>
      </div>
      <Card>
        <Row title={t("about.version")} control={<span className="text-fg/75 tabular-nums">{version}</span>} />
        <Row title={t("about.rescued")} control={
          <span className="font-semibold tabular-nums">{s.rescuedCount.toLocaleString(i18n.language)}</span>
        } />
        <Row title={t("about.license")} control={<span className="text-fg/75">{t("about.licenseVal")}</span>} />
        <Row id={themeId} title={t("gen.theme")} control={
          <Segmented<Theme> value={s.appTheme} label={t("gen.theme")} onChange={(appTheme) => update({ appTheme })}
            options={[
              { value: "system", label: t("gen.themeSystem") },
              { value: "light", label: t("gen.themeLight") },
              { value: "dark", label: t("gen.themeDark") },
            ]} />
        } />
      </Card>
      <div className="mt-3 flex flex-wrap gap-2">
        <Button onClick={() => ipc.openWelcome().catch(console.error)}><Sparkles size={14} />{t("about.welcome")}</Button>
        <Button><Heart size={14} />{t("about.support")}</Button>
      </div>
      <BlockTitle title={t("about.feedback")} />
      <div className="flex flex-wrap gap-2">
        <Button onClick={() => ipc.reportIssue(true).catch(console.error)}><Bug size={14} />{t("about.reportBug")}</Button>
        <Button onClick={() => ipc.reportIssue(false).catch(console.error)}><Lightbulb size={14} />{t("about.requestFeature")}</Button>
        <Button variant="ghost" onClick={() => ipc.openLogs().catch(console.error)}><FolderOpen size={14} />{t("about.openLogs")}</Button>
      </div>
      <Hint>{t("about.feedbackHint")}</Hint>
      <BlockTitle title={t("about.credits")} />
      <div className="space-y-1.5 px-1 text-[12px] text-fg/70">
        <p>{t("about.basedOn")}</p>
        <p>{t("about.data")}</p>
      </div>
    </>
  );
}
