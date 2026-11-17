// The charts of the views and dashboards, drawn by the same vector engine as the charts on sheets
// (xchart.tsx): sharp on screen, in print and in PDFs, with the same Format pane. Each chart keeps
// its look per plan (ui.chartLooks, by title), saved from "Format chart"; Copy and PNG give a
// page-wide picture with the title, Datum Plot a one-way copy of the values.
import { createEffect, createSignal, onCleanup, onMount, JSX, Show } from "solid-js";
import { Portal } from "solid-js/web";
import { call, toast, P, act, backstage } from "./store";
import { money, fmtDay } from "./api";
import { ChartData, ChartDef, Series, XChart, chartCanvas, lookOf, withLook } from "./xchart";
import { ChartPane, chartMenu } from "./sheetcharts";

/** The chart colours: clear and easy to tell apart. With Options › Colour-blind safe colours they
 * become the Okabe-Ito set, told apart with every common kind of colour-blindness. */
const STD = { blue: "#2F6FCF", orange: "#E07B24", green: "#2E9D57", red: "#C0392B", purple: "#6D3FC8", sky: "#7FB2E5", yellow: "#D4A017", grey: "#9AA3AF", black: "#1F2937" };
const SAFE = { blue: "#0072B2", orange: "#E69F00", green: "#009E73", red: "#D55E00", purple: "#CC79A7", sky: "#56B4E9", yellow: "#C9B200", grey: "#A3ABB8", black: "#1F2937" };
export const COL = { ...STD };
/** Switches the chart and status colours (the page's `data-cb`) to the colour-blind safe set or back. */
export const setColourSafe = (on: boolean) => { Object.assign(COL, on ? SAFE : STD); document.documentElement.dataset.cb = on ? "1" : "0"; };

export type Fmt = "pct" | "money" | "hours" | "num" | "date";
export const fmtv = (v: number, f: Fmt = "num") => f === "date" ? fmtDay(v) : f === "pct" ? `${v.toFixed(1)}%` : f === "money" ? money(v, { ...P().currency, decimals: 0 }) : f === "hours" ? `${Math.round(v).toLocaleString()} h` : (Math.round(v * 100) / 100).toLocaleString();
const FMT: Record<Fmt, string> = { pct: "@pct", money: "@money", hours: "@hours", num: "@num", date: "@day" };

/** One series of a view's chart (the shape the views build with `line` and `bar`). */
export type DS = { type?: "line" | "bar"; label?: string; data: unknown[]; borderColor?: string; backgroundColor?: string; borderDash?: number[]; borderWidth?: number; fill?: boolean | string; stack?: string; yAxisID?: string; order?: number; tension?: number; cubicInterpolationMode?: string };
/** A point's value: a number, or the y of an {x, y} point on a time axis. */
const yv = (v: unknown): number | null => (v !== null && typeof v === "object" ? (v as { y: number | null }).y : (v as number | null) ?? null);

export interface ChartProps {
  title: string;
  type: "line" | "bar" | "doughnut" | "pie";
  labels: string[];
  datasets: DS[];
  y?: string;
  x?: string;
  fmt?: Fmt;
  max?: number;
  stacked?: boolean;
  horizontal?: boolean;
  marker?: number | null;
  markerLabel?: string;
  /** "fill": as tall as its box (dashboard tiles). */
  size?: "tall" | "short" | "fill";
  /** No title bar (a dashboard tile has its own). */
  bare?: boolean;
  /** The day (since 1970) of each label, for a time axis in Datum Plot. */
  days?: number[];
  extra?: JSX.Element;
  /** A time axis: x is the day of each point ({x, y} data), ticks at `ticksAt`, named by `tickLabel`. */
  time?: { min: number; max: number; ticksAt: number[]; tickLabel: (i: number) => string; rotate?: boolean };
  /** A second value axis on the right (datasets with yAxisID "y2"): its title and top. */
  y2?: string;
  y2max?: number;
  /** The status line's text when the axis is a time axis. */
  markerText?: string;
  legend?: "top" | "bottom";
  /** The last value at the end of each line (default on). */
  ends?: boolean;
  /** A line under the title (data date and progress), and the title centred, as in a report. */
  subtitle?: string;
  center?: boolean;
  /** Bar corners (px) and line smoothing (0 straight). */
  radius?: number;
  tension?: number;
}

/** The chart a view describes, as the vector engine draws it (with the plan's saved look over it). */
const toChart = (p: ChartProps, w: number, h: number, doc = false): { c: ChartDef; d: ChartData } => {
  const round = p.type === "doughnut" || p.type === "pie", isLine = (d: DS) => d.type === "line" || (!d.type && p.type === "line");
  const fmt = FMT[p.fmt ?? "num"], pal = Object.values(COL).filter(c => c !== COL.black);
  const series: Series[] = p.datasets.map(ds => ({
    name: ds.label ?? "", vals: "", kind: round ? "" : isLine(ds) ? "line" : "column", y2: ds.yAxisID === "y2",
    color: (isLine(ds) ? ds.borderColor : ds.backgroundColor) || ds.borderColor || "", width: isLine(ds) ? (ds.borderWidth ?? 2.5) * (doc ? 1.15 : 1) : 0,
    dash: ds.borderDash?.length ? "dash" : "", marker: "none", smooth: (ds.tension ?? p.tension) !== 0, fill: !!ds.fill, colors: round ? p.labels.map((_, i) => pal[i % pal.length]) : undefined,
  }));
  const hasLine = p.datasets.some(isLine), mk = p.marker;
  const c0: ChartDef = {
    id: 0, cell: "", w, h, kind: round ? p.type : p.horizontal ? "bar" : p.type === "line" && p.datasets.every(isLine) ? "line" : "column", stack: p.stacked ? "stacked" : "", cats: "", series,
    title: doc ? p.title : "", subtitle: doc ? p.subtitle : undefined, legend: round ? "right" : p.legend ?? "top", font: doc ? 13 : 11, gap: p.horizontal ? 45 : 60, radius: p.radius ?? 2,
    x: { title: p.x }, y: { title: p.y, fmt, max: p.max ?? null, grid: true }, y2: p.y2 ? { title: p.y2, fmt, min: 0, max: p.y2max ?? null } : {},
    ends: p.ends !== false && hasLine && !p.horizontal, time: p.time, quiet: true,
    marker: mk !== null && mk !== undefined && mk >= 0 ? { at: mk, label: p.markerText ?? `${p.markerLabel ?? "Status"}${!p.time && p.labels[mk] ? ` · ${p.labels[mk]}` : ""}` } : null,
  };
  const looks = (P()?.ui as { chartLooks?: Record<string, Partial<ChartDef>> } | undefined)?.chartLooks;
  const c = withLook(c0, looks?.[p.title], true);
  const d: ChartData = {
    cats: p.labels, xs: p.days ?? [], cat_fmt: "", names: p.datasets.map(x => x.label ?? ""), fmts: [],
    vals: p.datasets.map(ds => ds.data.map(yv)),
    sx: p.time ? p.datasets.map(ds => ds.data.map(q => (q && typeof q === "object" ? (q as { x: number }).x : null))) : undefined,
    tips: p.time ? p.datasets.map(ds => ds.data.map(q => (q && typeof q === "object" ? (q as { t?: string }).t : undefined))) : undefined,
  };
  return { c, d };
};

/** The chart whose Format pane is open (its title), and its props; and the part it formats. */
const [formatting, setFormatting] = createSignal<ChartProps | null>(null);
const [fmtPart, setFmtPart] = createSignal("");
const looks = () => ((P()?.ui as { chartLooks?: Record<string, Partial<ChartDef>> } | undefined)?.chartLooks ?? {});
const saveLook = (title: string, nc: ChartDef | null) => { const l = { ...looks() }; if (nc) l[title] = lookOf(nc); else delete l[title]; return act({ Ui: { key: "chartLooks", value: l } }); };
const partOf = (e: Event) => ((e.target as Element).closest?.("[data-part]")?.getAttribute("data-part") ?? "area");

export const ChartBox = (p: ChartProps) => {
  let box!: HTMLDivElement;
  const [size, setSize] = createSignal<[number, number]>([600, 300]);
  /** Nothing to draw: no periods, or every value empty or zero (no costs, no hours yet). */
  const empty = () => !p.labels.length && !p.time || p.datasets.every(d => d.data.every(v => !yv(v)));
  const ro = new ResizeObserver(() => { const r = box.getBoundingClientRect(); if (r.width > 0 && r.height > 0) setSize([Math.floor(r.width), Math.floor(r.height)]); });
  onMount(() => { ro.observe(box); const r = box.getBoundingClientRect(); if (r.width > 0) setSize([Math.floor(r.width), Math.floor(Math.max(r.height, 200))]); });
  onCleanup(() => ro.disconnect());
  /** The chart drawn for a document: page-wide (960 × 470 px plus its title) at three times the sharpness, light colours. */
  const picture = async (): Promise<HTMLCanvasElement | null> => {
    const W = 960, H = (p.type === "doughnut" || p.type === "pie" ? 420 : 470) + (p.subtitle ? 62 : 40);
    const { c, d } = toChart(p, W, H, true);
    return chartCanvas({ ...c, w: W, h: H }, d, 3).catch(() => null);
  };
  const save = async () => {
    const pic = await picture();
    if (!pic) return;
    const { pngAt } = await import("./picture");
    const path = await call<string | null>("save_file", { name: `${p.title}.png`, data: pngAt(pic, 16) });
    if (path) toast(`Saved ${path}`, true);
  };
  /** A one-way copy of the values in Datum Plot: changes there never come back. */
  const toPlot = async () => {
    const units = p.fmt === "pct" ? "%" : p.fmt === "money" ? P().currency.code : p.fmt === "hours" ? "h" : "";
    // On a time axis every series is put on the chart's days: a point at a day, a period's bar at the day it ends.
    const ds0 = p.days ?? [];
    const onDays = (d: unknown[]) => ds0.map((day, j) => { const pt = (d as { x: number; y: number | null }[]).find(q => q.x <= day && (j === 0 ? q.x >= day : q.x > ds0[j - 1])); return pt ? pt.y : null; });
    const series = p.datasets.map(d => ({ name: String(d.label ?? ""), values: (p.time ? onDays(d.data) : d.data.map(yv)).map(v => (v === null || v === undefined ? null : +v)), color: String(d.borderColor && (d.type === "line" || p.type === "line") ? d.borderColor : d.backgroundColor ?? d.borderColor ?? ""), line: d.type === "line" || p.type === "line" }));
    const msg = await call<string>("open_in_plot", { chart: { title: p.title, y: p.y ?? "", units, labels: p.time ? ds0.map(d => fmtDay(d)) : p.labels, days: ds0, series } });
    if (msg) toast(msg, true);
  };
  const docChart = () => { const W = 960, H = (p.type === "doughnut" || p.type === "pie" ? 420 : 470) + (p.subtitle ? 62 : 40); const r = toChart(p, W, H, true); return { c: { ...r.c, w: W, h: H }, d: r.d }; };
  const menu = (e: MouseEvent) => {
    e.preventDefault(); e.stopPropagation();
    const [w, h] = size(), { c, d } = toChart(p, w, h), part = partOf(e), pic = docChart();
    chartMenu(e, { c, d, part, set: nc => saveLook(p.title, nc), format: pt => { setFmtPart(pt === "area" ? "" : pt); setFormatting(p); }, reset: () => saveLook(p.title, null),
      copy: async () => { const cv = await picture(); if (cv) (await import("./picture")).copyCanvas(cv, p.title, 16); }, pic: { ...pic, name: p.title },
      extra: [{ label: "Open in Datum Plot", icon: "chart", run: toPlot }] });
  };
  return (
    <div class="chartbox" data-marker={p.marker ?? ""} classList={{ tall: p.size === "tall", short: p.size === "short", grow: p.size === "fill", bare: p.bare }}>
      <Show when={!p.bare}><header classList={{ center: p.center }}><h3>{p.title}<Show when={p.subtitle}><small>{p.subtitle}</small></Show></h3>{p.extra}
        <button onClick={() => { setFmtPart(""); setFormatting(p); }} title="Format this chart: type, colours, lines, markers, labels, axes and legend (kept with the plan)">Format chart</button>
        <button onClick={async () => { const pic = await picture(); if (pic) (await import("./picture")).copyCanvas(pic, p.title, 16); }} title="Copy the chart as a picture, page-wide and sharp, to paste into a document, slides or a workbook">Copy</button><button onClick={save} title="Save as PNG">PNG</button><button onClick={toPlot} title="Open a copy of these values in Datum Plot, to style the graph there. The plan does not change.">Datum Plot</button></header></Show>
      <div class="cv" ref={box} classList={{ blank: empty() }} onContextMenu={menu} onDblClick={e => { const pt = partOf(e); setFmtPart(pt === "area" ? "" : pt); setFormatting(p); }}>{(() => { const [w, h] = size(); const { c, d } = toChart(p, w, h); return <XChart c={c} d={d} w={w} h={h} />; }) as unknown as JSX.Element}</div>
      <Show when={empty()}><p class="nodata">{p.fmt === "money" ? "No costs in this plan yet." : p.fmt === "hours" ? "No hours assigned yet." : "No data yet."}</p></Show>
    </div>
  );
};

/** The Format pane of a view's chart: its look is saved with the plan, by the chart's title. */
export const ChartFormat = () => (void createEffect(() => { if (backstage()) setFormatting(null); }), <Show when={formatting()}>{q => <Portal><FormatHost p={q()} /></Portal>}</Show>);
const FormatHost = (x: { p: ChartProps }) => {
  const cur = () => toChart(x.p, 600, 300);
  return <div class="chartpane-fixed"><ChartPane chart={cur().c} data={cur().d} set={nc => saveLook(x.p.title, nc)} close={() => setFormatting(null)} view reset={() => saveLook(x.p.title, null)} part={fmtPart()} /></div>;
};

/** Line dataset helpers: `fill` adds a soft fill under the line. */
export const line = (label: string, data: (number | null)[], color: string, dash?: number[], fill = false): DS =>
  ({ type: "line", label, data, borderColor: color, backgroundColor: color, borderDash: dash, borderWidth: dash ? 2 : 2.5, fill });
export const bar = (label: string, data: (number | null)[], color: string, stack?: string): DS =>
  ({ type: "bar", label, data, backgroundColor: color, borderColor: color, stack });
