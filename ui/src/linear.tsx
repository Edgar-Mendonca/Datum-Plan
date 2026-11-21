// The time-location (linear) view: chainage across, time down. Each task with a Chainage From/To is
// drawn from where it starts at its start to where it ends at its finish (linear.rs): a crew moving
// along the route is a sloped line, work at one place a vertical bar. Double-click to show it in the Gantt chart.
import { For, Show, createMemo, createResource } from "solid-js";
import { call, out, tick, setSel, setView, kept } from "./store";
import { Page } from "./views";
import { Seg } from "./ui";
import { dayDate, fmtDay } from "./api";

type Item = { uid: number; id: number; code: string; name: string; from: number; to: number; start: number; finish: number; critical: boolean; pct: number };
type Linear = { items: Item[]; lo: number; hi: number; start: number; finish: number; status: number | null; missing: number };
const [unit, setUnit] = kept("lin-unit", "km");
const [scale, setScale] = kept("lin-scale", "8");
const W = 980, LEFT = 70, TOP = 34;

/** A round step for about `n` ticks across `span`. */
const niceStep = (span: number, n: number) => { const raw = span / n, p = 10 ** Math.floor(Math.log10(raw || 1)); return [1, 2, 5, 10].map(m => m * p).find(s => s >= raw) ?? raw; };

export const LinearView = () => {
  const [data] = createResource(() => [tick(), out()?.tab] as const, () => call<Linear>("linear_view", {}));
  const geo = createMemo(() => {
    const d = data(); if (!d || !d.items.length) return null;
    const pad = Math.max((d.hi - d.lo) * 0.04, 0.5), lo = d.lo - pad, hi = d.hi + pad;
    const a = Math.floor(d.start / 1440) - 2, b = Math.ceil(d.finish / 1440) + 3, ppd = +scale();
    const x = (c: number) => LEFT + ((c - lo) / (hi - lo)) * (W - LEFT - 20), y = (t: number) => TOP + (t / 1440 - a) * ppd;
    const step = niceStep(hi - lo, 10), cticks: number[] = [];
    for (let c = Math.ceil(lo / step) * step; c <= hi; c += step) cticks.push(+c.toFixed(6));
    const tticks: { y: number; l: string; major: boolean }[] = [];
    for (let day = a; day <= b; day++) {
      const dt = dayDate(day);
      if (dt.getDate() === 1) tticks.push({ y: y(day * 1440), l: fmtDay(day, "month"), major: true });
      else if (ppd >= 6 && dt.getDay() === 1 && dt.getDate() > 2) tticks.push({ y: y(day * 1440), l: fmtDay(day), major: false });
    }
    return { h: TOP + (b - a) * ppd + 20, x, y, cticks, tticks };
  });
  const open = (uid: number) => { setSel([uid]); setView("gantt"); };
  return (
    <Page title="Time-Location" help="network" tools={<>
      <label class="muted">Unit <input value={unit()} style={{ width: "48px" }} onChange={e => setUnit(e.currentTarget.value.trim() || "km")} /></label>
      <Seg value={scale()} set={setScale} options={[["3", "Months"], ["8", "Weeks"], ["24", "Days"]]} />
    </>}>
      <Show when={data()}>{d => <Show when={geo()} keyed fallback={<p class="empty">No task has a chainage yet. Add the <b>Chainage From</b> and <b>Chainage To</b> columns (right-click a column heading › Insert Column) and type where each task starts and ends along the route, such as 12.5 or 12+500.</p>}>{g => (
        <div class="tsldwrap">
          <svg class="linear" width={W} height={g.h} viewBox={`0 0 ${W} ${g.h}`}>
            <For each={g.cticks}>{c => <g><line x1={g.x(c)} x2={g.x(c)} y1={TOP - 4} y2={g.h} class="tstick" /><text x={g.x(c)} y={TOP - 10} class="tsticklabel" text-anchor="middle">{c}</text></g>}</For>
            <text x={LEFT} y={10} class="tsticklabel">Chainage ({unit()})</text>
            <For each={g.tticks}>{t => <g><line x1={LEFT} x2={W - 20} y1={t.y} y2={t.y} class={t.major ? "tstick major" : "tstick"} /><text x={LEFT - 6} y={t.y + 4} class="tsticklabel" text-anchor="end">{t.l}</text></g>}</For>
            <Show when={d().status !== null}><line x1={LEFT} x2={W - 20} y1={g.y(d().status! + 1440)} y2={g.y(d().status! + 1440)} class="tsstatus" /></Show>
            <For each={d().items}>{it => { const x1 = g.x(it.from), x2 = g.x(it.to), y1 = g.y(it.start), y2 = g.y(it.finish), place = Math.abs(x2 - x1) < 2; return (
              <g class="tsbar" onDblClick={() => open(it.uid)}><title>{`${it.code ? it.code + " " : ""}${it.name}\n${it.from} – ${it.to} ${unit()}\n${fmtDay(Math.floor(it.start / 1440))} – ${fmtDay(Math.floor((it.finish - 1) / 1440))}${it.critical ? "\nCritical" : ""}`}</title>
                {place ? <rect x={x1 - 4} y={y1} width="8" height={Math.max(2, y2 - y1)} class="tsrect" classList={{ crit: it.critical }} />
                  : <line x1={x1} y1={y1} x2={x2} y2={y2} class="linline" classList={{ crit: it.critical }} />}
                <text x={Math.min(x1, x2) + (place ? 8 : 4)} y={(y1 + y2) / 2} class="tsname" classList={{ crit: it.critical }}>{it.code ? `${it.code} ` : ""}{it.name}</text>
              </g>); }}</For>
          </svg>
          <p class="muted tslegend"><span class="tskey crit" /> critical <span class="tskey" /> other work — a sloped line moves along the route, a bar stays in one place.{d().missing ? ` ${d().missing} tasks without a chainage are not shown.` : ""} Double-click to show a task in the Gantt chart.</p>
        </div>)}</Show>}</Show>
    </Page>
  );
};
