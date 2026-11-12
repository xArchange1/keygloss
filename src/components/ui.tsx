import { useId, useState, type InputHTMLAttributes, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { Check as CheckIcon, ChevronDown, CircleHelp, X } from "lucide-react";
import { useTranslation } from "react-i18next";

const cx = (...c: (string | false | null | undefined)[]) => c.filter(Boolean).join(" ");

/** `children` followed by a "?"; hovering or focusing either shows `help` in a themed popup. */
export function Help({ children, help }: { children: ReactNode; help?: ReactNode }) {
  const id = useId();
  const [at, setAt] = useState<{ left: number; top?: number; bottom?: number } | null>(null);
  if (!help) return <>{children}</>;
  const show = (e: { currentTarget: HTMLElement }) => {
    const r = e.currentTarget.getBoundingClientRect();
    const left = Math.max(8, Math.min(r.left, window.innerWidth - 288));
    // Below the label, or above it when the bottom of the window is near.
    setAt(r.bottom + 150 < window.innerHeight ? { left, top: r.bottom + 6 } : { left, bottom: window.innerHeight - r.top + 6 });
  };
  return (
    <span tabIndex={0} aria-describedby={at ? id : undefined} onMouseEnter={show} onFocus={show}
      onMouseLeave={() => setAt(null)} onBlur={() => setAt(null)}
      className="group inline-flex cursor-help items-center gap-1 rounded-sm">
      <span>{children}</span>
      <CircleHelp size={13} aria-hidden strokeWidth={2.2}
        className="shrink-0 text-fg/40 transition-colors group-hover:text-detail group-focus-visible:text-detail" />
      {at && createPortal(
        <div id={id} role="tooltip" style={at}
          className="kg-in pointer-events-none fixed z-50 w-max max-w-[280px] rounded-lg border border-fg/12 bg-raised px-3 py-2 text-[12px] leading-snug font-normal tracking-normal text-fg normal-case shadow-[0_8px_24px_rgb(0_0_0/0.22)]">
          {help}
        </div>,
        document.body,
      )}
    </span>
  );
}

export function PageTitle({ title, sub, help }: { title: string; sub?: string; help?: ReactNode }) {
  return (
    <header className="mb-3">
      <h1 className="text-[20px] font-semibold tracking-tight"><Help help={help}>{title}</Help></h1>
      {sub && <p className="mt-0.5 text-fg/70">{sub}</p>}
    </header>
  );
}

/** Small caps heading above a group of cards. */
export function SectionTitle({ children }: { children: ReactNode }) {
  return <h2 className="mt-3.5 mb-1.5 px-1 text-[11px] font-semibold tracking-[0.08em] text-detail uppercase">{children}</h2>;
}

export function BlockTitle({ title, sub, help }: { title: string; sub?: string; help?: ReactNode }) {
  return (
    <div className="mt-4 mb-2">
      <h2 className="text-[14px] font-semibold"><Help help={help}>{title}</Help></h2>
      {sub && <p className="mt-0.5 text-fg/70">{sub}</p>}
    </div>
  );
}

export function Hint({ children, tone }: { children: ReactNode; tone?: "warn" }) {
  return <p className={cx("mt-2 px-1 text-[12px]", tone === "warn" ? "font-medium text-fg" : "text-fg/65")}>{children}</p>;
}

export function Card({ children, className }: { children: ReactNode; className?: string }) {
  return (
    <div className={cx("divide-y divide-fg/[0.07] rounded-xl border border-fg/10 bg-fg/[0.035] shadow-[0_1px_0_rgb(255_255_255/0.04)_inset]", className)}>
      {children}
    </div>
  );
}

/** A settings row: title (with an optional `help` popup) and subtitle on the left, a control on the right. */
export function Row({ title, sub, help, control, disabled, id }: {
  title: ReactNode; sub?: ReactNode; help?: ReactNode; control?: ReactNode; disabled?: boolean; id?: string;
}) {
  return (
    <div className={cx("flex min-h-8 items-center gap-3 px-3 py-1.5", disabled && "opacity-50")}>
      <div className="min-w-0 flex-1">
        <div className="font-medium"><Help help={help}><span id={id}>{title}</span></Help></div>
        {sub && <div className="mt-0.5 text-[12px] text-fg/65">{sub}</div>}
      </div>
      {control && <div className="flex shrink-0 items-center">{control}</div>}
    </div>
  );
}

export function SwitchRow({ title, sub, help, checked, onChange, disabled }: {
  title: string; sub?: ReactNode; help?: ReactNode; checked: boolean; onChange: (v: boolean) => void; disabled?: boolean;
}) {
  const id = useId();
  return (
    <Row id={id} title={title} sub={sub} help={help} disabled={disabled}
      control={<Toggle checked={checked} onChange={onChange} disabled={disabled} labelledBy={id} />} />
  );
}

export function Toggle({ checked, onChange, disabled, labelledBy, label }: {
  checked: boolean; onChange: (v: boolean) => void; disabled?: boolean; labelledBy?: string; label?: string;
}) {
  return (
    <button type="button" role="switch" aria-checked={checked} aria-labelledby={labelledBy} aria-label={label}
      disabled={disabled} onClick={() => onChange(!checked)}
      className={cx(
        "relative h-[22px] w-[38px] shrink-0 rounded-full transition-colors duration-150",
        checked ? "bg-accent" : "bg-fg/20",
        !disabled && "cursor-pointer",
      )}>
      <span className={cx(
        "absolute top-[3px] left-[3px] size-4 rounded-full shadow-sm transition-transform duration-150",
        checked ? "translate-x-4 bg-on-accent" : "bg-raised",
      )} />
    </button>
  );
}

export function Segmented<T extends string>({ value, options, onChange, label, className }: {
  value: T; options: { value: T; label: ReactNode; title?: string }[]; onChange: (v: T) => void;
  label?: string; className?: string;
}) {
  return (
    <div role="radiogroup" aria-label={label}
      className={cx("inline-flex rounded-lg bg-fg/[0.08] p-0.5", className)}>
      {options.map((o) => (
        <button key={o.value} type="button" role="radio" aria-checked={o.value === value} title={o.title}
          onClick={() => onChange(o.value)}
          className={cx(
            "flex-1 cursor-pointer rounded-md px-3 py-[3px] text-[12px] whitespace-nowrap transition-colors",
            o.value === value ? "bg-raised font-semibold shadow-sm" : "text-fg/75 hover:text-fg",
          )}>
          {o.label}
        </button>
      ))}
    </div>
  );
}

export function Select<T extends string>({ value, options, onChange, label, title, className }: {
  value: T; options: { value: T; label: string }[]; onChange: (v: T) => void; label?: string; title?: string;
  className?: string;
}) {
  return (
    <span className={cx("relative inline-flex", className)}>
      <select value={value} aria-label={label} title={title} onChange={(e) => onChange(e.target.value as T)}
        className="h-7 w-full cursor-pointer appearance-none rounded-md border border-fg/15 bg-raised pr-7 pl-2.5 text-[12px] hover:border-fg/30 focus:border-detail">
        {options.map((o) => <option key={o.value} value={o.value}>{o.label}</option>)}
      </select>
      <ChevronDown size={14} className="pointer-events-none absolute top-1/2 right-2 -translate-y-1/2 text-fg/60" />
    </span>
  );
}

export function Button({ children, onClick, variant = "secondary", disabled, className, type = "button", title }: {
  children: ReactNode; onClick?: () => void; variant?: "primary" | "secondary" | "ghost"; disabled?: boolean;
  className?: string; type?: "button" | "submit"; title?: string;
}) {
  return (
    <button type={type} onClick={onClick} disabled={disabled} title={title}
      className={cx(
        "inline-flex h-7 items-center justify-center gap-1.5 rounded-md px-3 text-[12.5px] font-medium whitespace-nowrap transition",
        variant === "primary" && "bg-accent text-on-accent hover:brightness-110",
        variant === "secondary" && "border border-fg/15 bg-raised hover:border-fg/30",
        variant === "ghost" && "text-fg/75 hover:bg-fg/[0.07] hover:text-fg",
        disabled ? "cursor-default opacity-45" : "cursor-pointer",
        className,
      )}>
      {children}
    </button>
  );
}

/** Checkbox styled as a pill (Space / Enter / Tab choices). */
export function CheckPill({ checked, onChange, children }: {
  checked: boolean; onChange: (v: boolean) => void; children: ReactNode;
}) {
  return (
    <button type="button" role="checkbox" aria-checked={checked} onClick={() => onChange(!checked)}
      className={cx(
        "inline-flex h-7 cursor-pointer items-center gap-1.5 rounded-md border px-2.5 text-[12px] transition-colors",
        checked ? "border-accent/60 bg-accent/10 font-medium" : "border-fg/15 text-fg/70 hover:border-fg/30",
      )}>
      <span className={cx("grid size-3.5 place-items-center rounded-[4px]", checked ? "bg-accent text-on-accent" : "border border-fg/30")}>
        {checked && <CheckIcon size={11} strokeWidth={3} />}
      </span>
      {children}
    </button>
  );
}

export function KeyCaps({ parts, muted }: { parts: string[]; muted?: boolean }) {
  return (
    <span className="inline-flex items-center gap-1">
      {parts.map((p, i) => (
        <span key={i} className="inline-flex items-center gap-1">
          {i > 0 && <span className="text-[11px] text-fg/45">+</span>}
          <kbd className={cx(
            "inline-flex h-[22px] min-w-[22px] items-center justify-center rounded-[5px] border border-b-2 border-fg/20 bg-raised px-1.5 font-sans text-[11.5px] font-semibold text-detail",
            muted && "opacity-50",
          )}>{p}</kbd>
        </span>
      ))}
    </span>
  );
}

/** Words as removable chips. */
export function Chips({ words, onRemove, empty }: { words: string[]; onRemove: (w: string) => void; empty: string }) {
  const { t } = useTranslation();
  return (
    <div className="flex min-h-11 flex-wrap content-start gap-1.5 rounded-xl border border-fg/10 bg-fg/[0.035] p-2">
      {words.length === 0 && <span className="px-1.5 py-1 text-[12px] text-fg/60">{empty}</span>}
      {words.map((w) => (
        <span key={w} className="inline-flex h-6 items-center gap-0.5 rounded-full bg-fg/[0.08] pr-0.5 pl-2.5 text-[12.5px]">
          {w}
          <button type="button" aria-label={t("common.remove", { name: w })} onClick={() => onRemove(w)}
            className="grid size-5 cursor-pointer place-items-center rounded-full text-fg/55 hover:bg-fg/10 hover:text-fg">
            <X size={12} strokeWidth={2.5} />
          </button>
        </span>
      ))}
    </div>
  );
}

export function TextInput(props: InputHTMLAttributes<HTMLInputElement>) {
  return (
    <input {...props}
      className={cx("h-7 min-w-0 rounded-md border border-fg/15 bg-raised px-2.5 text-[12.5px] placeholder:text-fg/45 hover:border-fg/30 focus:border-detail", props.className)} />
  );
}
