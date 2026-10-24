// The task sheet: columns chosen per plan, inline editing, selection, clipboard, find and row menus.
interface Col { k: string; t: string; w: number; ed: boolean; cls: string }

const gbody = $('#gbody'), grows = $('#grows'), gspace = $('#gspace'), ghead = $('#ghead');
const cols = (): Col[] => [{ k: 'id', t: '', w: 42, ed: false, cls: 'id' }, ...V.columns.filter(([k]) => FIELDS[k]).map(([k, w]) => ({ k, t: FIELDS[k][0], w, ed: FIELDS[k][2], cls: FIELDS[k][3] ?? '' }))];
let COLS: Col[] = [];
const gridW = () => COLS.reduce((a, c) => a + c.w, 0);
const colX = (ci: number) => COLS.slice(0, ci).reduce((a, c) => a + c.w, 0);

function layout() {
  COLS = cols();
  cur.c = Math.min(cur.c, COLS.length - 1);
  const h = (rows.length + EXTRA) * ROW + 'px';
  gspace.style.height = h;
  gspace.style.width = gridW() + 46 + 'px';
  grows.style.width = gridW() + 'px';
  ghead.innerHTML = COLS.map((c, i) => `<div class="hc" style="width:${c.w}px" data-h="${i}">${c.t}<i class="rz" data-c="${i}"></i></div>`).join('')
    + '<div class="hc add" data-h="add" title="Add a column">Add New Column ▾</div>';
  const first = V.rows.length ? V.s : today;
  d0 = Math.floor(Math.min(first, today)) - 14;
  const last = Math.max(V.f, today) + 120;
  cspace.style.height = h;
  cspace.style.width = (last - d0) * pd + 'px';
  renderRows();
  draw();
}

function cellText(r: Row, k: string): string {
  if (k === 'pct') return r.pct + '%';
  if (k === 'crit') return r.crit ? 'Yes' : 'No';
  if (k === 'late') return r.late ? 'Yes' : 'No';
  return String((r as any)[k] ?? '');
}

function cell(r: Row | undefined, c: Col, ci: number, i: number): string {
  let v = '';
  if (r) {
    if (c.k === 'name') {
      const tg = r.sum ? `<b class="tg" data-t="${r.uid}">${collapsed.has(r.uid) ? '▸' : '▾'}</b>` : '<b class="tg"></b>';
      const ic = (r.manual ? '<i class="ind man" title="Manually scheduled">📌</i>' : '') + (r.notes ? '<i class="ind" title="Has notes">✎</i>' : '') + (r.ctype !== 'As Soon As Possible' && !r.sum ? `<i class="ind" title="${esc(r.ctype)} ${esc(r.cdate)}">⧗</i>` : '');
      v = `<span style="padding-left:${r.lvl * 16}px"></span>${tg}${esc(r.name)}${ic}`;
    } else v = esc(cellText(r, c.k));
  }
  const cls = `c ${c.cls}${i === cur.r && ci === cur.c ? ' cur' : ''}`;
  return `<div class="${cls}" style="width:${c.w}px">${v}</div>`;
}

function renderRows() {
  const a = Math.floor(gbody.scrollTop / ROW), b = Math.min(rows.length + EXTRA, a + Math.ceil(gbody.clientHeight / ROW) + 2);
  let html = '';
  for (let i = a; i < b; i++) {
    const r = rows[i];
    const cls = r ? `${sel.has(r.uid) ? ' sel' : ''}${r.sum ? ' sum' : ''}${r.late ? ' late' : ''}${r.off ? ' off' : ''}` : '';
    html += `<div class="row${cls}" style="top:${i * ROW}px;width:${gridW()}px" data-i="${i}">${COLS.map((c, ci) => cell(r, c, ci, i)).join('')}</div>`;
  }
  grows.innerHTML = html;
}

function ensureVisible(i: number) {
  if (i * ROW < gbody.scrollTop) gbody.scrollTop = i * ROW;
  else if ((i + 1) * ROW > gbody.scrollTop + gbody.clientHeight) gbody.scrollTop = (i + 1) * ROW - gbody.clientHeight;
}

function select(i: number, ci: number, e?: { shiftKey: boolean; ctrlKey: boolean }) {
  i = Math.max(0, Math.min(i, rows.length + EXTRA - 1));
  const moved = i !== cur.r;
  cur = { r: i, c: Math.max(0, Math.min(ci, COLS.length - 1)) };
  if (e?.shiftKey) {
    sel.clear();
    for (let k = Math.min(anchor, i); k <= Math.max(anchor, i); k++) if (rows[k]) sel.add(rows[k].uid);
  } else if (e?.ctrlKey && rows[i]) {
    sel.has(rows[i].uid) ? sel.delete(rows[i].uid) : sel.add(rows[i].uid);
    anchor = i;
  } else {
    sel.clear();
    if (rows[i]) sel.add(rows[i].uid);
    anchor = i;
  }
  ensureVisible(i);
  renderRows();
  draw();
  if (moved) renderDetails();
}

function startEdit(init?: string) {
  const c = COLS[cur.c], r = rows[cur.r];
  if (!c.ed) return;
  if (r?.sum && !['name', 'notes', 'code', 'preds', 'active', 'deadline'].includes(c.k)) return toast('Summary tasks are calculated from the tasks under them.');
  ensureVisible(cur.r);
  const inp = document.createElement('input');
  inp.id = 'edit';
  Object.assign(inp.style, { left: colX(cur.c) + 'px', top: cur.r * ROW + 'px', width: Math.max(c.w, 120) + 'px' });
  const list = choices(c.k);
  if (list) {
    let dl = $<HTMLDataListElement>('#dl-edit');
    if (!dl) { dl = document.createElement('datalist'); dl.id = 'dl-edit'; document.body.appendChild(dl); }
    dl.innerHTML = list.map(o => `<option value="${esc(o)}">`).join('');
    inp.setAttribute('list', 'dl-edit');
  }
  const value = !r ? '' : c.k === 'start' && !r.manual ? (r.cdate && r.ctype === 'Start No Earlier Than' ? r.start : '') : c.k === 'pct' ? String(r.pct) : cellText(r, c.k);
  inp.value = init ?? value;
  inp.dataset.orig = value;
  gbody.appendChild(inp);
  inp.focus();
  if (init === undefined) inp.select();
  editor = inp;
  inp.onkeydown = e => {
    e.stopPropagation();
    if (e.key === 'Enter') { e.preventDefault(); commit(1, 0); }
    else if (e.key === 'Tab') { e.preventDefault(); commit(0, e.shiftKey ? -1 : 1); }
    else if (e.key === 'Escape') { const x = editor; editor = null; x?.remove(); gbody.focus(); }
  };
  inp.onblur = () => { if (editor === inp) commit(0, 0); };
}

async function commit(dr: number, dc: number) {
  const inp = editor;
  if (!inp) return;
  editor = null;
  const c = COLS[cur.c], r = rows[cur.r], v = inp.value;
  inp.remove();
  if (r && v !== inp.dataset.orig) await set(r.uid, c.k, v);
  else if (!r && v.trim()) {
    if (c.k !== 'name') toast('Type the task name first.');
    else await op({ InsertRows: { at: V.rows.length, rows: [[['name', v]]] } });
  }
  let ci = cur.c + dc;
  while (dc && COLS[ci] && !COLS[ci].ed) ci += dc;
  select(cur.r + dr, COLS[ci] ? ci : cur.c);
  gbody.focus();
}

// ---------- columns ----------
const saveCols = (c: [string, number][]) => op({ Columns: { cols: c } });
function fieldPicker(x: number, y: number, at: number) {
  const have = new Set(V.columns.map(c => c[0]));
  const free = Object.keys(FIELDS).filter(k => !have.has(k)).sort((a, b) => FIELDS[a][0].localeCompare(FIELDS[b][0]));
  if (!free.length) return toast('Every field is already shown.');
  menu(free.map(k => [FIELDS[k][0], () => { const c = [...V.columns]; c.splice(at, 0, [k, FIELDS[k][1]]); saveCols(c); }] as MenuItem), x, y);
}
function headerMenu(ci: number, x: number, y: number) {
  const k = ci - 1, c = V.columns;
  menu([
    ['Insert Column…', () => fieldPicker(x, y, Math.max(0, k))],
    ['Hide Column', () => saveCols(c.filter((_, i) => i !== k)), k >= 0 && c[k]?.[0] !== 'name'],
    '-',
    ['Move Left', () => { const n = [...c]; [n[k - 1], n[k]] = [n[k], n[k - 1]]; saveCols(n); }, k > 0],
    ['Move Right', () => { const n = [...c]; [n[k + 1], n[k]] = [n[k], n[k + 1]]; saveCols(n); }, k >= 0 && k < c.length - 1],
    '-',
    ['Reset Columns', () => saveCols([['name', 250], ['dur', 78], ['start', 86], ['finish', 86], ['preds', 100], ['res', 140], ['pct', 78]])],
  ], x, y);
}

// ---------- row commands ----------
function insert(ms: boolean) {
  op({ Insert: { at: insertAt(), name: ms ? 'Milestone' : 'New Task', ms } }).then(v => v && select(Math.min(cur.r, rows.length - 1), 1));
}
async function del() {
  const u = picked();
  if (!u.length) return;
  sel.clear();
  await op({ Delete: { uids: u } });
}
const pct = (p: number) => {
  const u = rows.filter(r => !r.sum && picked().includes(r.uid));
  if (u.length) op({ Batch: u.map(r => ({ Set: { uid: r.uid, field: 'pct', value: String(p) } })) });
};
const moveRows = (up: boolean) => op({ Move: { uids: picked(), up } }).then(v => { if (v) { const i = rows.findIndex(r => r.uid === picked()[0]); if (i >= 0) select(i, cur.c, { shiftKey: false, ctrlKey: false }); } });
const toggle = (field: string, onOff: (r: Row) => boolean) => {
  const rs = rows.filter(r => picked().includes(r.uid));
  if (rs.length) { const v = onOff(rs[0]) ? 'No' : 'Yes'; op({ Batch: rs.map(r => ({ Set: { uid: r.uid, field, value: field === 'mode' ? (v === 'Yes' ? 'Manual' : 'Auto') : v } })) }); }
};
function fillDown() {
  const rs = rows.filter(r => picked().includes(r.uid)), c = COLS[cur.c];
  if (rs.length < 2 || !c.ed) return toast('Select two or more rows in an editable column to fill down.');
  const v = c.k === 'pct' ? String(rs[0].pct) : cellText(rs[0], c.k);
  op({ Batch: rs.slice(1).filter(r => !r.sum).map(r => ({ Set: { uid: r.uid, field: c.k, value: v } })) });
}

// ---------- clipboard: rows go to the core; text goes to Excel and other apps ----------
function copyRows(cut: boolean, e?: ClipboardEvent) {
  const u = picked();
  if (!u.length) return;
  const rs = V.rows.filter(r => u.includes(r.uid));
  const text = [V.columns.map(c => FIELDS[c[0]][0]).join('\t'), ...rs.map(r => V.columns.map(c => cellText(r, c[0]).replace(/[\t\n]/g, ' ')).join('\t'))].join('\n');
  if (e?.clipboardData) { e.clipboardData.setData('text/plain', `⁣DATUM⁣\n${text}`); e.preventDefault(); }
  inv<number>('copy', { uids: u }).then(n => { toast(`${n} task${n === 1 ? '' : 's'} ${cut ? 'cut' : 'copied'}.`, true); if (cut) del(); });
}
async function pasteText(text: string) {
  const at = insertAt();
  if (text.startsWith('⁣DATUM') && V.clip) return run('paste', { at });
  const lines = text.replace(/\r/g, '').split('\n').filter(l => l.trim());
  if (!lines.length) return;
  const cells = lines.map(l => l.split('\t'));
  const r = curRow();
  if (cells.length === 1 && cells[0].length === 1 && r && COLS[cur.c].ed) return set(r.uid, COLS[cur.c].k, cells[0][0].trim());
  // Rows from a spreadsheet: a header row naming fields is used when present, otherwise the visible columns from the cursor.
  const byTitle = (h: string) => Object.keys(FIELDS).find(k => FIELDS[k][0].toLowerCase() === h.trim().toLowerCase());
  const head = cells[0].map(byTitle);
  const keys = head.some(Boolean) ? (cells.shift(), head) : COLS.slice(Math.max(1, cur.c)).map(c => c.k);
  const out = cells.map(cs => cs.map((v, i) => [keys[i], v.trim()]).filter(([k, v]) => k && FIELDS[k!][2] && v) as [string, string][]).filter(x => x.some(([k]) => k === 'name'));
  if (!out.length) return toast('Nothing to paste: include a Task Name column.');
  await op({ InsertRows: { at, rows: out } });
}
document.addEventListener('copy', e => { if (!editor && !document.querySelector('.ov') && !$('#ganttView').hidden && document.activeElement === gbody) copyRows(false, e); });
document.addEventListener('cut', e => { if (!editor && !document.querySelector('.ov') && !$('#ganttView').hidden && document.activeElement === gbody) copyRows(true, e); });
document.addEventListener('paste', e => {
  if (editor || document.querySelector('.ov') || $('#ganttView').hidden || document.activeElement !== gbody) return;
  e.preventDefault();
  pasteText(e.clipboardData?.getData('text/plain') ?? '');
});

// ---------- find and replace ----------
let lastFind = '';
async function find(replace: boolean) {
  const b = await dialog(replace ? 'Replace' : 'Find', `<div class="form">
    <label>Find what</label><input id="f-what" value="${esc(lastFind)}">
    ${replace ? '<label>Replace with</label><input id="f-with">' : ''}
    <label>Look in</label><span>Task names, activity IDs, resources and notes</span></div>`, replace ? ['Replace All', 'Cancel'] : ['Find Next', 'Cancel'], '#f-what');
  if (b !== 0) return;
  lastFind = val('f-what');
  if (!lastFind) return;
  const q = lastFind.toLowerCase(), hit = (r: Row) => [r.name, r.code, r.res, r.notes].some(s => s.toLowerCase().includes(q));
  if (replace) {
    const w = val('f-with'), re = new RegExp(lastFind.replace(/[.*+?^${}()|[\]\\]/g, '\\$&'), 'gi');
    const ops = V.rows.filter(r => re.test(r.name) && ((re.lastIndex = 0), true)).map(r => ({ Set: { uid: r.uid, field: 'name', value: r.name.replace(re, w) } }));
    if (!ops.length) return toast(`"${lastFind}" was not found in task names.`);
    await op({ Batch: ops });
    return toast(`Replaced in ${ops.length} task name${ops.length === 1 ? '' : 's'}.`, true);
  }
  findNext(hit);
}
function findNext(hit?: (r: Row) => boolean) {
  const q = lastFind.toLowerCase();
  hit ??= (r: Row) => [r.name, r.code, r.res, r.notes].some(s => s.toLowerCase().includes(q));
  if (!q) return find(false);
  const n = V.rows.length, start = V.rows.findIndex(r => r.uid === curRow()?.uid);
  for (let k = 1; k <= n; k++) {
    const r = V.rows[(start + k + n) % n];
    if (hit(r)) {
      collapsed.clear(); rows = visible(); layout();
      return select(rows.findIndex(x => x.uid === r.uid), cur.c);
    }
  }
  toast(`"${lastFind}" was not found.`);
}

function rowMenu(x: number, y: number) {
  const r = curRow();
  menu([
    ['Insert Task', () => insert(false), true, 'Insert'], ['Insert Milestone', () => insert(true)], '-',
    ['Cut', () => copyRows(true), !!r, 'Ctrl+X'], ['Copy', () => copyRows(false), !!r, 'Ctrl+C'], ['Paste', () => run('paste', { at: insertAt() }), V.clip > 0, 'Ctrl+V'], '-',
    ['Indent', () => op({ Indent: { uids: picked() } }), !!r, 'Alt+Shift+→'], ['Outdent', () => op({ Outdent: { uids: picked() } }), !!r, 'Alt+Shift+←'],
    ['Move Up', () => moveRows(true), !!r, 'Alt+Shift+↑'], ['Move Down', () => moveRows(false), !!r, 'Alt+Shift+↓'], '-',
    ['Link Tasks', () => op({ Link: { uids: picked() } }), picked().length > 1, 'Ctrl+F2'], ['Unlink Tasks', () => op({ Unlink: { uids: picked() } }), !!r, 'Ctrl+Shift+F2'], '-',
    [r?.tl ? 'Remove from Timeline' : 'Add to Timeline', () => toggle('timeline', x => x.tl), !!r],
    [r?.manual ? 'Auto Schedule' : 'Manually Schedule', () => toggle('mode', x => x.manual), !!r && !r.sum],
    [r?.off ? 'Activate Task' : 'Inactivate Task', () => toggle('active', x => !x.off), !!r],
    ['Task Details', () => openDetails('general'), !!r, 'Shift+F2'], '-',
    ['Delete Task', del, !!r, 'Delete'],
  ], x, y);
}

// ---------- grid events ----------
grows.onmousedown = e => {
  const el = e.target as HTMLElement, rowEl = el.closest('.row') as HTMLElement | null;
  if (!rowEl) return;
  if (el.dataset.t) { const u = +el.dataset.t; collapsed.has(u) ? collapsed.delete(u) : collapsed.add(u); rows = visible(); layout(); return; }
  const ci = [...rowEl.children].indexOf(el.closest('.c') as Element);
  if (editor) commit(0, 0);
  const i = +rowEl.dataset.i!;
  if (e.button === 2 && rows[i] && sel.has(rows[i].uid)) { cur.c = ci < 0 ? cur.c : ci; return; }
  select(i, ci < 0 ? cur.c : ci, e);
  gbody.focus();
  e.preventDefault();
};
grows.ondblclick = e => { if ((e.target as HTMLElement).closest('.c.id')) openDetails('general'); else if (COLS[cur.c].ed) startEdit(); };
grows.oncontextmenu = e => { e.preventDefault(); rowMenu(e.clientX, e.clientY); };
ghead.onmousedown = e => {
  const t = e.target as HTMLElement, c = t.dataset.c;
  if (c !== undefined && e.button === 0) {
    const col = COLS[+c], x = e.clientX, w0 = col.w;
    const mv = (m: MouseEvent) => { col.w = Math.max(30, w0 + m.clientX - x); ghead.querySelectorAll<HTMLElement>('.hc')[+c].style.width = col.w + 'px'; grows.style.width = gridW() + 'px'; renderRows(); };
    document.addEventListener('mousemove', mv);
    document.addEventListener('mouseup', () => {
      document.removeEventListener('mousemove', mv);
      if (+c > 0) saveCols(V.columns.map((x, i) => i === +c - 1 ? [x[0], col.w] : x));
    }, { once: true });
    return;
  }
  const h = (t.closest('[data-h]') as HTMLElement | null)?.dataset.h;
  if (h === 'add') fieldPicker(e.clientX, e.clientY + 8, V.columns.length);
};
ghead.oncontextmenu = e => {
  e.preventDefault();
  const h = (e.target as HTMLElement).closest('[data-h]') as HTMLElement | null;
  if (h && h.dataset.h !== 'add') headerMenu(+h.dataset.h!, e.clientX, e.clientY);
};
