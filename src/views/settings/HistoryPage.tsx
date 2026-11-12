import { useEffect, useState } from "react";
import { Ban, Lock } from "lucide-react";
import { useTranslation } from "react-i18next";
import * as ipc from "../../lib/ipc";
import type { HistoryEntry } from "../../lib/ipc";
import { useSettings, useUpdate } from "../../store";
import { Button, Card, PageTitle, Row, Segmented } from "../../components/ui";

const without = (words: string[], w: string) => words.filter((x) => x !== w);
const KEEP_DAYS = [0, 3, 7, 30];

export function HistoryPage() {
  const { t, i18n } = useTranslation();
  const s = useSettings();
  const update = useUpdate();
  const [entries, setEntries] = useState<HistoryEntry[]>([]);
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
        <Button className="mt-1" disabled={!entries.length} onClick={clear}>{t("hist.clear")}</Button>
      </div>
      <p className="-mt-2.5 mb-[18px] flex items-start gap-1.5 text-fg/70">
        <Lock size={13} className="mt-[3px] shrink-0 text-detail" />{t("hist.local")}
      </p>
      <Card className="mb-3">
        <Row title={t("hist.auto")} help={t("hist.autoSub")} control={
          <Segmented value={String(s.historyKeepDays)} label={t("hist.auto")}
            onChange={(v) => update({ historyKeepDays: Number(v) })}
            options={KEEP_DAYS.map((d) => ({ value: String(d), label: d ? t("hist.days", { count: d }) : t("hist.autoOff") }))} />
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
