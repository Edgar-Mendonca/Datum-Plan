// The task sheet: virtual rows, MS Project density, inline editing, header menus.
import { For, Show, createSignal, createMemo, onMount, onCleanup, JSX } from "solid-js";
import { Col, Row } from "./api";
import { grid, cols, saveCols, sel, setSel, cur, setCur, set, act, openMenu, Item, fields, P, setSort, sort, setGroup, refresh, table, gridW } from "./store";
import { C } from "./cmds";
import * as D from "./dialogs";
import { Icon } from "./ui";

export const RH = 20;
export const [scrollTop, setScrollTop] = createSignal(0);
const [viewH, setViewH] = createSignal(600);

const CHOICES: Record<string, string[]> = {
  ctype: ["As Soon As Possible", "As Late As Possible", "Start No Earlier Than", "Start No Later Than", "Finish No Earlier Than", "Finish No Later Than", "Must Start On", "Must Finish On"],
  mode: ["Auto Scheduled", "Manually Scheduled"],
  type: ["Fixed Duration", "Fixed Units", "Fixed Work"],
  acttype: ["Task Dependent", "Resource Dependent", "Level of Effort", "Start Milestone", "Finish Milestone", "WBS Summary"],
  pcttype: ["Duration", "Units", "Physical"],
  accrual: ["Start", "Prorated", "End"],
};
const choices = (k: string): string[] | undefined => {
  if (CHOICES[k]) return CHOICES[k];
  if (k === "cal") return ["", ...P().calendars.map(c => c.name)];
  if (k.startsWith("code:")) return ["", ...(P().codes.find(c => c.name === k.slice(5))?.values.map(v => v.value) ?? [])];
  const def = k.startsWith("c:") && P().fields.find(f => f.key === k.slice(2));
  if (def && def.lookup.length) return [...(def.restrict ? [""] : []), ...def.lookup.map(l => l.value)];
  return undefined;
};
const right = (c: Col) => ["Num", "Int", "Cost", "Pct", "Work"].includes(c.kind) || c.key === "dur";

/** Field picker menu (Insert Column, Group by, Sort by). */
export const fieldMenu = (run: (key: string) => void): Item[] => {
  const groups = new Map<string, Item[]>();
  const names = new Map(P().fields.map(f => [`c:${f.key}`, f.alias]));
  for (const f of fields().task) {
    const g = groups.get(f.group) ?? [];
    const alias = names.get(f.key);
    g.push({ label: alias ? `${alias} (${f.name})` : f.name, run: () => run(f.key) });
    groups.set(f.group, g);
  }
  const items: Item[] = [...groups].map(([label, sub]) => ({ label, sub }));
  if (P().codes.length) items.push({ label: "Activity codes", sub: P().codes.map(c => ({ label: c.name, run: () => run(`code:${c.name}`) })) });
  items.push("-", { label: "All fields…", run: () => D.insertColumn(run) });
  return items;
};

export const Grid = (p: { width?: number }) => {
  let body!: HTMLDivElement, head!: HTMLDivElement;
  const [edit, setEdit] = createSignal<{ row: number; col: number; text: string } | null>(null);
  const [anchor, setAnchor] = createSignal(0);
  const widths = createMemo(() => cols().map(c => c[1]));
  const total = createMemo(() => 44 + widths().reduce((a, b) => a + b, 0));
  const rows = () => grid().rows;
  const first = () => Math.max(0, Math.floor(scrollTop() / RH) - 5);
  const visible = createMemo(() => rows().slice(first(), first() + Math.ceil(viewH() / RH) + 12));
  const selSet = createMemo(() => new Set(sel()));
  const ro = new ResizeObserver(() => setViewH(body.clientHeight));
  onMount(() => { ro.observe(body); document.addEventListener("grid-scroll", onScroll as EventListener); });
  onCleanup(() => { ro.disconnect(); document.removeEventListener("grid-scroll", onScroll as EventListener); });
  const onScroll = (e: CustomEvent<number>) => { if (Math.abs(body.scrollTop - e.detail) > 1) body.scrollTop = e.detail; };

  const pick = (k: number, e: MouseEvent | KeyboardEvent | null, col = cur().col) => {
    const r = rows()[k];
    if (!r) { setCur({ row: k, col }); setSel([]); return; }
    if (e?.shiftKey) { const [a, b] = [Math.min(anchor(), k), Math.max(anchor(), k)]; setSel(rows().slice(a, b + 1).map(x => x.uid).filter(u => u > 0)); }
    else if (e && (e as MouseEvent).ctrlKey) { setSel(selSet().has(r.uid) ? sel().filter(u => u !== r.uid) : [...sel(), r.uid]); setAnchor(k); }
    else { setSel(r.uid > 0 ? [r.uid] : []); setAnchor(k); }
    setCur({ row: k, col });
    const top = k * RH;
    if (top < body.scrollTop) body.scrollTop = top;
    else if (top + RH > body.scrollTop + body.clientHeight - RH) body.scrollTop = top - body.clientHeight + 2 * RH;
  };

  const startEdit = (row: number, col: number, text?: string) => {
    const r = rows()[row], c = grid().cols[col];
    if (row >= rows().length) { setEdit({ row, col, text: text ?? "" }); return; }
    if (!r || r.uid <= 0 || !c || !c.edit) return;
    if (c.kind === "Flag" && text === undefined) { set(r.uid, c.key, r.cells[col] === "Yes" ? "no" : "yes"); return; }
    setEdit({ row, col, text: text ?? r.cells[col] });
  };
  const commit = async (move: [number, number] | null) => {
    const e = edit();
    if (!e) return;
    setEdit(null);
    const r = rows()[e.row], c = grid().cols[e.col];
    if (!r && e.text.trim()) {
      // Typing in the blank row adds a task at the end.
      if (await act({ Insert: { at: grid().total, name: c?.key === "name" ? e.text : "New task", ms: false } })) {
        await refresh();
        const last = rows()[rows().length - 1];
        if (c && c.key !== "name" && last) await set(last.uid, c.key, e.text);
      }
    } else if (r && c && e.text !== r.cells[e.col]) await set(r.uid, c.key, e.text);
    if (move) pick(Math.max(0, e.row + move[0]), null, Math.max(0, Math.min(grid().cols.length - 1, e.col + move[1])));
    body.focus();
  };

  const key = (e: KeyboardEvent) => {
    if (edit()) return;
    const { row, col } = cur();
    const ctrl = e.ctrlKey || e.metaKey;
    const go = (dr: number, dc: number) => { e.preventDefault(); pick(Math.max(0, Math.min(rows().length, row + dr)), e.shiftKey ? e : null, Math.max(0, Math.min(grid().cols.length - 1, col + dc))); };
    switch (e.key) {
      case "ArrowDown": return go(1, 0);
      case "ArrowUp": return go(-1, 0);
      case "ArrowLeft": if (e.altKey && e.shiftKey) { e.preventDefault(); return C.outdent(); } return go(0, -1);
      case "ArrowRight": if (e.altKey && e.shiftKey) { e.preventDefault(); return C.indent(); } return go(0, 1);
      case "PageDown": return go(Math.floor(body.clientHeight / RH), 0);
      case "PageUp": return go(-Math.floor(body.clientHeight / RH), 0);
      case "Home": return ctrl ? go(-row, 0) : go(0, -col);
      case "End": return ctrl ? go(rows().length - row - 1, 0) : go(0, grid().cols.length);
      case "Tab": return go(0, e.shiftKey ? -1 : 1);
      case "Enter": e.preventDefault(); return startEdit(row, col);
      case "F2": e.preventDefault(); return e.shiftKey ? C.info() : ctrl ? C.link() : startEdit(row, col);
      case "Insert": e.preventDefault(); return C.newTask();
      case "Delete": e.preventDefault(); return C.del();
      case "Escape": return setSel([]);
    }
    if (ctrl && e.key.toLowerCase() === "a") { e.preventDefault(); setSel(rows().filter(r => r.uid > 0).map(r => r.uid)); return; }
    if (ctrl && e.key.toLowerCase() === "c") { e.preventDefault(); return C.copy(); }
    if (ctrl && e.key.toLowerCase() === "x") { e.preventDefault(); return C.cut(); }
    if (ctrl && e.key.toLowerCase() === "v") { e.preventDefault(); return C.paste(); }
    if (!ctrl && !e.altKey && e.key.length === 1) { e.preventDefault(); startEdit(row, col, e.key); }
  };

  const rowMenu = (e: MouseEvent, k: number) => {
    if (!selSet().has(rows()[k]?.uid)) pick(k, null);
    openMenu(e, [
      { label: "Cut", run: C.cut, keys: "Ctrl+X" }, { label: "Copy", run: C.copy, keys: "Ctrl+C" }, { label: "Paste", run: C.paste, keys: "Ctrl+V" }, "-",
      { label: "Insert Task", run: () => C.newTask(), keys: "Insert" }, { label: "Insert Milestone", run: () => C.newTask(true) }, { label: "Delete Task", run: C.del, keys: "Delete" }, "-",
      { label: "Indent", run: C.indent, keys: "Alt+Shift+→" }, { label: "Outdent", run: C.outdent, keys: "Alt+Shift+←" }, { label: "Link Tasks", run: C.link, keys: "Ctrl+F2" }, { label: "Unlink Tasks", run: C.unlink }, "-",
      { label: "Task Information…", run: C.info, keys: "Shift+F2" }, { label: "Notes…", run: C.notes }, { label: "Inspect…", run: () => D.inspect(rows()[k]?.uid) }, "-",
      { label: "Mark on Track", run: C.onTrack }, { label: "0% Complete", run: () => C.pct(0) }, { label: "50% Complete", run: () => C.pct(50) }, { label: "100% Complete", run: () => C.pct(100) },
      { label: "Split Task…", run: C.split }, { label: "Inactivate / Activate", run: C.inactive }, { label: "Manually Schedule", run: () => C.mode(true) }, { label: "Auto Schedule", run: () => C.mode(false) },
      { label: "Add to Timeline", run: C.timeline }, { label: "Scroll to Task", run: C.scrollTo },
    ]);
  };

  const headMenu = (e: MouseEvent, k: number) => {
    const c = grid().cols[k];
    const ins = (key: string) => { const v = [...cols()]; v.splice(k, 0, [key, 90]); saveCols(v); };
    openMenu(e, [
      { label: "Insert Column", sub: fieldMenu(ins) }, { label: "Hide Column", run: () => saveCols(cols().filter((_, j) => j !== k)), off: cols().length < 2 },
      { label: "Field Settings…", run: () => D.fieldSettings(k) }, { label: "Custom Fields…", run: () => D.customFields(c?.key.startsWith("c:") ? c.key.slice(2) : undefined) }, "-",
      { label: "Sort A to Z", run: () => setSort([[c.key, false]]), on: sort()[0]?.[0] === c.key && !sort()[0][1] }, { label: "Sort Z to A", run: () => setSort([[c.key, true]]), on: sort()[0]?.[0] === c.key && sort()[0][1] },
      { label: "No Sort", run: () => setSort([]) }, { label: "Group by This Field", run: () => setGroup([c.key]) }, { label: "Filter by This Field…", run: () => D.filterEditor(c.key) }, "-",
      { label: "Best Fit", run: () => { const v = [...cols()]; v[k] = [v[k][0], Math.min(400, Math.max(50, Math.max(c.name.length, ...rows().slice(0, 500).map(r => r.cells[k]?.length ?? 0)) * 7 + 20))]; saveCols(v); } },
      { label: `Table: ${table()}`, sub: [{ label: "Tables…", run: () => D.tablesDialog() }] },
    ]);
  };

  // Column resize and reorder by dragging the header.
  const resize = (e: MouseEvent, k: number) => {
    e.preventDefault(); e.stopPropagation();
    const x0 = e.clientX, w0 = widths()[k];
    const v = [...cols()];
    const mv = (m: MouseEvent) => { v[k] = [v[k][0], Math.max(24, w0 + m.clientX - x0)]; saveColsLocal([...v]); };
    const up = () => { removeEventListener("mousemove", mv); removeEventListener("mouseup", up); saveCols(v); };
    addEventListener("mousemove", mv); addEventListener("mouseup", up);
  };
  const [drag, setDrag] = createSignal<number | null>(null);
  const saveColsLocal = (v: [string, number][]) => saveColsQuiet(v);

  const cell = (r: Row, c: Col, k: number): JSX.Element => {
    const t = r.cells[k] ?? "";
    if (c.key === "mode") return <span class="mode" title={t}>{r.kind === "group" || r.kind === "project" ? "" : t.startsWith("Manual") ? <Icon n="manual" /> : <Icon n="auto" />}</span>;
    const icon = r.icons?.find(x => x[0] === k)?.[1];
    if (c.key === "name") return <>
      <span style={{ width: `${(r.lvl) * 16}px`, display: "inline-block" }} />
      {r.kids ? <b class="tg" onMouseDown={e => { e.stopPropagation(); if (r.uid > 0) C.toggle(r.uid); }}>{r.open ? "◢" : "▷"}</b> : <span class="tg" />}
      {t}
    </>;
    if (icon) return <span class="ico" title={t}>{icon}</span>;
    if (c.kind === "Pct" && c.key === "pct" && r.kind !== "group") return <span class="pctc"><i style={{ width: `${parseFloat(t) || 0}%` }} />{t}</span>;
    return t;
  };

  return (
    <div class="grid" style={{ width: p.width ? `${p.width}px` : undefined }}>
      <div class="ghead" ref={head}>
        <div class="hc idc" style={{ width: "44px" }} onContextMenu={e => openMenu(e, [{ label: "Select All", run: () => setSel(rows().filter(r => r.uid > 0).map(r => r.uid)) }, { label: "Tables…", run: () => D.tablesDialog() }])} />
        <For each={grid().cols}>{(c, k) => (
          <div class="hc" classList={{ r: right(c), drop: drag() !== null && drag() !== k() }} style={{ width: `${widths()[k()]}px` }} title={c.name}
            draggable onDragStart={() => setDrag(k())} onDragEnd={() => setDrag(null)} onDragOver={e => e.preventDefault()}
            onDrop={() => { const f = drag(); if (f === null || f === k()) return; const v = [...cols()]; const [m] = v.splice(f, 1); v.splice(k(), 0, m); saveCols(v); setDrag(null); }}
            onContextMenu={e => headMenu(e, k())} onDblClick={() => D.fieldSettings(k())}>
            <span>{c.name}</span><button class="dd" onClick={e => headMenu(e, k())}><Icon n="chev" /></button><i class="rz" onMouseDown={e => resize(e, k())} />
          </div>
        )}</For>
        <div class="hc add" onClick={e => openMenu(e, fieldMenu(key => saveCols([...cols(), [key, 90]])))}>Add New Column ▾</div>
      </div>
      <div class="gbody" ref={body} tabindex="0" onKeyDown={key}
        onScroll={() => { head.scrollLeft = body.scrollLeft; setScrollTop(body.scrollTop); document.dispatchEvent(new CustomEvent("chart-scroll", { detail: body.scrollTop })); }}>
        <div style={{ height: `${(rows().length + 3) * RH}px`, width: `${total() + 160}px`, position: "relative" }}>
          <For each={visible()}>{(r) => {
            const k = () => rows().indexOf(r);
            return (
              <div class="row" style={{ top: `${k() * RH}px` }}
                classList={{ sum: r.kind === "summary" || r.kind === "project", grp: r.kind === "group", sel: selSet().has(r.uid), cur: cur().row === k(), off: r.off, late: r.late, hl: r.hl, crit: r.crit, proj: r.kind === "project" }}
                onMouseDown={e => { if (e.button === 0) pick(k(), e); }} onContextMenu={e => rowMenu(e, k())}>
                <div class="c idc" style={{ width: "44px" }} title={r.late ? "Late" : ""}>{r.uid > 0 ? r.id : ""}{r.late && <i class="ind late">!</i>}{r.manual && <i class="ind">✎</i>}</div>
                <For each={grid().cols}>{(c, j) => (
                  <div class="c" classList={{ r: right(c), nm: c.key === "name", ro: !c.edit, at: cur().row === k() && cur().col === j() }} style={{ width: `${widths()[j()]}px` }}
                    onMouseDown={e => { if (e.button === 0) { pick(k(), e, j()); e.stopPropagation(); } }} onDblClick={() => c.key === "name" || !c.edit ? C.info() : startEdit(k(), j())}>
                    {cell(r, c, j())}
                  </div>
                )}</For>
              </div>
            );
          }}</For>
          <div class="row blank" style={{ top: `${rows().length * RH}px` }} classList={{ cur: cur().row === rows().length }} onMouseDown={() => pick(rows().length, null)}>
            <div class="c idc" style={{ width: "44px" }} />
            <For each={grid().cols}>{(c, j) => <div class="c" classList={{ at: cur().row === rows().length && cur().col === j() }} style={{ width: `${widths()[j()]}px` }} onDblClick={() => startEdit(rows().length, j())}>{c.key === "name" ? <span class="ph">Type a new task…</span> : ""}</div>}</For>
          </div>
          <Show when={edit()}>{e => {
            const c = grid().cols[e().col];
            const left = 44 + widths().slice(0, e().col).reduce((a, b) => a + b, 0);
            const opts = c && choices(c.key);
            const style = { left: `${left}px`, top: `${e().row * RH}px`, width: `${Math.max(widths()[e().col], c?.key === "name" ? 200 : 90)}px` };
            const keys = (x: KeyboardEvent) => {
              x.stopPropagation();
              if (x.key === "Enter") { x.preventDefault(); commit([1, 0]); }
              else if (x.key === "Tab") { x.preventDefault(); commit([0, x.shiftKey ? -1 : 1]); }
              else if (x.key === "Escape") { setEdit(null); body.focus(); }
            };
            return opts && !e().text.match(/^.$/)
              ? <select class="edit" style={style} value={e().text} ref={el => setTimeout(() => el.focus())} onChange={x => { setEdit({ ...e(), text: x.currentTarget.value }); commit(null); }} onKeyDown={keys} onBlur={() => setEdit(null)}>
                  <For each={opts}>{o => <option value={o}>{o}</option>}</For>
                </select>
              : <input class="edit" style={style} value={e().text} ref={el => setTimeout(() => { el.focus(); if (e().text.length > 1) el.select(); else el.setSelectionRange(1, 1); })}
                  onInput={x => setEdit({ ...e(), text: x.currentTarget.value })} onKeyDown={keys} onBlur={() => commit(null)} />;
          }}</Show>
        </div>
      </div>
    </div>
  );
};

// Live column widths while dragging (saved to the plan on mouse up).
import { setCols } from "./store";
const saveColsQuiet = (v: [string, number][]) => setCols(v);
export const gridWidth = gridW;
