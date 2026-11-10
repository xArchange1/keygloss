import { useEffect, useRef, useState, type KeyboardEvent } from "react";
import { createPortal } from "react-dom";
import { Check, ChevronDown } from "lucide-react";
import { useTranslation } from "react-i18next";
import type { TFunction } from "i18next";
import * as ipc from "../lib/ipc";
import type { Hotkey, HotkeyCheck, HotkeyTarget } from "../lib/ipc";
import { PRESETS, hotkeyParts, sameHotkey } from "../lib/hotkeys";
import { Button, KeyCaps } from "./ui";

/** Message for a check result, and whether the hotkey may be assigned. */
function verdictText(check: HotkeyCheck, t: TFunction): { text: string | null; ok: boolean } {
  if (check.conflictsWith) return { text: t("hkrec.ours", { what: t(`hkrec.what.${check.conflictsWith}`) }), ok: false };
  if (check.verdict === "blocked") return { text: t(check.reason ?? "hkrec.reason.critical"), ok: false };
  if (check.verdict === "warn") return { text: t(check.reason ?? "hkrec.reason.system"), ok: true };
  return { text: null, ok: true };
}

/** Current hotkey as key caps; a menu offers presets and "Custom…" (the recorder). */
export function HotkeyPicker({ target, value, onChange, label }: {
  target: HotkeyTarget; value: Hotkey; onChange: (h: Hotkey) => void; label: string;
}) {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const [recording, setRecording] = useState(false);
  const [note, setNote] = useState<string | null>(null);
  const wrap = useRef<HTMLDivElement>(null);
  const presets = PRESETS[target];
  const isCustom = !presets.some((p) => sameHotkey(p, value));

  useEffect(() => {
    if (!open) return;
    const close = (e: MouseEvent) => !wrap.current?.contains(e.target as Node) && setOpen(false);
    document.addEventListener("mousedown", close);
    return () => document.removeEventListener("mousedown", close);
  }, [open]);

  const choose = async (h: Hotkey) => {
    setOpen(false);
    if (sameHotkey(h, value)) return;
    const v = verdictText(await ipc.checkHotkey(target, h).catch(() => ({ verdict: "ok", reason: null, conflictsWith: null }) as HotkeyCheck), t);
    setNote(v.text);
    if (v.ok) onChange(h);
  };

  const onMenuKey = (e: KeyboardEvent) => {
    if (e.key === "Escape") { setOpen(false); wrap.current?.querySelector("button")?.focus(); return; }
    if (e.key !== "ArrowDown" && e.key !== "ArrowUp") return;
    e.preventDefault();
    const items = [...(e.currentTarget.querySelectorAll("[role=menuitem]") as NodeListOf<HTMLElement>)];
    const i = items.indexOf(document.activeElement as HTMLElement);
    items[(i + (e.key === "ArrowDown" ? 1 : items.length - 1)) % items.length]?.focus();
  };

  return (
    <div ref={wrap} className="relative flex flex-col items-end">
      <button type="button" aria-haspopup="menu" aria-expanded={open} aria-label={`${label}: ${hotkeyParts(value, t).join(" + ")}. ${t("hkrec.change")}`}
        onClick={() => setOpen(!open)}
        onKeyDown={(e) => e.key === "ArrowDown" && (e.preventDefault(), setOpen(true))}
        className="flex h-8 cursor-pointer items-center gap-2 rounded-lg border border-fg/15 bg-raised py-1 pr-2 pl-1.5 hover:border-fg/30">
        <KeyCaps parts={hotkeyParts(value, t)} />
        <ChevronDown size={14} className="text-fg/60" />
      </button>
      {open && (
        <div role="menu" onKeyDown={onMenuKey}
          ref={(el) => el?.querySelector<HTMLElement>("[role=menuitem]")?.focus()}
          className="kg-in absolute top-9 right-0 z-30 min-w-[220px] rounded-xl border border-fg/12 bg-raised p-1 shadow-[0_12px_32px_rgb(0_0_0/0.22)]">
          {[...presets, ...(isCustom ? [value] : [])].map((p, i) => (
            <button key={i} type="button" role="menuitem" onClick={() => choose(p)}
              className="flex w-full cursor-pointer items-center justify-between gap-4 rounded-lg px-2 py-1.5 hover:bg-fg/[0.07] focus-visible:bg-fg/[0.07]">
              <KeyCaps parts={hotkeyParts(p, t)} />
              {sameHotkey(p, value) && <Check size={14} className="text-accent" strokeWidth={2.5} />}
            </button>
          ))}
          <div className="my-1 h-px bg-fg/10" />
          <button type="button" role="menuitem" onClick={() => { setOpen(false); setRecording(true); }}
            className="w-full cursor-pointer rounded-lg px-2.5 py-1.5 text-left text-[12.5px] hover:bg-fg/[0.07] focus-visible:bg-fg/[0.07]">
            {t("hkrec.custom")}
          </button>
        </div>
      )}
      {note && <p role="status" className="mt-1.5 max-w-[280px] text-right text-[11.5px] text-fg/75">{note}</p>}
      {recording && (
        <Recorder target={target} onClose={() => setRecording(false)}
          onAssign={(h) => { setRecording(false); setNote(null); onChange(h); }} />
      )}
    </div>
  );
}

/** "New shortcut" dialog: the backend captures keys globally and sends `hotkey-captured`. */
function Recorder({ target, onAssign, onClose }: {
  target: HotkeyTarget; onAssign: (h: Hotkey) => void; onClose: () => void;
}) {
  const { t } = useTranslation();
  const [candidate, setCandidate] = useState<Hotkey | null>(null);
  const [check, setCheck] = useState<HotkeyCheck | null>(null);
  const dialog = useRef<HTMLDivElement>(null);
  const closeRef = useRef(onClose);
  closeRef.current = onClose;

  useEffect(() => {
    let seq = 0;
    ipc.startHotkeyRecording().catch(console.error);
    const unlisten = ipc.on("hotkey-captured", (h) => {
      if (h.mode === "key" && h.key === "Escape" && !Object.values(h.mods).some(Boolean)) return closeRef.current();
      const mine = ++seq;
      setCandidate(h);
      setCheck(null);
      ipc.checkHotkey(target, h).then((c) => mine === seq && setCheck(c)).catch(console.error);
    });
    const esc = (e: globalThis.KeyboardEvent) => e.key === "Escape" && closeRef.current();
    window.addEventListener("keydown", esc);
    dialog.current?.focus();
    return () => {
      ipc.stopHotkeyRecording().catch(console.error);
      unlisten.then((u) => u());
      window.removeEventListener("keydown", esc);
    };
  }, [target]);

  const verdict = check && verdictText(check, t);
  return createPortal(
    <div className="fixed inset-0 z-50 grid place-items-center bg-black/30 backdrop-blur-[2px]">
      <div ref={dialog} tabIndex={-1} role="dialog" aria-modal="true" aria-labelledby="kg-rec-title"
        className="kg-in w-[380px] rounded-2xl border border-fg/10 bg-raised p-6 text-fg shadow-[0_24px_60px_rgb(0_0_0/0.35)] outline-none">
        <h2 id="kg-rec-title" className="text-[15px] font-semibold">{t("hkrec.title")}</h2>
        <p className="mt-0.5 text-[12.5px] text-fg/70">{t("hkrec.for", { what: t(`hkrec.what.${target}`) })}</p>
        <div aria-live="polite" className="mt-4 grid h-16 place-items-center rounded-xl border border-dashed border-fg/20 bg-fg/[0.04]">
          {candidate
            ? <KeyCaps parts={hotkeyParts(candidate, t)} muted={verdict ? !verdict.ok : false} />
            : <span className="flex items-center gap-2 text-fg/60">
                <span className="size-2 animate-pulse rounded-full bg-detail" />{t("hkrec.waiting")}
              </span>}
        </div>
        <p className={`mt-3 min-h-[34px] text-[12px] ${verdict?.text ? "font-medium" : "text-fg/65"}`}>
          {verdict?.text ?? t("hkrec.hint")}
        </p>
        <div className="mt-4 flex justify-end gap-2">
          <Button onClick={onClose}>{t("common.cancel")}</Button>
          <Button variant="primary" disabled={!candidate || !verdict?.ok} onClick={() => candidate && onAssign(candidate)}>
            {t("hkrec.assign")}
          </Button>
        </div>
      </div>
    </div>,
    document.body,
  );
}
