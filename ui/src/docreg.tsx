// The Document Register module: vendor documents and their transmittal log, kept as two spreadsheets
// (regrid.tsx), with the client's own lists, classes, steps, codes and status words. The engine works
// out every date, status and percentage (docreg.rs) and offers the register to sheet formulas as
// Documents[…] and Transmittals[…] (regtable.rs); this page shows them and edits the register.
import { For, Show, createMemo, createResource, createSignal } from "solid-js";
import { Portal } from "solid-js/web";
import { DocClass, DocRegister, DocRow, DocStep, ListItem, Options, RegList, RegOut, Submission, VDoc } from "./api";
import { act, call, grid, out, P, tick, setView, openDialog, closeDialog, toast, opts, setOpts, openMenu, Item, kept } from "./store";
import { Kpi } from "./reports";
import { ChartBox, COL, bar, line } from "./charts";
import { Dialog, Seg, askText, askYes } from "./ui";
import { showTab } from "./plans";
import { Col, Edit, Opt, RegGrid, dt, parseDate } from "./regrid";
import { Attention, ColumnsTab, LCol, LList, ListsTab, RegBook, WordsTab, editLists, settle } from "./regkit";
import * as D from "./dialogs";

const FLAGS = ["MRB", "VDB", "IOM", "PIM", "Native", "Eng. review", "QA review"];
/** The standard status words by key (the engine's DOC_STATUS); the client's own replace them in Settings. */
const WORDS: [string, string, string][] = [["not_sub", "Not submitted", "No issue sent yet"], ["not_sub_late", "Not submitted – overdue", "The first issue is past its planned date"], ["review", "Under review", "With the client, within the review period"],
  ["review_late", "Review overdue", "With the client, past the review period"], ["resubmit", "Resubmit", "Returned with a code that asks for it again"], ["accepted", "Accepted – next issue", "Returned with a code that accepts it; the next issue is due"],
  ["final", "Final issued", "The last step of its class is done"], ["off", "Not counted", "Its state (such as Deleted) leaves it out of the counts"]];
const SCOL: Record<string, string> = { final: "good", accepted: "blue", review: "warn", review_late: "bad", resubmit: "bad", not_sub_late: "bad", not_sub: "grey", off: "grey" };
const STATES: ListItem[] = ["Active", "Hold", "Deleted", "Superseded"].map(v => ({ value: v, label: "", off: v === "Deleted" || v === "Superseded" }));
const LISTS = (): RegList[] => [{ name: "Vendors", field: "vendor", items: [] }, { name: "Packages", field: "package", items: [] }, { name: "Disciplines", field: "disc", items: [] }, { name: "Document types", field: "kind", items: [] }, { name: "States", field: "state", items: STATES.map(x => ({ ...x })) }];
const DEF: DocRegister = {
  p_date: null, p_unit: "weeks", review_days: 10, turnaround_days: 10, review_cal: "", work_cal: "",
  weights: [[35, 35, 60], [15, 15, 20], [10, 10, 0], [10, 10, 0], [20, 20, 0], [10, 10, 20]],
  codes: [{ code: "Code 1", meaning: "Approved – proceed", accepted: true, resubmit: false }, { code: "Code 2", meaning: "Approved with comments – proceed with the next revision", accepted: true, resubmit: false },
    { code: "Code 3", meaning: "Revise and resubmit", accepted: false, resubmit: true }, { code: "Code 4", meaning: "Retained for information", accepted: true, resubmit: false },
    { code: "Code 5", meaning: "Rejected", accepted: false, resubmit: true }, { code: "No Comment", meaning: "No reply within the review period (deemed accepted)", accepted: true, resubmit: false }],
  purposes: [["ICR", "Issued for client review"], ["IFA", "Issued for approval"], ["IFI", "Issued for information"], ["IFD", "Issued for design"], ["IFP", "Issued for purchase"], ["IFH", "Issued for HAZOP"], ["IFC1", "Issued for construction"], ["IFC2", "Issued final"], ["RED-LINE", "Red-line mark-up"], ["AS-BUILT", "As built"]],
  flow: { first: "ICR", approval: "IFA", construction: "IFC1", final: "IFC2" }, docs: [], log: [], lists: LISTS(), columns: [], words: {},
};
/** The plan's register (the defaults when it has none yet). */
export const reg = (): DocRegister => ({ ...DEF, ...(P().documents ?? {}) });
const save = (r: Partial<DocRegister>) => act({ Documents: { r: { ...reg(), ...r } } });
/** The register's classes and their steps (older plans: the three classes made from the flow and the weights, as the engine does). */
export const classesOf = (r: DocRegister): DocClass[] => {
  if (r.classes?.length) return r.classes;
  const f = r.flow, w = (i: number, k: number) => r.weights[i]?.[k] ?? 0;
  const st = (name: string, purposes: string, returned: boolean, weight: number): DocStep => ({ name, purposes, returned, codes: "", weight, days: null });
  const full = (k: number): DocClass => ({ name: String(k + 1), steps: [st("First issue submitted", f.first, false, w(0, k)), st("First issue accepted", f.first, true, w(1, k)), st("Approval issue submitted", f.approval, false, w(2, k)), st("Approval issue accepted", f.approval, true, w(3, k)), st("Construction issue submitted", f.construction, false, w(4, k)), st("Final issue submitted", f.final, false, w(5, k))] });
  return [full(0), full(1), { name: "3", steps: [st("Construction issue submitted", f.construction, false, w(0, 2) + w(2, 2) + w(4, 2)), st("Construction issue accepted", f.construction, true, w(1, 2) + w(3, 2)), st("Final issue submitted", f.final, false, w(5, 2))] }];
};
const purposesOf = (s: DocStep) => s.purposes.split(/[,;/]/).map(x => x.trim()).filter(Boolean);
/** A document's class name. */
export const className = (d: VDoc) => classesOf(reg())[Math.max(1, d.class) - 1]?.name ?? String(d.class);
const blankDoc = (id: number): VDoc => ({ id, number: "", title: "", vendor: "", package: "", disc: "", kind: "", class: 1, key: false, flags: [], offset: null, first: null, weight: 1, progress: true, recurring: false, task: 0, remarks: "", state: "", own: {} });
const next = (l: { id: number }[]) => Math.max(0, ...l.map(x => x.id)) + 1;
const today = () => new Date().toISOString().slice(0, 10);
/** The register's lists (older plans: the standard ones). */
const listsOf = (r: DocRegister) => (r.lists?.length ? r.lists : LISTS());
/** A list's values for a drop-down, and any value the documents use that the list lacks (plans from before the lists). */
const listOpts = (field: string): Opt[] => {
  const items = (listsOf(reg()).find(l => l.field === field)?.items ?? []).map(i => ({ value: i.value, label: i.label || (i.off ? "not counted" : "") }));
  const more = [...new Set(reg().docs.map(d => fieldOf(d, field).trim()).filter(v => v && !items.some(i => i.value.toLowerCase() === v.toLowerCase())))].sort();
  return [...items, ...more.map(value => ({ value }))];
};

/** The next submission of a document: the next revision letter and the purpose its cycle needs. */
const nextSub = (d: VDoc, log = reg().log): Submission => {
  const r = reg(), mine = log.filter(s => s.doc === d.id), last = mine[mine.length - 1];
  const rev = !last ? "A" : /^[A-Y]$/.test(last.rev) ? String.fromCharCode(last.rev.charCodeAt(0) + 1) : /^\d+$/.test(last.rev) ? String(+last.rev + 1) : /^([A-Z]+)(\d+)$/.test(last.rev) ? last.rev.replace(/\d+$/, n => String(+n + 1)) : last.rev;
  const code = r.codes.find(c => c.code === last?.code);
  // The purpose the cycle needs next: the same again when it is with the client or sent back, else
  // the first issue step of the class that nothing has been sent for yet.
  const steps = classesOf(r)[Math.max(1, d.class) - 1]?.steps ?? [];
  const sentFor = (s: DocStep) => mine.some(x => x.sent && purposesOf(s).some(p => p.toUpperCase() === x.purpose.toUpperCase()));
  const nextIssue = steps.find(s => !s.returned && !sentFor(s));
  const purpose = last && (code?.resubmit || !last.returned) ? last.purpose : (nextIssue ? purposesOf(nextIssue)[0] : undefined) ?? last?.purpose ?? r.flow.first;
  return { id: next(log), doc: d.id, rev, purpose, transmittal: "", sent: today(), ret_transmittal: "", returned: null, code: "", crs: "" };
};
/** The transmittal number after the highest one in the log (TR-0009 → TR-0010). */
const nextTransmittal = () => {
  const all = reg().log.map(s => s.transmittal).filter(t => /\d+\s*$/.test(t));
  if (!all.length) return "TR-0001";
  const best = all.reduce((a, b) => (+(b.match(/(\d+)\s*$/)![1]) > +(a.match(/(\d+)\s*$/)![1]) ? b : a));
  return best.replace(/(\d+)(\s*)$/, (_, n: string, s: string) => String(+n + 1).padStart(n.length, "0") + s);
};

// ---------- Settings ----------
const CalChoice = (p: { label: string; value: string; set: (v: string) => void }) => {
  const own = () => !!p.value;
  const make = async () => {
    const n = await askText("New calendar", "Name (for example: the client's working week, or the engineering office abroad)", "");
    if (!n) return;
    const base = P().calendars.find(c => c.name === P().calendar) ?? P().calendars[0];
    if (await act({ CalPut: { old: null, cal: { ...JSON.parse(JSON.stringify(base)), name: n } } })) { p.set(n); D.calendars(n); }
  };
  return <div class="dcal"><b>{p.label}</b>
    <label class="chk"><input type="radio" checked={!own()} onChange={() => p.set("")} /> The plan's calendar ({P().calendar})</label>
    <label class="chk"><input type="radio" checked={own()} onChange={() => p.set(P().calendars.find(c => c.name !== P().calendar)?.name ?? P().calendar)} /> A calendar of its own:
      <select disabled={!own()} value={p.value} onChange={e => p.set(e.currentTarget.value)}><For each={P().calendars}>{c => <option selected={c.name === p.value}>{c.name}</option>}</For></select></label>
    <span class="dcalb"><button class="btn sm" onClick={make}>New calendar…</button><button class="btn sm" disabled={!own()} onClick={() => D.calendars(p.value)}>Working time…</button></span>
  </div>;
};

const TABS: [string, string][] = [["general", "General"], ["codes", "Codes & purposes"], ["classes", "Classes & steps"], ["lists", "Lists"], ["columns", "Columns"], ["words", "Status words"], ["templates", "Templates"], ["delete", "Delete register"]];

export const docSettings = (first = "general") => openDialog(() => {
  const start = JSON.parse(JSON.stringify(reg())) as DocRegister;
  const [r, setR] = createSignal<DocRegister>({ ...start, classes: classesOf(start) });
  const [tab, setTab] = createSignal(first);
  // Lists and columns remember their first value, so a rename reaches every document using it.
  const [lists, setLists] = createSignal<LList[]>(editLists(listsOf(start), f => start.docs.map(d => fieldOf(d, f))));
  const [cols, setCols] = createSignal<LCol[]>((start.columns ?? []).map(c => ({ ...c, was: c.name })));
  const put = (x: Partial<DocRegister>) => setR({ ...r(), ...x });
  const cls = () => r().classes ?? [];
  const purposes = () => r().purposes.map(p => p[0]);
  const total = (k: number) => cls()[k]?.steps.reduce((a, s) => a + (s.weight || 0), 0) ?? 0;
  const setC = (k: number, x: Partial<DocClass>) => put({ classes: cls().map((c, j) => (j === k ? { ...c, ...x } : c)) });
  const setS = (k: number, i: number, x: Partial<DocStep>) => setC(k, { steps: cls()[k].steps.map((s, j) => (j === i ? { ...s, ...x } : s)) });
  const move = <T,>(v: T[], i: number, d: number) => { const w = [...v], j = i + d; if (j < 0 || j >= w.length) return w; [w[i], w[j]] = [w[j], w[i]]; return w; };
  const used = (field: string, value: string) => start.docs.filter(d => fieldOf(d, field).toLowerCase() === value.toLowerCase()).length;
  // Templates: a client's rules kept by name, for any plan (vendors and packages stay with the project).
  const tpls = () => opts()?.docTemplates ?? {};
  const RULES = ["p_unit", "review_days", "turnaround_days", "codes", "purposes", "flow", "classes", "words", "columns"] as const;
  const saveTpl = async () => {
    const name = (await askText("Save as Template", "Template name (for example the client's name)", ""))?.trim(); if (!name) return;
    const t = { ...Object.fromEntries(RULES.map(k => [k, k === "columns" ? cols().map(({ was, ...c }) => c) : r()[k]])), lists: lists().map(l => ({ name: l.name, field: l.field, items: ["vendor", "package"].includes(l.field) ? [] : l.items.map(({ was, ...i }) => i) })) } as Partial<DocRegister>;
    const o = { ...opts()!, docTemplates: { ...tpls(), [name]: t } } as Options;
    await call("set_options", { opts: o }); setOpts(o); toast(`Template “${name}” saved: Settings › Templates gives these rules to any plan's register.`, true);
  };
  const useTpl = (name: string) => {
    const t = JSON.parse(JSON.stringify(tpls()[name] ?? null)) as Partial<DocRegister> | null; if (!t) return;
    setR({ ...r(), ...t, lists: undefined, columns: undefined } as DocRegister);
    if (t.columns) setCols([...cols(), ...t.columns.filter(c => !cols().some(x => x.name === c.name))]);
    // A template's lists join the plan's: its items are added, nothing is taken away.
    if (t.lists) setLists(lists().map(l => { const tl = t.lists!.find(x => x.field === l.field); return tl ? { ...l, items: [...l.items, ...tl.items.filter(i => !l.items.some(x => x.value.toLowerCase() === i.value.toLowerCase()))] } : l; })
      .concat(t.lists.filter(tl => !lists().some(l => l.field === tl.field))));
    toast(`Template “${name}” applied: Save to keep it.`, true);
  };
  const delTpl = async (name: string) => { const m = { ...tpls() }; delete m[name]; const o = { ...opts()!, docTemplates: m } as Options; await call("set_options", { opts: o }); setOpts(o); };
  const ok = async () => {
    const bad = cls().findIndex((_, k) => Math.abs(total(k) - 100) > 0.01);
    if (bad >= 0) { toast(`The step weights of class ${cls()[bad].name} must add up to 100 %.`); setTab("classes"); return false; }
    const s = await settle(start.docs, lists(), cols(), listsOf(start), fieldOf, setField);
    return save({ ...r(), docs: s.rows, columns: s.columns, lists: s.lists });
  };
  return (
    <Dialog title="Document Register Settings" wide help="documents" okText="Save" ok={ok}
      extra={<div class="inline"><button class="btn sm" onClick={e => openMenu(e, Object.keys(tpls()).length ? Object.keys(tpls()).map(n => ({ label: n, run: () => useTpl(n) })) : [{ label: "No templates yet: Save as Template first", off: true }])}>Use a template ▾</button>
        <button class="btn sm" onClick={saveTpl}>Save as Template…</button></div>}>
      <div class="stabs2"><For each={TABS}>{t => <button classList={{ on: tab() === t[0] }} onClick={() => setTab(t[0])}>{t[1]}</button>}</For></div>
      <Show when={tab() === "general"}><div class="dset">
        <div>
          <h3>Planned dates</h3>
          <div class="prow"><label>P date (purchase order or kick-off)<input type="date" value={r().p_date ?? ""} placeholder={P().start} onChange={e => put({ p_date: e.currentTarget.value || null })} /></label>
            <label>P + n counts in<select value={r().p_unit} onChange={e => put({ p_unit: e.currentTarget.value })}><option value="weeks" selected={r().p_unit !== "days"}>weeks</option><option value="days" selected={r().p_unit === "days"}>days</option></select></label></div>
          <p class="muted">Empty: the plan's start ({dt(P().start)}). Each document's first issue is P + n, or a date of its own.</p>
          <div class="prow"><label>Client review (working days)<input type="number" min="0" value={r().review_days} onChange={e => put({ review_days: +e.currentTarget.value || 0 })} /></label>
            <label>Our turnaround (working days)<input type="number" min="0" value={r().turnaround_days} onChange={e => put({ turnaround_days: +e.currentTarget.value || 0 })} /></label></div>
        </div>
        <div>
          <CalChoice label="Client review days count on" value={r().review_cal} set={v => put({ review_cal: v })} />
          <CalChoice label="Our turnaround days count on" value={r().work_cal} set={v => put({ work_cal: v })} />
        </div>
      </div></Show>
      <Show when={tab() === "codes"}><div class="dset">
        <div>
          <h3>Return codes</h3>
          <p class="muted">Each client's codes mean what you say here: tick <b>Accepted</b> for a code that lets the document go on, <b>Resubmit</b> for one that sends it back (even if that is Code 1).</p>
          <table class="tbl"><thead><tr><th>Code</th><th>Meaning</th><th title="Counts as accepted">Accepted</th><th title="Asks for the document again">Resubmit</th><th /></tr></thead>
            <tbody><For each={r().codes}>{(c, i) => { const set = (x: Partial<typeof c>) => put({ codes: r().codes.map((y, j) => (j === i() ? { ...y, ...x } : y)) }); return <tr>
              <td><input style={{ width: "90px" }} value={c.code} onChange={e => set({ code: e.currentTarget.value })} /></td><td><input value={c.meaning} onChange={e => set({ meaning: e.currentTarget.value })} /></td>
              <td><input type="checkbox" checked={c.accepted} onChange={e => set({ accepted: e.currentTarget.checked, resubmit: e.currentTarget.checked ? false : c.resubmit })} /></td>
              <td><input type="checkbox" checked={c.resubmit} onChange={e => set({ resubmit: e.currentTarget.checked, accepted: e.currentTarget.checked ? false : c.accepted })} /></td>
              <td><button class="x" onClick={() => put({ codes: r().codes.filter((_, j) => j !== i()) })}>×</button></td></tr>; }}</For></tbody></table>
          <button class="btn sm" onClick={() => put({ codes: [...r().codes, { code: "New code", meaning: "", accepted: false, resubmit: false }] })}>+ Code</button>
        </div>
        <div>
          <h3>Issue purposes</h3>
          <textarea rows="8" value={r().purposes.map(p => `${p[0]} = ${p[1]}`).join("\n")} onChange={e => put({ purposes: e.currentTarget.value.split("\n").map(l => l.split("=").map(x => x.trim())).filter(x => x[0]).map(x => [x[0], x[1] ?? ""] as [string, string]) })} />
          <p class="muted">One per line: code = meaning.</p>
          <h3>Which issues mean what</h3>
          <p class="muted">Orders wait for a document's approval issue to be accepted; a task waits for its construction issue.</p>
          <div class="prow">
            {(["first", "approval", "construction", "final"] as const).map(k => <label>{{ first: "First issue", approval: "Approval issue", construction: "Construction issue", final: "Final issue" }[k]}<select value={r().flow[k]} onChange={e => put({ flow: { ...r().flow, [k]: e.currentTarget.value } })}><For each={purposes()}>{x => <option selected={x === r().flow[k]}>{x}</option>}</For></select></label>)}
          </div>
        </div>
      </div></Show>
      <Show when={tab() === "classes"}><div class="dclasses">
        <p class="muted">Each class has its own steps. A step counts when an issue with one of its purposes is <b>sent</b>, or when it <b>comes back</b> with a code that accepts it (or one of the codes you name, such as Code 1, Code 4). Its weight is the % of the document earned there. Days: from the step before (empty: the review period before a return, the turnaround after one).</p>
        <For each={cls()}>{(c, k) => <fieldset class="dcls"><legend><input class="dcname" value={c.name} title="The class's name, as the register writes it (1, 2, A, Approval…)" onChange={e => setC(k(), { name: e.currentTarget.value })} />
          <span classList={{ bad: Math.abs(total(k()) - 100) > 0.01 }}>Total {total(k())} %</span><button class="x" title="Remove the class" disabled={cls().length < 2} onClick={() => put({ classes: cls().filter((_, j) => j !== k()) })}>×</button></legend>
          <div class="dscroll"><table class="tbl dsteps"><thead><tr><th>Step</th><th>Issue purposes</th><th>Counts when</th><th>Codes that count</th><th>Weight %</th><th>Days</th><th /></tr></thead>
            <tbody><For each={c.steps}>{(s, i) => <tr>
              <td><input value={s.name} onChange={e => setS(k(), i(), { name: e.currentTarget.value })} /></td>
              <td><input list="dpurposes" style={{ width: "92px" }} value={s.purposes} placeholder="ICR, IFC" onChange={e => setS(k(), i(), { purposes: e.currentTarget.value })} /></td>
              <td><select value={s.returned ? "1" : ""} onChange={e => setS(k(), i(), { returned: !!e.currentTarget.value })}><option value="" selected={!s.returned}>sent</option><option value="1" selected={s.returned}>comes back</option></select></td>
              <td><input style={{ width: "110px" }} disabled={!s.returned} value={s.codes} placeholder={s.returned ? "any that accepts" : ""} onChange={e => setS(k(), i(), { codes: e.currentTarget.value })} /></td>
              <td><input type="number" min="0" max="100" style={{ width: "56px" }} value={s.weight} onChange={e => setS(k(), i(), { weight: +e.currentTarget.value || 0 })} /></td>
              <td><input type="number" min="0" style={{ width: "50px" }} value={s.days ?? ""} placeholder="auto" onChange={e => setS(k(), i(), { days: e.currentTarget.value === "" ? null : Math.max(0, +e.currentTarget.value) })} /></td>
              <td class="nowrap"><button class="x" title="Up" onClick={() => setC(k(), { steps: move(c.steps, i(), -1) })}>↑</button><button class="x" title="Down" onClick={() => setC(k(), { steps: move(c.steps, i(), 1) })}>↓</button><button class="x" title="Remove the step" onClick={() => setC(k(), { steps: c.steps.filter((_, j) => j !== i()) })}>×</button></td></tr>}</For></tbody></table></div>
          <button class="btn sm" onClick={() => setC(k(), { steps: [...c.steps, { name: "New step", purposes: "", returned: false, codes: "", weight: 0, days: null }] })}>+ Step</button></fieldset>}</For>
        <datalist id="dpurposes"><For each={purposes()}>{p => <option value={p} />}</For></datalist>
        <div class="inline"><button class="btn sm" onClick={() => put({ classes: [...cls(), { name: String(cls().length + 1), steps: JSON.parse(JSON.stringify(cls()[cls().length - 1]?.steps ?? [])) }] })}>+ Class</button>
          <button class="btn sm" onClick={() => put({ classes: classesOf({ ...r(), classes: [] }) })}>Back to the standard classes</button></div>
      </div></Show>
      <Show when={tab() === "lists"}><ListsTab lists={lists} set={setLists} cols={cols} setCols={setCols} fixed={["vendor", "package", "disc", "kind", "state"]} count={used} names={{ disc: "name" }} rows="document" /></Show>
      <Show when={tab() === "columns"}><ColumnsTab cols={cols} set={setCols} table="Documents" examples={["=[Review due]-TODAY()", '=IF([Status]="Resubmit","Chase","")', "=[First planned]+14"]} /></Show>
      <Show when={tab() === "words"}><WordsTab words={WORDS} value={() => r().words ?? {}} set={w => put({ words: w })} /></Show>
      <Show when={tab() === "templates"}>
        <p class="muted">A template keeps one client's rules: review days, codes, purposes, classes and steps, status words, your own columns, and the lists (not the vendors and packages, which belong to the project). Use it for the next project with that client.</p>
        <div class="inline"><button class="btn" onClick={saveTpl}>Save as Template…</button></div>
        <table class="tbl" style={{ "margin-top": "10px" }}><thead><tr><th>Template</th><th /></tr></thead>
          <tbody><For each={Object.keys(tpls())} fallback={<tr><td class="muted">No templates yet.</td><td /></tr>}>{n => <tr><td><b>{n}</b></td><td class="nowrap"><button class="btn sm" onClick={() => useTpl(n)}>Use</button> <button class="btn sm" onClick={() => delTpl(n)}>Delete</button></td></tr>}</For></tbody></table>
      </Show>
      <Show when={tab() === "delete"}>
        <p>Delete the register's contents. Edit › Undo (Ctrl+Z) brings them back while the plan is open.</p>
        <div class="inline"><button class="btn" onClick={async () => { if (await askYes("Delete every document", `Delete all ${reg().docs.length} documents and their ${reg().log.length} submissions? The settings, lists and classes stay.`, "Delete documents")) { await save({ docs: [], log: [] }); closeDialog(); } }}>Delete every document…</button>
          <button class="btn danger" onClick={async () => { if (await askYes("Delete the register", "Delete the whole register: documents, submissions, settings, lists, classes and columns? The module stays on, empty.", "Delete the register")) { await act({ Documents: { r: { ...DEF, lists: LISTS() } } }); closeDialog(); } }}>Delete the whole register…</button></div>
        <p class="muted">To take the module away from the plan: Project › Modules, untick the Document Register (you choose there whether its data stays in the plan).</p>
      </Show>
    </Dialog>
  );
});
const fieldOf = (d: VDoc, f: string) => (f === "vendor" ? d.vendor : f === "package" ? d.package : f === "disc" ? d.disc : f === "kind" ? d.kind : f === "state" ? d.state ?? "" : d.own?.[f] ?? "");
const setField = (d: VDoc, f: string, v: string): VDoc => (["vendor", "package", "disc", "kind", "state"].includes(f) ? { ...d, [f]: v } : { ...d, own: { ...(d.own ?? {}), [f]: v } });

// ---------- A document's details ----------
const editDoc = (d: VDoc) => openDialog(() => {
  const [x, setX] = createSignal<VDoc>({ ...d, flags: [...d.flags], own: { ...(d.own ?? {}) } });
  const put = (v: Partial<VDoc>) => setX({ ...x(), ...v });
  const tasks = () => grid().rows.filter(t => t.uid > 0);
  const T = (p: { l: string; k: "number" | "title" | "remarks" }) => <label>{p.l}<input value={x()[p.k]} onChange={e => put({ [p.k]: e.currentTarget.value } as Partial<VDoc>)} /></label>;
  const L = (p: { l: string; k: "vendor" | "package" | "disc" | "kind" | "state" }) => <label>{p.l}<input list={`dl-${p.k}`} value={x()[p.k] ?? ""} onChange={e => put({ [p.k]: e.currentTarget.value } as Partial<VDoc>)} />
    <datalist id={`dl-${p.k}`}><For each={listOpts(p.k)}>{o => <option value={o.value}>{o.label}</option>}</For></datalist></label>;
  return (
    <Dialog title={`Document ${x().number || ""}`} wide help="documents" okText="Save" ok={() => save({ docs: reg().docs.map(y => (y.id === d.id ? x() : y)) })}>
      <div class="dform">
        <T l="Document number" k="number" /><T l="Title" k="title" /><L l="Vendor" k="vendor" /><L l="Package / sub-system" k="package" /><L l="Discipline" k="disc" /><L l="Document type" k="kind" /><L l="State" k="state" />
        <label>Class<select value={x().class} onChange={e => put({ class: +e.currentTarget.value })}><For each={classesOf(reg())}>{(c, i) => <option value={i() + 1} selected={i() + 1 === x().class}>{c.name}</option>}</For></select></label>
        <label>First issue at P +<input type="number" min="0" step="1" value={x().offset ?? ""} onChange={e => put({ offset: e.currentTarget.value === "" ? null : +e.currentTarget.value })} /></label>
        <label>…or on this date<input type="date" value={x().first ?? ""} onChange={e => put({ first: e.currentTarget.value || null })} /></label>
        <label>Weight<input type="number" min="0" step="0.5" value={x().weight} onChange={e => put({ weight: +e.currentTarget.value || 0 })} /></label>
        <label>Needed for task<select value={String(x().task)} onChange={e => put({ task: +e.currentTarget.value })}><option value="0">—</option><For each={tasks()}>{t => <option value={String(t.uid)} selected={t.uid === x().task}>{t.id} · {t.name}</option>}</For></select></label>
        <T l="Remarks / action" k="remarks" />
        <For each={(reg().columns ?? []).filter(c => c.kind !== "formula")}>{c => <label>{c.name}<input type={c.kind === "date" ? "date" : c.kind === "number" ? "number" : "text"} list={c.kind === "list" ? `dl-own-${c.name}` : undefined} value={x().own?.[c.name] ?? ""} onChange={e => put({ own: { ...(x().own ?? {}), [c.name]: e.currentTarget.value } })} />
          <Show when={c.kind === "list"}><datalist id={`dl-own-${c.name}`}><For each={listOpts(c.name)}>{o => <option value={o.value}>{o.label}</option>}</For></datalist></Show></label>}</For>
      </div>
      <div class="dflags">
        <label class="chk"><input type="checkbox" checked={x().key} onChange={e => put({ key: e.currentTarget.checked })} /> Key document</label>
        <label class="chk"><input type="checkbox" checked={x().progress} onChange={e => put({ progress: e.currentTarget.checked })} /> Counted in progress</label>
        <label class="chk"><input type="checkbox" checked={x().recurring} onChange={e => put({ recurring: e.currentTarget.checked })} /> Recurring submission (a monthly report: never counted)</label>
        <For each={FLAGS}>{f => <label class="chk"><input type="checkbox" checked={x().flags.includes(f)} onChange={e => put({ flags: e.currentTarget.checked ? [...x().flags, f] : x().flags.filter(y => y !== f) })} /> {f}</label>}</For>
      </div>
    </Dialog>
  );
});

// ---------- Log a submission, or a return, for many documents at once ----------
const logMany = (docs: VDoc[], after?: () => void) => openDialog(() => {
  const [tr, setTr] = createSignal(nextTransmittal()), [sent, setSent] = createSignal(today()), [print, setPrint] = createSignal(false);
  const [rows, setRows] = createSignal(docs.map(d => { const s = nextSub(d); return { d, rev: s.rev, purpose: s.purpose }; }));
  const setRow = (i: number, x: Partial<{ rev: string; purpose: string }>) => setRows(rows().map((r, j) => (j === i ? { ...r, ...x } : r)));
  const ok = async () => {
    if (!sent()) { toast("Give the date sent."); return false; }
    const log = [...reg().log]; let id = next(log);
    for (const r of rows()) log.push({ id: id++, doc: r.d.id, rev: r.rev, purpose: r.purpose, transmittal: tr().trim(), sent: sent(), ret_transmittal: "", returned: null, code: "", crs: "" });
    const done = await save({ log });
    if (done) { toast(`${rows().length} submissions logged under ${tr() || "no transmittal number"}.`, true); after?.(); if (print()) printTransmittal(tr()); }
    return done;
  };
  return (
    <Dialog title="Log submission" wide help="documents" okText={`Log ${rows().length} submission${rows().length > 1 ? "s" : ""}`} ok={ok}>
      <div class="prow"><label>Transmittal no.<input class="mono" value={tr()} onInput={e => setTr(e.currentTarget.value)} /></label><label>Sent on<input type="date" value={sent()} onInput={e => setSent(e.currentTarget.value)} /></label></div>
      <table class="tbl" style={{ "margin-top": "10px" }}><thead><tr><th>Document no.</th><th>Title</th><th>Class</th><th>Rev</th><th>Purpose</th></tr></thead>
        <tbody><For each={rows()}>{(r, i) => <tr><td class="mono">{r.d.number}</td><td>{r.d.title}</td><td>{className(r.d)}</td>
          <td><input style={{ width: "54px" }} value={r.rev} onChange={e => setRow(i(), { rev: e.currentTarget.value })} /></td>
          <td><select value={r.purpose} onChange={e => setRow(i(), { purpose: e.currentTarget.value })}><For each={[...new Set([...reg().purposes.map(p => p[0]), r.purpose])]}>{p => <option selected={p === r.purpose}>{p}</option>}</For></select></td></tr>}</For></tbody></table>
      <p class="muted">Each document gets its own row in the transmittal log under the same transmittal number. Revision and purpose are proposed from each document's class steps; change any of them here. The review due date is worked out from the review calendar.</p>
      <label class="chk"><input type="checkbox" checked={print()} onChange={e => setPrint(e.currentTarget.checked)} /> Make the transmittal cover sheet (PDF) afterwards</label>
    </Dialog>
  );
});
const returnMany = (docs: VDoc[]) => openDialog(() => {
  const open = (d: VDoc) => [...reg().log].filter(s => s.doc === d.id && s.sent && !s.returned).sort((a, b) => (b.sent ?? "").localeCompare(a.sent ?? ""))[0];
  const withOpen = docs.filter(d => open(d)), without = docs.filter(d => !open(d));
  const [date, setDate] = createSignal(today()), [code, setCode] = createSignal(reg().codes[0]?.code ?? ""), [rt, setRt] = createSignal(""), [crs, setCrs] = createSignal("");
  const [codes, setCodes] = createSignal<Record<number, string>>({});
  const ok = () => {
    if (!withOpen.length) return true;
    const ids = new Map(withOpen.map(d => [open(d)!.id, d.id] as const));
    return save({ log: reg().log.map(s => (ids.has(s.id) ? { ...s, returned: date(), code: codes()[ids.get(s.id)!] ?? code(), ret_transmittal: rt() || s.ret_transmittal, crs: crs() || s.crs } : s)) });
  };
  return (
    <Dialog title="Log return" wide help="documents" okText={withOpen.length ? `Log ${withOpen.length} return${withOpen.length > 1 ? "s" : ""}` : "Close"} ok={ok}>
      <div class="prow"><label>Returned on<input type="date" value={date()} onInput={e => setDate(e.currentTarget.value)} /></label>
        <label>Code<select value={code()} onChange={e => setCode(e.currentTarget.value)}><For each={reg().codes}>{c => <option value={c.code} selected={c.code === code()} title={c.meaning}>{c.code}</option>}</For></select></label>
        <label>Return transmittal no.<input value={rt()} onInput={e => setRt(e.currentTarget.value)} /></label><label>Comments ref.<input value={crs()} onInput={e => setCrs(e.currentTarget.value)} /></label></div>
      <table class="tbl" style={{ "margin-top": "10px" }}><thead><tr><th>Document no.</th><th>Title</th><th>Open submission</th><th>Code</th></tr></thead>
        <tbody><For each={withOpen}>{d => { const s = open(d)!; return <tr><td class="mono">{d.number}</td><td>{d.title}</td><td>Rev {s.rev} · {s.purpose} · sent {dt(s.sent)} · {s.transmittal}</td>
          <td><select value={codes()[d.id] ?? code()} onChange={e => setCodes({ ...codes(), [d.id]: e.currentTarget.value })}><For each={reg().codes}>{c => <option value={c.code} selected={c.code === (codes()[d.id] ?? code())}>{c.code}</option>}</For></select></td></tr>; }}</For></tbody></table>
      <Show when={without.length}><p class="muted">Nothing open with the client for {without.map(d => d.number || d.title).join(", ")}: left as they are.</p></Show>
      <p class="muted">The return date and code go on each document's latest submission that has not come back yet.</p>
    </Dialog>
  );
});

// ---------- The transmittal cover sheet ----------
const printTransmittal = async (no: string) => {
  const r = reg(), subs = r.log.filter(s => s.transmittal === no);
  if (!subs.length) { toast(`No submissions under ${no}.`); return; }
  const esc = (s: string) => s.replace(/[&<>"]/g, c => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" }[c]!));
  const doc = (id: number) => r.docs.find(d => d.id === id);
  const vendors = [...new Set(subs.map(s => doc(s.doc)?.vendor).filter(Boolean))].join(", ");
  const el = document.createElement("div");
  el.id = "printroot"; el.className = "trprint";
  el.innerHTML = `<div class="dtrsheet"><h1>Transmittal ${esc(no)}</h1><div class="meta"><b>Project</b><span>${esc(P().name)}</span><b>Date sent</b><span>${esc(dt(subs[0].sent))}</span><b>From</b><span>${esc(vendors)}</span><b>Documents</b><span>${subs.length}</span></div>
    <table><thead><tr><th>#</th><th>Document no.</th><th>Title</th><th>Rev</th><th>Purpose</th><th>Class</th></tr></thead><tbody>${subs.map((s, i) => { const d = doc(s.doc); return `<tr><td>${i + 1}</td><td>${esc(d?.number ?? "")}</td><td>${esc(d?.title ?? "")}</td><td>${esc(s.rev)}</td><td>${esc(s.purpose)} – ${esc(r.purposes.find(p => p[0] === s.purpose)?.[1] ?? "")}</td><td>${esc(d ? className(d) : "")}</td></tr>`; }).join("")}</tbody></table>
    <div class="sign"><div>Sent by (name, signature, date)</div><div>Received by (name, signature, date)</div></div></div>`;
  if (document.getElementById("printroot")) { toast("Close File › Print first."); return; }
  document.body.append(el);
  document.documentElement.classList.add("printing");
  const end = () => { document.documentElement.classList.remove("printing"); el.remove(); window.removeEventListener("afterprint", end); };
  try { const path = await call<string | null>("save_pdf", { name: `Transmittal ${no}`, wMm: 210, hMm: 297, margins: [15, 15, 15, 15] }); end(); if (path) toast(`Saved ${path}`, true); }
  catch { window.addEventListener("afterprint", end); window.print(); }
};

// ---------- The document card ----------
const [cardId, setCardId] = createSignal<number | null>(null);
export const openCard = (id: number) => setCardId(id);
const Card = (p: { o: RegOut }) => {
  const d = () => reg().docs.find(x => x.id === cardId());
  const row = () => p.o.docs.find(x => x.id === cardId());
  const subs = () => reg().log.filter(s => s.doc === cardId()).sort((a, b) => (a.sent ?? "").localeCompare(b.sent ?? "") || a.id - b.id);
  const code = (c: string) => reg().codes.find(x => x.code === c);
  const purpose = (x: string) => reg().purposes.find(p => p[0] === x)?.[1] ?? "";
  const stepNow = () => { const r = row(); return r ? r.actual.findIndex(a => !a) : -1; };
  return <Show when={d() && row()}>
    <Portal><div class="dcard" onKeyDown={e => e.key === "Escape" && setCardId(null)}>
      <div class="inline"><span class="num">{d()!.number || "(no number)"}</span><span class="grow" /><button class="x" title="Close" onClick={() => setCardId(null)}>×</button></div>
      <h2>{d()!.title || "(no title)"}</h2>
      <div class="chips"><For each={[d()!.vendor, d()!.package, d()!.disc && `${d()!.disc}${listOpts("disc").find(o => o.value === d()!.disc)?.label ? ` · ${listOpts("disc").find(o => o.value === d()!.disc)!.label}` : ""}`, d()!.kind, `Class ${className(d()!)}`, d()!.state || "", d()!.flags.join(" · "), d()!.key ? "Key document" : ""].filter(Boolean)}>{c => <span class="chip">{c}</span>}</For></div>
      <div><span class={`st ${SCOL[row()!.skey] ?? ""}`}>{row()!.status}</span> <span class="muted">· earned {row()!.earned.toFixed(0)} %{row()!.late_for_task ? ` · late for ${row()!.task_name}` : ""}</span></div>
      <div class="dsteps2"><For each={row()!.steps}>{(s, i) => <div classList={{ d: !!row()!.actual[i()], c: i() === stepNow() }} title={`${s}: planned ${dt(row()!.planned[i()])}${row()!.actual[i()] ? `, done ${dt(row()!.actual[i()])}` : row()!.forecast[i()] ? `, forecast ${dt(row()!.forecast[i()])}` : ""}`}>{s}</div>}</For></div>
      <b class="muted" style={{ "font-size": "10px", "letter-spacing": ".08em" }}>HISTORY</b>
      <div class="dtl">
        <For each={subs()}>{s => <>
          <div class="ev done"><b>Rev {s.rev} · {s.purpose}</b> sent {dt(s.sent)}{s.transmittal ? ` · ${s.transmittal}` : ""}<small>{purpose(s.purpose)}{p.o.subs.find(x => x.id === s.id)?.due ? ` · review due ${dt(p.o.subs.find(x => x.id === s.id)!.due)}` : ""}</small></div>
          <Show when={s.returned}><div class="ev" classList={{ bad: !!code(s.code)?.resubmit, done: !!code(s.code)?.accepted }}><b>Returned {s.code || "(no code)"}</b> {dt(s.returned)}{s.ret_transmittal ? ` · ${s.ret_transmittal}` : ""}
            <small>{code(s.code)?.meaning ?? ""}{(p.o.subs.find(x => x.id === s.id)?.late ?? 0) > 0 ? ` · the client took ${p.o.subs.find(x => x.id === s.id)!.late} days past the review period` : ""}{s.crs ? ` · ${s.crs}` : ""}</small></div></Show>
        </>}</For>
        <For each={row()!.steps.map((s, i) => [s, i] as const).filter(([, i]) => !row()!.actual[i])}>{([s, i]) => <div class="ev next"><b>{s}</b> planned {dt(row()!.planned[i]) || "—"}<small>forecast {dt(row()!.forecast[i]) || "—"}</small></div>}</For>
      </div>
      <div class="acts"><button class="btn pri" onClick={() => logMany([d()!])}>Log submission</button><button class="btn" onClick={() => returnMany([d()!])}>Log return</button><button class="btn" onClick={() => editDoc(d()!)}>Edit details</button>
        <span class="grow" /><button class="btn danger" onClick={async () => { const x = d()!; if (await askYes("Delete the document", `Delete ${x.number || x.title} and its ${subs().length} submissions?`, "Delete")) { setCardId(null); save({ docs: reg().docs.filter(y => y.id !== x.id), log: reg().log.filter(s => s.doc !== x.id) }); } }}>Delete</button></div>
    </div></Portal>
  </Show>;
};

// ---------- Import from a spreadsheet ----------
type Field = [string, string, RegExp];
const DOC_FIELDS: Field[] = [["number", "Document number", /doc.*(no|num)|document no|number/i], ["state", "State (Active, Hold, Deleted…)", /doc.*state|^state$|row state/i], ["title", "Title", /title|description/i], ["vendor", "Vendor", /vendor|supplier/i], ["package", "Package / sub-system", /package|sub.?system/i],
  ["disc", "Discipline", /disc/i], ["kind", "Document type", /type/i], ["class", "Class", /class/i], ["first", "First issue date", /first issue date|planned date|\(date\)/i], ["offset", "First issue (P + n)", /planned sub|1st planned|p \+|offset/i],
  ["progress", "Counted in progress (Yes)", /incl.*progress|counted in progress|in progress/i],
  ["key", "Key document (Yes)", /key/i], ["weight", "Weight", /weight/i], ["remarks", "Remarks", /remark|comment/i], ...FLAGS.map(f => ["flag:" + f, `${f} (Yes)`, f === "Eng. review" ? /eng.*review|review.*eng/i : f === "QA review" ? /\bqa\b.*review|review.*(qual|\bqa\b)/i : new RegExp(`\\b${f}\\b`, "i")] as Field)];
const LOG_FIELDS: Field[] = [["number", "Document number", /doc.*(no|num)|document no/i], ["rev", "Revision", /^rev/i], ["purpose", "Issue purpose", /purpose/i], ["transmittal", "Transmittal no.", /^transmittal|transmittal no/i],
  ["sent", "Date sent", /submission date|sent|submitted/i], ["ret_transmittal", "Return transmittal no.", /return transmittal/i], ["returned", "Date returned", /return date|returned/i], ["code", "Return code", /code/i], ["crs", "Comments ref.", /crs|remark/i]];
export const yes = (v: string) => /^(y|yes|true|x|✓|1)$/i.test(v.trim());
export const asDate = (v: string) => { const d = parseDate(v); return d || null; };

/** A workbook's settings sheet: return codes, issue purposes, the P date, the review and turnaround
 *  days, P + n in weeks or days, and public holidays (a calendar for the reviews). */
const readSettings = (rows: string[][]) => {
  const out: Partial<DocRegister> & { holidays?: [string, string][] } = {};
  const find = (re: RegExp) => { for (let i = 0; i < rows.length; i++) for (let j = 0; j < rows[i].length; j++) if (re.test(rows[i][j] ?? "")) return [i, j] as const; return null; };
  const beside = (re: RegExp) => { const at = find(re); if (!at) return null; return rows[at[0]].slice(at[1] + 1).find(x => x?.trim()) ?? null; };
  const below = (at: readonly [number, number] | null, two = true) => { if (!at) return []; const v: [string, string][] = []; for (let i = at[0] + 1; i < rows.length; i++) { const a = (rows[i][at[1]] ?? "").trim(); if (!a) break; v.push([a, two ? (rows[i][at[1] + 1] ?? "").trim() : ""]); } return v; };
  const codes = below(find(/^(review|return)\s*code/i));
  if (codes.length) out.codes = codes.map(([c, m]) => { const re = /resubmit|re-submit|reject|revise/i.test(m); return { code: c, meaning: m, accepted: !re, resubmit: re }; });
  const pur = below(find(/^issue purpose/i));
  if (pur.length) out.purposes = pur.map(([c, m]) => [c, m] as [string, string]);
  const pd = beside(/^p date|po date|kick-?off date/i); if (pd && asDate(pd)) out.p_date = asDate(pd);
  const unit = beside(/offset unit|p \+ n.*unit/i); if (unit) out.p_unit = /day/i.test(unit) ? "days" : "weeks";
  const rv = beside(/review period|review.*working days/i); if (rv && +rv >= 0) out.review_days = Math.round(+rv);
  const tn = beside(/turnaround|resubmission turnaround/i); if (tn && +tn >= 0) out.turnaround_days = Math.round(+tn);
  const hol = find(/public holidays|^holidays/i);
  if (hol) { const v: [string, string][] = []; for (let i = hol[0] + 1; i < rows.length && v.length < 200; i++) { const d = asDate(rows[i][hol[1]] ?? ""); if (d) v.push([d, (rows[i][hol[1] + 1] ?? "").trim()]); else if ((rows[i][hol[1]] ?? "").trim() && v.length) break; } if (v.length) out.holidays = v; }
  return out;
};

export const docImport = async () => {
  const first = await call<{ path: string; names: string[]; rows: string[][] } | null>("docs_read", {});
  if (!first) return;
  openDialog(() => {
    const [data, setData] = createSignal(first), [kind, setKind] = createSignal<"docs" | "log" | "settings">("docs"), [mode, setMode] = createSignal<"add" | "update">("update");
    // The heading row: of the first 30, the one where the most cells look like headings.
    const guessHead = (rows: string[][]) => { const sc = rows.slice(0, 30).map(r => r.filter(c => c.length < 60 && [...DOC_FIELDS, ...LOG_FIELDS].some(([, , re]) => re.test(c))).length); return Math.max(0, sc.indexOf(Math.max(...sc))); };
    const [head, setHead] = createSignal(guessHead(first.rows));
    const fields = () => (kind() === "log" ? LOG_FIELDS : DOC_FIELDS);
    const headers = () => data().rows[head()] ?? [];
    const guess = () => headers().reduce<string[]>((m, h) => { const f = fields().find(([k, , re]) => re.test(h) && !m.includes(k)); return [...m, f ? f[0] : ""]; }, []);
    const [map, setMap] = createSignal<string[]>(guess());
    const body = () => data().rows.slice(head() + 1).filter(r => r.some(c => c.trim()));
    const sheet = async (name: string) => { const d = await call<typeof first>("docs_read", { path: data().path, sheet: name }); if (d) { setData(d); setHead(guessHead(d.rows)); setMap(guess()); if (/setting/i.test(name)) setKind("settings"); } };
    const col = (r: string[], f: string) => { const j = map().indexOf(f); return j >= 0 ? (r[j] ?? "").trim() : ""; };
    const found = createMemo(() => (kind() === "settings" ? readSettings(data().rows) : {}));
    const run = async () => {
      const R = reg();
      if (kind() === "settings") {
        const s = found(), { holidays, ...rest } = s;
        if (!Object.keys(s).length) { toast("No codes, purposes, dates or holidays were found on this sheet."); return false; }
        let cal = R.review_cal;
        if (holidays?.length) {
          const name = "Document reviews (holidays from the register)";
          const base = P().calendars.find(c => c.name === (R.review_cal || P().calendar)) ?? P().calendars[0];
          const c = { ...JSON.parse(JSON.stringify(base)), name, exceptions: holidays.map(([d, n]) => ({ name: n || "Holiday", from: d, to: d, hours: [] })) };
          if (await act({ CalPut: { old: P().calendars.some(x => x.name === name) ? name : null, cal: c } })) cal = name;
        }
        await save({ ...rest, review_cal: cal, work_cal: holidays?.length ? cal : R.work_cal });
        toast(`Settings brought in: ${[s.codes && `${s.codes.length} codes`, s.purposes && `${s.purposes.length} purposes`, s.p_date && "the P date", s.review_days !== undefined && "review days", s.turnaround_days !== undefined && "turnaround days", holidays && `${holidays.length} holidays (a calendar for the reviews)`].filter(Boolean).join(", ")}.`, true);
        return;
      }
      if (!map().includes("number")) { toast("Choose the column with the document number."); return false; }
      if (kind() === "docs") {
        const docs = [...R.docs]; let id = next(docs), added = 0, changed = 0;
        for (const r of body()) {
          const number = col(r, "number"); if (!number) continue;
          const at = mode() === "update" ? docs.findIndex(d => d.number === number) : -1;
          const old = at >= 0 ? docs[at] : blankDoc(id);
          // A date in the P + n column is the first issue's own date; text such as "monthly" marks a recurring submission.
          const offTxt = col(r, "offset"), offDate = /\d{4}-\d{2}-\d{2}|\d{1,2}[/-]\w{2,3}[/-]\d{2,4}/.test(offTxt) ? asDate(offTxt) : null, off = offDate ? null : offTxt.match(/^-?\d+(\.\d+)?$/) ?? offTxt.match(/[+]\s*(\d+(\.\d+)?)/);
          const cname = col(r, "class"), ci = classesOf(R).findIndex(c => c.name.toLowerCase() === cname.toLowerCase());
          const st = col(r, "state"), stItem = listOpts("state").find(o => o.value.toLowerCase() === st.toLowerCase() || (/^(deleted|void|cancel)/i.test(st) && o.value === "Deleted"));
          const d: VDoc = { ...old, number, title: col(r, "title") || old.title, vendor: col(r, "vendor") || old.vendor, package: col(r, "package") || old.package, disc: col(r, "disc") || old.disc, kind: col(r, "kind") || old.kind,
            class: ci >= 0 ? ci + 1 : +cname >= 1 && +cname <= classesOf(R).length ? +cname : old.class, offset: off ? +(off[1] ?? off[0]) : old.offset, first: asDate(col(r, "first")) ?? offDate ?? old.first, key: map().includes("key") ? yes(col(r, "key")) : old.key,
            progress: map().includes("progress") ? !/^(no|n|false|0)$/i.test(col(r, "progress")) : old.progress,
            weight: col(r, "weight") ? +col(r, "weight") || old.weight : old.weight, remarks: col(r, "remarks") || old.remarks, state: map().includes("state") ? (stItem?.value ?? (st && !/^active$/i.test(st) ? st : "")) : old.state,
            flags: FLAGS.some(f => map().includes("flag:" + f)) ? FLAGS.filter(f => yes(col(r, "flag:" + f))) : old.flags,
            recurring: old.recurring || /mthly|monthly|weekly|recurr/i.test(col(r, "offset")) };
          if (at >= 0) { docs[at] = d; changed++; } else { docs.push(d); id++; added++; }
        }
        save({ docs }); toast(`${added} documents added, ${changed} updated. New vendors, packages, disciplines and types joined their lists.`, true);
      } else {
        const log = [...R.log]; let id = next(log), added = 0, missing = 0;
        for (const r of body()) {
          const d = R.docs.find(x => x.number === col(r, "number")); if (!d) { if (col(r, "number")) missing++; continue; }
          const s: Submission = { id, doc: d.id, rev: col(r, "rev"), purpose: col(r, "purpose"), transmittal: col(r, "transmittal"), sent: asDate(col(r, "sent")), ret_transmittal: col(r, "ret_transmittal"), returned: asDate(col(r, "returned")), code: col(r, "code"), crs: col(r, "crs") };
          if (mode() === "update" && log.some(x => x.doc === s.doc && x.rev === s.rev && x.purpose === s.purpose && x.sent === s.sent)) continue;
          log.push(s); id++; added++;
        }
        save({ log }); toast(`${added} submissions added${missing ? `; ${missing} rows name documents not in the register` : ""}.`, true);
      }
    };
    return (
      <Dialog title="Import into the Document Register" wide help="documents" okText="Import" ok={run}>
        <div class="prow"><label>Bring in<Seg value={kind()} set={v => { setKind(v as "docs"); setMap(guess()); }} options={[["docs", "Documents (the register)"], ["log", "Submissions (transmittal log)"], ["settings", "Settings (codes, purposes, holidays…)"]]} /></label></div>
        <div class="prow"><Show when={data().names.length > 1}><label>Sheet<select onChange={e => sheet(e.currentTarget.value)}><For each={data().names}>{n => <option>{n}</option>}</For></select></label></Show>
          <Show when={kind() !== "settings"}><label>Headings are in row<input type="number" min="1" value={head() + 1} onChange={e => { setHead(Math.max(0, +e.currentTarget.value - 1)); setMap(guess()); }} /></label>
          <label>Rows already in the register<select value={mode()} onChange={e => setMode(e.currentTarget.value as "add")}><option value="update">{kind() === "docs" ? "are updated (same document number)" : "are skipped (same document, revision, purpose and date)"}</option><option value="add">are added again</option></select></label></Show></div>
        <Show when={kind() !== "settings"} fallback={<div class="panel ctr"><p class="muted">Read from this sheet (choose the workbook's settings sheet above):</p>
          <ul>
            <li>Return codes: {found().codes ? found().codes!.map(c => `${c.code}${c.resubmit ? " (resubmit)" : ""}`).join(", ") : <span class="muted">none found (a heading “Review Code” or “Return Code” with codes under it)</span>}</li>
            <li>Issue purposes: {found().purposes ? found().purposes!.map(p => p[0]).join(", ") : <span class="muted">none found (a heading “Issue Purpose”)</span>}</li>
            <li>P date: {found().p_date ? dt(found().p_date) : <span class="muted">—</span>} · P + n in {found().p_unit ?? <span class="muted">—</span>} · review {found().review_days ?? "—"} and turnaround {found().turnaround_days ?? "—"} working days</li>
            <li>Holidays: {found().holidays ? `${found().holidays!.length} (they make a calendar for the reviews and the turnaround)` : <span class="muted">none found (a heading “Public holidays” with dates under it)</span>}</li>
          </ul><p class="muted">Codes are read as accepting unless their meaning says resubmit, revise or rejected; check them in Settings afterwards.</p></div>}>
          <p class="muted">{body().length} rows under the headings. Choose what each column holds; columns left on "—" are not read.</p>
          <div class="dmap"><table class="tbl"><thead><tr><th>Column</th><th>Holds</th><th>First rows</th></tr></thead>
            <tbody><For each={headers()}>{(h, j) => <tr><td><b>{h || `(column ${j() + 1})`}</b></td>
              <td><select value={map()[j()] ?? ""} onChange={e => setMap(headers().map((_, k) => (k === j() ? e.currentTarget.value : (map()[k] === e.currentTarget.value && e.currentTarget.value ? "" : map()[k] ?? ""))))}><option value="">—</option><For each={fields()}>{f => <option value={f[0]} selected={f[0] === map()[j()]}>{f[1]}</option>}</For></select></td>
              <td class="muted">{body().slice(0, 3).map(r => r[j()] ?? "").filter(Boolean).join(" · ").slice(0, 80)}</td></tr>}</For></tbody></table></div>
        </Show>
      </Dialog>
    );
  });
};

// ---------- The view ----------
const docTab = kept("docs-tab", "dashboard");
const pct = (v: number) => `${v.toFixed(0)} %`;
export const DocumentsView = () => {
  const [data] = createResource(() => [tick(), out()?.tab] as const, () => call<{ tab: number; name: string; on: boolean; out: RegOut }[]>("documents"));
  const me = () => data()?.find(d => d.tab === out()?.tab)?.out;
  const [tab, setTab] = docTab, [q, setQ] = createSignal(""), [per, setPer] = createSignal<"weekly" | "monthly">("monthly");
  const [selDocs, setSelDocs] = createSignal<VDoc[]>([]), [selSubs, setSelSubs] = createSignal<Submission[]>([]);
  const rowOf = (id: number) => me()?.docs.find(x => x.id === id);
  const docs = createMemo(() => { const t = q().toLowerCase(); return reg().docs.filter(d => !t || `${d.number} ${d.title} ${d.vendor} ${d.package} ${d.disc} ${d.kind} ${d.remarks}`.toLowerCase().includes(t)); });
  // ---- register columns ----
  const hidden = () => (P().ui?.docHidden as string[] | undefined) ?? ["key", "subs", "purpose", "returned", "code", "task", "weight", "progress", "recurring", ...FLAGS.map(f => "f:" + f)];
  const setHidden = (h: string[]) => act({ Ui: { key: "docHidden", value: h } });
  const calc = (key: string, name: string, width: number, get: (r: DocRow) => string, more: Partial<Col<VDoc>> = {}): Col<VDoc> => ({ key, name, width, kind: "calc", get: d => { const r = rowOf(d.id); return r ? get(r) : ""; }, ...more });
  const tasks = () => grid().rows.filter(t => t.uid > 0);
  const allCols = createMemo((): Col<VDoc>[] => {
    const cls = classesOf(reg());
    const list = (key: string, name: string, width: number, field: string): Col<VDoc> => ({ key, name, width, kind: "list", get: d => fieldOf(d, field), options: () => listOpts(field), editList: () => docSettings("lists") });
    return [
      { key: "number", name: "Document No.", width: 170, kind: "text", get: d => d.number, cls: () => "mono" },
      { key: "title", name: "Title", width: 270, kind: "text", get: d => d.title },
      list("vendor", "Vendor", 130, "vendor"), list("package", "Package", 110, "package"), list("disc", "Disc.", 64, "disc"), list("kind", "Type", 100, "kind"),
      { key: "class", name: "Class", width: 58, kind: "list", strict: true, get: d => cls[Math.max(1, d.class) - 1]?.name ?? String(d.class), options: () => cls.map(c => ({ value: c.name, label: `${c.steps.length} steps` })), editList: () => docSettings("classes") },
      { key: "state", name: "State", width: 74, kind: "list", get: d => d.state || "Active", options: () => listOpts("state"), editList: () => docSettings("lists") },
      { key: "key", name: "Key", width: 44, kind: "check", get: d => (d.key ? "Yes" : "") },
      { key: "offset", name: "P +", width: 76, kind: "text", get: d => (d.first ? dt(d.first) : d.offset !== null && d.offset !== undefined ? String(d.offset) : ""), tip: `First issue at P + n ${reg().p_unit}, or a date of its own` },
      calc("first_planned", "1st planned", 82, r => r.planned[0] ?? "", { date: true }),
      calc("first_sent", "1st sent", 78, r => r.actual[0] ?? "", { date: true, cls: d => ((rowOf(d.id)?.late1 ?? 0) > 0 && rowOf(d.id)?.actual[0] ? "late" : "") }),
      calc("subs", "Subs", 46, r => (r.subs ? String(r.subs) : "")),
      calc("rev", "Rev", 44, r => r.rev), calc("purpose", "Purpose", 70, r => r.purpose),
      calc("sent", "Sent", 78, r => r.sent ?? "", { date: true }), calc("due", "Review due", 82, r => r.due ?? "", { date: true }),
      calc("returned", "Returned", 78, r => r.returned ?? "", { date: true }), calc("code", "Code", 76, r => r.code),
      calc("status", "Status", 168, r => r.status, { show: d => { const r = rowOf(d.id); return r ? <><span class={`st ${SCOL[r.skey] ?? ""}`}>{r.status}</span>{r.late_for_task && <span class="st bad" title={`Needed by ${dt(r.need)} for ${r.task_name}`}> late for task</span>}</> : ""; } }),
      calc("earned", "Earned", 62, r => pct(r.earned), { show: d => (d.recurring || !d.progress || rowOf(d.id)?.skey === "off" ? <span class="muted">—</span> : pct(rowOf(d.id)?.earned ?? 0)), cls: () => "num" }),
      calc("client_late", "Client late", 60, r => (r.client_late ? String(r.client_late) : ""), { cls: d => ((rowOf(d.id)?.client_late ?? 0) > 0 ? "late num" : "num"), tip: "Days the client kept it past the review period" }),
      calc("final", "Final (fc)", 82, r => r.finish ?? "", { date: true }),
      { key: "task", name: "Needed for task", width: 150, kind: "list", strict: true, get: d => String(tasks().find(t => t.uid === d.task)?.id ?? ""), show: d => { const t = tasks().find(t => t.uid === d.task); return t ? `${t.id} ${t.name}` : ""; }, options: () => tasks().map(t => ({ value: String(t.id), label: t.name })) },
      { key: "weight", name: "Weight", width: 58, kind: "number", get: d => String(d.weight) },
      { key: "progress", name: "Counted", width: 60, kind: "check", get: d => (d.progress ? "Yes" : "") },
      { key: "recurring", name: "Recurring", width: 66, kind: "check", get: d => (d.recurring ? "Yes" : "") },
      ...FLAGS.map((f): Col<VDoc> => ({ key: "f:" + f, name: f, width: 54, kind: "check", get: d => (d.flags.includes(f) ? "Yes" : "") })),
      { key: "remarks", name: "Remarks", width: 200, kind: "text", get: d => d.remarks },
      ...(reg().columns ?? []).map((c, i): Col<VDoc> => c.kind === "formula" ? calc("own:" + c.name, c.name, 110, r => r.own?.[i] ?? "", { tip: c.formula })
        : { key: "own:" + c.name, name: c.name, width: 110, kind: (c.kind === "list" ? "list" : c.kind === "date" ? "date" : c.kind === "number" ? "number" : "text"), get: d => d.own?.[c.name] ?? "", ...(c.kind === "list" ? { options: () => listOpts(c.name), editList: () => docSettings("lists") } : {}) }),
    ];
  });
  const cols = () => allCols().filter(c => !hidden().includes(c.key));
  /** One change to a document from the grid. */
  const apply = (d: VDoc, key: string, v: string): VDoc => {
    const cls = classesOf(reg());
    switch (key) {
      case "number": case "title": case "vendor": case "package": case "disc": case "kind": case "remarks": return { ...d, [key]: v };
      case "state": return { ...d, state: /^active$/i.test(v) ? "" : v };
      case "class": { const i = cls.findIndex(c => c.name.toLowerCase() === v.toLowerCase()); return i >= 0 ? { ...d, class: i + 1 } : d; }
      case "key": case "progress": case "recurring": return { ...d, [key]: !!v };
      case "weight": return { ...d, weight: Math.max(0, +v || 0) };
      case "task": return { ...d, task: tasks().find(t => String(t.id) === v)?.uid ?? 0 };
      case "offset": { if (!v.trim()) return { ...d, offset: null, first: null }; const n = v.trim().replace(/^p\s*\+\s*/i, ""); if (/^-?\d+(\.\d+)?$/.test(n)) return { ...d, offset: +n, first: null }; const x = parseDate(v); if (x) return { ...d, first: x, offset: null }; toast(`“${v}” is neither P + n nor a date.`); return d; }
    }
    if (key.startsWith("f:")) { const f = key.slice(2); return { ...d, flags: v ? [...new Set([...d.flags, f])] : d.flags.filter(x => x !== f) }; }
    if (key.startsWith("own:")) return { ...d, own: { ...(d.own ?? {}), [key.slice(4)]: v } };
    return d;
  };
  const setDocs = (e: Edit<VDoc>[]) => { const m = new Map(reg().docs.map(d => [d.id, d])); for (const x of e) { const d = m.get(x.row.id); if (d) m.set(d.id, apply(d, x.key, x.value)); } save({ docs: reg().docs.map(d => m.get(d.id)!) }); };
  const addDocs = (vals: Record<string, string>[]) => { let id = next(reg().docs); const fresh = vals.map(v => Object.entries(v).reduce((d, [k, x]) => apply(d, k, x), blankDoc(id++))); save({ docs: [...reg().docs, ...fresh] }); };
  const removeDocs = async (rows: VDoc[]) => {
    const ids = new Set(rows.map(d => d.id)), subs = reg().log.filter(s => ids.has(s.doc)).length;
    if (rows.length > 1 || subs) { if (!(await askYes("Delete documents", `Delete ${rows.length} document${rows.length > 1 ? "s" : ""}${subs ? ` and their ${subs} submissions` : ""}? Ctrl+Z brings them back.`, "Delete"))) return; }
    save({ docs: reg().docs.filter(d => !ids.has(d.id)), log: reg().log.filter(s => !ids.has(s.doc)) });
  };
  const docMenu = (rows: VDoc[]): Item[] => [
    { label: rows.length > 1 ? `Log submission for ${rows.length} documents…` : "Log submission…", icon: "send", run: () => logMany(rows, () => setTab("log")), off: !rows.length },
    { label: rows.length > 1 ? `Log return for ${rows.length} documents…` : "Log return…", icon: "inbox", run: () => returnMany(rows), off: !rows.length },
    { label: "Open the document card", icon: "info", run: () => openCard(rows[0].id), off: rows.length !== 1 }, { label: "Edit details…", icon: "edit", run: () => editDoc(rows[0]), off: rows.length !== 1 },
  ];
  // ---- the transmittal log ----
  const docOf = (id: number) => reg().docs.find(d => d.id === id);
  const subOut = (id: number) => me()?.subs.find(x => x.id === id);
  const logRows = createMemo(() => { const t = q().toLowerCase(); return [...reg().log].sort((a, b) => (a.sent ?? "9999").localeCompare(b.sent ?? "9999") || a.id - b.id).filter(s => { const d = docOf(s.doc); return !t || `${d?.number} ${d?.title} ${s.transmittal} ${s.ret_transmittal} ${s.code} ${s.crs}`.toLowerCase().includes(t); }); });
  const logCols = (): Col<Submission>[] => [
    { key: "doc", name: "Document No.", width: 180, kind: "list", strict: true, get: s => docOf(s.doc)?.number ?? "", show: s => (docOf(s.doc) ? <span class="mono">{docOf(s.doc)!.number}</span> : <span class="late">!! Not in register</span>), options: () => reg().docs.map(d => ({ value: d.number, label: `${d.title}${rowOf(d.id) ? ` · ${rowOf(d.id)!.status}` : ""}` })) },
    { key: "title", name: "Title", width: 230, kind: "calc", get: s => docOf(s.doc)?.title ?? "" },
    { key: "class", name: "Class", width: 50, kind: "calc", get: s => (docOf(s.doc) ? className(docOf(s.doc)!) : "") },
    { key: "vendor", name: "Vendor", width: 120, kind: "calc", get: s => docOf(s.doc)?.vendor ?? "" },
    { key: "rev", name: "Rev", width: 48, kind: "text", get: s => s.rev },
    { key: "purpose", name: "Purpose", width: 74, kind: "list", get: s => s.purpose, options: () => reg().purposes.map(p => ({ value: p[0], label: p[1] })), editList: () => docSettings("codes") },
    { key: "transmittal", name: "Transmittal", width: 150, kind: "text", get: s => s.transmittal, cls: () => "mono" },
    { key: "sent", name: "Sent", width: 84, kind: "date", get: s => s.sent ?? "" },
    { key: "due", name: "Review due", width: 84, kind: "calc", date: true, get: s => subOut(s.id)?.due ?? "" },
    { key: "ret_transmittal", name: "Return transmittal", width: 150, kind: "text", get: s => s.ret_transmittal, cls: () => "mono" },
    { key: "returned", name: "Returned", width: 84, kind: "date", get: s => s.returned ?? "" },
    { key: "code", name: "Code", width: 92, kind: "list", strict: true, get: s => s.code, options: () => reg().codes.map(c => ({ value: c.code, label: c.meaning })), editList: () => docSettings("codes") },
    { key: "crs", name: "Comments ref.", width: 130, kind: "text", get: s => s.crs },
    { key: "days", name: "Days", width: 50, kind: "calc", get: s => String(subOut(s.id)?.days ?? ""), cls: () => "num", tip: "Days with the client (to today while not returned)" },
    { key: "late", name: "Days late", width: 60, kind: "calc", get: s => (subOut(s.id)?.late ? String(subOut(s.id)!.late) : ""), cls: s => ((subOut(s.id)?.late ?? 0) > 0 ? "late num" : "num") },
    { key: "state", name: "State", width: 120, kind: "calc", get: s => `${subOut(s.id)?.state ?? ""}${subOut(s.id)?.accepted ? " · accepted" : ""}`, show: s => <span class={`st ${subOut(s.id)?.state === "Review overdue" ? "bad" : subOut(s.id)?.accepted ? "good" : ""}`}>{subOut(s.id)?.state}{subOut(s.id)?.accepted ? " · accepted" : ""}</span> },
  ];
  const applySub = (s: Submission, key: string, v: string): Submission | null => {
    if (key === "doc") { const d = reg().docs.find(x => x.number.toLowerCase() === v.trim().toLowerCase()); if (!d) { toast(`“${v}” is not a document number in the register.`); return null; } return { ...s, doc: d.id }; }
    if (key === "sent" || key === "returned") return { ...s, [key]: v || null };
    return { ...s, [key]: v };
  };
  const setSubs = (e: Edit<Submission>[]) => { const m = new Map(reg().log.map(s => [s.id, s])); for (const x of e) { const s = m.get(x.row.id); const n = s && applySub(s, x.key, x.value); if (n) m.set(n.id, n); } save({ log: reg().log.map(s => m.get(s.id)!) }); };
  const addSubs = (vals: Record<string, string>[]) => {
    const log = [...reg().log]; let bad = 0;
    for (const v of vals) {
      const d = reg().docs.find(x => x.number.toLowerCase() === (v.doc ?? "").trim().toLowerCase());
      if (!d) { bad++; continue; }
      // The document's next submission (its revision, purpose and today's date), with what was typed.
      let s = nextSub(d, log);
      for (const [k, x] of Object.entries(v)) if (k !== "doc") s = applySub(s, k, x) ?? s;
      log.push(s);
    }
    if (bad) toast(bad === vals.length ? "Start a new row with the document number: its title, class and vendor come from the register." : `${bad} rows name documents not in the register; they were left out.`);
    if (log.length > reg().log.length) save({ log });
  };
  const removeSubs = async (rows: Submission[]) => { if (rows.length > 1 && !(await askYes("Delete submissions", `Delete ${rows.length} submissions? Ctrl+Z brings them back.`, "Delete"))) return; const ids = new Set(rows.map(s => s.id)); save({ log: reg().log.filter(s => !ids.has(s.id)) }); };
  const subMenu = (rows: Submission[]): Item[] => [
    { label: "Open the document card", icon: "info", run: () => openCard(rows[0].doc), off: rows.length !== 1 },
    { label: rows[0]?.transmittal ? `Transmittal ${rows[0].transmittal}: cover sheet (PDF)…` : "Transmittal cover sheet", icon: "print", run: () => printTransmittal(rows[0].transmittal), off: !rows[0]?.transmittal },
  ];
  // ---- transmittals, one line each ----
  const transmittals = createMemo(() => {
    const m = new Map<string, Submission[]>();
    for (const s of reg().log) { if (!s.transmittal.trim()) continue; m.set(s.transmittal, [...(m.get(s.transmittal) ?? []), s]); }
    return [...m].sort((a, b) => (b[1][0].sent ?? "").localeCompare(a[1][0].sent ?? ""));
  });
  const count = (k: string) => me()?.docs.filter(d => d.skey === k).length ?? 0;
  const groupBy = (key: (d: VDoc) => string) => {
    const m = new Map<string, { n: number; first: number; fin: number; earned: number; w: number }>();
    for (const d of reg().docs) { const r = rowOf(d.id); if (!r || r.skey === "off") continue; const k = key(d) || "(none)", g = m.get(k) ?? { n: 0, first: 0, fin: 0, earned: 0, w: 0 }; g.n++; if (r.actual[0]) g.first++; if (r.done) g.fin++; const w = d.progress && !d.recurring ? d.weight : 0; g.earned += w * r.earned; g.w += w; m.set(k, g); }
    return [...m].sort((a, b) => b[1].n - a[1].n);
  };
  const Breakdown = (p: { title: string; key: (d: VDoc) => string }) => <div class="panel ctr"><h2>{p.title}</h2><table class="tbl"><thead><tr><th>{p.title.replace("By ", "")}</th><th class="r">Documents</th><th class="r">1st issued</th><th class="r">Final</th><th class="r">Earned</th></tr></thead>
    <tbody><For each={groupBy(p.key)}>{([k, g]) => <tr><td>{k}</td><td class="r">{g.n}</td><td class="r">{g.first}</td><td class="r">{g.fin}</td><td class="r">{g.w ? (g.earned / g.w).toFixed(1) : "—"} %</td></tr>}</For></tbody></table></div>;
  const Plans = () => <Show when={(data()?.length ?? 0) > 1}>
        <div class="panel ctr"><h2>The project's plans</h2>
          <table class="tbl"><thead><tr><th>Plan</th><th class="r">Documents</th><th class="r">1st issued</th><th class="r">Final</th><th class="r">Earned</th><th class="r">Planned</th><th class="r">Client late (d)</th><th /></tr></thead>
            <tbody><For each={data()}>{d => <tr><td><b>{d.name}</b>{d.tab === out()?.tab ? " (this plan)" : ""}{!d.on && <span class="muted"> · module off</span>}</td><td class="r">{d.out.docs.length}</td><td class="r">{d.out.docs.filter(x => x.actual[0]).length}</td><td class="r">{d.out.docs.filter(x => x.done).length}</td>
              <td class="r">{d.out.earned.toFixed(1)} %</td><td class="r">{d.out.planned.toFixed(1)} %</td><td class="r">{d.out.client_late}</td>
              <td>{d.tab !== out()?.tab && <button class="btn sm" onClick={() => showTab(d.tab).then(() => setView("documents"))}>Open</button>}</td></tr>}</For></tbody></table>
        </div>
      </Show>;
  const legend = <span class="rglegend"><span><i />you type</span><span><i class="calc" />the app works it out</span><span>▾ lists: Settings › Lists</span></span>;
  const columnsMenu = (e: MouseEvent) => openMenu(e, [...allCols().map(c => ({ label: c.name, on: !hidden().includes(c.key), run: () => setHidden(hidden().includes(c.key) ? hidden().filter(x => x !== c.key) : [...hidden(), c.key]) })), "-" as const, { label: "Your own columns…", run: () => docSettings("columns") }]);
  return (
    <RegBook title="Document Register" help="documents" tab={tab()} set={setTab} grids={["register", "log"]}
      tabs={[["dashboard", "Dashboard"], ["register", "Register"], ["log", "Transmittal log"], ["trans", "Transmittals"]]} tools={<>
      <button class="btn" onClick={() => docSettings()}>Settings…</button><button class="btn" onClick={docImport}>Import…</button>
      <button class="btn" onClick={async () => { const f = await call<string | null>("docs_export"); if (f) toast(`Saved ${f}`, true); }} title="A workbook (.xlsx) or an OpenDocument spreadsheet (.ods): choose the type in the Save window">Export…</button>
    </>}>
      <Show when={me()}>{o => <>
        <Show when={!o().weights_ok.every(Boolean)}><p class="dwarn">The progress weights of a class do not add up to 100 %: Settings › Classes & steps.</p></Show>
        <Show when={tab() === "dashboard"}>
        <div class="kpis">
          <Kpi v={String(o().docs.filter(d => d.skey !== "off").length)} l={`documents · ${reg().log.length} submissions`} />
          <Kpi v={`${o().docs.filter(d => d.actual[0]).length} / ${o().docs.filter(d => d.planned[0] && d.skey !== "off").length}`} l="first issues sent / planned" />
          <Kpi v={String(count("final"))} l="final issues" good={o().docs.length > 0 && o().docs.every(d => d.done || d.skey === "off")} />
          <Kpi v={`${count("review") + count("review_late")} · ${count("review_late")}`} l="with the client · overdue" bad={count("review_late") > 0} />
          <Kpi v={String(count("resubmit"))} l="to resubmit" bad={count("resubmit") > 0} />
          <Kpi v={String(count("not_sub_late"))} l="first issues overdue" bad={count("not_sub_late") > 0} />
          <Kpi v={`${o().earned.toFixed(1)} %`} l={`earned · planned ${o().planned.toFixed(1)} %`} bad={o().earned < o().planned - 0.05} good={o().earned >= o().planned && o().planned > 0} />
          <Kpi v={`${o().client_late} d`} l="the client kept documents past the review period" />
        </div>
        <div class="cols3">
          <Attention title="First issues overdue" empty="None: every first issue due is sent." more={() => setTab("register")}
            rows={o().docs.filter(d => d.skey === "not_sub_late").map(d => ({ key: docOf(d.id)?.number ?? "", text: docOf(d.id)?.title ?? "", note: d.late1 ? `${d.late1} d late` : d.status, bad: true, open: () => openCard(d.id) }))} />
          <Attention title="With the client past the review period" empty="None overdue." more={() => setTab("log")}
            rows={o().docs.filter(d => d.skey === "review_late").map(d => ({ key: docOf(d.id)?.number ?? "", text: docOf(d.id)?.title ?? "", note: d.status, bad: true, open: () => openCard(d.id) }))} />
          <Attention title="To resubmit" empty="Nothing returned for resubmission." more={() => setTab("register")}
            rows={o().docs.filter(d => d.skey === "resubmit").map(d => ({ key: docOf(d.id)?.number ?? "", text: docOf(d.id)?.title ?? "", note: d.status, open: () => openCard(d.id) }))} />
        </div>
        <div class="phead"><h2>Progress</h2><span class="grow" /><Seg value={per()} set={v => setPer(v as "monthly")} options={[["weekly", "Weekly"], ["monthly", "Monthly"]]} /></div>
        <Show when={o()[per()].length > 1} fallback={<p class="empty">Give the documents planned dates (P + n) to see the S-curve.</p>}>
          <div class="cols2">
            <ChartBox title="Document progress (weighted)" type="line" fmt="pct" max={100} labels={o()[per()].map(p => dt(p.end))} days={o()[per()].map(p => Math.round(Date.parse(p.end) / 864e5))}
              datasets={[line("Planned", o()[per()].map(p => p.planned), COL.grey, [5, 4]), line("Actual", o()[per()].map(p => p.actual), COL.blue)]} />
            <ChartBox title="First issues per period" type="bar" labels={o()[per()].map(p => dt(p.end))} datasets={[bar("Planned", o()[per()].map(p => p.first_planned), COL.grey), bar("Sent", o()[per()].map(p => p.first_done), COL.green)]} />
          </div>
        </Show>
        <div class="cols2"><Breakdown title="By vendor" key={d => d.vendor} /><Breakdown title="By discipline" key={d => d.disc} /><Breakdown title="By document type" key={d => d.kind} /><Breakdown title="By class" key={d => `Class ${className(d)}`} /></div>
        <Plans />
        </Show>
        <Show when={tab() === "register"}>
          <div class="rgbar">
            <button class="btn pri" onClick={() => addDocs([{ title: "New document" }])} title="Or type in the empty row at the foot of the register">+ Document</button>
            <button class="btn" disabled={!selDocs().length} onClick={() => logMany(selDocs(), () => setTab("log"))}>Log submission{selDocs().length > 1 ? ` (${selDocs().length})` : ""}</button>
            <button class="btn" disabled={!selDocs().length} onClick={() => returnMany(selDocs())}>Log return{selDocs().length > 1 ? ` (${selDocs().length})` : ""}</button>
            <button class="btn" disabled={selDocs().length !== 1} onClick={() => openCard(selDocs()[0].id)}>Card</button>
            <span class="sep" /><button class="btn danger" disabled={!selDocs().length} onClick={() => removeDocs(selDocs())}>Delete{selDocs().length > 1 ? ` (${selDocs().length})` : ""}</button>
            <span class="sep" /><input class="q" placeholder="Search number, title, vendor…" value={q()} onInput={e => setQ(e.currentTarget.value)} />
            <button class="btn" onClick={columnsMenu}>Columns ▾</button>{legend}
          </div>
          <RegGrid id="docs" rows={docs()} cols={cols()} rowId={d => d.id} set={setDocs} add={addDocs} remove={removeDocs} open={d => openCard(d.id)} menu={docMenu} selected={setSelDocs}
            newHint="Type a document number here to add one; paste rows from a workbook" rowCls={d => (rowOf(d.id)?.skey === "off" ? "off" : "")} />
          <Show when={!reg().docs.length}><p class="empty">No documents yet. Type in the first row, paste rows from your workbook, or <b>Import…</b> the vendor document register. Set the P date, review days, classes and lists in <b>Settings…</b>.</p></Show>
        </Show>
        <Show when={tab() === "log"}>
          <div class="rgbar">
            <button class="btn pri" onClick={() => toast("Type the document number in the empty row at the foot of the log: title, class and vendor fill in, and the revision and purpose are proposed.", true)}>+ Submission</button>
            <button class="btn" disabled={!selSubs().length} onClick={() => openCard(selSubs()[0].doc)}>Card</button>
            <button class="btn" disabled={!selSubs()[0]?.transmittal} onClick={() => printTransmittal(selSubs()[0].transmittal)}>Transmittal sheet…</button>
            <span class="sep" /><button class="btn danger" disabled={!selSubs().length} onClick={() => removeSubs(selSubs())}>Delete{selSubs().length > 1 ? ` (${selSubs().length})` : ""}</button>
            <span class="sep" /><input class="q" placeholder="Search number, transmittal, code…" value={q()} onInput={e => setQ(e.currentTarget.value)} />{legend}
          </div>
          <RegGrid id="log" rows={logRows()} cols={logCols()} rowId={s => s.id} set={setSubs} add={addSubs} remove={removeSubs} open={s => openCard(s.doc)} menu={subMenu} selected={setSelSubs}
            newHint="Type or pick a document number to log its next submission" />
        </Show>
        <Show when={tab() === "trans"}>
          <div class="panel ctr">
            <table class="tbl"><thead><tr><th>Transmittal</th><th>Sent</th><th class="r">Documents</th><th class="r">Returned</th><th>Codes</th><th>Documents</th><th /></tr></thead>
              <tbody><For each={transmittals()} fallback={<tr><td colspan="7" class="muted">No transmittal numbers in the log yet.</td></tr>}>{([no, subs]) => <tr>
                <td class="mono"><b>{no}</b></td><td>{dt(subs[0].sent)}</td><td class="r">{subs.length}</td><td class="r">{subs.filter(s => s.returned).length}</td>
                <td>{[...new Set(subs.map(s => s.code).filter(Boolean))].join(", ")}</td>
                <td class="muted" style={{ "max-width": "420px", overflow: "hidden", "text-overflow": "ellipsis", "white-space": "nowrap" }}>{subs.map(s => docOf(s.doc)?.number).join(", ")}</td>
                <td><button class="btn sm" onClick={() => printTransmittal(no)}>Cover sheet…</button></td></tr>}</For></tbody></table>
          </div>
        </Show>
        <Card o={o()} />
      </>}</Show>
      <Show when={!me()}><Plans /></Show>
    </RegBook>
  );
};
