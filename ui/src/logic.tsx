// The time-scaled logic diagram: every task a bar at its dates on a timescale, packed into lanes
// (tsld.rs; a task keeps its driving predecessor's lane), the links drawn between them, a band per
// phase if chosen, the status date. Double-click a bar to show it in the Gantt chart. For large
// plans: the date ruler stays at the top, Fit shows the whole plan, −/+ and Ctrl+wheel zoom about the
// pointer, and dragging the background moves the view.
import { For, Show, createEffect, createMemo, createResource } from "solid-js";
import { call, out, tick, setSel, setView, kept } from "./store";
import { Page } from "./views";
import { Seg } from "./ui";
import { dayDate, fmtDay } from "./api";

type Bar = { uid: number; id: number; code: string; name: string; lane: number; start: number; finish: number; critical: boolean; milestone: boolean; pct: number };
type Link = { from: number; to: number; kind: string; critical: boolean; driving: boolean };
type Tsld = { start: number; finish: number; lanes: number; bars: Bar[]; links: Link[]; bands: { name: string; lane: number; lanes: number }[]; status: number | null };
const LANE = 40, TOP = 34, LEFT = 0, MIN = 0.25, MAX = 60;
const [phase, setPhase] = kept("tsld-phase", "yes");
const [zoom, setZoom] = kept("tsld-zoom", "12");

export const LogicView = () => {
  const ppd = () => Math.min(MAX, Math.max(MIN, +zoom() || 12));
  // Below 3 px a day the names are left off (each bar keeps its tooltip), so the whole plan's shape fits.
  const names = () => ppd() >= 3;
  // Names are measured on the timescale (about 6 px a character), so a lane never runs one into the next.
  const [data] = createResource(() => [tick(), out()?.tab, phase(), zoom()] as const, () => call<Tsld>("logic_diagram", { phase: phase() === "yes", gap: Math.round(1440 * 8 / ppd()), per: names() ? Math.round(1440 * 6.2 / ppd()) : 0 }));
  const span = (d: Tsld) => [Math.floor(d.start / 1440) - 3, Math.ceil(d.finish / 1440) + 7] as const;
  const geo = createMemo(() => {
    const d = data(); if (!d) return null;
    const [a, b] = span(d);
    const x = (t: number) => LEFT + (t / 1440 - a) * ppd();
    // Month labels every `step` months, so they never overlap when zoomed out (a year a label at most).
    const step = [1, 2, 3, 6, 12].find(s => s * 30 * ppd() >= 56) ?? 12;
    const ticks: { x: number; l: string; major: boolean }[] = [];
    for (let day = a; day <= b; day++) {
      const dt = dayDate(day);
      if (dt.getDate() === 1) ticks.push({ x: x(day * 1440), l: (dt.getFullYear() * 12 + dt.getMonth()) % step ? "" : step === 12 ? String(dt.getFullYear()) : fmtDay(day, "month"), major: true });
      else if (ppd() >= 12 && dt.getDay() === 1) ticks.push({ x: x(day * 1440), l: String(dt.getDate()), major: false });
    }
    const at = new Map(d.bars.map(b => [b.uid, b]));
    return { a, w: (b - a) * ppd() + 40, h: TOP + Math.max(1, d.lanes) * LANE + 10, x, ticks, at };
  });
  const y = (lane: number) => TOP + lane * LANE + 27;
  const open = (uid: number) => { setSel([uid]); setView("gantt"); };
  // Zooming keeps the day under the pointer (or the middle of the view) where it was.
  let wrap: HTMLDivElement | undefined, keep: { day: number; px: number } | null = null;
  const zoomTo = (v: number, px = (wrap?.clientWidth ?? 0) / 2) => {
    const g = geo(); if (wrap && g) keep = { day: g.a + (wrap.scrollLeft + px) / ppd(), px };
    setZoom(String(Math.round(Math.min(MAX, Math.max(MIN, v)) * 100) / 100));
  };
  createEffect(() => { const g = geo(); if (g && wrap && keep) { wrap.scrollLeft = Math.max(0, (keep.day - g.a) * ppd() - keep.px); keep = null; } });
  const fit = () => { const d = data(); if (!d || !wrap) return; const [a, b] = span(d); zoomTo((wrap.clientWidth - 48) / (b - a), 0); keep = { day: a, px: 0 }; };
  const pan = (e: MouseEvent) => {
    if (e.button !== 0 || !wrap || (e.target as Element).closest(".tsbar")) return;
    const w = wrap, x0 = e.clientX, y0 = e.clientY, l0 = w.scrollLeft, t0 = w.scrollTop; w.style.cursor = "grabbing";
    const mv = (m: MouseEvent) => { w.scrollLeft = l0 - (m.clientX - x0); w.scrollTop = t0 - (m.clientY - y0); };
    const up = () => { w.style.cursor = ""; removeEventListener("mousemove", mv); removeEventListener("mouseup", up); };
    addEventListener("mousemove", mv); addEventListener("mouseup", up); e.preventDefault();
  };
  const wheel = (e: WheelEvent) => { if (!e.ctrlKey || !wrap) return; e.preventDefault(); zoomTo(ppd() * (e.deltaY < 0 ? 1.25 : 0.8), e.clientX - wrap.getBoundingClientRect().left); };
  return (
    <Page title="Time-scaled Logic" help="network" tools={<>
      <Seg value={phase()} set={setPhase} options={[["yes", "Bands by phase"], ["no", "One band"]]} />
      <Seg value={zoom()} set={setZoom} options={[["4", "Months"], ["12", "Weeks"], ["30", "Days"]]} />
      <button class="btn" title="Zoom out (Ctrl+wheel)" onClick={() => zoomTo(ppd() * 0.8)}>−</button><button class="btn" title="Zoom in (Ctrl+wheel)" onClick={() => zoomTo(ppd() * 1.25)}>+</button>
      <button class="btn" title="The whole plan across the window" onClick={fit}>Fit</button>
    </>}>
      <Show when={geo()} keyed fallback={<p class="muted">Working out the diagram…</p>}>{g => { const d = data()!; return (<>
        <div class="netleg tslegend"><b>Legend</b><span><span class="tskey crit" /> Critical <span class="tskey" /> Other work <span class="tskey link" /> Driving link <span class="tskey link loose" /> Other link</span><span class="muted">Drag to move, Ctrl+wheel to zoom, double-click a bar to show it in the Gantt chart.{names() ? "" : " Zoomed out: point at a bar for its name."}</span></div>
        <div class="tsldwrap" ref={wrap} onMouseDown={pan} onWheel={wheel}>
          <svg class="tsldhead" width={g.w} height={TOP} viewBox={`0 0 ${g.w} ${TOP}`}>
            <For each={g.ticks}>{t => <g><line x1={t.x} x2={t.x} y1={t.major ? 0 : 18} y2={TOP} class={t.major ? "tstick major" : "tstick"} /><Show when={t.l}><text x={t.x + 3} y={t.major ? 12 : 28} class="tsticklabel">{t.l}</text></Show></g>}</For>
            <Show when={d.status !== null}><line x1={g.x(d.status! + 1440)} x2={g.x(d.status! + 1440)} y1="0" y2={TOP} class="tsstatus" /></Show>
          </svg>
          <svg class="tsld" width={g.w} height={g.h - TOP} viewBox={`0 ${TOP} ${g.w} ${g.h - TOP}`}>
            <defs><marker id="tsa" viewBox="0 0 8 8" refX="7" refY="4" markerWidth="7" markerHeight="7" orient="auto"><path d="M0 0 L8 4 L0 8 z" class="tsarrow" /></marker>
              <marker id="tsac" viewBox="0 0 8 8" refX="7" refY="4" markerWidth="7" markerHeight="7" orient="auto"><path d="M0 0 L8 4 L0 8 z" class="tsarrow crit" /></marker></defs>
            <For each={d.bands}>{(b, k) => <g><rect x="0" y={TOP + b.lane * LANE} width={g.w} height={b.lanes * LANE} class={k() % 2 ? "tsband alt" : "tsband"} />
              <Show when={b.name}><text x="6" y={TOP + b.lane * LANE + 10} class="tsbandname">{b.name}</text></Show></g>}</For>
            <For each={g.ticks}>{t => <line x1={t.x} x2={t.x} y1={TOP} y2={g.h} class={t.major ? "tstick major" : "tstick"} />}</For>
            <Show when={d.status !== null}><line x1={g.x(d.status! + 1440)} x2={g.x(d.status! + 1440)} y1={TOP} y2={g.h} class="tsstatus" /></Show>
            <For each={d.links}>{l => { const p = g.at.get(l.from), s = g.at.get(l.to); if (!p || !s) return null;
              const fromEnd = l.kind === "FS" || l.kind === "FF", toEnd = l.kind === "FF" || l.kind === "SF";
              const x1 = g.x(fromEnd ? p.finish : p.start), x2 = g.x(toEnd ? s.finish : s.start), y1 = y(p.lane), y2 = y(s.lane);
              const mid = Math.max(x1 + 4, Math.min(x2 - 4, x1 + 8));
              const path = y1 === y2 ? `M${x1} ${y1} H${x2}` : `M${x1} ${y1} H${mid} V${y2} H${x2}`;
              return <path d={path} class="tslink" classList={{ crit: l.critical, loose: !l.driving }} marker-end={`url(#${l.critical ? "tsac" : "tsa"})`} />; }}</For>
            <For each={d.bars}>{b => { const x1 = g.x(b.start), x2 = Math.max(g.x(b.finish), x1 + 2), yy = y(b.lane); return (
              <g class="tsbar" onDblClick={() => open(b.uid)}><title>{`${b.code ? b.code + " " : ""}${b.name}\n${fmtDay(Math.floor(b.start / 1440))} – ${fmtDay(Math.floor((b.finish - 1) / 1440))}${b.critical ? "\nCritical" : ""}`}</title>
                {b.milestone ? <path d={`M${x1} ${yy - 6} L${x1 + 6} ${yy} L${x1} ${yy + 6} L${x1 - 6} ${yy} z`} class="tsms" classList={{ crit: b.critical }} />
                  : <><rect x={x1} y={yy - 4} width={x2 - x1} height="8" rx="2" class="tsrect" classList={{ crit: b.critical }} />
                    <Show when={b.pct > 0}><rect x={x1} y={yy - 2} width={(x2 - x1) * b.pct / 100} height="4" class="tsdone" /></Show></>}
                <Show when={names()}><text x={x1} y={yy - 8} class="tsname" classList={{ crit: b.critical }}>{b.code ? `${b.code} ` : ""}{b.name}</text></Show>
              </g>); }}</For>
          </svg>
        </div></>); }}</Show>
    </Page>
  );
};
