import { useEffect, useState } from "react";
import { ArrowLeftRight, X } from "lucide-react";
import { useTranslation } from "react-i18next";
import * as ipc from "../../lib/ipc";
import type { AppInfo, AppMode } from "../../lib/ipc";
import { useSettings, useUpdate } from "../../store";
import { BlockTitle, Button, Card, Chips, PageTitle, Segmented, Select, TextInput } from "../../components/ui";

const sorted = (words: string[]) => [...words].sort((a, b) => a.localeCompare(b));
const without = (words: string[], w: string) => words.filter((x) => x !== w);

type DeclinedList = "undoDeclinedWords" | "switchDeclinedWords";

/** "No thanks" answers: undo ones hold the typed word, switch ones the switched-to word. Shown as `from → to`. */
function DismissedList() {
  const { t } = useTranslation();
  const s = useSettings();
  const update = useUpdate();
  const words = [...s.undoDeclinedWords, ...s.switchDeclinedWords];
  const [swapped, setSwapped] = useState<Record<string, string>>({});
  useEffect(() => {
    ipc.swapLayout(words).then((out) => setSwapped(Object.fromEntries(words.map((w, i) => [w, out[i]])))).catch(console.error);
  }, [words.join("\n")]);

  const rows = [
    ...s.undoDeclinedWords.map((w) => ({ list: "undoDeclinedWords" as DeclinedList, word: w, from: w, to: swapped[w] })),
    ...s.switchDeclinedWords.map((w) => ({ list: "switchDeclinedWords" as DeclinedList, word: w, from: swapped[w], to: w })),
  ].sort((a, b) => a.word.localeCompare(b.word));
  type Row = (typeof rows)[number];
  const drop = (r: Row) => ({ [r.list]: without(s[r.list], r.word) });
  const toNever = (r: Row) => update({
    ...drop(r), ignoredWords: [...without(s.ignoredWords, r.from), r.from], forceSwapWords: without(s.forceSwapWords, r.from),
  });
  const toAlways = (r: Row) => update({
    ...drop(r), forceSwapWords: [...without(s.forceSwapWords, r.to), r.to], ignoredWords: without(s.ignoredWords, r.to),
  });

  return (
    <Card>
      {rows.length === 0 && <p className="px-3.5 py-3 text-[12px] text-fg/60">{t("exc.dismissedEmpty")}</p>}
      {rows.map((r) => (
        <div key={`${r.list}:${r.word}`} className="flex min-h-9 items-center gap-1.5 px-3 py-1.5">
          <div className="min-w-0 flex-1 truncate text-[12.5px]">
            <span className="font-medium">{r.from ?? "…"}</span>
            <span className="mx-1.5 text-fg/45" aria-label={t("exc.dismissedTo")}>→</span>
            <span>{r.to ?? "…"}</span>
          </div>
          <Button disabled={!r.from} title={t("exc.toNeverHelp", { word: r.from })} onClick={() => toNever(r)}>
            {t("exc.toNever")}
          </Button>
          <Button disabled={!r.to} title={t("exc.toAlwaysHelp", { from: r.from, to: r.to })} onClick={() => toAlways(r)}>
            {t("exc.toAlways")}
          </Button>
          <button type="button" aria-label={t("exc.dismissedRemove", { word: r.word })} title={t("exc.dismissedRemove", { word: r.word })}
            onClick={() => update(drop(r))}
            className="grid size-7 cursor-pointer place-items-center rounded-md text-fg/50 hover:bg-fg/[0.08] hover:text-fg">
            <X size={14} />
          </button>
        </div>
      ))}
    </Card>
  );
}

/** Input + Add button; reports why a word was not added. */
function WordAdder({ list, onAdd }: { list: string[]; onAdd: (w: string) => void }) {
  const { t } = useTranslation();
  const [text, setText] = useState("");
  const [outcome, setOutcome] = useState<string | null>(null);
  const add = () => {
    const w = text.trim().toLowerCase();
    if (!w) return;
    if (/\s/.test(w)) return setOutcome(t("exc.rejected", { word: w }));
    if (list.includes(w)) return setOutcome(t("exc.already", { word: w }));
    onAdd(w);
    setText("");
    setOutcome(null);
  };
  return (
    <>
      <form className="mb-2 flex gap-2" onSubmit={(e) => { e.preventDefault(); add(); }}>
        <TextInput className="flex-1" value={text} placeholder={t("exc.placeholder")} aria-label={t("exc.placeholder")}
          onChange={(e) => { setText(e.target.value); setOutcome(null); }} />
        <Button type="submit">{t("common.add")}</Button>
      </form>
      {outcome && <p role="status" className="mb-2 px-1 text-[12px] font-medium">{outcome}</p>}
    </>
  );
}

export function ExceptionsPage({ onOpenAmbiguous }: { onOpenAmbiguous: () => void }) {
  const { t } = useTranslation();
  const s = useSettings();
  const update = useUpdate();
  const [running, setRunning] = useState<AppInfo[]>([]);
  useEffect(() => { ipc.listApps().then(setRunning).catch(console.error); }, []);

  const names = Object.fromEntries(running.map((a) => [a.id, a.name]));
  const apps = Object.keys(s.appExceptionModes).sort((a, b) => (names[a] ?? a).localeCompare(names[b] ?? b));
  const available = running.filter((a) => !(a.id in s.appExceptionModes));

  const setMode = (id: string, mode: AppMode) => update({ appExceptionModes: { ...s.appExceptionModes, [id]: mode } });
  const setLayout = (id: string, layout: string) => {
    const { [id]: _, ...rest } = s.appForcedLayouts;
    update({ appForcedLayouts: layout ? { ...rest, [id]: layout } : rest });
  };
  const removeApp = (id: string) => {
    const { [id]: _m, ...modes } = s.appExceptionModes;
    const { [id]: _l, ...layouts } = s.appForcedLayouts;
    update({ appExceptionModes: modes, appForcedLayouts: layouts });
  };

  return (
    <>
      <PageTitle title={t("exc.appsTitle")} help={t("exc.appsSub")} />
      <Card>
        {apps.length === 0 && <p className="px-3.5 py-3 text-[12px] text-fg/60">{t("exc.appsEmpty")}</p>}
        {apps.map((id) => (
          <div key={id} className="flex min-h-9 items-center gap-2 px-3 py-1.5">
            <div className="min-w-0 flex-1 truncate">
              <span className="font-medium">{names[id] ?? id}</span>
              {names[id] && <span className="ml-2 text-[11.5px] text-fg/60">{id}</span>}
            </div>
            <Select value={s.appForcedLayouts[id] ?? ""} label={`${t("exc.layout")}: ${names[id] ?? id}`} title={t("exc.layoutHelp")}
              className="w-[68px]" onChange={(v) => setLayout(id, v)}
              options={[{ value: "", label: "—" }, { value: "en", label: "EN" }, { value: "ru", label: "RU" }]} />
            <Segmented value={s.appExceptionModes[id]} label={`${t("exc.mode")}: ${names[id] ?? id}`}
              onChange={(m) => setMode(id, m)}
              options={[{ value: "off", label: t("exc.off") }, { value: "soft", label: t("exc.soft") }]} />
            <button type="button" aria-label={t("common.remove", { name: names[id] ?? id })} onClick={() => removeApp(id)}
              className="grid size-7 cursor-pointer place-items-center rounded-md text-fg/50 hover:bg-fg/[0.08] hover:text-fg">
              <X size={14} />
            </button>
          </div>
        ))}
      </Card>
      <div className="mt-2.5">
        <Select value="" label={t("exc.fromRunning")} className="w-[240px]"
          onChange={(id) => id && setMode(id, "off")}
          options={[
            { value: "", label: available.length ? t("exc.fromRunning") : t("exc.noRunning") },
            ...available.map((a) => ({ value: a.id, label: `${a.name} (${a.id})` })),
          ]} />
      </div>

      <BlockTitle title={t("amb.title")} sub={t("amb.openSub")} />
      <Button onClick={onOpenAmbiguous}><ArrowLeftRight size={14} />{t("amb.open")}</Button>

      <BlockTitle title={t("exc.ignoredTitle")} help={t("exc.ignoredSub")} />
      <WordAdder list={s.ignoredWords}
        onAdd={(w) => update({ ignoredWords: [...s.ignoredWords, w], forceSwapWords: without(s.forceSwapWords, w) })} />
      <Chips words={sorted(s.ignoredWords)} empty={t("exc.empty")}
        onRemove={(w) => update({ ignoredWords: without(s.ignoredWords, w) })} />

      <BlockTitle title={t("exc.forceTitle")} sub={t("exc.forceSub")} />
      <WordAdder list={s.forceSwapWords}
        onAdd={(w) => update({ forceSwapWords: [...s.forceSwapWords, w], ignoredWords: without(s.ignoredWords, w) })} />
      <Chips words={sorted(s.forceSwapWords)} empty={t("exc.empty")}
        onRemove={(w) => update({ forceSwapWords: without(s.forceSwapWords, w) })} />

      <BlockTitle title={t("exc.dismissedTitle")} help={t("exc.dismissedSub")} />
      <DismissedList />
    </>
  );
}
