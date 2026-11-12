import { useEffect, useId, useState } from "react";
import { useTranslation } from "react-i18next";
import * as ipc from "../../lib/ipc";
import type { Theme, UiLanguage, UpdateChannel } from "../../lib/ipc";
import { useSettings, useUpdate } from "../../store";
import { Button, Card, PageTitle, Row, SectionTitle, Segmented, Select, SwitchRow } from "../../components/ui";

type UpdateState = "idle" | "checking" | "latest" | "available" | "installing" | "failed";

export function GeneralPage() {
  const { t } = useTranslation();
  const s = useSettings();
  const update = useUpdate();
  const [login, setLogin] = useState(false);
  const themeId = useId();
  useEffect(() => { ipc.getLaunchAtLogin().then(setLogin).catch(console.error); }, []);

  const [version, setVersion] = useState("");
  const [upd, setUpd] = useState<UpdateState>("idle");
  const [newVersion, setNewVersion] = useState("");
  const found = (v: string | null) => {
    if (v) setNewVersion(v);
    return v;
  };
  useEffect(() => {
    ipc.appVersion().then(setVersion).catch(console.error);
    ipc.pendingUpdate().then((v) => found(v) && setUpd("available")).catch(console.error);
  }, []);
  const check = () => {
    setUpd("checking");
    ipc.checkForUpdate().then((v) => setUpd(found(v) ? "available" : "latest")).catch((e) => {
      console.error(e);
      setUpd("failed");
    });
  };
  const install = () => {
    setUpd("installing");
    ipc.installUpdate().catch((e) => {
      console.error(e);
      setUpd("failed");
    });
  };

  return (
    <>
      <PageTitle title={t("gen.title")} />
      <Card>
        <Row id={themeId} title={t("gen.theme")} control={
          <Segmented<Theme> value={s.appTheme} label={t("gen.theme")} onChange={(appTheme) => update({ appTheme })}
            options={[
              { value: "system", label: t("gen.themeSystem") },
              { value: "light", label: t("gen.themeLight") },
              { value: "dark", label: t("gen.themeDark") },
            ]} />
        } />
        <Row title={t("gen.language")} control={
          <Select<UiLanguage> value={s.language} label={t("gen.language")} className="w-[170px]"
            onChange={(language) => update({ language })}
            options={[
              { value: "auto", label: t("gen.langAuto") },
              { value: "ru", label: "Русский" },
              { value: "en", label: "English" },
            ]} />
        } />
        <SwitchRow title={t("gen.login")} checked={login}
          onChange={(v) => ipc.setLaunchAtLogin(v).then(setLogin).catch(console.error)} />
      </Card>
      <SectionTitle>{t("upd.title")}</SectionTitle>
      <Card>
        <Row title={t(`upd.${upd}`, { version: newVersion })} sub={t("upd.current", { version })} control={
          upd === "available" || upd === "installing"
            ? <Button variant="primary" disabled={upd === "installing"} onClick={install}>{t("upd.install")}</Button>
            : <Button disabled={upd === "checking"} onClick={check}>{t("upd.check")}</Button>
        } />
        <Row title={t("upd.channel")} sub={t("upd.channelSub")} control={
          <Segmented<UpdateChannel> value={s.updateChannel} label={t("upd.channel")}
            onChange={(updateChannel) => { update({ updateChannel }); setUpd("idle"); }}
            options={[
              { value: "stable", label: t("upd.stable") },
              { value: "beta", label: t("upd.beta") },
            ]} />
        } />
        <SwitchRow title={t("upd.startup")} checked={s.updateCheckOnStartup}
          onChange={(updateCheckOnStartup) => update({ updateCheckOnStartup })} />
        <SwitchRow title={t("upd.auto")} sub={t("upd.autoSub")} disabled={!s.updateCheckOnStartup}
          checked={s.autoUpdate} onChange={(autoUpdate) => update({ autoUpdate })} />
        <SwitchRow title={t("upd.whatsNew")} sub={t("upd.whatsNewSub")} checked={s.showWhatsNew}
          onChange={(showWhatsNew) => update({ showWhatsNew })} />
      </Card>
    </>
  );
}
