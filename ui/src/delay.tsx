// Delay Analysis: the methods the plan's data allows, as-planned against as-built, and windows
// between snapshots. The engine measures (crates/core/src/delay.rs); this page only shows it.
import { For, Show, createResource, createSignal } from "solid-js";
import { call, tick, toast, setView } from "./store";
import { fmtDay } from "./api";
import { Page } from "./views";
import { ChartBox, bar, line } from "./charts";
import { Seg } from "./ui";
import { selectUid } from "./cmds";

type Method = { key: string; name: string; needs: string; ok: boolean; why: string };
type ApabRow = { uid: number; id: string; name: string; critical: boolean; bs: number; bf: number; s: number; f: number; started: boolean; finished: boolean; start_var: number; finish_var: number; planned_dur: number; dur: number };
type Slip = { uid: number; name: string; was: number; now: number; moved: number; critical: boolean };
type Win = { from: string; to: string; label: string; comment: string; finish_before: number; finish_after: number; moved: number; total: number; pct_before: number; pct_after: number; slips: Slip[] };
type DelayOut = { available: { baseline: number; actuals: number; snapshots: number; status: string | null; methods: Method[] }; apab: { planned_finish: number | null; finish: number; delay: number; rows: ApabRow[]; drivers: number[] } | null; windows: Win[] };

const iso = (s: string) => { const [y, m, d] = s.split("-").map(Number); return fmtDay(Math.round((Date.UTC(y, m - 1, d) - Date.UTC(1970, 0, 1)) / 864e5)); };
const sign = (v: number) => (v > 0 ? `+${v}` : `${v}`);

export const DelayView = () => {
  const [data] = createResource(() => tick(), () => call<DelayOut>("delay_analysis"));
  const [tab, setTab] = createSignal<"apab" | "windows">("apab");
  const [only, setOnly] = createSignal<"late" | "critical" | "all">("late");
  const go = (uid: number) => { setView("gantt"); selectUid(uid); };
  const exp = async () => { const p = await call<string | null>("delay_export"); if (p) toast(`Saved to ${p}`, true); };
  return (
    <Page title="Delay Analysis" help="delay" tools={<>
      <Seg value={tab()} set={v => setTab(v as "apab" | "windows")} options={[["apab", "As-planned v as-built"], ["windows", "Windows"]]} />
      <button class="btn" onClick={exp}>Export…</button>
    </>}>
      <Show when={data()}>{d => <>
        <div class="panel methods"><h2>What the plan holds</h2>
          <p class="muted">{d().available.baseline} tasks with a baseline · {d().available.actuals} started · {d().available.snapshots} snapshots{d().available.status ? ` · status date ${iso(d().available.status!)}` : ""}. Each method runs on the data it needs; the figures measure what moved, not who is responsible.</p>
          <div class="mgrid"><For each={d().available.methods}>{m => <div class="mcard" classList={{ ok: m.ok }}><b>{m.name}</b><span class="pill" classList={{ ok: m.ok }}>{m.ok ? "Can run" : "Not yet"}</span><p class="muted">{m.needs}</p><p>{m.why}</p></div>}</For></div>
        </div>
        <Show when={tab() === "apab"}>
          <Show when={d().apab} fallback={<p class="empty">Save a baseline on the plan as agreed (Progress › Set Baseline) to compare against it.</p>}>{a => {
            const rows = () => a().rows.filter(r => only() === "all" || (only() === "critical" ? r.critical : r.finish_var > 0 || r.start_var > 0));
            const drivers = () => a().drivers.map(u => a().rows.find(r => r.uid === u)!).filter(Boolean);
            return <>
              <div class="kpis">
                <div class="kpi"><div class="v">{a().planned_finish !== null ? fmtDay(a().planned_finish!) : "—"}</div><div class="l">planned finish (baseline)</div></div>
                <div class="kpi"><div class="v">{fmtDay(a().finish)}</div><div class="l">finish now</div></div>
                <div class="kpi" classList={{ bad: a().delay > 0, good: a().delay <= 0 }}><div class="v">{sign(a().delay)}</div><div class="l">working days at the finish</div></div>
              </div>
              <Show when={drivers().length}>
                <ChartBox title="Critical tasks finishing late (working days)" type="bar" horizontal size="short" fmt="num" labels={drivers().slice(0, 10).map(r => `${r.id} ${r.name}`)} datasets={[bar("Finish variance", drivers().slice(0, 10).map(r => r.finish_var), "#C0392B")]} />
              </Show>
              <div class="panel"><div class="row-tools"><Seg value={only()} set={v => setOnly(v as "late" | "critical" | "all")} options={[["late", "Late"], ["critical", "Critical"], ["all", "All"]]} /><span class="muted">{rows().length} tasks · A = actual date</span></div>
                <div class="fieldlist"><table class="tbl"><thead><tr><th>ID</th><th>Task</th><th class="r">Baseline start</th><th class="r">Start</th><th class="r">Var.</th><th class="r">Baseline finish</th><th class="r">Finish</th><th class="r">Var.</th><th class="r">Planned d</th><th class="r">Actual / forecast d</th></tr></thead>
                  <tbody><For each={rows().slice(0, 3000)}>{r => <tr class="link" onClick={() => go(r.uid)}>
                    <td>{r.id}</td><td>{r.critical ? <b>{r.name}</b> : r.name}</td><td class="r muted">{fmtDay(r.bs)}</td><td class="r">{fmtDay(r.s)}{r.started ? " A" : ""}</td><td class="r" classList={{ late: r.start_var > 0 }}>{sign(r.start_var)}</td>
                    <td class="r muted">{fmtDay(r.bf)}</td><td class="r">{fmtDay(r.f)}{r.finished ? " A" : ""}</td><td class="r" classList={{ late: r.finish_var > 0 }}>{sign(r.finish_var)}</td><td class="r muted">{r.planned_dur}</td><td class="r" classList={{ late: r.dur > r.planned_dur }}>{r.dur}</td>
                  </tr>}</For></tbody></table></div>
                <p class="muted">Variances in working days on the project calendar. Bold: on today's critical path. Click a task to find it in the Gantt.</p>
              </div>
            </>;
          }}</Show>
        </Show>
        <Show when={tab() === "windows"}>
          <Show when={d().windows.length} fallback={<p class="empty">Windows analysis needs two snapshots or more: take one at each update (Progress › Snapshot), with a note of what happened.</p>}>
            <ChartBox title="Project finish movement in each window (working days)" type="bar" size="short" fmt="num" labels={d().windows.map(w => w.label || iso(w.to))}
              datasets={[bar("Moved", d().windows.map(w => w.moved), "#C0392B"), line("Total", d().windows.map(w => w.total), "#0B5CAD")]} />
            <div class="panel"><table class="tbl"><thead><tr><th>Window</th><th class="r">Finish before</th><th class="r">Finish after</th><th class="r">Moved</th><th class="r">Total</th><th class="r">% complete</th><th>Tasks that slipped most</th><th>Notes</th></tr></thead>
              <tbody><For each={d().windows}>{w => <tr>
                <td>{iso(w.from)} → {iso(w.to)}<br /><span class="muted">{w.label}</span></td><td class="r">{fmtDay(w.finish_before)}</td><td class="r">{fmtDay(w.finish_after)}</td>
                <td class="r" classList={{ late: w.moved > 0 }}><b>{sign(w.moved)}</b></td><td class="r">{sign(w.total)}</td><td class="r">{w.pct_before.toFixed(0)}% → {w.pct_after.toFixed(0)}%</td>
                <td><For each={w.slips.slice(0, 3)}>{s => <div class="link" onClick={() => go(s.uid)}>{s.name} <span class="muted">+{s.moved} d{s.critical ? ", critical" : ""}</span></div>}</For></td>
                <td class="muted">{w.comment}</td>
              </tr>}</For></tbody></table>
              <p class="muted">Each window runs from one snapshot to the next (the last to today's plan when the status date is later). The tasks listed first were critical at the window's start. Notes are the snapshots' comments.</p></div>
          </Show>
        </Show>
      </>}</Show>
    </Page>
  );
};
