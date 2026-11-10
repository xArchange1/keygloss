import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { getCurrentWindow } from "@tauri-apps/api/window";
import * as ipc from "../lib/ipc";
import { Brand } from "../components/Brand";
import { Button } from "../components/ui";

type Item = { id: number; kind: "toast"; key: string; params: Record<string, string> } | { id: number; kind: "learn"; word: string };

const TOAST_MS = 4000;
const LEARN_MS = 20000;

/** Info toasts (`toast`) and the "Keep as is?" question (`learn-banner`). */
export function BannerView() {
  const { t } = useTranslation();
  const [item, setItem] = useState<Item | null>(null);
  const [leaving, setLeaving] = useState(false);
  const current = useRef<Item | null>(null);
  current.current = item;

  useEffect(() => {
    let seq = 0;
    const show = (next: Item) => {
      const prev = current.current;
      if (prev?.kind === "learn") ipc.dismissLearnBanner(prev.word).catch(console.error);
      setLeaving(false);
      setItem(next);
    };
    const subs = [
      ipc.on("toast", (p) => show({ id: ++seq, kind: "toast", key: p.key, params: p.params })),
      ipc.on("learn-banner", (p) => show({ id: ++seq, kind: "learn", word: p.word })),
    ];
    return () => subs.forEach((u) => u.then((f) => f()));
  }, []);

  const leave = () => {
    const id = current.current?.id;
    setLeaving(true);
    setTimeout(() => {
      if (current.current?.id !== id) return; // a newer message took its place
      setItem(null);
      setLeaving(false);
      if (ipc.inTauri) getCurrentWindow().hide().catch(console.error);
    }, 180);
  };

  // Auto-hide. An unanswered question is "abandoned", not declined (as in the original).
  useEffect(() => {
    if (!item) return;
    const timer = setTimeout(() => {
      if (item.kind === "learn") ipc.dismissLearnBanner(item.word).catch(console.error);
      leave();
    }, item.kind === "learn" ? LEARN_MS : TOAST_MS);
    return () => clearTimeout(timer);
  }, [item]);

  const answer = (keep: boolean) => {
    if (item?.kind !== "learn") return;
    ipc.answerLearnBanner(item.word, keep).catch(console.error);
    leave();
  };

  if (!item) return null;
  return (
    <div className="p-3">
      <div key={item.id} role={item.kind === "learn" ? "alertdialog" : "status"} aria-live="polite"
        className={`${leaving ? "kg-out" : "kg-in"} flex gap-3 rounded-[14px] border border-fg/10 bg-raised p-3.5 shadow-[0_10px_30px_rgb(0_0_0/0.25)]`}>
        <Brand size={30} />
        <div className="min-w-0 flex-1">
          {item.kind === "toast" ? (
            <p className="pt-1 text-[13px] leading-snug">{t(item.key, item.params)}</p>
          ) : (
            <>
              <p className="pt-0.5 text-[13.5px] font-semibold">{t("banner.learnTitle", { word: item.word })}</p>
              <p className="mt-0.5 text-[12.5px] text-fg/70">{t("banner.learnBody")}</p>
              <div className="mt-3 flex justify-end gap-2">
                <Button onClick={() => answer(false)}>{t("banner.no")}</Button>
                <Button variant="primary" onClick={() => answer(true)}>{t("banner.add")}</Button>
              </div>
            </>
          )}
        </div>
      </div>
    </div>
  );
}
