// Charts drawn as vectors (SVG), with the controls of a spreadsheet's charts: kinds and stacking,
// combined series on a secondary axis, titles, axis bounds, steps and number formats, gridlines,
// legend, data labels, colours, lines, markers, smooth lines and linear trend lines. Sharp on
// screen, in print and in PDFs. The data comes from the engine (`sheet::chart_data`).
import { JSX, Show, createSignal } from "solid-js";
import { render } from "solid-js/web";
import { P } from "./store";
import { money, fmtDay } from "./api";

export type Axis = { title?: string; min?: number | null; max?: number | null; major?: number | null; fmt?: string; grid?: boolean; hide?: boolean };
export type Series = { name: string; vals: string; kind?: string; y2?: boolean; color?: string; width?: number; dash?: string; marker?: string; smooth?: boolean; labels?: boolean; trend?: string; hide?: boolean;
  /** In the window only: a soft fill under a line, and a pie's own slice colours. */
  fill?: boolean; colors?: string[] };
export type ChartDef = { id: number; cell: string; w: number; h: number; kind: string; stack?: string; cats: string; series: Series[]; title?: string; legend?: string; labels?: boolean; font?: number; x?: Axis; y?: Axis; y2?: Axis; bg?: string; plot?: string; gap?: number; skip?: number[];
  /** In the window's views only: a subtitle, the status date's line, the last value at each line's end,
   *  rounded bars, a time axis (x by date, ticks named), and no "no data" text. */
  subtitle?: string; marker?: { at: number; label: string } | null; ends?: boolean; radius?: number; quiet?: boolean;
  time?: { min: number; max: number; ticksAt: number[]; tickLabel: (i: number) => string; rotate?: boolean } };
export type ChartData = { cats: string[]; xs: (number | null)[]; cat_fmt: string; names: string[]; vals: (number | null)[][]; fmts: string[];
  /** On a time axis: each series' own x (days), and the text of each point's tooltip. */
  sx?: (number | null)[][]; tips?: (string | undefined)[][] };

/** Calm, print-safe colours in the family's tones; series take them in turn. */
export const PALETTE = ["#1F5FAD", "#E07B24", "#8C96A3", "#2E9D57", "#C0392B", "#6D3FC8", "#1E9BB8", "#B8860B", "#5B7FA6", "#D4A017"];
export const KINDS: [string, string][] = [["column", "Column"], ["bar", "Bar"], ["line", "Line"], ["area", "Area"], ["scatter", "Scatter (X Y)"], ["pie", "Pie"], ["doughnut", "Doughnut"]];
const FONT = "'IBM Plex Sans', 'Segoe UI', sans-serif";

let mctx: CanvasRenderingContext2D | null | undefined;
const tw = (t: string, px: number, bold = false) => { if (mctx === undefined) mctx = document.createElement("canvas").getContext("2d"); if (!mctx) return t.length * px * 0.55; mctx.font = `${bold ? 600 : 400} ${px}px ${FONT}`; return mctx.measureText(t).width; };

// ---------- number formats (the common spreadsheet codes) ----------
const MON = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
const MONTH = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];
/** A spreadsheet date serial (days since 30-Dec-1899) as a date. */
const serialDate = (v: number) => new Date(Date.UTC(1899, 11, 30) + Math.round(v) * 864e5);
export const isDateFmt = (f: string) => /(^|[^"\\])[dmy]/i.test(f.replace(/"[^"]*"/g, "")) && !/[0#]/.test(f.replace(/"[^"]*"/g, ""));
/** A number written with a spreadsheet format code: 0, 0.0, #,##0, 0%, 0.0%, $#,##0, dd-mmm-yy, mmm-yy… ("" = a tidy general format). */
export const fmtNum = (v: number, code = ""): string => {
  if (!isFinite(v)) return "";
  // The window's own report formats: a % already in percent, money, hours, a day since 1970, a plain number.
  if (code.startsWith("@")) return code === "@pct" ? `${v.toFixed(1)}%` : code === "@pct0" ? `${Math.round(v)}%` : code === "@money" ? money(v, { ...P().currency, decimals: 0 }) : code === "@hours" ? `${Math.round(v).toLocaleString()} h` : code === "@day" ? fmtDay(v) : (Math.round(v * 100) / 100).toLocaleString();
  const f = code.split(";")[0].trim();
  if (!f || f.toLowerCase() === "general") { const a = Math.abs(v); const d = a >= 1000 ? 0 : a >= 10 ? 1 : a >= 1 ? 2 : 3; return (+v.toFixed(d)).toLocaleString("en-GB", { maximumFractionDigits: d }); }
  if (isDateFmt(f)) {
    const t = serialDate(v), y = t.getUTCFullYear(), m = t.getUTCMonth(), d = t.getUTCDate();
    return f.replace(/"([^"]*)"|\\(.)|yyyy|yy|mmmm|mmm|mm|m|dd|d/gi, (x, q, e) => q ?? e ?? ({ yyyy: String(y), yy: String(y).slice(2), mmmm: MONTH[m], mmm: MON[m], mm: String(m + 1).padStart(2, "0"), m: String(m + 1), dd: String(d).padStart(2, "0"), d: String(d) } as Record<string, string>)[x.toLowerCase()] ?? x);
  }
  const lit = (s: string) => s.replace(/"([^"]*)"/g, "$1").replace(/\\(.)/g, "$1").replace(/[_*]./g, "");
  const i0 = f.search(/[0#?]/), i1 = Math.max(f.lastIndexOf("0"), f.lastIndexOf("#"), f.lastIndexOf("?"));
  if (i0 < 0) return lit(f);
  const body = f.slice(i0, i1 + 1), pre = lit(f.slice(0, i0)), post = f.slice(i1 + 1), pct = post.replace(/"[^"]*"/g, "").includes("%") ? 100 : 1;
  const dec = body.includes(".") ? body.split(".")[1].replace(/[^0#?]/g, "").length : 0;
  const x = Math.abs(v * pct), s = body.includes(",") ? x.toLocaleString("en-GB", { minimumFractionDigits: dec, maximumFractionDigits: dec }) : x.toFixed(dec);
  return `${v < 0 ? "-" : ""}${pre}${s}${lit(post)}`;
};

// ---------- scales ----------
const niceStep = (span: number, n: number) => { const raw = span / Math.max(1, n), mag = 10 ** Math.floor(Math.log10(raw || 1)), f = raw / mag; return (f <= 1 ? 1 : f <= 2 ? 2 : f <= 2.5 ? 2.5 : f <= 5 ? 5 : 10) * mag; };
/** Bounds and step of a value axis: the user's where given, otherwise as a spreadsheet picks them
 *  (zero included unless all values sit high above it). */
export const scale = (lo: number, hi: number, a: Axis | undefined, room: number, zero: boolean) => {
  if (!isFinite(lo) || !isFinite(hi)) { lo = 0; hi = 1; }
  if (zero || lo > 0 && (hi - lo) / hi > 1 / 6) lo = Math.min(lo, 0);
  if (zero && hi < 0) hi = 0;
  const min0 = a?.min ?? null, max0 = a?.max ?? null;
  let mn = min0 ?? lo, mx = max0 ?? hi;
  if (mx <= mn) mx = mn + (mn === 0 ? 1 : Math.abs(mn) * 0.1);
  const step = a?.major && a.major > 0 ? a.major : niceStep(mx - mn, Math.max(2, Math.min(10, Math.floor(room / 42))));
  if (min0 === null) mn = Math.floor(mn / step + 1e-9) * step;
  if (max0 === null) mx = Math.ceil(mx / step - 1e-9) * step;
  if (mx <= mn) mx = mn + step;
  const ticks: number[] = []; for (let t = mn, k = 0; t <= mx + step * 1e-6 && k < 200; t = mn + step * ++k) ticks.push(+t.toFixed(12));
  return { mn, mx, ticks };
};

const dashOf = (d?: string, w = 2) => (d === "dash" ? `${w * 3.5} ${w * 2}` : d === "dot" ? `${w} ${w * 1.6}` : undefined);
/** A smooth path through points (Catmull-Rom as cubic Béziers); breaks at gaps. */
const pathOf = (pts: ([number, number] | null)[], smooth: boolean) => {
  let d = "", run: [number, number][] = [];
  const flush = () => {
    if (!run.length) return;
    d += `M${run[0][0]} ${run[0][1]}`;
    for (let i = 1; i < run.length; i++) {
      if (!smooth || run.length < 3) { d += `L${run[i][0]} ${run[i][1]}`; continue; }
      const p0 = run[i - 2] ?? run[i - 1], p1 = run[i - 1], p2 = run[i], p3 = run[i + 1] ?? p2, t = 6;
      d += `C${p1[0] + (p2[0] - p0[0]) / t} ${p1[1] + (p2[1] - p0[1]) / t} ${p2[0] - (p3[0] - p1[0]) / t} ${p2[1] - (p3[1] - p1[1]) / t} ${p2[0]} ${p2[1]}`;
    }
    run = [];
  };
  for (const p of pts) { if (p) run.push(p); else flush(); }
  flush();
  return d;
};
const Marker = (p: { x: number; y: number; kind: string; c: string; r: number }) => {
  const { x, y, r, c } = p;
  if (p.kind === "square") return <rect x={x - r} y={y - r} width={r * 2} height={r * 2} fill={c} stroke="#fff" stroke-width=".8" />;
  if (p.kind === "diamond") return <path d={`M${x} ${y - r * 1.25}L${x + r * 1.25} ${y}L${x} ${y + r * 1.25}L${x - r * 1.25} ${y}Z`} fill={c} stroke="#fff" stroke-width=".8" />;
  if (p.kind === "triangle") return <path d={`M${x} ${y - r * 1.2}L${x + r * 1.15} ${y + r * 0.9}L${x - r * 1.15} ${y + r * 0.9}Z`} fill={c} stroke="#fff" stroke-width=".8" />;
  return <circle cx={x} cy={y} r={r} fill={c} stroke="#fff" stroke-width=".8" />;
};

/** The colour of series `i` (its own, or the palette's). */
export const colorOf = (c: ChartDef, i: number) => c.series[i]?.color || PALETTE[i % PALETTE.length];
/** The kind a series is drawn as: its own in a column, line or area chart (a combined chart), else the chart's. */
export const kindOf = (c: ChartDef, i: number) => (["bar", "scatter", "pie", "doughnut"].includes(c.kind) ? c.kind : c.series[i]?.kind || c.kind);

// Colours that follow the theme inside the window (CSS variables) and stay light in pictures and print.
const INK = "var(--ch-ink, #3A4553)", TITLE = "var(--ch-title, #1B2733)", GRID = "var(--ch-grid, #E3E7EC)", AXIS = "var(--ch-axis, #8D99A8)", BG = "var(--ch-bg, #FFFFFF)", SUB = "var(--ch-sub, #5B6573)";
let gid = 0;

/** The parts of a chart a click selects (`data-part`): its area, plot, title, legend, axes and series. */
export const partName = (c: ChartDef, part: string, names?: string[]) => part === "title" ? "Chart Title" : part === "legend" ? "Legend" : part === "plot" ? "Plot Area" : part === "x" ? (c.kind === "bar" ? "Vertical (Category) Axis" : "Horizontal (Category) Axis")
  : part === "y" ? (c.kind === "bar" ? "Horizontal (Value) Axis" : "Vertical (Value) Axis") : part === "y2" ? "Secondary Vertical (Value) Axis" : part.startsWith("s") ? `Series “${names?.[+part.slice(1)] || c.series[+part.slice(1)]?.name || `Series ${+part.slice(1) + 1}`}”` : "Chart Area";
type Tip = { head: string; rows: { col: string; name: string; val: string }[]; guide?: number; band?: [number, number, number, number]; dots?: [number, number, string][] };

/** A chart as SVG at `w` × `h` px. In the window (`hover`, the default) moving over it shows the
 *  values at that place, and `sel` outlines the selected part. */
export const XChart = (p: { c: ChartDef; d?: ChartData; w: number; h: number; hover?: boolean; sel?: string }): JSX.Element => {
  // The chart's filter: hidden series and skipped categories are left out (colours stay with their series).
  const d0 = p.d ?? { cats: [], xs: [], cat_fmt: "", names: [], vals: [], fmts: [] }, keep = p.c.series.map((_, i) => i).filter(i => !p.c.series[i].hide);
  const skip = new Set(p.c.skip ?? []), cut = <T,>(a: T[] | undefined) => (skip.size && a ? a.filter((_, j) => !skip.has(j)) : a);
  const c: ChartDef = { ...p.c, series: keep.map(i => ({ ...p.c.series[i], color: colorOf(p.c, i) })) };
  const d: ChartData = { ...d0, cats: cut(d0.cats)!, xs: cut(d0.xs)!, names: keep.map(i => d0.names[i]), fmts: keep.map(i => d0.fmts[i]), vals: keep.map(i => cut(d0.vals[i] ?? [])!),
    sx: d0.sx && keep.map(i => cut(d0.sx![i] ?? [])!), tips: d0.tips && keep.map(i => cut(d0.tips![i] ?? [])!) };
  const sp = (i: number) => `s${keep[i]}`;
  const fs = c.font || 11, W = p.w, H = p.h;
  const time = c.time, sxOf = (i: number, j: number) => (time ? d.sx?.[i]?.[j] ?? d.xs[j] ?? null : null);
  const n = Math.max(d.cats.length, ...d.vals.map(v => v.length), 0);
  const names = c.series.map((s, i) => d.names[i] || s.name || `Series ${i + 1}`);
  const pie = c.kind === "pie" || c.kind === "doughnut", horiz = c.kind === "bar", scatter = c.kind === "scatter";
  const els: JSX.Element[] = [];
  /** Each part's box (for the selection outline) and each series' points (for its handles). */
  const box: Record<string, [number, number, number, number]> = { area: [0, 0, W, H] }, spts: [number, number][][] = c.series.map(() => []);
  const hit = (part: string, b: [number, number, number, number]) => { box[part] = b; return <rect data-part={part} x={b[0]} y={b[1]} width={Math.max(0, b[2])} height={Math.max(0, b[3])} fill="transparent" />; };
  const [hv, setHv] = createSignal<[number, number] | null>(null);
  let tipAt: (mx: number, my: number) => Tip | null = () => null;
  const text = (x: number, y: number, t: string, o: { size?: number; anchor?: string; bold?: boolean; color?: string; rot?: number; base?: string } = {}) =>
    <text pointer-events="none" x={x} y={y} font-size={String(o.size ?? fs)} font-family={FONT} font-weight={o.bold ? "600" : "400"} style={{ fill: o.color ?? INK }} text-anchor={(o.anchor ?? "start") as "start" | "middle" | "end"} dominant-baseline={(o.base ?? "middle") as "middle" | "hanging"} transform={o.rot ? `rotate(${o.rot} ${x} ${y})` : undefined}>{t}</text>;
  // Title (and subtitle) and legend take their room first.
  let top = 8, bottom = H - 8, left = 8, right = W - 8;
  if (c.title) { const tw0 = tw(c.title, fs * 1.35, true); els.push(text(W / 2, top + fs * 0.8, c.title, { size: fs * 1.35, bold: true, anchor: "middle", color: TITLE })); els.push(hit("title", [W / 2 - tw0 / 2 - 5, top + fs * 0.8 - fs * 0.95, tw0 + 10, fs * 1.9])); top += fs * 1.35 + 8; }
  if (c.subtitle) { els.push(text(W / 2, top + fs * 0.6, c.subtitle, { size: fs * 1.02, anchor: "middle", color: SUB })); top += fs * 1.3 + 4; }
  if (c.title || c.subtitle) top += 4;
  const legend = c.legend || (pie ? "right" : "bottom");
  const items = pie ? d.cats.map((t, i) => [t, PALETTE[i % PALETTE.length], "column"] as const) : names.map((t, i) => [t, colorOf(c, i), kindOf(c, i)] as const);
  if (legend !== "none" && items.length) {
    const sw = fs * 1.6, gap = fs * 1.2, iw = items.map(([t]) => sw + 5 + tw(t, fs)), lh = fs * 1.6;
    const sym = (x: number, y: number, col: string, k: string, i: number) => (k === "line" || k === "scatter"
      ? <line x1={x} x2={x + sw} y1={y} y2={y} stroke={col} stroke-width={Math.min(3, c.series[i]?.width || 2.25)} stroke-dasharray={dashOf(c.series[i]?.dash, 1.5)} stroke-linecap="round" />
      : <rect x={x + sw * 0.2} y={y - fs * 0.4} width={sw * 0.6} height={fs * 0.8} fill={col} rx="1.5" />);
    if (legend === "right" || legend === "left") {
      const lw = Math.min(W * 0.4, Math.max(...iw) + 8), x0 = legend === "right" ? right - lw : left, y0 = (top + bottom) / 2 - items.length * lh / 2;
      items.forEach(([t, col, k], i) => { const y = y0 + i * lh + lh / 2; els.push(sym(x0, y, col, k, i)); els.push(text(x0 + sw + 5, y, t)); });
      els.push(hit("legend", [x0 - 3, y0 - 2, lw + 2, items.length * lh + 4]));
      if (legend === "right") right -= lw + 8; else left += lw + 8;
    } else {
      const rows: number[][] = [[]]; let x = 0; const avail = right - left;
      iw.forEach((w, i) => { if (x + w > avail && rows[rows.length - 1].length) { rows.push([]); x = 0; } rows[rows.length - 1].push(i); x += w + gap; });
      const lhTot = rows.length * lh, y0 = legend === "top" ? top : bottom - lhTot;
      let lx0 = Infinity, lx1 = -Infinity;
      rows.forEach((r, ri) => { const rw = r.reduce((a, i) => a + iw[i] + gap, -gap); let xx = left + (avail - rw) / 2; lx0 = Math.min(lx0, xx); lx1 = Math.max(lx1, xx + rw); for (const i of r) { const y = y0 + ri * lh + lh / 2; els.push(sym(xx, y, items[i][1], items[i][2], i)); els.push(text(xx + sw + 5, y, items[i][0])); xx += iw[i] + gap; } });
      els.push(hit("legend", [lx0 - 4, y0 - 1, lx1 - lx0 + 8, lhTot + 2]));
      if (legend === "top") top += lhTot + 6; else bottom -= lhTot + 6;
    }
  }
  const bg = <rect data-part="area" x="0" y="0" width={W} height={H} style={{ fill: c.bg || BG }} />;
  const shell = (body: JSX.Element) => {
    let svg!: SVGSVGElement;
    const at = (e: MouseEvent): [number, number] => { const r = svg.getBoundingClientRect(); return [(e.clientX - r.left) * W / (r.width || W), (e.clientY - r.top) * H / (r.height || H)]; };
    return (
      <svg ref={svg} width={W} height={H} viewBox={`0 0 ${W} ${H}`} xmlns="http://www.w3.org/2000/svg" style={{ display: "block", "user-select": "none", "-webkit-user-select": "none" }}
        onMouseMove={p.hover === false ? undefined : e => setHv(at(e))} onMouseLeave={() => setHv(null)}>
        {body}
        {(() => (p.sel ? selection(p.sel) : null)) as unknown as JSX.Element}
        {p.hover === false ? null : (() => { const m = hv(); const t = m && tipAt(m[0], m[1]); return t ? tipBox(t, m![0], m![1]) : null; }) as unknown as JSX.Element}
      </svg>
    );
  };
  // The selected part: a dashed frame with corner handles, or a handle on each point of a series.
  const selection = (part: string) => {
    const SEL = "var(--blue, #2F6FCF)";
    if (part.startsWith("s")) return <g pointer-events="none">{(spts[keep.indexOf(+part.slice(1))] ?? []).slice(0, 400).map(([x, y]) => <rect x={x - 3} y={y - 3} width="6" height="6" fill="#fff" stroke={SEL} stroke-width="1.2" />)}</g>;
    const b = box[part]; if (!b) return null;
    const [x, y, w, h] = part === "area" ? [1, 1, W - 2, H - 2] : b;
    return <g pointer-events="none"><rect x={x} y={y} width={w} height={h} fill="none" stroke={SEL} stroke-width="1.2" stroke-dasharray={part === "area" ? undefined : "4 3"} />
      {[[x, y], [x + w, y], [x, y + h], [x + w, y + h]].map(([a, b2]) => <rect x={a - 3} y={b2 - 3} width="6" height="6" fill="#fff" stroke={SEL} stroke-width="1.2" />)}</g>;
  };
  // The values at the pointer: a box with the place's name and each series' value, in its colour.
  const tipBox = (t: Tip, mx: number, my: number) => {
    const f = Math.max(10.5, fs * 0.95), pad = 8, lh = f * 1.5, rows = t.rows.slice(0, 14);
    const w = Math.max(t.head ? tw(t.head, f, true) : 0, ...rows.map(r => 16 + tw(r.name, f) + 14 + tw(r.val, f, true))) + pad * 2;
    const ht = pad * 2 + lh * (rows.length + (t.head ? 1 : 0)) - f * 0.3;
    let x = mx + 16; if (x + w > W - 2) x = mx - 16 - w; x = Math.max(2, x);
    const y = Math.max(2, Math.min(H - ht - 2, my - ht / 2));
    return <g pointer-events="none">
      {t.band ? <rect x={t.band[0]} y={t.band[1]} width={t.band[2]} height={t.band[3]} style={{ fill: INK }} opacity=".07" /> : null}
      {t.guide !== undefined ? <line x1={t.guide} x2={t.guide} y1={box.plot?.[1] ?? 0} y2={(box.plot?.[1] ?? 0) + (box.plot?.[3] ?? H)} style={{ stroke: AXIS }} stroke-width="1" stroke-dasharray="3 3" /> : null}
      {(t.dots ?? []).map(([a, b, col]) => <circle cx={a} cy={b} r="4.2" fill={col} stroke="#fff" stroke-width="1.6" />)}
      <rect x={x} y={y} width={w} height={ht} rx="5" style={{ fill: "var(--ch-tip, #FFFFFF)", stroke: AXIS, filter: "drop-shadow(0 2px 5px rgba(0,0,0,.18))" }} stroke-width=".8" />
      {t.head ? text(x + pad, y + pad + f * 0.55, t.head, { size: f, bold: true, color: TITLE }) : null}
      {rows.map((r, k) => { const yy = y + pad + f * 0.55 + lh * (k + (t.head ? 1 : 0)); return <g>
        <rect x={x + pad} y={yy - f * 0.38} width={f * 0.76} height={f * 0.76} rx="1.5" fill={r.col} />
        {text(x + pad + 16, yy, r.name, { size: f })}{text(x + w - pad, yy, r.val, { size: f, bold: true, anchor: "end", color: TITLE })}</g>; })}
    </g>;
  };
  if (pie) {
    const cx = (left + right) / 2, cy = (top + bottom) / 2, r = Math.max(10, Math.min(right - left, bottom - top) / 2 - fs * 2);
    const v = (d.vals[0] ?? []).map(x => Math.max(0, x ?? 0)), tot = v.reduce((a, b) => a + b, 0), hole = c.kind === "doughnut" ? r * 0.58 : 0;
    box.plot = [cx - r, cy - r, r * 2, r * 2];
    tipAt = (mx, my) => {
      const dx = mx - cx, dy = my - cy, rr = Math.hypot(dx, dy); if (!tot || rr > r || rr < hole) return null;
      let a = Math.atan2(dy, dx) + Math.PI / 2; if (a < 0) a += Math.PI * 2;
      let acc = 0; for (let i = 0; i < v.length; i++) { acc += (v[i] / tot) * Math.PI * 2; if (a <= acc && v[i]) return { head: d.cats[i] ?? "", rows: [{ col: c.series[0]?.colors?.[i] ?? PALETTE[i % PALETTE.length], name: names[0], val: `${fmtNum(d.vals[0][i] ?? 0, c.y?.fmt || d.fmts[0] || "")} (${Math.round((v[i] / tot) * 1000) / 10}%)` }] }; }
      return null;
    };
    return shell(<>{bg}{els}{pieOf(c, d, cx, cy, r, fs, text)}</>);
  }
  // Value axes: the primary, and the secondary when a series is on it.
  const onY2 = (i: number) => !!c.series[i]?.y2 && !horiz && !scatter;
  const stacked = c.stack === "stacked" || c.stack === "percent", pctStack = c.stack === "percent";
  const len = (i: number) => (time ? d.vals[i]?.length ?? 0 : n);
  const val = (i: number, j: number) => d.vals[i]?.[j] ?? null;
  const extent = (two: boolean) => {
    let lo = Infinity, hi = -Infinity, zero = false;
    const idx = c.series.map((_, i) => i).filter(i => onY2(i) === two);
    if (!idx.length) return null;
    for (const i of idx) if (["column", "bar", "area"].includes(kindOf(c, i)) || c.series[i].fill) zero = true;
    if (stacked && !scatter && !time) {
      for (let j = 0; j < n; j++) {
        let pos = 0, neg = 0, tot = 0;
        for (const i of idx) { const v = val(i, j); if (v === null) continue; tot += Math.abs(v); if (v >= 0) pos += v; else neg += v; }
        if (pctStack) { if (tot) { hi = Math.max(hi, pos / tot); lo = Math.min(lo, neg / tot); } } else { hi = Math.max(hi, pos); lo = Math.min(lo, neg); }
      }
    } else for (const i of idx) for (const v of d.vals[i] ?? []) if (v !== null) { lo = Math.min(lo, v); hi = Math.max(hi, v); }
    if (!isFinite(lo)) { lo = 0; hi = 1; }
    return { lo, hi, zero };
  };
  const e1 = extent(false), e2 = extent(true);
  const fmtFor = (a: Axis | undefined, i: number) => a?.fmt || (pctStack ? "0%" : d.fmts[i] || "");
  const y1fmt = fmtFor(c.y, c.series.findIndex((_, i) => !onY2(i))), y2fmt = fmtFor(c.y2, c.series.findIndex((_, i) => onY2(i)));
  // Room for the axis titles and tick labels (a first pass with a guessed scale).
  const valLen = horiz ? right - left : bottom - top;
  const guess1 = e1 ? scale(e1.lo, e1.hi, c.y, valLen, e1.zero) : null, guess2 = e2 ? scale(e2.lo, e2.hi, c.y2, valLen, e2.zero) : null;
  const xs = d.xs.map(v => v ?? NaN), xfin = xs.filter(isFinite);
  const xsc = scatter ? scale(Math.min(...xfin, Infinity), Math.max(...xfin, -Infinity), c.x, right - left, false) : null;
  const catFmt = c.x?.fmt || d.cat_fmt;
  const catLabel = (j: number) => (scatter ? "" : c.x?.fmt && d.xs[j] !== null && d.xs[j] !== undefined ? fmtNum(d.xs[j]!, c.x.fmt) : d.cats[j] ?? "");
  let yT = 0, y2T = 0, xT = 0;
  if (c.y?.title && !c.y.hide) { if (horiz) { yT = bottom - fs * 0.7; bottom -= fs * 1.6; } else { yT = left + fs * 0.7; left += fs * 1.6; } }
  if (guess2 && c.y2?.title && !c.y2.hide) { y2T = right - fs * 0.7; right -= fs * 1.6; }
  if (c.x?.title && !c.x.hide) { if (horiz) { xT = left + fs * 0.7; left += fs * 1.6; } else { xT = bottom - fs * 0.7; bottom -= fs * 1.6; } }
  const tl = (g: ReturnType<typeof scale> | null, f: string) => (g ? Math.max(0, ...g.ticks.map(t => tw(fmtNum(t, f), fs))) : 0);
  const catW = horiz ? Math.min((right - left) * 0.4, Math.max(0, ...Array.from({ length: n }, (_, j) => tw(catLabel(j), fs)))) : 0;
  if (horiz) { left += (c.x?.hide ? 0 : catW + 8); bottom -= c.y?.hide ? 0 : fs * 1.6; }
  else { left += c.y?.hide || !guess1 ? 4 : tl(guess1, y1fmt) + 8; if (guess2) right -= c.y2?.hide ? 4 : tl(guess2, y2fmt) + 8; }
  // The last value at the end of each line needs room on the right.
  const lineIdx = c.series.map((_, i) => i).filter(i => ["line", "area"].includes(kindOf(c, i)));
  if (c.ends && lineIdx.length && !horiz) right -= Math.max(...lineIdx.map(i => { const v = [...(d.vals[i] ?? [])].reverse().find(x => x !== null); return v === undefined || v === null ? 0 : tw(fmtNum(v, onY2(i) ? y2fmt : y1fmt), fs, true) + 8; }), 0);
  // Category (or time) labels under the plot: level, slanted, or every k-th when crowded.
  let rot = 0, every = 1, xlabH = 0;
  if (!horiz) {
    const nt = time ? time.ticksAt.length : n;
    const band = (right - left) / Math.max(1, nt), lw = scatter && xsc ? Math.max(...xsc.ticks.map(t => tw(fmtNum(t, catFmt), fs))) : time ? Math.max(0, ...time.ticksAt.map((_, k) => tw(time.tickLabel(k), fs))) : Math.max(0, ...Array.from({ length: n }, (_, j) => tw(catLabel(j), fs)));
    if (!scatter && (lw + 6 > band || time?.rotate)) { rot = -45; every = Math.max(1, Math.ceil(fs * 1.5 / band)); }
    xlabH = c.x?.hide ? 0 : rot ? lw * 0.72 + fs * 0.8 + 6 : fs * 1.6;
    bottom -= c.x?.hide ? 4 : xlabH;
  }
  const pl = left, pr = Math.max(left + 20, right), pt = top + (c.marker ? fs * 1.7 : 0), pb = Math.max(pt + 20, bottom);
  const vlen = horiz ? pr - pl : pb - pt;
  const s1 = e1 ? scale(e1.lo, e1.hi, c.y, vlen, e1.zero) : null, s2 = e2 ? scale(e2.lo, e2.hi, c.y2, vlen, e2.zero) : null;
  const pos = (v: number, s: { mn: number; mx: number }) => { const t = (v - s.mn) / (s.mx - s.mn); return horiz ? pl + t * (pr - pl) : pb - t * (pb - pt); };
  const sc = (i: number) => (onY2(i) ? s2! : s1!);
  const band = (horiz ? pb - pt : pr - pl) / Math.max(1, n);
  const catAt = (j: number) => (horiz ? pt + band * (j + 0.5) : pl + band * (j + 0.5));
  const tX = (v: number) => (time ? pl + ((v - time.min) / Math.max(1e-9, time.max - time.min)) * (pr - pl) : pl);
  const xAt = (i: number, j: number) => (time ? tX(sxOf(i, j) ?? NaN) : scatter && xsc ? pl + ((xs[j] - xsc.mn) / (xsc.mx - xsc.mn)) * (pr - pl) : catAt(j));
  const plotBg = <rect data-part="plot" x={pl} y={pt} width={pr - pl} height={pb - pt} fill={c.plot || "transparent"} />;
  box.plot = [pl, pt, pr - pl, pb - pt];
  const grid: JSX.Element[] = [], axes: JSX.Element[] = [];
  if (s1) for (const t of s1.ticks) {
    const v = pos(t, s1);
    if (c.y?.grid) grid.push(horiz ? <line x1={v} x2={v} y1={pt} y2={pb} style={{ stroke: GRID }} stroke-width=".8" /> : <line x1={pl} x2={pr} y1={v} y2={v} style={{ stroke: GRID }} stroke-width=".8" />);
    if (!c.y?.hide) axes.push(horiz ? text(v, pb + fs * 0.9, fmtNum(t, y1fmt), { anchor: "middle" }) : text(pl - 6, v, fmtNum(t, y1fmt), { anchor: "end" }));
  }
  if (s2 && !c.y2?.hide) for (const t of s2.ticks) { const v = pos(t, s2); axes.push(text(pr + 6 + (c.ends && lineIdx.length ? 0 : 0), v, fmtNum(t, y2fmt))); if (c.y2?.grid) grid.push(<line x1={pl} x2={pr} y1={v} y2={v} style={{ stroke: GRID }} stroke-width=".8" stroke-dasharray="3 3" />); }
  if (time) time.ticksAt.forEach((t, k) => { if (k % every) return; const x = tX(t); if (c.x?.grid) grid.push(<line x1={x} x2={x} y1={pt} y2={pb} style={{ stroke: GRID }} stroke-width=".8" />); if (!c.x?.hide) axes.push(rot ? text(x + fs * 0.3, pb + 6, time.tickLabel(k), { anchor: "end", rot, base: "hanging" }) : text(x, pb + fs * 0.9, time.tickLabel(k), { anchor: "middle" })); });
  else if (scatter && xsc) { for (const t of xsc.ticks) { const x = pl + ((t - xsc.mn) / (xsc.mx - xsc.mn)) * (pr - pl); if (c.x?.grid) grid.push(<line x1={x} x2={x} y1={pt} y2={pb} style={{ stroke: GRID }} stroke-width=".8" />); if (!c.x?.hide) axes.push(text(x, pb + fs * 0.9, fmtNum(t, catFmt), { anchor: "middle" })); } }
  else for (let j = 0; j < n; j++) {
    const at = catAt(j);
    if (c.x?.grid) grid.push(horiz ? <line x1={pl} x2={pr} y1={pt + band * j} y2={pt + band * j} style={{ stroke: GRID }} stroke-width=".8" /> : <line x1={pl + band * j} x2={pl + band * j} y1={pt} y2={pb} style={{ stroke: GRID }} stroke-width=".8" />);
    if (c.x?.hide || j % every) continue;
    axes.push(horiz ? text(pl - 6, at, catLabel(j), { anchor: "end" }) : rot ? text(at + fs * 0.3, pb + 6, catLabel(j), { anchor: "end", rot, base: "hanging" }) : text(at, pb + fs * 0.9, catLabel(j), { anchor: "middle" }));
  }
  // Each axis's labels (and title) are a part a click selects.
  { const l1 = s1 ? tl(s1, y1fmt) : 0, l2 = s2 ? tl(s2, y2fmt) : 0, tt = fs * 1.6;
    if (horiz) {
      if (s1 && !c.y?.hide) axes.push(hit("y", [pl, pb + 2, pr - pl, fs * 1.6 + (yT ? tt : 0)]));
      if (!c.x?.hide) axes.push(hit("x", [pl - catW - 8 - (xT ? tt : 0), pt, catW + 6 + (xT ? tt : 0), pb - pt]));
    } else {
      if (s1 && !c.y?.hide) { const x0 = Math.min(yT ? yT - fs * 0.8 : Infinity, pl - l1 - 8); axes.push(hit("y", [x0, pt - fs * 0.7, pl - x0 - 1, pb - pt + fs * 1.4])); }
      if (s2 && !c.y2?.hide) axes.push(hit("y2", [pr + 1, pt - fs * 0.7, l2 + 8 + (y2T ? tt : 0), pb - pt + fs * 1.4]));
      if (!c.x?.hide) axes.push(hit("x", [pl - 4, pb + 2, pr - pl + 8, xlabH + (xT ? tt : 0)]));
    } }
  const zero1 = s1 ? pos(Math.min(Math.max(0, s1.mn), s1.mx), s1) : horiz ? pl : pb;
  axes.push(horiz ? <line x1={zero1} x2={zero1} y1={pt} y2={pb} style={{ stroke: AXIS }} stroke-width="1" /> : <line x1={pl} x2={pr} y1={zero1} y2={zero1} style={{ stroke: AXIS }} stroke-width="1" />);
  if (yT) axes.push(horiz ? text((pl + pr) / 2, yT, c.y!.title!, { anchor: "middle", bold: true }) : text(yT, (pt + pb) / 2, c.y!.title!, { anchor: "middle", bold: true, rot: -90 }));
  if (y2T && s2) axes.push(text(y2T, (pt + pb) / 2, c.y2!.title!, { anchor: "middle", bold: true, rot: 90 }));
  if (xT) axes.push(horiz ? text(xT, (pt + pb) / 2, c.x!.title!, { anchor: "middle", bold: true, rot: -90 }) : text((pl + pr) / 2, xT, c.x!.title!, { anchor: "middle", bold: true }));
  // The series: columns (or bars) side by side or stacked, then areas, lines and markers on top.
  const marks: JSX.Element[] = [], over: JSX.Element[] = [], labels: JSX.Element[] = [];
  const fmtV = (i: number, v: number) => fmtNum(v, d.fmts[i] || (onY2(i) ? y2fmt : y1fmt));
  const bars = c.series.map((_, i) => i).filter(i => ["column", "bar"].includes(kindOf(c, i)));
  const gapPct = c.gap ?? 150, slots = stacked ? 1 : Math.max(1, bars.length);
  // On a time axis, a bar is as wide as the gap between its neighbours allows.
  const tband = (i: number) => { const v = (d.sx?.[i] ?? []).filter((x): x is number => x !== null); let g = Infinity; for (let k = 1; k < v.length; k++) g = Math.min(g, v[k] - v[k - 1]); return isFinite(g) ? Math.abs(tX(v[0] + g) - tX(v[0])) : band; };
  const base = (j: number, i: number, v: number) => {
    if (!stacked || time) return 0;
    let s = 0, tot = 0;
    for (let k = 0; k < c.series.length; k++) { const x = val(k, j); if (x === null || onY2(k) !== onY2(i) || kindOf(c, k) !== kindOf(c, i)) continue; tot += Math.abs(x); if (k < i && Math.sign(x || 1) === Math.sign(v || 1)) s += x; }
    return pctStack && tot ? s / tot : s;
  };
  const scaled = (j: number, i: number, v: number) => { if (!pctStack || time) return v; let tot = 0; for (let k = 0; k < c.series.length; k++) { const x = val(k, j); if (x !== null && onY2(k) === onY2(i) && kindOf(c, k) === kindOf(c, i)) tot += Math.abs(x); } return tot ? v / tot : 0; };
  const rad = c.radius ?? 0;
  bars.forEach((i, bi) => {
    const s = sc(i); if (!s) return;
    const bnd = time ? tband(i) : band, bw = bnd / (slots + gapPct / 100);
    for (let j = 0; j < len(i); j++) {
      const v0 = val(i, j); if (v0 === null) continue;
      const v = scaled(j, i, v0), b = base(j, i, v), a = pos(b, s), z = pos(b + v, s), cx = time ? xAt(i, j) : catAt(j), mid = cx - (slots * bw) / 2 + (stacked ? 0 : bi * bw);
      if (time && !isFinite(cx)) continue;
      const [x, y, w, h] = horiz ? [Math.min(a, z), mid, Math.abs(z - a), bw] : [mid, Math.min(a, z), bw, Math.abs(z - a)];
      marks.push(<rect data-part={sp(i)} x={x} y={y} width={Math.max(0.5, w)} height={Math.max(0.5, h)} rx={Math.min(rad, w / 2, h / 2)} fill={colorOf(c, i)} />);
      spts[i].push(horiz ? [x + w, y + h / 2] : [x + w / 2, y]);
      if (c.series[i].labels || c.labels) labels.push(horiz ? text(Math.max(a, z) + 3, mid + bw / 2, fmtV(i, v0), { size: fs * 0.9 }) : text(mid + bw / 2, Math.min(a, z) - fs * 0.6, fmtV(i, v0), { size: fs * 0.9, anchor: "middle" }));
    }
  });
  const endsAt: number[] = [];
  c.series.forEach((ser, i) => {
    const k = kindOf(c, i); if (k === "column" || k === "bar") return;
    const s = sc(i); if (!s) return;
    const col = colorOf(c, i), w = ser.width || 2.25;
    const pts = Array.from({ length: len(i) }, (_, j) => { const v0 = val(i, j), x = xAt(i, j); if (v0 === null || !isFinite(x)) return null; const v = scaled(j, i, v0); return [x, pos(base(j, i, v) + v, s)] as [number, number]; });
    const floor = (j: number) => { const v0 = val(i, j); return v0 === null ? pb : pos(base(j, i, scaled(j, i, v0)), s); };
    if (k === "area" || ser.fill) {
      // An area (or a line's soft fill): down to the axis, or to the series below when stacked.
      const id = `xg${++gid}`;
      let d2 = "", run: number[] = [];
      const lowY = (j: number) => (k === "area" ? floor(j) : pos(Math.max(s.mn, Math.min(0, s.mx)), s));
      const flush = () => { if (run.length) { d2 += `M${run.map(j => `${pts[j]![0]} ${pts[j]![1]}`).join("L")}L${[...run].reverse().map(j => `${pts[j]![0]} ${lowY(j)}`).join("L")}Z`; } run = []; };
      pts.forEach((p2, j) => { if (p2) run.push(j); else flush(); }); flush();
      marks.push(<g><defs><linearGradient id={id} x1="0" x2="0" y1="0" y2="1"><stop offset="0" stop-color={col} stop-opacity={k === "area" ? ".75" : ".32"} /><stop offset="1" stop-color={col} stop-opacity={k === "area" ? ".55" : ".03"} /></linearGradient></defs><path data-part={sp(i)} d={d2} fill={`url(#${id})`} stroke={k === "area" ? col : "none"} stroke-width="1" pointer-events={k === "area" ? undefined : "none"} /></g>);
      if (k === "area") pts.forEach(q => q && spts[i].push(q));
      if (k === "area") return;
    }
    const smooth = !!ser.smooth;
    pts.forEach(q => q && spts[i].push(q));
    over.push(<path data-part={sp(i)} d={pathOf(pts, smooth)} fill="none" stroke="transparent" stroke-width="12" />);
    over.push(<path data-part={sp(i)} d={pathOf(pts, smooth)} fill="none" stroke={col} stroke-width={w} stroke-dasharray={dashOf(ser.dash, w)} stroke-linejoin="round" stroke-linecap="round" style={scatter && ser.marker !== "none" && !ser.width ? { display: "none" } : {}} />);
    const mk = ser.marker === "none" ? "" : ser.marker && ser.marker !== "auto" ? ser.marker : scatter || (!time && n <= 30 && !c.ends) ? "circle" : "";
    if (mk) pts.forEach(p2 => { if (p2) over.push(<g data-part={sp(i)}><Marker x={p2[0]} y={p2[1]} kind={mk} c={col} r={Math.max(2.5, w * 1.3)} /></g>); });
    if (ser.labels || c.labels) pts.forEach((p2, j) => { if (p2) labels.push(text(p2[0], p2[1] - fs * 0.9, fmtV(i, val(i, j)!), { size: fs * 0.9, anchor: "middle" })); });
    // The last value at the line's end, in its colour (labels kept apart).
    if (c.ends) { let j = pts.length - 1; while (j >= 0 && !pts[j]) j--; if (j >= 0) { let y = pts[j]![1]; while (endsAt.some(u => Math.abs(u - y) < fs * 1.2)) y -= fs * 1.2; endsAt.push(y); labels.push(text(pts[j]![0] + 6, y, fmtV(i, val(i, j)!), { bold: true, color: col })); } }
  });
  // Linear trend lines (least squares over the points shown).
  c.series.forEach((ser, i) => {
    if (ser.trend !== "linear") return;
    const s = sc(i); if (!s) return;
    const P: [number, number][] = []; for (let j = 0; j < len(i); j++) { const v = val(i, j); const x = time ? sxOf(i, j) : scatter ? xs[j] : j; if (v !== null && x !== null && isFinite(x)) P.push([x, v]); }
    if (P.length < 2) return;
    const mx = P.reduce((a, q) => a + q[0], 0) / P.length, my = P.reduce((a, q) => a + q[1], 0) / P.length, sxx = P.reduce((a, q) => a + (q[0] - mx) ** 2, 0);
    if (!sxx) return;
    const b = P.reduce((a, q) => a + (q[0] - mx) * (q[1] - my), 0) / sxx, a0 = my - b * mx, x0 = P[0][0], x1 = P[P.length - 1][0];
    const X = (x: number) => (time ? tX(x) : scatter && xsc ? pl + ((x - xsc.mn) / (xsc.mx - xsc.mn)) * (pr - pl) : catAt(x));
    const Y = (x: number) => pos(a0 + b * x, s);
    over.push(horiz ? <line x1={Y(x0)} x2={Y(x1)} y1={X(x0)} y2={X(x1)} stroke={colorOf(c, i)} stroke-width="1.5" stroke-dasharray="6 4" opacity=".85" /> : <line x1={X(x0)} x2={X(x1)} y1={Y(x0)} y2={Y(x1)} stroke={colorOf(c, i)} stroke-width="1.5" stroke-dasharray="6 4" opacity=".85" pointer-events="none" />);
  });
  // The status date: a dashed line with its label above the plot.
  const mk = c.marker;
  if (mk && !horiz) {
    const x = time ? tX(mk.at) : catAt(mk.at);
    if (isFinite(x) && x >= pl - 1 && x <= pr + 1) {
      const lw = tw(mk.label, fs * 0.92, true) + 14, lx = Math.min(Math.max(x - lw / 2, pl), pr - lw), hh = fs * 1.45;
      over.push(<g><line x1={x} x2={x} y1={pt} y2={pb} stroke="#2E9D57" stroke-width="1.4" stroke-dasharray="4 4" /><rect x={lx} y={pt - hh - 3} width={lw} height={hh} rx={hh / 2} fill="#2E9D57" />{text(lx + lw / 2, pt - 3 - hh / 2, mk.label, { size: fs * 0.92, bold: true, anchor: "middle", color: "#FFFFFF" })}</g>);
    }
  }
  const empty = !n || d.vals.every(v => v.every(x => x === null || x === 0));
  // What the pointer is over: a category (every series' value there), a time (each series' nearest
  // point), or a point of a scatter chart.
  const yOf = (i: number, j: number) => { const v0 = val(i, j), s = sc(i); if (v0 === null || !s) return null; const v = scaled(j, i, v0); return pos(base(j, i, v) + v, s); };
  const isLine = (i: number) => !["column", "bar"].includes(kindOf(c, i));
  const row = (i: number, j: number) => ({ col: colorOf(c, i), name: names[i], val: fmtV(i, val(i, j)!) });
  tipAt = (mx, my) => {
    if (empty || mx < pl - 2 || mx > pr + 2 || my < pt - 2 || my > pb + 2) return null;
    const idx = c.series.map((_, i) => i);
    if (scatter) {
      let best: [number, number, number] | null = null;
      for (const i of idx) for (let j = 0; j < n; j++) { const y = yOf(i, j), x = xAt(i, j); if (y === null || !isFinite(x)) continue; const dd = Math.hypot(x - mx, y - my); if (dd < 24 && (!best || dd < best[2])) best = [i, j, dd]; }
      if (!best) return null; const [i, j] = best;
      return { head: names[i], rows: [{ col: colorOf(c, i), name: fmtNum(xs[j], catFmt), val: fmtV(i, val(i, j)!) }], dots: [[xAt(i, j), yOf(i, j)!, colorOf(c, i)]] };
    }
    if (time) {
      let g: [number, number, number] | null = null; const near: [number, number][] = [];
      for (const i of idx) { let b = -1, bd = Infinity; for (let j = 0; j < len(i); j++) { if (val(i, j) === null) continue; const dd = Math.abs(xAt(i, j) - mx); if (dd < bd) { bd = dd; b = j; } } if (b >= 0 && bd <= Math.max(10, tband(i) / 2 + 2)) { near.push([i, b]); if (!g || bd < g[2]) g = [i, b, bd]; } }
      if (!g) return null;
      return { head: d.tips?.[g[0]]?.[g[1]] ?? fmtNum(sxOf(g[0], g[1]) ?? 0, "@day"), rows: near.map(([i, j]) => row(i, j)), guide: xAt(g[0], g[1]), dots: near.filter(([i]) => isLine(i)).map(([i, j]) => [xAt(i, j), yOf(i, j)!, colorOf(c, i)] as [number, number, string]) };
    }
    const j = Math.max(0, Math.min(n - 1, Math.floor(((horiz ? my - pt : mx - pl)) / band)));
    const here = idx.filter(i => val(i, j) !== null);
    if (!here.length) return null;
    return { head: catLabel(j), rows: here.map(i => row(i, j)), band: bars.length ? (horiz ? [pl, pt + band * j, pr - pl, band] : [pl + band * j, pt, band, pb - pt]) : undefined,
      guide: !bars.length && !horiz ? catAt(j) : undefined, dots: horiz ? [] : here.filter(isLine).map(i => [catAt(j), yOf(i, j)!, colorOf(c, i)] as [number, number, string]) };
  };
  return shell(<>
    {bg}{plotBg}{grid}{marks}{axes}{over}{labels}{els}
    <Show when={empty && !c.quiet}><text pointer-events="none" x={W / 2} y={(pt + pb) / 2} font-size={String(fs)} font-family={FONT} style={{ fill: AXIS }} text-anchor="middle">No data in the chart's ranges yet</text></Show>
  </>);
};

/** A pie or doughnut of the first series, slices labelled with their share. */
const pieOf = (c: ChartDef, d: ChartData, cx: number, cy: number, r: number, fs: number, text: (x: number, y: number, t: string, o?: { size?: number; anchor?: string; bold?: boolean; color?: string }) => JSX.Element) => {
  const v = (d.vals[0] ?? []).map(x => Math.max(0, x ?? 0)), tot = v.reduce((a, b) => a + b, 0);
  if (!tot) return c.quiet ? null : <text x={cx} y={cy} font-size={String(fs)} font-family={FONT} style={{ fill: AXIS }} text-anchor="middle">No data in the chart's ranges yet</text>;
  const hole = c.kind === "doughnut" ? r * 0.58 : 0, out: JSX.Element[] = [];
  let a = -Math.PI / 2;
  v.forEach((x, i) => {
    if (!x) return;
    const b = a + (x / tot) * Math.PI * 2, big = b - a > Math.PI ? 1 : 0, P = (ang: number, rr: number) => `${cx + rr * Math.cos(ang)} ${cy + rr * Math.sin(ang)}`;
    const dd = x === tot ? (hole ? `M${P(0, r)}A${r} ${r} 0 1 1 ${P(Math.PI, r)}A${r} ${r} 0 1 1 ${P(0, r)}M${P(0, hole)}A${hole} ${hole} 0 1 0 ${P(Math.PI, hole)}A${hole} ${hole} 0 1 0 ${P(0, hole)}Z` : `M${P(0, r)}A${r} ${r} 0 1 1 ${P(Math.PI, r)}A${r} ${r} 0 1 1 ${P(0, r)}Z`)
      : hole ? `M${P(a, r)}A${r} ${r} 0 ${big} 1 ${P(b, r)}L${P(b, hole)}A${hole} ${hole} 0 ${big} 0 ${P(a, hole)}Z` : `M${cx} ${cy}L${P(a, r)}A${r} ${r} 0 ${big} 1 ${P(b, r)}Z`;
    out.push(<path d={dd} fill={c.series[0]?.colors?.[i] ?? PALETTE[i % PALETTE.length]} style={{ stroke: BG }} stroke-width="1.5" fill-rule="evenodd" data-part="s0" />);
    if (x / tot >= 0.04) { const m = (a + b) / 2, rr = hole ? (r + hole) / 2 : r * 0.68; out.push(text(cx + rr * Math.cos(m), cy + rr * Math.sin(m), (c.labels || c.series[0]?.labels) ? fmtNum(d.vals[0][i] ?? 0, c.y?.fmt || d.fmts[0] || "") : `${Math.round((x / tot) * 100)}%`, { anchor: "middle", color: "#FFFFFF", bold: true, size: fs * 0.95 })); }
    a = b;
  });
  return <g>{out}</g>;
};

/** The look of a chart (what a saved style keeps): not its data, place or titles. */
export const lookOf = (c: ChartDef): Partial<ChartDef> => ({
  legend: c.legend, labels: c.labels, font: c.font, bg: c.bg, plot: c.plot, gap: c.gap, stack: c.stack, kind: c.kind,
  x: { fmt: c.x?.fmt, grid: c.x?.grid, title: c.x?.title }, y: { fmt: c.y?.fmt, grid: c.y?.grid, min: c.y?.min, max: c.y?.max, major: c.y?.major, title: c.y?.title }, y2: { fmt: c.y2?.fmt, grid: c.y2?.grid, min: c.y2?.min, max: c.y2?.max, major: c.y2?.major, title: c.y2?.title },
  series: c.series.map(s => ({ name: "", vals: "", kind: s.kind, y2: s.y2, color: s.color, width: s.width, dash: s.dash, marker: s.marker, smooth: s.smooth, labels: s.labels, trend: s.trend })),
});
/** A chart with a look given: `strict` takes each series' look by position only (a view's own series);
 *  otherwise the last look serves the series beyond it (a style saved from a smaller chart). */
export const withLook = (c: ChartDef, l: Partial<ChartDef> | undefined, strict = false): ChartDef => {
  if (!l) return c;
  const def = <T,>(o: T) => Object.fromEntries(Object.entries(o as object).filter(([, v]) => v !== undefined)) as Partial<T>;
  return {
    ...c, ...def(Object.fromEntries(Object.entries(l).filter(([k]) => !["series", "x", "y", "y2"].includes(k)))),
    x: { ...c.x, ...def(l.x ?? {}) }, y: { ...c.y, ...def(l.y ?? {}) }, y2: { ...c.y2, ...def(l.y2 ?? {}) },
    series: c.series.map((s, i) => { const t = l.series?.[i] ?? (strict ? undefined : l.series?.[l.series.length - 1]); return t ? { ...s, ...def(Object.fromEntries(Object.entries(t).filter(([k]) => k !== "name" && k !== "vals"))) } : s; }),
  };
};

/** A chart as a standalone SVG document (for print, PDF and pictures). */
export const chartSvg = (c: ChartDef, d: ChartData | undefined) => {
  const host = document.createElement("div");
  const dispose = render(() => <XChart c={c} d={d} w={c.w} h={c.h} hover={false} />, host);
  const svg = host.innerHTML; dispose();
  return svg;
};
export const svgUrl = (svg: string) => `data:image/svg+xml;base64,${btoa(unescape(encodeURIComponent(svg)))}`;
/** A chart drawn on a canvas at `scale` (2 = sharp on paper and high-density screens). */
export const chartCanvas = (c: ChartDef, d: ChartData | undefined, scale = 2) => new Promise<HTMLCanvasElement>((ok, no) => {
  const im = new Image();
  im.onload = () => { const cv = document.createElement("canvas"); cv.width = c.w * scale; cv.height = c.h * scale; const x = cv.getContext("2d")!; x.fillStyle = "#fff"; x.fillRect(0, 0, cv.width, cv.height); x.drawImage(im, 0, 0, cv.width, cv.height); ok(cv); };
  im.onerror = () => no(new Error("The chart could not be drawn as a picture."));
  im.src = svgUrl(chartSvg(c, d));
});

/** A new chart from a block of cells: its first column the categories, its first row the series names. */
export const chartFrom = (sheet: string, r0: number, c0: number, r1: number, c1: number, a1: (r: number, c: number) => string, cells: (a: string) => string, kind = "column"): ChartDef | null => {
  if (r1 < r0 || c1 < c0) return null;
  const head = (c: number) => { const t = cells(a1(r0, c)); return t !== "" && isNaN(+t); };
  const hasHead = Array.from({ length: c1 - c0 + 1 }, (_, i) => c0 + i).some(head) && r1 > r0;
  const firstText = r1 > r0 && Array.from({ length: r1 - r0 }, (_, i) => r0 + 1 + i).some(r => { const t = cells(a1(r, c0)); return t !== "" && isNaN(+t); });
  const catCol = c1 > c0 && (firstText || /date|week|month|period|day/i.test(cells(a1(r0, c0)))) ? c0 : -1;
  const d0 = hasHead ? r0 + 1 : r0, cols = Array.from({ length: c1 - c0 + 1 }, (_, i) => c0 + i).filter(c => c !== catCol);
  if (!cols.length) return null;
  void sheet;
  return {
    id: 0, cell: "", w: 560, h: 320, kind, cats: catCol >= 0 ? `${a1(d0, catCol)}:${a1(r1, catCol)}` : "",
    series: cols.map((c, i) => ({ name: hasHead ? `=${a1(r0, c)}` : `Series ${i + 1}`, vals: `${a1(d0, c)}:${a1(r1, c)}`, color: PALETTE[i % PALETTE.length] })),
    title: "", legend: "bottom", x: {}, y: { grid: true }, y2: {}, gap: 150,
  };
};
