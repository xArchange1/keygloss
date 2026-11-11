import { useEffect, useId, useState } from "react";
import { useTranslation } from "react-i18next";
import * as ipc from "../../lib/ipc";
import type { Theme, UiLanguage } from "../../lib/ipc";
import { useSettings, useUpdate } from "../../store";
import { Card, PageTitle, Row, Segmented, Select, SwitchRow } from "../../components/ui";

export function GeneralPage() {
  const { t } = useTranslation();
  const s = useSettings();
  const update = useUpdate();
  const [login, setLogin] = useState(false);
  const themeId = useId();
  useEffect(() => { ipc.getLaunchAtLogin().then(setLogin).catch(console.error); }, []);

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
    </>
  );
}
