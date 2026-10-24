// Views other than the Gantt chart.
import { For, Show, createSignal, createResource, createMemo, createEffect, JSX } from "solid-js";
import { Net, Row, Usage, Check, Resource, fmtDay, dayDate, dateDay, money } from "./api";
import { call, grid, out, P, query, tick, sel, setSel, act, setFilter, setView, openMenu, toast, setBackstage, loadOut } from "./store";
import { C, selectUid } from "./cmds";
import { Seg, Icon, Check as Chk } from "./ui";

const Page = (p: { title: string; tools?: JSX.Element; children: JSX.Element }) => (
  <section class="page"><div class="ptools"><h1>{p.title}</h1><span class="grow" />{p.tools}</div>{p.children}</section>
);

// ---------- Network Diagram (MS Project style boxes) ----------
export const Network = () => {
  const [scale, setScale] = createSignal(1);
  const [net] = createResource(() => [tick(), JSON.stringify(query())], () => call<Net>("network", { q: query() }));
  const rows = createMemo(() => new Map(grid().rows.map(r => [r.uid, r])));
  const W = 176, H = 82, GX = 222, GY = 104;
  const pos = (n: { col: number; row: number }) => [20 + n.col * GX, 20 + n.row * GY];
  const at = createMemo(() => new Map((net()?.nodes ?? []).map(n => [n.uid, pos(n)])));
  return (
    <Page title="Network Diagram" tools={<>
      <span class="muted">{net()?.nodes.length ?? 0} tasks</span>
      <button class="btn" onClick={() => setScale(Math.max(0.3, scale() / 1.25))}>−</button><button class="btn" onClick={() => setScale(Math.min(2, scale() * 1.25))}>+</button>
      <button class="btn" onClick={() => C.print()}>Print</button>
    </>}>
      <div class="netwrap" onWheel={e => { if (e.ctrlKey) { e.preventDefault(); setScale(Math.max(0.3, Math.min(2, scale() * (e.deltaY < 0 ? 1.1 : 1 / 1.1)))); } }}>
        <Show when={net()} fallback={<p class="empty">Loading…</p>}>{n => (
          <svg class="net" width={(n().cols * GX + 40) * scale()} height={(n().rows * GY + 40) * scale()} viewBox={`0 0 ${n().cols * GX + 40} ${n().rows * GY + 40}`}>
            <defs><marker id="ar" viewBox="0 0 8 8" refX="7" refY="4" markerWidth="7" markerHeight="7" orient="auto"><path d="M0 0L8 4L0 8z" fill="#6E7F99" /></marker>
              <marker id="arc" viewBox="0 0 8 8" refX="7" refY="4" markerWidth="7" markerHeight="7" orient="auto"><path d="M0 0L8 4L0 8z" fill="#D64545" /></marker></defs>
            <For each={n().links}>{l => {
              const a = at().get(l.from), b = at().get(l.to);
              if (!a || !b) return null;
              const x1 = a[0] + W, y1 = a[1] + H / 2, x2 = b[0], y2 = b[1] + H / 2, mx = x2 - 18;
              return <path d={`M${x1} ${y1}H${mx}V${y2}H${x2 - 1}`} fill="none" stroke={l.crit ? "#D64545" : "#6E7F99"} stroke-width={l.crit ? 1.6 : 1} marker-end={`url(#${l.crit ? "arc" : "ar"})`} />;
            }}</For>
            <For each={n().nodes}>{nd => {
              const r = () => rows().get(nd.uid);
              const [x, y] = pos(nd);
              const cells = (r: Row) => { const g = grid(); const v = (k: string) => r.cells[g.cols.findIndex(c => c.key === k)] ?? ""; return v; };
              return <Show when={r()}>{row => {
                const v = cells(row());
                const done = row().bar.pct === 100, prog = row().bar.pct > 0 && !done;
                return (
                  <g transform={`translate(${x},${y})`} class="nbox" classList={{ crit: row().crit, sel: sel().includes(nd.uid) }} onClick={() => selectUid(nd.uid)} onDblClick={() => C.info()}>
                    {row().ms ? <path d={`M10 0H${W - 10}L${W} ${H / 2}L${W - 10} ${H}H10L0 ${H / 2}z`} /> : <rect width={W} height={H} rx="3" />}
                    <Show when={done || prog}><path d={done ? `M4 4L${W - 4} ${H - 4}M${W - 4} 4L4 ${H - 4}` : `M4 4L${W - 4} ${H - 4}`} class="x" /></Show>
                    <text x="8" y="16" class="t">{row().name.length > 30 ? row().name.slice(0, 29) + "…" : row().name}</text>
                    <line x1="0" x2={W} y1="23" y2="23" />
                    <text x="8" y="38">Start: {fmtDay(row().bar.s)}</text><text x={W / 2 + 6} y="38">ID: {row().id}</text>
                    <text x="8" y="54">Finish: {fmtDay(row().bar.f % 1 ? row().bar.f : row().bar.f - 1)}</text><text x={W / 2 + 6} y="54">Dur: {v("dur") || Math.round((row().bar.f - row().bar.s) * 10) / 10 + "d"}</text>
                    <text x="8" y="70" class="m">{row().res.length > 30 ? row().res.slice(0, 29) + "…" : row().res}</text>
                  </g>
                );
              }}</Show>;
            }}</For>
          </svg>
        )}</Show>
      </div>
    </Page>
  );
};

// ---------- Calendar ----------
export const CalendarView = () => {
  const [month, setMonth] = createSignal<Date>((() => { const s = out()?.summary.s; const d = s ? dayDate(s) : new Date(); return new Date(d.getFullYear(), d.getMonth(), 1); })());
  const tasks = () => grid().rows.filter(r => r.kind === "task" && !r.off);
  const days = createMemo(() => {
    const m = month(), first = new Date(m); first.setDate(1 - ((first.getDay() + 6) % 7));
    return Array.from({ length: 42 }, (_, k) => { const d = new Date(first); d.setDate(first.getDate() + k); return d; });
  });
  const on = (d: Date) => { const n = dateDay(d); return tasks().filter(r => Math.floor(r.bar.s) <= n && n < Math.max(Math.ceil(r.bar.f), Math.floor(r.bar.s) + 1)); };
  const shift = (k: number) => setMonth(new Date(month().getFullYear(), month().getMonth() + k, 1));
  const td = dateDay(new Date());
  return (
    <Page title={month().toLocaleString("en-GB", { month: "long", year: "numeric" })} tools={<><button class="btn" onClick={() => shift(-1)}>‹</button><button class="btn" onClick={() => setMonth(new Date(new Date().getFullYear(), new Date().getMonth(), 1))}>Today</button><button class="btn" onClick={() => shift(1)}>›</button></>}>
      <div class="calgrid">
        <For each={["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"]}>{d => <div class="dh">{d}</div>}</For>
        <For each={days()}>{d => {
          const list = on(d);
          return (
            <div class="day" classList={{ out: d.getMonth() !== month().getMonth(), nw: d.getDay() % 6 === 0, today: dateDay(d) === td }}>
              <span class="dn">{d.getDate()}</span>
              <For each={list.slice(0, 4)}>{r => <div class="chip" classList={{ ms: r.ms, crit: r.crit, sel: sel().includes(r.uid) }} title={r.name} onClick={() => selectUid(r.uid)} onDblClick={() => C.info()}>{r.name}</div>}</For>
              <Show when={list.length > 4}><div class="chip more">+{list.length - 4} more</div></Show>
            </div>
          );
        }}</For>
      </div>
    </Page>
  );
};

// ---------- Task Usage, Resource Usage ----------
const fmtB = (b: number, unit: string) => unit === "month" ? fmtDay(b, "month") : unit === "quarter" ? `Q${Math.floor(dayDate(b).getMonth() / 3) + 1} ${dayDate(b).getFullYear()}` : fmtDay(b);
export const UsageView = (p: { by: "task" | "resource" }) => {
  const [unit, setUnit] = createSignal("week");
  const [what, setWhat] = createSignal("work");
  const [u] = createResource(() => [tick(), unit(), what()], () => call<Usage>("usage", { by: p.by, what: what(), unit: unit() }));
  const fmt = (v: number) => (v === 0 ? "" : what() === "cost" ? money(v, P().currency) : `${Math.round(v * 10) / 10}h`);
  return (
    <Page title={p.by === "task" ? "Task Usage" : "Resource Usage"} tools={<>
      <Seg options={[["work", "Work"], ["cost", "Cost"]]} value={what()} set={setWhat} />
      <Seg options={[["day", "Days"], ["week", "Weeks"], ["month", "Months"], ["quarter", "Quarters"]]} value={unit()} set={setUnit} />
    </>}>
      <div class="wlwrap">
        <Show when={u()}>{x => (
          <table class="tbl wltbl">
            <thead><tr><th class="nm">{p.by === "task" ? "Task / assignment" : "Resource / assignment"}</th><th class="r">Total</th><For each={x().buckets}>{b => <th class="r">{fmtB(b, unit())}</th>}</For></tr></thead>
            <tbody><For each={x().rows}>{r => (
              <tr classList={{ sub: r.of !== null, sumr: r.sum }}>
                <td class="nm" style={{ "padding-left": `${6 + r.lvl * 14}px` }}>{r.name}</td><td class="r">{fmt(r.total)}</td>
                <For each={r.vals}>{(v, k) => <td class="r wl" classList={{ over: r.over[k()] }}>{fmt(v)}</td>}</For>
              </tr>
            )}</For></tbody>
          </table>
        )}</Show>
      </div>
    </Page>
  );
};

// ---------- Resource Sheet ----------
const RES_COLS: [string, string, number][] = [["name", "Resource Name", 180], ["kind", "Type", 90], ["label", "Material", 70], ["initials", "Initials", 60], ["group", "Group", 100], ["max", "Max", 60],
  ["rate", "Std. Rate", 90], ["ovt", "Ovt. Rate", 90], ["per_use", "Cost/Use", 80], ["cal", "Base Calendar", 110], ["code", "Code", 70], ["email", "Email", 150], ["work", "Work", 80], ["cost", "Cost", 100]];
export const ResourceSheet = () => {
  const [rows] = createResource(() => tick(), () => call<[number, boolean, string[]][]>("resources", { cols: RES_COLS.map(c => c[0]) }));
  const raw = (r: Resource, k: string): string => k === "max" ? `${Math.round(r.max * 100)}%` : k === "kind" ? r.kind : k === "cal" ? r.cal ?? "" : String((r as any)[k] ?? "");
  const [pick, setPick] = createSignal<number[]>([]);
  return (
    <Page title="Resource Sheet" tools={<>
      <button class="btn" onClick={() => act({ ResInsert: { name: `Resource ${P().resources.length + 1}` } })}>+ Add resource</button>
      <button class="btn" disabled={!pick().length} onClick={() => act({ ResDelete: { uids: pick() } }).then(() => setPick([]))}>Delete</button>
      <button class="btn" onClick={() => C.level()}>Level all</button>
    </>}>
      <div class="wlwrap">
        <table class="tbl sheet">
          <thead><tr><th /><th>ID</th><For each={RES_COLS}>{c => <th style={{ "min-width": `${c[2]}px` }}>{c[1]}</th>}</For><th /></tr></thead>
          <tbody><For each={P().resources}>{(r, k) => {
            const calc = () => rows()?.find(x => x[0] === r.uid);
            return (
              <tr classList={{ over: calc()?.[1] }}>
                <td><input type="checkbox" checked={pick().includes(r.uid)} onChange={e => setPick(e.currentTarget.checked ? [...pick(), r.uid] : pick().filter(x => x !== r.uid))} /></td>
                <td class="muted">{k() + 1}{calc()?.[1] && <span class="late" title="Overallocated"> ◆</span>}</td>
                <For each={RES_COLS}>{([key], j) => <td>{
                  key === "work" || key === "cost" ? <span class="ro">{calc()?.[2][j()]}</span>
                    : key === "kind" ? <select value={r.kind} onChange={e => act({ ResSet: { uid: r.uid, field: "kind", value: e.currentTarget.value } })}><option>Work</option><option>Material</option><option>Cost</option></select>
                    : key === "cal" ? <select value={r.cal ?? ""} onChange={e => act({ ResJson: { uid: r.uid, field: "cal", value: e.currentTarget.value || null } })}><option value="">(project)</option><For each={P().calendars}>{c => <option>{c.name}</option>}</For></select>
                    : <input value={raw(r, key)} onChange={e => act({ ResSet: { uid: r.uid, field: key, value: e.currentTarget.value } })} />
                }</td>}</For>
                <td><button class="btn sm" onClick={() => import("./dialogs").then(d => d.resourceInfo(r.uid))}>Info…</button></td>
              </tr>
            );
          }}</For></tbody>
        </table>
        <Show when={!P().resources.length}><p class="empty">No resources yet. Add people, equipment, materials or cost items, or type names in a task's Resource Names column.</p></Show>
      </div>
    </Page>
  );
};

// ---------- Resource Graph ----------
export const ResourceGraph = () => {
  const [unit, setUnit] = createSignal("week");
  const [res, setRes] = createSignal(0);
  const [u] = createResource(() => [tick(), unit()], () => call<Usage>("usage", { by: "resource", what: "work", unit: unit() }));
  const work = () => P().resources.filter(r => r.kind === "Work");
  createEffect(() => { if (!res() && work()[0]) setRes(work()[0].uid); });
  const row = () => u()?.rows.find(r => r.of === null && r.uid === res());
  const r = () => P().resources.find(x => x.uid === res());
  const cap = (b: number, k: number) => { const n = (u()?.buckets[k + 1] ?? b + (unit() === "day" ? 1 : 7)) - b; const days = unit() === "day" ? 1 : unit() === "week" ? 5 : Math.round(n * 5 / 7); return (r()?.max ?? 1) * days * P().day_min / 60; };
  return (
    <Page title="Resource Graph" tools={<>
      <select value={res()} onChange={e => setRes(+e.currentTarget.value)}><For each={work()}>{x => <option value={x.uid}>{x.name}</option>}</For></select>
      <Seg options={[["day", "Days"], ["week", "Weeks"], ["month", "Months"]]} value={unit()} set={setUnit} />
    </>}>
      <Show when={row() && u()} fallback={<p class="empty">Add work resources and assign them to see their load.</p>}>{_ => {
        const vals = row()!.vals, b = u()!.buckets, max = Math.max(1, ...vals, ...b.map(cap)), W = Math.max(600, vals.length * 34), H = 300;
        return (
          <div class="wlwrap panel"><svg width={W + 60} height={H + 50}>
            <For each={[0, 0.25, 0.5, 0.75, 1]}>{f => <><line x1="50" x2={W + 50} y1={H - f * H + 10} y2={H - f * H + 10} stroke="#E6EBF2" /><text x="44" y={H - f * H + 14} text-anchor="end" class="ax">{Math.round(max * f)}h</text></>}</For>
            <For each={vals}>{(v, k) => {
              const x = 54 + k() * (W / vals.length), w = W / vals.length - 6, c = cap(b[k()], k()), h = v / max * H;
              return <><rect x={x} y={H - h + 10} width={w} height={h} fill={v > c + 0.01 ? "#D64545" : "#6E9FE6"} rx="2"><title>{`${fmtB(b[k()], unit())}: ${Math.round(v * 10) / 10}h of ${Math.round(c)}h`}</title></rect>
                <line x1={x - 2} x2={x + w + 2} y1={H - c / max * H + 10} y2={H - c / max * H + 10} stroke="#14325E" stroke-dasharray="4 3" />
                <Show when={vals.length < 40 || k() % Math.ceil(vals.length / 30) === 0}><text x={x + w / 2} y={H + 28} text-anchor="middle" class="ax">{fmtB(b[k()], unit())}</text></Show></>;
            }}</For>
          </svg><p class="muted">Bars show assigned hours; the dashed line is capacity (max units × working hours). Red is over capacity.</p></div>
        );
      }}</Show>
    </Page>
  );
};

// ---------- Team Planner ----------
export const TeamPlanner = () => {
  const tasks = () => grid().rows.filter(r => r.kind === "task" && !r.off);
  const s = () => out()!.summary, span = () => Math.max(7, s().f - s().s + 2);
  const lanes = createMemo(() => {
    const people = P().resources.filter(r => r.kind === "Work");
    const m = new Map<string, Row[]>(people.map(r => [r.name, []]));
    const none: Row[] = [];
    for (const t of tasks()) { const names = t.res.split(",").map(x => x.trim().replace(/\[.*\]/, "")).filter(Boolean); let any = false; for (const n of names) if (m.has(n)) { m.get(n)!.push(t); any = true; } if (!any) none.push(t); }
    return [...[...m].map(([k, v]) => [k, v] as [string, Row[]]), ["Unassigned tasks", none] as [string, Row[]]];
  });
  const W = 1100, px = (d: number) => ((d - s().s) / span()) * W;
  return (
    <Page title="Team Planner" tools={<span class="muted">Each lane shows a person's tasks. Overlaps mean overallocation.</span>}>
      <div class="wlwrap panel"><svg width={W + 190} height={lanes().length * 44 + 30}>
        <For each={lanes()}>{([name, list], k) => <g transform={`translate(0,${k() * 44 + 20})`}>
          <text x="6" y="22" class="lane">{name}</text><line x1="0" x2={W + 190} y1="40" y2="40" stroke="#E6EBF2" />
          <For each={list}>{(t, j) => {
            const overlap = list.slice(0, j()).some(o => o.bar.s < t.bar.f && t.bar.s < o.bar.f);
            return <rect x={180 + px(t.bar.s)} y={overlap ? 22 : 6} width={Math.max(4, px(t.bar.f) - px(t.bar.s))} height="14" rx="3" fill={t.crit ? "#F2B0B0" : "#A9CAF6"} stroke={overlap && name !== "Unassigned tasks" ? "#D64545" : "#2F6FCF"} onClick={() => selectUid(t.uid)} onDblClick={() => C.info()}><title>{`${t.name}\n${fmtDay(t.bar.s)} → ${fmtDay(t.bar.f)}`}</title></rect>;
          }}</For>
        </g>}</For>
      </svg></div>
    </Page>
  );
};

// ---------- Risks ----------
export const Risks = () => {
  const blank = () => ({ id: Math.max(0, ...P().risks.map(r => r.id)) + 1, name: "New risk", cause: "", prob: 3, impact: 3, owner: "", response: "", status: "Open", tasks: [], kind: "Threat" });
  const put = (r: any) => act({ Risk: { id: r.id, risk: r } });
  const color = (s: number) => s >= 15 ? "#F8D4D4" : s >= 8 ? "#FDF0D2" : "#DDF2E5";
  return (
    <Page title="Risk Register" tools={<button class="btn pri" onClick={() => put(blank())}>+ Add risk</button>}>
      <div class="cols">
        <div class="panel" style={{ "grid-column": "span 2" }}>
          <table class="tbl"><thead><tr><th>ID</th><th>Risk</th><th>Type</th><th>Cause</th><th>Prob. (1–5)</th><th>Impact (1–5)</th><th>Score</th><th>Owner</th><th>Response</th><th>Status</th><th>Tasks</th><th /></tr></thead>
            <tbody><For each={P().risks}>{r => (
              <tr>
                <td>R{r.id}</td><td><input value={r.name} onChange={e => put({ ...r, name: e.currentTarget.value })} /></td>
                <td><select value={r.kind} onChange={e => put({ ...r, kind: e.currentTarget.value })}><option>Threat</option><option>Opportunity</option></select></td>
                <td><input value={r.cause} onChange={e => put({ ...r, cause: e.currentTarget.value })} /></td>
                <td><input type="number" min="1" max="5" value={r.prob} onChange={e => put({ ...r, prob: +e.currentTarget.value })} /></td>
                <td><input type="number" min="1" max="5" value={r.impact} onChange={e => put({ ...r, impact: +e.currentTarget.value })} /></td>
                <td style={{ background: color(r.prob * r.impact) }}><b>{r.prob * r.impact}</b></td>
                <td><input value={r.owner} onChange={e => put({ ...r, owner: e.currentTarget.value })} /></td>
                <td><input value={r.response} onChange={e => put({ ...r, response: e.currentTarget.value })} /></td>
                <td><select value={r.status} onChange={e => put({ ...r, status: e.currentTarget.value })}><option>Open</option><option>Mitigating</option><option>Closed</option></select></td>
                <td class="muted">{r.tasks.length}</td><td><button class="x" onClick={() => act({ Risk: { id: r.id, risk: null } })}>×</button></td>
              </tr>
            )}</For></tbody></table>
          <Show when={!P().risks.length}><p class="empty">No risks recorded. Add threats and opportunities, score them, and link them to tasks in the details pane.</p></Show>
        </div>
        <div class="panel"><h2>Probability × impact</h2>
          <div class="matrix"><For each={[5, 4, 3, 2, 1]}>{p => <For each={[1, 2, 3, 4, 5]}>{i => {
            const n = P().risks.filter(r => r.prob === p && r.impact === i && r.status !== "Closed").length;
            return <div style={{ background: color(p * i) }}>{n || ""}</div>;
          }}</For>}</For></div><p class="muted">Rows: probability (5 at the top). Columns: impact.</p>
        </div>
      </div>
    </Page>
  );
};

// ---------- Check Schedule (DCMA 14 and P6 checks) ----------
export const Checks = () => {
  const [lim, setLim] = createSignal({ pct: 5, high_float: 44, high_dur: 44, long_lag: 20, index: 0.95 });
  const [list] = createResource(() => [tick(), lim()], () => call<Check[]>("checks", { limits: lim() }));
  const show = (c: Check) => {
    if (!c.uids.length) return toast("No tasks to show for this check.", true);
    setFilter({ name: c.name, rules: c.uids.map(u => ({ field: "uid", test: "equals", value: String(u), value2: "", or: true })), summaries: true, highlight: false });
    setView("gantt");
  };
  const fmt = (c: Check) => (c.key === "cpli" || c.key === "bei" ? c.value.toFixed(2) : c.key === "cpt" ? (c.pass ? "Pass" : "Fail") : `${c.value.toFixed(1)}%`);
  return (
    <Page title="Check Schedule" tools={<>
      <label class="muted">Limit % <input type="number" style={{ width: "60px" }} value={lim().pct} onChange={e => setLim({ ...lim(), pct: +e.currentTarget.value })} /></label>
      <label class="muted">High float (d) <input type="number" style={{ width: "60px" }} value={lim().high_float} onChange={e => setLim({ ...lim(), high_float: +e.currentTarget.value })} /></label>
      <label class="muted">High duration (d) <input type="number" style={{ width: "60px" }} value={lim().high_dur} onChange={e => setLim({ ...lim(), high_dur: +e.currentTarget.value })} /></label>
      <button class="btn" onClick={() => C.print()}>Print</button>
    </>}>
      <Show when={list()}>{l => <>
        <div class="kpis"><div class="kpi" classList={{ good: l().every(c => c.pass) }}><div class="v">{l().filter(c => c.pass).length} / {l().length}</div><div class="l">checks passed</div></div></div>
        <div class="panel" style={{ "margin-top": "14px" }}><table class="tbl">
          <thead><tr><th>Check</th><th>What it looks for</th><th class="r">Tasks</th><th class="r">Of</th><th class="r">Result</th><th>Limit</th><th /></tr></thead>
          <tbody><For each={l()}>{c => <tr class="link" onClick={() => show(c)}>
            <td><b>{c.name}</b></td><td class="muted">{c.about}</td><td class="r">{c.count}</td><td class="r">{c.total}</td><td class="r">{fmt(c)}</td><td>{c.limit}</td>
            <td><span class="pill" classList={{ ok: c.pass, err: !c.pass }}>{c.pass ? "Pass" : "Fail"}</span></td>
          </tr>}</For></tbody>
        </table><p class="muted">Click a check to filter the Gantt chart to its tasks.</p></div>
      </>}</Show>
    </Page>
  );
};

// ---------- Portfolio (workspace folder) ----------
/** Plans grouped by folder (the EPS), with a rollup row per folder. */
const eps = (cards: any[]) => {
  const out: any[] = [];
  const folders = [...new Set(cards.map(c => c.folder))].sort();
  const all = new Set<string>();
  folders.forEach(f => { const parts = f ? f.split(" › ") : []; parts.forEach((_: string, k: number) => all.add(parts.slice(0, k + 1).join(" › "))); });
  const heads = [...all].sort();
  const under = (h: string) => cards.filter(c => c.folder === h || c.folder.startsWith(h + " › "));
  for (const c of cards.filter(c => !c.folder)) out.push({ ...c, depth: 0 });
  for (const h of heads) {
    const u = under(h);
    const depth = h.split(" › ").length - 1;
    out.push({ head: h, depth, n: u.length, pct: u.reduce((a, c) => a + c.pct, 0) / Math.max(1, u.length), late: u.reduce((a, c) => a + c.late, 0), cost: "" });
    for (const c of cards.filter(c => c.folder === h)) out.push({ ...c, depth: depth + 1 });
  }
  return out;
};
export const Portfolio = () => {
  const [shut, setShut] = createSignal<string[]>([]);
  const [again, setAgain] = createSignal(0);
  const [data] = createResource(() => again(), k => call<[string, any[]]>("workspace", { choose: k < 0 }));
  return (
    <Page title="Portfolio" tools={<><span class="muted">{data()?.[0] || "No workspace folder"}</span><button class="btn" onClick={() => setAgain(-Math.abs(again()) - 1)}><Icon n="folder" /> Choose folder…</button><button class="btn" onClick={() => setAgain(Math.abs(again()) + 1)}>Refresh</button></>}>
      <div class="panel"><table class="tbl">
        <thead><tr><th>Plan</th><th>Manager</th><th>Start</th><th>Finish</th><th class="r">Tasks</th><th>Complete</th><th class="r">Cost</th><th class="r">SPI</th><th class="r">CPI</th><th class="r">Late</th></tr></thead>
        <tbody><For each={eps(data()?.[1] ?? [])}>{c => c.head !== undefined ? (
          <tr class="eps" onClick={() => setShut(shut().includes(c.head) ? shut().filter(x => x !== c.head) : [...shut(), c.head])}>
            <td colspan="5" style={{ "padding-left": `${6 + c.depth * 16}px` }}><b>{shut().includes(c.head) ? "▷" : "◢"} {c.head.split(" › ").pop() || "Workspace"}</b> <span class="muted">{c.n} plans</span></td>
            <td><div class="meter"><i style={{ width: `${c.pct}%` }} /></div></td><td class="r">{c.cost}</td><td /><td /><td class="r" classList={{ late: c.late > 0 }}>{c.late}</td></tr>
        ) : <Show when={!shut().some(h => c.folder === h || c.folder.startsWith(h + " › "))}><tr class="link" onClick={() => loadOut("open", { path: c.path })}>
          <td style={{ "padding-left": `${22 + c.depth * 16}px` }}><b>{c.name}</b>{c.error && <div class="late">{c.error}</div>}</td><td>{c.manager}</td><td>{c.start}</td><td>{c.finish}</td><td class="r">{c.tasks}</td>
          <td><div class="meter"><i style={{ width: `${c.pct}%` }} /></div></td><td class="r">{c.cost}</td>
          <td class="r" classList={{ late: c.spi !== null && c.spi < 0.95 }}>{c.spi?.toFixed(2) ?? "—"}</td><td class="r" classList={{ late: c.cpi !== null && c.cpi < 0.95 }}>{c.cpi?.toFixed(2) ?? "—"}</td>
          <td class="r" classList={{ late: c.late > 0 }}>{c.late}</td>
        </tr></Show>}</For></tbody>
      </table><Show when={!data()?.[1].length}><p class="empty">Choose a folder that holds your .dplan files to see every plan side by side. Subfolders become the EPS (enterprise project structure) levels.</p></Show></div>
    </Page>
  );
};
export { Chk, openMenu, setSel, setBackstage };
