// Sheets dialogs: Insert Function (every worksheet function, by category, with what it does) and
// Plan Data (a live list of tasks or resources: one formula that always shows the plan as it is now).
import { For, Show, createEffect, createMemo, createSignal } from "solid-js";
import { Dialog, Row } from "./ui";
import { openDialog } from "./store";
import { Catalog, Fn, sigArgs } from "./formulas";

const CATS: Record<string, string> = { MathTrig: "Math", Statistical: "Statistics", Lookup: "Lookup and reference", Text: "Text", DateTime: "Date and time", Logical: "Logical", Financial: "Financial", Information: "Information", Engineering: "Engineering", Database: "Database", Dynamic: "Dynamic arrays", Web: "Web", Cube: "Cube", Compatibility: "Compatibility" };
const catName = (c: string) => CATS[c] ?? c;
/** The functions people use most, shown first. */
const COMMON = ["SUM", "AVERAGE", "COUNT", "COUNTA", "COUNTIF", "COUNTIFS", "SUMIF", "SUMIFS", "MIN", "MAX", "IF", "IFS", "IFERROR", "XLOOKUP", "VLOOKUP", "INDEX", "MATCH", "FILTER", "SORT", "UNIQUE", "TODAY", "NETWORKDAYS", "WORKDAY", "EOMONTH", "DATEDIF", "TEXT", "ROUND", "TEXTJOIN"];

/** Insert Function: pick a function; `pick` gets its name. */
export const insertFunction = (cat: Catalog | undefined, pick: (name: string) => void) => openDialog(() => {
  const [q, setQ] = createSignal(""), [c, setC] = createSignal("common"), [sel, setSel] = createSignal("SUM");
  const all = () => cat?.functions ?? [];
  const cats = createMemo(() => [...new Set(all().map(f => f[3]))].sort((a, b) => catName(a).localeCompare(catName(b))));
  const list = createMemo((): Fn[] => {
    const s = q().trim().toUpperCase();
    if (s) return all().filter(f => f[0].includes(s) || f[2].toUpperCase().includes(s)).sort((a, b) => +!a[0].startsWith(s) - +!b[0].startsWith(s)).slice(0, 200);
    return c() === "common" ? COMMON.map(n => all().find(f => f[0] === n)!).filter(Boolean) : all().filter(f => f[3] === c());
  });
  const cur = () => all().find(f => f[0] === sel());
  // A search picks its best match, so Insert (or Enter) takes it straight away.
  createEffect(() => { const l = list(); if (l.length && !l.some(f => f[0] === sel())) setSel(l[0][0]); });
  return (
    <Dialog title="Insert Function" wide okText="Insert" ok={() => { if (cur()) pick(sel()); }} help="sheets">
      <div class="form">
        <Row label="Search"><input value={q()} placeholder="What do you want to do? e.g. lookup, workdays, average" onInput={e => setQ(e.currentTarget.value)} /></Row>
        <Show when={!q().trim()}><Row label="Category"><select onChange={e => setC(e.currentTarget.value)}><option value="common">Most used</option><For each={cats()}>{x => <option value={x}>{catName(x)}</option>}</For></select></Row></Show>
      </div>
      <div class="fnlist"><For each={list()}>{f => <button classList={{ on: f[0] === sel() }} onClick={() => setSel(f[0])} onDblClick={() => { pick(f[0]); import("./store").then(s => s.closeDialog()); }}><b>{f[0]}</b><span>{f[2]}</span></button>}</For></div>
      <Show when={cur()}>{f => <div class="fnhelp"><code>{f()[1]}</code><p>{f()[2]}</p><Show when={sigArgs(f()[1]).length}><p class="muted">Arguments in [brackets] are optional.</p></Show></div>}</Show>
    </Dialog>
  );
});

const DATES = ["Start", "Finish", "Baseline Start", "Baseline Finish", "Actual Start", "Actual Finish"];
const MONEY = ["Cost", "Baseline Cost", "Planned Value", "Earned Value", "Actual Cost", "Rate"];
const PCT = ["% Complete", "Planned % Complete"];
const ROWS: [string, string, string][] = [
  ["tasks", "Tasks (no summaries)", "Tasks[Summary]=FALSE"], ["all", "All rows", ""], ["milestones", "Milestones", "Tasks[Milestone]=TRUE"],
  ["critical", "Critical tasks", "(Tasks[Critical]=TRUE)*(Tasks[Summary]=FALSE)"], ["late", "Late tasks", "(Tasks[Late]=TRUE)*(Tasks[Summary]=FALSE)"],
  ["open", "Not finished", "(Tasks[% Complete]<100)*(Tasks[Summary]=FALSE)"], ["now", "In progress", "(Tasks[% Complete]>0)*(Tasks[% Complete]<100)*(Tasks[Summary]=FALSE)"],
  ["next", "Starting in the next 4 weeks", "(Tasks[Start]>=TODAY())*(Tasks[Start]<TODAY()+28)*(Tasks[Summary]=FALSE)"],
];

/** The formula, headings and formats for a Plan Data list. */
export const planData = (table: "Tasks" | "Resources", all: string[], cols: string[], rows: string, sort: string, desc: boolean, top: number) => {
  const idx = cols.map(c => all.indexOf(c) + 1).filter(i => i > 0);
  let f = idx.length === all.length ? table : `CHOOSECOLS(${table},${idx.join(",")})`;
  const cond = table === "Tasks" ? ROWS.find(r => r[0] === rows)?.[2] ?? "" : "";
  if (cond) f = `FILTER(${f},${cond},"None")`;
  const si = cols.indexOf(sort);
  if (si >= 0) f = `SORT(${f},${si + 1},${desc ? -1 : 1})`;
  if (top > 0) f = `TAKE(${f},${top})`;
  const fmt = cols.map(c => (DATES.includes(c) ? "date" : MONEY.includes(c) ? "money" : PCT.includes(c) ? '0"%"' : c === "Country" ? "flag" : ""));
  return { formula: `=${f}`, heads: cols, fmt };
};

/** Plan Data: choose a table, columns, rows and order; `put` writes it at the selected cell. */
export const planDataDialog = (cat: Catalog | undefined, put: (d: ReturnType<typeof planData>) => void) => openDialog(() => {
  const [table, setTable] = createSignal<"Tasks" | "Resources">("Tasks");
  const all = () => (table() === "Tasks" ? cat?.tasks : cat?.resources) ?? [];
  const [cols, setCols] = createSignal<string[]>(["Name", "Start", "Finish", "% Complete"]);
  const [rows, setRows] = createSignal("tasks"), [sort, setSort] = createSignal(""), [desc, setDesc] = createSignal(false), [top, setTop] = createSignal(0);
  const pickTable = (t: "Tasks" | "Resources") => { setTable(t); setCols(t === "Tasks" ? ["Name", "Start", "Finish", "% Complete"] : ["Name", "Group", "Country", "Work"]); setSort(""); };
  const toggle = (c: string) => setCols(cols().includes(c) ? cols().filter(x => x !== c) : all().filter(x => x === c || cols().includes(x)));
  const d = () => planData(table(), all(), cols(), rows(), sort(), desc(), top());
  return (
    <Dialog title="Plan Data" wide okText="Insert" help="sheets" ok={() => { if (!cols().length) return false; put(d()); }}>
      <p class="muted">Puts a live list at the selected cell: headings, then one formula that always shows the plan as it is now. No copying and pasting.</p>
      <div class="form">
        <Row label="From"><select onChange={e => pickTable(e.currentTarget.value as "Tasks")}><option value="Tasks">Tasks</option><option value="Resources">Resources</option></select></Row>
        <Show when={table() === "Tasks"}><Row label="Rows"><select onChange={e => setRows(e.currentTarget.value)}><For each={ROWS}>{r => <option value={r[0]} selected={r[0] === rows()}>{r[1]}</option>}</For></select></Row></Show>
        <Row label="Sort by"><select onChange={e => setSort(e.currentTarget.value)}><option value="">(plan order)</option><For each={cols()}>{c => <option selected={c === sort()}>{c}</option>}</For></select>
          <label class="chk"><input type="checkbox" checked={desc()} onChange={e => setDesc(e.currentTarget.checked)} />Largest or latest first</label></Row>
        <Row label="Show"><select onChange={e => setTop(+e.currentTarget.value)}><option value="0">Every row</option><For each={[5, 10, 20]}>{n => <option value={n}>The first {n}</option>}</For></select></Row>
      </div>
      <div class="colpick"><For each={all()}>{c => <label class="chk"><input type="checkbox" checked={cols().includes(c)} onChange={() => toggle(c)} />{c}</label>}</For></div>
      <div class="fnhelp"><code>{d().formula}</code></div>
    </Dialog>
  );
});
