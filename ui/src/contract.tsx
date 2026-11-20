// Modules (Project › Modules) and the Contract Register: contract dates against
// their forecast with delay damages, payment milestones and the cash-in curve. The engine works
// every figure out (contract.rs); this page shows them and edits the contract.
import { For, Show, createResource, createSignal } from "solid-js";
import { Contract, ContractOut, money } from "./api";
import { act, call, grid, out, P, sel as taskSel, tick, setView, openDialog, toast, kept, openMenu } from "./store";
import { Attention, RegBook } from "./regkit";
import { Kpi } from "./reports";
import { ChartBox, COL, line } from "./charts";
import { Dialog, askYes } from "./ui";
import { Col, Edit, RegGrid, dt, parseDate } from "./regrid";
import { showTab } from "./plans";

/** The modules a plan can take on (the engine's `contract::MODULES`), with their view. */
export const MODULES: [string, string, string, string][] = [
  ["contract", "Contract Register", "Contract dates against their forecast and delay damages at risk, payments and the cash-in curve, variations, notices and their time bars, guarantees, invoices and obligations.", "contract"],
  ["documents", "Document Register", "Vendor documents and every submission: review cycles in working days, return codes, status, client delay, weighted progress and S-curves.", "documents"],
  ["procurement", "Procurement Register", "Sub-orders from enquiry to delivery: planned against actual and promised dates, the date each is needed, an expediting log, weighted progress and S-curves.", "procurement"],
];
export const hasModule = (k: string) => (out()?.project.modules ?? []).includes(k);
/** Whether a module holds data in this plan, and the op that empties it. */
const DATA: Record<string, [() => boolean, object]> = {
  contract: [() => { const c = P().contract; return !!c && !!(c.dates.length || c.payments.length || c.value || c.variations?.length || c.notices?.length || c.guarantees?.length || c.invoices?.length || c.obligations?.length); }, { Contract: { c: {} } }],
  documents: [() => { const r = P().documents; return !!r && !!(r.docs.length || r.log.length); }, { Documents: { r: {} } }],
  procurement: [() => !!P().procurement?.items.length, { Procurement: { r: {} } }],
};

/** Project › Modules: tick the modules this plan uses. Switching one off keeps its data unless the user deletes it. */
export const modules = () => openDialog(() => {
  const [on, setOn] = createSignal<string[]>([...(P().modules ?? [])]), [drop, setDrop] = createSignal<string[]>([]);
  const off = () => (P().modules ?? []).filter(k => !on().includes(k) && DATA[k]?.[0]());
  const ok = async () => {
    for (const k of off()) if (drop().includes(k)) await act(DATA[k][1]);
    const fresh = on().find(k => !(P().modules ?? []).includes(k));
    await act({ Modules: { on: on() } });
    if (fresh) setView(MODULES.find(m => m[0] === fresh)![3] as "contract");
  };
  return (
    <Dialog title="Modules" help="modules" okText="Apply" ok={ok}>
      <p>Modules add registers to this plan. A plan shows only the ones ticked here; the rest of the app stays as it is.</p>
      <For each={MODULES}>{([k, l, d]) => <label class="chk modrow"><input type="checkbox" checked={on().includes(k)} onChange={() => setOn(on().includes(k) ? on().filter(x => x !== k) : [...on(), k])} /><span><b>{l}</b><small class="muted">{d}</small></span></label>}</For>
      <For each={off()}>{k => <div class="modoff"><b>{MODULES.find(m => m[0] === k)?.[1]} is being switched off.</b>
        <label class="chk"><input type="radio" checked={!drop().includes(k)} onChange={() => setDrop(drop().filter(x => x !== k))} /> Keep its data in the plan, hidden, for later</label>
        <label class="chk"><input type="radio" checked={drop().includes(k)} onChange={() => setDrop([...drop(), k])} /> Delete its data</label></div>}</For>
    </Dialog>
  );
});

const STATUS: Record<string, string> = { Met: "good", "Met late": "warn", "On track": "good", "At risk": "warn", Late: "bad", Overdue: "bad", Paid: "good", Invoiced: "warn", Due: "warn" };
const KEY: Record<string, string> = { ok: "good", warn: "warn", bad: "bad" };
type LKey = "dates" | "payments" | "variations" | "notices" | "guarantees" | "invoices" | "obligations";
type Any = { id: number } & Record<string, unknown>;
const NUM = new Set(["value", "ld_rate", "ld_cap", "grace", "pct", "amount", "terms", "claimed", "approved", "days_claimed", "days_granted", "days", "after", "certified", "retention"]);
const DATE = new Set(["date", "invoiced", "paid", "submitted", "decided", "event_date", "sent", "from", "expires", "released", "due", "done"]);
const BLANK: Record<LKey, (id: number) => Any> = {
  dates: id => ({ id, name: "", task: 0, date: null, value: 0, ld_rate: 0, ld_cap: 0, grace: 0, note: "" }),
  payments: id => ({ id, name: "", pct: 0, amount: 0, task: 0, terms: 0, invoiced: null, paid: null }),
  variations: id => ({ id, no: "", desc: "", raised_by: "", status: "Draft", submitted: null, decided: null, claimed: 0, approved: 0, days_claimed: 0, days_granted: 0, delay_event: "", remarks: "" }),
  notices: id => ({ id, no: "", event: "", kind: "", event_date: null, days: 0, sent: null, our_ref: "", linked: "" }),
  guarantees: id => ({ id, name: "", basis: "", amount: 0, issuer: "", number: "", from: null, expires: null, task: 0, after: 0, released: null }),
  invoices: id => ({ id, no: "", payment: 0, amount: 0, invoiced: null, terms: 0, certified: 0, retention: 0, paid: null, remarks: "" }),
  obligations: id => ({ id, clause: "", what: "", party: "", owner: "", due: null, every: "", done: null, remarks: "" }),
};
const today = () => new Date(Date.now() - new Date().getTimezoneOffset() * 6e4).toISOString().slice(0, 10);
const nextNo = (l: Any[], key: string, first: string) => {
  const all = l.map(x => String(x[key] ?? "")).filter(t => /\d+\s*$/.test(t));
  if (!all.length) return first;
  const best = all.reduce((a, b) => (+(b.match(/(\d+)\s*$/)![1]) > +(a.match(/(\d+)\s*$/)![1]) ? b : a));
  return best.replace(/(\d+)(\s*)$/, (_, n: string, s: string) => String(+n + 1).padStart(n.length, "0") + s);
};

const conTab = kept("contract-tab", "dashboard");
export const ContractView = () => {
  const [tab, setTab] = conTab;
  const [data] = createResource(() => [tick(), out()?.tab] as const, () => call<{ tab: number; name: string; on: boolean; out: ContractOut }[]>("contract"));
  const k = (): Contract => ({ number: "", value: 0, warn_days: 14, dates: [], payments: [], ...(P().contract ?? {}) });
  const save = (c: Partial<Contract>) => act({ Contract: { c: { ...k(), ...c } } });
  const next = (l: { id: number }[]) => Math.max(0, ...l.map(x => x.id)) + 1;
  const me = () => data()?.find(d => d.tab === out()?.tab);
  const o = () => me()?.out;
  const cur = () => P().currency;
  const m$ = (v: number) => money(v, { ...cur(), decimals: 0 });
  const n$ = (v: number) => (v ? v.toLocaleString("en-GB", { maximumFractionDigits: 2 }) : "");
  const tasks = () => grid().rows.filter(r => r.uid > 0);
  const rows = (key: LKey) => ((k()[key] ?? []) as unknown as Any[]);
  // ---- one grid per list: typed values become the stored ones ----
  const conv = (key: string, v: string): unknown => {
    if (key === "task") return v.trim() ? tasks().find(t => String(t.id) === v.trim() || t.name.toLowerCase() === v.trim().toLowerCase())?.uid ?? undefined : 0;
    if (key === "payment") return v.trim() ? k().payments.find(p => p.name.toLowerCase() === v.trim().toLowerCase() || String(p.id) === v.trim())?.id ?? undefined : 0;
    if (NUM.has(key)) { const n = +v.replace(/[,\s%]/g, ""); return v.trim() === "" ? 0 : isFinite(n) ? n : undefined; }
    if (DATE.has(key)) { const d = parseDate(v); return d === null ? undefined : d || null; }
    return v;
  };
  const setRows = (key: LKey) => (e: Edit<Any>[]) => {
    const m = new Map(rows(key).map(x => [x.id, { ...x }])); let bad = 0;
    for (const x of e) { const r = m.get(x.row.id), v = conv(x.key, x.value); if (!r) continue; if (v === undefined) { bad++; continue; } r[x.key] = v; }
    if (bad) toast(`${bad} value${bad > 1 ? "s were" : " was"} not understood (a date, a number, or a task or payment of the plan) and left as it was.`);
    save({ [key]: rows(key).map(x => m.get(x.id)!) } as Partial<Contract>);
  };
  const addRows = (key: LKey, first: Partial<Any> = {}) => (vals: Record<string, string>[]) => {
    const l = [...rows(key)];
    for (const v of vals) { const r = { ...BLANK[key](next(l)), ...first }; for (const [c, x] of Object.entries(v)) { const y = conv(c, x); if (y !== undefined) r[c] = y; } l.push(r); }
    save({ [key]: l } as Partial<Contract>);
  };
  const removeRows = (key: LKey) => async (rs: Any[]) => {
    if (rs.length > 1 && !(await askYes("Delete rows", `Delete ${rs.length} rows? Ctrl+Z brings them back.`, "Delete"))) return;
    const ids = new Set(rs.map(x => x.id)); save({ [key]: rows(key).filter(x => !ids.has(x.id)) } as Partial<Contract>);
  };
  const [sel, setSel] = createSignal<Any[]>([]);
  // ---- columns ----
  const txt = (key: string, name: string, width: number, more: Partial<Col<Any>> = {}): Col<Any> => ({ key, name, width, kind: "text", get: r => String(r[key] ?? ""), ...more });
  const num = (key: string, name: string, width: number, more: Partial<Col<Any>> = {}): Col<Any> => ({ key, name, width, kind: "number", get: r => (r[key] ? String(r[key]) : ""), show: r => n$(r[key] as number), cls: () => "num", ...more });
  const date = (key: string, name: string, width = 84): Col<Any> => ({ key, name, width, kind: "date", get: r => (r[key] as string) ?? "" });
  const used = (key: LKey, f: string, more: string[] = []) => () => [...new Set([...more, ...rows(key).map(x => String(x[f] ?? "")).filter(Boolean)])].map(v => ({ value: v }));
  const list = (lk: LKey, key: string, name: string, width: number, std: string[] = []): Col<Any> => ({ key, name, width, kind: "list", get: r => String(r[key] ?? ""), options: used(lk, key, std) });
  const taskCol = (name = "Task"): Col<Any> => ({ key: "task", name, width: 190, kind: "list", strict: true, get: r => String(tasks().find(t => t.uid === r.task)?.id ?? ""),
    show: r => { const t = tasks().find(t => t.uid === r.task); return t ? `${t.id} ${t.name}` : ""; }, options: () => tasks().map(t => ({ value: String(t.id), label: t.name })) });
  const out1 = (lk: "variations" | "notices" | "guarantees" | "invoices" | "obligations", id: number) => o()?.[lk].find(x => x.id === id);
  const status = (lk: "variations" | "notices" | "guarantees" | "invoices" | "obligations", name = "Status", width = 130): Col<Any> => ({ key: "status_", name, width, kind: "calc", get: r => out1(lk, r.id)?.status ?? "",
    show: r => { const x = out1(lk, r.id); return x ? <span class={`st ${KEY[x.key] ?? ""}`}>{x.status}</span> : ""; } });
  const calcDate = (lk: "notices" | "guarantees" | "invoices" | "obligations", name: string): Col<Any> => ({ key: "due_", name, width: 84, kind: "calc", date: true, get: r => out1(lk, r.id)?.due ?? "" });
  const left = (lk: "notices" | "guarantees" | "invoices" | "obligations", name = "Days left"): Col<Any> => ({ key: "left_", name, width: 64, kind: "calc", get: r => { const x = out1(lk, r.id)?.left; return x === null || x === undefined ? "" : String(x); }, cls: r => ((out1(lk, r.id)?.left ?? 0) < 0 ? "late num" : "num") });
  const ms = (id: number) => o()?.milestones.find(x => x.id === id), py = (id: number) => o()?.payments.find(x => x.id === id);
  const COLS: Record<LKey, () => Col<Any>[]> = {
    dates: () => [txt("name", "Contract date", 220), taskCol("Met by task"), date("date", "Contract"),
      { key: "fc", name: "Forecast", width: 84, kind: "calc", date: true, get: r => ms(r.id)?.forecast ?? "" },
      { key: "bl", name: "Baseline", width: 84, kind: "calc", date: true, get: r => ms(r.id)?.baseline ?? "" },
      { key: "var", name: "Against contract", width: 84, kind: "calc", get: r => { const v = ms(r.id)?.var; return v === null || v === undefined ? "" : v > 0 ? `${v} d late` : v < 0 ? `${-v} d early` : "on the day"; }, cls: r => ((ms(r.id)?.var ?? 0) > 0 ? "late" : "") },
      { key: "st", name: "Status", width: 90, kind: "calc", get: r => ms(r.id)?.status ?? "", show: r => <span class={`st ${STATUS[ms(r.id)?.status ?? ""] ?? ""}`}>{ms(r.id)?.status}</span> },
      num("ld_rate", "Damages %/wk", 70, { tip: "Delay damages in % of the value per week or part week late" }), num("grace", "Grace (d)", 60), num("ld_cap", "Cap %", 56), num("value", "Of value", 100, { tip: "The value the damages are a share of; empty = the contract value" }),
      { key: "ld", name: "At risk", width: 100, kind: "calc", get: r => m$(ms(r.id)?.ld ?? 0), cls: r => ((ms(r.id)?.ld ?? 0) > 0 ? "late num" : "num") }, txt("note", "Note", 200)],
    payments: () => [txt("name", "Payment milestone", 220), num("pct", "Share %", 60), num("amount", "Amount", 100, { tip: "An amount instead of a share" }), taskCol("Task that earns it"), num("terms", "Terms (d)", 60),
      { key: "pl", name: "Planned", width: 84, kind: "calc", date: true, get: r => py(r.id)?.planned ?? "" },
      { key: "due_", name: "Due (forecast)", width: 84, kind: "calc", date: true, get: r => py(r.id)?.due ?? "" },
      date("invoiced", "Invoiced"), date("paid", "Paid"),
      { key: "st", name: "Status", width: 90, kind: "calc", get: r => py(r.id)?.status ?? "", show: r => <span class={`st ${STATUS[py(r.id)?.status ?? ""] ?? ""}`}>{py(r.id)?.status}</span> },
      { key: "val", name: "Value", width: 100, kind: "calc", get: r => m$(py(r.id)?.amount ?? 0), cls: () => "num" }],
    variations: () => [txt("no", "VO No.", 90, { cls: () => "mono" }), txt("desc", "Description", 260), list("variations", "raised_by", "Raised by", 80, ["Client", "Us"]),
      { key: "status", name: "Status", width: 96, kind: "list", get: r => String(r.status ?? ""), options: used("variations", "status", ["Draft", "Submitted", "Approved", "Rejected"]), show: r => { const x = out1("variations", r.id); return <span class={`st ${KEY[x?.key ?? ""] ?? ""}`}>{String(r.status || "Draft")}</span>; } },
      date("submitted", "Submitted"), date("decided", "Decided"), num("claimed", "Claimed", 100), num("approved", "Approved", 100), num("days_claimed", "Days claimed", 64), num("days_granted", "Days granted", 64),
      { key: "wait", name: "Waiting (d)", width: 64, kind: "calc", get: r => { const x = out1("variations", r.id)?.left; return x === null || x === undefined ? "" : String(x); }, cls: () => "num", tip: "Days since a submitted variation went to the client" },
      { key: "delay_event", name: "Delay event", width: 130, kind: "list", get: r => String(r.delay_event ?? ""), options: () => (P().delays ?? []).map((d: { name: string }) => ({ value: d.name })) }, txt("remarks", "Remarks", 200)],
    notices: () => [txt("no", "Notice", 90, { cls: () => "mono" }), txt("event", "Event", 260), { key: "kind", name: "Kind", width: 110, kind: "list", get: r => String(r.kind ?? ""), options: () => (k().notice_kinds ?? []).map(x => ({ value: x.name, label: `${x.days} days` })), editList: () => contractSettings() },
      date("event_date", "Event date"), num("days", "Days allowed", 64, { tip: "Empty: the kind's days (Settings)" }), calcDate("notices", "Due by"), date("sent", "Sent"), txt("our_ref", "Our ref.", 110), status("notices", "Status", 120), left("notices"),
      { key: "linked", name: "Linked", width: 90, kind: "list", get: r => String(r.linked ?? ""), options: () => rows("variations").map(v => ({ value: String(v.no), label: String(v.desc) })) }],
    guarantees: () => [txt("name", "Guarantee / insurance", 200), txt("basis", "Basis", 130), num("amount", "Amount", 100), list("guarantees", "issuer", "Issuer", 100), txt("number", "Number", 100), date("from", "From"), date("expires", "Expires"), left("guarantees"),
      taskCol("Released by task"), num("after", "+ days", 54), calcDate("guarantees", "Release"), date("released", "Released"), status("guarantees", "Status", 120)],
    invoices: () => [txt("no", "Invoice", 90, { cls: () => "mono" }), { key: "payment", name: "Payment milestone", width: 180, kind: "list", strict: true, get: r => k().payments.find(p => p.id === r.payment)?.name ?? "", options: () => k().payments.map(p => ({ value: p.name })) },
      num("amount", "Amount", 100), date("invoiced", "Invoiced"), num("terms", "Terms (d)", 60, { tip: "Empty: the contract's days (Settings)" }), calcDate("invoices", "Due"), num("certified", "Certified", 100), num("retention", "Retention", 90), date("paid", "Paid"), status("invoices", "Status", 110), left("invoices"), txt("remarks", "Remarks", 200)],
    obligations: () => [txt("clause", "Clause", 70), txt("what", "Obligation", 260), list("obligations", "party", "Party", 70, ["Us", "Client"]), list("obligations", "owner", "Owner", 110), date("due", "First due"),
      { key: "every", name: "Every", width: 80, kind: "list", strict: true, get: r => String(r.every ?? ""), options: () => [{ value: "" , label: "once" }, { value: "week" }, { value: "month" }, { value: "quarter" }] },
      date("done", "Last done"), calcDate("obligations", "Next due"), left("obligations"), status("obligations", "Status", 110), txt("remarks", "Remarks", 200)],
  };
  // ---- the many-at-once buttons ----
  const markSent = () => { const ids = new Set(sel().map(x => x.id)); save({ notices: (k().notices ?? []).map(n => (ids.has(n.id) && !n.sent ? { ...n, sent: today() } : n)) }); };
  const markDone = () => { const ids = new Set(sel().map(x => x.id)); save({ obligations: (k().obligations ?? []).map(n => (ids.has(n.id) ? { ...n, done: today() } : n)) }); };
  const raiseInvoices = () => {
    const l = [...(k().invoices ?? [])] as unknown as Any[];
    for (const p of sel()) { const r = BLANK.invoices(next(l)); r.no = nextNo(l, "no", "INV-001"); r.payment = p.id; r.amount = py(p.id)?.amount ?? 0; r.invoiced = today(); l.push(r); }
    save({ invoices: l as never }).then(() => { setTab("invoices"); toast(`${sel().length} invoice${sel().length > 1 ? "s" : ""} raised: type the numbers and dates as they really are.`, true); });
  };
  const fromDelay = (e: MouseEvent) => openMenu(e, (P().delays ?? []).length ? (P().delays ?? []).map((d: { name: string; description: string }) => ({ label: d.name, run: () => addRows("variations", { raised_by: "Us", status: "Draft", no: nextNo(rows("variations"), "no", "VO-001"), delay_event: d.name })([{ desc: d.description || d.name }]) })) : [{ label: "No delay events yet: add them in Delay analysis", off: true, run: () => {} }]);
  const NEW: Record<LKey, [string, string, string]> = { dates: ["+ Contract date", "name", "Select the task that meets it in the task sheet first"], payments: ["+ Payment", "name", ""], variations: ["+ Variation", "no", ""], notices: ["+ Notice", "no", ""],
    guarantees: ["+ Guarantee", "name", ""], invoices: ["+ Invoice", "no", ""], obligations: ["+ Obligation", "what", ""] };
  const FIRST: Partial<Record<LKey, () => Partial<Any>>> = {
    dates: () => ({ task: sel0() }), payments: () => ({ task: sel0() }), variations: () => ({ no: nextNo(rows("variations"), "no", "VO-001"), status: "Draft" }), notices: () => ({ no: nextNo(rows("notices"), "no", "NOT-001") }),
    invoices: () => ({ no: nextNo(rows("invoices"), "no", "INV-001") }),
  };
  const sel0 = () => picked();
  const picked = () => taskSel().find(u => u > 0) ?? 0;
  const Sheet = (p: { k: LKey; hint: string }) => (
    <>
      <div class="rgbar">
        <button class="btn pri" onClick={() => addRows(p.k, FIRST[p.k]?.())([{}])} title={NEW[p.k][2] || "Or type in the empty row at the foot"}>{NEW[p.k][0]}</button>
        <Show when={p.k === "payments"}><button class="btn" disabled={!sel().length} onClick={raiseInvoices}>Raise invoice{sel().length > 1 ? `s (${sel().length})` : ""}</button></Show>
        <Show when={p.k === "variations"}><button class="btn" onClick={fromDelay}>From a delay event ▾</button></Show>
        <Show when={p.k === "notices"}><button class="btn" disabled={!sel().length} onClick={markSent}>Mark sent{sel().length > 1 ? ` (${sel().length})` : ""}</button></Show>
        <Show when={p.k === "obligations"}><button class="btn" disabled={!sel().length} onClick={markDone}>Mark done{sel().length > 1 ? ` (${sel().length})` : ""}</button></Show>
        <span class="sep" /><button class="btn danger" disabled={!sel().length} onClick={() => removeRows(p.k)(sel())}>Delete{sel().length > 1 ? ` (${sel().length})` : ""}</button>
        <span class="grow" /><Tot k={p.k} />
        <span class="rglegend"><span><i />you type</span><span><i class="calc" />the app works it out</span></span>
      </div>
      <RegGrid id={`contract-${p.k}`} rows={rows(p.k)} cols={COLS[p.k]()} rowId={x => x.id} set={setRows(p.k)} add={addRows(p.k, FIRST[p.k]?.())} remove={removeRows(p.k)} selected={setSel} newHint={p.hint} />
    </>
  );
  const Tot = (p: { k: LKey }) => <Show when={o()}>{s => <span class="muted rgtot">{
    p.k === "variations" ? `claimed ${m$(s().vo_claimed)} · approved ${m$(s().vo_approved)} · waiting ${m$(s().vo_pending)} · ${s().vo_days} days granted · revised contract ${m$(s().revised_value)}`
    : p.k === "payments" ? `${s().pay_pct.toFixed(1)} % of the value · received ${m$(s().received)}`
    : p.k === "invoices" ? `invoiced ${m$(s().invoiced)} · certified ${m$(s().certified)} · paid ${m$(s().inv_paid)} · retention ${m$(s().retention)}`
    : p.k === "dates" ? `${s().late} late · ${s().at_risk} at risk · damages ${m$(s().ld)} (cap ${m$(s().ld_max)})`
    : p.k === "notices" ? `${s().notices_due} due within 7 days · ${s().notices_late} overdue`
    : p.k === "guarantees" ? `${s().guar_warn} to act on` : `${s().obl_late} overdue · ${s().obl_due} due within 7 days`}</span>}</Show>;
  const Spark = (p: { t: [string, string][]; date: string | null }) => {
    const pts = () => (p.date ? p.t.map(([, f]) => (Date.parse(f) - Date.parse(p.date!)) / 864e5) : []);
    const W = 90, H = 22, max = () => Math.max(5, ...pts().map(Math.abs));
    return <Show when={pts().length > 1} fallback={<span class="muted">—</span>}><svg width={W} height={H} class="spark"><line x1="0" x2={W} y1={H / 2} y2={H / 2} stroke="#C9CFD8" stroke-dasharray="2 2" />
      <polyline fill="none" stroke={pts()[pts().length - 1] > 0 ? "#C73B3B" : "#2E9D57"} stroke-width="1.5" points={pts().map((v, i) => `${(i / (pts().length - 1)) * (W - 4) + 2},${H / 2 - (v / max()) * (H / 2 - 2)}`).join(" ")} />
      <title>{p.t.map(([d, f]) => `${dt(d)}: forecast ${dt(f)}`).join("\n")}</title></svg></Show>;
  };
  const Plans = () => <Show when={(data()?.length ?? 0) > 1}>
    <div class="panel ctr"><h2>The project's plans</h2>
      <table class="tbl"><thead><tr><th>Plan</th><th>Contract dates</th><th>Late</th><th>At risk</th><th class="r">Damages at risk</th><th class="r">Variations approved</th><th class="r">Payments</th><th class="r">Received</th><th /></tr></thead>
        <tbody><For each={data()}>{d => <tr>
          <td><b>{d.name}</b>{d.tab === out()?.tab ? " (this plan)" : ""}</td>
          <td>{d.on ? d.out.milestones.length : <span class="muted">module off</span>}</td><td classList={{ bad: d.out.late > 0 }}>{d.out.late}</td><td>{d.out.at_risk}</td>
          <td class="r">{m$(d.out.ld)}</td><td class="r">{m$(d.out.vo_approved)}</td><td class="r">{m$(d.out.payments.reduce((a, p) => a + p.amount, 0))}</td><td class="r">{m$(d.out.received)}</td>
          <td>{d.tab !== out()?.tab && <button class="btn sm" onClick={() => showTab(d.tab).then(() => setView("contract"))}>Open</button>}</td>
        </tr>}</For>
        <tr class="tot"><td><b>Total</b></td><td>{data()!.reduce((a, d) => a + d.out.milestones.length, 0)}</td><td>{data()!.reduce((a, d) => a + d.out.late, 0)}</td><td>{data()!.reduce((a, d) => a + d.out.at_risk, 0)}</td>
          <td class="r">{m$(data()!.reduce((a, d) => a + d.out.ld, 0))}</td><td class="r">{m$(data()!.reduce((a, d) => a + d.out.vo_approved, 0))}</td><td class="r">{m$(data()!.reduce((a, d) => a + d.out.payments.reduce((x, p) => x + p.amount, 0), 0))}</td><td class="r">{m$(data()!.reduce((a, d) => a + d.out.received, 0))}</td><td /></tr></tbody></table>
      <p class="muted">The master and its open subplans. Open a subplan to edit its contract.</p>
    </div>
  </Show>;
  const go = (t: string) => () => setTab(t);
  const nameOf = (lk: LKey, id: number, f: string) => String(rows(lk).find(x => x.id === id)?.[f] ?? "");
  const badge = (n: number | undefined) => (n ? n : undefined);
  return (
    <RegBook title="Contract Register" help="contract" tab={tab()} set={t => { setSel([]); setTab(t); }} grids={["dates", "payments", "variations", "notices", "guarantees", "invoices", "obligations"]}
      tabs={[["dashboard", "Dashboard"], ["dates", "Contract dates", badge((o()?.late ?? 0) + (o()?.at_risk ?? 0))], ["payments", "Payments"], ["variations", "Variations", badge(o()?.variations.filter(x => x.key === "warn").length)],
        ["notices", "Notices", badge(o()?.notices_due)], ["guarantees", "Guarantees", badge(o()?.guar_warn)], ["invoices", "Invoices", badge(o()?.inv_overdue)], ["obligations", "Obligations", badge((o()?.obl_late ?? 0) + (o()?.obl_due ?? 0))]]}
      tools={<button class="btn" onClick={() => contractSettings()}>Settings…</button>}>
      <Show when={o()}>{s => <>
        <Show when={tab() === "dashboard"}>
        <div class="kpis">
          <Kpi v={m$(s().revised_value)} l={`${k().number ? `contract ${k().number}` : "contract value"} with ${m$(s().vo_approved)} of variations`} />
          <Kpi v={`${s().late} late · ${s().at_risk} at risk`} l={`of ${s().milestones.length} contract dates`} bad={s().late > 0} good={!s().late && !s().at_risk && s().milestones.length > 0} />
          <Kpi v={m$(s().ld)} l={`delay damages at today's forecast (cap ${m$(s().ld_max)})`} bad={s().ld > 0} />
          <Kpi v={m$(s().vo_pending)} l={`claimed in variations waiting for the client · ${s().vo_days} days granted`} />
          <Kpi v={String(s().notices_due)} l={`notices due within 7 days · ${s().notices_late} overdue`} bad={s().notices_due > 0} />
          <Kpi v={String(s().guar_warn)} l="guarantees to extend, expiring or expired" bad={s().guar_warn > 0} />
          <Kpi v={m$(s().invoiced)} l={`invoiced · ${m$(s().inv_paid)} paid · ${s().inv_overdue} overdue`} bad={s().inv_overdue > 0} />
          <Kpi v={`${s().obl_late} · ${s().obl_due}`} l="obligations overdue · due within 7 days" bad={s().obl_late > 0} />
        </div>
        <div class="cols3">
          <Attention title="Contract dates late or at risk" empty="Every contract date is on track." more={go("dates")}
            rows={s().milestones.filter(m => m.status === "Late" || m.status === "At risk").map(m => ({ key: m.name, text: `contract ${dt(m.date)} · forecast ${dt(m.forecast)}`, note: m.status, bad: m.status === "Late", open: go("dates") }))} />
          <Attention title="Notices to send" empty="No notice is waiting to be sent." more={go("notices")}
            rows={(k().notices ?? []).map(n => ({ n, x: out1("notices", n.id) })).filter(({ n, x }) => !n.sent && x?.left !== null && x?.left !== undefined).sort((a, b) => (a.x!.left ?? 0) - (b.x!.left ?? 0)).map(({ n, x }) => ({ key: n.no || "Notice", text: n.event, note: x!.status, bad: x!.key === "bad", open: go("notices") }))} />
          <Attention title="Guarantees to act on" empty="Every guarantee is in force for as long as it is needed." more={go("guarantees")}
            rows={s().guarantees.filter(x => x.key === "warn" || x.key === "bad").map(x => ({ key: nameOf("guarantees", x.id, "name"), text: `expires ${dt(nameOf("guarantees", x.id, "expires") || null)}${x.due ? ` · release ${dt(x.due)}` : ""}`, note: x.status, bad: x.key === "bad", open: go("guarantees") }))} />
          <Attention title="Invoices overdue" empty="No invoice is past its payment date." more={go("invoices")}
            rows={s().invoices.filter(x => x.key === "bad").map(x => ({ key: nameOf("invoices", x.id, "no") || "Invoice", text: `${m$(+nameOf("invoices", x.id, "amount") || 0)} · due ${dt(x.due)}`, note: x.status, bad: true, open: go("invoices") }))} />
          <Attention title="Obligations due" empty="Nothing due in the next week." more={go("obligations")}
            rows={s().obligations.filter(x => x.key === "bad" || x.key === "warn").map(x => ({ key: nameOf("obligations", x.id, "clause") || "—", text: nameOf("obligations", x.id, "what"), note: x.status, bad: x.key === "bad", open: go("obligations") }))} />
          <Attention title="Payments due or overdue" empty="Nothing due to invoice." more={go("payments")}
            rows={s().payments.filter(p => p.status === "Due" || p.status === "Overdue").map(p => ({ key: p.name, text: `${m$(p.amount)} · due ${dt(p.due)}`, note: p.status, bad: p.status === "Overdue", open: go("payments") }))} />
        </div>
        <Show when={s().cash.length > 1}>
          <ChartBox title="Cash in" type="line" fmt="money" labels={s().cash.map(c => dt(c[0]))} days={s().cash.map(c => Math.round(Date.parse(c[0]) / 864e5))}
            datasets={[line("Planned", s().cash.map(c => c[1]), COL.grey, [5, 4]), line("Forecast", s().cash.map(c => c[2]), COL.blue), line("Received", s().cash.map(c => c[3]), COL.green)]} />
        </Show>
        <Show when={s().milestones.some(m => m.trend.length > 1)}>
          <div class="panel ctr"><h2>Contract dates: forecast at each period close</h2>
            <table class="tbl"><tbody><For each={s().milestones}>{m => <tr><td>{m.name}</td><td>{dt(m.date)}</td><td><Spark t={m.trend} date={m.date} /></td><td><span class={`st ${STATUS[m.status] ?? ""}`}>{m.status}</span></td></tr>}</For></tbody></table></div>
        </Show>
        <Plans />
        </Show>
        <Show when={tab() === "dates"}><Sheet k="dates" hint="Type a contract date's name here; pick the task that meets it" /></Show>
        <Show when={tab() === "payments"}><Sheet k="payments" hint="Type a payment milestone here: its share or amount, the task that earns it, the terms" /></Show>
        <Show when={tab() === "variations"}><Sheet k="variations" hint="Type a VO number here" /></Show>
        <Show when={tab() === "notices"}><Sheet k="notices" hint="Type a notice number here: the event, its kind and date set the time bar" /></Show>
        <Show when={tab() === "guarantees"}><Sheet k="guarantees" hint="Type a guarantee or insurance here" /></Show>
        <Show when={tab() === "invoices"}><Sheet k="invoices" hint="Type an invoice number here, or select payments and Raise invoice" /></Show>
        <Show when={tab() === "obligations"}><Sheet k="obligations" hint="Type a clause here: what is to be done, by whom, from when and how often" /></Show>
      </>}</Show>
      <Show when={!o()}><Plans /></Show>
    </RegBook>
  );
};

/** The contract's settings: number, value, warning days, the kinds of notice and their days, guarantee and invoice days. */
export const contractSettings = () => openDialog(() => {
  const c = (): Contract => ({ number: "", value: 0, warn_days: 14, dates: [], payments: [], ...(P().contract ?? {}) });
  const [v, setV] = createSignal({ number: c().number, value: c().value, warn_days: c().warn_days, expiry_warn: c().expiry_warn || 60, invoice_terms: c().invoice_terms || 30 });
  const [kinds, setKinds] = createSignal((c().notice_kinds ?? []).map(x => ({ ...x })));
  const set = (f: string, x: string | number) => setV({ ...v(), [f]: x });
  const ok = () => act({ Contract: { c: { ...c(), ...v(), notice_kinds: kinds().filter(x => x.name.trim()) } } });
  return (
    <Dialog title="Contract settings" help="contract" okText="Save" ok={ok}>
      <div class="prow">
        <label>Contract no.<input value={v().number} onInput={e => set("number", e.currentTarget.value)} /></label>
        <label>Contract value<input type="number" min="0" value={v().value || ""} onInput={e => set("value", +e.currentTarget.value || 0)} /></label>
      </div>
      <div class="prow">
        <label title="A contract date with less float than this is At risk">At risk under (days)<input type="number" min="0" value={v().warn_days} onInput={e => set("warn_days", +e.currentTarget.value || 0)} /></label>
        <label title="A guarantee expiring within this many days is flagged">Guarantee expiry warning (days)<input type="number" min="0" value={v().expiry_warn} onInput={e => set("expiry_warn", +e.currentTarget.value || 0)} /></label>
        <label title="Days to pay an invoice that does not give its own terms">Invoice terms (days)<input type="number" min="0" value={v().invoice_terms} onInput={e => set("invoice_terms", +e.currentTarget.value || 0)} /></label>
      </div>
      <h3>Kinds of notice and their time bars</h3>
      <p class="muted">The days the contract allows after the event for each kind of notice: a new notice of that kind takes them.</p>
      <table class="tbl nkinds"><thead><tr><th>Kind of notice</th><th class="r">Days allowed</th><th /></tr></thead>
        <tbody><For each={kinds()}>{(x, i) => <tr>
          <td><input value={x.name} onInput={e => setKinds(kinds().map((y, j) => (j === i() ? { ...y, name: e.currentTarget.value } : y)))} /></td>
          <td class="r"><input type="number" min="0" style={{ width: "70px" }} value={x.days} onInput={e => setKinds(kinds().map((y, j) => (j === i() ? { ...y, days: +e.currentTarget.value || 0 } : y)))} /></td>
          <td><button class="x" title="Remove" onClick={() => setKinds(kinds().filter((_, j) => j !== i()))}>×</button></td></tr>}</For></tbody></table>
      <button class="btn sm" onClick={() => setKinds([...kinds(), { name: "", days: 28 }])}>+ Kind</button>
      <Show when={!kinds().length}> <button class="btn sm" onClick={() => setKinds([{ name: "Delay", days: 28 }, { name: "Variation", days: 28 }, { name: "Claim particulars", days: 42 }, { name: "Force majeure", days: 14 }])}>Add the usual ones</button></Show>
    </Dialog>
  );
});

/** Opens the view, or offers to switch the module on. */
export const openContract = () => (hasModule("contract") ? setView("contract") : (toast("The Contract Register is a module: tick it in Project › Modules."), modules()));
export const openProcurement = () => (hasModule("procurement") ? setView("procurement") : (toast("The Procurement Register is a module: tick it in Project › Modules."), modules()));
export const openDocuments = () => (hasModule("documents") ? setView("documents") : (toast("The Document Register is a module: tick it in Project › Modules."), modules()));
