// The Task Board: the plan's tasks (or the registers' documents or orders) as cards in columns and
// swimlanes. The engine works out every card (board.rs); dragging one is one change it makes: a
// task's actual start or finish or the field its column stands for, a document's submission or
// return, an order's milestones, or a block with its reason.
import { For, Show, createMemo, createResource, createSignal } from "solid-js";
import { act, call, out, P, tick, setSel, setView, setBackstage, openDialog } from "./store";
import { Board } from "./api";
import { Page } from "./views";
import { Dialog, Seg, askText } from "./ui";
import { hasModule } from "./contract";
import { dt } from "./regrid";

type Wait = { kind: string; label: string; state: string; late: boolean };
type Card = { key: string; id: number; code: string; name: string; col: string; lane: string; crit: boolean; float_days: number | null; pct: number; steps: [number, number]; people: string; start: string | null; finish: string | null; late: boolean; blocked: string; waits: Wait[]; note: string };
type Col = { key: string; name: string; count: number; wip: number; over: boolean };
type BoardOut = { kind: string; by: string; cols: Col[]; lanes: string[]; cards: Card[]; day: string | null; movable: boolean };

const today = () => new Date(Date.now() - new Date().getTimezoneOffset() * 6e4).toISOString().slice(0, 10);
const cfg = (): Board => ({ kind: "", by: "", lanes: "", weeks: 0, wip: {}, blocked: {}, ...(P().board ?? {}) });
const setCfg = (b: Partial<Board>) => act({ Board: { b: { ...cfg(), ...b } } });

/** The fields a board can be laid out by: status, phase, people, and the plan's codes and own fields. */
const fields = (): [string, string][] => [
  ["phase", "Phase"], ["res", "Resource names"], ["responsible", "Responsible"], ["priority", "Priority"],
  ...(P().codes ?? []).map(c => [`code:${c.name}`, c.name] as [string, string]),
  ...(P().fields ?? []).filter(f => f.alias && !f.formula).map(f => [f.key, f.alias] as [string, string]),
];
const LANES: Record<string, [string, string][]> = { documents: [["vendor", "Vendor"], ["package", "Package"], ["disc", "Discipline"], ["kind", "Document type"]], orders: [["vendor", "Sub-vendor"], ["origin", "Country of origin"]] };

export const BoardView = () => {
  const [data] = createResource(() => [tick(), out()?.tab] as const, () => call<BoardOut>("board"));
  const [q, setQ] = createSignal(""), [drag, setDrag] = createSignal<Card | null>(null), [over, setOver] = createSignal("");
  const kind = () => cfg().kind || "tasks";
  const cards = createMemo(() => { const t = q().toLowerCase(); return (data()?.cards ?? []).filter(k => !t || `${k.code} ${k.name} ${k.people} ${k.note}`.toLowerCase().includes(t)); });
  const at = (lane: string, col: string) => cards().filter(k => k.lane === lane && k.col === col);
  const drop = async (col: string) => {
    const k = drag(); setDrag(null); setOver("");
    if (!k || k.col === col) return;
    let reason = "";
    if (col === "blocked") { const r = await askText("Blocked", `Why is ${k.code} ${k.name} blocked?`, k.blocked); if (!r?.trim()) return; reason = r; }
    const date = data()?.day ?? today();
    await act({ BoardMove: { kind: kind(), id: k.id, to: col, date, reason } });
  };
  const open = (k: Card) => {
    if (kind() === "tasks") { setSel([k.id]); setView("gantt"); }
    else setView(kind() === "documents" ? "documents" : "procurement");
  };
  const wipDialog = () => openDialog(() => {
    const [w, setW] = createSignal<Record<string, number>>({ ...cfg().wip });
    return (
      <Dialog title="Work in progress limits" help="board" okText="Save" ok={() => setCfg({ wip: Object.fromEntries(Object.entries(w()).filter(([, v]) => v > 0)) })}>
        <p class="muted">The most cards a column should hold; a column over its limit turns red. Empty: no limit.</p>
        <table class="tbl"><tbody><For each={data()?.cols ?? []}>{c => <tr><td>{c.name}</td>
          <td class="r"><input type="number" min="0" style={{ width: "70px" }} value={w()[c.key] || ""} onInput={e => setW({ ...w(), [c.key]: +e.currentTarget.value || 0 })} /></td></tr>}</For></tbody></table>
      </Dialog>
    );
  });
  const printA3 = async () => { const m = await import("./print"); m.printPaper("A3", true); setBackstage("print"); };
  const kinds = (): [string, string][] => [["tasks", "Tasks"], ...(hasModule("documents") ? [["documents", "Documents"] as [string, string]] : []), ...(hasModule("procurement") ? [["orders", "Orders"] as [string, string]] : [])];
  return (
    <Page title="Task Board" help="board" tools={<>
      <Show when={kinds().length > 1}><Seg value={kind()} set={v => setCfg({ kind: v === "tasks" ? "" : v, lanes: "" })} options={kinds()} /></Show>
      <Show when={kind() === "tasks"}>
        <label class="muted">Columns <select value={cfg().by || "status"} onChange={e => setCfg({ by: e.currentTarget.value === "status" ? "" : e.currentTarget.value })}>
          <option value="status">Status</option><For each={fields()}>{([k, n]) => <option value={k} selected={cfg().by === k}>{n}</option>}</For></select></label>
      </Show>
      <label class="muted">Lanes <select value={cfg().lanes} onChange={e => setCfg({ lanes: e.currentTarget.value })}>
        <option value="">None</option><For each={kind() === "tasks" ? fields() : LANES[kind()]}>{([k, n]) => <option value={k} selected={cfg().lanes === k}>{n}</option>}</For></select></label>
      <Show when={kind() === "tasks"}>
        <label class="muted" title="Only the work starting within these weeks of the status date, what is under way and what finished in the last two weeks">Look-ahead <select value={String(cfg().weeks)} onChange={e => setCfg({ weeks: +e.currentTarget.value })}>
          <For each={[0, 1, 2, 3, 4, 6, 8, 12]}>{n => <option value={String(n)} selected={cfg().weeks === n}>{n ? `${n} week${n > 1 ? "s" : ""}` : "All"}</option>}</For></select></label>
      </Show>
      <input class="q" placeholder="Search cards…" value={q()} onInput={e => setQ(e.currentTarget.value)} />
      <button class="btn" onClick={wipDialog}>WIP limits…</button>
      <button class="btn" onClick={printA3} title="File › Print on A3 landscape, with the plan's header and footer">Print A3…</button>
    </>}>
      <Show when={data()}>{o => <div class="kb" classList={{ lanes: o().lanes.length > 1 || !!o().lanes[0] }} style={{ "grid-template-columns": `${o().lanes.length > 1 || o().lanes[0] ? "140px " : ""}repeat(${o().cols.length}, minmax(220px, 1fr))` }}>
        <Show when={o().lanes.length > 1 || o().lanes[0]}><div class="kbcorner" /></Show>
        <For each={o().cols}>{c => <div class="kbhead" classList={{ over: c.over, blocked: c.key === "blocked" }}>
          <b>{c.name}</b><span>{c.count}{c.wip ? ` / ${c.wip}` : ""}</span></div>}</For>
        <For each={o().lanes}>{lane => <>
          <Show when={o().lanes.length > 1 || lane}><div class="kblane"><b>{lane}</b><span>{cards().filter(k => k.lane === lane).length}</span></div></Show>
          <For each={o().cols}>{c => <div class="kbcol" classList={{ drop: over() === `${lane}|${c.key}`, blocked: c.key === "blocked", over: c.over }}
            onDragOver={e => { if (drag() && o().movable) { e.preventDefault(); setOver(`${lane}|${c.key}`); } }} onDragLeave={() => setOver("")} onDrop={e => { e.preventDefault(); drop(c.key); }}>
            <For each={at(lane, c.key)}>{k => <div class="kc" classList={{ crit: k.crit, late: k.late, done: k.pct >= 100 }} draggable={o().movable} onDragStart={e => { setDrag(k); e.dataTransfer?.setData("text/plain", k.key); }} onDragEnd={() => { setDrag(null); setOver(""); }} onDblClick={() => open(k)} title="Drag to another column; double-click to open">
              <div class="kct"><span class="kcode">{k.code}</span><span class="grow" />
                <Show when={k.float_days !== null && kind() !== "documents"}><span class="kfloat" classList={{ neg: (k.float_days ?? 0) < 0, zero: k.float_days === 0 }} title="Total float, days">{k.float_days} d</span></Show></div>
              <div class="kcn">{k.name}</div>
              <Show when={k.note}><div class="kcnote muted">{k.note}</div></Show>
              <Show when={kind() !== "orders" || k.pct > 0}><div class="kbar" title={`${Math.round(k.pct)} %`}><i style={{ width: `${Math.min(100, k.pct)}%` }} /></div></Show>
              <div class="kcm">
                <Show when={k.steps[1]}><span title="Steps done">☑ {k.steps[0]}/{k.steps[1]}</span></Show>
                <Show when={k.people}><span class="kpeople" title={k.people}>{k.people}</span></Show>
                <Show when={k.finish}><span classList={{ late: k.late }} title={kind() === "tasks" ? "Finish" : "Due"}>{dt(k.finish)}</span></Show>
              </div>
              <Show when={k.waits.length}><div class="kwaits"><For each={k.waits}>{w => <span class="kw" classList={{ late: w.late }} title={`${w.kind}: ${w.state}`}>{w.kind === "document" ? "▤" : w.kind === "order" ? "▣" : "→"} {w.label}<Show when={w.kind !== "task"}> · {w.state}</Show></span>}</For></div></Show>
              <Show when={k.blocked}><div class="kblock">⛔ {k.blocked}</div></Show>
            </div>}</For>
          </div>}</For>
        </>}</For>
        <Show when={!o().cards.length}><p class="empty kbempty">{kind() === "tasks" ? (cfg().weeks ? "No work in this look-ahead." : "No tasks yet.") : kind() === "documents" ? "No documents in the Document Register yet." : "No orders in the Procurement Register yet."}</p></Show>
      </div>}</Show>
      <Show when={data() && !data()!.movable}><p class="muted">Phases come from the outline: cards cannot move between them here.</p></Show>
    </Page>
  );
};
