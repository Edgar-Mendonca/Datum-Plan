// Chart Tools: what a spreadsheet offers around a selected chart. The Chart Design tab of the ribbon
// (elements, quick layouts, colours, styles, switch row/column, data, type), the three buttons beside a
// chart (Elements, Styles, Filters) and the Change Chart Type dialog with its previews and Combo tab.
// Every change goes through the chart's own `set`, so each is one Undo step.
import { For, JSX, Show, createSignal, onCleanup, onMount } from "solid-js";
import { ChartData, ChartDef, Series, Axis, PALETTE, XChart, colorOf, kindOf } from "./xchart";
import { openDialog, closeDialog, openMenu, Item, toast } from "./store";
import { Dialog } from "./ui";

/** What the tools work on: a chart, its data, how to change it, and how to open its Format pane. */
export type ChartCtl = { c: ChartDef; d?: ChartData; set: (c: ChartDef) => void; format: (part: string) => void; a1?: (r: number, c: number) => string; parse?: (a: string) => [number, number] };

const SAFE = ["#0072B2", "#E69F00", "#009E73", "#D55E00", "#CC79A7", "#56B4E9", "#C9B200", "#5E6A78"];
/** Colour sets for Change Colours: the series take them in turn. */
export const COLOURS: [string, string[]][] = [
  ["Colourful", PALETTE], ["Colour-blind safe", SAFE],
  ["Blues", ["#0B3C7A", "#1F5FAD", "#4A86C8", "#7FB2E5", "#A9CCEE", "#5B7FA6"]],
  ["Oranges", ["#8A3A06", "#C2560F", "#E07B24", "#F0A35E", "#F7C79A", "#B8860B"]],
  ["Greens", ["#155B32", "#2E9D57", "#5DBB7E", "#95D5A9", "#C7E9D1", "#6E8B3D"]],
  ["Greys", ["#1F2937", "#4B5563", "#6B7280", "#9CA3AF", "#C9CED6", "#E5E7EB"]],
  ["Blue and orange", ["#1F5FAD", "#E07B24", "#7FB2E5", "#F0A35E", "#0B3C7A", "#8A3A06"]],
];
const recolour = (c: ChartDef, set: string[]): ChartDef => ({ ...c, series: c.series.map((s, i) => ({ ...s, color: set[i % set.length] })) });
const ax = (a: Axis | undefined, x: Partial<Axis>): Axis => ({ ...(a ?? {}), ...x });
const lines = (c: ChartDef, x: Partial<Series>) => c.series.map(s => ({ ...s, ...x }));

/** Chart styles: a look in one click (the series keep their colours). */
export const STYLES: [string, (c: ChartDef) => ChartDef][] = [
  ["Standard", c => ({ ...c, bg: "", plot: "", font: 0, labels: false, gap: 150, legend: c.legend === "none" ? "bottom" : c.legend, x: ax(c.x, { grid: false }), y: ax(c.y, { grid: true, hide: false }), series: lines(c, { width: 0, marker: "", dash: "", labels: false }) })],
  ["Report", c => ({ ...c, bg: "", plot: "", font: 11, labels: false, gap: 60, legend: "bottom", x: ax(c.x, { grid: false }), y: ax(c.y, { grid: true, hide: false }), series: lines(c, { width: 2.25, marker: "none", labels: false }) })],
  ["Light grid", c => ({ ...c, bg: "", plot: "#F5F8FC", gap: 100, x: ax(c.x, { grid: true }), y: ax(c.y, { grid: true, hide: false }), series: lines(c, { marker: "", labels: false }) })],
  ["Labels, no grid", c => ({ ...c, bg: "", plot: "", labels: true, gap: 80, legend: "top", x: ax(c.x, { grid: false }), y: ax(c.y, { grid: false }), series: lines(c, { labels: false }) })],
  ["Bold", c => ({ ...c, font: 12.5, gap: 50, y: ax(c.y, { grid: true }), series: lines(c, { width: 3.5, marker: "circle" }) })],
  ["Soft", c => ({ ...c, bg: "#FBFBF8", plot: "#F2F4F1", gap: 110, font: 11, y: ax(c.y, { grid: true }), series: lines(c, { smooth: true, marker: "none", width: 2.5 }) })],
  ["Markers", c => ({ ...c, plot: "", gap: 150, x: ax(c.x, { grid: true }), y: ax(c.y, { grid: true }), series: lines(c, { marker: "diamond", width: 1.5, smooth: false }) })],
  ["Minimal", c => ({ ...c, bg: "", plot: "", labels: true, legend: "none", gap: 70, x: ax(c.x, { grid: false }), y: ax(c.y, { grid: false, hide: true }), series: lines(c, { marker: "none" }) })],
];
/** Quick layouts: where the title, legend, labels, axis titles and gridlines go. */
export const LAYOUTS: [string, (c: ChartDef) => ChartDef][] = [
  ["Title, legend on the right", c => ({ ...c, title: c.title || "Chart Title", legend: "right", labels: false, y: ax(c.y, { grid: true }) })],
  ["Title, legend on top, labels", c => ({ ...c, title: c.title || "Chart Title", legend: "top", labels: true, y: ax(c.y, { grid: false, hide: true }) })],
  ["Title, legend below, gridlines", c => ({ ...c, title: c.title || "Chart Title", legend: "bottom", labels: false, y: ax(c.y, { grid: true, hide: false }) })],
  ["No title, labels", c => ({ ...c, title: "", legend: "bottom", labels: true, y: ax(c.y, { grid: true }) })],
  ["Title and axis titles", c => ({ ...c, title: c.title || "Chart Title", legend: "right", x: ax(c.x, { title: c.x?.title || "Axis Title" }), y: ax(c.y, { title: c.y?.title || "Axis Title", grid: true }) })],
  ["Axis titles, no legend", c => ({ ...c, title: c.title || "Chart Title", legend: "none", x: ax(c.x, { title: c.x?.title || "Axis Title" }), y: ax(c.y, { title: c.y?.title || "Axis Title", grid: true }) })],
];

const SAMPLE: ChartData = { cats: ["Q1", "Q2", "Q3", "Q4"], xs: [], cat_fmt: "", names: ["A", "B", "C"], vals: [[4, 6, 5, 8], [3, 4, 6, 5], [2, 3, 2, 4]], fmts: [] };
/** A small picture of a chart as it would look (its own data, or a sample). */
export const Thumb = (p: { c: ChartDef; d?: ChartData; w?: number; h?: number }) => {
  const w = p.w ?? 132, h = p.h ?? 84, has = !!p.d?.vals.some(v => v.some(x => x !== null));
  const c: ChartDef = { ...p.c, w, h, font: Math.min(7, (p.c.font || 11) * 0.62), title: p.c.title ? " " : "", subtitle: undefined, marker: null, ends: false, quiet: true, x: ax(p.c.x, { title: p.c.x?.title ? " " : "" }), y: ax(p.c.y, { title: p.c.y?.title ? " " : "" }) };
  const d = has ? p.d! : { ...SAMPLE, vals: SAMPLE.vals.slice(0, Math.max(1, Math.min(3, p.c.series.length))) };
  if (!has) c.series = d.vals.map((_, i) => p.c.series[i] ?? { name: "", vals: "" });
  return <XChart c={c} d={d} w={w} h={h} hover={false} />;
};

/** A box under a button (or beside a chart) that closes when the pointer goes down outside it. */
export const popover = (at: { x: number; y: number }, body: () => JSX.Element, cls = "") => openDialog(() => {
  let el!: HTMLDivElement;
  const key = (k: KeyboardEvent) => { if (k.key === "Escape") { k.stopPropagation(); closeDialog(); } };
  onMount(() => { document.addEventListener("keydown", key, true); const r = el.getBoundingClientRect(); if (r.right > innerWidth - 4) el.style.left = `${Math.max(4, innerWidth - r.width - 4)}px`; if (r.bottom > innerHeight - 4) el.style.top = `${Math.max(4, innerHeight - r.height - 4)}px`; });
  onCleanup(() => document.removeEventListener("keydown", key, true));
  return <div class="cpov" onMouseDown={e => e.target === e.currentTarget && closeDialog()}><div class={`chpop ${cls}`} ref={el} style={{ left: `${at.x}px`, top: `${at.y}px` }}>{body()}</div></div>;
});
const below = (e: MouseEvent) => { const r = (e.currentTarget as HTMLElement).getBoundingClientRect(); return { x: r.left, y: r.bottom + 2 }; };
const beside = (e: MouseEvent) => { const r = (e.currentTarget as HTMLElement).getBoundingClientRect(); return { x: r.right + 6, y: r.top }; };

// ---------- chart elements ----------
const hasY2 = (c: ChartDef) => c.series.some(s => s.y2);
const round = (c: ChartDef) => c.kind === "pie" || c.kind === "doughnut";
/** Each element: is it shown, turn it on or off, and its options (a submenu). */
const ELEMENTS: { l: string; on: (c: ChartDef) => boolean; flip: (c: ChartDef, on: boolean) => ChartDef; more?: (c: ChartDef, set: (c: ChartDef) => void) => Item[]; axes?: boolean }[] = [
  { l: "Axes", axes: true, on: c => !c.x?.hide || !c.y?.hide, flip: (c, on) => ({ ...c, x: ax(c.x, { hide: !on }), y: ax(c.y, { hide: !on }), y2: ax(c.y2, { hide: !on }) }),
    more: (c, set) => [{ label: "Primary Horizontal", on: !c.x?.hide, run: () => set({ ...c, x: ax(c.x, { hide: !c.x?.hide }) }) }, { label: "Primary Vertical", on: !c.y?.hide, run: () => set({ ...c, y: ax(c.y, { hide: !c.y?.hide }) }) },
      ...(hasY2(c) ? [{ label: "Secondary Vertical", on: !c.y2?.hide, run: () => set({ ...c, y2: ax(c.y2, { hide: !c.y2?.hide }) }) }] : [])] },
  { l: "Axis Titles", axes: true, on: c => !!(c.x?.title || c.y?.title), flip: (c, on) => ({ ...c, x: ax(c.x, { title: on ? c.x?.title || "Axis Title" : "" }), y: ax(c.y, { title: on ? c.y?.title || "Axis Title" : "" }), y2: hasY2(c) ? ax(c.y2, { title: on ? c.y2?.title || "Axis Title" : "" }) : c.y2 }),
    more: (c, set) => [{ label: "Primary Horizontal", on: !!c.x?.title, run: () => set({ ...c, x: ax(c.x, { title: c.x?.title ? "" : "Axis Title" }) }) }, { label: "Primary Vertical", on: !!c.y?.title, run: () => set({ ...c, y: ax(c.y, { title: c.y?.title ? "" : "Axis Title" }) }) },
      ...(hasY2(c) ? [{ label: "Secondary Vertical", on: !!c.y2?.title, run: () => set({ ...c, y2: ax(c.y2, { title: c.y2?.title ? "" : "Axis Title" }) }) }] : [])] },
  { l: "Chart Title", on: c => !!c.title, flip: (c, on) => ({ ...c, title: on ? c.title || "Chart Title" : "" }) },
  { l: "Data Labels", on: c => !!c.labels || c.series.some(s => s.labels), flip: (c, on) => ({ ...c, labels: on, series: c.series.map(s => ({ ...s, labels: false })) }),
    more: (c, set) => c.series.map((s, i) => ({ label: s.name.startsWith("=") ? `Series ${i + 1}` : s.name || `Series ${i + 1}`, on: !!s.labels || !!c.labels, run: () => set({ ...c, labels: false, series: c.series.map((x, j) => ({ ...x, labels: j === i ? !(x.labels || c.labels) : !!(x.labels || c.labels) })) }) })) },
  { l: "Gridlines", axes: true, on: c => !!(c.y?.grid || c.x?.grid), flip: (c, on) => ({ ...c, y: ax(c.y, { grid: on }), x: on ? c.x : ax(c.x, { grid: false }) }),
    more: (c, set) => [{ label: "Primary Major Horizontal", on: !!c.y?.grid, run: () => set({ ...c, y: ax(c.y, { grid: !c.y?.grid }) }) }, { label: "Primary Major Vertical", on: !!c.x?.grid, run: () => set({ ...c, x: ax(c.x, { grid: !c.x?.grid }) }) }] },
  { l: "Legend", on: c => c.legend !== "none", flip: (c, on) => ({ ...c, legend: on ? "bottom" : "none" }),
    more: (c, set) => [["right", "Right"], ["top", "Top"], ["left", "Left"], ["bottom", "Bottom"]].map(([k, l]) => ({ label: l, on: (c.legend || "bottom") === k, run: () => set({ ...c, legend: k }) })) },
  { l: "Trendline", axes: true, on: c => c.series.some(s => s.trend === "linear"), flip: (c, on) => ({ ...c, series: c.series.map(s => ({ ...s, trend: on ? "linear" : "" })) }),
    more: (c, set) => c.series.map((s, i) => ({ label: s.name.startsWith("=") ? `Series ${i + 1}` : s.name || `Series ${i + 1}`, on: s.trend === "linear", run: () => set({ ...c, series: c.series.map((x, j) => (j === i ? { ...x, trend: x.trend === "linear" ? "" : "linear" } : x)) }) })) },
];
/** Add Chart Element (the ribbon's menu): every element with its options. */
export const elementMenu = (e: MouseEvent, t: ChartCtl) => openMenu(e, ELEMENTS.filter(x => !(x.axes && round(t.c))).map(x => ({ label: x.l, sub: [{ label: "None", on: !x.on(t.c), run: () => t.set(x.flip(t.c, false)) }, { label: x.l === "Chart Title" ? "Above Chart" : "Show", on: x.on(t.c), run: () => t.set(x.flip(t.c, true)) }, ...(x.more ? ["-" as const, ...x.more(t.c, t.set)] : [])] })));
/** The series' name as the chart shows it. */
const sname = (t: ChartCtl, i: number) => t.d?.names[i] || (t.c.series[i].name.startsWith("=") ? "" : t.c.series[i].name) || `Series ${i + 1}`;

// ---------- the three buttons beside a chart ----------
/** + : the chart's elements, each ticked on or off, its options under the arrow. */
const ElementsPop = (p: { t: () => ChartCtl | null }) => <div class="chlist"><b>Chart Elements</b>
  <For each={ELEMENTS.filter(x => !(x.axes && round(p.t()!.c)))}>{x => <div class="chrow"><label><input type="checkbox" checked={x.on(p.t()!.c)} onChange={e => p.t()!.set(x.flip(p.t()!.c, e.currentTarget.checked))} />{x.l}</label>
    <Show when={x.more}><button class="more" title="Options" onClick={e => { const t = p.t()!; openMenu(beside(e), x.more!(t.c, t.set)); }}>›</button></Show></div>}</For></div>;
/** Brush: styles drawn as they would look, and colour sets. */
const StylesPop = (p: { t: () => ChartCtl | null }) => {
  const [tab, setTab] = createSignal("style");
  return <div class="chstylepop"><div class="cftabs"><button classList={{ on: tab() === "style" }} onClick={() => setTab("style")}>Style</button><button classList={{ on: tab() === "colour" }} onClick={() => setTab("colour")}>Colour</button></div>
    <Show when={tab() === "style"}><div class="chgal"><For each={STYLES}>{([l, f]) => <button title={l} onClick={() => p.t()!.set(f(p.t()!.c))}><Thumb c={f(p.t()!.c)} d={p.t()!.d} w={120} h={76} /><small>{l}</small></button>}</For></div></Show>
    <Show when={tab() === "colour"}><ColourList t={p.t} /></Show></div>;
};
const ColourList = (p: { t: () => ChartCtl | null }) => <div class="chcols"><For each={COLOURS}>{([l, set]) => <button onClick={() => p.t()!.set(recolour(p.t()!.c, set))}><span>{set.slice(0, 6).map(x => <i style={{ background: x }} />)}</span>{l}</button>}</For></div>;
/** Funnel: which series and which categories the chart shows (the cells stay as they are). */
const FiltersPop = (p: { t: () => ChartCtl | null }) => {
  const t = () => p.t()!, skip = () => new Set(t().c.skip ?? []), cats = () => (t().d?.cats ?? []).slice(0, 400);
  const setSkip = (j: number, on: boolean) => { const s = skip(); if (on) s.delete(j); else s.add(j); t().set({ ...t().c, skip: [...s].sort((a, b) => a - b) }); };
  return <div class="chlist chfilter"><b>Values</b>
    <small>Series</small>
    <For each={t().c.series}>{(s, i) => <label><input type="checkbox" checked={!s.hide} onChange={e => { const on = e.currentTarget.checked; if (!on && t().c.series.filter(x => !x.hide).length < 2) { e.currentTarget.checked = true; toast("A chart shows at least one series.", false); return; } t().set({ ...t().c, series: t().c.series.map((x, j) => (j === i() ? { ...x, hide: !on } : x)) }); }} /><i class="cfsw" style={{ background: colorOf(t().c, i()) }} />{sname(t(), i())}</label>}</For>
    <Show when={cats().length}><small>Categories <button class="link" onClick={() => t().set({ ...t().c, skip: [] })}>Select all</button></small>
      <div class="chcats"><For each={cats()}>{(cat, j) => <label><input type="checkbox" checked={!skip().has(j())} onChange={e => setSkip(j(), e.currentTarget.checked)} />{cat || `(${j() + 1})`}</label>}</For></div></Show></div>;
};
/** The three buttons beside a selected chart on a sheet. */
export const SideButtons = (p: { t: () => ChartCtl | null }) => (
  <div class="ch-side" onMouseDown={e => e.stopPropagation()}>
    <button title="Chart Elements: title, axes, labels, gridlines, legend, trendline" onClick={e => popover(beside(e), () => <ElementsPop t={p.t} />)}>+</button>
    <button title="Chart Styles: looks and colours" onClick={e => popover(beside(e), () => <StylesPop t={p.t} />)}><svg viewBox="0 0 16 16"><path d="M10.5 2.5l3 3-6 6-3-3zM4 10l2 2c0 1.6-1.4 2.5-3.5 2.2.6-.6.6-1.3.6-2C3.1 11 3.5 10.4 4 10z" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linejoin="round" /></svg></button>
    <button title="Chart Filters: which series and categories the chart shows" onClick={e => popover(beside(e), () => <FiltersPop t={p.t} />)}><svg viewBox="0 0 16 16"><path d="M2 3h12l-4.6 5.4V13l-2.8 1V8.4z" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linejoin="round" /></svg></button>
  </div>
);

// ---------- the ribbon's gallery pieces ----------
export const quickLayout = (e: MouseEvent, t: ChartCtl) => popover(below(e), () => <div class="chgal"><For each={LAYOUTS}>{([l, f], i) => <button title={l} onClick={() => { closeDialog(); t.set(f(t.c)); }}><Thumb c={f(t.c)} d={t.d} w={110} h={72} /><small>Layout {i() + 1}</small></button>}</For></div>);
export const changeColours = (e: MouseEvent, t: () => ChartCtl | null) => popover(below(e), () => <ColourList t={t} />);
export const moreStyles = (e: MouseEvent, t: () => ChartCtl | null) => popover(below(e), () => <div class="chgal wide"><For each={STYLES}>{([l, f]) => <button title={l} onClick={() => { closeDialog(); t()!.set(f(t()!.c)); }}><Thumb c={f(t()!.c)} d={t()!.d} w={120} h={76} /><small>{l}</small></button>}</For></div>);

// ---------- switch row / column ----------
type Rng = { sh: string; r0: number; c0: number; r1: number; c1: number };
/** Each series becomes a category and each category a series: the series must read one row (or one
 *  column) each, side by side, over the same cells. Null when the chart's ranges are not laid out so. */
export const switchRowCol = (c: ChartDef, a1: (r: number, c: number) => string, parse: (a: string) => [number, number]): ChartDef | null => {
  const rng = (s: string): Rng | null => {
    const m = /^(?:(.+)!)?\$?([A-Z]+)\$?(\d+)(?::\$?([A-Z]+)\$?(\d+))?$/i.exec(s.trim()); if (!m) return null;
    const [r0, c0] = parse(`${m[2]}${m[3]}`), [r1, c1] = parse(m[4] ? `${m[4]}${m[5]}` : `${m[2]}${m[3]}`);
    return r0 < 0 || c0 < 0 ? null : { sh: m[1] ? `${m[1]}!` : "", r0: Math.min(r0, r1), c0: Math.min(c0, c1), r1: Math.max(r0, r1), c1: Math.max(c0, c1) };
  };
  const vs = c.series.map(s => rng(s.vals));
  if (!vs.length || vs.some(v => !v)) return null;
  const v = vs as Rng[], byCol = v.every(x => x.c0 === x.c1 && x.r0 === v[0].r0 && x.r1 === v[0].r1 && x.sh === v[0].sh), byRow = v.every(x => x.r0 === x.r1 && x.c0 === v[0].c0 && x.c1 === v[0].c1 && x.sh === v[0].sh);
  if (!byCol && !byRow) return null;
  const at = v.map(x => (byCol ? x.c0 : x.r0)), lo = Math.min(...at), hi = Math.max(...at);
  if (hi - lo + 1 !== at.length || new Set(at).size !== at.length) return null;
  const sh = v[0].sh, A = (r: number, col: number) => `${sh}${a1(r, col)}`;
  // The categories' cells beside the block, and the series names' cells across it (each optional).
  const cats = rng(c.cats), nameAt = c.series.map(s => (s.name.startsWith("=") ? rng(s.name.slice(1)) : null));
  const namesLine = nameAt.every(n => n && (byCol ? n.r0 === nameAt[0]!.r0 : n.c0 === nameAt[0]!.c0)) ? nameAt[0] : null;
  const n = byCol ? v[0].r1 - v[0].r0 + 1 : v[0].c1 - v[0].c0 + 1;
  if (n > 40) return null;
  const series: Series[] = Array.from({ length: n }, (_, k) => {
    const old = c.series[k];
    const vals = byCol ? `${A(v[0].r0 + k, lo)}:${A(v[0].r0 + k, hi)}` : `${A(lo, v[0].c0 + k)}:${A(hi, v[0].c0 + k)}`;
    const name = cats ? `=${byCol ? A(cats.r0 + k, cats.c0) : A(cats.r0, cats.c0 + k)}` : `Series ${k + 1}`;
    return { ...(old ?? {}), name, vals, hide: false, color: old?.color || PALETTE[k % PALETTE.length] };
  });
  const newCats = namesLine ? (byCol ? `${A(namesLine.r0, lo)}:${A(namesLine.r0, hi)}` : `${A(lo, namesLine.c0)}:${A(hi, namesLine.c0)}`) : "";
  return { ...c, cats: newCats, series, skip: [] };
};

// ---------- Change Chart Type ----------
const TYPES: [string, string, [string, string, (c: ChartDef) => ChartDef][]][] = [
  ["column", "Column", [["Clustered Column", "", c => ({ ...c, kind: "column", stack: "" })], ["Stacked Column", "", c => ({ ...c, kind: "column", stack: "stacked" })], ["100% Stacked Column", "", c => ({ ...c, kind: "column", stack: "percent" })]]],
  ["bar", "Bar", [["Clustered Bar", "", c => ({ ...c, kind: "bar", stack: "" })], ["Stacked Bar", "", c => ({ ...c, kind: "bar", stack: "stacked" })], ["100% Stacked Bar", "", c => ({ ...c, kind: "bar", stack: "percent" })]]],
  ["line", "Line", [["Line", "", c => ({ ...c, kind: "line", stack: "", series: lines(c, { marker: "none" }) })], ["Line with Markers", "", c => ({ ...c, kind: "line", stack: "", series: lines(c, { marker: "circle" }) })], ["Stacked Line", "", c => ({ ...c, kind: "line", stack: "stacked" })], ["Smooth Line", "", c => ({ ...c, kind: "line", stack: "", series: lines(c, { smooth: true, marker: "none" }) })]]],
  ["area", "Area", [["Area", "", c => ({ ...c, kind: "area", stack: "" })], ["Stacked Area", "", c => ({ ...c, kind: "area", stack: "stacked" })], ["100% Stacked Area", "", c => ({ ...c, kind: "area", stack: "percent" })]]],
  ["scatter", "X Y (Scatter)", [["Scatter", "", c => ({ ...c, kind: "scatter", stack: "", series: lines(c, { marker: "circle", width: 0 }) })], ["Scatter with Lines", "", c => ({ ...c, kind: "scatter", stack: "", series: lines(c, { marker: "circle", width: 1.75 }) })], ["Scatter with Smooth Lines", "", c => ({ ...c, kind: "scatter", stack: "", series: lines(c, { marker: "none", width: 2, smooth: true }) })]]],
  ["pie", "Pie", [["Pie", "", c => ({ ...c, kind: "pie", stack: "" })], ["Doughnut", "", c => ({ ...c, kind: "doughnut", stack: "" })]]],
];
const comboOk = (c: ChartDef) => ["column", "line", "area"].includes(c.kind);
/** The Change Chart Type dialog: types on the left, their variants drawn with the chart's own data, a
 *  large preview, and a Combo tab that sets each series' type and axis. */
export const changeType = (t: ChartCtl) => openDialog(() => {
  const start = t.c;
  const fam = (c: ChartDef) => (c.kind === "doughnut" ? "pie" : comboOk(c) && c.series.some((s, i) => s.y2 || kindOf(c, i) !== c.kind) ? "combo" : c.kind);
  const [cur, setCur] = createSignal<ChartDef>(start), [ty, setTy] = createSignal(fam(start));
  const variants = () => TYPES.find(x => x[0] === ty())?.[2] ?? [];
  const pick = (k: string) => { setTy(k); if (k === "combo") { const c = comboOk(cur()) ? cur() : { ...cur(), kind: "column", stack: "" }; setCur(c.series.length > 1 && !c.series.some(s => s.y2 || (s.kind && s.kind !== c.kind)) ? { ...c, series: c.series.map((s, i) => (i === c.series.length - 1 ? { ...s, kind: "line", y2: true } : { ...s, kind: "column" })) } : c); } else setCur(variants()[0][2]({ ...cur(), series: cur().series.map(s => ({ ...s, kind: "", y2: false })) })); };
  const setS = (i: number, x: Partial<Series>) => setCur({ ...cur(), series: cur().series.map((s, j) => (j === i ? { ...s, ...x } : s)) });
  return <Dialog title="Change Chart Type" wide ok={() => { t.set(cur()); }}>
    <div class="cttype">
      <nav><For each={[...TYPES.map(x => [x[0], x[1]]), ["combo", "Combo"]]}>{([k, l]) => <button classList={{ on: ty() === k }} onClick={() => pick(k)}>{l}</button>}</For></nav>
      <div class="ctmain">
        <Show when={ty() !== "combo"} fallback={<>
          <p class="muted">Choose how each series is drawn and which axis it is measured on.</p>
          <table class="cttab"><thead><tr><th>Series</th><th>Chart type</th><th>Secondary axis</th></tr></thead><tbody>
            <For each={cur().series}>{(s, i) => <tr><td><i class="cfsw" style={{ background: colorOf(cur(), i()) }} />{sname({ ...t, c: cur() }, i())}</td>
              <td><select value={s.kind || cur().kind} onChange={e => setS(i(), { kind: e.currentTarget.value })}><option value="column">Clustered Column</option><option value="line">Line</option><option value="area">Area</option></select></td>
              <td><input type="checkbox" checked={!!s.y2} onChange={e => setS(i(), { y2: e.currentTarget.checked })} /></td></tr>}</For></tbody></table>
          <div class="inline"><button class="btn sm" onClick={() => setCur({ ...cur(), kind: "column", stack: "", series: cur().series.map((s, i) => ({ ...s, kind: i === cur().series.length - 1 ? "line" : "column", y2: false })) })}>Columns and a line</button>
            <button class="btn sm" onClick={() => setCur({ ...cur(), kind: "column", stack: "", series: cur().series.map((s, i) => ({ ...s, kind: i === cur().series.length - 1 ? "line" : "column", y2: i === cur().series.length - 1 })) })}>Columns and a line on the secondary axis</button>
            <button class="btn sm" onClick={() => setCur({ ...cur(), kind: "column", stack: "stacked", series: cur().series.map((s, i) => ({ ...s, kind: i === cur().series.length - 1 ? "area" : "column", y2: false })) })}>Stacked columns and an area</button></div></>}>
          <div class="ctvars"><For each={variants()}>{([l, , f]) => { const c = () => f(cur()); return <button classList={{ on: c().kind === cur().kind && (c().stack ?? "") === (cur().stack ?? "") && JSON.stringify(c().series.map(s => [s.marker, s.smooth, s.width])) === JSON.stringify(cur().series.map(s => [s.marker, s.smooth, s.width])) }} title={l} onClick={() => setCur(c())}><Thumb c={c()} d={t.d} w={112} h={72} /><small>{l}</small></button>; }}</For></div>
        </Show>
        <div class="ctprev">{(() => <Thumb c={{ ...cur(), font: 11 }} d={t.d} w={540} h={290} />) as unknown as JSX.Element}</div>
      </div>
    </div>
  </Dialog>;
});
