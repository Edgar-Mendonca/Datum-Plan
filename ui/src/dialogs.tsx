// Dialogs: information, settings and editors, MS Project style.
import { For, Show, createSignal, createMemo, Index } from "solid-js";
import { createStore, produce } from "solid-js/store";
import { Calendar, CustomDef, Filter, Rule, Out, Diff, iso, dateDay, invoke } from "./api";
import { act, call, closeDialog, cols, fields, filter, group, openDialog, out, P, saveCols, setFilter, setGroup, setSort, sort, toast, setOut, useTable, tables, table, opts, setBackstage, grid, query } from "./store";
import { Dialog, Row, Text, Pick, Check, Tabs } from "./ui";
import { TaskTab, useInfo, TABS } from "./details";

const KINDS = ["text", "number", "flag", "date", "cost", "duration", "start", "finish", "outline"];
const COUNT: Record<string, number> = { text: 30, number: 20, flag: 20, date: 10, cost: 10, duration: 10, start: 10, finish: 10, outline: 10 };
const TESTS = ["equals", "does not equal", "is greater than", "is greater than or equal to", "is less than", "is less than or equal to", "is within", "is not within", "contains", "does not contain", "contains exactly"];
const fieldOpts = () => fields().task.map(f => [f.key, f.name] as [string, string]);

// ---------- Task Information ----------
export const taskInfo = (uid: number, start = "general") => openDialog(() => {
  const [tab, setTab] = createSignal(start);
  const [info] = useInfo(() => uid);
  return (
    <Dialog title="Task Information" wide>
      <Show when={info()}>{i => <>
        <div class="tihead"><b>{i().task.code}</b> {i().task.name}</div>
        <Tabs tabs={TABS.filter(t => t[0] !== "risks")} on={tab()} set={setTab} />
        <div class="dbody tibody"><TaskTab info={i()} tab={tab()} /></div>
      </>}</Show>
    </Dialog>
  );
});

/** Every field of a task (MS Project Inspect / Task Form with all fields). */
export const inspect = (uid?: number) => uid && openDialog(() => {
  const [info] = useInfo(() => uid);
  const [find, setFind] = createSignal("");
  return (
    <Dialog title="Inspect task: all fields" wide>
      <input class="find" placeholder="Find a field" value={find()} onInput={e => setFind(e.currentTarget.value)} />
      <div class="fieldlist"><Show when={info()}>{i => <table class="tbl"><tbody>
        <For each={i().fields.filter(f => (f[3] || find()) && (f[1] + f[2]).toLowerCase().includes(find().toLowerCase()))}>{f => <tr><td class="muted">{f[2]}</td><td>{f[1]}</td><td><b>{f[3]}</b></td></tr>}</For>
      </tbody></table>}</Show></div>
    </Dialog>
  );
});

// ---------- Project Information ----------
export const projectInfo = () => openDialog(() => {
  const p = P();
  const [v, setV] = createStore({ name: p.name, start: p.start, status: p.status ?? "", calendar: p.calendar, day_hours: p.day_min / 60, manual_default: p.manual_default,
    code: p.code, description: p.description, client: p.client, manager: p.manager, location: p.location, company: p.company });
  const s = out()!.summary;
  return (
    <Dialog title={`Project Information for '${p.name}'`} wide ok={async () => {
      const ok = await act({ Batch: [
        { Project: { name: v.name, start: v.start, status: v.status, calendar: v.calendar, day_hours: +v.day_hours, manual_default: v.manual_default } },
        ...(["code", "description", "client", "manager", "location", "company"] as const).map(k => ({ ProjectSet: { field: k, value: v[k] } })),
      ] });
      return ok;
    }}>
      <div class="cols2">
        <div class="form">
          <Row label="Project name"><Text value={v.name} set={x => setV("name", x)} /></Row>
          <Row label="Project code"><Text value={v.code} set={x => setV("code", x)} /></Row>
          <Row label="Start date"><Text type="date" value={v.start} set={x => setV("start", x)} /></Row>
          <Row label="Status date"><Text type="date" value={v.status} set={x => setV("status", x)} /></Row>
          <Row label="Calendar"><Pick value={v.calendar} set={x => setV("calendar", x)} options={p.calendars.map(c => c.name)} /></Row>
          <Row label="Hours per day"><Text type="number" value={v.day_hours} set={x => setV("day_hours", +x)} /></Row>
          <Row label="New tasks"><Pick value={v.manual_default ? "m" : "a"} set={x => setV("manual_default", x === "m")} options={[["a", "Auto scheduled"], ["m", "Manually scheduled"]]} /></Row>
        </div>
        <div class="form">
          <Row label="Client"><Text value={v.client} set={x => setV("client", x)} /></Row>
          <Row label="Manager"><Text value={v.manager} set={x => setV("manager", x)} /></Row>
          <Row label="Company"><Text value={v.company} set={x => setV("company", x)} /></Row>
          <Row label="Location"><Text value={v.location} set={x => setV("location", x)} /></Row>
          <Row label="Description"><textarea value={v.description} onChange={e => setV("description", e.currentTarget.value)} /></Row>
        </div>
      </div>
      <h4>Statistics</h4>
      <table class="tbl stats"><thead><tr><th /><th>Start</th><th>Finish</th></tr></thead><tbody>
        <tr><td>Current</td><td>{s.start}</td><td>{s.finish}</td></tr><tr><td>Baseline</td><td>—</td><td>{s.bl_finish || "—"}</td></tr><tr><td>Variance</td><td /><td>{s.var_finish || "—"}</td></tr>
      </tbody></table>
      <table class="tbl stats"><thead><tr><th>Duration</th><th>Work</th><th>Cost</th><th>Complete</th><th>Tasks</th><th>PV</th><th>EV</th><th>AC</th></tr></thead>
        <tbody><tr><td>{s.dur}</td><td>{s.work}</td><td>{s.cost}</td><td>{Math.round(s.pct)}%</td><td>{s.done} / {s.tasks}</td><td>{s.pv}</td><td>{s.ev}</td><td>{s.ac}</td></tr></tbody></table>
    </Dialog>
  );
});

// ---------- Resource Information ----------
export const resourceInfo = (uid: number) => openDialog(() => {
  const r = () => P().resources.find(x => x.uid === uid)!;
  const [tab, setTab] = createSignal("general");
  const [tables, setTables] = createStore<any[][]>(JSON.parse(JSON.stringify(r().tables.length ? r().tables : [[{ from: null, std: r().rate, ovt: r().ovt, per_use: r().per_use }]])));
  const [avail, setAvail] = createStore<any[]>(JSON.parse(JSON.stringify(r().avail)));
  const [t, setT] = createSignal(0);
  return (
    <Dialog title={`Resource Information: ${r().name}`} wide ok={() => act({ Batch: [{ ResJson: { uid, field: "tables", value: tables } }, { ResJson: { uid, field: "avail", value: avail } }] })}>
      <Tabs tabs={[["general", "General"], ["costs", "Costs"], ["avail", "Availability"], ["notes", "Notes"]]} on={tab()} set={setTab} />
      <div class="dbody tibody">
        <Show when={tab() === "general"}><div class="form">
          <For each={[["name", "Name"], ["initials", "Initials"], ["group", "Group"], ["code", "Code"], ["email", "Email"], ["label", "Material label"]] as [string, string][]}>{([k, l]) =>
            <Row label={l}><Text value={(r() as any)[k]} set={x => act({ ResSet: { uid, field: k, value: x } })} /></Row>}</For>
          <Row label="Type"><Pick value={r().kind} set={x => act({ ResSet: { uid, field: "kind", value: x } })} options={["Work", "Material", "Cost"]} /></Row>
          <Row label="Max units"><Text value={`${Math.round(r().max * 100)}%`} set={x => act({ ResSet: { uid, field: "max", value: x } })} /></Row>
          <Row label="Base calendar"><Pick value={r().cal ?? ""} set={x => act({ ResJson: { uid, field: "cal", value: x || null } })} options={[["", "(project calendar)"], ...P().calendars.map(c => [c.name, c.name] as [string, string])]} /></Row>
        </div></Show>
        <Show when={tab() === "costs"}>
          <div class="ptools">Cost rate table <Index each={["A", "B", "C", "D", "E"]}>{(l, k) => <button class="btn sm" classList={{ pri: t() === k }} onClick={() => { if (!tables[k]) setTables(k, [{ from: null, std: 0, ovt: 0, per_use: 0 }]); setT(k); }}>{l()}</button>}</Index></div>
          <table class="tbl"><thead><tr><th>Effective date</th><th>Standard rate</th><th>Overtime rate</th><th>Per use cost</th><th /></tr></thead><tbody>
            <For each={tables[t()] ?? []}>{(x, k) => <tr>
              <td><input type="date" value={x.from ?? ""} disabled={k() === 0} onChange={e => setTables(t(), k(), "from", e.currentTarget.value || null)} /></td>
              <td><input type="number" value={x.std} onChange={e => setTables(t(), k(), "std", +e.currentTarget.value)} /></td>
              <td><input type="number" value={x.ovt} onChange={e => setTables(t(), k(), "ovt", +e.currentTarget.value)} /></td>
              <td><input type="number" value={x.per_use} onChange={e => setTables(t(), k(), "per_use", +e.currentTarget.value)} /></td>
              <td><button class="x" disabled={k() === 0} onClick={() => setTables(t(), v => v.filter((_, j) => j !== k()))}>×</button></td></tr>}</For>
            <tr class="add"><td colspan="5"><button class="btn sm" onClick={() => setTables(t(), v => [...(v ?? []), { from: iso(dateDay(new Date())), std: 0, ovt: 0, per_use: 0 }])}>+ Rate change</button></td></tr>
          </tbody></table><p class="muted">Assignments use table A unless set otherwise. Rates change on their effective dates.</p>
        </Show>
        <Show when={tab() === "avail"}>
          <table class="tbl"><thead><tr><th>Available from</th><th>Available to</th><th>Units</th><th /></tr></thead><tbody>
            <For each={avail}>{(x, k) => <tr><td><input type="date" value={x.from ?? ""} onChange={e => setAvail(k(), "from", e.currentTarget.value || null)} /></td>
              <td><input type="date" value={x.to ?? ""} onChange={e => setAvail(k(), "to", e.currentTarget.value || null)} /></td>
              <td><input type="number" value={x.max * 100} onChange={e => setAvail(k(), "max", +e.currentTarget.value / 100)} />%</td>
              <td><button class="x" onClick={() => setAvail(v => v.filter((_, j) => j !== k()))}>×</button></td></tr>}</For>
            <tr class="add"><td colspan="4"><button class="btn sm" onClick={() => setAvail(v => [...v, { from: null, to: null, max: 1 }])}>+ Period</button></td></tr>
          </tbody></table>
        </Show>
        <Show when={tab() === "notes"}><textarea class="dnotes" value={r().notes} onChange={e => act({ ResSet: { uid, field: "notes", value: e.currentTarget.value } })} /></Show>
      </div>
    </Dialog>
  );
});

// ---------- Change Working Time (calendars) ----------
const hm = (m: number) => `${String(Math.floor(m / 60)).padStart(2, "0")}:${String(m % 60).padStart(2, "0")}`;
const parseHours = (s: string): [number, number][] => s.split(",").map(x => x.trim()).filter(Boolean).map(x => { const [a, b] = x.split("-").map(t => { const [h, m] = t.trim().split(":").map(Number); return h * 60 + (m || 0); }); return [a, b] as [number, number]; });
const fmtHours = (h: [number, number][]) => h.map(([a, b]) => `${hm(a)}-${hm(b)}`).join(", ");
export const calendars = () => openDialog(() => {
  const [name, setName] = createSignal(P().calendar);
  const cal = () => P().calendars.find(c => c.name === name()) ?? P().calendars[0];
  const [draft, setDraft] = createStore<Calendar>(JSON.parse(JSON.stringify(cal())));
  const load = (n: string) => { setName(n); setDraft(JSON.parse(JSON.stringify(cal()))); };
  const save = () => act({ CalPut: { old: name(), cal: draft } }).then(ok => { if (ok) setName(draft.name); return ok; });
  return (
    <Dialog title="Change Working Time" wide ok={save}>
      <div class="ptools">
        For calendar <Pick value={name()} set={load} options={P().calendars.map(c => c.name)} />
        <button class="btn sm" onClick={async () => { const n = prompt("Name of the new calendar", "New calendar"); if (n && await act({ CalPut: { old: null, cal: { ...JSON.parse(JSON.stringify(draft)), name: n } } })) load(n); }}>Create New Calendar…</button>
        <button class="btn sm" onClick={async () => { if (await act({ CalDelete: { name: name() } })) load(P().calendar); }}>Delete</button>
        <span class="grow" /><label>Name <input value={draft.name} onChange={e => setDraft("name", e.currentTarget.value)} /></label>
      </div>
      <div class="cols2">
        <div><h4>Work weeks</h4><table class="tbl"><tbody>
          <For each={["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"]}>{(d, k) => <tr>
            <td>{d}</td><td><input type="checkbox" checked={draft.week[k()].length > 0} onChange={e => setDraft("week", k(), e.currentTarget.checked ? [[480, 720], [780, 1020]] : [])} /></td>
            <td><input value={fmtHours(draft.week[k()])} placeholder="Nonworking" onChange={e => setDraft("week", k(), parseHours(e.currentTarget.value))} /></td></tr>}</For>
        </tbody></table><p class="muted">Working times like 08:00-12:00, 13:00-17:00. 24 hours: 00:00-24:00.</p></div>
        <div><h4>Exceptions (holidays, special working days)</h4><table class="tbl"><thead><tr><th>Name</th><th>From</th><th>To</th><th>Working times</th><th /></tr></thead><tbody>
          <For each={draft.exceptions}>{(x, k) => <tr>
            <td><input value={x.name} onChange={e => setDraft("exceptions", k(), "name", e.currentTarget.value)} /></td>
            <td><input type="date" value={x.from} onChange={e => setDraft("exceptions", k(), "from", e.currentTarget.value)} /></td>
            <td><input type="date" value={x.to} onChange={e => setDraft("exceptions", k(), "to", e.currentTarget.value)} /></td>
            <td><input value={fmtHours(x.hours)} placeholder="Nonworking" onChange={e => setDraft("exceptions", k(), "hours", parseHours(e.currentTarget.value))} /></td>
            <td><button class="x" onClick={() => setDraft("exceptions", v => v.filter((_, j) => j !== k()))}>×</button></td></tr>}</For>
          <tr class="add"><td colspan="5"><button class="btn sm" onClick={() => { const d = iso(dateDay(new Date())); setDraft("exceptions", v => [...v, { name: "Holiday", from: d, to: d, hours: [] }]); }}>+ Exception</button></td></tr>
        </tbody></table></div>
      </div>
    </Dialog>
  );
});

// ---------- Schedule Options ----------
export const scheduleOptions = () => openDialog(() => {
  const [o, setO] = createStore({ ...P().opts });
  return (
    <Dialog title="Schedule Options" ok={() => act({ ProjectSet: { field: "opts", value: { ...o, critical_float: Math.round(+o.critical_float) } } })}>
      <div class="form">
        <Row label="Out-of-sequence progress"><Pick value={o.retained ? "r" : "p"} set={x => setO("retained", x === "r")} options={[["r", "Retained logic"], ["p", "Progress override"]]} /></Row>
        <Row label="Critical tasks"><Pick value={o.longest_path ? "lp" : "tf"} set={x => setO("longest_path", x === "lp")} options={[["tf", "Total float ≤ limit"], ["lp", "Longest path"]]} /></Row>
        <Row label="Critical when float ≤ (days)"><Text type="number" value={o.critical_float / P().day_min} set={x => setO("critical_float", +x * P().day_min)} /></Row>
        <Row label="Lag calendar"><Pick value={o.lag_cal} set={x => setO("lag_cal", x)} options={[["Successor", "Successor's calendar"], ["Predecessor", "Predecessor's calendar"], ["H24", "24-hour calendar"], ["Project", "Project calendar"]]} /></Row>
        <Row label="Float paths"><Text type="number" value={o.float_paths} set={x => setO("float_paths", +x)} /></Row>
        <Row label="New tasks"><Pick value={o.task_type_default} set={x => setO("task_type_default", x)} options={[["FixedDuration", "Fixed Duration"], ["FixedUnits", "Fixed Units"], ["FixedWork", "Fixed Work"]]} /></Row>
        <Row label=""><Check label="New tasks are effort driven" value={o.effort_default} set={x => setO("effort_default", x)} /></Row>
        <Row label=""><Check label="Schedule from the project finish date" value={o.from_finish} set={x => setO("from_finish", x)} /></Row>
        <Row label="Finish date"><Text type="date" value={o.finish_date} disabled={!o.from_finish} set={x => setO("finish_date", x || null)} /></Row>
        <Row label=""><Check label="Show open-ended tasks as critical" value={o.open_ends_critical} set={x => setO("open_ends_critical", x)} /></Row>
        <Row label="Leveling order"><Pick value={o.level_order} set={x => setO("level_order", x)} options={[["Standard", "Standard"], ["Priority", "Priority, Standard"], ["IdOnly", "ID only"]]} /></Row>
        <Row label=""><Check label="Level only within available slack" value={o.level_within_slack} set={x => setO("level_within_slack", x)} /></Row>
      </div>
    </Dialog>
  );
});

// ---------- Baselines, update, split, snapshot ----------
export const setBaseline = (uids: number[]) => openDialog(() => {
  const [n, setN] = createSignal(0), [all, setAll] = createSignal(true), [clear, setClear] = createSignal(false);
  return (
    <Dialog title="Set Baseline" ok={() => act({ Baseline: { uids: all() ? [] : uids, clear: clear(), n: n() } }).then(ok => { if (ok) toast(clear() ? "Baseline cleared." : "Baseline saved.", true); return ok; })}>
      <div class="form">
        <Row label="Action"><Pick value={clear() ? "c" : "s"} set={x => setClear(x === "c")} options={[["s", "Set baseline"], ["c", "Clear baseline"]]} /></Row>
        <Row label="Baseline"><Pick value={String(n())} set={x => setN(+x)} options={Array.from({ length: 11 }, (_, k) => [String(k), k ? `Baseline ${k}` : "Baseline"] as [string, string])} /></Row>
        <Row label="For"><Pick value={all() ? "a" : "s"} set={x => setAll(x === "a")} options={[["a", "Entire project"], ["s", `Selected tasks (${uids.length})`]]} /></Row>
      </div>
    </Dialog>
  );
});
export const updateProject = () => openDialog(() => {
  const [d, setD] = createSignal(P().status ?? iso(dateDay(new Date()))), [re, setRe] = createSignal(false);
  return (
    <Dialog title="Update Project" ok={() => act({ UpdateProject: { through: d(), reschedule: re() } })}>
      <div class="form">
        <Row label="Update work as complete through"><Text type="date" value={d()} set={setD} /></Row>
        <Row label=""><Check label="Reschedule uncompleted work to start after this date" value={re()} set={setRe} /></Row>
      </div><p class="muted">Sets % complete as planned up to the date, sets the status date, and keeps actual dates.</p>
    </Dialog>
  );
});
export const splitTask = (uid: number) => openDialog(() => {
  const [after, setAfter] = createSignal("1d"), [gap, setGap] = createSignal("1d");
  const mins = (s: string) => { const m = s.trim().match(/^([\d.]+)\s*(m|h|d|w)?/i); if (!m) return 0; const v = +m[1]; const u = (m[2] || "d").toLowerCase(); return Math.round(v * (u === "m" ? 1 : u === "h" ? 60 : u === "w" ? 5 * P().day_min : P().day_min)); };
  return (
    <Dialog title="Split Task" ok={() => act({ Split: { uid, after: mins(after()), gap: mins(gap()) } })}>
      <div class="form"><Row label="After working"><Text value={after()} set={setAfter} /></Row><Row label="Pause for"><Text value={gap()} set={setGap} /></Row></div>
      <p class="muted">A pause of 0 removes the split at that point.</p>
    </Dialog>
  );
});
export const snapshot = () => openDialog(() => {
  const [label, setLabel] = createSignal(`Period ending ${P().status ?? iso(dateDay(new Date()))}`), [comment, setComment] = createSignal("");
  return (
    <Dialog title="Store Period Performance" ok={() => act({ Snapshot: { label: label(), comment: comment() } }).then(ok => { if (ok) toast("Snapshot stored.", true); return ok; })}>
      <div class="form"><Row label="Label"><Text value={label()} set={setLabel} /></Row><Row label="Comment"><Text value={comment()} set={setComment} /></Row></div>
      <p class="muted">Freezes dates, % complete and cost at the status date for trend reports.</p>
    </Dialog>
  );
});

// ---------- columns, field settings, tables ----------
export const insertColumn = (run?: (key: string) => void) => openDialog(() => {
  const [find, setFind] = createSignal("");
  const list = createMemo(() => [...fields().task.map(f => [f.key, f.name, f.group] as [string, string, string]), ...P().codes.map(c => [`code:${c.name}`, c.name, "Activity codes"] as [string, string, string])]
    .map(([k, n, g]) => { const a = k.startsWith("c:") && P().fields.find(f => f.key === k.slice(2))?.alias; return [k, a ? `${a} (${n})` : n, g] as [string, string, string]; })
    .filter(x => (x[1] + x[2]).toLowerCase().includes(find().toLowerCase())));
  const add = (k: string) => { (run ?? ((key: string) => saveCols([...cols(), [key, 90]])))(k); closeDialog(); };
  return (
    <Dialog title="Insert Column">
      <input class="find" placeholder={`Search ${fields().task.length} fields`} value={find()} onInput={e => setFind(e.currentTarget.value)} />
      <div class="fieldlist"><table class="tbl"><tbody><For each={list().slice(0, 300)}>{([k, n, g]) => <tr class="link" onClick={() => add(k)}><td>{n}</td><td class="muted">{g}</td></tr>}</For></tbody></table></div>
    </Dialog>
  );
});
export const fieldSettings = (k: number) => openDialog(() => {
  const c = cols()[k];
  const [key, setKey] = createSignal(c[0]), [w, setW] = createSignal(c[1]);
  return (
    <Dialog title="Field Settings" ok={() => { const v = [...cols()]; v[k] = [key(), +w()]; saveCols(v); }}>
      <div class="form">
        <Row label="Field name"><Pick value={key()} set={setKey} options={fieldOpts()} /></Row>
        <Row label="Width"><Text type="number" value={w()} set={x => setW(+x)} /></Row>
        <Row label="Move"><div><button class="btn sm" onClick={() => { if (k > 0) { const v = [...cols()]; [v[k - 1], v[k]] = [v[k], v[k - 1]]; saveCols(v); closeDialog(); } }}>← Left</button> <button class="btn sm" onClick={() => { const v = [...cols()]; if (k < v.length - 1) { [v[k + 1], v[k]] = [v[k], v[k + 1]]; saveCols(v); closeDialog(); } }}>Right →</button></div></Row>
      </div><p class="muted">Rename a custom field in Custom Fields (its alias is shown as the column title).</p>
    </Dialog>
  );
});
export const tablesDialog = () => openDialog(() => {
  const [name, setName] = createSignal("");
  return (
    <Dialog title="More Tables">
      <table class="tbl"><tbody><For each={Object.keys(tables())}>{t => <tr class="link" classList={{ sel: t === table() }} onClick={() => { useTable(t); closeDialog(); }}><td>{t}</td><td class="muted">{tables()[t].length} columns</td></tr>}</For></tbody></table>
      <div class="ptools"><input placeholder="New table name" value={name()} onInput={e => setName(e.currentTarget.value)} />
        <button class="btn sm" disabled={!name()} onClick={() => { act({ Ui: { key: "tables", value: { ...(P().ui?.tables ?? {}), [name()]: cols() } } }).then(() => useTable(name())); closeDialog(); }}>Copy current table</button></div>
    </Dialog>
  );
});

// ---------- custom fields: values, lookup, formula, rollup, graphical indicators ----------
export const customFields = (start?: string) => openDialog(() => {
  const [kind, setKind] = createSignal(start ? start.replace(/\d+$/, "").replace(/^r/, "") : "text");
  const [key, setKey] = createSignal(start ?? "text1");
  const blank = (k: string): CustomDef => ({ key: k, alias: "", lookup: [], restrict: false, default: "", formula: "", rollup: "None", indicators: [], show_icons: false });
  const [d, setD] = createStore<CustomDef>(JSON.parse(JSON.stringify(P().fields.find(f => f.key === key()) ?? blank(key()))));
  const pick = (k: string) => { setKey(k); setD(JSON.parse(JSON.stringify(P().fields.find(f => f.key === k) ?? blank(k)))); };
  const [mode, setMode] = createSignal<"none" | "lookup" | "formula">(d.formula ? "formula" : d.lookup.length ? "lookup" : "none");
  const [err, setErr] = createSignal("");
  const save = async () => { const def = { ...d, formula: mode() === "formula" ? d.formula : "", lookup: mode() === "lookup" ? d.lookup : [] }; const ok = await act({ FieldDef: { def } }, true); if (!ok) setErr("The formula has an error. Check field names in [brackets] and the function names."); else setErr(""); return ok; };
  const insert = (t: string) => setD("formula", d.formula + t);
  return (
    <Dialog title="Custom Fields" wide ok={save} extra={<button class="btn" onClick={save}>Apply</button>}>
      <div class="cols2">
        <div>
          <div class="ptools">Type <Pick value={kind()} set={k => { setKind(k); pick(`${k}1`); }} options={KINDS.map(k => [k, k[0].toUpperCase() + k.slice(1)] as [string, string])} /></div>
          <div class="fieldlist short"><table class="tbl"><tbody><For each={Array.from({ length: COUNT[kind()] }, (_, k) => `${kind()}${k + 1}`)}>{k => {
            const f = () => P().fields.find(x => x.key === k);
            return <tr class="link" classList={{ sel: k === key() }} onClick={() => pick(k)}><td>{k[0].toUpperCase() + k.slice(1)}</td><td>{f()?.alias}</td><td class="muted">{f()?.formula ? "formula" : f()?.lookup.length ? "lookup" : ""}</td></tr>;
          }}</For></tbody></table></div>
        </div>
        <div class="form">
          <Row label="Field"><b>{key()}</b></Row>
          <Row label="Rename (alias)"><Text value={d.alias} set={x => setD("alias", x)} placeholder="e.g. Discipline" /></Row>
          <Row label="Custom attributes"><Pick value={mode()} set={x => setMode(x as any)} options={[["none", "None (typed values)"], ["lookup", "Lookup table"], ["formula", "Formula"]]} /></Row>
          <Show when={mode() === "lookup"}><Row label="Lookup values"><div>
            <table class="tbl"><tbody><For each={d.lookup}>{(l, k) => <tr><td><input value={l.value} onChange={e => setD("lookup", k(), "value", e.currentTarget.value)} /></td><td><input value={l.desc} placeholder="Description" onChange={e => setD("lookup", k(), "desc", e.currentTarget.value)} /></td><td><button class="x" onClick={() => setD("lookup", v => v.filter((_, j) => j !== k()))}>×</button></td></tr>}</For>
              <tr class="add"><td colspan="3"><button class="btn sm" onClick={() => setD("lookup", v => [...v, { value: "", desc: "" }])}>+ Value</button></td></tr></tbody></table>
            <Check label="Only allow values in the list" value={d.restrict} set={x => setD("restrict", x)} /><div>Default <input value={d.default} onChange={e => setD("default", e.currentTarget.value)} /></div>
          </div></Row></Show>
          <Show when={mode() === "formula"}><Row label="Formula"><div>
            <textarea class="formula" value={d.formula} onInput={e => setD("formula", e.currentTarget.value)} placeholder='IIf([% Complete]=100,"Done",IIf([Finish]<Now(),"Late","On time"))' />
            <div class="ptools">
              <select onChange={e => { insert(`[${e.currentTarget.value}]`); e.currentTarget.value = ""; }}><option value="">Field ▾</option><For each={fields().task.filter(f => !f.key.startsWith("c:"))}>{f => <option>{f.name}</option>}</For></select>
              <select onChange={e => { insert(e.currentTarget.value); e.currentTarget.value = ""; }}><option value="">Function ▾</option><For each={["IIf(, , )", "Switch(, )", "Choose(, )", "Abs()", "Round(, 0)", "Int()", "Len()", "Left(, )", "Mid(, , )", "InStr(, )", "UCase()", "Trim()", "IsNull()", "Nz(, 0)", "Now()", "Date()", "ProjDateDiff(, )", "ProjDateAdd(, )", "DateDiff(\"d\", , )", "DateAdd(\"d\", , )", "Year()", "Month()", "Weekday()", "CStr()", "CDbl()", "Max(, )", "Min(, )"]}>{f => <option>{f}</option>}</For></select>
              <For each={["+", "-", "*", "/", "&", "=", "<>", "<", ">", "And", "Or", "Not"]}>{o => <button class="btn sm" onClick={() => insert(` ${o} `)}>{o}</button>}</For>
            </div>
            <Show when={err()}><div class="late">{err()}</div></Show>
          </div></Row></Show>
          <Row label="Summary rows"><Pick value={d.rollup} set={x => setD("rollup", x)} options={["None", "Max", "Min", "Sum", "Average", "Count"].map(x => [x, x === "None" ? "Same as tasks" : `Rollup: ${x}`] as [string, string])} /></Row>
          <Row label="Graphical indicators"><div>
            <table class="tbl"><thead><tr><th>Rows</th><th>Test</th><th>Value</th><th>Icon</th><th /></tr></thead><tbody>
              <For each={d.indicators}>{(x, k) => <tr>
                <td><select value={x.scope} onChange={e => setD("indicators", k(), "scope", +e.currentTarget.value)}><option value="0">Tasks</option><option value="1">Summary</option></select></td>
                <td><select value={x.test} onChange={e => setD("indicators", k(), "test", e.currentTarget.value)}><For each={[...TESTS, "is any value"]}>{t => <option>{t}</option>}</For></select></td>
                <td><input value={x.value} size="8" onChange={e => setD("indicators", k(), "value", e.currentTarget.value)} /></td>
                <td><select value={x.icon} onChange={e => setD("indicators", k(), "icon", e.currentTarget.value)}><For each={["🟢", "🟡", "🔴", "🔵", "⚪", "✔", "✖", "⚠", "★", "▲", "▼", "◆", "⚑"]}>{i => <option>{i}</option>}</For></select></td>
                <td><button class="x" onClick={() => setD("indicators", v => v.filter((_, j) => j !== k()))}>×</button></td></tr>}</For>
              <tr class="add"><td colspan="5"><button class="btn sm" onClick={() => setD("indicators", v => [...v, { scope: 0, test: "equals", value: "", icon: "🟢" }])}>+ Indicator</button></td></tr>
            </tbody></table><Check label="Show icons instead of values" value={d.show_icons} set={x => setD("show_icons", x)} />
          </div></Row>
        </div>
      </div>
    </Dialog>
  );
});

// ---------- activity codes ----------
export const activityCodes = () => openDialog(() => {
  const [name, setName] = createSignal(P().codes[0]?.name ?? "");
  const code = () => P().codes.find(c => c.name === name());
  const put = (def: any) => act({ CodeDef: { old: name() || null, def } });
  return (
    <Dialog title="Activity Codes" wide>
      <div class="cols2">
        <div><table class="tbl"><tbody><For each={P().codes}>{c => <tr class="link" classList={{ sel: c.name === name() }} onClick={() => setName(c.name)}><td>{c.name}</td><td class="muted">{c.values.length} values</td></tr>}</For></tbody></table>
          <button class="btn sm" onClick={async () => { const n = prompt("Code name (e.g. Phase, Area, Discipline)"); if (n && await act({ CodeDef: { old: null, def: { name: n, scope: "Project", values: [] } } })) setName(n); }}>+ New code</button>
          <Show when={code()}><button class="btn sm" onClick={() => act({ CodeDef: { old: name(), def: null } }).then(() => setName(P().codes[0]?.name ?? ""))}>Delete</button></Show></div>
        <Show when={code()} fallback={<p class="empty">Codes classify tasks (P6 activity codes): group, filter and colour by them.</p>}>{c => <div>
          <div class="form"><Row label="Name"><Text value={c().name} set={x => put({ ...c(), name: x }).then(() => setName(x))} /></Row></div>
          <table class="tbl"><thead><tr><th>Value</th><th>Description</th><th>Colour</th><th /></tr></thead><tbody>
            <For each={c().values}>{(v, k) => <tr>
              <td><input value={v.value} onChange={e => put({ ...c(), values: c().values.map((x, j) => (j === k() ? { ...x, value: e.currentTarget.value } : x)) })} /></td>
              <td><input value={v.desc} onChange={e => put({ ...c(), values: c().values.map((x, j) => (j === k() ? { ...x, desc: e.currentTarget.value } : x)) })} /></td>
              <td><input type="color" value={v.color || "#6E9FE6"} onChange={e => put({ ...c(), values: c().values.map((x, j) => (j === k() ? { ...x, color: e.currentTarget.value } : x)) })} /></td>
              <td><button class="x" onClick={() => put({ ...c(), values: c().values.filter((_, j) => j !== k()) })}>×</button></td></tr>}</For>
            <tr class="add"><td colspan="4"><button class="btn sm" onClick={() => put({ ...c(), values: [...c().values, { value: `V${c().values.length + 1}`, desc: "", color: "" }] })}>+ Value</button></td></tr>
          </tbody></table></div>}</Show>
      </div>
    </Dialog>
  );
});

// ---------- filter, group, sort ----------
export const filterEditor = (field?: string) => openDialog(() => {
  const [f, setF] = createStore<Filter>(JSON.parse(JSON.stringify(field ? { name: "Custom filter", rules: [{ field, test: "equals", value: "", value2: "", or: false }], summaries: true, highlight: false } : filter().rules.length ? filter() : { name: "Custom filter", rules: [{ field: "name", test: "contains", value: "", value2: "", or: false }], summaries: true, highlight: false })));
  const saved = () => (P().ui?.filters ?? []) as Filter[];
  return (
    <Dialog title="Filter Definition" wide ok={() => setFilter(JSON.parse(JSON.stringify(f)))} okText="Apply"
      extra={<button class="btn" onClick={() => act({ Ui: { key: "filters", value: [...saved().filter(x => x.name !== f.name), JSON.parse(JSON.stringify(f))] } }).then(() => toast("Filter saved with the plan.", true))}>Save filter</button>}>
      <div class="form"><Row label="Name"><Text value={f.name} set={x => setF("name", x)} /></Row></div>
      <table class="tbl"><thead><tr><th>And/Or</th><th>Field name</th><th>Test</th><th>Value(s)</th><th /></tr></thead><tbody>
        <For each={f.rules}>{(r: Rule, k) => <tr>
          <td>{k() ? <select value={r.or ? "or" : "and"} onChange={e => setF("rules", k(), "or", e.currentTarget.value === "or")}><option value="and">And</option><option value="or">Or</option></select> : ""}</td>
          <td><Pick value={r.field} set={x => setF("rules", k(), "field", x)} options={fieldOpts()} /></td>
          <td><Pick value={r.test} set={x => setF("rules", k(), "test", x)} options={TESTS} /></td>
          <td><input value={r.value} onChange={e => setF("rules", k(), "value", e.currentTarget.value)} />{r.test.includes("within") && <> and <input value={r.value2} onChange={e => setF("rules", k(), "value2", e.currentTarget.value)} /></>}</td>
          <td><button class="x" onClick={() => setF("rules", v => v.filter((_, j) => j !== k()))}>×</button></td></tr>}</For>
        <tr class="add"><td colspan="5"><button class="btn sm" onClick={() => setF("rules", v => [...v, { field: "name", test: "equals", value: "", value2: "", or: false }])}>+ Condition</button></td></tr>
      </tbody></table>
      <Check label="Show related summary rows" value={f.summaries} set={x => setF("summaries", x)} /><Check label="Highlight instead of hiding" value={f.highlight} set={x => setF("highlight", x)} />
      <p class="muted">Values can be text, numbers, dates or durations. Use [Field name] to compare with another field.</p>
    </Dialog>
  );
});
export const groupEditor = () => openDialog(() => {
  const [g, setG] = createSignal<string[]>(group().length ? [...group()] : ["critical"]);
  return (
    <Dialog title="Group Definition" ok={() => { setGroup(g().filter(Boolean)); }} okText="Apply">
      <For each={g()}>{(k, j) => <div class="ptools">{j() ? "Then by" : "Group by"} <Pick value={k} set={x => setG(g().map((y, i) => (i === j() ? x : y)))} options={fieldOpts()} /><button class="x" onClick={() => setG(g().filter((_, i) => i !== j()))}>×</button></div>}</For>
      <button class="btn sm" onClick={() => setG([...g(), "res"])}>+ Then by</button>
    </Dialog>
  );
});
export const sortEditor = () => openDialog(() => {
  const [s, setS] = createSignal<[string, boolean][]>(sort().length ? [...sort()] : [["start", false]]);
  return (
    <Dialog title="Sort" ok={() => { setSort(s()); }} okText="Sort" extra={<button class="btn" onClick={() => { setSort([]); closeDialog(); }}>Reset</button>}>
      <For each={s()}>{([k, d], j) => <div class="ptools">{j() ? "Then by" : "Sort by"} <Pick value={k} set={x => setS(s().map((y, i) => (i === j() ? [x, y[1]] : y)))} options={fieldOpts()} />
        <Pick value={d ? "d" : "a"} set={x => setS(s().map((y, i) => (i === j() ? [y[0], x === "d"] : y)))} options={[["a", "Ascending"], ["d", "Descending"]]} /></div>}</For>
      <Show when={s().length < 3}><button class="btn sm" onClick={() => setS([...s(), ["name", false]])}>+ Then by</button></Show>
      <p class="muted">Sorting keeps the outline: tasks are sorted within their summary.</p>
    </Dialog>
  );
});

// ---------- files ----------
export const importFile = async () => {
  const r = await call<{ plan: Out | null; table: { headers: string[]; rows: string[][]; guess: string[] } | null; fields: [string, string][] } | null>("import");
  if (!r) return;
  if (r.plan) { setOut(r.plan); setBackstage(null); toast("Imported. Save it as a .dplan file to keep it.", true); return; }
  const t = r.table!;
  openDialog(() => {
    const [map, setMap] = createSignal([...t.guess]);
    return (
      <Dialog title="Import Wizard: map columns" wide okText="Import" ok={async () => {
        const res = await call<{ plan: Out; skipped: number }>("import_table", { map: map() });
        if (!res) return false;
        setOut(res.plan); setBackstage(null); toast(res.skipped ? `Imported. ${res.skipped} values could not be read and were skipped.` : "Imported.", !res.skipped);
      }}>
        <p class="muted">Choose the Datum Plan field for each column. Columns set to (skip) are left out.</p>
        <div class="mapwrap"><table class="tbl map"><thead><tr><For each={t.headers}>{(h, k) => <th>{h}<br /><select value={map()[k()]} onChange={e => setMap(map().map((x, j) => (j === k() ? e.currentTarget.value : x)))}><option value="">(skip)</option><For each={r.fields}>{f => <option value={f[0]}>{f[1]}</option>}</For></select></th>}</For></tr></thead>
          <tbody><For each={t.rows}>{row => <tr><For each={row}>{c => <td>{c}</td>}</For></tr>}</For></tbody></table></div>
      </Dialog>
    );
  });
};
export const exportAs = async (kind: string) => {
  const path = await call<string | null>("export", { kind, cols: cols().map(c => c[0]) });
  if (path) toast(`Exported to ${path}`, true);
};
export const compare = async () => {
  const r = await call<[string, Diff[]] | null>("compare");
  if (!r) return;
  openDialog(() => (
    <Dialog title={`Compare with ${r[0]}`} wide>
      <p class="muted">{r[1].length} differences. Was = the other file, Now = this plan.</p>
      <div class="fieldlist"><table class="tbl"><thead><tr><th>Change</th><th>Task</th><th>Start was → now</th><th>Finish was → now</th><th>Duration</th><th>Cost</th><th>Finish variance</th></tr></thead>
        <tbody><For each={r[1]}>{d => <tr><td><span class="pill" classList={{ ok: d.change === "Added", err: d.change === "Removed", warn: d.change === "Changed" }}>{d.change}</span></td><td>{d.name}</td>
          <For each={[0, 1, 2, 3]}>{k => <td>{d.was[k] || "—"}{d.was[k] !== d.now[k] ? <> → <b>{d.now[k] || "—"}</b></> : ""}</td>}</For><td>{d.finish_var}</td></tr>}</For></tbody></table></div>
    </Dialog>
  ));
};
export const shortcuts = () => openDialog(() => (
  <Dialog title="Keyboard Shortcuts">
    <table class="keys"><tbody><For each={[["Insert", "New task"], ["Delete", "Delete tasks"], ["Alt+Shift+→ / ←", "Indent / outdent"], ["Ctrl+F2", "Link selected tasks"], ["Shift+F2", "Task information"], ["F2 / Enter", "Edit cell"],
      ["Ctrl+C / X / V", "Copy, cut, paste tasks"], ["Ctrl+Z / Y", "Undo, redo"], ["Ctrl+S", "Save"], ["Ctrl+O", "Open"], ["Ctrl+P", "Print"], ["Ctrl+K or Alt+Q", "Tell me what you want to do"],
      ["Ctrl+wheel", "Zoom the timescale"], ["Ctrl+A", "Select all"], ["Esc", "Cancel"]]}>{([k, v]) => <tr><td>{k}</td><td>{v}</td></tr>}</For></tbody></table>
  </Dialog>
));
/** Confirms closing with unsaved changes. */
export const confirmClose = (then: () => void) => openDialog(() => (
  <Dialog title="Save changes?" okText="Save" cancel="Cancel" ok={async () => { const o = await call<Out | null>("save", { asNew: false }); if (o) then(); return !!o; }}
    extra={<button class="btn" onClick={() => { closeDialog(); then(); }}>Don't save</button>}>
    <p>Do you want to save the changes to “{P().name}”?</p>
  </Dialog>
));
export { produce, opts, grid, query };

// ---------- formula columns (Excel style, by column) ----------
const TYPES: [string, string][] = [["number", "Number"], ["text", "Text"], ["date", "Date"], ["duration", "Duration"], ["cost", "Cost"], ["flag", "Yes/No"]];
const FUNCS: [string, string][] = [
  ["IF(test, then, else)", "IF(, , )"], ["IFS(test1, v1, test2, v2)", "IFS(, , , )"], ["AND / OR / NOT", "AND(, )"], ["ROUND(x, digits)", "ROUND(, 0)"], ["MIN / MAX", "MAX(, )"], ["ABS(x)", "ABS()"],
  ["SUM / AVERAGE", "SUM(, )"], ["IFERROR(x, alt)", "IFERROR(, 0)"], ["TODAY()", "TODAY()"], ["WORKDAY(date, days)", "WORKDAY(, )"], ["NETWORKDAYS(a, b)", "NETWORKDAYS(, )"],
  ["DATEDIF / DAYS(end, start)", "DAYS(, )"], ["EDATE(date, months)", "EDATE(, 1)"], ["YEAR / MONTH / WEEKNUM", "WEEKNUM()"], ["TEXT(x, format)", 'TEXT(, "dd-mmm-yy")'],
  ["LEFT / RIGHT / MID", "LEFT(, 3)"], ["CONCAT(a, b)", "CONCAT(, )"], ["UPPER / LOWER / TRIM", "UPPER()"], ["SWITCH(x, v1, r1, …)", "SWITCH(, , )"],
];
export const formulaColumn = (key?: string, insertAt?: number) => openDialog(() => {
  const existing = key ? P().fields.find(f => f.key === key) : undefined;
  const [kind, setKind] = createSignal(key ? key.replace(/\d+$/, "") : "number");
  const [title, setTitle] = createSignal(existing?.alias ?? "");
  const [f, setF] = createSignal(existing?.formula?.startsWith("=") ? existing.formula : existing?.formula ? `=${existing.formula}` : "=");
  const [err, setErr] = createSignal("");
  let box!: HTMLTextAreaElement;
  const insert = (t: string) => { const a = box.selectionStart ?? f().length, b = box.selectionEnd ?? a; setF(f().slice(0, a) + t + f().slice(b)); setTimeout(() => { box.focus(); box.selectionStart = box.selectionEnd = a + t.length; }); };
  const free = () => { const used = new Set([...P().fields.map(x => x.key), ...cols().map(c => c[0].replace(/^c:/, ""))]); for (let k = 1; k <= 30; k++) { const x = `${kind()}${k}`; if (!used.has(x) && fields().task.some(t => t.key === `c:${x}`)) return x; } return null; };
  const [find, setFind] = createSignal("");
  const list = createMemo(() => fields().task.filter(x => !x.key.startsWith("c:") || P().fields.some(d => `c:${d.key}` === x.key && d.alias)).map(x => [x.key, P().fields.find(d => `c:${d.key}` === x.key)?.alias || P().titles?.[x.key] || x.name] as [string, string]).filter(x => x[1].toLowerCase().includes(find().toLowerCase())).slice(0, 120));
  const save = async () => {
    const k = key ?? free();
    if (!k) { setErr(`All ${kind()} fields are in use.`); return false; }
    if (!title().trim()) { setErr("Give the column a name."); return false; }
    const def = { ...(existing ?? { lookup: [], restrict: false, default: "", rollup: "None", indicators: [], show_icons: false }), key: k, alias: title().trim(), formula: f().trim() === "=" ? "" : f().trim() };
    try { await invoke("apply", { op: { FieldDef: { def } } }).then(o => setOut(o as Out)); } catch (e) { setErr(String(e)); return false; }
    if (!cols().some(c => c[0] === `c:${k}`)) { const v = [...cols()]; v.splice(insertAt ?? v.length, 0, [`c:${k}`, 110]); saveCols(v); }
    toast(`${title()} = ${def.formula || "(no formula)"}`, true);
  };
  return (
    <Dialog title={key ? `Formula for ${existing?.alias || key}` : "New Formula Column"} wide ok={save} okText={key ? "Save" : "Add column"}>
      <div class="cols2">
        <div class="form">
          <Row label="Column name"><Text value={title()} set={setTitle} placeholder="e.g. Progress variance" /></Row>
          <Row label="Type"><Pick value={kind()} set={setKind} options={key ? TYPES.filter(t => t[0] === kind()) : TYPES} /></Row>
          <Row label="Formula"><div>
            <textarea class="formula" ref={box} value={f()} onInput={e => { setF(e.currentTarget.value); setErr(""); }} placeholder="=[% Complete]-[Baseline % Complete]" />
            <div class="ptools"><For each={["+", "-", "*", "/", "&", "=", "<>", ">", "<", "(", ")"]}>{o => <button class="btn sm" onClick={() => insert(o)}>{o}</button>}</For></div>
            <Show when={err()}><div class="late">{err()}</div></Show>
            <p class="muted">Excel formulas, one per column: it is worked out for every task. Refer to other columns by name in square brackets, e.g. <code>=[Cost]-[Baseline Cost]</code>, <code>=IF([Finish]&gt;[Baseline Finish],"Late","On time")</code>. Dates are Excel dates; durations are in days.</p>
          </div></Row>
          <Row label="Functions"><select onChange={e => { insert(e.currentTarget.value); e.currentTarget.value = ""; }}><option value="">Insert function…</option><For each={FUNCS}>{x => <option value={x[1]}>{x[0]}</option>}</For></select></Row>
        </div>
        <div>
          <input class="find" placeholder="Find a field to insert" value={find()} onInput={e => setFind(e.currentTarget.value)} />
          <div class="fieldlist short"><table class="tbl"><tbody><For each={list()}>{([, n]) => <tr class="link" onClick={() => insert(`[${n}]`)}><td>[{n}]</td></tr>}</For></tbody></table></div>
        </div>
      </div>
    </Dialog>
  );
});

// ---------- reporting periods ----------
export const periodSettings = () => openDialog(() => {
  const [o, setO] = createStore({ ...P().periods });
  return (
    <Dialog title="Reporting Periods" ok={() => act({ ProjectSet: { field: "periods", value: { ...o } } })}>
      <div class="form">
        <Row label="Week ends on"><Pick value={String(o.week_end)} set={x => setO("week_end", +x)} options={["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"].map((d, k) => [String(k), d] as [string, string])} /></Row>
        <Row label="Month closes on"><Pick value={String(o.month_end)} set={x => setO("month_end", +x)} options={[["0", "Last day of the month"], ...Array.from({ length: 28 }, (_, k) => [String(k + 1), `Day ${k + 1}`] as [string, string])]} /></Row>
        <Row label="Weight progress by"><Pick value={o.weight} set={x => setO("weight", x)} options={[["duration", "Duration"], ["cost", "Cost (baseline)"], ["work", "Hours"], ["physical", "Physical % complete"]]} /></Row>
      </div>
      <p class="muted">Weekly and monthly reports and S-curves use these cut-offs. They are saved with this plan.</p>
    </Dialog>
  );
});
