import { useEffect, useRef, useState, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { useTranslation } from "react-i18next";
import { CHANGELOG } from "../changelog";
import * as ipc from "../lib/ipc";
import type { PendingUpdate } from "../lib/ipc";
import { notesFor } from "../lib/releaseNotes";
import { Button } from "./ui";

function Notes({ lines }: { lines: string[] }) {
  return (
    <ul className="mt-1.5 list-disc space-y-1 pl-5 text-[12.5px] text-fg/85">
      {lines.map((line) => <li key={line}>{line}</li>)}
    </ul>
  );
}

/** Modal shell: closes on Escape. */
function Dialog({ title, onClose, children, actions }: { title: string; onClose: () => void; children: ReactNode; actions: ReactNode }) {
  const dialog = useRef<HTMLDivElement>(null);
  const close = useRef(onClose);
  close.current = onClose;

  useEffect(() => {
    const esc = (e: KeyboardEvent) => e.key === "Escape" && close.current();
    window.addEventListener("keydown", esc);
    dialog.current?.focus();
    return () => window.removeEventListener("keydown", esc);
  }, []);

  return createPortal(
    <div className="fixed inset-0 z-50 grid place-items-center bg-black/30 backdrop-blur-[2px]">
      <div ref={dialog} tabIndex={-1} role="dialog" aria-modal="true" aria-labelledby="kg-new-title"
        className="kg-in flex max-h-[80vh] w-[440px] flex-col rounded-2xl border border-fg/10 bg-raised p-6 text-fg shadow-[0_24px_60px_rgb(0_0_0/0.35)] outline-none">
        <h2 id="kg-new-title" className="text-[15px] font-semibold">{title}</h2>
        <div className="mt-3 min-h-0 flex-1 space-y-4 overflow-y-auto">{children}</div>
        <div className="mt-5 flex items-center justify-end gap-2">{actions}</div>
      </div>
    </div>,
    document.body,
  );
}

/** Release notes dialog (src/changelog.ts), newest first. */
export function WhatsNew({ onClose }: { onClose: () => void }) {
  const { t, i18n } = useTranslation();
  const lang = i18n.language === "ru" ? "ru" : "en";
  return (
    <Dialog title={t("whatsNew.title")} onClose={onClose}
      actions={<Button variant="primary" onClick={onClose}>{t("whatsNew.close")}</Button>}>
      {CHANGELOG.map((r) => (
        <section key={r.version}>
          <h3 className="text-[12px] font-semibold text-detail tabular-nums">{r.version}</h3>
          <Notes lines={r[lang]} />
        </section>
      ))}
    </Dialog>
  );
}

/** The update found on startup: its release notes and an install button. */
export function UpdateDialog({ update, onClose }: { update: PendingUpdate; onClose: () => void }) {
  const { t, i18n } = useTranslation();
  const lines = notesFor(update.notes ?? "", i18n.language === "ru" ? "ru" : "en");
  const [state, setState] = useState<"idle" | "installing" | "failed">("idle");
  const install = () => {
    setState("installing");
    ipc.installUpdate().catch((e) => {
      console.error(e);
      setState("failed");
    });
  };
  return (
    <Dialog title={t("updDialog.title", { version: update.version })} onClose={onClose} actions={<>
      {state === "failed" && <span className="mr-auto text-[12px] text-fg/70">{t("upd.failed")}</span>}
      <Button variant="ghost" onClick={onClose}>{t("updDialog.later")}</Button>
      <Button variant="primary" disabled={state === "installing"} onClick={install}>
        {state === "installing" ? t("upd.installing", { version: update.version }) : t("upd.install")}
      </Button>
    </>}>
      <section>
        <h3 className="text-[12px] font-semibold text-detail">{t("whatsNew.title")}</h3>
        {lines.length ? <Notes lines={lines} /> : <p className="mt-1.5 text-[12.5px] text-fg/70">{t("updDialog.noNotes")}</p>}
      </section>
    </Dialog>
  );
}
