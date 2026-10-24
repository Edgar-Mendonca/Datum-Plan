// Spreadsheet tabs inside the plan (quantities, estimates, progress measurement). Cells take
// Excel formulas and can read the tasks through the "Plan" sheet, e.g. =SUM(Plan!M:M).
import { For, Show, createSignal, createResource, createMemo, onMount, onCleanup } from "solid-js";
import { SheetOut } from "./api";
import { act, call, P, tick, openMenu, toast } from "./store";

const RH = 22, CW = 96, HW = 46;
const colName = (c: number) => { let s = ""; c += 1; while (c > 0) { const m = (c - 1) % 26; s = String.fromCharCode(65 + m) + s; c = Math.floor((c - 1) / 26); } return s; };
const a1 = (r: number, c: number) => `${colName(c)}${r + 1}`;
const parseA1 = (s: string): [number, number] => { const m = s.match(/^([A-Z]+)(\d+)$/)!; let c = 0; for (const ch of m[1]) c = c * 26 + ch.charCodeAt(0) - 64; return [+m[2] - 1, c - 1]; };

export const Sheets = () => {
  const [k, setK] = createSignal(0);
  const [data] = createResource(() => [tick(), k(), P().sheets.length], () => (P().sheets.length ? call<SheetOut>("sheet_view", { k: Math.min(k(), P().sheets.length - 1) }) : undefined));
  const cells = createMemo(() => new Map((data()?.cells ?? []).map(c => [c[0], c])));
  const [cur, setCur] = createSignal<[number, number]>([0, 0]);
  const [end, setEnd] = createSignal<[number, number]>([0, 0]);
  const [edit, setEdit] = createSignal<string | null>(null);
  const [top, setTop] = createSignal(0);
  const [h, setH] = createSignal(600);
  const nrows = () => Math.max(200, (data()?.rows ?? 0) + 30), ncols = () => Math.max(26, (data()?.cols ?? 0) + 6);
  const width = (c: number) => P().sheets[k()]?.widths?.[c] ?? CW;
  const lefts = createMemo(() => { let x = HW; return Array.from({ length: ncols() }, (_, c) => { const l = x; x += width(c); return l; }); });
  let box!: HTMLDivElement;
  const ro = new ResizeObserver(() => setH(box.clientHeight));
  onMount(() => box && ro.observe(box));
  onCleanup(() => ro.disconnect());
  const range = () => { const [a, b] = cur(), [c, d] = end(); return [Math.min(a, c), Math.min(b, d), Math.max(a, c), Math.max(b, d)]; };
  const inR = (r: number, c: number) => { const [r0, c0, r1, c1] = range(); return r >= r0 && r <= r1 && c >= c0 && c <= c1; };
  const put = async (list: [string, string][]) => { await act({ SheetCells: { k: k(), cells: list, fmt: null } }); };
  const commit = async (move: [number, number]) => {
    const v = edit();
    setEdit(null);
    if (v !== null && v !== (cells().get(a1(...cur()))?.[1] ?? "")) await put([[a1(...cur()), v]]);
    go(cur()[0] + move[0], cur()[1] + move[1]);
    box.focus();
  };
  const go = (r: number, c: number, ext = false) => {
    r = Math.max(0, r); c = Math.max(0, Math.min(ncols() - 1, c));
    if (ext) setEnd([r, c]); else { setCur([r, c]); setEnd([r, c]); }
    if (r * RH < box.scrollTop) box.scrollTop = r * RH; else if ((r + 2) * RH > box.scrollTop + box.clientHeight - 24) box.scrollTop = (r + 2) * RH - box.clientHeight + 24;
  };
  const clear = () => { const [r0, c0, r1, c1] = range(); const out: [string, string][] = []; for (let r = r0; r <= r1; r++) for (let c = c0; c <= c1; c++) if (cells().has(a1(r, c))) out.push([a1(r, c), ""]); if (out.length) put(out); };
  const key = (e: KeyboardEvent) => {
    if (edit() !== null) return;
    const [r, c] = e.shiftKey ? end() : cur();
    const mv = (dr: number, dc: number) => { e.preventDefault(); go(r + dr, c + dc, e.shiftKey); };
    switch (e.key) {
      case "ArrowDown": return mv(1, 0); case "ArrowUp": return mv(-1, 0); case "ArrowLeft": return mv(0, -1); case "ArrowRight": return mv(0, 1);
      case "Tab": e.preventDefault(); return go(cur()[0], cur()[1] + (e.shiftKey ? -1 : 1));
      case "Enter": case "F2": e.preventDefault(); return setEdit(cells().get(a1(...cur()))?.[1] ?? "");
      case "Delete": case "Backspace": e.preventDefault(); return clear();
    }
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "a") { e.preventDefault(); setCur([0, 0]); setEnd([nrows() - 1, ncols() - 1]); return; }
    if (!e.ctrlKey && !e.metaKey && !e.altKey && e.key.length === 1) { e.preventDefault(); setEdit(e.key); }
  };
  const onCopy = (e: ClipboardEvent) => {
    if (edit() !== null) return;
    e.preventDefault();
    const [r0, c0, r1, c1] = range();
    const lines = [];
    for (let r = r0; r <= r1; r++) { const l = []; for (let c = c0; c <= c1; c++) l.push(cells().get(a1(r, c))?.[2] ?? ""); lines.push(l.join("\t")); }
    e.clipboardData?.setData("text/plain", lines.join("\n"));
  };
  const onPaste = (e: ClipboardEvent) => {
    if (edit() !== null) return;
    e.preventDefault();
    const rows = (e.clipboardData?.getData("text/plain") ?? "").replace(/\r/g, "").split("\n").filter((l, i, a) => l || i < a.length - 1).map(l => l.split("\t"));
    const [r0, c0] = cur();
    put(rows.flatMap((l, i) => l.map((v, j) => [a1(r0 + i, c0 + j), v] as [string, string])));
  };
  const down = (e: MouseEvent) => {
    if (e.button !== 0) return;
    const b = box.getBoundingClientRect();
    const y = e.clientY - b.top + box.scrollTop - RH, x = e.clientX - b.left + box.scrollLeft;
    const r = Math.floor(y / RH), c = lefts().findIndex((l, j) => x >= l && x < l + width(j));
    if (r < 0 || c < 0) return;
    if (edit() !== null) commit([0, 0]);
    go(r, c, e.shiftKey);
    const mv = (m: MouseEvent) => { const y2 = m.clientY - b.top + box.scrollTop - RH, x2 = m.clientX - b.left + box.scrollLeft; const c2 = lefts().findIndex((l, j) => x2 >= l && x2 < l + width(j)); setEnd([Math.max(0, Math.floor(y2 / RH)), c2 < 0 ? end()[1] : c2]); };
    const up = () => { removeEventListener("mousemove", mv); removeEventListener("mouseup", up); };
    addEventListener("mousemove", mv); addEventListener("mouseup", up);
  };
  const setFmt = (f: string) => { const [r0, c0, r1, c1] = range(); const out: [string, string][] = []; for (let r = r0; r <= r1; r++) for (let c = c0; c <= c1; c++) out.push([a1(r, c), ""]); act({ SheetCells: { k: k(), cells: out, fmt: f } }); };
  const addSheet = () => act({ Sheet: { k: null, sheet: { name: `Sheet${P().sheets.length + 1}`, cells: {}, fmt: {}, widths: {} } } }).then(() => setK(P().sheets.length - 1));
  const tabMenu = (e: MouseEvent, i: number) => openMenu(e, [
    { label: "Rename…", run: () => { const n = prompt("Sheet name", P().sheets[i].name); if (n?.trim()) act({ Sheet: { k: i, sheet: { ...P().sheets[i], name: n.trim() } } }); } },
    { label: "Delete Sheet", run: () => { if (confirm(`Delete ${P().sheets[i].name}?`)) act({ Sheet: { k: i, sheet: null } }).then(() => setK(0)); } },
  ]);
  const vis = createMemo(() => { const f = Math.max(0, Math.floor(top() / RH) - 2); return Array.from({ length: Math.ceil(h() / RH) + 6 }, (_, j) => f + j).filter(r => r < nrows()); });
  const curCell = () => cells().get(a1(...cur()));
  return (
    <section class="ganttview">
      <div class="ptools" style={{ padding: "6px 10px", "border-bottom": "1px solid var(--line)" }}>
        <b>Sheets</b><span class="muted">Excel formulas; read tasks with Plan!A:R, e.g. =SUMIF(Plan!I:I,100,Plan!M:M)</span><span class="grow" />
        <select onChange={e => { setFmt(e.currentTarget.value); e.currentTarget.value = "x"; }}><option value="x">Number format…</option><option value="">General</option><option value="0">0</option><option value="0.00">0.00</option><option value="pct">Percent</option><option value="money">Currency</option><option value="date">Date</option></select>
        <button class="btn sm" onClick={addSheet}>+ Sheet</button>
      </div>
      <Show when={P().sheets.length} fallback={<div class="page"><p class="empty">No sheets yet. Sheets are spreadsheets inside the plan for quantities, estimates or progress measurement.</p><button class="btn pri" style={{ width: "fit-content" }} onClick={addSheet}>Add a sheet</button></div>}>
        <div class="fbar"><span class="fname">{a1(...cur())}</span><span class="fx" style={{ "text-align": "center", "line-height": "27px" }}>fx</span>
          <input value={edit() ?? curCell()?.[1] ?? ""} onInput={e => setEdit(e.currentTarget.value)} onKeyDown={e => { if (e.key === "Enter") commit([1, 0]); if (e.key === "Escape") setEdit(null); }} /></div>
        <div class="gscroll sheetgrid" ref={el => { box = el; ro.observe(el); }} tabindex="0" onKeyDown={key} onMouseDown={down} onCopy={onCopy} onPaste={onPaste} onScroll={() => setTop(box.scrollTop)}
          onDblClick={() => setEdit(curCell()?.[1] ?? "")}>
          <div class="gsheet" style={{ width: `${lefts()[ncols() - 1] + width(ncols() - 1) + 40}px`, height: `${(nrows() + 1) * RH}px` }}>
            <div class="ghead" style={{ height: `${RH}px` }}>
              <div class="hc idc" style={{ width: `${HW}px`, position: "sticky", left: "0", "z-index": 2 }} />
              <For each={Array.from({ length: ncols() }, (_, c) => c)}>{c => <div class="hc colh" classList={{ on: inR(cur()[0], c) || (range()[1] <= c && c <= range()[3]) }} style={{ width: `${width(c)}px`, "justify-content": "center", "align-items": "center" }}>{colName(c)}</div>}</For>
            </div>
            <div class="gbody">
              <For each={vis()}>{r => (
                <div class="row" style={{ top: `${r * RH}px`, height: `${RH}px` }}>
                  <div class="c idc" style={{ width: `${HW}px`, position: "sticky", left: "0", "z-index": 1, "text-align": "center" }}>{r + 1}</div>
                  <For each={Array.from({ length: ncols() }, (_, c) => c)}>{c => {
                    const x = () => cells().get(a1(r, c));
                    return <div class="c" classList={{ r: !!x()?.[3], in: inR(r, c), at: cur()[0] === r && cur()[1] === c, err: !!x()?.[2].startsWith("#") }} style={{ width: `${width(c)}px`, "line-height": `${RH - 1}px` }}>{x()?.[2] ?? ""}</div>;
                  }}</For>
                </div>
              )}</For>
              <Show when={edit() !== null}>
                <input class="edit" style={{ left: `${lefts()[cur()[1]]}px`, top: `${cur()[0] * RH}px`, width: `${Math.max(width(cur()[1]), 160)}px`, height: `${RH}px` }} value={edit()!}
                  ref={el => setTimeout(() => { el.focus(); el.setSelectionRange(el.value.length, el.value.length); })} onMouseDown={e => e.stopPropagation()}
                  onInput={e => setEdit(e.currentTarget.value)} onBlur={() => edit() !== null && commit([0, 0])}
                  onKeyDown={e => { e.stopPropagation(); if (e.key === "Enter") { e.preventDefault(); commit([e.shiftKey ? -1 : 1, 0]); } else if (e.key === "Tab") { e.preventDefault(); commit([0, e.shiftKey ? -1 : 1]); } else if (e.key === "Escape") { setEdit(null); box.focus(); } }} />
              </Show>
            </div>
          </div>
        </div>
      </Show>
      <div class="sheettabs">
        <For each={P().sheets}>{(s, i) => <button classList={{ on: i() === k() }} onClick={() => setK(i())} onContextMenu={e => { e.preventDefault(); tabMenu(e, i()); }} onDblClick={() => tabMenu(new MouseEvent("click", { clientX: 200, clientY: innerHeight - 80 }), i())}>{s.name}</button>}</For>
        <button onClick={addSheet} title="New sheet">＋</button>
      </div>
    </section>
  );
};
export { parseA1, toast };
