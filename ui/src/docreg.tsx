// The Document Register module: vendor documents, the transmittal log and the rules of the review
// cycle. The engine works out every date, status and percentage (docreg.rs); this page shows
// them, edits the register, and brings documents and submissions in from a spreadsheet.
import { For, Show, createMemo, createResource, createSignal } from "solid-js";
import { DocClass, DocRegister, DocStep, Options, RegOut, Submission, VDoc } from "./api";
import { act, call, grid, out, P, tick, setView, openDialog, toast, opts, setOpts, openMenu } from "./store";
import { Page } from "./views";
import { Kpi } from "./reports";
import { ChartBox, COL, bar, line } from "./charts";
import { Dialog, Seg, askText } from "./ui";
import { showTab } from "./plans";
import * as D from "./dialogs";

const FLAGS = ["MRB", "VDB", "IOM", "PIM", "Native", "Eng. review", "QA review"];
const ST: Record<string, string> = { "Final issued": "good", "Accepted – next issue": "good", "Under review": "", "Review overdue": "bad", Resubmit: "warn", "Not submitted – overdue": "bad", "Not submitted": "" };
const DEF: DocRegister = {
  p_date: null, p_unit: "weeks", review_days: 10, turnaround_days: 10, review_cal: "", work_cal: "",
  weights: [[35, 35, 60], [15, 15, 20], [10, 10, 0], [10, 10, 0], [20, 20, 0], [10, 10, 20]],
  codes: [{ code: "Code 1", meaning: "Approved – proceed", accepted: true, resubmit: false }, { code: "Code 2", meaning: "Approved with comments – proceed with the next revision", accepted: true, resubmit: false },
    { code: "Code 3", meaning: "Revise and resubmit", accepted: false, resubmit: true }, { code: "Code 4", meaning: "Retained for information", accepted: true, resubmit: false },
    { code: "Code 5", meaning: "Rejected", accepted: false, resubmit: true }, { code: "No Comment", meaning: "No reply within the review period (deemed accepted)", accepted: true, resubmit: false }],
  purposes: [["ICR", "Issued for client review"], ["IFA", "Issued for approval"], ["IFI", "Issued for information"], ["IFD", "Issued for design"], ["IFP", "Issued for purchase"], ["IFH", "Issued for HAZOP"], ["IFC1", "Issued for construction"], ["IFC2", "Issued final"], ["RED-LINE", "Red-line mark-up"], ["AS-BUILT", "As built"]],
  flow: { first: "ICR", approval: "IFA", construction: "IFC1", final: "IFC2" }, docs: [], log: [],
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
const blankDoc = (id: number): VDoc => ({ id, number: "", title: "New document", vendor: "", package: "", disc: "", kind: "", class: 1, key: false, flags: [], offset: null, first: null, weight: 1, progress: true, recurring: false, task: 0, remarks: "" });
const next = (l: { id: number }[]) => Math.max(0, ...l.map(x => x.id)) + 1;
const dt = (v: string | null | undefined) => (v ? new Date(v + "T00:00").toLocaleDateString("en-GB", { day: "2-digit", month: "short", year: "2-digit" }) : "");
const today = () => new Date().toISOString().slice(0, 10);

/** The next submission of a document: the next revision letter and the purpose its cycle needs. */
const nextSub = (d: VDoc): Submission => {
  const r = reg(), log = r.log.filter(s => s.doc === d.id), last = log[log.length - 1];
  const rev = !last ? "A" : /^[A-Y]$/.test(last.rev) ? String.fromCharCode(last.rev.charCodeAt(0) + 1) : /^\d+$/.test(last.rev) ? String(+last.rev + 1) : last.rev;
  const code = r.codes.find(c => c.code === last?.code);
  // The purpose the cycle needs next: the same again when it is with the client or sent back, else
  // the first issue step of the class that nothing has been sent for yet.
  const steps = classesOf(r)[Math.max(1, d.class) - 1]?.steps ?? [];
  const sentFor = (s: DocStep) => log.some(x => x.sent && purposesOf(s).some(p => p.toUpperCase() === x.purpose.toUpperCase()));
  const nextIssue = steps.find(s => !s.returned && !sentFor(s));
  const purpose = last && (code?.resubmit || !last.returned) ? last.purpose : (nextIssue ? purposesOf(nextIssue)[0] : undefined) ?? last?.purpose ?? r.flow.first;
  return { id: next(r.log), doc: d.id, rev, purpose, transmittal: "", sent: today(), ret_transmittal: "", returned: null, code: "", crs: "" };
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
      <select disabled={!own()} value={p.value} onChange={e => p.set(e.currentTarget.value)}><For each={P().calendars}>{c => <option>{c.name}</option>}</For></select></label>
    <span class="dcalb"><button class="btn sm" onClick={make}>New calendar…</button><button class="btn sm" disabled={!own()} onClick={() => D.calendars(p.value)}>Working time…</button></span>
  </div>;
};

export const docSettings = () => openDialog(() => {
  const start = JSON.parse(JSON.stringify(reg())) as DocRegister;
  const [r, setR] = createSignal<DocRegister>({ ...start, classes: classesOf(start) });
  const put = (x: Partial<DocRegister>) => setR({ ...r(), ...x });
  const cls = () => r().classes ?? [];
  const purposes = () => r().purposes.map(p => p[0]);
  const total = (k: number) => cls()[k]?.steps.reduce((a, s) => a + (s.weight || 0), 0) ?? 0;
  const setC = (k: number, x: Partial<DocClass>) => put({ classes: cls().map((c, j) => (j === k ? { ...c, ...x } : c)) });
  const setS = (k: number, i: number, x: Partial<DocStep>) => setC(k, { steps: cls()[k].steps.map((s, j) => (j === i ? { ...s, ...x } : s)) });
  const move = (k: number, i: number, d: number) => { const v = [...cls()[k].steps], j = i + d; if (j < 0 || j >= v.length) return; [v[i], v[j]] = [v[j], v[i]]; setC(k, { steps: v }); };
  // Templates: a client's rules kept by name, for any plan.
  const tpls = () => opts()?.docTemplates ?? {};
  const RULES = ["p_unit", "review_days", "turnaround_days", "codes", "purposes", "flow", "classes"] as const;
  const saveTpl = async () => {
    const name = (await askText("Save as Template", "Template name (for example the client's name)", ""))?.trim(); if (!name) return;
    const t = Object.fromEntries(RULES.map(k => [k, r()[k]])) as Partial<DocRegister>;
    const o = { ...opts()!, docTemplates: { ...tpls(), [name]: t } } as Options;
    await call("set_options", { opts: o }); setOpts(o); toast(`Template “${name}” saved: Settings › Templates gives these rules to any plan's register.`, true);
  };
  const useTpl = (name: string) => { const t = tpls()[name]; if (t) setR({ ...r(), ...JSON.parse(JSON.stringify(t)) }); };
  const delTpl = async (name: string) => { const m = { ...tpls() }; delete m[name]; const o = { ...opts()!, docTemplates: m } as Options; await call("set_options", { opts: o }); setOpts(o); };
  return (
    <Dialog title="Document Register Settings" wide help="documents" okText="Save" ok={() => { const bad = cls().findIndex((_, k) => Math.abs(total(k) - 100) > 0.01); if (bad >= 0) { toast(`The step weights of class ${cls()[bad].name} must add up to 100 %.`); return false; } save(r()); }}
      extra={<div class="inline"><button class="btn sm" onClick={e => openMenu(e, Object.keys(tpls()).length ? Object.keys(tpls()).map(n => ({ label: n, run: () => useTpl(n), sub: undefined })) : [{ label: "No templates yet: Save as Template first", off: true }])}>Use a template ▾</button>
        <button class="btn sm" onClick={saveTpl}>Save as Template…</button>
        <Show when={Object.keys(tpls()).length}><button class="btn sm" onClick={e => openMenu(e, Object.keys(tpls()).map(n => ({ label: `Delete “${n}”`, run: () => delTpl(n) })))}>Delete a template ▾</button></Show></div>}>
      <div class="dset">
        <div>
          <h3>Planned dates</h3>
          <div class="prow"><label>P date (purchase order or kick-off)<input type="date" value={r().p_date ?? ""} placeholder={P().start} onChange={e => put({ p_date: e.currentTarget.value || null })} /></label>
            <label>P + n counts in<select value={r().p_unit} onChange={e => put({ p_unit: e.currentTarget.value })}><option value="weeks">weeks</option><option value="days">days</option></select></label></div>
          <p class="muted">Empty: the plan's start ({dt(P().start)}). Each document's first issue is P + n, or a date of its own.</p>
          <div class="prow"><label>Client review (working days)<input type="number" min="0" value={r().review_days} onChange={e => put({ review_days: +e.currentTarget.value || 0 })} /></label>
            <label>Our turnaround (working days)<input type="number" min="0" value={r().turnaround_days} onChange={e => put({ turnaround_days: +e.currentTarget.value || 0 })} /></label></div>
          <CalChoice label="Client review days count on" value={r().review_cal} set={v => put({ review_cal: v })} />
          <CalChoice label="Our turnaround days count on" value={r().work_cal} set={v => put({ work_cal: v })} />
          <h3>Which issues mean what</h3>
          <p class="muted">Orders wait for a document's approval issue to be accepted; a task waits for its construction issue. The next submission of a document takes the next issue of its class.</p>
          <div class="prow">
            {(["first", "approval", "construction", "final"] as const).map(k => <label>{{ first: "First issue", approval: "Approval issue", construction: "Construction issue", final: "Final issue" }[k]}<select value={r().flow[k]} onChange={e => put({ flow: { ...r().flow, [k]: e.currentTarget.value } })}><For each={purposes()}>{x => <option>{x}</option>}</For></select></label>)}
          </div>
        </div>
        <div>
          <h3>Classes and their steps</h3>
          <p class="muted">Each class has its own steps. A step counts when an issue with one of its purposes is <b>sent</b>, or when it <b>comes back</b> with a code that accepts it (or one of the codes you name, such as Code 1, Code 4). Its weight is the % of the document earned there. Days: from the step before (empty: the review period before a return, the turnaround after one).</p>
          <For each={cls()}>{(c, k) => <fieldset class="dcls"><legend><input class="dcname" value={c.name} title="The class's name, as the register writes it (1, 2, A, Approval…)" onChange={e => setC(k(), { name: e.currentTarget.value })} />
            <span classList={{ bad: Math.abs(total(k()) - 100) > 0.01 }}>Total {total(k())} %</span><button class="x" title="Remove the class" disabled={cls().length < 2} onClick={() => put({ classes: cls().filter((_, j) => j !== k()) })}>×</button></legend>
            <table class="tbl dsteps"><thead><tr><th>Step</th><th>Issue purposes</th><th>Counts when</th><th>Codes that count</th><th>Weight %</th><th>Days</th><th /></tr></thead>
              <tbody><For each={c.steps}>{(s, i) => <tr>
                <td><input value={s.name} onChange={e => setS(k(), i(), { name: e.currentTarget.value })} /></td>
                <td><input list="dpurposes" style={{ width: "92px" }} value={s.purposes} placeholder="ICR, IFC" onChange={e => setS(k(), i(), { purposes: e.currentTarget.value })} /></td>
                <td><select value={s.returned ? "1" : ""} onChange={e => setS(k(), i(), { returned: !!e.currentTarget.value })}><option value="">sent</option><option value="1">comes back</option></select></td>
                <td><input style={{ width: "110px" }} disabled={!s.returned} value={s.codes} placeholder={s.returned ? "any that accepts" : ""} onChange={e => setS(k(), i(), { codes: e.currentTarget.value })} /></td>
                <td><input type="number" min="0" max="100" style={{ width: "56px" }} value={s.weight} onChange={e => setS(k(), i(), { weight: +e.currentTarget.value || 0 })} /></td>
                <td><input type="number" min="0" style={{ width: "50px" }} value={s.days ?? ""} placeholder="auto" onChange={e => setS(k(), i(), { days: e.currentTarget.value === "" ? null : Math.max(0, +e.currentTarget.value) })} /></td>
                <td class="nowrap"><button class="x" title="Up" onClick={() => move(k(), i(), -1)}>↑</button><button class="x" title="Down" onClick={() => move(k(), i(), 1)}>↓</button><button class="x" title="Remove the step" onClick={() => setC(k(), { steps: c.steps.filter((_, j) => j !== i()) })}>×</button></td></tr>}</For></tbody></table>
            <button class="btn sm" onClick={() => setC(k(), { steps: [...c.steps, { name: "New step", purposes: "", returned: false, codes: "", weight: 0, days: null }] })}>+ Step</button></fieldset>}</For>
          <datalist id="dpurposes"><For each={purposes()}>{p => <option value={p} />}</For></datalist>
          <div class="inline"><button class="btn sm" onClick={() => put({ classes: [...cls(), { name: String(cls().length + 1), steps: JSON.parse(JSON.stringify(cls()[cls().length - 1]?.steps ?? [])) }] })}>+ Class</button>
            <button class="btn sm" onClick={() => put({ classes: classesOf({ ...r(), classes: [] }) })}>Back to the standard classes</button></div>
          <h3>Return codes</h3>
          <p class="muted">Each client's codes mean what you say here: tick <b>Accepted</b> for a code that lets the document go on, <b>Resubmit</b> for one that sends it back (even if that is Code 1).</p>
          <table class="tbl"><thead><tr><th>Code</th><th>Meaning</th><th title="Counts as accepted">Accepted</th><th title="Asks for the document again">Resubmit</th><th /></tr></thead>
            <tbody><For each={r().codes}>{(c, i) => { const set = (x: Partial<typeof c>) => put({ codes: r().codes.map((y, j) => (j === i() ? { ...y, ...x } : y)) }); return <tr>
              <td><input style={{ width: "90px" }} value={c.code} onChange={e => set({ code: e.currentTarget.value })} /></td><td><input value={c.meaning} onChange={e => set({ meaning: e.currentTarget.value })} /></td>
              <td><input type="checkbox" checked={c.accepted} onChange={e => set({ accepted: e.currentTarget.checked, resubmit: e.currentTarget.checked ? false : c.resubmit })} /></td>
              <td><input type="checkbox" checked={c.resubmit} onChange={e => set({ resubmit: e.currentTarget.checked, accepted: e.currentTarget.checked ? false : c.accepted })} /></td>
              <td><button class="x" onClick={() => put({ codes: r().codes.filter((_, j) => j !== i()) })}>×</button></td></tr>; }}</For></tbody></table>
          <button class="btn sm" onClick={() => put({ codes: [...r().codes, { code: "New code", meaning: "", accepted: false, resubmit: false }] })}>+ Code</button>
          <h3>Issue purposes</h3>
          <textarea rows="5" value={r().purposes.map(p => `${p[0]} = ${p[1]}`).join("\n")} onChange={e => put({ purposes: e.currentTarget.value.split("\n").map(l => l.split("=").map(x => x.trim())).filter(x => x[0]).map(x => [x[0], x[1] ?? ""] as [string, string]) })} />
          <p class="muted">One per line: code = meaning.</p>
        </div>
      </div>
    </Dialog>
  );
});

// ---------- A document ----------
const editDoc = (d: VDoc) => openDialog(() => {
  const [x, setX] = createSignal<VDoc>({ ...d, flags: [...d.flags] });
  const put = (v: Partial<VDoc>) => setX({ ...x(), ...v });
  const tasks = () => grid().rows.filter(t => t.uid > 0);
  const T = (p: { l: string; k: "number" | "title" | "vendor" | "package" | "disc" | "kind" | "remarks" }) => <label>{p.l}<input value={x()[p.k]} onChange={e => put({ [p.k]: e.currentTarget.value } as Partial<VDoc>)} /></label>;
  return (
    <Dialog title={`Document ${x().number || ""}`} wide help="documents" okText="Save" ok={() => save({ docs: reg().docs.map(y => (y.id === d.id ? x() : y)) })}>
      <div class="dform">
        <T l="Document number" k="number" /><T l="Title" k="title" /><T l="Vendor" k="vendor" /><T l="Package / sub-system" k="package" /><T l="Discipline" k="disc" /><T l="Document type" k="kind" />
        <label>Class<select value={x().class} onChange={e => put({ class: +e.currentTarget.value })}><For each={classesOf(reg())}>{(c, i) => <option value={i() + 1}>{c.name}</option>}</For></select></label>
        <label>First issue at P +<input type="number" min="0" step="1" value={x().offset ?? ""} onChange={e => put({ offset: e.currentTarget.value === "" ? null : +e.currentTarget.value })} /></label>
        <label>…or on this date<input type="date" value={x().first ?? ""} onChange={e => put({ first: e.currentTarget.value || null })} /></label>
        <label>Weight<input type="number" min="0" step="0.5" value={x().weight} onChange={e => put({ weight: +e.currentTarget.value || 0 })} /></label>
        <label>Needed for task<select value={String(x().task)} onChange={e => put({ task: +e.currentTarget.value })}><option value="0">—</option><For each={tasks()}>{t => <option value={String(t.uid)}>{t.id} · {t.name}</option>}</For></select></label>
        <T l="Remarks / action" k="remarks" />
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

// ---------- Import from a spreadsheet ----------
type Field = [string, string, RegExp];
const DOC_FIELDS: Field[] = [["number", "Document number", /doc.*(no|num)|document no|number/i], ["state", "Row state (deleted, void or cancelled rows are left out)", /doc.*state|^state$|row state/i], ["title", "Title", /title|description/i], ["vendor", "Vendor", /vendor|supplier/i], ["package", "Package / sub-system", /package|sub.?system/i],
  ["disc", "Discipline", /disc/i], ["kind", "Document type", /type/i], ["class", "Class", /class/i], ["first", "First issue date", /first issue date|planned date|\(date\)/i], ["offset", "First issue (P + n)", /planned sub|1st planned|p \+|offset/i],
  ["progress", "Counted in progress (Yes)", /incl.*progress|counted in progress|in progress/i],
  ["key", "Key document (Yes)", /key/i], ["weight", "Weight", /weight/i], ["remarks", "Remarks", /remark|comment/i], ...FLAGS.map(f => ["flag:" + f, `${f} (Yes)`, f === "Eng. review" ? /eng.*review|review.*eng/i : f === "QA review" ? /\bqa\b.*review|review.*(qual|\bqa\b)/i : new RegExp(`\\b${f}\\b`, "i")] as Field)];
const LOG_FIELDS: Field[] = [["number", "Document number", /doc.*(no|num)|document no/i], ["rev", "Revision", /^rev/i], ["purpose", "Issue purpose", /purpose/i], ["transmittal", "Transmittal no.", /^transmittal|transmittal no/i],
  ["sent", "Date sent", /submission date|sent|submitted/i], ["ret_transmittal", "Return transmittal no.", /return transmittal/i], ["returned", "Date returned", /return date|returned/i], ["code", "Return code", /code/i], ["crs", "Comments ref.", /crs|remark/i]];
export const yes = (v: string) => /^(y|yes|true|x|✓|1)$/i.test(v.trim());
export const asDate = (v: string) => { const t = v.trim(); if (/^\d{4}-\d{2}-\d{2}/.test(t)) return t.slice(0, 10); const n = Date.parse(t); return isNaN(n) ? null : new Date(n - new Date(n).getTimezoneOffset() * 6e4).toISOString().slice(0, 10); };

export const docImport = async () => {
  const first = await call<{ path: string; names: string[]; rows: string[][] } | null>("docs_read", {});
  if (!first) return;
  openDialog(() => {
    const [data, setData] = createSignal(first), [kind, setKind] = createSignal<"docs" | "log">("docs"), [mode, setMode] = createSignal<"add" | "update">("update");
    // The heading row: of the first 30, the one where the most cells look like headings.
    const guessHead = (rows: string[][]) => { const sc = rows.slice(0, 30).map(r => r.filter(c => c.length < 60 && [...DOC_FIELDS, ...LOG_FIELDS].some(([, , re]) => re.test(c))).length); return Math.max(0, sc.indexOf(Math.max(...sc))); };
    const [head, setHead] = createSignal(guessHead(first.rows));
    const fields = () => (kind() === "docs" ? DOC_FIELDS : LOG_FIELDS);
    const headers = () => data().rows[head()] ?? [];
    const guess = () => headers().reduce<string[]>((m, h) => { const f = fields().find(([k, , re]) => re.test(h) && !m.includes(k)); return [...m, f ? f[0] : ""]; }, []);
    const [map, setMap] = createSignal<string[]>(guess());
    const body = () => data().rows.slice(head() + 1).filter(r => r.some(c => c.trim()));
    const sheet = async (name: string) => { const d = await call<typeof first>("docs_read", { path: data().path, sheet: name }); if (d) { setData(d); setHead(guessHead(d.rows)); setMap(guess()); } };
    const col = (r: string[], f: string) => { const j = map().indexOf(f); return j >= 0 ? (r[j] ?? "").trim() : ""; };
    const run = () => {
      const R = reg();
      if (!map().includes("number")) { toast("Choose the column with the document number."); return false; }
      if (kind() === "docs") {
        const docs = [...R.docs]; let id = next(docs), added = 0, changed = 0;
        for (const r of body()) {
          const number = col(r, "number"); if (!number || /^(deleted|void|cancel)/i.test(col(r, "state"))) continue;
          const at = mode() === "update" ? docs.findIndex(d => d.number === number) : -1;
          const old = at >= 0 ? docs[at] : blankDoc(id);
          // A date in the P + n column is the first issue's own date; text such as "monthly" marks a recurring submission.
          const offTxt = col(r, "offset"), offDate = /\d{4}-\d{2}-\d{2}|\d{1,2}[/-]\w{2,3}[/-]\d{2,4}/.test(offTxt) ? asDate(offTxt) : null, off = offDate ? null : offTxt.match(/^-?\d+(\.\d+)?$/) ?? offTxt.match(/[+]\s*(\d+(\.\d+)?)/);
          const cname = col(r, "class"), ci = classesOf(R).findIndex(c => c.name.toLowerCase() === cname.toLowerCase());
          const d: VDoc = { ...old, number, title: col(r, "title") || old.title, vendor: col(r, "vendor") || old.vendor, package: col(r, "package") || old.package, disc: col(r, "disc") || old.disc, kind: col(r, "kind") || old.kind,
            class: ci >= 0 ? ci + 1 : +cname >= 1 && +cname <= classesOf(R).length ? +cname : old.class, offset: off ? +(off[1] ?? off[0]) : old.offset, first: asDate(col(r, "first")) ?? offDate ?? old.first, key: map().includes("key") ? yes(col(r, "key")) : old.key,
            progress: map().includes("progress") ? !/^(no|n|false|0)$/i.test(col(r, "progress")) : old.progress,
            weight: col(r, "weight") ? +col(r, "weight") || old.weight : old.weight, remarks: col(r, "remarks") || old.remarks,
            flags: FLAGS.some(f => map().includes("flag:" + f)) ? FLAGS.filter(f => yes(col(r, "flag:" + f))) : old.flags,
            recurring: old.recurring || /mthly|monthly|weekly|recurr/i.test(col(r, "offset")) };
          if (at >= 0) { docs[at] = d; changed++; } else { docs.push(d); id++; added++; }
        }
        save({ docs }); toast(`${added} documents added, ${changed} updated.`, true);
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
        <div class="prow"><label>Bring in<Seg value={kind()} set={v => { setKind(v as "docs"); setMap(guess()); }} options={[["docs", "Documents (the register)"], ["log", "Submissions (transmittal log)"]]} /></label></div>
        <div class="prow"><Show when={data().names.length > 1}><label>Sheet<select onChange={e => sheet(e.currentTarget.value)}><For each={data().names}>{n => <option>{n}</option>}</For></select></label></Show>
          <label>Headings are in row<input type="number" min="1" value={head() + 1} onChange={e => { setHead(Math.max(0, +e.currentTarget.value - 1)); setMap(guess()); }} /></label>
          <label>Rows already in the register<select value={mode()} onChange={e => setMode(e.currentTarget.value as "add")}><option value="update">{kind() === "docs" ? "are updated (same document number)" : "are skipped (same document, revision, purpose and date)"}</option><option value="add">are added again</option></select></label></div>
        <p class="muted">{body().length} rows under the headings. Choose what each column holds; columns left on "—" are not read.</p>
        <div class="dmap"><table class="tbl"><thead><tr><th>Column</th><th>Holds</th><th>First rows</th></tr></thead>
          <tbody><For each={headers()}>{(h, j) => <tr><td><b>{h || `(column ${j() + 1})`}</b></td>
            <td><select value={map()[j()] ?? ""} onChange={e => setMap(headers().map((_, k) => (k === j() ? e.currentTarget.value : (map()[k] === e.currentTarget.value && e.currentTarget.value ? "" : map()[k] ?? ""))))}><option value="">—</option><For each={fields()}>{f => <option value={f[0]}>{f[1]}</option>}</For></select></td>
            <td class="muted">{body().slice(0, 3).map(r => r[j()] ?? "").filter(Boolean).join(" · ").slice(0, 80)}</td></tr>}</For></tbody></table></div>
      </Dialog>
    );
  });
};

// ---------- The view ----------
export const DocumentsView = () => {
  const [data] = createResource(() => [tick(), out()?.tab] as const, () => call<{ tab: number; name: string; on: boolean; out: RegOut }[]>("documents"));
  const me = () => data()?.find(d => d.tab === out()?.tab)?.out;
  const [tab, setTab] = createSignal("register"), [q, setQ] = createSignal(""), [fst, setFst] = createSignal(""), [fven, setFven] = createSignal(""), [per, setPer] = createSignal<"weekly" | "monthly">("monthly");
  const rowOf = (id: number) => me()?.docs.find(x => x.id === id);
  const docs = createMemo(() => reg().docs.filter(d => {
    const r = rowOf(d.id), t = q().toLowerCase();
    return (!t || `${d.number} ${d.title} ${d.vendor} ${d.disc} ${d.kind}`.toLowerCase().includes(t)) && (!fst() || r?.status === fst()) && (!fven() || d.vendor === fven());
  }));
  const putDoc = (d: VDoc) => save({ docs: reg().docs.map(x => (x.id === d.id ? d : x)) });
  const putSub = (s: Submission) => save({ log: reg().log.map(x => (x.id === s.id ? s : x)) });
  const addSub = (d: VDoc) => { save({ log: [...reg().log, nextSub(d)] }); setTab("log"); };
  const vendors = () => [...new Set(reg().docs.map(d => d.vendor).filter(Boolean))].sort();
  const count = (st: string) => me()?.docs.filter(d => d.status === st).length ?? 0;
  const groupBy = (key: (d: VDoc) => string) => {
    const m = new Map<string, { n: number; first: number; fin: number; earned: number; w: number }>();
    for (const d of reg().docs) { const r = rowOf(d.id); if (!r) continue; const k = key(d) || "(none)", g = m.get(k) ?? { n: 0, first: 0, fin: 0, earned: 0, w: 0 }; g.n++; if (r.actual[0]) g.first++; if (r.done) g.fin++; const w = d.progress && !d.recurring ? d.weight : 0; g.earned += w * r.earned; g.w += w; m.set(k, g); }
    return [...m].sort((a, b) => b[1].n - a[1].n);
  };
  const Breakdown = (p: { title: string; key: (d: VDoc) => string }) => <div class="panel ctr"><h2>{p.title}</h2><table class="tbl"><thead><tr><th>{p.title.replace("By ", "")}</th><th class="r">Documents</th><th class="r">1st issued</th><th class="r">Final</th><th class="r">Earned</th></tr></thead>
    <tbody><For each={groupBy(p.key)}>{([k, g]) => <tr><td>{k}</td><td class="r">{g.n}</td><td class="r">{g.first}</td><td class="r">{g.fin}</td><td class="r">{g.w ? (g.earned / g.w).toFixed(1) : "—"} %</td></tr>}</For></tbody></table></div>;
  const purposeOpts = () => reg().purposes.map(p => p[0]);
  return (
    <Page title="Document Register" help="documents" tools={<>
      <button class="btn" onClick={docSettings}>Settings…</button><button class="btn" onClick={docImport}>Import…</button>
      <button class="btn" onClick={async () => { const f = await call<string | null>("docs_export"); if (f) toast(`Saved ${f}`, true); }} title="A workbook (.xlsx) or an OpenDocument spreadsheet (.ods): choose the type in the Save window">Export…</button>
      <button class="btn pri" onClick={() => { const id = next(reg().docs); save({ docs: [...reg().docs, blankDoc(id)] }).then(() => editDoc(reg().docs.find(d => d.id === id) ?? blankDoc(id))); }}>+ Document</button>
    </>}>
      <Show when={me()}>{o => <>
        <div class="kpis">
          <Kpi v={String(reg().docs.length)} l={`documents · ${reg().log.length} submissions`} />
          <Kpi v={`${o().docs.filter(d => d.actual[0]).length} / ${o().docs.filter(d => d.planned[0]).length}`} l="first issues sent / planned" />
          <Kpi v={String(o().docs.filter(d => d.done).length)} l="final issues" good={o().docs.length > 0 && o().docs.every(d => d.done)} />
          <Kpi v={`${count("Under review")} · ${count("Review overdue")}`} l="with the client · overdue" bad={count("Review overdue") > 0} />
          <Kpi v={String(count("Resubmit"))} l="to resubmit" bad={count("Resubmit") > 0} />
          <Kpi v={`${o().earned.toFixed(1)} %`} l={`earned · planned ${o().planned.toFixed(1)} %`} bad={o().earned < o().planned - 0.05} good={o().earned >= o().planned && o().planned > 0} />
          <Kpi v={`${o().client_late} d`} l="the client kept documents past the review period" />
        </div>
        <Show when={!o().weights_ok.every(Boolean)}><p class="dwarn">The progress weights of a class do not add up to 100 %: Settings.</p></Show>
        <div class="dtabs"><Seg value={tab()} set={setTab} options={[["register", "Register"], ["log", "Transmittal log"], ["progress", "Progress"]]} /></div>
        <Show when={tab() === "register"}>
          <div class="panel ctr">
            <div class="phead"><input placeholder="Search number, title, vendor…" value={q()} onInput={e => setQ(e.currentTarget.value)} style={{ width: "240px" }} />
              <select value={fst()} onChange={e => setFst(e.currentTarget.value)}><option value="">Every status</option><For each={Object.keys(ST)}>{s => <option>{s}</option>}</For></select>
              <select value={fven()} onChange={e => setFven(e.currentTarget.value)}><option value="">Every vendor</option><For each={vendors()}>{v => <option>{v}</option>}</For></select>
              <span class="muted">{docs().length} shown</span></div>
            <table class="tbl dreg"><thead><tr><th>Document No.</th><th>Title</th><th>Vendor</th><th>Disc.</th><th>Cl.</th><th title="First issue at P + n">P +</th><th>1st planned</th><th>1st sent</th><th>Subs</th><th>Latest</th><th>Sent</th><th>Review due</th><th>Returned</th><th>Code</th><th>Status</th><th class="r">Earned</th><th class="r" title="Days the client kept it past the review period">Client late</th><th>Final (forecast)</th><th /></tr></thead>
              <tbody><For each={docs()}>{d => { const r = () => rowOf(d.id); return (
                <tr classList={{ key: d.key }}>
                  <td><input class="mono" value={d.number} onChange={e => putDoc({ ...d, number: e.currentTarget.value })} /></td>
                  <td><input value={d.title} onChange={e => putDoc({ ...d, title: e.currentTarget.value })} /></td>
                  <td><input style={{ width: "90px" }} value={d.vendor} onChange={e => putDoc({ ...d, vendor: e.currentTarget.value })} /></td>
                  <td><input style={{ width: "50px" }} value={d.disc} onChange={e => putDoc({ ...d, disc: e.currentTarget.value })} /></td>
                  <td><select value={d.class} onChange={e => putDoc({ ...d, class: +e.currentTarget.value })}><For each={classesOf(reg())}>{(c, i) => <option value={i() + 1}>{c.name}</option>}</For></select></td>
                  <td><input type="number" style={{ width: "52px" }} value={d.offset ?? ""} placeholder={d.first ? "date" : ""} onChange={e => putDoc({ ...d, offset: e.currentTarget.value === "" ? null : +e.currentTarget.value })} /></td>
                  <td class="muted">{dt(r()?.planned[0])}</td><td classList={{ bad: (r()?.late1 ?? 0) > 0 }}>{dt(r()?.actual[0])}</td>
                  <td>{r()?.subs || ""}</td><td>{r()?.rev}{r()?.purpose ? ` ${r()!.purpose}` : ""}</td><td>{dt(r()?.sent)}</td><td>{dt(r()?.due)}</td><td>{dt(r()?.returned)}</td><td>{r()?.code}</td>
                  <td><span class={`st ${ST[r()?.status ?? ""] ?? ""}`}>{r()?.status}</span>{r()?.late_for_task && <span class="st bad" title={`Needed by ${dt(r()?.need)} for ${r()?.task_name}`}> late for task</span>}</td>
                  <td class="r">{d.recurring || !d.progress ? <span class="muted">—</span> : `${(r()?.earned ?? 0).toFixed(0)} %`}</td>
                  <td class="r" classList={{ bad: (r()?.client_late ?? 0) > 0 }}>{r()?.client_late || ""}</td>
                  <td class="muted">{dt(r()?.finish)}</td>
                  <td class="acts"><button class="btn sm" title="Log a submission of this document" onClick={() => addSub(d)}>+ Sub</button><button class="x" title="Edit" onClick={() => editDoc(d)}>✎</button>
                    <button class="x" title="Remove the document and its submissions" onClick={() => save({ docs: reg().docs.filter(x => x.id !== d.id), log: reg().log.filter(s => s.doc !== d.id) })}>×</button></td>
                </tr>); }}</For></tbody></table>
            <Show when={!reg().docs.length}><p class="empty">No documents yet. <b>Import…</b> the vendor document register from a workbook (.xlsx), or add them with <b>+ Document</b>. Set the P date and the review cycle in <b>Settings…</b>.</p></Show>
          </div>
        </Show>
        <Show when={tab() === "log"}>
          <div class="panel ctr">
            <div class="phead"><span class="muted">One row per submission: every revision and issue purpose. The review due date and the days late follow from the settings.</span></div>
            <table class="tbl"><thead><tr><th>Document</th><th>Rev</th><th>Purpose</th><th>Transmittal</th><th>Sent</th><th>Review due</th><th>Return transmittal</th><th>Returned</th><th>Code</th><th>Comments ref.</th><th class="r">Days</th><th class="r">Late</th><th>State</th><th /></tr></thead>
              <tbody><For each={[...reg().log].sort((a, b) => (b.sent ?? "").localeCompare(a.sent ?? ""))}>{s => { const x = () => me()?.subs.find(y => y.id === s.id), d = () => reg().docs.find(y => y.id === s.doc); return (
                <tr>
                  <td title={d()?.title}><b class="mono">{d()?.number || "?"}</b><div class="muted small">{d()?.title}</div></td>
                  <td><input style={{ width: "44px" }} value={s.rev} onChange={e => putSub({ ...s, rev: e.currentTarget.value })} /></td>
                  <td><select value={s.purpose} onChange={e => putSub({ ...s, purpose: e.currentTarget.value })}><For each={[...new Set([...purposeOpts(), s.purpose].filter(Boolean))]}>{p => <option>{p}</option>}</For></select></td>
                  <td><input style={{ width: "110px" }} value={s.transmittal} onChange={e => putSub({ ...s, transmittal: e.currentTarget.value })} /></td>
                  <td><input type="date" value={s.sent ?? ""} onChange={e => putSub({ ...s, sent: e.currentTarget.value || null })} /></td>
                  <td>{dt(x()?.due)}</td>
                  <td><input style={{ width: "110px" }} value={s.ret_transmittal} onChange={e => putSub({ ...s, ret_transmittal: e.currentTarget.value })} /></td>
                  <td><input type="date" value={s.returned ?? ""} onChange={e => putSub({ ...s, returned: e.currentTarget.value || null })} /></td>
                  <td><select value={s.code} onChange={e => putSub({ ...s, code: e.currentTarget.value })}><option value="">—</option><For each={reg().codes}>{c => <option value={c.code} title={c.meaning}>{c.code}</option>}</For></select></td>
                  <td><input style={{ width: "110px" }} value={s.crs} onChange={e => putSub({ ...s, crs: e.currentTarget.value })} /></td>
                  <td class="r">{x()?.days ?? ""}</td><td class="r" classList={{ bad: (x()?.late ?? 0) > 0 }}>{x()?.late || ""}</td>
                  <td><span class={`st ${x()?.state === "Review overdue" ? "bad" : x()?.accepted ? "good" : ""}`}>{x()?.state}{x()?.accepted ? " · accepted" : ""}</span></td>
                  <td><button class="x" onClick={() => save({ log: reg().log.filter(y => y.id !== s.id) })}>×</button></td>
                </tr>); }}</For></tbody></table>
            <Show when={!reg().log.length}><p class="empty">No submissions yet. Click <b>+ Sub</b> on a document in the Register, or import a transmittal log.</p></Show>
          </div>
        </Show>
        <Show when={tab() === "progress"}>
          <div class="phead"><Seg value={per()} set={v => setPer(v as "monthly")} options={[["weekly", "Weekly"], ["monthly", "Monthly"]]} /></div>
          <Show when={o()[per()].length > 1} fallback={<p class="empty">Give the documents planned dates (P + n) to see the S-curve.</p>}>
            <ChartBox title="Document progress (weighted)" type="line" fmt="pct" max={100} labels={o()[per()].map(p => dt(p.end))} days={o()[per()].map(p => Math.round(Date.parse(p.end) / 864e5))}
              datasets={[line("Planned", o()[per()].map(p => p.planned), COL.grey, [5, 4]), line("Actual", o()[per()].map(p => p.actual), COL.blue)]} />
            <ChartBox title="First issues per period" type="bar" labels={o()[per()].map(p => dt(p.end))} datasets={[bar("Planned", o()[per()].map(p => p.first_planned), COL.grey), bar("Sent", o()[per()].map(p => p.first_done), COL.green)]} />
          </Show>
          <div class="cols2"><Breakdown title="By vendor" key={d => d.vendor} /><Breakdown title="By discipline" key={d => d.disc} /><Breakdown title="By document type" key={d => d.kind} /><Breakdown title="By class" key={d => `Class ${className(d)}`} /></div>
        </Show>
      </>}</Show>
      <Show when={(data()?.length ?? 0) > 1}>
        <div class="panel ctr"><h2>The project's plans</h2>
          <table class="tbl"><thead><tr><th>Plan</th><th class="r">Documents</th><th class="r">1st issued</th><th class="r">Final</th><th class="r">Earned</th><th class="r">Planned</th><th class="r">Client late (d)</th><th /></tr></thead>
            <tbody><For each={data()}>{d => <tr><td><b>{d.name}</b>{d.tab === out()?.tab ? " (this plan)" : ""}{!d.on && <span class="muted"> · module off</span>}</td><td class="r">{d.out.docs.length}</td><td class="r">{d.out.docs.filter(x => x.actual[0]).length}</td><td class="r">{d.out.docs.filter(x => x.actual[5]).length}</td>
              <td class="r">{d.out.earned.toFixed(1)} %</td><td class="r">{d.out.planned.toFixed(1)} %</td><td class="r">{d.out.client_late}</td>
              <td>{d.tab !== out()?.tab && <button class="btn sm" onClick={() => showTab(d.tab).then(() => setView("documents"))}>Open</button>}</td></tr>}</For></tbody></table>
        </div>
      </Show>
    </Page>
  );
};
