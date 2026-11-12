import { useEffect, useRef } from "react";
import { createPortal } from "react-dom";
import { useTranslation } from "react-i18next";
import { CHANGELOG } from "../changelog";
import { Button } from "./ui";

/** Release notes dialog (src/changelog.ts), newest first. */
export function WhatsNew({ onClose }: { onClose: () => void }) {
  const { t, i18n } = useTranslation();
  const dialog = useRef<HTMLDivElement>(null);
  const lang = i18n.language === "ru" ? "ru" : "en";
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
        <h2 id="kg-new-title" className="text-[15px] font-semibold">{t("whatsNew.title")}</h2>
        <div className="mt-3 min-h-0 flex-1 space-y-4 overflow-y-auto">
          {CHANGELOG.map((r) => (
            <section key={r.version}>
              <h3 className="text-[12px] font-semibold text-detail tabular-nums">{r.version}</h3>
              <ul className="mt-1.5 list-disc space-y-1 pl-5 text-[12.5px] text-fg/85">
                {r[lang].map((line) => <li key={line}>{line}</li>)}
              </ul>
            </section>
          ))}
        </div>
        <div className="mt-5 flex justify-end">
          <Button variant="primary" onClick={onClose}>{t("whatsNew.close")}</Button>
        </div>
      </div>
    </div>,
    document.body,
  );
}
