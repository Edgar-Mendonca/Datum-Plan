// Dashboard and reports: KPIs, S-curves, earned value, progress for a period, with branding.
import { For, Show, createSignal, createResource, JSX } from "solid-js";
import { Curve, Grid, Rule, fmtDay, money, dateDay, iso } from "./api";
import { call, out, P, tick, toast } from "./store";
import { C } from "./cmds";
import { Seg } from "./ui";

const q = (cols: string[], rules: Rule[] = [], extra: object = {}) => ({
  cols, filter: { name: "", rules, summaries: false, highlight: false }, group: [], sort: [], flat: false, collapsed: [], level: 0, hide_summaries: false, project_summary: false, baseline: 0, ...extra,
});
const r = (field: string, test: string, value = "", value2 = "", or = false): Rule => ({ field, test, value, value2, or });
const useRows = (key: () => any, query: () => object) => createResource(() => [tick(), JSON.stringify(key())], () => call<Grid>("query", { q: query() }));

// ---------- charts ----------
export const LineChart = (p: { x: number[]; series: { name: string; color: string; vals: number[]; dash?: boolean }[]; money?: boolean; h?: number; marker?: number | null }) => {
  const W = 640, H = p.h ?? 220, L = 64, B = 26;
  const max = () => Math.max(1, ...p.series.flatMap(s => s.vals));
  const px = (k: number) => L + (k / Math.max(1, p.x.length - 1)) * (W - L - 10);
  const py = (v: number) => H - B - (v / max()) * (H - B - 10);
  const fmt = (v: number) => (p.money ? money(v, { ...P().currency, decimals: 0 }) : String(Math.round(v)));
  const step = () => Math.max(1, Math.ceil(p.x.length / 8));
  return (
    <svg viewBox={`0 0 ${W} ${H}`} class="chartsvg">
      <For each={[0, 0.25, 0.5, 0.75, 1]}>{f => <><line x1={L} x2={W - 10} y1={py(max() * f)} y2={py(max() * f)} stroke="#E6EBF2" /><text x={L - 6} y={py(max() * f) + 4} text-anchor="end" class="ax">{fmt(max() * f)}</text></>}</For>
      <For each={p.x}>{(d, k) => <Show when={k() % step() === 0}><text x={px(k())} y={H - 8} text-anchor="middle" class="ax">{fmtDay(d)}</text></Show>}</For>
      {(() => { const k = p.marker != null ? p.x.findIndex(d => d >= p.marker!) : -1; return k >= 0 ? <line x1={px(k)} x2={px(k)} y1="6" y2={H - B} stroke="#2E9D57" stroke-dasharray="5 3" /> : null; })()}
      <For each={p.series}>{s => <polyline fill="none" stroke={s.color} stroke-width="2" stroke-dasharray={s.dash ? "5 3" : undefined} points={s.vals.map((v, k) => `${px(k)},${py(v)}`).join(" ")} />}</For>
      <For each={p.series}>{(s, k) => <><rect x={L + 8 + k() * 130} y="4" width="12" height="3" fill={s.color} /><text x={L + 24 + k() * 130} y="9" class="ax">{s.name}</text></>}</For>
    </svg>
  );
};
export const Bars = (p: { items: [string, number, string?][]; money?: boolean; max?: number }) => {
  const max = () => p.max ?? Math.max(1, ...p.items.map(i => i[1]));
  return <div class="hbars"><For each={p.items}>{([l, v, c]) => <div class="hb"><span title={l}>{l}</span><div><i style={{ width: `${(v / max()) * 100}%`, background: c ?? "var(--blue)" }} /></div><b>{p.money ? money(v, { ...P().currency, decimals: 0 }) : `${Math.round(v)}%`}</b></div>}</For></div>;
};
const cum = (v: number[]) => { let s = 0; return v.map(x => (s += x)); };

/** Report page header with logo and company, as printed. */
export const Brand = (p: { title: string; sub?: string }) => (
  <div class="brand">
    <Show when={P().logo}><img src={P().logo} alt="" /></Show>
    <div><h1>{p.title}</h1><div class="muted">{[P().name, P().company, P().client].filter(Boolean).join(" · ")}{p.sub ? ` · ${p.sub}` : ""}</div></div>
    <span class="grow" /><div class="muted r">Status date {out()!.summary.status || "not set"}<br />Printed {fmtDay(dateDay(new Date()), "long")}</div>
  </div>
);
const Kpi = (p: { v: JSX.Element; l: string; bad?: boolean; good?: boolean }) => <div class="kpi" classList={{ bad: p.bad, good: p.good }}><div class="v">{p.v}</div><div class="l">{p.l}</div></div>;
const Table = (p: { g?: Grid; empty: string }) => (
  <Show when={p.g?.rows.length} fallback={<p class="empty">{p.empty}</p>}>
    <table class="tbl"><thead><tr><For each={p.g!.cols}>{c => <th classList={{ r: ["Cost", "Num", "Pct", "Work", "Dur"].includes(c.kind) }}>{c.name}</th>}</For></tr></thead>
      <tbody><For each={p.g!.rows.slice(0, 200)}>{row => <tr classList={{ sumr: row.kind !== "task" }}><For each={row.cells}>{(c, k) => <td classList={{ r: ["Cost", "Num", "Pct", "Work", "Dur"].includes(p.g!.cols[k()].kind), late: row.late && k() === 0 }}>{c}</td>}</For></tr>}</For></tbody></table>
  </Show>
);

// ---------- dashboard ----------
export const Dashboard = () => {
  const s = () => out()!.summary;
  const [curve] = createResource(() => tick(), () => call<Curve>("curve", { unit: "week" }));
  const [top] = useRows(() => 1, () => q(["name", "pct", "cost", "finish"], [r("level", "equals", "1"), r("summary", "equals", "Yes")]));
  const [late] = useRows(() => 2, () => q(["name", "finish", "pct", "res"], [r("late", "equals", "Yes"), r("summary", "equals", "No")]));
  const [ms] = useRows(() => 3, () => q(["name", "finish", "b0finish", "vfinish"], [r("milestone", "equals", "Yes")]));
  return (
    <section class="page report">
      <div class="ptools noprint"><span class="grow" /><button class="btn" onClick={() => C.print()}>Print / PDF</button><button class="btn" onClick={() => saveHtml("Dashboard")}>Save as HTML</button></div>
      <Brand title="Project Dashboard" />
      <Show when={s().error}><div class="panel late">{s().error}</div></Show>
      <div class="kpis">
        <Kpi v={`${Math.round(s().pct)}%`} l="Complete" /><Kpi v={s().finish} l={`Finish${s().var_finish ? ` (${s().var_finish} vs baseline)` : ""}`} /><Kpi v={s().cost} l="Cost" />
        <Kpi v={s().spi?.toFixed(2) ?? "—"} l="SPI (schedule)" bad={s().spi !== null && s().spi! < 0.95} good={s().spi !== null && s().spi! >= 1} />
        <Kpi v={s().cpi?.toFixed(2) ?? "—"} l="CPI (cost)" bad={s().cpi !== null && s().cpi! < 0.95} good={s().cpi !== null && s().cpi! >= 1} />
        <Kpi v={s().late} l="Late tasks" bad={s().late > 0} /><Kpi v={`${s().done} / ${s().tasks}`} l="Tasks done" /><Kpi v={s().critical} l="Critical tasks" />
      </div>
      <div class="cols">
        <div class="panel"><h2>Cost S-curve (cumulative)</h2>
          <Show when={curve()}>{c => <LineChart x={c().buckets} money marker={s().status_day} series={[
            { name: "Planned (baseline)", color: "#9AA4B2", vals: cum(c().planned), dash: true }, { name: "Forecast", color: "#2F6FCF", vals: cum(c().forecast) },
            { name: "Earned", color: "#2E9D57", vals: cum(c().earned) }, { name: "Actual", color: "#D64545", vals: cum(c().actual) }]} />}</Show>
          <Show when={!P().snapshots.length && curve() && !curve()!.planned.some(x => x)}><p class="muted">Set a baseline (Project › Set Baseline) to compare planned and earned value.</p></Show>
        </div>
        <div class="panel"><h2>Progress by phase</h2><Bars items={(top()?.rows ?? []).map(x => [x.name, parseFloat(x.cells[1]) || 0] as [string, number])} /></div>
        <div class="panel"><h2>Cost by phase</h2><Bars money items={(top()?.rows ?? []).map(x => [x.name, parseFloat(x.cells[2].replace(/[^\d.-]/g, "")) || 0, "#6E9FE6"] as [string, number, string])} /></div>
        <div class="panel"><h2>Milestones</h2><Table g={ms()} empty="No milestones." /></div>
        <div class="panel"><h2>Late tasks</h2><Table g={late()} empty="Nothing is late." /></div>
      </div>
    </section>
  );
};

// ---------- reports gallery ----------
const REPORTS: [string, string, string][] = [
  ["overview", "Project Overview", "Schedule, cost, milestones and late work at a glance."],
  ["progress", "Progress Report", "What was done and what is due in a week or month, with photos of the plan's health."],
  ["ev", "Earned Value", "Planned, earned and actual cost over time, SPI, CPI, EAC and the EV table."],
  ["cost", "Cost Overview", "Cash flow per period, cumulative cost and cost by phase."],
  ["work", "Work Overview", "Hours per resource, remaining work and overallocation."],
  ["critical", "Critical Tasks", "The tasks that drive the finish date."],
  ["late", "Late & Slipping Tasks", "Late tasks and tasks behind their baseline."],
  ["milestones", "Milestone Report", "Every milestone with baseline and variance."],
  ["upcoming", "Upcoming Tasks", "Tasks starting or finishing in the next period."],
  ["trend", "Trend (Snapshots)", "Finish date, complete and cost across stored snapshots."],
];
export const Reports = () => {
  const [pick, setPick] = createSignal<string | null>(null);
  return (
    <Show when={pick()} fallback={
      <section class="page"><div class="ptools"><h1>Reports</h1></div>
        <div class="tiles"><For each={REPORTS}>{([k, n, d]) => <button class="tile" onClick={() => setPick(k)}><div class="pic"><svg class="ic"><use href="#i-report" /></svg></div><b>{n}</b><span>{d}</span></button>}</For></div>
      </section>}>
      {k => <Report kind={k()} back={() => setPick(null)} />}
    </Show>
  );
};

const Report = (p: { kind: string; back: () => void }) => {
  const s = () => out()!.summary;
  const name = () => REPORTS.find(x => x[0] === p.kind)![1];
  const [period, setPeriod] = createSignal("week");
  const status = () => s().status_day ?? dateDay(new Date()) + 1;
  const from = () => status() - (period() === "week" ? 7 : 30), to = () => status() + (period() === "week" ? 7 : 30);
  const [curve] = createResource(() => [tick(), p.kind], () => call<Curve>("curve", { unit: p.kind === "ev" || p.kind === "cost" ? (s().f - s().s > 200 ? "month" : "week") : "week" }));
  const cols: Record<string, string[]> = {
    critical: ["code", "name", "dur", "start", "finish", "tf", "res"], late: ["code", "name", "finish", "b0finish", "vfinish", "pct", "res"],
    milestones: ["code", "name", "finish", "b0finish", "vfinish", "pct"], upcoming: ["code", "name", "start", "finish", "dur", "res"],
    ev: ["name", "bcws", "bcwp", "acwp", "sv", "cv", "spi", "cpi", "bac", "eac", "vac"], cost: ["name", "cost", "b0cost", "vcost", "acost", "rcost"],
    work: ["name", "work", "b0work", "awork", "rwork", "workpct"], overview: ["name", "start", "finish", "pct", "cost"],
  };
  const rules = (): Rule[] => ({
    critical: [r("critical", "equals", "Yes"), r("summary", "equals", "No")], late: [r("late", "equals", "Yes"), r("vfinish", "is greater than", "0", "", true)],
    milestones: [r("milestone", "equals", "Yes")], upcoming: [r("start", "is within", iso(status()), iso(to())), r("finish", "is within", iso(status()), iso(to()), true)],
    overview: [r("level", "equals", "1")], ev: [r("level", "is less than or equal to", "2")], cost: [r("level", "is less than or equal to", "2")], work: [r("level", "is less than or equal to", "2")],
  } as Record<string, Rule[]>)[p.kind] ?? [];
  const [g] = useRows(() => [p.kind, period()], () => q(cols[p.kind] ?? ["name"], rules()));
  const [done] = useRows(() => ["done", period()], () => q(["code", "name", "afinish", "pct", "res"], [r("afinish", "is within", iso(from()), iso(status()))]));
  const [prog] = useRows(() => ["prog", period()], () => q(["code", "name", "start", "finish", "pct", "rem"], [r("pct", "is greater than", "0"), r("pct", "is less than", "100"), r("summary", "equals", "No")]));
  const [due] = useRows(() => ["due", period()], () => q(["code", "name", "start", "finish", "res"], [r("start", "is within", iso(status()), iso(to())), r("summary", "equals", "No"), r("pct", "equals", "0")]));
  const [usage] = createResource(() => [tick(), p.kind], () => (p.kind === "work" ? call<[number, boolean, string[]][]>("resources", { cols: ["name", "work", "cost", "over", "max"] }) : undefined));
  return (
    <section class="page report">
      <div class="ptools noprint"><button class="btn" onClick={p.back}>‹ All reports</button><span class="grow" />
        <Show when={p.kind === "progress" || p.kind === "upcoming"}><Seg options={[["week", "Weekly"], ["month", "Monthly"]]} value={period()} set={setPeriod} /></Show>
        <button class="btn" onClick={() => C.print()}>Print / PDF</button><button class="btn" onClick={() => saveHtml(name())}>Save as HTML</button></div>
      <Brand title={p.kind === "progress" ? `${period() === "week" ? "Weekly" : "Monthly"} Progress Report` : name()} sub={p.kind === "progress" ? `${fmtDay(from(), "long")} – ${fmtDay(status() - 1, "long")}` : undefined} />
      <div class="kpis">
        <Kpi v={`${Math.round(s().pct)}%`} l="Complete" /><Kpi v={s().start} l="Start" /><Kpi v={s().finish} l="Finish" /><Kpi v={s().var_finish || "—"} l="Finish variance" />
        <Kpi v={s().cost} l="Cost" /><Kpi v={s().spi?.toFixed(2) ?? "—"} l="SPI" bad={(s().spi ?? 1) < 0.95} /><Kpi v={s().cpi?.toFixed(2) ?? "—"} l="CPI" bad={(s().cpi ?? 1) < 0.95} /><Kpi v={s().late} l="Late tasks" bad={s().late > 0} />
      </div>
      <Show when={p.kind === "progress"}>
        <div class="panel"><h2>Completed this {period()}</h2><Table g={done()} empty="Nothing finished in this period." /></div>
        <div class="panel"><h2>In progress</h2><Table g={prog()} empty="No tasks in progress." /></div>
        <div class="panel"><h2>Starting next {period()}</h2><Table g={due()} empty="Nothing starts in the next period." /></div>
      </Show>
      <Show when={(p.kind === "ev" || p.kind === "cost") && curve()}>{cv => {
        const c = cv();
        return <div class="panel"><h2>{p.kind === "ev" ? "Earned value over time" : "Cumulative cost"}</h2>
          <LineChart x={c.buckets} money marker={s().status_day} series={p.kind === "ev"
            ? [{ name: "Planned value (PV)", color: "#9AA4B2", vals: cum(c.planned), dash: true }, { name: "Earned value (EV)", color: "#2E9D57", vals: cum(c.earned) }, { name: "Actual cost (AC)", color: "#D64545", vals: cum(c.actual) }]
            : [{ name: "Baseline", color: "#9AA4B2", vals: cum(c.planned), dash: true }, { name: "Forecast", color: "#2F6FCF", vals: cum(c.forecast) }]} />
          <Show when={p.kind === "cost"}><h2>Cash flow per period</h2><div class="cash"><For each={c.forecast}>{(v, k) => <div title={`${fmtDay(c.buckets[k()])}: ${money(v, P().currency)}`}><i style={{ height: `${(v / Math.max(1, ...c.forecast)) * 100}%` }} /></div>}</For></div></Show>
        </div>;
      }}</Show>
      <Show when={p.kind === "work" && usage()}><div class="panel"><h2>Resources</h2><Bars items={usage()!.map(x => [x[2][0], parseFloat(x[2][1]) || 0, x[1] ? "#D64545" : "#6E9FE6"] as [string, number, string])} max={Math.max(1, ...usage()!.map(x => parseFloat(x[2][1]) || 0))} /><p class="muted">Bars are total hours; red resources are overallocated.</p></div></Show>
      <Show when={p.kind === "trend"}>
        <div class="panel"><h2>Snapshots</h2>
          <Show when={P().snapshots.length} fallback={<p class="empty">No snapshots yet. Use Project › Store Period Performance each reporting period.</p>}>
            <LineChart x={P().snapshots.map(x => (x.date ? dateDay(new Date(x.date + "T00:00")) : 0))} series={[{ name: "% complete", color: "#2E9D57", vals: P().snapshots.map(x => x.pct) }]} />
            <table class="tbl"><thead><tr><th>Date</th><th>Label</th><th>Finish</th><th class="r">Complete</th><th class="r">Cost</th><th>Comment</th></tr></thead>
              <tbody><For each={P().snapshots}>{x => <tr><td>{x.date}</td><td>{x.label}</td><td>{fmtDay(x.finish / 1440 - 0.01, "long")}</td><td class="r">{Math.round(x.pct)}%</td><td class="r">{money(x.cost, P().currency)}</td><td>{x.comment}</td></tr>}</For></tbody></table>
          </Show></div>
      </Show>
      <Show when={cols[p.kind]}><div class="panel"><h2>{name()}</h2><Table g={g()} empty="No tasks match." /></div></Show>
    </section>
  );
};

/** Saves the visible report as a standalone HTML file with its styles. */
export const saveHtml = async (title: string) => {
  const el = document.querySelector(".report");
  if (!el) return;
  const css = [...document.styleSheets].flatMap(s => { try { return [...s.cssRules].map(r => r.cssText); } catch { return []; } }).join("\n");
  const html = `<!doctype html><html><head><meta charset="utf-8"><title>${title}</title><style>${css}\n.noprint{display:none}body{overflow:auto;height:auto}</style></head><body>${el.outerHTML}</body></html>`;
  const path = await call<string | null>("save_file", { name: `${P().name} - ${title}.html`, data: html });
  if (path) toast(`Saved ${path}`, true);
};
