// The Procurement Register module: sub-orders from enquiry to delivery, kept as spreadsheets like the
// Document Register (regrid.tsx): the register (one row per order, every milestone planned and
// actual) and the expediting log (one row per note), with the client's lists, own columns and status
// words. The engine works everything out (procure.rs) and offers Orders[…] and Expediting[…] to the
// sheets (regtable.rs); linking orders to the Document Register is the user's choice (Settings).
import { For, Show, createMemo, createResource, createSignal } from "solid-js";
import { Portal } from "solid-js/web";
import { ItemOut, Options, PItem, ProcOut, ProcRegister, RegList } from "./api";
import { act, call, grid, out, P, tick, setView, openDialog, closeDialog, toast, opts, setOpts, openMenu, Item } from "./store";
import { Page } from "./views";
import { Kpi } from "./reports";
import { ChartBox, COL, line } from "./charts";
import { Dialog, Seg, askText, askYes } from "./ui";
import { showTab } from "./plans";
import { asDate, reg as docReg } from "./docreg";
import { Col, Edit, Opt, RegGrid, dt } from "./regrid";
import { ColumnsTab, LCol, LList, ListsTab, WordsTab, editLists, settle } from "./regkit";

const MS = ["RFQ issued", "PO placed", "Manufacturing start", "Inspection release (IRN)", "Delivered"];
const SHORT = ["RFQ", "PO", "Mfg start", "IRN", "Delivery"];
const PRESETS: [string, string, number[]][] = [["balanced", "Balanced: most credit for manufacturing", [5, 15, 60, 10, 10]], ["order", "More credit at the order", [5, 25, 50, 10, 10]], ["delivery", "More credit at delivery", [5, 10, 50, 15, 20]]];
const WORDS: [string, string, string][] = [["not_started", "Not started", "Nothing done yet"], ["rfq", "RFQ issued", "Enquiry sent"], ["po", "PO placed", "Order placed, manufacturing not started"], ["mfg", "In manufacture", "Manufacturing started"],
  ["irn", "Inspection released", "Released after inspection, not delivered"], ["delivered", "Delivered", "Delivered"], ["off", "Not counted", "Its state (such as Cancelled) leaves it out of progress"]];
const SCOL: Record<string, string> = { delivered: "good", irn: "good", mfg: "blue", po: "blue", rfq: "warn", not_started: "grey", off: "grey" };
const LISTS = (): RegList[] => [{ name: "Sub-vendors", field: "vendor", items: [] }, { name: "Countries of origin", field: "origin", items: [] },
  { name: "States", field: "state", items: ["Active", "Hold", "Cancelled"].map(v => ({ value: v, label: "", off: v === "Cancelled" })) }];
const DEF: ProcRegister = { preset: "balanced", weights: [5, 15, 60, 10, 10], weighting: "value", irn_days: 14, warn_days: 14, link_docs: false, items: [], lists: LISTS(), columns: [], words: {} };
export const preg = (): ProcRegister => ({ ...DEF, ...(P().procurement ?? {}) });
const save = (r: Partial<ProcRegister>) => act({ Procurement: { r: { ...preg(), ...r } } });
const blank = (id: number): PItem => ({ id, tag: "", name: "", vendor: "", origin: "", po_no: "", value: 0, manual_w: 1, long_lead: false, task: 0, need: 0, required: null, planned: [null, null, null, null, null], actual: [null, null, null, null, null], mfg_pct: 0, promised: null, log: [], docs: [], remarks: "", state: "", own: {} });
const next = (l: { id: number }[]) => Math.max(0, ...l.map(x => x.id)) + 1;
const today = () => new Date().toISOString().slice(0, 10);
const docsOn = () => (P().modules ?? []).includes("documents");
const listsOf = (r: ProcRegister) => (r.lists?.length ? r.lists : LISTS());
const fieldOf = (it: PItem, f: string) => (f === "vendor" ? it.vendor : f === "origin" ? it.origin : f === "state" ? it.state ?? "" : it.own?.[f] ?? "");
const setField = (it: PItem, f: string, v: string): PItem => (["vendor", "origin", "state"].includes(f) ? { ...it, [f]: v } : { ...it, own: { ...(it.own ?? {}), [f]: v } });
/** A list's values for a drop-down, and any value the orders use that the list lacks. */
const listOpts = (field: string): Opt[] => {
  const items = (listsOf(preg()).find(l => l.field === field)?.items ?? []).map(i => ({ value: i.value, label: i.label || (i.off ? "not counted" : "") }));
  const more = [...new Set(preg().items.map(x => fieldOf(x, field).trim()).filter(v => v && !items.some(i => i.value.toLowerCase() === v.toLowerCase())))].sort();
  return [...items, ...more.map(value => ({ value }))];
};
const tasks = () => grid().rows.filter(t => t.uid > 0);
/** The next milestone not done yet (0–4), or 5 when delivered. */
const nextMs = (it: PItem) => { const k = it.actual.findIndex(a => !a); return k < 0 ? 5 : k; };

// ---------- Settings ----------
const TABS: [string, string][] = [["general", "General"], ["lists", "Lists"], ["columns", "Columns"], ["words", "Status words"], ["templates", "Templates"], ["delete", "Delete register"]];
export const procSettings = (first = "general") => openDialog(() => {
  const start = JSON.parse(JSON.stringify(preg())) as ProcRegister;
  const [r, setR] = createSignal<ProcRegister>(start);
  const [tab, setTab] = createSignal(first);
  const [lists, setLists] = createSignal<LList[]>(editLists(listsOf(start), f => start.items.map(x => fieldOf(x, f))));
  const [cols, setCols] = createSignal<LCol[]>((start.columns ?? []).map(c => ({ ...c, was: c.name })));
  const put = (x: Partial<ProcRegister>) => setR({ ...r(), ...x });
  const w = () => PRESETS.find(p => p[0] === r().preset)?.[2] ?? r().weights;
  const total = () => w().reduce((a, b) => a + b, 0);
  const used = (f: string, v: string) => start.items.filter(x => fieldOf(x, f).toLowerCase() === v.toLowerCase()).length;
  const tpls = () => opts()?.procTemplates ?? {};
  const saveTpl = async () => {
    const name = (await askText("Save as Template", "Template name (for example the client's name)", ""))?.trim(); if (!name) return;
    const t: Partial<ProcRegister> = { preset: r().preset, weights: r().weights, weighting: r().weighting, irn_days: r().irn_days, warn_days: r().warn_days, words: r().words, columns: cols().map(({ was, ...c }) => c),
      lists: lists().map(l => ({ name: l.name, field: l.field, items: l.field === "state" ? l.items.map(({ was, ...i }) => i) : [] })) };
    const o = { ...opts()!, procTemplates: { ...tpls(), [name]: t } } as Options;
    await call("set_options", { opts: o }); setOpts(o); toast(`Template “${name}” saved.`, true);
  };
  const useTpl = (name: string) => {
    const t = JSON.parse(JSON.stringify(tpls()[name] ?? null)) as Partial<ProcRegister> | null; if (!t) return;
    setR({ ...r(), ...t, lists: undefined, columns: undefined } as ProcRegister);
    if (t.columns) setCols([...cols(), ...t.columns.filter(c => !cols().some(x => x.name === c.name))]);
    if (t.lists) setLists(lists().map(l => { const tl = t.lists!.find(x => x.field === l.field); return tl ? { ...l, items: [...l.items, ...tl.items.filter(i => !l.items.some(x => x.value.toLowerCase() === i.value.toLowerCase()))] } : l; }).concat(t.lists.filter(tl => !lists().some(l => l.field === tl.field))));
    toast(`Template “${name}” applied: Save to keep it.`, true);
  };
  const delTpl = async (name: string) => { const m = { ...tpls() }; delete m[name]; const o = { ...opts()!, procTemplates: m } as Options; await call("set_options", { opts: o }); setOpts(o); };
  const ok = async () => {
    if (Math.abs(total() - 100) > 0.01) { toast("The milestone weights must add up to 100 %."); setTab("general"); return false; }
    const s = await settle(start.items, lists(), cols(), listsOf(start), fieldOf, setField);
    return save({ ...r(), items: s.rows, columns: s.columns, lists: s.lists });
  };
  return (
    <Dialog title="Procurement Register Settings" wide help="procurement" okText="Save" ok={ok}
      extra={<div class="inline"><button class="btn sm" onClick={e => openMenu(e, Object.keys(tpls()).length ? Object.keys(tpls()).map(n => ({ label: n, run: () => useTpl(n) })) : [{ label: "No templates yet: Save as Template first", off: true }])}>Use a template ▾</button>
        <button class="btn sm" onClick={saveTpl}>Save as Template…</button></div>}>
      <div class="stabs2"><For each={TABS}>{t => <button classList={{ on: tab() === t[0] }} onClick={() => setTab(t[0])}>{t[1]}</button>}</For></div>
      <Show when={tab() === "general"}><div class="dset">
        <div>
          <h3>Milestone weights (rules of credit)</h3>
          <For each={PRESETS}>{p => <label class="chk"><input type="radio" checked={r().preset === p[0]} onChange={() => put({ preset: p[0] })} /> {p[1]}</label>}</For>
          <label class="chk"><input type="radio" checked={!PRESETS.some(p => p[0] === r().preset)} onChange={() => put({ preset: "own", weights: [...w()] })} /> My own weights</label>
          <table class="tbl"><thead><tr><th>Milestone</th><th class="r">%</th></tr></thead><tbody>
            <For each={MS}>{(m, i) => <tr><td>{m === "Manufacturing start" ? "Manufacturing (pro rata to its %)" : m}</td><td class="r"><input type="number" min="0" max="100" style={{ width: "60px" }} disabled={PRESETS.some(p => p[0] === r().preset)} value={w()[i()]} onChange={e => put({ weights: w().map((v, j) => (j === i() ? +e.currentTarget.value || 0 : v)) })} /></td></tr>}</For>
            <tr class="tot"><td><b>Total</b></td><td class="r" classList={{ bad: Math.abs(total() - 100) > 0.01 }}><b>{total()}</b></td></tr></tbody></table>
        </div>
        <div>
          <h3>Orders weigh against each other by</h3>
          <For each={[["value", "Their PO value (equal when no values are typed)"], ["equal", "Equally"], ["manual", "A weight typed for each"]]}>{([k, l]) => <label class="chk"><input type="radio" checked={r().weighting === k} onChange={() => put({ weighting: k })} /> {l}</label>}</For>
          <h3>Dates</h3>
          <div class="prow"><label>Inspection release (days before delivery, when not typed)<input type="number" min="0" value={r().irn_days} onChange={e => put({ irn_days: +e.currentTarget.value || 0 })} /></label>
            <label>Amber when the float is under (days)<input type="number" min="0" value={r().warn_days} onChange={e => put({ warn_days: +e.currentTarget.value || 0 })} /></label></div>
          <h3>Vendor documents</h3>
          <label class="chk"><input type="checkbox" checked={r().link_docs} onChange={e => put({ link_docs: e.currentTarget.checked })} /> Link orders to the Document Register</label>
          <p class="muted">{r().link_docs ? (docsOn() ? "Each order can list the vendor documents it waits for: the register shows how many are accepted and warns when drawings hold up manufacturing." : "Switch on the Document Register in Project › Modules for the links to count.") : "Off: orders stand on their own, without vendor documents."}</p>
        </div>
      </div></Show>
      <Show when={tab() === "lists"}><ListsTab lists={lists} set={setLists} cols={cols} setCols={setCols} fixed={["vendor", "origin", "state"]} count={used} rows="order" /></Show>
      <Show when={tab() === "columns"}><ColumnsTab cols={cols} set={setCols} table="Orders" examples={["=[Delivery planned]-14", '=IF([Float]<0,"Expedite","")', "=[Forecast]-[Delivery planned]"]} /></Show>
      <Show when={tab() === "words"}><WordsTab words={WORDS} value={() => r().words ?? {}} set={x => put({ words: x })} /></Show>
      <Show when={tab() === "templates"}>
        <p class="muted">A template keeps one client's rules: the milestone weights, the weighting, the inspection release and amber days, status words, your own columns and the states (not the sub-vendors and countries, which belong to the project).</p>
        <div class="inline"><button class="btn" onClick={saveTpl}>Save as Template…</button></div>
        <table class="tbl" style={{ "margin-top": "10px" }}><thead><tr><th>Template</th><th /></tr></thead>
          <tbody><For each={Object.keys(tpls())} fallback={<tr><td class="muted">No templates yet.</td><td /></tr>}>{n => <tr><td><b>{n}</b></td><td class="nowrap"><button class="btn sm" onClick={() => useTpl(n)}>Use</button> <button class="btn sm" onClick={() => delTpl(n)}>Delete</button></td></tr>}</For></tbody></table>
      </Show>
      <Show when={tab() === "delete"}>
        <p>Delete the register's contents. Edit › Undo (Ctrl+Z) brings them back while the plan is open.</p>
        <div class="inline"><button class="btn" onClick={async () => { if (await askYes("Delete every order", `Delete all ${preg().items.length} orders and their expediting notes? The settings and lists stay.`, "Delete orders")) { await save({ items: [] }); closeDialog(); } }}>Delete every order…</button>
          <button class="btn danger" onClick={async () => { if (await askYes("Delete the register", "Delete the whole register: orders, notes, settings, lists and columns? The module stays on, empty.", "Delete the register")) { await act({ Procurement: { r: { ...DEF, lists: LISTS() } } }); closeDialog(); } }}>Delete the whole register…</button></div>
        <p class="muted">To take the module away from the plan: Project › Modules, untick the Procurement Register.</p>
      </Show>
    </Dialog>
  );
});

// ---------- An order's details ----------
const editItem = (it: PItem) => openDialog(() => {
  const [x, setX] = createSignal<PItem>(JSON.parse(JSON.stringify(it)));
  const put = (v: Partial<PItem>) => setX({ ...x(), ...v });
  const [dq, setDq] = createSignal("");
  const T = (p: { l: string; k: "tag" | "name" | "po_no" | "remarks" }) => <label>{p.l}<input value={x()[p.k]} onChange={e => put({ [p.k]: e.currentTarget.value } as Partial<PItem>)} /></label>;
  const L = (p: { l: string; k: "vendor" | "origin" | "state" }) => <label>{p.l}<input list={`pl-${p.k}`} value={x()[p.k] ?? ""} onChange={e => put({ [p.k]: e.currentTarget.value } as Partial<PItem>)} />
    <datalist id={`pl-${p.k}`}><For each={listOpts(p.k)}>{o => <option value={o.value}>{o.label}</option>}</For></datalist></label>;
  const TaskSel = (p: { l: string; v: number; set: (v: number) => void }) => <label>{p.l}<select value={String(p.v)} onChange={e => p.set(+e.currentTarget.value)}><option value="0">—</option><For each={tasks()}>{t => <option value={String(t.uid)} selected={t.uid === p.v}>{t.id} · {t.name}</option>}</For></select></label>;
  const setDate = (k: "planned" | "actual", i: number, v: string) => put({ [k]: x()[k].map((d, j) => (j === i ? v || null : d)) } as Partial<PItem>);
  return (
    <Dialog title={`Order ${x().tag || x().name}`} wide help="procurement" okText="Save" ok={() => save({ items: preg().items.map(y => (y.id === it.id ? x() : y)) })}>
      <div class="dform">
        <T l="Item no." k="tag" /><T l="Order / equipment" k="name" /><L l="Sub-vendor" k="vendor" /><L l="Country of origin" k="origin" /><T l="PO number" k="po_no" /><L l="State" k="state" />
        <label>PO value<input type="number" min="0" value={x().value || ""} onChange={e => put({ value: +e.currentTarget.value || 0 })} /></label>
        <Show when={preg().weighting === "manual"}><label>Weight<input type="number" min="0" value={x().manual_w} onChange={e => put({ manual_w: +e.currentTarget.value || 0 })} /></label></Show>
        <TaskSel l="Task for its manufacture & delivery" v={x().task} set={task => put({ task })} />
        <TaskSel l="Task that needs it" v={x().need} set={need => put({ need })} />
        <label>…or required on<input type="date" value={x().required ?? ""} onChange={e => put({ required: e.currentTarget.value || null })} /></label>
        <label>Manufacturing %<input type="number" min="0" max="100" value={x().mfg_pct} onChange={e => put({ mfg_pct: +e.currentTarget.value || 0 })} /></label>
        <label>Vendor's promised delivery<input type="date" value={x().promised ?? ""} onChange={e => put({ promised: e.currentTarget.value || null })} /></label>
        <T l="Remarks" k="remarks" />
        <For each={(preg().columns ?? []).filter(c => c.kind !== "formula")}>{c => <label>{c.name}<input type={c.kind === "date" ? "date" : c.kind === "number" ? "number" : "text"} list={c.kind === "list" ? `pl-own-${c.name}` : undefined} value={x().own?.[c.name] ?? ""} onChange={e => put({ own: { ...(x().own ?? {}), [c.name]: e.currentTarget.value } })} />
          <Show when={c.kind === "list"}><datalist id={`pl-own-${c.name}`}><For each={listOpts(c.name)}>{o => <option value={o.value}>{o.label}</option>}</For></datalist></Show></label>}</For>
      </div>
      <label class="chk" style={{ "margin-top": "8px" }}><input type="checkbox" checked={x().long_lead} onChange={e => put({ long_lead: e.currentTarget.checked })} /> Long-lead item</label>
      <table class="tbl" style={{ "margin-top": "8px" }}><thead><tr><th>Milestone</th><th>Planned</th><th>Actual</th></tr></thead>
        <tbody><For each={MS}>{(m, i) => <tr><td>{m}</td><td><input type="date" value={x().planned[i()] ?? ""} onChange={e => setDate("planned", i(), e.currentTarget.value)} /></td><td><input type="date" value={x().actual[i()] ?? ""} onChange={e => setDate("actual", i(), e.currentTarget.value)} /></td></tr>}</For></tbody></table>
      <p class="muted">Empty planned dates come from the task for its manufacture and delivery (start and finish); the inspection release from the delivery less the days in Settings.</p>
      <Show when={preg().link_docs}>
        <h3>Vendor documents it waits for</h3>
        <Show when={docsOn() && docReg().docs.length} fallback={<p class="muted">The Document Register is off or empty in this plan.</p>}>
          <input placeholder="Find a document…" value={dq()} onInput={e => setDq(e.currentTarget.value)} style={{ width: "240px" }} />
          <div class="pdocs"><For each={docReg().docs.filter(d => !dq() || `${d.number} ${d.title} ${d.vendor}`.toLowerCase().includes(dq().toLowerCase()))}>{d =>
            <label class="chk"><input type="checkbox" checked={x().docs.includes(d.id)} onChange={e => put({ docs: e.currentTarget.checked ? [...x().docs, d.id] : x().docs.filter(y => y !== d.id) })} /> <span class="mono">{d.number}</span> {d.title}</label>}</For></div>
        </Show>
      </Show>
    </Dialog>
  );
});

// ---------- A milestone, or an expediting note, for many orders at once ----------
const recordMany = (items: PItem[]) => openDialog(() => {
  const first = Math.min(4, ...items.map(nextMs));
  const [k, setK] = createSignal(first), [date, setDate] = createSignal(today());
  const ok = () => {
    const ids = new Set(items.map(x => x.id));
    return save({ items: preg().items.map(it => (ids.has(it.id) ? { ...it, actual: it.actual.map((d, i) => (i === k() ? date() || null : d)) as PItem["actual"], mfg_pct: k() >= 3 ? 100 : it.mfg_pct } : it)) });
  };
  return (
    <Dialog title="Record a milestone" okText={`Record for ${items.length} order${items.length > 1 ? "s" : ""}`} ok={ok}>
      <div class="prow"><label>Milestone<select value={k()} onChange={e => setK(+e.currentTarget.value)}><For each={MS}>{(m, i) => <option value={i()} selected={i() === k()}>{m}</option>}</For></select></label>
        <label>Done on<input type="date" value={date()} onInput={e => setDate(e.currentTarget.value)} /></label></div>
      <p class="muted">{items.map(x => x.tag || x.name).join(", ")}. The inspection release or delivery also makes manufacturing 100 %.</p>
    </Dialog>
  );
});
const noteMany = (items: PItem[], after?: () => void) => openDialog(() => {
  const [date, setDate] = createSignal(today()), [note, setNote] = createSignal(""), [by, setBy] = createSignal("");
  const ok = async () => {
    if (!note().trim()) { toast("Write the note."); return false; }
    const ids = new Set(items.map(x => x.id));
    const done = await save({ items: preg().items.map(it => (ids.has(it.id) ? { ...it, log: [...it.log, { date: date() || null, note: note().trim(), by: by().trim() }] } : it)) });
    if (done) after?.();
    return done;
  };
  return (
    <Dialog title="Expediting note" okText={`Add to ${items.length} order${items.length > 1 ? "s" : ""}`} ok={ok}>
      <div class="prow"><label>Date<input type="date" value={date()} onInput={e => setDate(e.currentTarget.value)} /></label><label>By<input value={by()} onInput={e => setBy(e.currentTarget.value)} /></label></div>
      <label style={{ display: "flex", "flex-direction": "column", gap: "2px", "margin-top": "6px" }}>What the vendor said, what was agreed<textarea rows="3" value={note()} onInput={e => setNote(e.currentTarget.value)} /></label>
      <p class="muted">{items.map(x => x.tag || x.name).join(", ")}</p>
    </Dialog>
  );
});

// ---------- The order card ----------
const [cardId, setCardId] = createSignal<number | null>(null);
const Card = (p: { o: ProcOut }) => {
  const it = () => preg().items.find(x => x.id === cardId());
  const row = () => p.o.items.find(x => x.id === cardId());
  return <Show when={it() && row()}>
    <Portal><div class="dcard">
      <div class="inline"><span class="num">{it()!.tag || "(no item no.)"}</span><span class="grow" /><button class="x" title="Close" onClick={() => setCardId(null)}>×</button></div>
      <h2>{it()!.name || "(no name)"}</h2>
      <div class="chips"><For each={[it()!.vendor, it()!.origin, it()!.po_no && `PO ${it()!.po_no}`, it()!.value ? `${it()!.value.toLocaleString()}` : "", it()!.long_lead ? "Long lead" : "", it()!.state || ""].filter(Boolean)}>{c => <span class="chip">{c}</span>}</For></div>
      <div><span class={`st ${SCOL[row()!.skey] ?? ""}`}>{row()!.status}</span> <span class="muted">· earned {row()!.earned.toFixed(0)} %{row()!.next ? ` · next: ${row()!.next}${row()!.next_date ? ` (planned ${dt(row()!.next_date)})` : ""}` : ""}</span></div>
      <div class="dsteps2"><For each={SHORT}>{(s, i) => <div classList={{ d: !!row()!.actual[i()], c: i() === nextMs(it()!) }} title={`${MS[i()]}: planned ${dt(row()!.planned[i()]) || "—"}${row()!.actual[i()] ? `, done ${dt(row()!.actual[i()])}` : ""}`}>{s}</div>}</For></div>
      <table class="tbl"><thead><tr><th>Milestone</th><th>Planned</th><th>Actual</th></tr></thead><tbody>
        <For each={MS}>{(m, i) => <tr><td>{m}</td><td>{dt(row()!.planned[i()])}</td><td>{dt(row()!.actual[i()])}</td></tr>}</For></tbody></table>
      <div class="muted">Promised {dt(it()!.promised) || "—"} · forecast {dt(row()!.forecast) || "—"}{row()!.var ? ` (${row()!.var! > 0 ? "+" : ""}${row()!.var} d)` : ""} · required {dt(row()!.required) || "—"}{row()!.need_name ? ` for ${row()!.need_name}` : ""} · float <span class={`st ${row()!.rag === "red" ? "bad" : row()!.rag === "amber" ? "warn" : row()!.rag === "green" ? "good" : ""}`}>{row()!.float ?? "—"}</span>
        {row()!.docs ? ` · documents ${row()!.docs_ok}/${row()!.docs} accepted${row()!.docs_late ? " (holding up manufacturing)" : ""}` : ""}</div>
      <b class="muted" style={{ "font-size": "10px", "letter-spacing": ".08em" }}>EXPEDITING</b>
      <div class="dtl"><For each={[...it()!.log].sort((a, b) => (b.date ?? "").localeCompare(a.date ?? ""))} fallback={<div class="muted">No notes yet.</div>}>{n => <div class="ev done"><b>{dt(n.date)}</b>{n.by ? ` · ${n.by}` : ""}<small>{n.note}</small></div>}</For></div>
      <div class="acts"><button class="btn pri" onClick={() => recordMany([it()!])}>Record milestone</button><button class="btn" onClick={() => noteMany([it()!])}>Note</button><button class="btn" onClick={() => editItem(it()!)}>Edit details</button>
        <span class="grow" /><button class="btn danger" onClick={async () => { const x = it()!; if (await askYes("Delete the order", `Delete ${x.tag || x.name} and its ${x.log.length} notes?`, "Delete")) { setCardId(null); save({ items: preg().items.filter(y => y.id !== x.id) }); } }}>Delete</button></div>
    </div></Portal>
  </Show>;
};

// ---------- Import ----------
const FIELDS: [string, string, RegExp][] = [["tag", "Item no.", /^item$|item no|^no\.?$|s\.?no|^tag/i], ["name", "Order / equipment", /order|equipment|description|item name/i], ["vendor", "Sub-vendor", /vendor|supplier/i], ["origin", "Country of origin", /origin|country/i],
  ["po_no", "PO number", /po no|order no/i], ["value", "PO value", /value|price|amount/i], ["state", "State (Active, Hold, Cancelled…)", /^state$|status of order|order state/i],
  ...MS.map((m, i) => [`p${i}`, `Planned: ${m}`, [/plan.*rfq|^rfq issue$/i, /plan.*po\b|po placement/i, /plan.*(mfg|manuf)/i, /plan.*(irn|ready)|ready to ship/i, /plan.*deliver|^delivery to works$/i][i]] as [string, string, RegExp]),
  ...MS.map((m, i) => [`a${i}`, `Actual: ${m}`, new RegExp(`(actual.*)?^(${["rfq issued", "po placed", "mfg start|manufactur.*start", "irn date|irn", "delivered"][i]})`, "i")] as [string, string, RegExp]),
  ["mfg_pct", "Manufacturing %", /mfg %|manufactur.*%/i], ["promised", "Promised / forecast delivery", /forecast deliv|promised/i], ["remarks", "Remarks / expediting notes", /remark|note/i]];
/** A workbook's settings sheet: the milestone weights in use, the weighting, the inspection release days. */
const readSettings = (rows: string[][]) => {
  const out: Partial<ProcRegister> = {};
  const find = (re: RegExp) => { for (let i = 0; i < rows.length; i++) for (let j = 0; j < rows[i].length; j++) if (re.test(rows[i][j] ?? "")) return [i, j] as const; return null; };
  const beside = (re: RegExp) => { const at = find(re); return at ? rows[at[0]].slice(at[1] + 1).find(x => x?.trim()) ?? null : null; };
  const wt = beside(/weighting|weigh.*by/i); if (wt) out.weighting = /equal/i.test(wt) ? "equal" : /value|price/i.test(wt) ? "value" : "manual";
  const irn = beside(/irn before|inspection release.*day|irn.*days/i); if (irn && +irn >= 0) out.irn_days = Math.round(+irn);
  const amber = beside(/amber|warn/i); if (amber && +amber >= 0) out.warn_days = Math.round(+amber);
  // The weights: the five milestone rows under a "Milestone" heading, in the column marked active (else the option in use).
  const head = find(/^milestone$/i);
  if (head) {
    const hr = rows[head[0]], use = beside(/option in use|weight.*in use/i) ?? "";
    let col = hr.findIndex(h => /^active$/i.test(h.trim()));
    if (col < 0) col = hr.findIndex(h => use && h.toLowerCase().startsWith(use.toLowerCase()));
    if (col < 0) col = head[1] + 1;
    const res = [/rfq/i, /po\b|order placed|purchase order/i, /manuf|mfg/i, /irn|inspection/i, /deliver/i];
    const ws = res.map(re => { const r = rows.slice(head[0] + 1, head[0] + 10).find(x => re.test(x[head[1]] ?? "")); const v = r ? parseFloat(r[col]) : NaN; return isNaN(v) ? null : v <= 1 ? v * 100 : v; });
    if (ws.every(v => v !== null)) { const w = ws.map(v => Math.round(v! * 1000) / 1000); const p = PRESETS.find(p => p[2].every((x, i) => Math.abs(x - w[i]) < 0.01)); out.preset = p ? p[0] : "own"; out.weights = w; }
  }
  return out;
};
export const procImport = async () => {
  const first = await call<{ path: string; names: string[]; rows: string[][] } | null>("docs_read", {});
  if (!first) return;
  openDialog(() => {
    const [data, setData] = createSignal(first), [kind, setKind] = createSignal<"orders" | "settings">(/setting/i.test(first.names[0] ?? "") ? "settings" : "orders");
    const guessHead = (rows: string[][]) => { const sc = rows.slice(0, 30).map(r => r.filter(c => c.length < 60 && FIELDS.some(([, , re]) => re.test(c))).length); return Math.max(0, sc.indexOf(Math.max(...sc))); };
    const [head, setHead] = createSignal(guessHead(first.rows));
    const headers = () => data().rows[head()] ?? [];
    // Planned and actual columns often share a heading (planned first): a second "Mfg Start" makes the first one planned.
    const guess = () => headers().reduce<string[]>((m, h) => {
      const f = FIELDS.find(([k, , re]) => re.test(h) && !m.includes(k));
      const twice = !f && FIELDS.find(([k, , re]) => k[0] === "a" && re.test(h) && m.includes(k) && !m.includes("p" + k.slice(1)));
      return twice ? [...m.map(x => (x === twice[0] ? "p" + x.slice(1) : x)), twice[0]] : [...m, f ? f[0] : ""];
    }, []);
    const [map, setMap] = createSignal<string[]>(guess());
    const body = () => data().rows.slice(head() + 1).filter(r => r.some(c => c.trim()));
    const sheet = async (name: string) => { const d = await call<typeof first>("docs_read", { path: data().path, sheet: name }); if (d) { setData(d); setHead(guessHead(d.rows)); setMap(guess()); setKind(/setting/i.test(name) ? "settings" : "orders"); } };
    const col = (r: string[], f: string) => { const j = map().indexOf(f); return j >= 0 ? (r[j] ?? "").trim() : ""; };
    const found = createMemo(() => (kind() === "settings" ? readSettings(data().rows) : {}));
    const run = () => {
      if (kind() === "settings") {
        const s = found(); if (!Object.keys(s).length) { toast("No weights, weighting or inspection days were found on this sheet."); return false; }
        save(s); toast(`Settings brought in: ${[s.weights && `milestone weights (${s.weights.join(" / ")})`, s.weighting && `weighting by ${s.weighting}`, s.irn_days !== undefined && `inspection release ${s.irn_days} days before delivery`].filter(Boolean).join(", ")}.`, true); return;
      }
      if (!map().includes("name") && !map().includes("tag")) { toast("Choose the column with the order or the item number."); return false; }
      const items = [...preg().items]; let id = next(items), added = 0, changed = 0;
      for (const r of body()) {
        const tag = col(r, "tag"), name = col(r, "name"); if ((!tag && !name) || /^total/i.test(name)) continue;
        const at = items.findIndex(x => (tag && x.tag === tag) || (!tag && x.name === name));
        const o = at >= 0 ? items[at] : blank(id);
        const dates = (k: "p" | "a", old: (string | null)[]) => old.map((d, i) => asDate(col(r, `${k}${i}`)) ?? d) as PItem["planned"];
        const pct = col(r, "mfg_pct").replace("%", "");
        const n: PItem = { ...o, tag: tag || o.tag, name: name || o.name, vendor: col(r, "vendor") || o.vendor, origin: col(r, "origin") || o.origin, po_no: col(r, "po_no") || o.po_no, value: +col(r, "value").replace(/[^\d.]/g, "") || o.value,
          planned: dates("p", o.planned), actual: dates("a", o.actual), mfg_pct: pct ? Math.min(100, (+pct <= 1 && pct.includes(".") ? +pct * 100 : +pct) || 0) : o.mfg_pct, promised: asDate(col(r, "promised")) ?? o.promised, remarks: col(r, "remarks") || o.remarks,
          state: map().includes("state") ? (/^active$/i.test(col(r, "state")) ? "" : col(r, "state")) : o.state };
        if (at >= 0) { items[at] = n; changed++; } else { items.push(n); id++; added++; }
      }
      save({ items }); toast(`${added} orders added, ${changed} updated. New sub-vendors and countries joined their lists.`, true);
    };
    return (
      <Dialog title="Import into the Procurement Register" wide help="procurement" okText="Import" ok={run}>
        <div class="prow"><label>Bring in<Seg value={kind()} set={v => setKind(v as "orders")} options={[["orders", "Orders (the register)"], ["settings", "Settings (weights, weighting, inspection days)"]]} /></label></div>
        <div class="prow"><Show when={data().names.length > 1}><label>Sheet<select onChange={e => sheet(e.currentTarget.value)}><For each={data().names}>{n => <option>{n}</option>}</For></select></label></Show>
          <Show when={kind() === "orders"}><label>Headings are in row<input type="number" min="1" value={head() + 1} onChange={e => { setHead(Math.max(0, +e.currentTarget.value - 1)); setMap(guess()); }} /></label></Show></div>
        <Show when={kind() === "orders"} fallback={<div class="panel ctr"><p class="muted">Read from this sheet (choose the workbook's settings sheet above):</p><ul>
          <li>Milestone weights: {found().weights ? `${found().weights!.join(" / ")} %${found().preset !== "own" ? ` (${PRESETS.find(p => p[0] === found().preset)?.[1]})` : ""}` : <span class="muted">none found (a “Milestone” heading with RFQ, PO, manufacturing, IRN and delivery rows)</span>}</li>
          <li>Orders weigh by: {found().weighting ?? <span class="muted">—</span>} · inspection release {found().irn_days ?? "—"} days before delivery</li></ul></div>}>
          <p class="muted">{body().length} rows under the headings. Orders with the same item number are updated. Columns left on "—" are not read.</p>
          <div class="dmap"><table class="tbl"><thead><tr><th>Column</th><th>Holds</th><th>First rows</th></tr></thead>
            <tbody><For each={headers()}>{(h, j) => <tr><td><b>{h || `(column ${j() + 1})`}</b></td>
              <td><select value={map()[j()] ?? ""} onChange={e => setMap(headers().map((_, k) => (k === j() ? e.currentTarget.value : (map()[k] === e.currentTarget.value && e.currentTarget.value ? "" : map()[k] ?? ""))))}><option value="">—</option><For each={FIELDS}>{f => <option value={f[0]} selected={f[0] === map()[j()]}>{f[1]}</option>}</For></select></td>
              <td class="muted">{body().slice(0, 3).map(r => r[j()] ?? "").filter(Boolean).join(" · ").slice(0, 80)}</td></tr>}</For></tbody></table></div>
        </Show>
      </Dialog>
    );
  });
};

// ---------- The view ----------
type Note = { it: PItem; k: number; date: string | null; note: string; by: string; id: number };
export const ProcurementView = () => {
  const [data] = createResource(() => [tick(), out()?.tab] as const, () => call<{ tab: number; name: string; on: boolean; out: ProcOut }[]>("procurement"));
  const me = () => data()?.find(d => d.tab === out()?.tab)?.out;
  const [tab, setTab] = createSignal("register"), [q, setQ] = createSignal(""), [per, setPer] = createSignal<"weekly" | "monthly">("monthly");
  const [sel, setSel] = createSignal<PItem[]>([]), [selN, setSelN] = createSignal<Note[]>([]);
  const rowOf = (id: number) => me()?.items.find(x => x.id === id);
  const items = createMemo(() => { const t = q().toLowerCase(); return preg().items.filter(it => !t || `${it.tag} ${it.name} ${it.vendor} ${it.origin} ${it.po_no} ${it.remarks}`.toLowerCase().includes(t)); });
  // ---- register columns ----
  const hidden = () => (P().ui?.procHidden as string[] | undefined) ?? ["long_lead", "p0", "a0", "p2", "a2", "p3", "a3", "task", "need", "weight", "docs", "value"];
  const setHidden = (h: string[]) => act({ Ui: { key: "procHidden", value: h } });
  const calc = (key: string, name: string, width: number, get: (r: ItemOut) => string, more: Partial<Col<PItem>> = {}): Col<PItem> => ({ key, name, width, kind: "calc", get: it => { const r = rowOf(it.id); return r ? get(r) : ""; }, ...more });
  const list = (key: string, name: string, width: number): Col<PItem> => ({ key, name, width, kind: "list", get: it => fieldOf(it, key), options: () => listOpts(key), editList: () => procSettings("lists") });
  const taskCol = (key: "task" | "need", name: string): Col<PItem> => ({ key, name, width: 170, kind: "list", strict: true, get: it => String(tasks().find(t => t.uid === it[key])?.id ?? ""), show: it => { const t = tasks().find(t => t.uid === it[key]); return t ? `${t.id} ${t.name}` : ""; }, options: () => tasks().map(t => ({ value: String(t.id), label: t.name })) });
  const ms = (i: number): Col<PItem>[] => [
    { key: `p${i}`, name: `${SHORT[i]} planned`, width: 86, kind: "date", get: it => it.planned[i] ?? "", show: it => (it.planned[i] ? dt(it.planned[i]) : <span class="muted" title="From the task, or the delivery less the inspection days">{dt(rowOf(it.id)?.planned[i])}</span>), tip: "Typed, or (empty) worked out from the task" },
    { key: `a${i}`, name: MS[i] === "Manufacturing start" ? "Mfg started" : MS[i] === "Inspection release (IRN)" ? "IRN" : MS[i], width: 86, kind: "date", get: it => it.actual[i] ?? "", cls: it => (!it.actual[i] && (rowOf(it.id)?.days_late ?? 0) > 0 && nextMs(it) === i ? "late" : "") },
  ];
  const allCols = createMemo((): Col<PItem>[] => [
    { key: "tag", name: "Item", width: 64, kind: "text", get: it => it.tag },
    { key: "name", name: "Order / equipment", width: 230, kind: "text", get: it => it.name },
    list("vendor", "Sub-vendor", 140), list("origin", "Origin", 90),
    { key: "po_no", name: "PO No.", width: 110, kind: "text", get: it => it.po_no, cls: () => "mono" },
    { key: "value", name: "PO value", width: 90, kind: "number", get: it => (it.value ? String(it.value) : "") },
    { key: "state", name: "State", width: 76, kind: "list", get: it => it.state || "Active", options: () => listOpts("state"), editList: () => procSettings("lists") },
    { key: "long_lead", name: "Long lead", width: 56, kind: "check", get: it => (it.long_lead ? "Yes" : "") },
    ...ms(0), ...ms(1), ...ms(2),
    { key: "mfg", name: "Mfg %", width: 54, kind: "number", get: it => String(it.mfg_pct) },
    ...ms(3), ...ms(4),
    { key: "promised", name: "Promised", width: 86, kind: "date", get: it => it.promised ?? "" },
    calc("forecast", "Forecast", 82, r => r.forecast ?? "", { date: true }),
    calc("var", "Var.", 50, r => (r.var ? `${r.var > 0 ? "+" : ""}${r.var}` : ""), { cls: it => ((rowOf(it.id)?.var ?? 0) > 0 ? "late num" : "num"), tip: "Forecast against planned delivery (days)" }),
    { key: "required", name: "Required", width: 84, kind: "date", get: it => it.required ?? "", show: it => (it.required ? dt(it.required) : <span class="muted" title={rowOf(it.id)?.need_name}>{dt(rowOf(it.id)?.required)}</span>), tip: "Typed, or (empty) the start of the task that needs it" },
    calc("float", "Float", 54, r => (r.float === null ? "" : String(r.float)), { show: it => { const r = rowOf(it.id); return r && r.float !== null ? <span class={`st ${r.rag === "red" ? "bad" : r.rag === "amber" ? "warn" : r.rag === "green" ? "good" : ""}`}>{r.float}</span> : ""; }, cls: () => "num" }),
    calc("status", "Status", 140, r => r.status, { show: it => { const r = rowOf(it.id); return r ? <span class={`st ${SCOL[r.skey] ?? ""}`}>{r.status}</span> : ""; } }),
    calc("next", "Next", 160, r => (r.next ? `${r.next}${r.days_late > 0 ? ` (${r.days_late} d late)` : ""}` : ""), { cls: it => ((rowOf(it.id)?.days_late ?? 0) > 0 ? "late" : "") }),
    calc("earned", "Earned", 60, r => `${r.earned.toFixed(0)} %`, { cls: () => "num" }),
    taskCol("task", "Manufacture task"), taskCol("need", "Needed by task"),
    { key: "weight", name: "Weight", width: 58, kind: "number", get: it => String(it.manual_w) },
    calc("docs", "Documents", 76, r => (r.docs ? `${r.docs_ok}/${r.docs}` : ""), { cls: it => (rowOf(it.id)?.docs_late ? "late" : ""), tip: "Linked vendor documents accepted / linked" }),
    { key: "remarks", name: "Remarks", width: 200, kind: "text", get: it => it.remarks },
    ...(preg().columns ?? []).map((c, i): Col<PItem> => c.kind === "formula" ? calc("own:" + c.name, c.name, 110, r => r.own?.[i] ?? "", { tip: c.formula })
      : { key: "own:" + c.name, name: c.name, width: 110, kind: (c.kind === "list" ? "list" : c.kind === "date" ? "date" : c.kind === "number" ? "number" : "text"), get: it => it.own?.[c.name] ?? "", ...(c.kind === "list" ? { options: () => listOpts(c.name), editList: () => procSettings("lists") } : {}) }),
  ]);
  const cols = () => allCols().filter(c => !hidden().includes(c.key));
  const apply = (it: PItem, key: string, v: string): PItem => {
    const m = key.match(/^([pa])(\d)$/);
    if (m) { const k = m[1] === "p" ? "planned" : "actual", i = +m[2]; return { ...it, [k]: it[k].map((d, j) => (j === i ? v || null : d)), ...(k === "actual" && i >= 3 && v ? { mfg_pct: 100 } : {}) } as PItem; }
    switch (key) {
      case "tag": case "name": case "vendor": case "origin": case "po_no": case "remarks": return { ...it, [key]: v };
      case "state": return { ...it, state: /^active$/i.test(v) ? "" : v };
      case "value": return { ...it, value: Math.max(0, +v || 0) };
      case "weight": return { ...it, manual_w: Math.max(0, +v || 0) };
      case "mfg": return { ...it, mfg_pct: Math.min(100, Math.max(0, +v || 0)) };
      case "long_lead": return { ...it, long_lead: !!v };
      case "promised": case "required": return { ...it, [key]: v || null };
      case "task": case "need": return { ...it, [key]: tasks().find(t => String(t.id) === v)?.uid ?? 0 };
    }
    if (key.startsWith("own:")) return { ...it, own: { ...(it.own ?? {}), [key.slice(4)]: v } };
    return it;
  };
  const setItems = (e: Edit<PItem>[]) => { const m = new Map(preg().items.map(x => [x.id, x])); for (const x of e) { const it = m.get(x.row.id); if (it) m.set(it.id, apply(it, x.key, x.value)); } save({ items: preg().items.map(x => m.get(x.id)!) }); };
  const addItems = (vals: Record<string, string>[]) => { let id = next(preg().items); save({ items: [...preg().items, ...vals.map(v => Object.entries(v).reduce((it, [k, x]) => apply(it, k, x), blank(id++)))] }); };
  const removeItems = async (rows: PItem[]) => {
    const notes = rows.reduce((a, x) => a + x.log.length, 0);
    if ((rows.length > 1 || notes) && !(await askYes("Delete orders", `Delete ${rows.length} order${rows.length > 1 ? "s" : ""}${notes ? ` and their ${notes} expediting notes` : ""}? Ctrl+Z brings them back.`, "Delete"))) return;
    const ids = new Set(rows.map(x => x.id)); save({ items: preg().items.filter(x => !ids.has(x.id)) });
  };
  const menu = (rows: PItem[]): Item[] => [
    { label: rows.length > 1 ? `Record a milestone for ${rows.length} orders…` : "Record a milestone…", icon: "check", run: () => recordMany(rows), off: !rows.length },
    { label: rows.length > 1 ? `Expediting note for ${rows.length} orders…` : "Expediting note…", icon: "note", run: () => noteMany(rows, () => setTab("log")), off: !rows.length },
    { label: "Open the order card", icon: "info", run: () => setCardId(rows[0].id), off: rows.length !== 1 }, { label: "Edit details…", icon: "edit", run: () => editItem(rows[0]), off: rows.length !== 1 },
  ];
  // ---- the expediting log ----
  const notes = createMemo((): Note[] => { const t = q().toLowerCase(); return preg().items.flatMap(it => it.log.map((x, k) => ({ it, k, date: x.date, note: x.note, by: x.by, id: it.id * 10000 + k }))).filter(n => !t || `${n.it.tag} ${n.it.name} ${n.note} ${n.by}`.toLowerCase().includes(t)).sort((a, b) => (a.date ?? "9999").localeCompare(b.date ?? "9999") || a.id - b.id); });
  const noteCols = (): Col<Note>[] => [
    { key: "order", name: "Item", width: 90, kind: "list", strict: true, get: n => n.it.tag || n.it.name, options: () => preg().items.map(x => ({ value: x.tag || x.name, label: `${x.name}${x.vendor ? ` · ${x.vendor}` : ""}` })) },
    { key: "name", name: "Order", width: 230, kind: "calc", get: n => n.it.name },
    { key: "vendor", name: "Sub-vendor", width: 130, kind: "calc", get: n => n.it.vendor },
    { key: "date", name: "Date", width: 90, kind: "date", get: n => n.date ?? "" },
    { key: "note", name: "Note", width: 460, kind: "text", get: n => n.note },
    { key: "by", name: "By", width: 100, kind: "text", get: n => n.by },
    { key: "status", name: "Order status", width: 130, kind: "calc", get: n => rowOf(n.it.id)?.status ?? "" },
  ];
  const byTag = (v: string) => preg().items.find(x => (x.tag || x.name).toLowerCase() === v.trim().toLowerCase());
  const setNotes = (e: Edit<Note>[]) => {
    let items = preg().items.map(x => ({ ...x, log: [...x.log] }));
    for (const x of e) {
      const it = items.find(y => y.id === x.row.it.id); if (!it) continue;
      const n = { ...it.log[x.row.k] };
      if (x.key === "order") { const to = byTag(x.value); if (!to) { toast(`“${x.value}” is not an order in the register.`); continue; } it.log.splice(x.row.k, 1); items.find(y => y.id === to.id)!.log.push(n); continue; }
      it.log[x.row.k] = { ...n, [x.key]: x.key === "date" ? x.value || null : x.value };
    }
    save({ items });
  };
  const addNotes = (vals: Record<string, string>[]) => {
    const items = preg().items.map(x => ({ ...x, log: [...x.log] })); let bad = 0;
    for (const v of vals) { const it = v.order ? items.find(y => (y.tag || y.name).toLowerCase() === v.order.toLowerCase()) : undefined; if (!it) { bad++; continue; } it.log.push({ date: v.date || today(), note: v.note ?? "", by: v.by ?? "" }); }
    if (bad) toast(bad === vals.length ? "Start a new row with the order's item number: its name and sub-vendor come from the register." : `${bad} rows name orders not in the register; they were left out.`);
    if (bad < vals.length) save({ items });
  };
  const removeNotes = (rows: Note[]) => { const gone = new Set(rows.map(n => n.id)); save({ items: preg().items.map(it => ({ ...it, log: it.log.filter((_, k) => !gone.has(it.id * 10000 + k)) })) }); };
  const by = (key: (it: PItem) => string) => { const m = new Map<string, { n: number; red: number; done: number; e: number; w: number }>(); for (const it of preg().items) { const r = rowOf(it.id); if (!r || r.skey === "off") continue; const k = key(it) || "(none)", g = m.get(k) ?? { n: 0, red: 0, done: 0, e: 0, w: 0 }; g.n++; if (r.rag === "red") g.red++; if (r.actual[4]) g.done++; g.e += r.weight * r.earned; g.w += r.weight; m.set(k, g); } return [...m].sort((a, b) => b[1].n - a[1].n); };
  const legend = <span class="rglegend"><span><i />you type</span><span><i class="calc" />the app works it out</span><span>grey dates: from the task</span></span>;
  const columnsMenu = (e: MouseEvent) => openMenu(e, [...allCols().map(c => ({ label: c.name, on: !hidden().includes(c.key), run: () => setHidden(hidden().includes(c.key) ? hidden().filter(x => x !== c.key) : [...hidden(), c.key]) })), "-" as const, { label: "Your own columns…", run: () => procSettings("columns") }]);
  const n = (f: (x: ItemOut) => boolean) => me()?.items.filter(x => x.skey !== "off" && f(x)).length ?? 0;
  return (
    <Page title="Procurement Register" help="procurement" tools={<>
      <button class="btn" onClick={() => procSettings()}>Settings…</button><button class="btn" onClick={procImport}>Import…</button>
      <button class="btn" onClick={async () => { const f = await call<string | null>("proc_export"); if (f) toast(`Saved ${f}`, true); }} title="A workbook (.xlsx) or an OpenDocument spreadsheet (.ods): choose the type in the Save window">Export…</button>
    </>}>
      <Show when={me()}>{o => <>
        <div class="kpis">
          <Kpi v={String(n(() => true))} l={`orders · ${preg().items.filter(x => x.long_lead).length} long lead`} />
          <Kpi v={`${n(x => !!x.actual[1])} / ${n(() => true)}`} l="purchase orders placed" />
          <Kpi v={`${n(x => !!x.actual[4])} / ${n(() => true)}`} l="delivered" />
          <Kpi v={`${n(x => x.rag === "red")} · ${n(x => x.rag === "amber")}`} l="late for the plan · little float" bad={n(x => x.rag === "red") > 0} />
          <Kpi v={String(n(x => x.days_late > 0))} l="milestones overdue" bad={n(x => x.days_late > 0) > 0} />
          <Kpi v={`${o().earned.toFixed(1)} %`} l={`earned · planned ${o().planned.toFixed(1)} %`} bad={o().earned < o().planned - 0.05} good={o().earned >= o().planned && o().planned > 0} />
          <Show when={preg().link_docs}><Kpi v={String(n(x => x.docs_late))} l="held up by vendor documents" bad={n(x => x.docs_late) > 0} /></Show>
        </div>
        <Show when={!o().weights_ok}><p class="dwarn">The milestone weights do not add up to 100 %: Settings.</p></Show>
        <div class="dtabs"><Seg value={tab()} set={setTab} options={[["register", "Register"], ["log", "Expediting log"], ["progress", "Progress"]]} /></div>
        <Show when={tab() === "register"}>
          <div class="rgbar">
            <button class="btn pri" onClick={() => addItems([{ name: "New order" }])} title="Or type in the empty row at the foot of the register">+ Order</button>
            <button class="btn" disabled={!sel().length} onClick={() => recordMany(sel())}>Record milestone{sel().length > 1 ? ` (${sel().length})` : ""}</button>
            <button class="btn" disabled={!sel().length} onClick={() => noteMany(sel(), () => setTab("log"))}>Note{sel().length > 1 ? ` (${sel().length})` : ""}</button>
            <button class="btn" disabled={sel().length !== 1} onClick={() => setCardId(sel()[0].id)}>Card</button>
            <button class="btn" disabled={sel().length !== 1} onClick={() => editItem(sel()[0])}>Details…</button>
            <span class="sep" /><button class="btn danger" disabled={!sel().length} onClick={() => removeItems(sel())}>Delete{sel().length > 1 ? ` (${sel().length})` : ""}</button>
            <span class="sep" /><input class="q" placeholder="Search item, order, vendor, PO…" value={q()} onInput={e => setQ(e.currentTarget.value)} />
            <button class="btn" onClick={columnsMenu}>Columns ▾</button>{legend}
          </div>
          <RegGrid id="orders" rows={items()} cols={cols()} rowId={x => x.id} set={setItems} add={addItems} remove={removeItems} open={x => setCardId(x.id)} menu={menu} selected={setSel}
            newHint="Type an item number or an order here to add one; paste rows from a workbook" rowCls={x => (rowOf(x.id)?.skey === "off" ? "off" : "")} />
          <Show when={!preg().items.length}><p class="empty">No orders yet. Type in the first row, paste rows from your workbook, or <b>Import…</b> a procurement status register. Link each to the task for its manufacture and delivery and to the task that needs it (the Manufacture task and Needed by task columns, or Details…).</p></Show>
        </Show>
        <Show when={tab() === "log"}>
          <div class="rgbar">
            <button class="btn pri" onClick={() => toast("Type the order's item number in the empty row at the foot of the log, then the note; or select orders in the Register and click Note.", true)}>+ Note</button>
            <button class="btn" disabled={selN().length !== 1} onClick={() => setCardId(selN()[0].it.id)}>Card</button>
            <span class="sep" /><button class="btn danger" disabled={!selN().length} onClick={() => removeNotes(selN())}>Delete{selN().length > 1 ? ` (${selN().length})` : ""}</button>
            <span class="sep" /><input class="q" placeholder="Search item, note, by…" value={q()} onInput={e => setQ(e.currentTarget.value)} />{legend}
          </div>
          <RegGrid id="notes" rows={notes()} cols={noteCols()} rowId={x => x.id} set={setNotes} add={addNotes} remove={removeNotes} open={x => setCardId(x.it.id)} selected={setSelN} newHint="Type or pick an order's item number to add a note" />
        </Show>
        <Show when={tab() === "progress"}>
          <div class="phead"><Seg value={per()} set={v => setPer(v as "monthly")} options={[["weekly", "Weekly"], ["monthly", "Monthly"]]} /></div>
          <Show when={o()[per()].length > 1} fallback={<p class="empty">Give the orders planned dates (or link them to tasks) to see the S-curve.</p>}>
            <ChartBox title="Procurement progress (weighted)" type="line" fmt="pct" max={100} labels={o()[per()].map(p => dt(p.end))} days={o()[per()].map(p => Math.round(Date.parse(p.end) / 864e5))}
              datasets={[line("Planned", o()[per()].map(p => p.planned), COL.grey, [5, 4]), line("Actual", o()[per()].map(p => p.actual), COL.blue)]} />
          </Show>
          <div class="cols2"><For each={[["By sub-vendor", (it: PItem) => it.vendor], ["By country of origin", (it: PItem) => it.origin]] as [string, (it: PItem) => string][]}>{([t, f]) =>
            <div class="panel ctr"><h2>{t}</h2><table class="tbl"><thead><tr><th>{t.replace("By ", "")}</th><th class="r">Orders</th><th class="r">Late</th><th class="r">Delivered</th><th class="r">Earned</th></tr></thead>
              <tbody><For each={by(f)}>{([k, g]) => <tr><td>{k}</td><td class="r">{g.n}</td><td class="r" classList={{ bad: g.red > 0 }}>{g.red}</td><td class="r">{g.done}</td><td class="r">{g.w ? (g.e / g.w).toFixed(1) : "—"} %</td></tr>}</For></tbody></table></div>}</For></div>
        </Show>
        <Card o={o()} />
      </>}</Show>
      <Show when={(data()?.length ?? 0) > 1}>
        <div class="panel ctr"><h2>The project's plans</h2>
          <table class="tbl"><thead><tr><th>Plan</th><th class="r">Orders</th><th class="r">PO placed</th><th class="r">Delivered</th><th class="r">Late</th><th class="r">Earned</th><th class="r">Planned</th><th /></tr></thead>
            <tbody><For each={data()}>{d => <tr><td><b>{d.name}</b>{d.tab === out()?.tab ? " (this plan)" : ""}{!d.on && <span class="muted"> · module off</span>}</td><td class="r">{d.out.items.length}</td><td class="r">{d.out.items.filter(x => x.actual[1]).length}</td><td class="r">{d.out.items.filter(x => x.actual[4]).length}</td>
              <td class="r">{d.out.items.filter(x => x.rag === "red").length}</td><td class="r">{d.out.earned.toFixed(1)} %</td><td class="r">{d.out.planned.toFixed(1)} %</td>
              <td>{d.tab !== out()?.tab && <button class="btn sm" onClick={() => showTab(d.tab).then(() => setView("procurement"))}>Open</button>}</td></tr>}</For></tbody></table>
        </div>
      </Show>
    </Page>
  );
};
