import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { getCurrentWindow } from "@tauri-apps/api/window";
import * as ipc from "../lib/ipc";
import { Brand } from "../components/Brand";
import { Button } from "../components/ui";

type Item =
  | { kind: "toast"; key: string; params: Record<string, string>; ms: number }
  | { kind: "learn"; word: string }
  | { kind: "switch"; from: string; to: string }
  | { kind: "update"; version: string };
/** `id` tells a message from the one that replaced it. */
type Shown = Item & { id: number };

/** An unanswered question is "abandoned", not declined (as in the original). */
const dismiss = (item: Item) => {
  if (item.kind === "learn") ipc.dismissLearnBanner(item.word).catch(console.error);
  if (item.kind === "switch") ipc.dismissSwitchBanner(item.to).catch(console.error);
};

const TOAST_MS = 4000;
/** The "Got it" toast that follows an answered question. */
const ANSWERED_MS = 3000;
const LEARN_MS = 20000;
const UPDATE_MS = 10000;
/** "Installing…" stays up until the installer closes the app. */
const INSTALLING_MS = 120000;

const lifetime = (item: Item) => item.kind === "toast" ? item.ms : item.kind === "update" ? UPDATE_MS : LEARN_MS;

/** Info toasts (`toast`), the "Keep as is?" / "Always switch?" questions (`learn-banner`, `switch-banner`)
 * and the update offer (`update-banner`). */
export function BannerView() {
  const { t } = useTranslation();
  const [item, setItem] = useState<Shown | null>(null);
  const [leaving, setLeaving] = useState(false);
  const current = useRef<Shown | null>(null);
  current.current = item;
  const seq = useRef(0);

  const show = (next: Item) => {
    if (current.current) dismiss(current.current);
    setLeaving(false);
    setItem({ ...next, id: ++seq.current });
  };

  useEffect(() => {
    const subs = [
      ipc.on("toast", (p) => show({
        kind: "toast", key: p.key, params: p.params,
        ms: current.current && current.current.kind !== "toast" ? ANSWERED_MS : TOAST_MS,
      })),
      ipc.on("learn-banner", (p) => show({ kind: "learn", word: p.word })),
      ipc.on("switch-banner", (p) => show({ kind: "switch", from: p.from, to: p.to })),
      ipc.on("update-banner", (p) => show({ kind: "update", version: p.version })),
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

  // Auto-hide.
  useEffect(() => {
    if (!item) return;
    const timer = setTimeout(() => {
      dismiss(item);
      leave();
    }, lifetime(item));
    return () => clearTimeout(timer);
  }, [item]);

  const answer = (yes: boolean) => {
    if (item?.kind === "learn") ipc.answerLearnBanner(item.word, yes).catch(console.error);
    else if (item?.kind === "switch") ipc.answerSwitchBanner(item.to, yes).catch(console.error);
    else return;
    leave();
  };

  const install = (version: string) => {
    show({ kind: "toast", key: "upd.installing", params: { version }, ms: INSTALLING_MS });
    ipc.installUpdate().catch((e) => {
      console.error(e);
      show({ kind: "toast", key: "upd.failed", params: {}, ms: TOAST_MS });
    });
  };

  const notes = () => {
    ipc.openUpdate().catch(console.error);
    leave();
  };

  if (!item) return null;
  return (
    <div className="p-3">
      <div key={item.id} role={item.kind === "toast" ? "status" : "alertdialog"} aria-live="polite"
        className={`${leaving ? "kg-out" : "kg-in"} flex gap-3 rounded-[14px] border border-fg/10 bg-raised p-3.5 shadow-[0_10px_30px_rgb(0_0_0/0.25)]`}>
        <Brand size={30} />
        <div className="min-w-0 flex-1">
          {item.kind === "toast" ? (
            <p className="pt-1 text-[13px] leading-snug">{t(item.key, item.params)}</p>
          ) : item.kind === "update" ? (
            <>
              <p className="pt-0.5 text-[13.5px] font-semibold">{t("banner.updateTitle", { version: item.version })}</p>
              <p className="mt-0.5 text-[12.5px] text-fg/70">{t("banner.updateBody")}</p>
              <div className="mt-3 flex justify-end gap-2">
                <Button onClick={notes}>{t("banner.updateNotes")}</Button>
                <Button variant="primary" onClick={() => install(item.version)}>{t("banner.updateInstall")}</Button>
              </div>
            </>
          ) : (
            <>
              <p className="pt-0.5 text-[13.5px] font-semibold">
                {item.kind === "learn" ? t("banner.learnTitle", { word: item.word }) : t("banner.switchTitle", { from: item.from, to: item.to })}
              </p>
              <p className="mt-0.5 text-[12.5px] text-fg/70">{t(item.kind === "learn" ? "banner.learnBody" : "banner.switchBody")}</p>
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
