// Delay Analysis: the methods the plan's data allows, as-planned against as-built, and windows
// between snapshots. The engine measures (crates/core/src/delay.rs); this page only shows it.
import { For, Show, createResource, createSignal } from "solid-js";
import { call, tick, toast, setView, act, P, grid, openDialog } from "./store";
import { fmtDay } from "./api";
import { Dialog, Row } from "./ui";
import { Page } from "./views";
import { ChartBox, bar, line } from "./charts";
import { Seg } from "./ui";
import { selectUid } from "./cmds";

type Method = { key: string; name: string; needs: string; ok: boolean; why: string };
type ApabRow = { uid: number; id: string; name: string; critical: boolean; bs: number; bf: number; s: number; f: number; started: boolean; finished: boolean; start_var: number; finish_var: number; planned_dur: number; dur: number };
type Slip = { uid: number; name: string; was: number; now: number; moved: number; critical: boolean };
type Win = { from: string; to: string; label: string; comment: string; finish_before: number; finish_after: number; moved: number; total: number; pct_before: number; pct_after: number; slips: Slip[] };
type Ev = { id: number; name: string; date: string | null; description: string; on: boolean; extend: [number, number][]; hold: [number, string][]; frag: { name: string; dur: number; after: number[]; before: number[] }[] };
type EvImpact = { id: number; name: string; date: string | null; before: number; after: number; impact: number; total: number; notes: string[] };
type Impact = { key: string; basis: string; fidelity: string | null; start_finish: number; end_finish: number; total: number; events: EvImpact[] };
type Res = { Ok: Impact } | { Err: string };
type DelayOut = { tia: Res | null; iap: Res | null; cab: Res | null; available: { baseline: number; actuals: number; snapshots: number; status: string | null; methods: Method[] }; apab: { planned_finish: number | null; finish: number; delay: number; rows: ApabRow[]; drivers: number[] } | null; windows: Win[] };

const iso = (s: string) => { const [y, m, d] = s.split("-").map(Number); return fmtDay(Math.round((Date.UTC(y, m - 1, d) - Date.UTC(1970, 0, 1)) / 864e5)); };
const sign = (v: number) => (v > 0 ? `+${v}` : `${v}`);
type Tab = "apab" | "windows" | "events" | "tia" | "iap" | "cab";

/** Leaf tasks to pick from: [uid, "ID name"]. */
const taskList = (): [number, string][] => grid().rows.filter(r => r.uid > 0 && r.kind === "task" && !r.sub).map(r => [r.uid, `${r.id}. ${r.name}`]);
const nameOf = (uid: number) => grid().rows.find(r => r.uid === uid)?.name ?? `#${uid}`;
const dayMin = () => P().day_min || 480;

/** Add or change a delay event: its name, date, description, and what it added to the plan. */
const editEvent = (e0?: Ev) => openDialog(() => {
  const blank: Ev = { id: 0, name: "", date: P().status ?? null, description: "", on: true, extend: [], hold: [], frag: [] };
  const [e, setE] = createSignal<Ev>(e0 ? JSON.parse(JSON.stringify(e0)) : blank);
  const tasks = taskList();
  const first = tasks[0]?.[0] ?? 0;
  const pick = (v: number, set: (u: number) => void) => <select onChange={x => set(+x.currentTarget.value)}><For each={tasks}>{([u, n]) => <option value={u} selected={u === v}>{n}</option>}</For></select>;
  const up = (f: (x: Ev) => void) => { const x = JSON.parse(JSON.stringify(e())); f(x); setE(x); };
  return <Dialog title={e0 ? "Delay event" : "New delay event"} wide okText="Save" ok={() => {
    if (!e().name.trim()) { toast("Give the event a name."); return false; }
    act({ Delay: { id: e().id, ev: e() } });
  }}>
    <div class="form">
      <Row label="Name"><input value={e().name} onChange={x => up(v => { v.name = x.currentTarget.value; })} placeholder="What happened, in a few words" /></Row>
      <Row label="Arose on"><input type="date" value={e().date ?? ""} onChange={x => up(v => { v.date = x.currentTarget.value || null; })} /></Row>
      <Row label="Description"><textarea rows="2" value={e().description} onChange={x => up(v => { v.description = x.currentTarget.value; })} placeholder="The facts and the records behind them (letters, minutes, site diary)" /></Row>
    </div>
    <h3>What it added to the plan</h3>
    <p class="muted">Extra working days on tasks, a task that could not start before a date, or new activities (a fragnet) between tasks.</p>
    <table class="tbl evfx"><tbody>
      <For each={e().extend}>{(x, k) => <tr><td>Extend</td><td>{pick(x[0], u => up(v => { v.extend[k()][0] = u; }))}</td><td>by <input type="number" min="0" step="0.5" style={{ width: "70px" }} value={x[1] / dayMin()} onChange={y => up(v => { v.extend[k()][1] = Math.max(0, Math.round(+y.currentTarget.value * dayMin())); })} /> days</td><td><button class="x" onClick={() => up(v => { v.extend.splice(k(), 1); })}>×</button></td></tr>}</For>
      <For each={e().hold}>{(x, k) => <tr><td>Hold</td><td>{pick(x[0], u => up(v => { v.hold[k()][0] = u; }))}</td><td>not before <input type="date" value={x[1]} onChange={y => up(v => { v.hold[k()][1] = y.currentTarget.value; })} /></td><td><button class="x" onClick={() => up(v => { v.hold.splice(k(), 1); })}>×</button></td></tr>}</For>
      <For each={e().frag}>{(x, k) => <tr><td>New activity</td><td><input value={x.name} placeholder="Activity name" onChange={y => up(v => { v.frag[k()].name = y.currentTarget.value; })} /> <input type="number" min="0" step="0.5" style={{ width: "60px" }} value={x.dur / dayMin()} onChange={y => up(v => { v.frag[k()].dur = Math.max(0, Math.round(+y.currentTarget.value * dayMin())); })} /> days</td>
        <td>after {pick(x.after[0] ?? first, u => up(v => { v.frag[k()].after = [u]; }))} before {pick(x.before[0] ?? first, u => up(v => { v.frag[k()].before = [u]; }))}</td><td><button class="x" onClick={() => up(v => { v.frag.splice(k(), 1); })}>×</button></td></tr>}</For>
    </tbody></table>
    <p><button class="btn" onClick={() => up(v => { v.extend.push([first, dayMin()]); })}>+ Extension</button> <button class="btn" onClick={() => up(v => { v.hold.push([first, P().status ?? P().start]); })}>+ Hold</button> <button class="btn" onClick={() => up(v => { v.frag.push({ name: "", dur: dayMin(), after: [first], before: [first] }); })}>+ New activity</button></p>
  </Dialog>;
});

/** One method's result on the delay events. */
const ImpactView = (p: { r: Res | null; empty: string }) => (
  <Show when={p.r} fallback={<p class="empty">{p.empty}</p>}>{r => <Show when={"Ok" in r() ? (r() as { Ok: Impact }).Ok : null} fallback={<p class="empty">{(r() as { Err: string }).Err}</p>}>{ok => { const x = ok(); return <>
    <p class="muted">{x.basis}</p><Show when={x.fidelity}><p class="muted">{x.fidelity}</p></Show>
    <div class="kpis">
      <div class="kpi"><div class="v">{fmtDay(x.start_finish)}</div><div class="l">{x.key === "cab" ? "finish as built" : "finish before the events"}</div></div>
      <div class="kpi"><div class="v">{fmtDay(x.end_finish)}</div><div class="l">{x.key === "cab" ? "finish but for the events" : "finish with the events"}</div></div>
      <div class="kpi" classList={{ bad: x.total > 0 }}><div class="v">{sign(x.total)}</div><div class="l">working days from the events</div></div>
    </div>
    <div class="panel"><table class="tbl"><thead><tr><th>Event</th><th>Arose</th><th class="r">Finish before</th><th class="r">Finish after</th><th class="r">Effect (d)</th><th class="r">Total (d)</th><th>Notes</th></tr></thead>
      <tbody><For each={x.events}>{e => <tr><td><b>{e.name}</b></td><td>{e.date ? iso(e.date) : "—"}</td><td class="r">{fmtDay(e.before)}</td><td class="r">{fmtDay(e.after)}</td><td class="r" classList={{ late: e.impact > 0 }}><b>{sign(e.impact)}</b></td><td class="r">{sign(e.total)}</td><td class="muted">{e.notes.join(" ")}</td></tr>}</For></tbody></table>
      <p class="muted">Events are applied in the order they arose{x.key === "cab" ? " (taken out latest first)" : ""}; each effect is measured with the events before it already in place, so effects that overlap are not counted twice.</p></div>
  </>; }}</Show>}</Show>
);

export const DelayView = () => {
  const [data] = createResource(() => tick(), () => call<DelayOut>("delay_analysis"));
  const [tab, setTab] = createSignal<Tab>("apab");
  const [only, setOnly] = createSignal<"late" | "critical" | "all">("late");
  const go = (uid: number) => { setView("gantt"); selectUid(uid); };
  const exp = async () => { const p = await call<string | null>("delay_export"); if (p) toast(`Saved to ${p}`, true); };
  return (
    <Page title="Delay Analysis" help="delay" tools={<>
      <Seg value={tab()} set={v => setTab(v as Tab)} options={[["apab", "As-planned v as-built"], ["windows", "Windows"], ["events", "Delay events"], ["tia", "Time impact"], ["iap", "Impacted as-planned"], ["cab", "Collapsed as-built"]]} />
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
        <Show when={tab() === "events"}>
          <div class="panel"><div class="row-tools"><button class="btn pri" onClick={() => editEvent()}>+ Add event</button><span class="muted">What happened and what it added to the plan. Switch an event off to leave it out of the analyses.</span></div>
            <Show when={(P().delays ?? []).length} fallback={<p class="empty">No delay events yet. Add each event with the date it arose and its effect: extra days on a task, a task held to a date, or new activities.</p>}>
              <table class="tbl"><thead><tr><th>On</th><th>Event</th><th>Arose</th><th>Effect on the plan</th><th /></tr></thead>
                <tbody><For each={P().delays ?? []}>{(e: Ev) => <tr>
                  <td><input type="checkbox" checked={e.on} onChange={x => act({ Delay: { id: e.id, ev: { ...e, on: x.currentTarget.checked } } })} /></td>
                  <td class="link" onClick={() => editEvent(e)}><b>{e.name}</b><br /><span class="muted">{e.description}</span></td><td>{e.date ? iso(e.date) : "—"}</td>
                  <td class="muted">{[...e.extend.map(x => `${nameOf(x[0])} +${x[1] / dayMin()} d`), ...e.hold.map(x => `${nameOf(x[0])} not before ${iso(x[1])}`), ...e.frag.map(f => `new: ${f.name} (${f.dur / dayMin()} d)`)].join("; ") || "—"}</td>
                  <td><Show when={e.frag.length}><button class="btn sm" title="Put this event's new activities into the plan, so the plan as built shows them" onClick={() => act({ DelayFrag: { id: e.id } }).then(() => toast("The new activities are at the end of the plan.", true))}>Add to plan</button></Show> <button class="x" onClick={() => act({ Delay: { id: e.id, ev: null } })}>×</button></td>
                </tr>}</For></tbody></table>
            </Show></div>
        </Show>
        <Show when={tab() === "tia"}><ImpactView r={d().tia} empty="Time impact needs delay events: add them on the Delay events tab." /></Show>
        <Show when={tab() === "iap"}><ImpactView r={d().iap} empty="Impacted as-planned needs a baseline and delay events." /></Show>
        <Show when={tab() === "cab"}><ImpactView r={d().cab} empty="Collapsed as-built needs actual dates and delay events." /></Show>
      </>}</Show>
    </Page>
  );
};
