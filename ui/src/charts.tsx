// Interactive charts in a GraphPad Prism style: clean black axes, outward ticks, no clutter,
// colour-blind-safe colours, hover crosshair with every value, and save as PNG.
import { onMount, onCleanup, createEffect, on, JSX, Show } from "solid-js";
import { Chart, registerables, type ChartDataset, type Plugin } from "chart.js";
import { call, toast, themeTick, P } from "./store";
import { money } from "./api";

Chart.register(...registerables);

/** Okabe-Ito colours, as Prism's colour-blind-safe scheme. */
export const COL = { blue: "#0072B2", orange: "#E69F00", green: "#009E73", red: "#D55E00", purple: "#CC79A7", sky: "#56B4E9", yellow: "#F0E442", grey: "#8C8C8C", black: "#1A1A1A" };

const cssv = (n: string) => getComputedStyle(document.documentElement).getPropertyValue(n).trim();

/** A vertical line on hover across the plot, and an optional dashed marker (status date). */
const lines = (marker: () => number | null | undefined, label: string): Plugin => ({
  id: "lines",
  afterDatasetsDraw(ch) {
    const { ctx, chartArea: a, scales } = ch;
    const x = scales.x;
    const m = marker();
    ctx.save();
    if (m !== null && m !== undefined && m >= 0) {
      const px = x.getPixelForValue(m);
      ctx.strokeStyle = COL.green; ctx.setLineDash([5, 4]); ctx.lineWidth = 1.2;
      ctx.beginPath(); ctx.moveTo(px, a.top); ctx.lineTo(px, a.bottom); ctx.stroke();
      ctx.setLineDash([]); ctx.fillStyle = COL.green; ctx.font = "500 10px 'IBM Plex Mono', monospace"; ctx.fillText(label, px + 4, a.top + 10);
    }
    const act = ch.tooltip?.getActiveElements?.() ?? [];
    if (act.length) {
      const px = act[0].element.x;
      ctx.strokeStyle = cssv("--muted"); ctx.lineWidth = 0.8;
      ctx.beginPath(); ctx.moveTo(px, a.top); ctx.lineTo(px, a.bottom); ctx.stroke();
    }
    ctx.restore();
  },
});
/** White (or dark) background so saved pictures are not transparent. */
const paper: Plugin = { id: "paper", beforeDraw(ch) { const c = ch.ctx; c.save(); c.globalCompositeOperation = "destination-over"; c.fillStyle = cssv("--bg2") || "#fff"; c.fillRect(0, 0, ch.width, ch.height); c.restore(); } };

export type Fmt = "pct" | "money" | "hours" | "num";
export const fmtv = (v: number, f: Fmt = "num") => f === "pct" ? `${v.toFixed(1)}%` : f === "money" ? money(v, { ...P().currency, decimals: 0 }) : f === "hours" ? `${Math.round(v).toLocaleString()} h` : (Math.round(v * 100) / 100).toLocaleString();

export interface ChartProps {
  title: string;
  type: "line" | "bar";
  labels: string[];
  datasets: ChartDataset<"line" | "bar", (number | null)[]>[];
  y?: string;
  x?: string;
  fmt?: Fmt;
  max?: number;
  stacked?: boolean;
  horizontal?: boolean;
  marker?: number | null;
  markerLabel?: string;
  size?: "tall" | "short";
  extra?: JSX.Element;
}

export const ChartBox = (p: ChartProps) => {
  let cv!: HTMLCanvasElement;
  let ch: Chart | undefined;
  const build = () => {
    ch?.destroy();
    const ink = cssv("--text"), muted = cssv("--muted"), grid = cssv("--line2");
    const font = { family: "'IBM Plex Sans', 'Segoe UI', sans-serif", size: 11 };
    const axis = (title?: string, isY = false) => ({
      title: { display: !!title, text: title, color: ink, font: { ...font, size: 11.5, weight: 600 as const } },
      border: { color: ink, width: 1.2 },
      grid: { display: isY, color: grid, drawTicks: true, tickColor: ink, tickLength: 5, drawOnChartArea: isY },
      ticks: { color: ink, font, maxRotation: 0, autoSkipPadding: 14, ...(isY ? { callback: (v: string | number) => fmtv(+v, p.fmt) } : {}) },
      stacked: p.stacked,
    });
    const vAxis = { ...axis(p.y, true), beginAtZero: true, ...(p.max !== undefined ? { max: p.max } : {}) };
    const hAxis = axis(p.x, false);
    // A horizontal bar chart names every bar (task names), so none may be skipped.
    if (p.horizontal) hAxis.ticks = { ...hAxis.ticks, autoSkip: false } as typeof hAxis.ticks;
    ch = new Chart(cv, {
      type: p.type,
      data: { labels: p.labels, datasets: p.datasets as ChartDataset<"line" | "bar", (number | null)[]>[] },
      options: {
        responsive: true, maintainAspectRatio: false, animation: { duration: 250 },
        indexAxis: p.horizontal ? "y" : "x",
        interaction: { mode: "index", intersect: false },
        layout: { padding: { top: 4, right: 10 } },
        elements: { line: { borderWidth: 2, tension: 0.25 }, point: { radius: p.labels.length > 40 ? 0 : 2.5, hoverRadius: 5, hitRadius: 8 }, bar: { borderWidth: 0 } },
        scales: (p.horizontal ? { x: vAxis, y: hAxis } : { x: hAxis, y: vAxis }) as never,
        plugins: {
          legend: { position: "top", align: "end", labels: { color: ink, font, boxWidth: 14, boxHeight: 3, padding: 12 } },
          tooltip: {
            backgroundColor: cssv("--bg2"), titleColor: ink, bodyColor: ink, borderColor: cssv("--line3"), borderWidth: 1, cornerRadius: 0, padding: 8,
            titleFont: { ...font, weight: 600 as const }, bodyFont: font,
            callbacks: { label: c => ` ${c.dataset.label}: ${c.parsed[p.horizontal ? "x" : "y"] === null ? "—" : fmtv(c.parsed[p.horizontal ? "x" : "y"] as number, p.fmt)}` },
          },
        },
      },
      plugins: [paper, lines(() => p.marker, p.markerLabel ?? "Status")],
    });
    ch.options.color = muted;
  };
  onMount(build);
  onCleanup(() => ch?.destroy());
  createEffect(on(() => [p.labels, p.datasets, themeTick(), p.marker], () => build(), { defer: true }));
  const save = async () => {
    if (!ch) return;
    const path = await call<string | null>("save_file", { name: `${p.title}.png`, data: ch.toBase64Image("image/png", 1) });
    if (path) toast(`Saved ${path}`, true);
  };
  return (
    <div class="chartbox" classList={{ tall: p.size === "tall", short: p.size === "short" }}>
      <header><h3>{p.title}</h3>{p.extra}<button onClick={save} title="Save as PNG">PNG</button></header>
      <div class="cv"><canvas ref={cv} /></div>
      <Show when={!p.labels.length}><p class="empty">No data yet.</p></Show>
    </div>
  );
};

/** Line dataset helpers. */
export const line = (label: string, data: (number | null)[], color: string, dash?: number[], fill = false): ChartDataset<"line", (number | null)[]> =>
  ({ type: "line", label, data, borderColor: color, backgroundColor: fill ? color + "22" : color, borderDash: dash, fill, spanGaps: false, pointBackgroundColor: color });
export const bar = (label: string, data: (number | null)[], color: string, stack?: string): ChartDataset<"bar", (number | null)[]> =>
  ({ type: "bar", label, data, backgroundColor: color, stack, maxBarThickness: 26 });
