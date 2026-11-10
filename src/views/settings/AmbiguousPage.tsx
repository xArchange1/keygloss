import { useEffect, useState } from "react";
import { ChevronLeft } from "lucide-react";
import { useTranslation } from "react-i18next";
import * as ipc from "../../lib/ipc";
import type { AmbiguousPair } from "../../lib/ipc";
import { useSettings, useUpdate } from "../../store";
import { Button, Hint, PageTitle, Segmented } from "../../components/ui";

type Choice = "en" | "auto" | "ru";

/**
 * "Who wins": no store of its own. The winner goes into forceSwapWords and the loser is removed,
 * exactly like the original's AmbiguousPairs.choose; "auto" removes both.
 */
export function AmbiguousPage({ onBack }: { onBack: () => void }) {
  const { t } = useTranslation();
  const s = useSettings();
  const update = useUpdate();
  const [pairs, setPairs] = useState<AmbiguousPair[] | null>(null);
  useEffect(() => { ipc.getAmbiguousPairs().then(setPairs).catch(() => setPairs([])); }, []);

  const choice = (p: AmbiguousPair): Choice =>
    s.forceSwapWords.includes(p.ru) ? "ru" : s.forceSwapWords.includes(p.en) ? "en" : "auto";
  const choose = (p: AmbiguousPair, c: Choice) => {
    const force = s.forceSwapWords.filter((w) => w !== p.en && w !== p.ru);
    const winner = c === "en" ? p.en : c === "ru" ? p.ru : null;
    update({
      forceSwapWords: winner ? [...force, winner] : force,
      ignoredWords: winner ? s.ignoredWords.filter((w) => w !== winner) : s.ignoredWords,
    });
  };

  return (
    <>
      <Button variant="ghost" className="-ml-2 mb-3" onClick={onBack}><ChevronLeft size={15} />{t("amb.back")}</Button>
      <PageTitle title={t("amb.title")} sub={t("amb.sub")} />
      {!pairs && <p className="text-fg/60">{t("amb.loading")}</p>}
      <div className="flex flex-col items-center gap-1.5">
        {pairs?.map((p) => (
          <Segmented key={p.en} className="w-[300px]" value={choice(p)} label={`${p.en} / ${p.ru}`}
            onChange={(c) => choose(p, c)}
            options={[
              { value: "en", label: p.en, title: t("amb.rowTip", { en: p.en, ru: p.ru }) },
              { value: "auto", label: t("amb.auto"), title: t("amb.rowTip", { en: p.en, ru: p.ru }) },
              { value: "ru", label: p.ru, title: t("amb.rowTip", { en: p.en, ru: p.ru }) },
            ]} />
        ))}
      </div>
      <Hint>{t("amb.hint")}</Hint>
    </>
  );
}
