// What the registers' Settings share (Document and Procurement): the Lists tab (values with names,
// states that count or not, lists of your own), your own Columns (text, number, date, list, a
// formula per row), the client's Status words, and how renames and removals reach every row.
import { For, JSX, Show, createSignal } from "solid-js";
import { Help } from "./help";
import { ListItem, OwnCol, RegList } from "./api";
import { askText, askYes } from "./ui";
import { toast } from "./store";

/** A register as a workbook: the Dashboard first, then its sheets, with tabs along the bottom as in
 *  Sheets. Grid tabs (`grids`) fill the window; the others scroll. */
export const RegBook = (p: { title: string; help: string; tools: JSX.Element; tabs: [string, string][]; grids: string[]; tab: string; set: (k: string) => void; children: JSX.Element }) => (
  <section class="page regbook">
    <div class="ptools"><h1>{p.title}<Help k={p.help} /></h1><span class="grow" />{p.tools}</div>
    <div class="rbbody" classList={{ full: p.grids.includes(p.tab) }}>{p.children}</div>
    <div class="sheettabs regtabs"><For each={p.tabs}>{([k, l]) => <button classList={{ on: p.tab === k }} onClick={() => p.set(k)}>{l}</button>}</For></div>
  </section>
);
/** "Needs attention" on a dashboard: a few rows, each opening its card. */
export const Attention = (p: { title: string; rows: { key: string; text: string; note: string; bad?: boolean; open: () => void }[]; empty: string; more?: () => void }) => (
  <div class="panel ctr attn"><h2>{p.title}</h2>
    <Show when={p.rows.length} fallback={<p class="muted">{p.empty}</p>}>
      <For each={p.rows.slice(0, 8)}>{r => <button class="attnrow" onClick={r.open}><b>{r.key}</b><span>{r.text}</span><span class="st" classList={{ bad: !!r.bad }}>{r.note}</span></button>}</For>
      <Show when={p.rows.length > 8}><button class="btn sm" onClick={p.more}>All {p.rows.length}…</button></Show>
    </Show>
  </div>
);

export type LItem = ListItem & { was?: string };
export type LList = Omit<RegList, "items"> & { items: LItem[] };
export type LCol = OwnCol & { was?: string };
const move = <T,>(v: T[], i: number, d: number) => { const w = [...v], j = i + d; if (j < 0 || j >= w.length) return w; [w[i], w[j]] = [w[j], w[i]]; return w; };

/** A register's lists for editing: each item remembers its value (for renames), and values the rows
 *  use that a list lacks (plans from before the lists) join it. */
export const editLists = (lists: RegList[], used: (field: string) => string[]): LList[] => lists.map(l => {
  const more = [...new Set(used(l.field).map(v => v.trim()).filter(v => v && !l.items.some(i => i.value.toLowerCase() === v.toLowerCase())))].sort();
  return { ...l, items: [...l.items.map(i => ({ ...i, was: i.value })), ...more.map(v => ({ value: v, label: "", off: false, was: v }))] };
});

/** The rows after the Settings: renamed list values and columns followed, removed values in use
 *  cleared (when the user agrees); the lists and columns to keep. */
export const settle = async <R extends { own?: Record<string, string> }>(rows: R[], lists: LList[], cols: LCol[], before: RegList[], get: (r: R, f: string) => string, put: (r: R, f: string, v: string) => R) => {
  let out: R[] = rows.map(r => ({ ...r, own: { ...(r.own ?? {}) } }));
  for (const l of lists) for (const it of l.items) if (it.was && it.was !== it.value) out = out.map(r => (get(r, l.field) === it.was ? put(r, l.field, it.value) : r));
  const used = (f: string, v: string) => rows.some(r => get(r, f).toLowerCase() === v.toLowerCase());
  const gone = lists.flatMap(l => (before.find(x => x.field === l.field)?.items ?? []).filter(i => !l.items.some(x => x.was === i.value)).map(i => [l.field, i.value] as const)).filter(([f, v]) => used(f, v));
  if (gone.length && await askYes("Values removed from the lists", `${gone.map(g => `“${g[1]}”`).join(", ")} ${gone.length > 1 ? "are" : "is"} still used. Clear ${gone.length > 1 ? "them" : "it"} from those rows too? (No keeps the rows as they are and the value comes back to its list.)`, "Clear from the rows"))
    out = out.map(r => gone.reduce<R>((x, [f, v]) => (get(x, f) === v ? put(x, f, "") : x), r));
  for (const c of cols) if (c.was && c.was !== c.name) out = out.map(r => { const own = { ...(r.own ?? {}) }; if (c.was! in own) { own[c.name] = own[c.was!]; delete own[c.was!]; } return { ...r, own }; });
  const columns: OwnCol[] = cols.filter(c => c.name.trim()).map(({ was, ...c }) => ({ ...c, name: c.name.trim() }));
  // A list column has its list (named as the column); a list renamed with its column follows.
  let ls = lists.map(l => { const c = cols.find(c => c.was === l.field && c.was !== c.name); return c ? { ...l, field: c.name, name: c.name } : l; });
  for (const c of columns.filter(c => c.kind === "list")) if (!ls.some(l => l.field === c.name)) ls = [...ls, { name: c.name, field: c.name, items: [] }];
  return { rows: out, columns, lists: ls.map(l => ({ ...l, items: l.items.filter(i => i.value.trim()).map(({ was, ...i }) => ({ ...i, value: i.value.trim() })) })) as RegList[] };
};

/** Settings › Lists. `fixed` are the standard lists (they cannot be removed). */
export const ListsTab = (p: { lists: () => LList[]; set: (l: LList[]) => void; cols: () => LCol[]; setCols: (c: LCol[]) => void; fixed: string[]; count: (field: string, value: string) => number; names?: Record<string, string>; rows: string }) => {
  const [pasteIn, setPasteIn] = createSignal<number | null>(null);
  const setL = (k: number, x: Partial<LList>) => p.set(p.lists().map((l, j) => (j === k ? { ...l, ...x } : l)));
  const setI = (k: number, i: number, x: Partial<LItem>) => setL(k, { items: p.lists()[k].items.map((it, j) => (j === i ? { ...it, ...x } : it)) });
  return <>
    <p class="muted">Each list is a drop-down in the register: type in a cell to search it. Rename a value here and every {p.rows} using it follows; the number beside a value is how many {p.rows}s use it. A value typed in the register that is not in its list is added to it.</p>
    <div class="dlists"><For each={p.lists()}>{(l, k) => (
      <div class="dlist"><h4>{p.fixed.includes(l.field) ? <span>{l.name}</span> : <span>{l.name} <small class="muted">(own)</small></span>}<span class="grow" /><small class="muted">{l.items.length}</small>
        <Show when={!p.fixed.includes(l.field)}><button class="x" title="Remove the list" onClick={() => p.set(p.lists().filter((_, j) => j !== k()))}>×</button></Show></h4>
        <For each={l.items}>{(it, i) => <div class="li">
          <input class="v" value={it.value} onChange={e => setI(k(), i(), { value: e.currentTarget.value })} title="As written in the register" />
          <Show when={l.field === "state"} fallback={<input value={it.label} placeholder={p.names?.[l.field] ?? "note"} onChange={e => setI(k(), i(), { label: e.currentTarget.value })} />}>
            <label class="chk" title={`${p.rows[0].toUpperCase()}${p.rows.slice(1)}s in this state count in progress and status`}><input type="checkbox" checked={!it.off} onChange={e => setI(k(), i(), { off: !e.currentTarget.checked })} /> counts</label></Show>
          <span class="u">{it.was ? p.count(l.field, it.was) || "" : ""}</span>
          <button class="x" title="Up" onClick={() => setL(k(), { items: move(l.items, i(), -1) })}>↑</button><button class="x" title="Down" onClick={() => setL(k(), { items: move(l.items, i(), 1) })}>↓</button>
          <button class="x" title="Remove" onClick={() => setL(k(), { items: l.items.filter((_, j) => j !== i()) })}>×</button></div>}</For>
        <Show when={pasteIn() === k()}><textarea placeholder={"One per line (a column copied from a spreadsheet).\nTwo columns (value, name) for codes with names."} onChange={e => {
          const rows = e.currentTarget.value.split(/\r?\n/).map(x => x.split("\t").map(y => y.trim())).filter(x => x[0]);
          setL(k(), { items: [...l.items, ...rows.filter(x => !l.items.some(i => i.value.toLowerCase() === x[0].toLowerCase())).map(x => ({ value: x[0], label: x[1] ?? "", off: false }))] }); setPasteIn(null);
        }} /></Show>
        <div class="add"><a onClick={() => setL(k(), { items: [...l.items, { value: "", label: "", off: false }] })}>+ Add</a><a onClick={() => setPasteIn(pasteIn() === k() ? null : k())}>Paste a list…</a></div>
      </div>)}</For></div>
    <div class="inline" style={{ "margin-top": "10px" }}><button class="btn sm" onClick={async () => {
      const n = (await askText("New list", "Name of the list (it becomes a column of the register too): Area, Unit, Inspector…", ""))?.trim(); if (!n) return;
      if (p.lists().some(l => l.name.toLowerCase() === n.toLowerCase() || l.field === n) || p.cols().some(c => c.name === n)) { toast(`There is already a list or a column called “${n}”.`); return; }
      p.set([...p.lists(), { name: n, field: n, items: [] }]); p.setCols([...p.cols(), { name: n, kind: "list", formula: "" }]);
    }}>+ New list</button></div>
  </>;
};

/** Settings › Columns: your own columns. */
export const ColumnsTab = (p: { cols: () => LCol[]; set: (c: LCol[]) => void; table: string; examples: string[] }) => <>
  <p class="muted">Columns of your own, after the standard ones: text, a number, a date, a list (its values in <b>Lists</b>) or a formula worked out per row. A formula reads the row's columns by name: {p.examples.map((x, i) => <>{i ? ", " : ""}<code>{x}</code></>)}. The sheets read them too: <code>{p.table}[Column]</code>.</p>
  <table class="tbl"><thead><tr><th>Column</th><th>Holds</th><th>Formula</th><th /></tr></thead>
    <tbody><For each={p.cols()}>{(c, i) => { const set = (x: Partial<LCol>) => p.set(p.cols().map((y, j) => (j === i() ? { ...y, ...x } : y))); return <tr>
      <td><input value={c.name} onChange={e => set({ name: e.currentTarget.value })} /></td>
      <td><select value={c.kind} onChange={e => set({ kind: e.currentTarget.value })}><For each={[["text", "Text"], ["number", "Number"], ["date", "Date"], ["list", "A list"], ["formula", "A formula"]]}>{o => <option value={o[0]} selected={o[0] === c.kind}>{o[1]}</option>}</For></select></td>
      <td><input style={{ width: "360px" }} class="mono" disabled={c.kind !== "formula"} value={c.formula} placeholder={c.kind === "formula" ? p.examples[0] : ""} onChange={e => set({ formula: e.currentTarget.value })} /></td>
      <td class="nowrap"><button class="x" onClick={() => p.set(move(p.cols(), i(), -1))}>↑</button><button class="x" onClick={() => p.set(move(p.cols(), i(), 1))}>↓</button><button class="x" onClick={() => p.set(p.cols().filter((_, j) => j !== i()))}>×</button></td></tr>; }}</For></tbody></table>
  <button class="btn sm" onClick={() => p.set([...p.cols(), { name: `Column ${p.cols().length + 1}`, kind: "text", formula: "" }])}>+ Column</button>
</>;

/** Settings › Status words. `words`: key, standard words, when it applies. */
export const WordsTab = (p: { words: [string, string, string][]; value: () => Record<string, string>; set: (w: Record<string, string>) => void }) => <>
  <p class="muted">The words the Status column uses, as the client writes them. Leave one empty to keep the standard words.</p>
  <table class="tbl"><thead><tr><th>When</th><th>Standard words</th><th>The client's words</th></tr></thead>
    <tbody><For each={p.words}>{w => <tr><td class="muted">{w[2]}</td><td>{w[1]}</td><td><input style={{ width: "260px" }} value={p.value()[w[0]] ?? ""} placeholder={w[1]} onChange={e => p.set({ ...p.value(), [w[0]]: e.currentTarget.value })} /></td></tr>}</For></tbody></table>
</>;
