// A register as a spreadsheet (the Document Register and its transmittal log): type in cells, Tab
// and Enter, drop-down lists you type to search, copy and paste blocks to and from a workbook, fill
// down, clear, select rows and act on them, sort and filter by column. Columns the engine works out
// are grey and cannot be typed over. The grid keeps no data of its own: every change goes back
// through `set` (or `add` for new rows), and the engine's answer redraws it.
import { For, JSX, Show, createEffect, createMemo, createSignal, onCleanup } from "solid-js";
import { Portal } from "solid-js/web";
import { openMenu, Item, toast } from "./store";
import { order } from "./cells";

export type Opt = { value: string; label?: string };
export type Col<R> = {
  key: string; name: string; width: number;
  /** text, number, date (shown 18 Nov 26, typed in most date styles), list (drop-down), check, calc (worked out) */
  kind: "text" | "number" | "date" | "list" | "check" | "calc";
  get: (r: R) => string;
  show?: (r: R) => JSX.Element | string;
  options?: () => Opt[];
  /** "Edit the list…" at the foot of the drop-down. */
  editList?: () => void;
  /** A worked-out (calc) column holding dates (YYYY-MM-DD): shown 18 Nov 26, sorted by date. */
  date?: boolean;
  /** Only the list's values (else a value typed that is not in the list is kept and added to it). */
  strict?: boolean;
  tip?: string;
  cls?: (r: R) => string;
};
export type Edit<R> = { row: R; key: string; value: string };

const H = 24;
export const dt = (v: string | null | undefined) => (v ? new Date(v + "T00:00").toLocaleDateString("en-GB", { day: "2-digit", month: "short", year: "2-digit" }) : "");
const MON = ["jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec"];
/** A typed date in most styles (2026-11-18, 18/11/2026, 18-Nov-26, 18 Nov 2026, a spreadsheet serial) as YYYY-MM-DD; "" stays "". */
export const parseDate = (v: string): string | null => {
  const t = v.trim(); if (!t) return "";
  let m = t.match(/^(\d{4})-(\d{1,2})-(\d{1,2})/); if (m) return iso(+m[1], +m[2], +m[3]);
  // 18/11/2026 or 11/18/2026, as Options › Date format orders day and month.
  m = t.match(/^(\d{1,2})[/.\-](\d{1,2})[/.\-](\d{2,4})$/); if (m) return order.dayFirst ? iso(yr(+m[3]), +m[2], +m[1]) : iso(yr(+m[3]), +m[1], +m[2]);
  m = t.match(/^(\d{1,2})[\s\-/]+([A-Za-z]{3,})[a-z]*[\s\-/,]+(\d{2,4})$/); if (m && MON.indexOf(m[2].slice(0, 3).toLowerCase()) >= 0) return iso(yr(+m[3]), MON.indexOf(m[2].slice(0, 3).toLowerCase()) + 1, +m[1]);
  if (/^\d{5}(\.\d+)?$/.test(t)) { const d = new Date(Date.UTC(1899, 11, 30) + Math.floor(+t) * 864e5); return d.toISOString().slice(0, 10); }
  const n = Date.parse(t); return isNaN(n) ? null : new Date(n - new Date(n).getTimezoneOffset() * 6e4).toISOString().slice(0, 10);
};
const yr = (y: number) => (y < 100 ? 2000 + y : y);
const iso = (y: number, mo: number, d: number) => { const x = new Date(Date.UTC(y, mo - 1, d)); return x.getUTCMonth() === mo - 1 ? x.toISOString().slice(0, 10) : null; };

export const RegGrid = <R,>(p: {
  id: string; rows: R[]; cols: Col<R>[]; rowId: (r: R) => number;
  set: (e: Edit<R>[]) => void;
  /** New rows typed or pasted under the last one (values by column key). */
  add?: (vals: Record<string, string>[]) => void;
  remove?: (rows: R[]) => void;
  open?: (r: R) => void;
  menu?: (rows: R[]) => Item[];
  selected?: (rows: R[]) => void;
  newHint?: string;
  rowCls?: (r: R) => string;
}) => {
  let box!: HTMLDivElement;
  const [cur, setCur] = createSignal<[number, number]>([0, 1]);
  const [anc, setAnc] = createSignal<[number, number]>([0, 1]);
  const [extra, setExtra] = createSignal<Set<number>>(new Set());
  const [edit, setEdit] = createSignal<string | null>(null);
  const [pick, setPick] = createSignal(0);
  const [sort, setSort] = createSignal<[string, number] | null>(null);
  const [filt, setFilt] = createSignal<Record<string, string>>({});
  const [top, setTop] = createSignal(0), [hgt, setHgt] = createSignal(600);
  const widthKey = `rg-w-${p.id}`;
  const [widths, setWidths] = createSignal<Record<string, number>>((() => { try { return JSON.parse(localStorage.getItem(widthKey) ?? "{}"); } catch { return {}; } })());
  const w = (c: Col<R>) => widths()[c.key] ?? c.width;
  const text = (c: Col<R>, r: R) => { const v = c.get(r); return c.kind === "date" || c.date ? dt(v) : c.kind === "check" ? (v ? "Yes" : "") : v; };
  // What the grid shows: filtered, then sorted.
  const view = createMemo(() => {
    let v = p.rows.filter(r => Object.entries(filt()).every(([k, x]) => { const c = p.cols.find(c => c.key === k); return !c || text(c, r) === x; }));
    const s = sort();
    if (s) { const c = p.cols.find(c => c.key === s[0]); if (c) { const key = (r: R) => (c.kind === "date" || c.date ? c.get(r) || "9999" : c.kind === "number" || /^-?[\d.]+\s*%?$/.test(text(c, r)) ? String(1e9 + parseFloat(text(c, r)) || 0).padStart(14, "0") : text(c, r).toLowerCase()); v = [...v].sort((a, b) => key(a).localeCompare(key(b), undefined, { numeric: true }) * s[1]); } }
    return v;
  });
  const n = () => view().length + (p.add ? 1 : 0);
  const C = () => p.cols.length;
  const clampR = (r: number) => Math.max(0, Math.min(n() - 1, r)), clampC = (c: number) => Math.max(1, Math.min(C(), c));
  const rect = () => { const [r1, c1] = cur(), [r2, c2] = anc(); return [Math.min(r1, r2), Math.max(r1, r2), Math.min(c1, c2), Math.max(c1, c2)] as const; };
  const inRect = (r: number, c: number) => { const [a, b, x, y] = rect(); return r >= a && r <= b && c >= x && c <= y; };
  /** The selected rows: those of the range, and those picked with Ctrl. */
  const selRows = createMemo(() => { const [a, b] = rect(), out: R[] = []; view().forEach((r, i) => { if ((i >= a && i <= b) || extra().has(p.rowId(r))) out.push(r); }); return out; });
  createEffect(() => p.selected?.(selRows()));
  const col = (c: number) => p.cols[c - 1];
  const row = (r: number) => view()[r];
  const go = (r: number, c: number, keep = false) => {
    const rr = clampR(r), cc = clampC(c);
    setCur([rr, cc]); if (!keep) { setAnc([rr, cc]); setExtra(new Set<number>()); }
    // Into view.
    const y = rr * H;
    if (y < box.scrollTop) box.scrollTop = y; else if (y + H > box.scrollTop + box.clientHeight - H - 4) box.scrollTop = y - box.clientHeight + 2 * H + 4;
  };
  const focus = () => box?.focus({ preventScroll: true });
  // ---- editing ----
  let input: HTMLInputElement | undefined;
  const begin = (start?: string) => {
    const [r, c] = cur(), cl = col(c); if (!cl || cl.kind === "calc") return;
    if (cl.kind === "check") { const x = row(r); if (x) p.set([{ row: x, key: cl.key, value: cl.get(x) ? "" : "Yes" }]); return; }
    const x = row(r);
    setEdit(start ?? (x ? (cl.kind === "date" ? dt(cl.get(x)) : cl.get(x)) : "")); setPick(0);
    queueMicrotask(() => { input?.focus(); if (start === undefined) input?.select(); else input?.setSelectionRange(input.value.length, input.value.length); });
  };
  const opts = createMemo(() => {
    const cl = col(cur()[1]); if (!cl?.options || edit() === null) return [];
    const q = (edit() ?? "").toLowerCase(), x = row(cur()[0]), was = x ? cl.get(x).toLowerCase() : "";
    const all = cl.options(), t = (edit() ?? "").trim();
    const hit = all.filter(o => !q || q === was || `${o.value} ${o.label ?? ""}`.toLowerCase().includes(q)).slice(0, 200);
    // What is in the list first (Enter picks the first match, as a workbook completes a typed value);
    // a value that is not there yet comes last, to keep it as typed. A strict list offers only its own.
    return !cl.strict && t && !all.some(o => o.value.toLowerCase() === t.toLowerCase()) ? [...hit, { value: t, label: "new: added to the list" }] : hit;
  });
  /** The value to keep for what was typed: a date as YYYY-MM-DD, a list value as listed (or as typed). */
  const value = (cl: Col<R>, t: string): string | null => {
    if (cl.kind === "date") { const d = parseDate(t); if (d === null) { toast(`“${t}” is not a date. Type it as 18/11/2026, 18-Nov-26 or 2026-11-18.`); return null; } return d; }
    if (cl.kind === "number") { const s = t.trim().replace(/[, ]/g, ""); if (s && isNaN(+s)) { toast(`“${t}” is not a number.`); return null; } return s; }
    if (cl.kind === "check") return /^(y|yes|true|x|✓|1)$/i.test(t.trim()) ? "Yes" : "";
    if (cl.kind === "list" && cl.options) { const o = cl.options().find(o => o.value.toLowerCase() === t.trim().toLowerCase()); return o ? o.value : t.trim(); }
    return t;
  };
  const commit = (dr = 1, dc = 0) => {
    const t = edit(); if (t === null) return;
    const [r, c] = cur(), cl = col(c);
    const chosen = cl.options && opts().length && t !== "" && dropAt() ? opts()[Math.min(pick(), opts().length - 1)]?.value ?? t : t;
    if (cl.strict && chosen && !cl.options?.().some(o => o.value === chosen)) { toast(`“${chosen}” is not in the list.`); return; }
    const v = value(cl, chosen);
    if (v === null) return;
    setEdit(null);
    const x = row(r);
    if (x) { if (v !== cl.get(x)) p.set([{ row: x, key: cl.key, value: v }]); }
    else if (v !== "" && p.add) p.add([{ [cl.key]: v }]);
    go(r + dr, c + dc); focus();
  };
  const cancel = () => { setEdit(null); focus(); };
  // ---- the formula bar: the selected cell's whole content, to read or change ----
  const [bar, setBar] = createSignal<string | null>(null);
  const curCol = () => col(cur()[1]), curRow = () => row(cur()[0]);
  /** What the bar shows: the cell as it is typed (a date as 18 Nov 26), or what the app worked out. */
  const raw = () => { const c = curCol(), r = curRow(); if (!c || !r) return ""; return c.kind === "date" || c.date ? dt(c.get(r)) : c.kind === "check" ? (c.get(r) ? "Yes" : "") : c.get(r); };
  const barText = () => (edit() !== null ? edit()! : bar() !== null ? bar()! : raw());
  const barSave = () => {
    const t = bar(); if (t === null) return true;
    const cl = curCol(), x = curRow(); setBar(null);
    if (!cl || cl.kind === "calc" || t === raw()) return true;
    if (cl.strict && t && !cl.options?.().some(o => o.value.toLowerCase() === t.trim().toLowerCase())) { toast(`“${t}” is not in the list.`); return false; }
    const v = value(cl, t); if (v === null) return false;
    if (x) { if (v !== cl.get(x)) p.set([{ row: x, key: cl.key, value: v }]); }
    else if (v !== "" && p.add) p.add([{ [cl.key]: v }]);
    return true;
  };
  const barKey = (e: KeyboardEvent) => {
    if (e.key === "Enter") { e.preventDefault(); if (barSave()) { go(cur()[0] + 1, cur()[1]); focus(); } }
    else if (e.key === "Escape") { e.preventDefault(); setBar(null); focus(); }
    else if (e.key === "Tab") { e.preventDefault(); if (barSave()) { go(cur()[0], cur()[1] + (e.shiftKey ? -1 : 1)); focus(); } }
  };
  /** Where the cursor is: the column and row, or the size of the selection. */
  const where = () => { const [a, b, x, y] = rect(); const c = curCol(); const nr = b - a + 1, nc = y - x + 1;
    if (nr > 1 || nc > 1) return nc === C() ? (nr > 1 ? `${nr} rows` : `Row ${a + 1}`) : `${nr} row${nr > 1 ? "s" : ""} × ${nc} column${nc > 1 ? "s" : ""}`; return c ? `${c.name} · ${cur()[0] < view().length ? `row ${cur()[0] + 1}` : "new row"}` : ""; };
  // ---- keyboard ----
  const key = (e: KeyboardEvent) => {
    if (edit() !== null) return;
    const [r, c] = cur(), ctrl = e.ctrlKey || e.metaKey, k = e.key;
    const page = Math.max(1, Math.floor(box.clientHeight / H) - 2);
    const mv: Record<string, [number, number]> = { ArrowDown: [1, 0], ArrowUp: [-1, 0], ArrowLeft: [0, -1], ArrowRight: [0, 1], PageDown: [page, 0], PageUp: [-page, 0] };
    if (mv[k]) { e.preventDefault(); const [dr, dc] = mv[k]; if (ctrl) go(dr > 0 ? n() - 1 : dr < 0 ? 0 : r, dc > 0 ? C() : dc < 0 ? 1 : c, e.shiftKey); else go(r + dr, c + dc, e.shiftKey); return; }
    if (k === "Tab") { e.preventDefault(); go(r, c + (e.shiftKey ? -1 : 1)); return; }
    if (k === "Home") { e.preventDefault(); go(ctrl ? 0 : r, 1, e.shiftKey); return; }
    if (k === "End") { e.preventDefault(); go(ctrl ? n() - 1 : r, C(), e.shiftKey); return; }
    if (k === "Enter" || k === "F2") { e.preventDefault(); if (k === "Enter" && !e.altKey && col(c)?.kind === "calc") { const x = row(r); if (x) p.open?.(x); return; } begin(); return; }
    if (k === "F4" || (e.altKey && k === "ArrowDown")) { e.preventDefault(); begin(); return; }
    if (k === "Delete" || k === "Backspace") { e.preventDefault(); clear(); return; }
    if (k === " " && col(c)?.kind === "check") { e.preventDefault(); begin(); return; }
    if (ctrl && k.toLowerCase() === "a") { e.preventDefault(); setAnc([0, 1]); setCur([n() - 1, C()]); return; }
    if (ctrl && k.toLowerCase() === "d") { e.preventDefault(); fillDown(); return; }
    if (!ctrl && !e.altKey && k.length === 1) { e.preventDefault(); begin(k); }
  };
  const editable = () => { const [a, b, x, y] = rect(), out: [R, Col<R>][] = []; for (let i = a; i <= b; i++) for (let j = x; j <= y; j++) { const r = row(i), cl = col(j); if (r && cl && cl.kind !== "calc") out.push([r, cl]); } return out; };
  const clear = () => { const e = editable().filter(([r, cl]) => cl.get(r) !== "").map(([row, cl]) => ({ row, key: cl.key, value: "" })); if (e.length) p.set(e); };
  const fillDown = () => {
    const [a, b, x, y] = rect(); if (b <= a) return;
    const e: Edit<R>[] = [];
    for (let j = x; j <= y; j++) { const cl = col(j), src = row(a); if (!cl || cl.kind === "calc" || !src) continue; for (let i = a + 1; i <= b; i++) { const r = row(i); if (r && cl.get(r) !== cl.get(src)) e.push({ row: r, key: cl.key, value: cl.get(src) }); } }
    if (e.length) p.set(e);
  };
  // ---- clipboard: the block as tab-separated text, as a workbook copies it ----
  const copy = (e: ClipboardEvent) => {
    if (edit() !== null) return;
    const [a, b, x, y] = rect(), lines: string[] = [];
    for (let i = a; i <= b; i++) { const r = row(i); if (!r) continue; const cells: string[] = []; for (let j = x; j <= y; j++) cells.push(text(col(j), r).replace(/[\t\n]/g, " ")); lines.push(cells.join("\t")); }
    e.clipboardData?.setData("text/plain", lines.join("\n")); e.preventDefault();
  };
  const paste = (e: ClipboardEvent) => {
    if (edit() !== null) return;
    const t = e.clipboardData?.getData("text/plain") ?? ""; if (!t) return;
    e.preventDefault();
    const lines = t.replace(/\r/g, "").replace(/\n$/, "").split("\n").map(l => l.split("\t"));
    const [a, , x] = rect();
    // One value pasted over a range fills the range.
    const [r1, r2, c1, c2] = rect(), one = lines.length === 1 && lines[0].length === 1;
    const edits: Edit<R>[] = [], adds: Record<string, string>[] = [];
    let bad = 0;
    const put = (i: number, j: number, s: string) => {
      const cl = col(j); if (!cl || cl.kind === "calc") return;
      const v = value(cl, s); if (v === null) { bad++; return; }
      const r = row(i);
      if (r) { if (cl.get(r) !== v) edits.push({ row: r, key: cl.key, value: v }); }
      else if (p.add) { const k = i - view().length; (adds[k] ??= {})[cl.key] = v; }
    };
    if (one) for (let i = r1; i <= r2; i++) for (let j = c1; j <= c2; j++) put(i, j, lines[0][0]);
    else lines.forEach((l, i) => l.forEach((s, j) => put(a + i, x + j, s)));
    if (edits.length) p.set(edits);
    const fresh = adds.filter(v => v && Object.values(v).some(Boolean));
    if (fresh.length) p.add?.(fresh);
    if (!one) { setAnc([a, x]); setCur([clampR(a + lines.length - 1), clampC(x + Math.max(...lines.map(l => l.length)) - 1)]); }
    if (bad) toast(`${bad} pasted values were not dates or numbers where the column needs one; they were left out.`);
  };
  // Only while the grid has the keyboard (a text box elsewhere copies and pastes as usual).
  const onCopy = (e: ClipboardEvent) => document.activeElement === box && copy(e);
  const onPaste = (e: ClipboardEvent) => document.activeElement === box && paste(e);
  document.addEventListener("copy", onCopy); document.addEventListener("paste", onPaste);
  onCleanup(() => { document.removeEventListener("copy", onCopy); document.removeEventListener("paste", onPaste); });
  // ---- mouse ----
  let drag = false;
  const down = (e: MouseEvent, r: number, c: number) => {
    if (e.button === 2 && inRect(r, c)) return;
    if (edit() !== null) commit(0, 0);
    if (c === 0) {
      // The row number: whole rows; Shift for a run of rows, Ctrl for one more row.
      if (e.ctrlKey || e.metaKey) { const x = row(r); if (x) { const s = new Set(extra()); const id = p.rowId(x); s.has(id) ? s.delete(id) : s.add(id); setExtra(s); } return; }
      if (e.shiftKey) { setCur([r, C()]); setAnc([anc()[0], 1]); } else { setAnc([r, 1]); setCur([r, C()]); setExtra(new Set<number>()); }
      drag = true; focus(); e.preventDefault(); return;
    }
    if (e.shiftKey) setCur([r, c]); else go(r, c);
    drag = true; focus(); e.preventDefault();
  };
  const over = (r: number, c: number) => { if (drag) setCur([r, c === 0 ? C() : c]); };
  const up = () => { drag = false; };
  document.addEventListener("mouseup", up); onCleanup(() => document.removeEventListener("mouseup", up));
  const dbl = (r: number, c: number) => { const x = row(r); if (c === 0 || col(c)?.kind === "calc") { if (x) p.open?.(x); return; } begin(); };
  const ctx = (e: MouseEvent, r: number, c: number) => {
    e.preventDefault();
    if (!inRect(r, c) && !(c === 0 && row(r) && extra().has(p.rowId(row(r)!)))) go(r, c === 0 ? 1 : c);
    const rows = selRows(), nr = rows.length;
    const items: Item[] = [
      ...(p.menu?.(rows) ?? []), ...(p.menu ? ["-" as const] : []),
      { label: "Copy", keys: "Ctrl+C", icon: "copy", run: () => document.execCommand("copy") },
      { label: "Paste: Ctrl+V", off: true, icon: "paste" },
      { label: "Fill down", keys: "Ctrl+D", icon: "filldown", run: fillDown, off: rect()[1] <= rect()[0] },
      { label: "Clear contents", keys: "Del", icon: "clear", run: clear },
      ...(p.remove ? ["-" as const, { label: nr > 1 ? `Delete ${nr} rows` : "Delete row", icon: "delete", run: () => p.remove!(rows), off: !nr }] : []),
    ];
    openMenu(e, items);
  };
  // ---- column heads: sort, filter by a value, width ----
  const head = (e: MouseEvent, cl: Col<R>) => {
    const vals = [...new Set(p.rows.map(r => text(cl, r)))].sort((a, b) => a.localeCompare(b, undefined, { numeric: true })).slice(0, 40);
    const f = filt()[cl.key];
    openMenu(e, [
      { label: "Sort A → Z", icon: "sortaz", run: () => setSort([cl.key, 1]) }, { label: "Sort Z → A", icon: "sortza", run: () => setSort([cl.key, -1]) },
      ...(sort() ? [{ label: "Back to the register's order", run: () => setSort(null) }] : []), "-",
      ...(f !== undefined ? [{ label: `Clear the filter (${f || "empty"})`, icon: "filter", run: () => { const x = { ...filt() }; delete x[cl.key]; setFilt(x); } }] : []),
      { label: "Show only…", sub: vals.map(v => ({ label: v || "(empty)", on: f === v, run: () => setFilt({ ...filt(), [cl.key]: v }) })) },
    ]);
  };
  const resize = (e: MouseEvent, cl: Col<R>) => {
    e.preventDefault(); e.stopPropagation();
    const x0 = e.clientX, w0 = w(cl);
    const mv = (m: MouseEvent) => setWidths({ ...widths(), [cl.key]: Math.max(36, Math.round(w0 + m.clientX - x0)) });
    const end = () => { document.removeEventListener("mousemove", mv); document.removeEventListener("mouseup", end); try { localStorage.setItem(widthKey, JSON.stringify(widths())); } catch { /* no storage */ } };
    document.addEventListener("mousemove", mv); document.addEventListener("mouseup", end);
  };
  // ---- the rows on screen ----
  const first = () => Math.max(0, Math.floor(top() / H) - 10);
  const last = () => Math.min(n(), Math.ceil((top() + hgt()) / H) + 10);
  const idx = () => Array.from({ length: Math.max(0, last() - first()) }, (_, k) => first() + k);
  const measure = () => { if (box) { setTop(box.scrollTop); setHgt(box.clientHeight); } };
  const ro = new ResizeObserver(measure); onCleanup(() => ro.disconnect());
  // The drop-down under the cell being typed in.
  const [dropAt, setDropAt] = createSignal<{ x: number; y: number; w: number } | null>(null);
  createEffect(() => {
    if (edit() === null || !col(cur()[1])?.options) { setDropAt(null); return; }
    const [r, c] = cur(); queueMicrotask(() => { const el = box?.querySelector(`[data-rc="${r}:${c}"]`)?.getBoundingClientRect(); if (el) setDropAt({ x: Math.min(el.left, innerWidth - 480), y: el.bottom, w: Math.max(col(c)?.strict ? 460 : 260, el.width) }); });
  });
  const ekey = (e: KeyboardEvent) => {
    const cl = col(cur()[1]), list = !!cl.options && !!dropAt();
    // The grid itself must not see these keys too (it would move again, or start editing again).
    if (["Enter", "Tab", "Escape"].includes(e.key)) e.stopPropagation();
    if (e.key === "Enter") { e.preventDefault(); commit(e.shiftKey ? -1 : 1, 0); }
    else if (e.key === "Tab") { e.preventDefault(); commit(0, e.shiftKey ? -1 : 1); }
    else if (e.key === "Escape") { e.preventDefault(); cancel(); }
    else if (list && e.key === "ArrowDown") { e.preventDefault(); setPick(Math.min(opts().length - 1, pick() + 1)); }
    else if (list && e.key === "ArrowUp") { e.preventDefault(); setPick(Math.max(0, pick() - 1)); }
  };
  const filters = () => Object.entries(filt());
  return (
    <div class="rgwrap">
      <div class="rgfx">
        <span class="rgwhere" title="The selected cell">{where()}</span><i>fx</i>
        <input value={barText()} readOnly={!curCol() || curCol()!.kind === "calc" || (!curRow() && !p.add)} classList={{ calc: curCol()?.kind === "calc" }}
          title={curCol()?.kind === "calc" ? `Worked out by the app${curCol()?.tip ? `: ${curCol()!.tip}` : ""}` : "The cell's content: type here to change it, Enter to keep it, Esc to leave it"}
          onFocus={() => { if (edit() !== null) commit(0, 0); if (curCol()?.kind !== "calc") setBar(raw()); }}
          onInput={e => setBar(e.currentTarget.value)} onKeyDown={barKey} onBlur={() => { if (bar() !== null) barSave(); }} />
      </div>
      <Show when={filters().length || sort()}>
        <div class="rgchips">
          <For each={filters()}>{([k, v]) => <button class="chip on" onClick={() => { const x = { ...filt() }; delete x[k]; setFilt(x); }}>{p.cols.find(c => c.key === k)?.name}: {v || "(empty)"} ×</button>}</For>
          <Show when={sort()}><button class="chip on" onClick={() => setSort(null)}>Sorted by {p.cols.find(c => c.key === sort()![0])?.name} {sort()![1] > 0 ? "↑" : "↓"} ×</button></Show>
          <span class="muted">{view().length} of {p.rows.length} shown</span>
        </div>
      </Show>
      <div class="rg" ref={el => { box = el; ro.observe(el); queueMicrotask(measure); }} tabindex="0" onKeyDown={key} onScroll={measure}>
        <table style={{ width: `${40 + p.cols.reduce((a, c) => a + w(c), 0)}px` }}>
          <colgroup><col style={{ width: "40px" }} /><For each={p.cols}>{c => <col style={{ width: `${w(c)}px` }} />}</For></colgroup>
          <thead><tr><th class="rh" onClick={() => { setAnc([0, 1]); setCur([n() - 1, C()]); focus(); }} title="Select every row" />
            <For each={p.cols}>{c => <th classList={{ calc: c.kind === "calc", on: !!filt()[c.key] || sort()?.[0] === c.key }} title={c.tip ?? (c.kind === "calc" ? "Worked out by the app" : "")}>
              <span>{c.name}</span><button class="rgf" onClick={e => head(e, c)} title="Sort or filter">▾</button><i class="rgw" onMouseDown={e => resize(e, c)} /></th>}</For></tr></thead>
          <tbody>
            <tr style={{ height: `${first() * H}px` }} />
            <For each={idx()}>{i => { const r = () => row(i); return (
              <tr classList={{ newrow: !r(), sel: !!r() && extra().has(p.rowId(r()!)), [p.rowCls && r() ? p.rowCls(r()!) : ""]: !!(p.rowCls && r() && p.rowCls(r()!)) }}>
                <td class="rh" classList={{ on: inRect(i, 1) && inRect(i, C()) || (!!r() && extra().has(p.rowId(r()!))) }} onMouseDown={e => down(e, i, 0)} onMouseEnter={() => over(i, 0)} onDblClick={() => dbl(i, 0)} onContextMenu={e => ctx(e, i, 0)} title={r() ? "Click to select the row, Shift or Ctrl for more · double-click to open" : p.newHint}>{r() ? i + 1 : "+"}</td>
                <For each={p.cols}>{(c, j) => { const cj = () => j() + 1; return (
                  <td data-rc={`${i}:${cj()}`} classList={{ calc: c.kind === "calc", in: inRect(i, cj()), cur: cur()[0] === i && cur()[1] === cj(), dd: c.kind === "list" && !!r(), ck: c.kind === "check", num: c.kind === "number", [r() && c.cls ? c.cls(r()!) : ""]: !!(r() && c.cls && c.cls(r()!)) }}
                    onMouseDown={e => down(e, i, cj())} onMouseEnter={() => over(i, cj())} onDblClick={() => dbl(i, cj())} onContextMenu={e => ctx(e, i, cj())}>
                    <Show when={edit() !== null && cur()[0] === i && cur()[1] === cj()} fallback={r() ? (c.show ? c.show(r()!) : text(c, r()!)) : (i === view().length && cj() === 1 ? <span class="muted">{p.newHint}</span> : "")}>
                      <input class="rgin" ref={input} value={edit() ?? ""} onInput={e => { setEdit(e.currentTarget.value); setPick(0); }} onKeyDown={ekey} onBlur={() => queueMicrotask(() => { if (edit() !== null && document.activeElement !== input && !document.querySelector(".rgdrop:hover")) commit(0, 0); })} />
                    </Show>
                    <Show when={c.kind === "list" && !!r() && cur()[0] === i && cur()[1] === cj() && edit() === null}><button class="rgopen" onMouseDown={e => { e.stopPropagation(); e.preventDefault(); go(i, cj()); begin(""); }} title="Choose from the list (Alt+Down)">▾</button></Show>
                  </td>); }}</For>
              </tr>); }}</For>
            <tr style={{ height: `${Math.max(0, n() - last()) * H}px` }} />
          </tbody>
        </table>
      </div>
      <Show when={dropAt()}>{a => <Portal><div class="rgdrop" style={{ left: `${a().x}px`, top: `${Math.min(a().y, innerHeight - 250)}px`, width: `${a().w}px` }}>
        <For each={opts()}>{(o, k) => <div class="rgo" classList={{ on: k() === pick() }} onMouseDown={e => { e.preventDefault(); setEdit(o.value); setPick(k()); commit(1, 0); }}><b>{o.value}</b>{o.label && <small>{o.label}</small>}</div>}</For>
        <Show when={!opts().length}><div class="rgo muted">Nothing in the list matches “{edit()}”.</div></Show>
        <Show when={col(cur()[1])?.editList}><div class="rgfoot" onMouseDown={e => { e.preventDefault(); cancel(); col(cur()[1])?.editList?.(); }}>Edit the list…</div></Show>
      </div></Portal>}</Show>
    </div>
  );
};
