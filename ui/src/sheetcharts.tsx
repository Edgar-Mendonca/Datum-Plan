// Charts on a sheet: drawn over the cells at their anchor cell, moved by dragging, resized from
// their edges and corners, and formatted in the Format Chart pane (type, data, series, axes, looks,
// saved styles). A click selects a part (title, legend, an axis, a series, the plot); double-click
// formats it; right-click gives the chart's menu; the title is edited in place. Each change is one
// Undo step (Op::SheetCharts).
import { For, Index, JSX, Show, createEffect, createSignal, on, onCleanup } from "solid-js";
import { ChartData, ChartDef, Series, Axis, KINDS, PALETTE, XChart, colorOf, kindOf, lookOf, withLook, partName, chartSvg, chartCanvas } from "./xchart";
import { opts, setOpts, call, toast, openMenu, Item } from "./store";
import { Options } from "./api";
import { askText, Icon } from "./ui";
import { sheetK, setSheetK } from "./sheetlayout";
import { ChartCtl, SideButtons, changeType, selectData, moveChart } from "./charttools";

export const [chartSel, setChartSel] = createSignal(-1);
export const [fmtChart, setFmtChart] = createSignal<number | null>(null);
/** The selected part of the selected chart ("" = the whole chart). */
export const [chartPart, setChartPart] = createSignal("");
const [editTitle, setEditTitle] = createSignal<number | null>(null);
/** The selected chart on the sheet, for the Chart Design tab and the buttons beside it. */
export const [chartCtl, setChartCtl] = createSignal<ChartCtl | null>(null);
const partOf = (e: Event) => ((e.target as Element).closest?.("[data-part]")?.getAttribute("data-part") ?? "area");
const jOf = (e: Event) => (e.target as Element).closest?.("[data-j]")?.getAttribute("data-j") ?? null;
/** The series a part belongs to (a series "s2" or one of its points "p2:5"), or -1. */
export const seriesOf = (part: string) => (part.startsWith("s") ? +part.slice(1) : part.startsWith("p") ? +part.slice(1).split(":")[0] : -1);

/** A chart's look back to the standard one: palette colours, plain lines, no labels, white areas. */
export const plainLook = (c: ChartDef): ChartDef => ({ ...c, labels: false, font: 0, bg: "", plot: "", gap: 150,
  series: c.series.map((s, i) => ({ ...s, color: PALETTE[i % PALETTE.length], width: 0, dash: "", marker: "", smooth: false, labels: false, trend: "", pts: [], lpos: "", lwhat: "" })) });
/** A series' bars or points with their own colour: one set ("" back to the series' colour). */
export const withPoint = (c: ChartDef, part: string, color: string): ChartDef => {
  const [i, j] = part.slice(1).split(":").map(Number);
  return { ...c, series: c.series.map((s, k) => (k === i ? { ...s, pts: [...(s.pts ?? []).filter(q => q.i !== j), ...(color ? [{ i: j, color }] : [])].sort((a, b) => a.i - b.i) } : s)) };
};

/** Saves a chart as a picture: PNG (sharp, for documents) or SVG (vector, for drawings and the web). */
export const savePicture = async (c: ChartDef, d: ChartData | undefined, name: string, svg: boolean) => {
  const file = `${(name || "Chart").replace(/[\\/:*?"<>|]/g, "_")}.${svg ? "svg" : "png"}`;
  const data = svg ? chartSvg(c, d) : (await import("./picture")).pngAt(await chartCanvas(c, d, 3), 16);
  const path = await call<string | null>("save_file", { name: file, data });
  if (path) toast(`Saved ${path}`, true);
};
const saveStyle = async (c: ChartDef) => {
  const name = (await askText("Save as Template", "Template name", ""))?.trim(); if (!name) return;
  const o = { ...opts()!, chartStyles: { ...(opts()?.chartStyles ?? {}), [name]: lookOf(c) } } as Options;
  await call("set_options", { opts: o }); setOpts(o); toast(`Template “${name}” saved: right-click any chart › Apply Template, in any plan.`, true);
};

/** The right-click menu of a chart (on a sheet, or in a view with `view`), for the part clicked. */
/** The icon of a chart's part (in menus and the Format pane). */
export const partIcon = (part: string) => part === "title" ? "ctitle" : part === "legend" ? "legend" : part === "plot" ? "plot" : ["x", "y"].includes(part) ? "axes" : part === "y2" ? "y2axis" : part.startsWith("s") ? "series" : part.startsWith("p") ? "ptcolor" : "fmtpane";
export const chartMenu = (e: MouseEvent, o: { c: ChartDef; d?: ChartData; part: string; set: (c: ChartDef) => void; format: (part: string) => void; remove?: () => void; title?: () => void;
  order?: (front: boolean) => void; duplicate?: () => void; copy: () => void; reset?: () => void; extra?: Item[]; pic?: { c: ChartDef; d?: ChartData; name: string }; move?: () => void }) => {
  const c = o.c, part = o.part, si = seriesOf(part), s = c.series[si];
  const setS = (x: Partial<Series>) => o.set({ ...c, series: c.series.map((y, j) => (j === si ? { ...y, ...x } : y)) });
  const setA = (k: "x" | "y" | "y2", x: Partial<Axis>) => o.set({ ...c, [k]: { ...(c[k] ?? {}), ...x } });
  const styles = opts()?.chartStyles ?? {};
  const items: Item[] = [{ label: `Format ${partName(c, part, o.d?.names)}…`, icon: partIcon(part), keys: "Dbl-click", run: () => o.format(part) }];
  if (part.startsWith("p")) items.push({ label: "Point Colour", icon: "ptcolor", sub: [{ label: "As the series", run: () => o.set(withPoint(c, part, "")) }, "-", ...PALETTE.map((x): Item => ({ label: x, swatch: x, run: () => o.set(withPoint(c, part, x)) }))] });
  if (s) items.push(
    { label: "Change Series Chart Type", icon: "ctype", sub: ["column", "line", "area"].includes(c.kind) ? [["column", "Columns"], ["line", "Line"], ["area", "Area"]].map(([k, l]) => ({ label: l, on: kindOf(c, si) === k, run: () => setS({ kind: k }) })) : [{ label: "Only in column, line and area charts", off: true }] },
    { label: s.y2 ? "Plot on Primary Axis" : "Plot on Secondary Axis", icon: "y2axis", off: !["column", "line", "area"].includes(c.kind), run: () => setS({ y2: !s.y2 }) },
    { label: s.labels ? "Remove Data Labels" : "Add Data Labels", icon: "labels", run: () => setS({ labels: !s.labels }) },
    { label: s.trend === "linear" ? "Remove Trendline" : "Add Trendline", icon: "trend", off: ["pie", "doughnut"].includes(c.kind), run: () => setS({ trend: s.trend === "linear" ? "" : "linear" }) });
  if (part === "legend") items.push({ label: "Legend Position", icon: "legend", sub: [["bottom", "Bottom"], ["top", "Top"], ["right", "Right"], ["left", "Left"]].map(([k, l]) => ({ label: l, on: (c.legend || "bottom") === k, run: () => o.set({ ...c, legend: k }) })) });
  if (part === "x" || part === "y" || part === "y2") items.push(
    { label: c[part]?.grid ? "Remove Major Gridlines" : "Add Major Gridlines", icon: "gridlines", run: () => setA(part, { grid: !c[part]?.grid }) },
    { label: "Number Format", icon: "axes", sub: [["", "As the cells"], ["0", "0"], ["#,##0", "#,##0"], ["0%", "0%"], ["0.0%", "0.0%"], ["mmm-yy", "mmm-yy"], ["dd-mmm-yy", "dd-mmm-yy"]].map(([k, l]) => ({ label: l, on: (c[part]?.fmt ?? "") === k, run: () => setA(part, { fmt: k }) })) });
  if (o.title && (part === "title" || part === "area")) items.push({ label: c.title ? "Edit Chart Title" : "Add Chart Title", icon: "ctitle", run: o.title });
  items.push("-",
    { label: "Change Chart Type…", icon: "ctype", run: () => changeType({ c, d: o.d, set: o.set, format: o.format }) },
    ...(o.remove ? [{ label: "Select Data…", icon: "seldata", run: () => o.format("data") }] : []),
    ...(o.move ? [{ label: "Move Chart…", icon: "movechart", run: o.move }] : []),
    "-",
    { label: "Save as Template…", icon: "template", run: () => saveStyle(c) },
    { label: "Apply Template", icon: "style", sub: Object.keys(styles).length ? Object.keys(styles).map(n => ({ label: n, run: () => o.set(withLook(c, styles[n])) })) : [{ label: "No templates yet: Save as Template first", off: true }] },
    { label: "Reset to Match Style", icon: "resetstyle", run: () => (o.reset ? o.reset() : o.set(plainLook(c))) },
    "-",
    { label: "Copy as Picture", icon: "copy", run: o.copy },
    { label: "Save as Picture", icon: "savepic", sub: [false, true].map(svg => ({ label: svg ? "SVG (vector, for drawings)…" : "PNG (for documents and slides)…", run: () => savePicture(o.pic?.c ?? c, o.pic ? o.pic.d : o.d, o.pic?.name ?? (c.title || "Chart"), svg) })) },
    ...(o.extra ?? []));
  if (o.duplicate) items.push({ label: "Duplicate", icon: "dup", run: o.duplicate });
  if (o.order) items.push({ label: "Bring to Front", icon: "front", run: () => o.order!(true) }, { label: "Send to Back", icon: "back", run: () => o.order!(false) });
  if (o.remove) {
    items.push("-");
    if (s) items.push({ label: "Delete Series", icon: "del", off: c.series.length < 2, run: () => o.set({ ...c, series: c.series.filter((_, j) => j !== si) }) });
    else if (part === "title" && c.title) items.push({ label: "Delete Chart Title", icon: "del", run: () => o.set({ ...c, title: "" }) });
    else if (part === "legend") items.push({ label: "Delete Legend", icon: "del", run: () => o.set({ ...c, legend: "none" }) });
    items.push({ label: "Delete Chart", icon: "del", keys: "Del", run: o.remove });
  }
  openMenu(e, items);
};

type Geo = { x: (c: number) => number; y: (r: number) => number; cell: (cx: number, cy: number) => [number, number]; a1: (r: number, c: number) => string; parse: (a: string) => [number, number]; zoom: () => number };

/** The charts of sheet `k` over its cells. */
export const ChartLayer = (p: { k: number; charts: ChartDef[]; data?: ChartData[]; g: Geo; put: (list: ChartDef[], k?: number) => Promise<unknown>; focus: () => void;
  /** A chart sheet: the one chart fills this size and does not move. */ fill?: [number, number]; moved?: (k: number) => void }) => {
  const put1 = (i: number, c: ChartDef) => p.put(p.charts.map((x, j) => (j === i ? c : x)));
  const drag = (e: MouseEvent, i: number, how: string) => {
    e.preventDefault(); e.stopPropagation();
    // A click selects the part under the pointer (a second click on the title edits it); a drag moves the chart.
    const part = how === "move" ? partOf(e) : "", jv = jOf(e), was = chartSel() === i ? chartPart() : null;
    if (chartSel() !== i) setChartPart("");
    if (fmtChart() !== null) setFmtChart(p.charts[i].id);
    setChartSel(i); p.focus();
    const o = p.charts[i], x0 = e.clientX, y0 = e.clientY, z = p.g.zoom();
    let next = o;
    const el = () => document.querySelector<HTMLElement>(`.schart[data-i="${i}"]`);
    // Where the chart was grabbed, measured once: the chart then follows the pointer smoothly and
    // keeps its place in the cell it is dropped on (a cell and an offset inside it).
    const r0 = el()?.getBoundingClientRect(), gx = x0 - (r0?.left ?? x0), gy = y0 - (r0?.top ?? y0);
    const [ar, ac] = p.g.parse(o.cell), left0 = p.g.x(ac) + (o.dx ?? 0), top0 = p.g.y(ar) + (o.dy ?? 0);
    let moved = false;
    const mv = (m: MouseEvent) => {
      if (p.fill) return;
      const dx = (m.clientX - x0) / z, dy = (m.clientY - y0) / z;
      if (!moved && Math.abs(dx) < 3 && Math.abs(dy) < 3) return;
      moved = true;
      if (how === "move") {
        const [r, c] = p.g.cell(m.clientX - gx + 1, m.clientY - gy + 1);
        if (r >= 0 && c >= 0) { const L = Math.max(0, left0 + dx), T = Math.max(0, top0 + dy); next = { ...o, cell: p.g.a1(r, c), dx: Math.max(0, Math.round(L - p.g.x(c))), dy: Math.max(0, Math.round(T - p.g.y(r))) }; }
      }
      else {
        let w = o.w, h = o.h;
        if (how.includes("e")) w = o.w + dx;
        if (how.includes("s")) h = o.h + dy;
        next = { ...o, w: Math.round(Math.max(160, w)), h: Math.round(Math.max(110, h)) };
      }
      const [r, c] = p.g.parse(next.cell), box = el();
      if (box) Object.assign(box.style, { left: `${p.g.x(c) + (next.dx ?? 0)}px`, top: `${p.g.y(r) + (next.dy ?? 0)}px`, width: `${next.w}px`, height: `${next.h}px` });
    };
    const up = () => {
      removeEventListener("mousemove", mv); removeEventListener("mouseup", up);
      if (next !== o) { p.put(p.charts.map((x, j) => (j === i ? next : x))); return; }
      if (how !== "move") return;
      if (part === "title" && was === "title") setEditTitle(o.id);
      // A second click on a selected series selects the one bar or point under the pointer.
      else if (part.startsWith("s") && jv !== null && was && seriesOf(was) === +part.slice(1)) setChartPart(`p${part.slice(1)}:${jv}`);
      else setChartPart(part === "area" ? "" : part);
    };
    addEventListener("mousemove", mv); addEventListener("mouseup", up);
  };
  // The tools keep the sheet and charts they were opened on (picking a range may show another tab meanwhile).
  const ctl = (i: number): ChartCtl | null => { const c = p.charts[i], list = p.charts, kk = p.k; return c ? { c, d: p.data?.[i], set: x => p.put(list.map((y, j) => (j === i ? x : y)), kk), format: fmt(c, i), a1: p.g.a1, parse: p.g.parse, k: p.k, move: () => moveChart(p.k, i, k => { setChartSel(0); setChartPart(""); p.moved?.(k); }),
    reselect: () => { if (sheetK() !== kk) setSheetK(kk); setChartPart(""); setChartSel(i); } } : null; };
  const fmt = (c: ChartDef, i: number) => (pt: string) => { if (pt === "data") { const t = ctl(i); if (t) selectData(t); return; } setChartPart(pt === "area" ? "" : pt); if (!pt || pt === "area") setPaneTab("chart"); setFmtChart(c.id); };
  createEffect(() => setChartCtl(ctl(chartSel())));
  onCleanup(() => setChartCtl(null));
  const copy = async (i: number) => { const cv = await chartCanvas(p.charts[i], p.data?.[i], 2); (await import("./picture")).copyCanvas(cv, "Chart", 16); };
  const menu = (e: MouseEvent, i: number, part: string) => {
    const c = p.charts[i];
    chartMenu(e, { c, d: p.data?.[i], part, set: x => put1(i, x), format: fmt(c, i), move: ctl(i)?.move,
      remove: () => { p.put(p.charts.filter((_, j) => j !== i)); setChartSel(p.fill ? 0 : -1); setFmtChart(null); },
      title: () => { if (!c.title) put1(i, { ...c, title: "Chart Title" }); setEditTitle(c.id); },
      order: front => { const rest = p.charts.filter((_, j) => j !== i); p.put(front ? [...rest, c] : [c, ...rest]); setChartSel(front ? rest.length : 0); },
      duplicate: () => { const [r, cc] = p.g.parse(c.cell); p.put([...p.charts, { ...c, id: 0, cell: p.g.a1(r + 2, cc + 1), dx: c.dx, dy: c.dy }]); setChartSel(p.charts.length); },
      copy: () => copy(i) });
  };
  return (
    <For each={p.charts}>{(ch, i) => { const rc = () => p.g.parse(ch.cell); return (
      <div class="schart" data-i={i()} classList={{ on: chartSel() === i(), fill: !!p.fill }} style={p.fill ? { left: "0", top: "0", width: `${p.fill[0]}px`, height: `${p.fill[1]}px` } : { left: `${p.g.x(rc()[1]) + (ch.dx ?? 0)}px`, top: `${p.g.y(rc()[0]) + (ch.dy ?? 0)}px`, width: `${ch.w}px`, height: `${ch.h}px` }}
        onMouseDown={e => { if (e.button === 0) drag(e, i(), "move"); else { e.stopPropagation(); if (chartSel() !== i()) setChartPart(""); setChartSel(i()); } }}
        onDblClick={e => { e.stopPropagation(); const p0 = partOf(e), cur = chartPart(), pt = cur.startsWith("p") && p0.startsWith("s") && seriesOf(cur) === +p0.slice(1) ? cur : p0; setChartPart(pt === "area" ? "" : pt); setEditTitle(null); setFmtChart(ch.id); }}
        onContextMenu={e => { e.preventDefault(); e.stopPropagation(); const p0 = partOf(e), cur = chartSel() === i() ? chartPart() : "", pt = cur.startsWith("p") && p0.startsWith("s") && seriesOf(cur) === +p0.slice(1) ? cur : p0; setChartSel(i()); setChartPart(pt === "area" ? "" : pt); menu(e, i(), pt); }}>
        {(() => { const d = p.data?.[i()], [w, h] = p.fill ?? [ch.w, ch.h]; return <XChart c={ch} d={d} w={w} h={h} sel={chartSel() === i() ? chartPart() || "area" : undefined} />; }) as unknown as JSX.Element}
        <Show when={editTitle() === ch.id}><TitleBox c={ch} done={t => { setEditTitle(null); if (t !== null && t !== (ch.title ?? "")) put1(i(), { ...ch, title: t }); p.focus(); }} /></Show>
        <Show when={chartSel() === i()}>
          <Show when={!p.fill}><i class="ch-e" onMouseDown={e => drag(e, i(), "e")} /><i class="ch-s" onMouseDown={e => drag(e, i(), "s")} /><i class="ch-se" onMouseDown={e => drag(e, i(), "se")} /></Show>
          <SideButtons t={chartCtl} more={e => menu(e, i(), chartPart() || "area")} />
        </Show>
      </div>); }}</For>
  );
};

/** The title edited where it stands: Enter keeps it, Escape leaves it as it was. */
const TitleBox = (p: { c: ChartDef; done: (t: string | null) => void }) => {
  const fs = (p.c.font || 11) * 1.35;
  let el!: HTMLInputElement, over = false;
  const end = (t: string | null) => { if (!over) { over = true; p.done(t); } };
  setTimeout(() => { el?.focus(); el?.select(); });
  return <input ref={el} class="ch-title" value={p.c.title ?? ""} style={{ "font-size": `${fs}px` }} onMouseDown={e => e.stopPropagation()} onDblClick={e => e.stopPropagation()}
    onKeyDown={e => { e.stopPropagation(); if (e.key === "Enter") end(e.currentTarget.value.trim()); if (e.key === "Escape") end(null); }} onBlur={e => end(e.currentTarget.value.trim())} />;
};

// ---------- the Format Chart pane ----------
const FMTS: [string, string][] = [["", "As the cells"], ["0", "0"], ["0.0", "0.0"], ["#,##0", "#,##0"], ["#,##0.00", "#,##0.00"], ["0%", "0%"], ["0.0%", "0.0%"], ["dd-mmm-yy", "dd-mmm-yy"], ["mmm-yy", "mmm-yy"], ["mmm yyyy", "mmm yyyy"], ["dd/mm/yyyy", "dd/mm/yyyy"]];
const Row = (p: { l: string; children: JSX.Element; title?: string }) => <label class="cfrow" title={p.title}><span>{p.l}</span>{p.children}</label>;
const Sel = (p: { value: string; set: (v: string) => void; options: [string, string][] }) => <select value={p.value} onChange={e => p.set(e.currentTarget.value)}><For each={p.options}>{([v, l]) => <option value={v}>{l}</option>}</For></select>;
const Txt = (p: { value: string; set: (v: string) => void; placeholder?: string; title?: string }) => <input value={p.value} placeholder={p.placeholder} title={p.title} onChange={e => p.set(e.currentTarget.value)} />;
const Num = (p: { value: number | null | undefined; set: (v: number | null) => void; placeholder?: string; step?: string }) => <input type="number" step={p.step ?? "any"} value={p.value ?? ""} placeholder={p.placeholder ?? "Auto"} onChange={e => { const v = e.currentTarget.value.trim(); p.set(v === "" ? null : +v); }} />;
const Chk = (p: { value: boolean; set: (v: boolean) => void; l: string }) => <label class="cfchk"><input type="checkbox" checked={p.value} onChange={e => p.set(e.currentTarget.checked)} />{p.l}</label>;
const Col = (p: { value: string; set: (v: string) => void }) => <input type="color" value={/^#[0-9a-f]{6}$/i.test(p.value) ? p.value : "#ffffff"} onChange={e => p.set(e.currentTarget.value.toUpperCase())} />;

const [paneTab, setPaneTab] = createSignal("chart");
export { setPaneTab };
export const ChartPane = (p: { chart: ChartDef; data?: ChartData; set: (c: ChartDef) => void; close: () => void; view?: boolean; reset?: () => void; part?: string }) => {
  const [tab, setTab] = [paneTab, setPaneTab];
  // The pane follows the part selected in the chart: its tab, and the part's own settings in view.
  createEffect(on(() => p.part, part => {
    if (!part) return;
    setTab(part.startsWith("s") || part.startsWith("p") ? "series" : part === "x" || part === "y" || part === "y2" ? "axes" : "chart");
    setTimeout(() => { const el = document.querySelector<HTMLElement>(`.chartpane [data-k="${part.startsWith("p") ? `s${seriesOf(part)}` : part}"]`); if (el) { el.scrollIntoView({ block: "nearest" }); el.classList.add("flash"); setTimeout(() => el.classList.remove("flash"), 900); if (part === "title") el.querySelector("input")?.focus(); } });
  }));
  const c = () => p.chart;
  const set = (x: Partial<ChartDef>) => p.set({ ...c(), ...x });
  const setS = (i: number, x: Partial<Series>) => set({ series: c().series.map((s, j) => (j === i ? { ...s, ...x } : s)) });
  const setA = (k: "x" | "y" | "y2", x: Partial<Axis>) => set({ [k]: { ...(c()[k] ?? {}), ...x } } as Partial<ChartDef>);
  const combo = () => ["column", "line", "area"].includes(c().kind);
  const styles = () => opts()?.chartStyles ?? {};
  const delStyle = async (name: string) => { const s = { ...styles() }; delete s[name]; const o = { ...opts()!, chartStyles: s } as Options; await call("set_options", { opts: o }); setOpts(o); };
  const AxisBox = (q: { k: "x" | "y" | "y2"; l: string }) => {
    const a = () => c()[q.k] ?? {};
    return <fieldset class="cfset" data-k={q.k}><legend>{q.l}</legend>
      <Row l="Title"><Txt value={a().title ?? ""} set={v => setA(q.k, { title: v })} /></Row>
      <Show when={q.k !== "x" || c().kind === "scatter"}><Row l="Minimum"><Num value={a().min} set={v => setA(q.k, { min: v })} /></Row><Row l="Maximum"><Num value={a().max} set={v => setA(q.k, { max: v })} /></Row>
        <Row l="Major unit" title="The step between gridlines and labels"><Num value={a().major} set={v => setA(q.k, { major: v && v > 0 ? v : null })} /></Row></Show>
      <Row l="Number format"><Sel value={FMTS.some(f => f[0] === (a().fmt ?? "")) ? a().fmt ?? "" : "custom"} set={v => setA(q.k, { fmt: v === "custom" ? a().fmt || "0.00" : v })} options={[...FMTS, ["custom", "Custom…"]]} /></Row>
      <Show when={!FMTS.some(f => f[0] === (a().fmt ?? ""))}><Row l="Format code"><Txt value={a().fmt ?? ""} set={v => setA(q.k, { fmt: v })} placeholder="#,##0.0  or  dd-mmm" /></Row></Show>
      <Show when={q.k === "x" && !["scatter", "pie", "doughnut"].includes(c().kind)}>
        <Row l="Label angle"><Sel value={a().angle === null || a().angle === undefined ? "" : String(a().angle)} set={v => setA("x", { angle: v === "" ? null : +v })} options={[["", "Automatic"], ["0", "Level"], ["-45", "Slanted"], ["-90", "Upright"]]} /></Row>
        <Row l="Label every" title="Show a label on every n-th category (0: as many as fit)"><Num value={a().every || null} set={v => setA("x", { every: Math.max(0, Math.round(v ?? 0)) })} placeholder="Auto" step="1" /></Row></Show>
      <Chk l="Gridlines" value={!!a().grid} set={v => setA(q.k, { grid: v })} /><Chk l="Hide this axis" value={!!a().hide} set={v => setA(q.k, { hide: v })} />
    </fieldset>;
  };
  return (
    <aside class="chartpane" onMouseDown={e => e.stopPropagation()} onKeyDown={e => e.stopPropagation()}>
      <header><b><Icon n={partIcon(p.part || "area")} />{p.part ? `Format ${partName(c(), p.part, p.data?.names)}` : "Format Chart"}</b><button class="x" title="Close" onClick={p.close}>×</button></header>
      <div class="cftabs"><For each={[["chart", "Chart", "fmtpane"], ["series", "Series", "series"], ["axes", "Axes", "axes"], ["style", "Style", "style"]]}>{([k, l, ic]) => <button classList={{ on: tab() === k }} onClick={() => setTab(k)}><Icon n={ic} />{l}</button>}</For></div>
      <div class="cfbody">
        <Show when={tab() === "chart"}>
          <Row l="Chart type"><Sel value={c().kind} set={v => set({ kind: v })} options={KINDS} /></Row>
          <Show when={["column", "bar", "line", "area"].includes(c().kind)}><Row l="Stacking"><Sel value={c().stack ?? ""} set={v => set({ stack: v })} options={[["", "Side by side"], ["stacked", "Stacked"], ["percent", "100% stacked"]]} /></Row></Show>
          <Show when={!p.view}><div data-k="title"><Row l="Title"><Txt value={c().title ?? ""} set={v => set({ title: v })} placeholder="No title" /></Row></div></Show>
          <div data-k="legend"><Row l="Legend"><Sel value={c().legend || "bottom"} set={v => set({ legend: v })} options={[["bottom", "Bottom"], ["top", "Top"], ["right", "Right"], ["left", "Left"], ["none", "None"]]} /></Row></div>
          <Row l="Text size (pt)"><Num value={c().font || null} set={v => set({ font: v ?? 0 })} placeholder="11" /></Row>
          <Chk l="Data labels on every series" value={!!c().labels} set={v => set({ labels: v })} />
          <Show when={["column", "bar"].includes(c().kind) || c().series.some((_, i) => ["column", "bar"].includes(kindOf(c(), i)))}><Row l="Gap between bars (%)" title="The space between groups of bars, as a share of a bar's width"><Num value={c().gap ?? 150} set={v => set({ gap: Math.max(0, Math.min(500, v ?? 150)) })} step="10" /></Row></Show>
          <Show when={["column", "line", "area"].includes(c().kind)}><Chk l="Data table under the chart" value={!!c().table} set={v => set({ table: v })} /></Show>
          <Row l="Empty cells" title="How a chart shows cells with no value"><Sel value={c().empty ?? ""} set={v => set({ empty: v })} options={[["", "Gaps"], ["zero", "Zero"], ["span", "Connect the line"]]} /></Row>
          <div data-k="area"><Row l="Chart area"><Col value={c().bg || "#FFFFFF"} set={v => set({ bg: v })} /></Row></div>
          <div data-k="plot"><Row l="Plot area"><Col value={c().plot || "#FFFFFF"} set={v => set({ plot: v === "#FFFFFF" ? "" : v })} /></Row></div>
          <Show when={!p.view}><Row l="Size (px)"><span class="cfwh"><Num value={c().w} set={v => set({ w: Math.max(160, v ?? c().w) })} step="10" />×<Num value={c().h} set={v => set({ h: Math.max(110, v ?? c().h) })} step="10" /></span></Row>
          <Row l="Categories (X)" title="The cells with the labels or dates under the bars, e.g. A2:A20 or Data!A2:A20"><Txt value={c().cats} set={v => set({ cats: v.trim() })} placeholder="A2:A20" /></Row></Show>
          <Show when={p.reset}><p class="muted">This chart's look is kept with the plan. <button class="btn sm" onClick={() => p.reset?.()}>Back to the standard look</button></p></Show>
        </Show>
        <Show when={tab() === "series"}>
          <Index each={c().series}>{(s, i) => (
            <fieldset class="cfset" data-k={`s${i}`}><legend><i class="cfsw" style={{ background: colorOf(c(), i) }} />{p.data?.names[i] || s().name || `Series ${i + 1}`}</legend>
              <Show when={!p.view}><Row l="Name" title="Text, or =B1 to read it from a cell"><Txt value={s().name} set={v => setS(i, { name: v })} /></Row>
              <Row l="Values"><Txt value={s().vals} set={v => setS(i, { vals: v.trim() })} placeholder="B2:B20" /></Row></Show>
              <Show when={combo()}><Row l="Draw as" title="A combined chart: each series as columns, a line or an area"><Sel value={s().kind ?? ""} set={v => setS(i, { kind: v })} options={[["", "Same as the chart"], ["column", "Columns"], ["line", "Line"], ["area", "Area"]]} /></Row>
                <Chk l="On the secondary axis (right)" value={!!s().y2} set={v => setS(i, { y2: v })} /></Show>
              <Row l="Colour"><Col value={colorOf(c(), i)} set={v => setS(i, { color: v })} /></Row>
              <Show when={["line", "scatter", "area"].includes(kindOf(c(), i))}>
                <Row l="Line width (pt)"><Num value={s().width || null} set={v => setS(i, { width: v ?? 0 })} placeholder="2.25" step="0.25" /></Row>
                <Row l="Dash"><Sel value={s().dash ?? ""} set={v => setS(i, { dash: v })} options={[["", "Solid"], ["dash", "Dashed"], ["dot", "Dotted"]]} /></Row>
                <Row l="Markers"><Sel value={s().marker ?? ""} set={v => setS(i, { marker: v })} options={[["", "Automatic"], ["none", "None"], ["circle", "Circle"], ["square", "Square"], ["diamond", "Diamond"], ["triangle", "Triangle"]]} /></Row>
                <Chk l="Smooth line" value={!!s().smooth} set={v => setS(i, { smooth: v })} />
              </Show>
              <Chk l="Data labels" value={!!s().labels} set={v => setS(i, { labels: v })} />
              <Show when={s().labels || c().labels}>
                <Row l="Labels show"><Sel value={s().lwhat ?? ""} set={v => setS(i, { lwhat: v })} options={[["", "Value"], ["name", "Series name"], ["cat", "Category"], ["catval", "Category and value"]]} /></Row>
                <Row l="Label position"><Sel value={s().lpos ?? ""} set={v => setS(i, { lpos: v })} options={["line", "scatter", "area"].includes(kindOf(c(), i)) ? [["", "Above"], ["below", "Below"], ["center", "Centre"]] : [["", "Outside end"], ["inside", "Inside end"], ["center", "Centre"], ["base", "Inside base"]]} /></Row></Show>
              <Show when={p.part?.startsWith(`p${i}:`)}><Row l={`Point ${+p.part!.split(":")[1] + 1} colour`}><span class="cfwh"><Col value={(s().pts ?? []).find(q => q.i === +p.part!.split(":")[1])?.color || colorOf(c(), i)} set={v => p.set(withPoint(c(), p.part!, v))} /><button class="btn sm" onClick={() => p.set(withPoint(c(), p.part!, ""))}>As the series</button></span></Row></Show>
              <Show when={(s().pts ?? []).length}><p class="muted">{(s().pts ?? []).length} bar(s) or point(s) with their own colour. <button class="btn sm" onClick={() => setS(i, { pts: [] })}>All as the series</button></p></Show>
              <Show when={!["pie", "doughnut"].includes(c().kind)}><Chk l="Linear trend line" value={s().trend === "linear"} set={v => setS(i, { trend: v ? "linear" : "" })} /></Show>
              <Show when={!p.view}><div class="cfbtns">
                <button class="btn sm" disabled={i === 0} onClick={() => { const v = [...c().series]; [v[i - 1], v[i]] = [v[i], v[i - 1]]; set({ series: v }); }}>Up</button>
                <button class="btn sm" disabled={i === c().series.length - 1} onClick={() => { const v = [...c().series]; [v[i + 1], v[i]] = [v[i], v[i + 1]]; set({ series: v }); }}>Down</button>
                <button class="btn sm" onClick={() => set({ series: c().series.filter((_, j) => j !== i) })}>Remove</button>
              </div></Show>
            </fieldset>)}</Index>
          <Show when={!p.view}><button class="btn sm" onClick={() => set({ series: [...c().series, { name: `Series ${c().series.length + 1}`, vals: "", color: PALETTE[c().series.length % PALETTE.length] }] })}>+ Add series</button></Show>
        </Show>
        <Show when={tab() === "axes"}>
          <Show when={!["pie", "doughnut"].includes(c().kind)} fallback={<p class="muted">A pie has no axes; its number format is the values' own (Axes come back with another chart type).</p>}>
            <AxisBox k="x" l={c().kind === "bar" ? "Category axis (vertical)" : c().kind === "scatter" ? "X axis" : "Category axis"} />
            <AxisBox k="y" l={c().kind === "bar" ? "Value axis (horizontal)" : "Value axis"} />
            <Show when={c().series.some(s => s.y2)}><AxisBox k="y2" l="Secondary value axis (right)" /></Show>
          </Show>
        </Show>
        <Show when={tab() === "style"}>
          <p class="muted">A template keeps a chart's look (colours, lines, markers, labels, legend, axis formats and bounds, background) to give any chart, in any plan, the same look in one click.</p>
          <button class="btn sm pri" onClick={() => saveStyle(c())}>Save this look as a template…</button>
          <For each={Object.keys(styles())} fallback={<p class="muted">No templates saved yet.</p>}>{name => (
            <div class="cfstyle"><button class="btn sm" title="Give this chart that look" onClick={() => p.set(withLook(c(), styles()[name]))}>Apply</button><b>{name}</b><button class="x" title="Delete the template" onClick={() => delStyle(name)}>×</button></div>)}</For>
        </Show>
      </div>
    </aside>
  );
};
