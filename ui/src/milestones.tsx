// The Milestones view: every milestone against its baselines, deadline and contract date, with the
// float left, how far it moved since the last period close and a status in words (milestones.rs),
// and a chart of each milestone's baselines and forecast on one time axis.
import { For, Show, createMemo, createResource, createSignal } from "solid-js";
import { call, out, tick, setSel, setView, kept, BL_COLOURS } from "./store";
import { Page } from "./views";
import { Kpi } from "./reports";
import { Seg } from "./ui";
import { dt } from "./regrid";
import { milestoneTrend } from "./dialogs";

type MsRow = { uid: number; id: number; code: string; name: string; phase: string; forecast: string | null; actual: string | null; done: boolean; baselines: [number, string][]; var: number | null; deadline: string | null; contract: string; contract_date: string | null; float_days: number | null; critical: boolean; moved: number | null; trend: [string, string][]; status: string; key: string };
type MsOut = { rows: MsRow[]; baselines: number[]; day: string | null; achieved: number; late: number; slipping: number; at_risk: number };
const KEY: Record<string, string> = { ok: "good", warn: "warn", bad: "bad" };
const [base, setBase] = kept("ms-base", 0);
const [show, setShow] = kept("ms-show", "all");
const blName = (n: number) => (n ? `Baseline ${n}` : "Baseline");

export const MilestonesView = () => {
  const [data] = createResource(() => [tick(), out()?.tab, base()] as const, () => call<MsOut>("milestones", { base: base() }));
  const [q, setQ] = createSignal("");
  const rows = createMemo(() => {
    const t = q().toLowerCase(), d = data();
    if (!d) return [];
    const soon = d.day ? Date.parse(d.day) + 42 * 864e5 : Infinity;
    return d.rows.filter(r => (!t || `${r.code} ${r.name} ${r.phase}`.toLowerCase().includes(t))
      && (show() === "all" || (show() === "open" && !r.done) || (show() === "issues" && ["bad", "warn"].includes(r.key) && !r.done) || (show() === "soon" && !r.done && r.forecast !== null && Date.parse(r.forecast) <= soon)));
  });
  const open = (r: MsRow) => { setSel([r.uid]); setView("gantt"); };
  const bl = (r: MsRow, n: number) => r.baselines.find(b => b[0] === n)?.[1] ?? null;
  // The chart's time axis: every date shown, with a little room.
  const axis = createMemo(() => {
    const ds = rows().flatMap(r => [r.forecast, r.actual, r.deadline, r.contract_date, ...r.baselines.map(b => b[1])]).filter(Boolean).map(d => Date.parse(d!));
    if (data()?.day) ds.push(Date.parse(data()!.day!));
    if (!ds.length) return null;
    const a = Math.min(...ds) - 7 * 864e5, b = Math.max(...ds) + 7 * 864e5;
    return { a, b, x: (d: string) => ((Date.parse(d) - a) / (b - a)) * 100 };
  });
  const months = createMemo(() => {
    const ax = axis(); if (!ax) return [];
    const out: { x: number; l: string }[] = [], d = new Date(ax.a); d.setDate(1); d.setMonth(d.getMonth() + 1);
    while (d.getTime() < ax.b && out.length < 60) { out.push({ x: ((d.getTime() - ax.a) / (ax.b - ax.a)) * 100, l: d.toLocaleDateString("en-GB", { month: "short", year: "2-digit" }) }); d.setMonth(d.getMonth() + 1); }
    return out;
  });
  return (
    <Page title="Milestones" help="milestones" tools={<>
      <label class="muted">Compare with <select value={String(base())} onChange={e => setBase(+e.currentTarget.value)}>
        <For each={data()?.baselines.length ? data()!.baselines : [0]}>{n => <option value={String(n)} selected={n === base()}>{blName(n)}</option>}</For></select></label>
      <Seg value={show()} set={setShow} options={[["all", "All"], ["open", "Open"], ["issues", "Late or slipping"], ["soon", "Next 6 weeks"]]} />
      <input class="q" placeholder="Search milestones…" value={q()} onInput={e => setQ(e.currentTarget.value)} />
      <button class="btn" onClick={milestoneTrend} title="Each milestone's forecast at every period close">Trend chart…</button>
    </>}>
      <Show when={data()}>{d => <>
        <div class="kpis">
          <Kpi v={String(d().rows.length)} l={`milestones · ${d().achieved} achieved`} />
          <Kpi v={String(d().late)} l="late for their baseline, deadline or contract date" bad={d().late > 0} />
          <Kpi v={String(d().slipping)} l="moved later since the last period close" bad={d().slipping > 0} />
          <Kpi v={String(d().at_risk)} l="at risk: critical with little float" />
        </div>
        <Show when={!d().baselines.length}><p class="dwarn">No baseline yet: set one (Progress › Set Baseline) to measure the milestones against it.</p></Show>
        <div class="panel ctr mstable">
          <table class="tbl"><thead><tr><th>ID</th><th>Milestone</th><th>Phase</th>
            <For each={d().baselines}>{n => <th classList={{ on: n === base() }} title={n === base() ? "Compared with" : ""}><i class="blkey" style={{ background: BL_COLOURS[n % BL_COLOURS.length] }} />{blName(n)}</th>}</For>
            <th>Forecast / actual</th><th class="r" title={`Against ${blName(base())}, calendar days`}>Var.</th><th>Deadline</th><th>Contract date</th><th class="r" title="Total float, working days">Float</th><th class="r" title="Days moved since the last period close">Moved</th><th>Status</th></tr></thead>
            <tbody><For each={rows()} fallback={<tr><td colspan="12" class="muted">No milestones here.</td></tr>}>{r => <tr onDblClick={() => open(r)} title="Double-click to show it in the Gantt chart" classList={{ crit: r.critical && !r.done }}>
              <td class="mono">{r.code || r.id}</td><td><b>{r.name}</b></td><td class="muted">{r.phase}</td>
              <For each={d().baselines}>{n => <td classList={{ on: n === base() }}>{dt(bl(r, n))}</td>}</For>
              <td>{r.done ? <span title="Actual">✓ {dt(r.actual ?? r.forecast)}</span> : dt(r.forecast)}</td>
              <td class="r" classList={{ late: (r.var ?? 0) > 0 }}>{r.var === null ? "" : r.var > 0 ? `+${r.var}` : r.var}</td>
              <td classList={{ late: !!r.deadline && !!r.forecast && r.forecast > r.deadline && !r.done }}>{dt(r.deadline)}</td>
              <td title={r.contract}>{r.contract_date ? `${dt(r.contract_date)} · ${r.contract}` : ""}</td>
              <td class="r" classList={{ late: (r.float_days ?? 0) < 0 }}>{r.float_days ?? ""}</td>
              <td class="r" classList={{ late: (r.moved ?? 0) > 0 }}>{r.moved ? (r.moved > 0 ? `+${r.moved}` : r.moved) : ""}</td>
              <td><span class={`st ${KEY[r.key] ?? ""}`}>{r.status}</span></td>
            </tr>}</For></tbody></table>
        </div>
        <Show when={axis() && rows().length}>
          <div class="panel ctr mschart"><h2>Baselines and forecast</h2>
            <p class="muted mslegend"><For each={d().baselines}>{n => <span><i class="blkey" style={{ background: BL_COLOURS[n % BL_COLOURS.length] }} />{blName(n)}</span>}</For>
              <span><i class="msdia" /> forecast</span><span><i class="msdia done" /> achieved</span><span><i class="msdia late" /> late</span><span>▏ deadline or contract date</span></p>
            <div class="msgrid">
              <div class="msaxis"><For each={months()}>{m => <span style={{ left: `${m.x}%` }}>{m.l}</span>}</For>
                <Show when={d().day}><span class="mstoday" style={{ left: `${axis()!.x(d().day!)}%` }}>status</span></Show></div>
              <For each={rows()}>{r => { const ax = axis()!, f = r.done ? r.actual ?? r.forecast : r.forecast, b = bl(r, base()); return (
                <div class="msrow" onDblClick={() => open(r)}>
                  <div class="msname" title={r.name}>{r.name}</div>
                  <div class="mslane">
                    <For each={months()}>{m => <i class="msm" style={{ left: `${m.x}%` }} />}</For>
                    <Show when={d().day}><i class="mstodayl" style={{ left: `${ax.x(d().day!)}%` }} /></Show>
                    <Show when={b && f}><i class="msslip" classList={{ late: (r.var ?? 0) > 0 }} style={{ left: `${Math.min(ax.x(b!), ax.x(f!))}%`, width: `${Math.abs(ax.x(f!) - ax.x(b!))}%` }} /></Show>
                    <For each={r.baselines}>{([n, dd]) => <i class="msbl" style={{ left: `${ax.x(dd)}%`, background: BL_COLOURS[n % BL_COLOURS.length] }} title={`${blName(n)}: ${dt(dd)}`} />}</For>
                    <For each={[r.deadline, r.contract_date].filter(Boolean)}>{dd => <i class="msdl" style={{ left: `${ax.x(dd!)}%` }} title={`Deadline or contract date: ${dt(dd)}`} />}</For>
                    <Show when={f}><i class="msdia" classList={{ done: r.done, late: r.key === "bad" }} style={{ left: `${ax.x(f!)}%` }} title={`${r.done ? "Achieved" : "Forecast"}: ${dt(f)}`} /></Show>
                  </div>
                </div>); }}</For>
            </div>
          </div>
        </Show>
      </>}</Show>
    </Page>
  );
};
