import { useEffect, useRef, useState } from "react";
import { Ban, Lock, Minus, Plus } from "lucide-react";
import { useTranslation } from "react-i18next";
import * as ipc from "../../lib/ipc";
import type { HistoryEntry } from "../../lib/ipc";
import { useSettings, useUpdate } from "../../store";
import { Button, Card, PageTitle, Row, SwitchRow, Toggle } from "../../components/ui";

const without = (words: string[], w: string) => words.filter((x) => x !== w);
const MAX_DAYS = 30;
const clampDays = (n: number) => Math.min(MAX_DAYS, Math.max(1, n));

export function HistoryPage() {
  const { t, i18n } = useTranslation();
  const s = useSettings();
  const update = useUpdate();
  const [entries, setEntries] = useState<HistoryEntry[]>([]);
  // The last period, restored when automatic clearing is turned back on.
  const lastDays = useRef(7);
  if (s.historyKeepDays) lastDays.current = s.historyKeepDays;
  useEffect(() => {
    const off = ipc.on("history-changed", setEntries);
    return () => { off.then((f) => f()); };
  }, []);
  // Refetched when the period changes: reading the history removes what is now too old.
  useEffect(() => {
    ipc.getHistory().then(setEntries).catch(console.error);
  }, [s.historyKeepDays]);

  const toggleNever = (word: string) =>
    s.ignoredWords.includes(word)
      ? update({ ignoredWords: without(s.ignoredWords, word) })
      : update({ ignoredWords: [...s.ignoredWords, word], forceSwapWords: without(s.forceSwapWords, word) });
  const clear = () => ipc.clearHistory().then(() => setEntries([])).catch(console.error);

  return (
    <>
      <div className="flex items-start justify-between gap-4">
        <PageTitle title={t("hist.title")} />
        <Button className={s.historyEnabled ? "mt-1" : "mt-1 opacity-50"} disabled={!entries.length} onClick={clear}>{t("hist.clear")}</Button>
      </div>
      <p className="-mt-2.5 mb-[18px] flex items-start gap-1.5 text-fg/70">
        <Lock size={13} className="mt-[3px] shrink-0 text-detail" />{t("hist.local")}
      </p>
      <Card className="mb-3">
        <SwitchRow title={t("hist.enabled")} checked={s.historyEnabled} onChange={(historyEnabled) => update({ historyEnabled })} />
        <Row title={t("hist.auto")} help={t("hist.autoSub")} disabled={!s.historyEnabled} control={
          <div className="flex items-center gap-3">
            {s.historyKeepDays > 0 && (
              <DaysStepper value={s.historyKeepDays} onChange={(historyKeepDays) => update({ historyKeepDays })} />
            )}
            <Toggle label={t("hist.auto")} checked={s.historyKeepDays > 0}
              onChange={(on) => update({ historyKeepDays: on ? lastDays.current : 0 })} />
          </div>
        } />
      </Card>
      <Card>
        {entries.length === 0 && <p className="px-3.5 py-3 text-[12px] text-fg/60">{t("hist.empty")}</p>}
        {entries.map((e, i) => {
          const word = e.from.toLowerCase();
          const never = s.ignoredWords.includes(word);
          return (
            <div key={`${e.at}-${i}`} className="flex items-center gap-3 px-3.5 py-2.5">
              <div className="min-w-0 flex-1">
                <div className="text-[11.5px] font-semibold text-fg/60">
                  {new Date(e.at * 1000).toLocaleString(i18n.language, { dateStyle: "long", timeStyle: "short" })}
                </div>
                <div className="mt-0.5 truncate">
                  <span className="text-fg/55">{e.from}</span>
                  <span className="mx-1.5 text-fg/40">→</span>
                  <span className="font-medium">{e.to}</span>
                </div>
              </div>
              <button type="button" aria-pressed={never} onClick={() => toggleNever(word)}
                className={`inline-flex h-7 shrink-0 cursor-pointer items-center gap-1.5 rounded-md border px-2.5 text-[12px] transition-colors ${
                  never ? "border-accent/60 bg-accent/10 font-medium" : "border-transparent text-fg/65 hover:border-fg/15 hover:text-fg"}`}>
                <Ban size={13} className={never ? "text-accent" : undefined} />{t("hist.never")}
              </button>
            </div>
          );
        })}
      </Card>
    </>
  );
}

/** 1 to 30 days: the buttons step by one, typed input applies on Enter or blur. */
function DaysStepper({ value, onChange }: { value: number; onChange: (v: number) => void }) {
  const { t } = useTranslation();
  const [draft, setDraft] = useState<string | null>(null);
  const commit = () => {
    const n = Number.parseInt(draft ?? "", 10);
    if (n && clampDays(n) !== value) onChange(clampDays(n));
    setDraft(null);
  };
  const step = "grid size-6 cursor-pointer place-items-center rounded-md text-fg/65 hover:bg-fg/10 hover:text-fg disabled:cursor-default disabled:opacity-40 disabled:hover:bg-transparent";
  return (
    <div className="flex items-center gap-1.5">
      <div className="inline-flex items-center rounded-lg bg-fg/[0.08] p-0.5">
        <button type="button" className={step} aria-label={t("hist.fewer")} disabled={value <= 1}
          onClick={() => onChange(clampDays(value - 1))}><Minus size={13} /></button>
        <input inputMode="numeric" aria-label={t("hist.auto")} value={draft ?? String(value)}
          onChange={(e) => setDraft(e.target.value.replace(/\D/g, "").slice(0, 2))} onBlur={commit}
          onKeyDown={(e) => {
            if (e.key === "Enter") commit();
            else if (e.key === "Escape") setDraft(null);
          }}
          className="h-6 w-7 bg-transparent text-center text-[12px] font-semibold tabular-nums" />
        <button type="button" className={step} aria-label={t("hist.more")} disabled={value >= MAX_DAYS}
          onClick={() => onChange(clampDays(value + 1))}><Plus size={13} /></button>
      </div>
      <span className="text-[12px] text-fg/75">{t("hist.days", { count: value })}</span>
    </div>
  );
}
