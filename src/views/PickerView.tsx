import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import * as ipc from "../lib/ipc";

/**
 * Snippet picker. The window never takes focus: digits and Esc are handled by the backend's
 * keyboard hook; this view only renders the list and forwards clicks.
 */
export function PickerView() {
  const { t } = useTranslation();
  const [snippets, setSnippets] = useState<[string, string][] | null>(null);

  useEffect(() => {
    const subs = [
      ipc.on("snippet-picker-open", (p) => setSnippets(p.snippets)),
      ipc.on("snippet-picker-close", () => setSnippets(null)),
    ];
    return () => subs.forEach((u) => u.then((f) => f()));
  }, []);

  if (!snippets) return null;
  return (
    <div className="p-2">
      <div className="kg-in flex max-h-[calc(100vh-16px)] flex-col overflow-hidden rounded-xl border border-fg/10 bg-raised shadow-[0_12px_36px_rgb(0_0_0/0.28)]">
        <ul className="overflow-y-auto py-1.5">
          {snippets.map(([name, text], i) => (
            <li key={i}>
              <button type="button" tabIndex={-1} onClick={() => ipc.snippetPickerChoose(i).catch(console.error)}
                className="flex h-7 w-full cursor-pointer items-center gap-3 px-3 text-left hover:bg-fg/[0.07]">
                <span className="w-4 shrink-0 text-center text-[12px] font-semibold text-detail tabular-nums">{i < 9 ? i + 1 : ""}</span>
                <span className="max-w-[40%] shrink-0 truncate font-semibold">{name}</span>
                <span className="min-w-0 flex-1 truncate text-fg/70">{text.replace(/\s*\n\s*/g, " ⏎ ")}</span>
              </button>
            </li>
          ))}
        </ul>
        <p className="border-t border-fg/[0.08] px-3 py-1 text-[11.5px] text-fg/60">{t("snip.pickTip")}</p>
      </div>
    </div>
  );
}
