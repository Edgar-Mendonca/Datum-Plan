// Schedule Risk: a Monte Carlo analysis of the plan. The engine (crates/core/src/risk.rs) runs the
// plan many times with every unfinished task's duration drawn from its range and each open risk
// happening with its probability; this page shows when the project is likely to finish and why.
import { For, Show, createSignal } from "solid-js";
import { SimResult, fmtAt, money } from "./api";
import { call, P, setView, act } from "./store";
import { selectUid } from "./cmds";
import { Page } from "./views";
import { Kpi } from "./reports";
import { ChartBox, bar, line, COL } from "./charts";
import { fmtDay } from "./api";

const [res, setRes] = createSignal<SimResult | null>(null);
/** The last run is kept with the plan (project.ui.risk), so it is still there after reopening and prints. */
const saved = () => (P().ui?.risk?.result ? (P().ui.risk as { result: SimResult; at: string; opts: { iterations: number; low: number; high: number } }) : null);
const shown = () => res() ?? saved()?.result ?? null;
const [busy, setBusy] = createSignal(false);
const [opts, setOpts] = createSignal({ iterations: 1000, low: 10, high: 25, seed: 1, dist: "tri", corr_by: "", corr: 0.5, target_date: "", target_cost: 0 });

/** Finish against cost for each run, with the targets: the runs in the lower-left box meet both. */
const Scatter = (p: { r: SimResult }) => {
  const W = 560, H = 260, L = 70, B = 34, pts = () => p.r.scatter ?? [];
  const xs = () => pts().map(q => q[0]), ys = () => pts().map(q => q[1]);
  const tx = () => Math.floor(((p.r.target_finish ?? p.r.planned) - 1) / 1440), ty = () => p.r.target_cost ?? p.r.cost;
  const x0 = () => Math.min(...xs(), tx()) - 1, x1 = () => Math.max(...xs(), tx()) + 1, y0 = () => Math.min(...ys(), ty()) * 0.98, y1 = () => Math.max(...ys(), ty()) * 1.02 || 1;
  const X = (v: number) => L + (v - x0()) / Math.max(1e-9, x1() - x0()) * (W - L - 10), Y = (v: number) => H - B - (v - y0()) / Math.max(1e-9, y1() - y0()) * (H - B - 10);
  return <svg class="scatter" viewBox={`0 0 ${W} ${H}`} role="img" aria-label="Finish against cost for each run">
    <rect x={L} y={Y(ty())} width={Math.max(0, X(tx()) - L)} height={Math.max(0, H - B - Y(ty()))} class="ok-box" />
    <For each={pts()}>{q => <circle cx={X(q[0])} cy={Y(q[1])} r="2.2" class={q[0] <= tx() && q[1] <= ty() + 1e-6 ? "in" : "out"} />}</For>
    <line x1={X(tx())} x2={X(tx())} y1="6" y2={H - B} class="target" /><line x1={L} x2={W - 6} y1={Y(ty())} y2={Y(ty())} class="target" />
    <line x1={L} x2={W - 6} y1={H - B} y2={H - B} class="axis" /><line x1={L} x2={L} y1="6" y2={H - B} class="axis" />
    <text x={X(tx())} y={H - B + 14} class="lab" text-anchor="middle">{fmtDay(tx())}</text><text x={L - 6} y={Y(ty()) + 4} class="lab" text-anchor="end">{cur(ty())}</text>
    <text x={(L + W) / 2} y={H - 4} class="lab" text-anchor="middle">Finish</text><text x="12" y={(H - B) / 2} class="lab" transform={`rotate(-90 12 ${(H - B) / 2})`} text-anchor="middle">Cost</text>
  </svg>;
};

export const runRisk = async () => {
  setBusy(true);
  try {
    const r = await call<SimResult>("risk_run", { opts: opts() });
    if (r) { setRes(r); act({ Ui: { key: "risk", value: { result: r, at: new Date().toISOString().slice(0, 16).replace("T", " "), opts: opts() } } }); }
  } finally { setBusy(false); }
};

const pct = (v: number) => `${Math.round(v * 100)}%`;
const cur = (v: number) => money(v, { ...P().currency, decimals: 0 });

export const ScheduleRisk = () => {
  const num = (k: "iterations" | "low" | "high", v: string) => setOpts({ ...opts(), [k]: Math.max(0, +v || 0) });
  return (
    <Page title="Schedule Risk" help="simrisk" tools={<>
      <label class="muted">Runs <select value={opts().iterations} onChange={e => num("iterations", e.currentTarget.value)}><For each={[500, 1000, 2000, 5000]}>{n => <option value={n}>{n.toLocaleString()}</option>}</For></select></label>
      <label class="muted" title="Tasks without their own optimistic and pessimistic durations vary between these limits.">Default range −<input class="num" type="number" min="0" max="100" value={opts().low} onChange={e => num("low", e.currentTarget.value)} />% to +<input class="num" type="number" min="0" value={opts().high} onChange={e => num("high", e.currentTarget.value)} />%</label>
      <label class="muted" title="Triangular: the planned duration is the most likely. Uniform: any value in the range is as likely.">Draw <select value={opts().dist} onChange={e => setOpts({ ...opts(), dist: e.currentTarget.value })}><option value="tri">Triangular</option><option value="uniform">Uniform</option></select></label>
      <label class="muted" title="Tasks that move together (the same crew, the same weather): all tasks, or the tasks that share a value of an activity code.">Move together <select value={opts().corr_by} onChange={e => setOpts({ ...opts(), corr_by: e.currentTarget.value })}><option value="">None</option><option value="all">All tasks</option><For each={P().codes}>{c => <option value={c.name}>By {c.name}</option>}</For></select>
        <Show when={opts().corr_by}> <input class="num" type="number" min="0" max="0.99" step="0.1" value={opts().corr} onChange={e => setOpts({ ...opts(), corr: Math.min(0.99, Math.max(0, +e.currentTarget.value || 0)) })} /></Show></label>
      <label class="muted" title="The date and budget to meet; empty uses the plan's finish and cost.">Targets <input type="date" value={opts().target_date} onChange={e => setOpts({ ...opts(), target_date: e.currentTarget.value })} /> <input class="num" style={{ width: "90px" }} type="number" min="0" placeholder="Cost" value={opts().target_cost || ""} onChange={e => setOpts({ ...opts(), target_cost: Math.max(0, +e.currentTarget.value || 0) })} /></label>
      <button class="btn pri" disabled={busy()} onClick={runRisk}>{busy() ? "Running…" : res() ? "Run again" : "Run analysis"}</button>
    </>}>
      <Show when={!res() && saved()}><p class="muted">Results of the run on {saved()!.at} ({saved()!.opts.iterations.toLocaleString()} runs, −{saved()!.opts.low}% to +{saved()!.opts.high}%). Run again after changing the plan.</p></Show>
      <Show when={shown()} fallback={
        <div class="panel"><p>Schedule Risk runs the plan {opts().iterations.toLocaleString()} times. In each run every unfinished task takes a duration between its <b>optimistic</b> and <b>pessimistic</b> duration (most often its planned one), and each open risk in the <a class="link" onClick={() => setView("risks")}>Risk Register</a> with a <b>Delay (days)</b> happens with its probability (1 = 10 %, 2 = 30 %, 3 = 50 %, 4 = 70 %, 5 = 90 %).</p>
          <p class="muted">Add the Optimistic Duration and Pessimistic Duration columns to the task sheet (right-click a column heading › Insert Column) to give tasks their own range. Tasks without one use the default range above.</p>
          <p><button class="btn pri" disabled={busy()} onClick={runRisk}>{busy() ? "Running…" : "Run analysis"}</button></p></div>
      }>{r => {
        const total = () => r().runs;
        let acc = 0;
        const cum = () => { acc = 0; return r().days.map(([, n]) => (acc += n) / total() * 100); };
        const mark = () => r().days.findIndex(([d]) => d >= Math.floor((r().planned - 1) / 1440));
        return <>
          <div class="kpis">
            <Kpi v={fmtAt(r().planned, true)} l={`Planned finish: ${pct(r().planned_chance)} chance`} bad={r().planned_chance < 0.5} good={r().planned_chance >= 0.8} />
            <Kpi v={fmtAt(r().p50, true)} l="P50: as likely early as late" />
            <Kpi v={fmtAt(r().p80, true)} l="P80: 8 in 10 runs finish by then" />
            <Kpi v={fmtAt(r().p90, true)} l="P90" />
            <Kpi v={cur(r().cost_p50)} l={`Cost P50 (plan ${cur(r().cost)})`} />
            <Kpi v={cur(r().cost_p80)} l="Cost P80" />
            <Show when={r().joint !== undefined}><Kpi v={pct(r().joint!)} l={`Finish by ${fmtAt(r().target_finish ?? r().planned, true)} and cost within ${cur(r().target_cost ?? r().cost)}`} bad={r().joint! < 0.5} good={r().joint! >= 0.8} /></Show>
          </div>
          <div class="cols riskcharts">
            <div class="panel" style={{ "grid-column": "span 2" }}>
              <ChartBox title="Finish date: share of runs per day and cumulative" type="bar" fmt="pct" y="Runs" days={r().days.map(([d]) => d)} labels={r().days.map(([d]) => fmtDay(d))}
                datasets={[bar("Finished that day", r().days.map(([, n]) => n / total() * 100), COL.sky), line("Finished by then (cumulative)", cum(), COL.blue)]}
                marker={mark()} markerLabel="Planned" />
            </div>
            <div class="panel">
              <Show when={r().drivers.length} fallback={<p class="empty">No task's duration moves the finish much.</p>}>
                <ChartBox title="What drives the finish" type="bar" horizontal fmt="num" labels={r().drivers.map(d => d.name)} datasets={[bar("Correlation with the finish", r().drivers.map(d => Math.round(d.value * 100) / 100), COL.orange)]} />
              </Show>
              <p class="muted">The longer these tasks take, the later the project finishes (1 = always together).</p>
            </div>
          </div>
          <Show when={(r().scatter ?? []).length}>
            <div class="panel"><h2>Joint confidence: finish and cost together</h2>
              <Scatter r={r()} />
              <p class="muted">Each dot is a run. The shaded box holds the runs that meet both targets: {pct(r().joint ?? 0)} of them; the date alone {pct(r().finish_chance ?? 0)}, the cost alone {pct(r().cost_chance ?? 0)}.</p></div>
          </Show>
          <div class="cols">
            <div class="panel"><h2>Criticality: how often each task was critical</h2>
              <table class="tbl"><tbody><For each={r().criticality}>{t => <tr class="click" onClick={() => { setView("gantt"); selectUid(t.uid); }}><td>{t.name}</td><td class="r"><span class="meter"><i style={{ width: pct(t.value) }} /></span> {pct(t.value)}</td></tr>}</For></tbody></table>
            </div>
            <Show when={r().risks.length}><div class="panel"><h2>Risks in the analysis</h2>
              <table class="tbl"><tbody><For each={r().risks}>{t => <tr><td>R{t.uid} {t.name}</td><td class="r">happened in {pct(t.value)} of runs</td></tr>}</For></tbody></table>
            </div></Show>
            <div class="panel"><h2>About this run</h2><ul class="muted"><li>{r().runs.toLocaleString()} runs; the same settings give the same answer.</li><For each={r().notes}>{n => <li>{n}</li>}</For></ul></div>
          </div>
        </>;
      }}</Show>
    </Page>
  );
};
