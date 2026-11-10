import { useEffect, useRef, useState } from "react";
import { GripVertical, X } from "lucide-react";
import { useTranslation } from "react-i18next";

type Pair = [string, string];
interface RowData { id: number; a: string; b: string }

let nextId = 0;
const blank = (): RowData => ({ id: nextId++, a: "", b: "" });
const toRows = (pairs: Pair[]) => [...pairs.map(([a, b]) => ({ id: nextId++, a, b })), blank()];
const toPairs = (rows: RowData[]): Pair[] => rows.filter((r) => r.a || r.b).map((r) => [r.a, r.b]);

/** Two-column list editor that always keeps one trailing empty row; rows reorder by drag or Alt+↑/↓. */
export function PairEditor({ pairs, onChange, cols, placeholders }: {
  pairs: Pair[]; onChange: (p: Pair[]) => void; cols: [string, string]; placeholders: [string, string];
}) {
  const { t } = useTranslation();
  const [rows, setRows] = useState(() => toRows(pairs));
  const [drag, setDrag] = useState<{ from: number; over: number } | null>(null);
  const saved = useRef(JSON.stringify(pairs));
  const rowsRef = useRef(rows);
  rowsRef.current = rows;

  // Adopt outside changes (another window, the backend), but not our own echo.
  useEffect(() => {
    const json = JSON.stringify(pairs);
    if (json !== saved.current) { saved.current = json; setRows(toRows(pairs)); }
  }, [pairs]);

  const commit = (next: RowData[]) => {
    const p = toPairs(next);
    const json = JSON.stringify(p);
    if (json !== saved.current) { saved.current = json; onChange(p); }
  };
  const withTrailing = (rs: RowData[]) => (rs.length && !rs[rs.length - 1].a && !rs[rs.length - 1].b ? rs : [...rs, blank()]);
  const apply = (next: RowData[]) => { const n = withTrailing(next); setRows(n); commit(n); };

  const edit = (i: number, field: "a" | "b", v: string) =>
    setRows((rs) => withTrailing(rs.map((r, j) => (j === i ? { ...r, [field]: v } : r))));
  const remove = (i: number) => apply(rows.filter((_, j) => j !== i));
  const move = (from: number, to: number) => {
    const last = rows.length - 1; // the trailing empty row stays last
    if (from === to || from >= last || to < 0 || to >= last) return;
    const n = [...rows];
    n.splice(to, 0, ...n.splice(from, 1));
    apply(n);
  };

  const cell = "h-8 min-w-0 rounded-md border border-transparent bg-transparent px-2 text-[12.5px] placeholder:text-fg/45 hover:border-fg/15 focus:border-detail focus:bg-raised";
  return (
    <div className="rounded-xl border border-fg/10 bg-fg/[0.035] p-1.5">
      <div className="grid grid-cols-[22px_minmax(0,2fr)_minmax(0,3fr)_28px] gap-1.5 px-0.5 pt-1 pb-1.5 text-[11px] font-semibold tracking-[0.06em] text-fg/65 uppercase">
        <span />
        <span className="px-2">{cols[0]}</span>
        <span className="px-2">{cols[1]}</span>
      </div>
      {rows.map((r, i) => {
        const isTrailing = i === rows.length - 1;
        return (
          <div key={r.id}
            onDragOver={(e) => { if (drag && !isTrailing) { e.preventDefault(); setDrag({ ...drag, over: i }); } }}
            onDrop={(e) => { e.preventDefault(); if (drag) move(drag.from, drag.over); setDrag(null); }}
            className={`grid grid-cols-[22px_minmax(0,2fr)_minmax(0,3fr)_28px] items-center gap-1.5 rounded-lg px-0.5 transition-colors ${
              drag?.from === i ? "opacity-40" : ""} ${drag && drag.over === i && drag.from !== i ? "bg-detail/10" : ""}`}>
            {isTrailing ? <span /> : (
              <button type="button" draggable aria-label={t("common.moveRow")} title={t("common.moveRow")}
                onDragStart={(e) => {
                  setDrag({ from: i, over: i });
                  e.dataTransfer.effectAllowed = "move";
                  e.dataTransfer.setDragImage(e.currentTarget.parentElement!, 12, 16);
                }}
                onDragEnd={() => setDrag(null)}
                onKeyDown={(e) => {
                  if (!e.altKey || (e.key !== "ArrowUp" && e.key !== "ArrowDown")) return;
                  e.preventDefault();
                  const to = i + (e.key === "ArrowUp" ? -1 : 1);
                  move(i, to);
                  requestAnimationFrame(() =>
                    (document.querySelectorAll<HTMLElement>(`[data-grip="${r.id}"]`)[0])?.focus());
                }}
                data-grip={r.id}
                className="grid h-8 cursor-grab place-items-center rounded text-fg/40 hover:text-fg/75 active:cursor-grabbing">
                <GripVertical size={14} />
              </button>
            )}
            <input className={cell} value={r.a} placeholder={isTrailing ? placeholders[0] : undefined} aria-label={cols[0]}
              onChange={(e) => edit(i, "a", e.target.value)} onBlur={() => commit(rowsRef.current)} />
            <input className={cell} value={r.b} placeholder={isTrailing ? placeholders[1] : undefined} aria-label={cols[1]}
              onChange={(e) => edit(i, "b", e.target.value)} onBlur={() => commit(rowsRef.current)} />
            {isTrailing ? <span /> : (
              <button type="button" aria-label={t("common.deleteRow")} title={t("common.deleteRow")} onClick={() => remove(i)}
                className="grid size-7 cursor-pointer place-items-center rounded-md text-fg/45 hover:bg-fg/[0.08] hover:text-fg">
                <X size={14} />
              </button>
            )}
          </div>
        );
      })}
    </div>
  );
}
