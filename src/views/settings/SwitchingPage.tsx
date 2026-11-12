import { useId, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import * as ipc from "../../lib/ipc";
import type { Hotkey, HotkeyTarget, Settings, Sound } from "../../lib/ipc";
import { PRESETS } from "../../lib/hotkeys";
import { useSettings, useUpdate } from "../../store";
import { HotkeyPicker } from "../../components/HotkeyPicker";
import { Button, Card, CheckPill, Hint, PageTitle, Row, SectionTitle, Select, SwitchRow } from "../../components/ui";

/** Toggle row for an optional hotkey (null = off) plus its picker row while on. */
export function OptionalHotkey({ title, sub, target, field }: {
  title: string; sub: string; target: HotkeyTarget; field: "caseChangeHotkey" | "snippetPickHotkey";
}) {
  const { t } = useTranslation();
  const s = useSettings();
  const update = useUpdate();
  const value = s[field];
  return (
    <>
      <SwitchRow title={title} sub={sub} checked={!!value}
        onChange={(on) => update({ [field]: on ? PRESETS[target][0] : null } as Partial<Settings>)} />
      {value && (
        <Row title={t("common.shortcut")}
          control={<HotkeyPicker target={target} value={value} label={title}
            onChange={(h: Hotkey) => update({ [field]: h } as Partial<Settings>)} />} />
      )}
    </>
  );
}

export function SwitchingPage() {
  const { t } = useTranslation();
  const s = useSettings();
  const update = useUpdate();
  const volId = useId();
  const [badFile, setBadFile] = useState(false);
  // The volume moved since its last preview: played once the pointer or key lets go.
  const volumeMoved = useRef(false);
  const previewVolume = () => {
    if (volumeMoved.current) preview(s.sound, s.soundVolume);
    volumeMoved.current = false;
  };
  const preview = (sound: Sound, volume: number) => ipc.previewSound(sound, volume).catch(console.error);
  // The backend selects and plays a picked file itself.
  const chooseFile = () => ipc.chooseSoundFile().then(() => setBadFile(false), () => setBadFile(true));
  const setSound = (sound: Sound) => {
    if (sound === "custom" && !s.soundFile) return chooseFile();
    update({ sound });
    preview(sound, s.soundVolume);
  };

  return (
    <>
      <PageTitle title={t("switch.title")} sub={t("switch.sub")} />
      <Card>
        <SwitchRow title={t("switch.auto")} sub={t("switch.autoSub")} checked={s.autoEnabled}
          onChange={(v) => update({ autoEnabled: v })} />
        <SwitchRow title={t("switch.live")} sub={t("switch.liveSub")} disabled={!s.autoEnabled}
          checked={s.autoEnabled && s.liveFixEnabled} onChange={(v) => update({ liveFixEnabled: v })} />
        <SwitchRow title={t("switch.group")} disabled={s.autoEnabled}
          sub={s.autoEnabled ? t("switch.groupAutoOff") : t("switch.groupSub")}
          checked={!s.autoEnabled && s.groupConvert} onChange={(v) => update({ groupConvert: v })} />
        <Row title={t("switch.minLetters")} sub={t("switch.minLettersSub")} disabled={!s.autoEnabled} control={
          <Select value={String(s.autoMinLetters)} label={t("switch.minLetters")} className="w-[64px]"
            onChange={(v) => update({ autoMinLetters: Number(v) })}
            options={["1", "2", "3", "4", "5", "6"].map((n) => ({ value: n, label: n }))} />
        } />
        <SwitchRow title={t("switch.dev")} sub={t("switch.devSub")} checked={s.developerMode}
          onChange={(v) => update({ developerMode: v })} />
        <Row title={t("switch.manual")} sub={t("switch.manualSub")}
          control={<HotkeyPicker target="manual" value={s.hotkey} label={t("switch.manual")}
            onChange={(hotkey) => update({ hotkey })} />} />
        <SwitchRow title={t("switch.learn")} sub={t("switch.learnSub")} checked={s.learnOnUndoEnabled}
          onChange={(v) => update({ learnOnUndoEnabled: v })} />
      </Card>

      <SectionTitle>{t("switch.trig")}</SectionTitle>
      <Card>
        <Row title={t("switch.trigAfter")} control={
          <div className="flex gap-1.5">
            <CheckPill checked={s.triggerSpace} onChange={(v) => update({ triggerSpace: v })}>{t("keys.space")}</CheckPill>
            <CheckPill checked={s.triggerEnter} onChange={(v) => update({ triggerEnter: v })}>{t("keys.enter")}</CheckPill>
            <CheckPill checked={s.triggerTab} onChange={(v) => update({ triggerTab: v })}>{t("keys.tab")}</CheckPill>
          </div>
        } />
        <SwitchRow title={t("switch.arrows")} sub={t("switch.arrowsSub")} checked={s.arrowsCancel}
          onChange={(v) => update({ arrowsCancel: v })} />
      </Card>

      <SectionTitle>{t("switch.fixSection")}</SectionTitle>
      <Card>
        <SwitchRow title={t("switch.typoFix")} sub={t("switch.typoFixSub")} checked={s.typoFix}
          onChange={(v) => update({ typoFix: v })} />
        <SwitchRow title={t("switch.twoCaps")} sub={t("switch.twoCapsSub")} checked={s.twoCapsFix}
          onChange={(v) => update({ twoCapsFix: v })} />
        <SwitchRow title={t("switch.chatter")} sub={t("switch.chatterSub")} checked={s.dedupeChatter}
          onChange={(v) => update({ dedupeChatter: v })} />
        <OptionalHotkey title={t("switch.caseTitle")} sub={t("switch.caseSub")} target="caseChange" field="caseChangeHotkey" />
      </Card>

      <SectionTitle>{t("switch.pasteSection")}</SectionTitle>
      <Card>
        <SwitchRow title={t("switch.plain")} sub={t("switch.plainSub")} checked={s.plainPaste}
          onChange={(v) => update({ plainPaste: v })} />
        {s.plainPaste && (
          <Row title={t("common.shortcut")}
            control={<HotkeyPicker target="plainPaste" value={s.plainPasteHotkey} label={t("switch.plain")}
              onChange={(plainPasteHotkey) => update({ plainPasteHotkey })} />} />
        )}
      </Card>

      <SectionTitle>{t("switch.soundSection")}</SectionTitle>
      <Card>
        <Row title={t("switch.soundOn")} control={
          <Select<Sound> value={s.sound} label={t("switch.soundOn")} className="w-[170px]" onChange={setSound}
            options={[
              { value: "off", label: t("switch.soundOff") },
              { value: "beep", label: t("switch.soundBeep") },
              { value: "whoosh", label: t("switch.soundWhoosh") },
              { value: "custom", label: t("switch.soundCustom") },
            ]} />
        } />
        {s.sound === "custom" && (
          <Row title={s.soundFile || t("switch.soundNoFile")} sub={t("switch.soundFileSub")}
            control={<Button onClick={chooseFile}>{t("switch.soundChoose")}</Button>} />
        )}
        <Row id={volId} title={t("switch.volume")} disabled={s.sound === "off"} control={
          <div className="flex items-center gap-3">
            <input type="range" min={0} max={1} step={0.05} value={s.soundVolume} aria-labelledby={volId}
              disabled={s.sound === "off"} className="w-44 cursor-pointer"
              onChange={(e) => { volumeMoved.current = true; update({ soundVolume: Number(e.target.value) }); }}
              onPointerUp={previewVolume} onKeyUp={previewVolume} />
            <span className="w-9 text-right text-[12px] text-fg/70 tabular-nums">{Math.round(s.soundVolume * 100)}%</span>
          </div>
        } />
      </Card>
      {badFile && <Hint tone="warn">{t("switch.soundBadFile")}</Hint>}
    </>
  );
}
