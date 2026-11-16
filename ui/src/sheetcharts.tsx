// Charts on a sheet: drawn over the cells at their anchor cell, moved by dragging, resized from
// their edges and corners, and formatted in the Format Chart pane (type, data, series, axes, looks,
// saved styles). Each change is one Undo step (Op::SheetCharts).
import { For, Index, JSX, Show, createSignal } from "solid-js";
import { ChartData, ChartDef, Series, Axis, KINDS, PALETTE, XChart, colorOf, kindOf, lookOf, withLook } from "./xchart";
import { opts, setOpts, call, toast } from "./store";
import { Options } from "./api";

export const [chartSel, setChartSel] = createSignal(-1);
export const [fmtChart, setFmtChart] = createSignal<number | null>(null);

type Geo = { x: (c: number) => number; y: (r: number) => number; cell: (cx: number, cy: number) => [number, number]; a1: (r: number, c: number) => string; parse: (a: string) => [number, number]; zoom: () => number };

/** The charts of sheet `k` over its cells. */
export const ChartLayer = (p: { k: number; charts: ChartDef[]; data?: ChartData[]; g: Geo; put: (list: ChartDef[]) => Promise<unknown>; focus: () => void }) => {
  const drag = (e: MouseEvent, i: number, how: string) => {
    e.preventDefault(); e.stopPropagation(); setChartSel(i); p.focus();
    const o = p.charts[i], x0 = e.clientX, y0 = e.clientY, z = p.g.zoom();
    let next = o;
    const el = () => document.querySelector<HTMLElement>(`.schart[data-i="${i}"]`);
    const mv = (m: MouseEvent) => {
      const dx = (m.clientX - x0) / z, dy = (m.clientY - y0) / z;
      if (how === "move") { const [r, c] = p.g.cell(m.clientX - (x0 - (el()?.getBoundingClientRect().left ?? x0)) + 2, m.clientY - (y0 - (el()?.getBoundingClientRect().top ?? y0)) + 2); if (r >= 0 && c >= 0) next = { ...o, cell: p.g.a1(r, c) }; }
      else {
        let w = o.w, h = o.h;
        if (how.includes("e")) w = o.w + dx;
        if (how.includes("s")) h = o.h + dy;
        next = { ...o, w: Math.round(Math.max(160, w)), h: Math.round(Math.max(110, h)) };
      }
      const [r, c] = p.g.parse(next.cell), box = el();
      if (box) Object.assign(box.style, { left: `${p.g.x(c)}px`, top: `${p.g.y(r)}px`, width: `${next.w}px`, height: `${next.h}px` });
    };
    const up = () => { removeEventListener("mousemove", mv); removeEventListener("mouseup", up); if (next !== o) p.put(p.charts.map((x, j) => (j === i ? next : x))); };
    addEventListener("mousemove", mv); addEventListener("mouseup", up);
  };
  return (
    <For each={p.charts}>{(ch, i) => { const rc = () => p.g.parse(ch.cell); return (
      <div class="schart" data-i={i()} classList={{ on: chartSel() === i() }} style={{ left: `${p.g.x(rc()[1])}px`, top: `${p.g.y(rc()[0])}px`, width: `${ch.w}px`, height: `${ch.h}px` }}
        title="Drag to move · drag an edge or corner to resize · double-click to format · Delete removes it" onMouseDown={e => { if (e.button === 0) drag(e, i(), "move"); else { setChartSel(i()); } }} onDblClick={e => { e.stopPropagation(); setFmtChart(ch.id); }}>
        {(() => { const d = p.data?.[i()]; return <XChart c={ch} d={d} w={ch.w} h={ch.h} />; }) as unknown as JSX.Element}
        <Show when={chartSel() === i()}>
          <i class="ch-e" onMouseDown={e => drag(e, i(), "e")} /><i class="ch-s" onMouseDown={e => drag(e, i(), "s")} /><i class="ch-se" onMouseDown={e => drag(e, i(), "se")} />
          <div class="ch-tools" onMouseDown={e => e.stopPropagation()}>
            <button title="Format Chart: type, data, series, axes and looks" onClick={() => setFmtChart(ch.id)}>Format</button>
            <button title="Copy the chart as a picture, sharp, to paste into a document, slides or a workbook" onClick={async () => { const { chartCanvas } = await import("./xchart"); const cv = await chartCanvas(ch, p.data?.[i()], 2); (await import("./picture")).copyCanvas(cv, "Chart", 16); }}>Copy</button>
            <button title="Remove the chart" onClick={() => { p.put(p.charts.filter((_, j) => j !== i())); setChartSel(-1); }}>×</button>
          </div>
        </Show>
      </div>); }}</For>
  );
};

// ---------- the Format Chart pane ----------
const FMTS: [string, string][] = [["", "As the cells"], ["0", "0"], ["0.0", "0.0"], ["#,##0", "#,##0"], ["#,##0.00", "#,##0.00"], ["0%", "0%"], ["0.0%", "0.0%"], ["dd-mmm-yy", "dd-mmm-yy"], ["mmm-yy", "mmm-yy"], ["mmm yyyy", "mmm yyyy"], ["dd/mm/yyyy", "dd/mm/yyyy"]];
const Row = (p: { l: string; children: JSX.Element; title?: string }) => <label class="cfrow" title={p.title}><span>{p.l}</span>{p.children}</label>;
const Sel = (p: { value: string; set: (v: string) => void; options: [string, string][] }) => <select value={p.value} onChange={e => p.set(e.currentTarget.value)}><For each={p.options}>{([v, l]) => <option value={v}>{l}</option>}</For></select>;
const Txt = (p: { value: string; set: (v: string) => void; placeholder?: string; title?: string }) => <input value={p.value} placeholder={p.placeholder} title={p.title} onChange={e => p.set(e.currentTarget.value)} />;
const Num = (p: { value: number | null | undefined; set: (v: number | null) => void; placeholder?: string; step?: string }) => <input type="number" step={p.step ?? "any"} value={p.value ?? ""} placeholder={p.placeholder ?? "Auto"} onChange={e => { const v = e.currentTarget.value.trim(); p.set(v === "" ? null : +v); }} />;
const Chk = (p: { value: boolean; set: (v: boolean) => void; l: string }) => <label class="cfchk"><input type="checkbox" checked={p.value} onChange={e => p.set(e.currentTarget.checked)} />{p.l}</label>;
const Col = (p: { value: string; set: (v: string) => void }) => <input type="color" value={/^#[0-9a-f]{6}$/i.test(p.value) ? p.value : "#ffffff"} onChange={e => p.set(e.currentTarget.value.toUpperCase())} />;

export const ChartPane = (p: { chart: ChartDef; data?: ChartData; set: (c: ChartDef) => void; close: () => void; view?: boolean; reset?: () => void }) => {
  const [tab, setTab] = createSignal("chart");
  const c = () => p.chart;
  const set = (x: Partial<ChartDef>) => p.set({ ...c(), ...x });
  const setS = (i: number, x: Partial<Series>) => set({ series: c().series.map((s, j) => (j === i ? { ...s, ...x } : s)) });
  const setA = (k: "x" | "y" | "y2", x: Partial<Axis>) => set({ [k]: { ...(c()[k] ?? {}), ...x } } as Partial<ChartDef>);
  const combo = () => ["column", "line", "area"].includes(c().kind);
  const styles = () => opts()?.chartStyles ?? {};
  const saveStyle = async () => {
    const name = prompt("Save this chart's look as a style named:", "")?.trim(); if (!name) return;
    const o = { ...opts()!, chartStyles: { ...styles(), [name]: lookOf(c()) } } as Options;
    await call("set_options", { opts: o }); setOpts(o); toast(`Style “${name}” saved: Format Chart › Style applies it to any chart, in any plan.`, true);
  };
  const delStyle = async (name: string) => { const s = { ...styles() }; delete s[name]; const o = { ...opts()!, chartStyles: s } as Options; await call("set_options", { opts: o }); setOpts(o); };
  const AxisBox = (q: { k: "x" | "y" | "y2"; l: string }) => {
    const a = () => c()[q.k] ?? {};
    return <fieldset class="cfset"><legend>{q.l}</legend>
      <Row l="Title"><Txt value={a().title ?? ""} set={v => setA(q.k, { title: v })} /></Row>
      <Show when={q.k !== "x" || c().kind === "scatter"}><Row l="Minimum"><Num value={a().min} set={v => setA(q.k, { min: v })} /></Row><Row l="Maximum"><Num value={a().max} set={v => setA(q.k, { max: v })} /></Row>
        <Row l="Major unit" title="The step between gridlines and labels"><Num value={a().major} set={v => setA(q.k, { major: v && v > 0 ? v : null })} /></Row></Show>
      <Row l="Number format"><Sel value={FMTS.some(f => f[0] === (a().fmt ?? "")) ? a().fmt ?? "" : "custom"} set={v => setA(q.k, { fmt: v === "custom" ? a().fmt || "0.00" : v })} options={[...FMTS, ["custom", "Custom…"]]} /></Row>
      <Show when={!FMTS.some(f => f[0] === (a().fmt ?? ""))}><Row l="Format code"><Txt value={a().fmt ?? ""} set={v => setA(q.k, { fmt: v })} placeholder="#,##0.0  or  dd-mmm" /></Row></Show>
      <Chk l="Gridlines" value={!!a().grid} set={v => setA(q.k, { grid: v })} /><Chk l="Hide this axis" value={!!a().hide} set={v => setA(q.k, { hide: v })} />
    </fieldset>;
  };
  return (
    <aside class="chartpane" onMouseDown={e => e.stopPropagation()} onKeyDown={e => e.stopPropagation()}>
      <header><b>Format Chart</b><button class="x" title="Close" onClick={p.close}>×</button></header>
      <div class="cftabs"><For each={[["chart", "Chart"], ["series", "Series"], ["axes", "Axes"], ["style", "Style"]]}>{([k, l]) => <button classList={{ on: tab() === k }} onClick={() => setTab(k)}>{l}</button>}</For></div>
      <div class="cfbody">
        <Show when={tab() === "chart"}>
          <Row l="Chart type"><Sel value={c().kind} set={v => set({ kind: v })} options={KINDS} /></Row>
          <Show when={["column", "bar", "line", "area"].includes(c().kind)}><Row l="Stacking"><Sel value={c().stack ?? ""} set={v => set({ stack: v })} options={[["", "Side by side"], ["stacked", "Stacked"], ["percent", "100% stacked"]]} /></Row></Show>
          <Show when={!p.view}><Row l="Title"><Txt value={c().title ?? ""} set={v => set({ title: v })} placeholder="No title" /></Row></Show>
          <Row l="Legend"><Sel value={c().legend || "bottom"} set={v => set({ legend: v })} options={[["bottom", "Bottom"], ["top", "Top"], ["right", "Right"], ["left", "Left"], ["none", "None"]]} /></Row>
          <Row l="Text size (pt)"><Num value={c().font || null} set={v => set({ font: v ?? 0 })} placeholder="11" /></Row>
          <Chk l="Data labels on every series" value={!!c().labels} set={v => set({ labels: v })} />
          <Show when={["column", "bar"].includes(c().kind) || c().series.some((_, i) => ["column", "bar"].includes(kindOf(c(), i)))}><Row l="Gap between bars (%)" title="The space between groups of bars, as a share of a bar's width"><Num value={c().gap ?? 150} set={v => set({ gap: Math.max(0, Math.min(500, v ?? 150)) })} step="10" /></Row></Show>
          <Row l="Chart area"><Col value={c().bg || "#FFFFFF"} set={v => set({ bg: v })} /></Row>
          <Row l="Plot area"><Col value={c().plot || "#FFFFFF"} set={v => set({ plot: v === "#FFFFFF" ? "" : v })} /></Row>
          <Show when={!p.view}><Row l="Size (px)"><span class="cfwh"><Num value={c().w} set={v => set({ w: Math.max(160, v ?? c().w) })} step="10" />×<Num value={c().h} set={v => set({ h: Math.max(110, v ?? c().h) })} step="10" /></span></Row>
          <Row l="Categories (X)" title="The cells with the labels or dates under the bars, e.g. A2:A20 or Data!A2:A20"><Txt value={c().cats} set={v => set({ cats: v.trim() })} placeholder="A2:A20" /></Row></Show>
          <Show when={p.reset}><p class="muted">This chart's look is kept with the plan. <button class="btn sm" onClick={() => p.reset?.()}>Back to the standard look</button></p></Show>
        </Show>
        <Show when={tab() === "series"}>
          <Index each={c().series}>{(s, i) => (
            <fieldset class="cfset"><legend><i class="cfsw" style={{ background: colorOf(c(), i) }} />{p.data?.names[i] || s().name || `Series ${i + 1}`}</legend>
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
          <p class="muted">A style keeps a chart's look (colours, lines, markers, labels, legend, axis formats and bounds, background) to give any chart, in any plan, the same look in one click.</p>
          <button class="btn sm pri" onClick={saveStyle}>Save this look as a style…</button>
          <For each={Object.keys(styles())} fallback={<p class="muted">No styles saved yet.</p>}>{name => (
            <div class="cfstyle"><button class="btn sm" title="Give this chart that look" onClick={() => p.set(withLook(c(), styles()[name]))}>Apply</button><b>{name}</b><button class="x" title="Delete the style" onClick={() => delStyle(name)}>×</button></div>)}</For>
        </Show>
      </div>
    </aside>
  );
};
