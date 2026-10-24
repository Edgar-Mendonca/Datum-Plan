// Datum Plan window. The Rust core owns the plan and computes every date; this file
// draws what it returns and sends edits back as operations.

type LinkT = 'FS' | 'SS' | 'FF' | 'SF';
interface Row {
  uid: number; id: number; name: string; lvl: number; sum: boolean; ms: boolean; dur: string; start: string; finish: string;
  s: number; f: number; preds: string; links: [number, LinkT][]; pct: number; tf: string; ff: string; crit: boolean; snet: string; notes: string;
}
interface View {
  name: string; start: string; startIso: string; finish: string; s: number; f: number; week: boolean[]; rows: Row[];
  error: string | null; path: string | null; dirty: boolean; undo: boolean; redo: boolean;
}
interface Recent { path: string; name: string; modified: string }
interface Upd { current: string; latest: string | null; available: boolean; notes: string; published: string; url: string | null; sums: string | null; size: number; page: string }
interface Tpl { id: string; name: string; desc: string; color: string; tasks: number; days: number; bars: [number, number, number][] }
interface Col { k: keyof Row; t: string; w: number; ed?: string; cls?: string }

const TAURI = (window as any).__TAURI__;
const inv = <T>(cmd: string, args?: object): Promise<T> => TAURI.core.invoke(cmd, args);
const $ = <E extends HTMLElement = HTMLElement>(s: string) => document.querySelector(s) as E;
const esc = (s: string) => s.replace(/[&<>"]/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[c]!));
const DAY = 864e5, ROW = 24, EXTRA = 40;
const today = Math.floor((Date.now() - new Date().getTimezoneOffset() * 6e4) / DAY);

const COLS: Col[] = [
  { k: 'id', t: '', w: 42, cls: 'id' },
  { k: 'name', t: 'Task Name', w: 250, ed: 'name', cls: 'nm' },
  { k: 'dur', t: 'Duration', w: 78, ed: 'dur' },
  { k: 'start', t: 'Start', w: 86, ed: 'start' },
  { k: 'finish', t: 'Finish', w: 86, ed: 'finish' },
  { k: 'preds', t: 'Predecessors', w: 104, ed: 'preds' },
  { k: 'pct', t: '% Complete', w: 80, ed: 'pct', cls: 'r' },
  { k: 'tf', t: 'Total Float', w: 78, cls: 'r ro' },
];

let V: View;
let rows: Row[] = [];
let version = '';
let upd: Upd | null = null, updErr = '', updPct = -1;
const collapsed = new Set<number>();
const sel = new Set<number>();
let cur = { r: 0, c: 1 }, anchor = 0;
let pd = 16, d0 = 0, showCrit = true, tab = 'task';
let editor: HTMLInputElement | null = null;
let view = 'gantt', tpls: Tpl[] = [];
const endDay = (r: Row) => r.ms ? r.s : r.f - 1;
const isLate = (r: Row) => !r.sum && r.pct < 100 && endDay(r) < today;

const gbody = $('#gbody'), grows = $('#grows'), gspace = $('#gspace'), ghead = $('#ghead');
const cbody = $('#cbody'), cspace = $('#cspace');
const scaleCv = $<HTMLCanvasElement>('#scale'), barsCv = $<HTMLCanvasElement>('#bars');

// ---------- talking to the core ----------
async function call<T>(cmd: string, args?: object): Promise<T | null> {
  try { return await inv<T>(cmd, args); } catch (e) { toast(String(e)); return null; }
}
async function run(cmd: string, args?: object) {
  const v = await call<View | null>(cmd, args);
  if (v) show(v);
  return v;
}
const op = (o: object) => run('apply', { op: o });

function show(v: View) {
  V = v;
  rows = visible();
  const t = `${v.name}${v.dirty ? ' *' : ''} - Datum Plan`;
  $('#title').textContent = t;
  TAURI.window.getCurrentWindow().setTitle(t).catch(() => {});
  $<HTMLButtonElement>('[data-a=undo]').disabled = !v.undo;
  $<HTMLButtonElement>('[data-a=redo]').disabled = !v.redo;
  const n = v.rows.filter(r => !r.sum).length;
  $('#stInfo').textContent = v.rows.length ? `${n} tasks · Start ${v.start} · Finish ${v.finish}` : 'Type a task name to begin';
  setStatus(v.error ?? 'Ready', !!v.error);
  cur.r = Math.min(cur.r, rows.length + EXTRA - 1);
  layout();
  if (!$('#page').hidden) renderPage();
}

function visible(): Row[] {
  const out: Row[] = [];
  let hide = -1;
  for (const r of V.rows) {
    if (hide >= 0 && r.lvl > hide) continue;
    hide = -1;
    out.push(r);
    if (r.sum && collapsed.has(r.uid)) hide = r.lvl;
  }
  return out;
}

function picked(): number[] {
  const u = rows.filter(r => sel.has(r.uid)).map(r => r.uid);
  if (u.length) return u;
  const r = rows[cur.r];
  return r ? [r.uid] : [];
}

// ---------- grid ----------
const gridW = () => COLS.reduce((a, c) => a + c.w, 0);
const colX = (ci: number) => COLS.slice(0, ci).reduce((a, c) => a + c.w, 0);

function layout() {
  const h = (rows.length + EXTRA) * ROW + 'px';
  gspace.style.height = h;
  gspace.style.width = gridW() + 'px';
  grows.style.width = gridW() + 'px';
  ghead.innerHTML = COLS.map((c, i) => `<div class="hc" style="width:${c.w}px">${c.t}<i class="rz" data-c="${i}"></i></div>`).join('') + '<div class="hc" style="width:40px;border:0"></div>';
  const first = V.rows.length ? V.s : today;
  d0 = Math.min(first, today) - 14;
  const last = Math.max(V.f, today) + 120;
  cspace.style.height = h;
  cspace.style.width = (last - d0) * pd + 'px';
  renderRows();
  draw();
}

function cell(r: Row | undefined, c: Col, ci: number, i: number): string {
  let v = '';
  if (r) {
    if (c.k === 'name') {
      const tg = r.sum ? `<b class="tg" data-t="${r.uid}">${collapsed.has(r.uid) ? '▸' : '▾'}</b>` : '<b class="tg"></b>';
      v = `<span style="padding-left:${r.lvl * 16}px"></span>${tg}${esc(r.name)}${r.notes ? '<span class="note" title="Has notes">✎</span>' : ''}`;
    } else if (c.k === 'pct') v = r.pct + '%';
    else v = esc(String(r[c.k]));
  }
  const cls = `c ${c.cls ?? ''}${i === cur.r && ci === cur.c ? ' cur' : ''}`;
  return `<div class="${cls}" style="width:${c.w}px">${v}</div>`;
}

function renderRows() {
  const a = Math.floor(gbody.scrollTop / ROW), b = Math.min(rows.length + EXTRA, a + Math.ceil(gbody.clientHeight / ROW) + 2);
  let html = '';
  for (let i = a; i < b; i++) {
    const r = rows[i];
    html += `<div class="row${r && sel.has(r.uid) ? ' sel' : ''}${r && isLate(r) ? ' late' : ''}${r?.sum ? ' sum' : ''}" style="top:${i * ROW}px;width:${gridW()}px" data-i="${i}">${COLS.map((c, ci) => cell(r, c, ci, i)).join('')}</div>`;
  }
  grows.innerHTML = html;
}

function ensureVisible(i: number) {
  if (i * ROW < gbody.scrollTop) gbody.scrollTop = i * ROW;
  else if ((i + 1) * ROW > gbody.scrollTop + gbody.clientHeight) gbody.scrollTop = (i + 1) * ROW - gbody.clientHeight;
}

function select(i: number, ci: number, e?: { shiftKey: boolean; ctrlKey: boolean }) {
  i = Math.max(0, Math.min(i, rows.length + EXTRA - 1));
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
}

function startEdit(init?: string) {
  const c = COLS[cur.c], r = rows[cur.r];
  if (!c.ed) return;
  if (r?.sum && c.ed !== 'name') return toast('Summary tasks are calculated from the tasks under them.');
  ensureVisible(cur.r);
  const inp = document.createElement('input');
  inp.id = 'edit';
  Object.assign(inp.style, { left: colX(cur.c) + 'px', top: cur.r * ROW + 'px', width: c.w + 'px' });
  const val = !r ? '' : c.ed === 'start' ? (r.snet ? r.start : '') : c.ed === 'pct' ? String(r.pct) : String(r[c.k]);
  inp.value = init ?? val;
  inp.dataset.orig = val;
  gbody.appendChild(inp);
  inp.focus();
  if (init === undefined) inp.select();
  editor = inp;
  inp.onkeydown = e => {
    e.stopPropagation();
    if (e.key === 'Enter') { e.preventDefault(); commit(1, 0); }
    else if (e.key === 'Tab') { e.preventDefault(); commit(0, e.shiftKey ? -1 : 1); }
    else if (e.key === 'Escape') { cancelEdit(); }
  };
  inp.onblur = () => { if (editor === inp) commit(0, 0); };
}

function cancelEdit() {
  const inp = editor;
  editor = null;
  inp?.remove();
  gbody.focus();
}

async function commit(dr: number, dc: number) {
  const inp = editor;
  if (!inp) return;
  editor = null;
  const c = COLS[cur.c], r = rows[cur.r], val = inp.value;
  inp.remove();
  if (r && val !== inp.dataset.orig) await op({ Set: { uid: r.uid, field: c.ed, value: val } });
  else if (!r && val.trim()) {
    if (c.ed !== 'name') toast('Type the task name first.');
    else await op({ Insert: { at: V.rows.length, name: val, ms: false } });
  }
  let ci = cur.c + dc;
  while (dc && COLS[ci] && !COLS[ci].ed) ci += dc;
  select(cur.r + dr, COLS[ci] ? ci : cur.c);
  gbody.focus();
}

// ---------- Gantt ----------
const dowMon = (d: number) => (new Date(d * DAY).getUTCDay() + 6) % 7;
const ymd = (d: number) => { const t = new Date(d * DAY); return [t.getUTCFullYear(), t.getUTCMonth(), t.getUTCDate()]; };
const MON = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'];
type Unit = 'day' | 'week' | 'month' | 'year';
function floorU(d: number, u: Unit): number {
  const [y, m] = ymd(d);
  return u === 'day' ? d : u === 'week' ? d - dowMon(d) : u === 'month' ? Date.UTC(y, m, 1) / DAY : Date.UTC(y, 0, 1) / DAY;
}
function nextU(d: number, u: Unit): number {
  const [y, m] = ymd(d);
  return u === 'day' ? d + 1 : u === 'week' ? d + 7 : u === 'month' ? Date.UTC(y, m + 1, 1) / DAY : Date.UTC(y + 1, 0, 1) / DAY;
}
function label(d: number, u: Unit, top: boolean): string {
  const [y, m, day] = ymd(d);
  if (u === 'day') return pd >= 26 ? `${'MTWTFSS'[dowMon(d)]} ${day}` : 'MTWTFSS'[dowMon(d)];
  if (u === 'week') return top ? `${day} ${MON[m]} '${String(y).slice(2)}` : `${day} ${MON[m]}`;
  if (u === 'month') return top ? `${MON[m]} ${y}` : MON[m];
  return String(y);
}

function fitCanvas(cv: HTMLCanvasElement, w: number, h: number) {
  const r = devicePixelRatio || 1;
  if (cv.width !== Math.round(w * r) || cv.height !== Math.round(h * r)) {
    cv.width = Math.round(w * r); cv.height = Math.round(h * r);
    cv.style.width = w + 'px'; cv.style.height = h + 'px';
  }
  const ctx = cv.getContext('2d')!;
  ctx.setTransform(r, 0, 0, r, 0, 0);
  ctx.clearRect(0, 0, w, h);
  return ctx;
}

let drawPending = false;
function draw() {
  if (drawPending) return;
  drawPending = true;
  requestAnimationFrame(() => { drawPending = false; drawScale(); drawBars(); });
}

function drawScale() {
  const w = cbody.clientWidth, x0 = cbody.scrollLeft;
  const ctx = fitCanvas(scaleCv, scaleCv.parentElement!.clientWidth, 40);
  ctx.fillStyle = '#F7F9FC'; ctx.fillRect(0, 0, w + 40, 40);
  const [top, bot]: [Unit, Unit] = pd >= 16 ? ['week', 'day'] : pd >= 4 ? ['month', 'week'] : ['year', 'month'];
  const first = d0 + Math.floor(x0 / pd) - 1, last = d0 + Math.ceil((x0 + w) / pd) + 1;
  ctx.font = '11px "Segoe UI", sans-serif'; ctx.textBaseline = 'middle';
  for (const [u, y, isTop] of [[top, 0, true], [bot, 20, false]] as [Unit, number, boolean][]) {
    for (let d = floorU(first, u); d < last; d = nextU(d, u)) {
      const x = (d - d0) * pd - x0, x2 = (nextU(d, u) - d0) * pd - x0;
      ctx.fillStyle = '#DCE2EA'; ctx.fillRect(Math.round(x), y + 2, 1, 18);
      ctx.fillStyle = u === 'day' && !V.week[dowMon(d)] ? '#97A1AF' : '#3B4657';
      let t = label(d, u, isTop), tw = ctx.measureText(t).width;
      if (tw >= x2 - x - 4 && u === 'week') { t = String(ymd(d)[2]); tw = ctx.measureText(t).width; }
      if (tw < x2 - x - 4) ctx.fillText(t, u === 'day' ? x + (x2 - x - tw) / 2 : Math.max(x + 4, Math.min(4, x2 - tw - 4)), y + 11);
    }
  }
  ctx.fillStyle = '#DCE2EA'; ctx.fillRect(0, 20, w + 40, 1); ctx.fillRect(0, 39, w + 40, 1);
}

function drawBars() {
  const w = cbody.clientWidth, h = cbody.clientHeight, x0 = cbody.scrollLeft, top = cbody.scrollTop;
  const ctx = fitCanvas(barsCv, w, h);
  const X = (d: number) => (d - d0) * pd - x0;
  if (pd >= 8) {
    ctx.fillStyle = '#F4F6FA';
    for (let d = d0 + Math.floor(x0 / pd); X(d) < w; d++) if (!V.week[dowMon(d)]) ctx.fillRect(X(d), 0, pd, h);
  }
  const a = Math.floor(top / ROW), b = Math.min(rows.length, a + Math.ceil(h / ROW) + 1);
  for (let i = a; i < b; i++) if (sel.has(rows[i].uid)) { ctx.fillStyle = 'rgba(31,78,154,.08)'; ctx.fillRect(0, i * ROW - top, w, ROW); }
  ctx.strokeStyle = '#F2B233'; ctx.setLineDash([4, 3]); ctx.beginPath(); ctx.moveTo(X(today) + .5, 0); ctx.lineTo(X(today) + .5, h); ctx.stroke(); ctx.setLineDash([]);

  // links
  const at = new Map(rows.map((r, i) => [r.uid, i]));
  const ends = (r: Row) => r.ms ? [X(r.s) - 6, X(r.s) + 6] : [X(r.s), X(r.f)];
  ctx.lineWidth = 1;
  rows.forEach((r, i) => r.links.forEach(([pu, k]) => {
    const pi = at.get(pu);
    if (pi === undefined || Math.max(i, pi) < a || Math.min(i, pi) > b) return;
    const p = rows[pi], [p1, p2] = ends(p), [s1, s2] = ends(r);
    const fx = k[0] === 'F' ? p2 : p1, tx = k[1] === 'S' ? s1 : s2, out = k[0] === 'F' ? 1 : -1, inn = k[1] === 'S' ? 1 : -1;
    const y1 = pi * ROW - top + ROW / 2, y2 = i * ROW - top + ROW / 2, ax = fx + out * 6;
    const pts: [number, number][] = (inn === 1 ? tx >= ax : tx <= ax)
      ? [[fx, y1], [ax, y1], [ax, y2], [tx, y2]]
      : (() => { const ym = y2 + (y2 > y1 ? -ROW / 2 : ROW / 2), bx = tx - inn * 6; return [[fx, y1], [ax, y1], [ax, ym], [bx, ym], [bx, y2], [tx, y2]] as [number, number][]; })();
    const col = showCrit && r.crit && p.crit ? '#D64545' : '#7A8BA6';
    ctx.strokeStyle = col; ctx.beginPath();
    pts.forEach(([x, y], n) => n ? ctx.lineTo(Math.round(x) + .5, Math.round(y) + .5) : ctx.moveTo(Math.round(x) + .5, Math.round(y) + .5));
    ctx.stroke();
    ctx.fillStyle = col; ctx.beginPath(); ctx.moveTo(tx, y2); ctx.lineTo(tx - inn * 5, y2 - 3.5); ctx.lineTo(tx - inn * 5, y2 + 3.5); ctx.fill();
  }));

  // bars
  ctx.font = '11px "Segoe UI", sans-serif'; ctx.textBaseline = 'middle';
  for (let i = a; i < b; i++) {
    const r = rows[i], y = i * ROW - top, x1 = X(r.s), x2 = X(r.f);
    let lx = x2 + 6;
    if (r.ms) {
      ctx.fillStyle = showCrit && r.crit ? '#D64545' : '#14325E';
      ctx.beginPath(); ctx.moveTo(x1, y + 5); ctx.lineTo(x1 + 7, y + 12); ctx.lineTo(x1, y + 19); ctx.lineTo(x1 - 7, y + 12); ctx.fill();
      lx = x1 + 11;
    } else if (r.sum) {
      ctx.fillStyle = '#14325E';
      ctx.fillRect(x1, y + 7, x2 - x1, 5);
      ctx.beginPath(); ctx.moveTo(x1, y + 7); ctx.lineTo(x1 + 5, y + 7); ctx.lineTo(x1, y + 16); ctx.fill();
      ctx.beginPath(); ctx.moveTo(x2, y + 7); ctx.lineTo(x2 - 5, y + 7); ctx.lineTo(x2, y + 16); ctx.fill();
    } else {
      ctx.fillStyle = showCrit && r.crit ? '#F2B0B0' : '#A9CAF6';
      ctx.fillRect(x1, y + 6, Math.max(x2 - x1, 2), 12);
      ctx.fillStyle = showCrit && r.crit ? '#C73B3B' : '#2F6FCF';
      ctx.fillRect(x1, y + 10, (x2 - x1) * r.pct / 100, 4);
      ctx.strokeStyle = showCrit && r.crit ? '#C73B3B' : '#2F6FCF'; ctx.strokeRect(x1 + .5, y + 6.5, Math.max(x2 - x1, 2) - 1, 11);
    }
    ctx.fillStyle = '#3B4657';
    ctx.fillText(r.ms ? `${r.name}  ${r.start}` : r.name, lx, y + 12);
  }
  if (drag) {
    const r = drag.r, y = drag.i * ROW - top;
    ctx.setLineDash([4, 3]); ctx.strokeStyle = '#2F6FCF'; ctx.lineWidth = 1.5;
    if (drag.mode === 'link') { ctx.beginPath(); ctx.moveTo(X(r.f), y + 12); ctx.lineTo(drag.mx - x0, drag.my - top); ctx.stroke(); }
    else { const g1 = X(r.s + (drag.mode === 'move' ? drag.dd : 0)), g2 = X(r.f + drag.dd); ctx.strokeRect(g1 + .5, y + 3.5, Math.max(g2 - g1, 6), 17); }
    ctx.setLineDash([]); ctx.lineWidth = 1;
  }
}

function setZoom(v: number, keepDay?: number) {
  const day = keepDay ?? d0 + (cbody.scrollLeft + cbody.clientWidth / 2) / pd;
  pd = Math.max(0.8, Math.min(48, v));
  $<HTMLInputElement>('#zoom').value = String(Math.round(Math.log(pd / 0.8) / Math.log(60) * 100));
  layout();
  cbody.scrollLeft = (day - d0) * pd - cbody.clientWidth / 2;
}
function fit() {
  if (!V.rows.length) return;
  setZoom((cbody.clientWidth - 60) / Math.max(7, V.f - V.s + 6));
  cbody.scrollLeft = (V.s - d0 - 3) * pd;
}
function scrollToTask() {
  const r = rows[cur.r];
  if (r) cbody.scrollLeft = Math.max(0, (r.s - d0) * pd - 60);
}

// ---------- commands ----------
async function guard(): Promise<boolean> {
  if (!V?.dirty) return true;
  const b = await dialog('Datum Plan', `<p>Do you want to save changes to "${esc(V.name)}"?</p>`, ['Save', "Don't Save", 'Cancel']);
  if (b === 0) return !!(await run('save', { asNew: false }));
  return b === 1;
}
async function newPlan(template?: string) {
  if (!(await guard())) return;
  collapsed.clear(); sel.clear(); cur = { r: 0, c: 1 };
  await run('new_plan', { template: template ?? null });
  closeBs();
  setView('gantt');
  if (template) fit(); else setZoom(16, today + 20);
}
async function openPlan(path?: string) {
  if (!(await guard())) return;
  const v = await run('open', { path: path ?? null });
  if (v) { collapsed.clear(); sel.clear(); cur = { r: 0, c: 1 }; closeBs(); fit(); }
}
async function save(asNew: boolean) {
  const v = await run('save', { asNew });
  if (v) toast('Saved ' + (v.path ?? ''), true);
}
const undoRedo = (redo: boolean) => run('history', { redo });
function insert(ms: boolean) {
  const r = rows[cur.r];
  const at = r ? V.rows.findIndex(x => x.uid === r.uid) : V.rows.length;
  op({ Insert: { at, name: ms ? 'Milestone' : 'New Task', ms } }).then(v => v && select(Math.min(cur.r, rows.length - 1), 1));
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
const toggleCrit = () => { showCrit = !showCrit; ribbon(); draw(); };

async function taskInfo() {
  const r = rows[cur.r];
  if (!r) return;
  const dis = r.sum ? 'disabled' : '';
  const b = await dialog(r.sum ? 'Summary Task Information' : 'Task Information', `<div class="form">
    <label>Name</label><input id="f-name" value="${esc(r.name)}">
    <label>Duration</label><input id="f-dur" value="${esc(r.dur)}" ${dis}>
    <label>% Complete</label><input id="f-pct" type="number" min="0" max="100" value="${r.pct}" ${dis}>
    <label>Start no earlier than</label><input id="f-start" type="date" value="${r.snet}" ${dis}>
    <label>Predecessors</label><input id="f-preds" value="${esc(r.preds)}" placeholder="e.g. 3, 5SS+2d" ${dis}>
    <label>Start / Finish</label><span>${r.start} → ${r.finish} · total float ${r.tf} · free float ${r.ff}</span>
    <label>Notes</label><textarea id="f-notes">${esc(r.notes)}</textarea></div>`, ['OK', 'Cancel'], '#f-name');
  if (b !== 0) return;
  const v = (k: string) => $<HTMLInputElement>('#f-' + k).value;
  const ops = [];
  if (v('name') !== r.name) ops.push(['name', v('name')]);
  if (v('notes') !== r.notes) ops.push(['notes', v('notes')]);
  if (!r.sum) {
    if (v('dur') !== r.dur) ops.push(['dur', v('dur')]);
    if (v('pct') !== String(r.pct)) ops.push(['pct', v('pct')]);
    if (v('start') !== r.snet) ops.push(['start', v('start')]);
    if (v('preds') !== r.preds) ops.push(['preds', v('preds')]);
  }
  if (ops.length) op({ Batch: ops.map(([field, value]) => ({ Set: { uid: r.uid, field, value } })) });
}

async function projectInfo() {
  const days = ['Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat', 'Sun'];
  const b = await dialog('Project Information', `<div class="form">
    <label>Project name</label><input id="p-name" value="${esc(V.name)}">
    <label>Start date</label><input id="p-start" type="date" value="${V.startIso}">
    <label>Finish date</label><span>${V.finish} (calculated)</span>
    <label>Working days</label><div class="week">${days.map((d, i) => `<label><input type="checkbox" id="p-w${i}" ${V.week[i] ? 'checked' : ''}> ${d}</label>`).join('')}</div>
    </div>`, ['OK', 'Cancel'], '#p-name');
  if (b !== 0) return;
  op({ Project: { name: $<HTMLInputElement>('#p-name').value, start: $<HTMLInputElement>('#p-start').value, week: days.map((_, i) => $<HTMLInputElement>('#p-w' + i).checked) } });
}

const KEYS: [string, string][] = [
  ['Ctrl+N / Ctrl+O / Ctrl+S', 'New, open, save'], ['Ctrl+Shift+S', 'Save as'], ['Ctrl+Z / Ctrl+Y', 'Undo, redo'],
  ['Insert', 'New task above the selection'], ['Delete', 'Delete the selected tasks'], ['Enter / F2', 'Edit the cell'],
  ['Tab / Shift+Tab', 'Next / previous cell while editing'], ['Alt+Shift+→ / ←', 'Indent / outdent'],
  ['Ctrl+F2 / Ctrl+Shift+F2', 'Link / unlink the selected tasks'], ['Shift+F2', 'Task information'],
  ['Ctrl+Shift+F5', 'Scroll to task'], ['Ctrl+wheel', 'Zoom the Gantt chart'], ['Shift+click, Ctrl+click', 'Select several tasks'],
];
const shortcuts = () => dialog('Keyboard Shortcuts', `<table class="keys">${KEYS.map(([k, d]) => `<tr><td>${k}</td><td>${d}</td></tr>`).join('')}</table>`, ['Close']);

// ---------- ribbon ----------
type Btn = [icon: string, label: string, act: () => void, big?: boolean, on?: () => boolean];
function ribbonDef(): Record<string, [string, (Btn | Btn[] | 'pct')[]][]> {
  return {
    task: [
      ['View', [['gantt', 'Gantt\nChart', () => {}, true, () => true]]],
      ['Schedule', ['pct', [['indent', 'Indent', () => op({ Indent: { uids: picked() } })], ['outdent', 'Outdent', () => op({ Outdent: { uids: picked() } })]],
        [['link', 'Link Tasks', () => op({ Link: { uids: picked() } })], ['unlink', 'Unlink Tasks', () => op({ Unlink: { uids: picked() } })]]]],
      ['Insert', [['task', 'Task', () => insert(false), true], ['ms', 'Milestone', () => insert(true), true]]],
      ['Properties', [['info', 'Information', taskInfo, true]]],
      ['Editing', [['scroll', 'Scroll\nto Task', scrollToTask, true], ['del', 'Delete', del, true]]],
    ],
    project: [['Properties', [['proj', 'Project\nInformation', projectInfo, true]]]],
    view: [
      ['Zoom', [['zin', 'Zoom In', () => setZoom(pd * 1.5), true], ['zout', 'Zoom Out', () => setZoom(pd / 1.5), true], ['fit', 'Entire\nProject', fit, true]]],
      ['Show', [['crit', 'Critical\nPath', toggleCrit, true, () => showCrit]]],
    ],
    help: [['Help', [['upd', 'Check for\nUpdates', () => openBs('about'), true], ['keys', 'Keyboard\nShortcuts', shortcuts, true], ['info', 'About\nDatum Plan', () => openBs('about'), true]]]],
  };
}
let acts: (() => void)[] = [];
function ribbon() {
  acts = [];
  const btn = ([ic, lb, fn, big, on]: Btn) => {
    acts.push(fn);
    return `<button class="rb${big ? ' big' : ''}${on?.() ? ' on' : ''}" data-r="${acts.length - 1}"><svg><use href="#i-${ic}"/></svg><span>${lb.replace('\n', '<br>')}</span></button>`;
  };
  $('#ribbon').innerHTML = ribbonDef()[tab].map(([cap, items]) => `<div class="grp"><div class="items">${items.map(it => {
    if (it === 'pct') return `<div class="pcts">${[0, 25, 50, 75, 100].map(p => { acts.push(() => pct(p)); return `<button class="rb" data-r="${acts.length - 1}" title="Mark ${p}% complete"><svg><use href="#i-pct"/></svg>${p}%</button>`; }).join('')}</div>`;
    if (Array.isArray(it[0])) return `<div class="stack">${(it as Btn[]).map(btn).join('')}</div>`;
    return btn(it as Btn);
  }).join('')}</div><div class="cap">${cap}</div></div>`).join('');
}

// ---------- File backstage, Home and updates ----------
let bsPage = 'home';
function openBs(page = 'home') { bsPage = page; $('#bs').hidden = false; renderBs(); }
function closeBs() { $('#bs').hidden = true; gbody.focus(); }

function md(s: string): string {
  return esc(s).split('\n').map(l => {
    l = l.replace(/\*\*(.+?)\*\*/g, '<b>$1</b>');
    if (/^#{1,3} /.test(l)) return `<h3>${l.replace(/^#+ /, '')}</h3>`;
    if (/^\s*[-*] /.test(l)) return `<ul><li>${l.replace(/^\s*[-*] /, '')}</li></ul>`;
    return l.trim() ? `<p>${l}</p>` : '';
  }).join('').replace(/<\/ul><ul>/g, '');
}

function updCard(full: boolean): string {
  let pill = '<span class="pill">Checking for updates…</span>', action = '';
  if (updErr) { pill = `<span class="pill err">${esc(updErr)}</span>`; action = '<button class="btn wide" data-u="check">Try again</button>'; }
  else if (updPct >= 0) { pill = '<span class="pill warn">Downloading the update…</span>'; action = `<div class="prog"><i style="width:${updPct}%"></i></div><div class="muted">${updPct}% · Datum Plan restarts when it is installed.</div>`; }
  else if (upd?.available) {
    pill = `<span class="pill warn">Version ${esc(upd.latest!)} is available</span>`;
    action = upd.url ? `<button class="btn pri wide" data-u="install">Update now${upd.size ? ` · ${(upd.size / 1048576).toFixed(1)} MB` : ''}</button>` : '';
    action += `<div class="notes"><h3>What's new in ${esc(upd.latest!)}${upd.published ? ` · ${upd.published}` : ''}</h3>${md(upd.notes)}</div>`;
  } else if (upd) {
    pill = '<span class="pill ok">You have the latest version</span>';
    action = `<button class="btn wide" data-u="check">Check for updates</button>`;
    if (full && upd.notes) action += `<div class="notes"><h3>What's new in ${esc(upd.latest ?? version)}</h3>${md(upd.notes)}</div>`;
  }
  return `<div class="card"><div class="brand"><img src="logo.svg" alt=""><div><b>Datum Plan</b><span class="muted">Version ${version}</span></div></div>${pill}${action}
    <div class="muted" style="margin-top:10px">Updates come from <a href="#" data-u="page">GitHub Releases</a>.</div></div>`;
}

async function renderBs() {
  document.querySelectorAll('#bs aside button').forEach(b => b.classList.toggle('on', (b as HTMLElement).dataset.b === bsPage));
  const main = $('#bsMain');
  if (bsPage === 'about') {
    main.innerHTML = `<h1>About &amp; Updates</h1><div style="max-width:560px">${updCard(true)}
      <h2>Datum Plan</h2><p class="muted">Project scheduling with the strengths of Microsoft Project and Primavera P6. Plans are saved as .dplan files on your computer.<br>© 2026 Edgar Mendonca · MIT licence</p></div>`;
    return;
  }
  const hr = new Date().getHours();
  const recent = await call<Recent[]>('recent') ?? [];
  const tile = (a: string, icon: string, t: string, d: string) => `<button class="tile" data-h="${a}"><div class="pic"><svg class="ic"><use href="#${icon}"/></svg></div><b>${t}</b><span>${d}</span></button>`;
  const mini = (t: Tpl) => { const h = Math.min(6, 84 / t.bars.length); return `<svg viewBox="0 0 200 96" preserveAspectRatio="none">${t.bars.map(([k, a, b], i) => { const x = 8 + a * 184, w = Math.max((b - a) * 184, 1.5), y = 6 + i * h;
    return k === 2 ? `<rect x="${x - 1.5}" y="${y}" width="3" height="3" fill="#14325E" transform="rotate(45 ${x} ${y + 1.5})"/>` : `<rect x="${x}" y="${y + (k ? 1 : .5)}" width="${w}" height="${k ? 1.6 : h - 1.6}" fill="${k ? '#14325E' : t.color}" rx=".6"/>`; }).join('')}</svg>`; };
  main.innerHTML = `<h1>Good ${hr < 12 ? 'morning' : hr < 17 ? 'afternoon' : 'evening'}</h1><div class="home"><div>
    <h2>New</h2><div class="tiles">${tile('blank', 'i-new', 'Blank project', 'An empty plan starting today.')}${tile('open', 'i-open', 'Open a plan…', 'A .dplan file saved on this computer.')}</div>
    <h2>Templates</h2><div class="tiles">${tpls.map(t => `<button class="tile" data-h="t:${t.id}"><div class="pic">${mini(t)}<small>${t.tasks} tasks · ${t.days} days</small></div><b>${esc(t.name)}</b><span>${esc(t.desc)}</span></button>`).join('')}</div>
    <p class="muted">Templates start next Monday. Change the start date in Project › Project Information and every task moves with it.</p>
    <h2>Recent</h2>${recent.length ? `<table class="recent"><tr><th>Name</th><th>Date modified</th></tr>${recent.map(r =>
      `<tr class="f" data-p="${esc(r.path)}"><td><b>${esc(r.name)}</b><div class="p">${esc(r.path)}</div></td><td>${r.modified}</td></tr>`).join('')}</table>`
      : '<p class="muted">Plans you open or save appear here.</p>'}
  </div><div>${updCard(false)}</div></div>`;
}

async function checkUpdate() {
  updErr = ''; upd = null;
  if (!$('#bs').hidden) renderBs();
  try { upd = await inv<Upd>('check_update'); } catch (e) { updErr = String(e); }
  $('#updBadge').hidden = !upd?.available;
  if (!$('#bs').hidden) renderBs();
}

async function installUpdate() {
  if (!upd?.url || !(await guard())) return;
  updPct = 0; renderBs();
  try { await inv('install_update', { url: upd.url, sums: upd.sums }); }
  catch (e) { updPct = -1; updErr = String(e); renderBs(); }
}

// ---------- dialogs, toast, status ----------
function dialog(title: string, body: string, buttons: string[], focus?: string): Promise<number> {
  return new Promise(res => {
    const ov = document.createElement('div');
    ov.className = 'ov';
    ov.innerHTML = `<div class="dlg"><header>${title}</header><div class="bd">${body}</div><footer>${buttons.map((b, i) => `<button class="btn${i ? '' : ' pri'}" data-i="${i}">${b}</button>`).join('')}</footer></div>`;
    document.body.appendChild(ov);
    const done = (i: number) => { ov.remove(); res(i); gbody.focus(); };
    ov.onclick = e => { const i = (e.target as HTMLElement).dataset.i; if (i !== undefined) done(+i); };
    ov.onkeydown = e => {
      e.stopPropagation();
      if (e.key === 'Escape') done(buttons.length - 1);
      if (e.key === 'Enter' && (e.target as HTMLElement).tagName !== 'TEXTAREA') { e.preventDefault(); done(0); }
    };
    (ov.querySelector(focus ?? '.btn') as HTMLElement).focus();
  });
}
let toastT = 0;
function toast(msg: string, ok = false) {
  const t = $('#toast');
  t.textContent = msg; t.className = ok ? 'ok' : ''; t.hidden = false;
  clearTimeout(toastT);
  toastT = setTimeout(() => (t.hidden = true), ok ? 2500 : 6000);
}
function setStatus(msg: string, err = false) { const s = $('#stMsg'); s.textContent = msg; s.className = err ? 'err' : ''; }

// ---------- Gantt mouse: select, drag to move, drag the end to resize, drag down to link ----------
interface Drag { r: Row; i: number; mode: 'move' | 'size' | 'link'; x: number; y: number; dd: number; mx: number; my: number }
let drag: Drag | null = null;
const tip = $('#tip');
const iso = (d: number) => new Date(d * DAY).toISOString().slice(0, 10);
const fmtD = (d: number) => { const [y, m, dd] = ymd(d); return `${String(dd).padStart(2, '0')}-${MON[m]}-${String(y).slice(2)}`; };
const pos = (e: MouseEvent) => { const b = cbody.getBoundingClientRect(); return [e.clientX - b.left + cbody.scrollLeft, e.clientY - b.top + cbody.scrollTop]; };
function hit(e: MouseEvent) {
  const [x, y] = pos(e), i = Math.floor(y / ROW), r = rows[i];
  let part = '';
  if (r && !r.sum) {
    const x1 = (r.s - d0) * pd, x2 = (r.f - d0) * pd;
    part = r.ms ? (Math.abs(x - x1) <= 8 ? 'move' : '') : x >= x2 - 5 && x <= x2 + 3 ? 'size' : x >= x1 - 2 && x < x2 - 5 ? 'move' : '';
  }
  return { i, x, y, r, part };
}
function showTip(e: MouseEvent, html: string) {
  tip.innerHTML = html; tip.hidden = false;
  tip.style.left = Math.min(e.clientX + 14, innerWidth - tip.offsetWidth - 8) + 'px';
  tip.style.top = e.clientY + 18 + 'px';
}
cbody.onmousedown = e => {
  if (e.button !== 0) return;
  const h = hit(e);
  select(h.i, cur.c, e);
  if (!h.r || !h.part || e.shiftKey || e.ctrlKey) return;
  drag = { r: h.r, i: h.i, mode: h.part as 'move' | 'size', x: h.x, y: h.y, dd: 0, mx: h.x, my: h.y };
  e.preventDefault();
};
document.addEventListener('mousemove', e => {
  if (!drag) {
    if (!cbody.contains(e.target as Node)) return void (tip.hidden = true);
    const h = hit(e), r = h.r;
    cbody.style.cursor = h.part === 'size' ? 'ew-resize' : h.part ? 'grab' : 'default';
    if (r && h.part) showTip(e, `<b>${esc(r.name)}</b><br>${r.start} → ${r.finish} · ${r.dur}<br>${r.pct}% complete · total float ${r.tf}<br><span class="muted">Drag to move · drag the end to resize · drag down to link</span>`);
    else tip.hidden = true;
    return;
  }
  const [x, y] = pos(e), d = drag;
  d.mx = x; d.my = y; d.dd = Math.round((x - d.x) / pd);
  if (d.mode === 'move' && Math.abs(y - d.y) > ROW) d.mode = 'link';
  const t = rows[Math.floor(y / ROW)];
  showTip(e, d.mode === 'link' ? `Link <b>${d.r.id}</b> → <b>${t && !t.sum && t !== d.r ? t.id + ' ' + esc(t.name) : '…'}</b>`
    : d.mode === 'move' ? `Start <b>${fmtD(d.r.s + d.dd)}</b>` : `Finish <b>${fmtD(Math.max(d.r.s, d.r.f - 1 + d.dd))}</b>`);
  draw();
});
document.addEventListener('mouseup', async () => {
  const d = drag;
  if (!d) return;
  drag = null; tip.hidden = true; draw();
  if (d.mode === 'link') {
    const t = rows[Math.floor(d.my / ROW)];
    if (t && t !== d.r && !t.sum) await op({ Set: { uid: t.uid, field: 'preds', value: t.preds ? `${t.preds}, ${d.r.id}` : String(d.r.id) } });
  } else if (d.dd && d.mode === 'move') await op({ Set: { uid: d.r.uid, field: 'start', value: iso(d.r.s + d.dd) } });
  else if (d.dd) await op({ Set: { uid: d.r.uid, field: 'finish', value: iso(Math.max(d.r.s, d.r.f - 1 + d.dd)) } });
});

// ---------- view sidebar and pages ----------
const VIEWS: ([string, string, string, string?] | '-')[] = [
  ['gantt', 'gantt', 'Gantt'], ['list', 'list', 'Task List'], ['cal', 'cal', 'Calendar'], ['dash', 'dash', 'Dashboard'], ['report', 'report', 'Reports'], '-',
  ['net', 'net', 'Network', '0.8'], ['res', 'res', 'Resources', '0.4'], ['load', 'load', 'Workload', '0.4'],
];
function renderViews() {
  $('#views').innerHTML = VIEWS.map(v => v === '-' ? '<hr>' : `<button data-v="${v[0]}" class="${view === v[0] ? 'on' : ''}${v[3] ? ' soon' : ''}" title="${v[3] ? `${v[2]}: coming in ${v[3]}` : v[2]}"><svg><use href="#i-${v[1]}"/></svg>${v[2]}</button>`).join('');
}
function setView(v: string) {
  const def = VIEWS.find(x => x !== '-' && x[0] === v) as [string, string, string, string?];
  if (def[3]) return toast(`${def[2]} arrives in version ${def[3]}. See the roadmap under Help › About.`);
  view = v;
  renderViews();
  const grid = v === 'gantt' || v === 'list';
  $('#ganttView').hidden = !grid;
  $('#page').hidden = grid;
  $('#ganttView').classList.toggle('list', v === 'list');
  if (grid) { layout(); gbody.focus(); } else renderPage();
}
$('#views').onclick = e => { const b = (e.target as HTMLElement).closest('[data-v]') as HTMLElement | null; if (b) setView(b.dataset.v!); };

const leaves = () => V.rows.filter(r => !r.sum);
const span = (r: Row) => r.ms ? 0 : r.f - r.s;
const pctNow = (L: Row[]) => { const w = L.reduce((a, r) => a + span(r), 0); return w ? L.reduce((a, r) => a + span(r) * r.pct, 0) / w : 0; };
const planned = (L: Row[], d: number) => { const w = L.reduce((a, r) => a + span(r), 0); return w ? 100 * L.reduce((a, r) => a + (span(r) ? span(r) * Math.min(1, Math.max(0, (d - r.s) / span(r))) : 0), 0) / w : 0; };
const tbl = (L: Row[], extra?: (r: Row) => string) => L.length
  ? `<table class="tbl"><tr><th>ID</th><th>Task</th><th>Start</th><th>Finish</th><th class="r">Done</th>${extra ? '<th></th>' : ''}</tr>${L.map(r =>
    `<tr><td>${r.id}</td><td${isLate(r) ? ' class="late"' : ''}>${esc(r.name)}</td><td>${r.start}</td><td>${r.finish}</td><td class="r">${r.pct}%</td>${extra ? `<td>${extra(r)}</td>` : ''}</tr>`).join('')}</table>`
  : '<div class="empty">None</div>';
const meter = (p: number) => `<div class="meter"><i style="width:${p}%"></i></div>`;

let calOff = 0, rep: { unit: 'week' | 'month'; off: number } = { unit: 'week', off: 0 };
function renderPage() {
  const pg = $('#page');
  if (!V.rows.length) { pg.innerHTML = '<h1>Nothing to show yet</h1><p class="muted">Add tasks in the Gantt view, or start from a template under File › Home.</p>'; return; }
  pg.innerHTML = view === 'cal' ? calendarPage() : view === 'dash' ? dashboardPage() : reportPage();
}

function calendarPage(): string {
  const [ty, tm] = ymd(today), first = Date.UTC(ty, tm + calOff, 1) / DAY, [y, m] = ymd(first), start = first - dowMon(first);
  const byDay = new Map<number, Row[]>();
  const add = (d: number, r: Row) => byDay.set(d, [...(byDay.get(d) ?? []), r]);
  for (const r of leaves()) { add(r.s, r); if (!r.ms && endDay(r) !== r.s) add(endDay(r), r); }
  let cells = '';
  for (let k = 0; k < 42; k++) {
    const d = start + k, [, mm, dd] = ymd(d), it = byDay.get(d) ?? [];
    const chip = (r: Row) => `<div class="chip${r.ms ? ' ms' : ''}${showCrit && r.crit ? ' crit' : ''}" data-u="${r.uid}" title="${esc(r.name)} · ${r.start} → ${r.finish}">${r.ms ? '◆' : r.s === d ? '▶' : '■'} ${esc(r.name)}</div>`;
    cells += `<div class="day${mm !== m ? ' out' : ''}${V.week[dowMon(d)] ? '' : ' nw'}${d === today ? ' today' : ''}"><span class="dn">${dd}</span>${it.slice(0, 4).map(chip).join('')}${it.length > 4 ? `<div class="chip more">+${it.length - 4} more</div>` : ''}</div>`;
  }
  const MONF = ['January', 'February', 'March', 'April', 'May', 'June', 'July', 'August', 'September', 'October', 'November', 'December'];
  return `<div class="ptools"><h1>${MONF[m]} ${y}</h1><span class="grow"></span><div class="seg"><button data-cal="-1">‹</button><button data-cal="0">Today</button><button data-cal="1">›</button></div></div>
    <div class="calgrid">${['Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat', 'Sun'].map(x => `<div class="dh">${x}</div>`).join('')}${cells}</div>
    <p class="muted">▶ starts · ■ finishes · ◆ milestone. Click a task to open it in the Gantt chart.</p>`;
}

function scurve(L: Row[]): string {
  const a = V.s, b = Math.max(V.f, a + 1), W = 600, H = 170, px = (d: number) => 30 + (d - a) / (b - a) * (W - 40), py = (p: number) => H - 20 - p / 100 * (H - 30);
  const pts = Array.from({ length: 61 }, (_, k) => { const d = a + (b - a) * k / 60; return `${px(d).toFixed(1)},${py(planned(L, d)).toFixed(1)}`; }).join(' ');
  const t = Math.min(Math.max(today, a), b), act = pctNow(L);
  return `<svg viewBox="0 0 ${W} ${H}" style="width:100%;height:auto">
    ${[0, 50, 100].map(p => `<line x1="30" x2="${W - 10}" y1="${py(p)}" y2="${py(p)}" stroke="#E3E9F2"/><text x="2" y="${py(p) + 4}" font-size="10" fill="#6B7686">${p}%</text>`).join('')}
    <polyline points="${pts}" fill="none" stroke="#2F6FCF" stroke-width="2"/>
    <line x1="${px(t)}" x2="${px(t)}" y1="8" y2="${H - 20}" stroke="#F2B233" stroke-dasharray="4 3"/>
    <circle cx="${px(t)}" cy="${py(act)}" r="5" fill="#2E9D57"/>
    <text x="30" y="${H - 4}" font-size="10" fill="#6B7686">${V.start}</text><text x="${W - 10}" y="${H - 4}" font-size="10" fill="#6B7686" text-anchor="end">${V.finish}</text></svg>
    <div class="muted"><span style="color:#2F6FCF">━</span> planned progress · <span style="color:#2E9D57">●</span> actual today (${act.toFixed(0)}%)</div>`;
}

function dashboardPage(): string {
  const L = leaves(), done = L.filter(r => r.pct === 100), prog = L.filter(r => r.pct > 0 && r.pct < 100), late = L.filter(isLate);
  const now = pctNow(L), plan = planned(L, today + 1), left = Math.max(0, V.f - 1 - today);
  const kpi = (v: string, l: string, c = '') => `<div class="kpi ${c}"><div class="v">${v}</div><div class="l">${l}</div></div>`;
  const phases = V.rows.filter(r => r.sum && r.lvl === Math.min(...V.rows.filter(x => x.sum).map(x => x.lvl)));
  const ms = L.filter(r => r.ms && r.s >= today).slice(0, 8);
  return `<div class="ptools"><h1>${esc(V.name)}: dashboard</h1></div>
    <div class="kpis">${kpi(now.toFixed(0) + '%', `complete · planned ${plan.toFixed(0)}% by today`, now + 0.5 >= plan ? 'good' : 'bad')}
      ${kpi(V.finish, `finish · ${left} days to go`)}${kpi(String(L.length), 'tasks')}${kpi(String(done.length), 'completed', 'good')}
      ${kpi(String(prog.length), 'in progress')}${kpi(String(late.length), 'late', late.length ? 'bad' : 'good')}${kpi(String(L.filter(r => r.crit).length), 'critical tasks')}</div>
    <div class="cols">
      <div class="panel"><h2 style="margin-top:0">Progress curve</h2>${scurve(L)}</div>
      <div class="panel"><h2 style="margin-top:0">Phases</h2>${phases.length ? `<table class="tbl"><tr><th>Phase</th><th>Finish</th><th>Done</th><th></th></tr>${phases.map(r => `<tr><td>${esc(r.name)}</td><td>${r.finish}</td><td class="r">${r.pct}%</td><td>${meter(r.pct)}</td></tr>`).join('')}</table>` : '<div class="empty">Indent tasks under summary tasks to see phases.</div>'}</div>
      <div class="panel"><h2 style="margin-top:0">Upcoming milestones</h2>${tbl(ms)}</div>
      <div class="panel"><h2 style="margin-top:0">Late tasks</h2>${tbl(late)}</div>
    </div>`;
}

function reportPage(): string {
  const [ty, tm] = ymd(today);
  const ps = rep.unit === 'week' ? today - dowMon(today) + 7 * rep.off : Date.UTC(ty, tm + rep.off, 1) / DAY;
  const pe = rep.unit === 'week' ? ps + 7 : nextU(ps, 'month'), ne = rep.unit === 'week' ? pe + 7 : nextU(pe, 'month');
  const L = leaves(), inP = (d: number, a = ps, b = pe) => d >= a && d < b;
  const finished = L.filter(r => r.pct === 100 && inP(endDay(r))), due = L.filter(r => r.pct < 100 && inP(endDay(r)));
  const starting = L.filter(r => !r.ms && inP(r.s)), active = L.filter(r => r.pct > 0 && r.pct < 100), late = L.filter(isLate);
  const ms = L.filter(r => r.ms && inP(r.s)), next = L.filter(r => inP(r.s, pe, ne));
  const now = pctNow(L), plan = planned(L, pe), title = rep.unit === 'week' ? `Week of ${fmtD(ps)} to ${fmtD(pe - 1)}` : `${MON[ymd(ps)[1]]} ${ymd(ps)[0]}`;
  const sec = (h: string, L2: Row[]) => `<div class="panel"><h2 style="margin-top:0">${h} <span class="muted">(${L2.length})</span></h2>${tbl(L2)}</div>`;
  return `<div class="ptools"><div class="seg"><button data-rep="week" class="${rep.unit === 'week' ? 'on' : ''}">Weekly</button><button data-rep="month" class="${rep.unit === 'month' ? 'on' : ''}">Monthly</button></div>
      <div class="seg"><button data-rp="-1">‹ Previous</button><button data-rp="0">Current</button><button data-rp="1">Next ›</button></div><span class="grow"></span>
      <button class="btn" data-print="1">Print or save as PDF</button></div>
    <h1>${esc(V.name)}: progress report</h1><div class="muted">${title} · produced ${fmtD(today)}</div>
    <div class="kpis" style="margin-top:14px"><div class="kpi"><div class="v">${now.toFixed(0)}%</div><div class="l">complete now</div></div>
      <div class="kpi"><div class="v">${plan.toFixed(0)}%</div><div class="l">planned by end of period</div></div>
      <div class="kpi ${late.length ? 'bad' : 'good'}"><div class="v">${late.length}</div><div class="l">late tasks</div></div>
      <div class="kpi"><div class="v">${V.finish}</div><div class="l">forecast finish</div></div></div>
    <div class="cols">${sec('Finished this period', finished)}${sec('Due to finish this period', due)}${sec('Starting this period', starting)}${sec('Milestones this period', ms)}${sec('In progress', active)}${sec('Late', late)}${sec('Starting next period (look-ahead)', next)}</div>`;
}

$('#page').onclick = e => {
  const el = e.target as HTMLElement, d = el.dataset;
  const chip = (el.closest('[data-u]') as HTMLElement | null)?.dataset.u;
  if (d.cal) { calOff = d.cal === '0' ? 0 : calOff + +d.cal; renderPage(); }
  else if (d.rep) { rep = { unit: d.rep as 'week' | 'month', off: 0 }; renderPage(); }
  else if (d.rp) { rep.off = d.rp === '0' ? 0 : rep.off + +d.rp; renderPage(); }
  else if (d.print) print();
  else if (chip) { setView('gantt'); const i = rows.findIndex(r => r.uid === +chip); if (i >= 0) { select(i, 1); scrollToTask(); } }
};

// ---------- events ----------
let syncing = false;
gbody.onscroll = () => {
  ghead.scrollLeft = gbody.scrollLeft;
  renderRows();
  if (!syncing) { syncing = true; cbody.scrollTop = gbody.scrollTop; syncing = false; }
  draw();
};
cbody.onscroll = () => {
  if (!syncing) { syncing = true; gbody.scrollTop = cbody.scrollTop; syncing = false; }
  draw();
};
cbody.onwheel = e => { if (e.ctrlKey) { e.preventDefault(); setZoom(pd * (e.deltaY < 0 ? 1.2 : 1 / 1.2), d0 + (cbody.scrollLeft + e.offsetX) / pd); } };
cbody.ondblclick = () => taskInfo();
gbody.tabIndex = 0;

grows.onmousedown = e => {
  const el = e.target as HTMLElement, rowEl = el.closest('.row') as HTMLElement | null;
  if (!rowEl) return;
  if (el.dataset.t) { const u = +el.dataset.t; collapsed.has(u) ? collapsed.delete(u) : collapsed.add(u); rows = visible(); layout(); return; }
  const ci = [...rowEl.children].indexOf(el.closest('.c') as Element);
  if (editor) commit(0, 0);
  select(+rowEl.dataset.i!, ci < 0 ? cur.c : ci, e);
  gbody.focus();
  e.preventDefault();
};
grows.ondblclick = e => { if (COLS[cur.c].ed) startEdit(); else if ((e.target as HTMLElement).closest('.c.id')) taskInfo(); };

ghead.onmousedown = e => {
  const c = (e.target as HTMLElement).dataset.c;
  if (c === undefined) return;
  const col = COLS[+c], x = e.clientX, w0 = col.w;
  const mv = (m: MouseEvent) => { col.w = Math.max(30, w0 + m.clientX - x); layout(); };
  document.addEventListener('mousemove', mv);
  document.addEventListener('mouseup', () => document.removeEventListener('mousemove', mv), { once: true });
};
$('#split').onmousedown = e => {
  const g = $('#grid'), x = e.clientX, w0 = g.offsetWidth;
  const mv = (m: MouseEvent) => { g.style.width = Math.max(120, w0 + m.clientX - x) + 'px'; draw(); };
  document.addEventListener('mousemove', mv);
  document.addEventListener('mouseup', () => document.removeEventListener('mousemove', mv), { once: true });
};
$('#zoom').oninput = e => setZoom(0.8 * Math.pow(60, +(e.target as HTMLInputElement).value / 100));
$('#ribbon').onclick = e => { const b = (e.target as HTMLElement).closest('[data-r]') as HTMLElement | null; if (b) acts[+b.dataset.r!](); };
$('#tabs').onclick = e => {
  const t = (e.target as HTMLElement).dataset.tab;
  if (!t) return;
  if (t === 'file') return openBs('home');
  tab = t;
  document.querySelectorAll('#tabs button').forEach(b => b.classList.toggle('on', (b as HTMLElement).dataset.tab === t));
  ribbon();
};
const ACTIONS: Record<string, () => void> = {
  save: () => save(false), undo: () => undoRedo(false), redo: () => undoRedo(true), home: () => openBs('home'),
  zin: () => setZoom(pd * 1.5), zout: () => setZoom(pd / 1.5),
};
document.querySelectorAll('[data-a]').forEach(b => ((b as HTMLElement).onclick = () => ACTIONS[(b as HTMLElement).dataset.a!]()));
$('#bs aside').onclick = e => {
  const b = ((e.target as HTMLElement).closest('[data-b]') as HTMLElement | null)?.dataset.b;
  if (b === 'back') closeBs();
  else if (b === 'new') newPlan();
  else if (b === 'open') openPlan();
  else if (b === 'save' || b === 'saveas') save(b === 'saveas').then(closeBs);
  else if (b) { bsPage = b; renderBs(); }
};
$('#bsMain').onclick = e => {
  const el = e.target as HTMLElement, h = (el.closest('[data-h]') as HTMLElement | null)?.dataset.h;
  const p = (el.closest('[data-p]') as HTMLElement | null)?.dataset.p, u = el.dataset.u;
  if (h === 'blank') newPlan();
  else if (h?.startsWith('t:')) newPlan(h.slice(2));
  else if (h === 'open') openPlan();
  else if (p) openPlan(p);
  else if (u === 'check') checkUpdate();
  else if (u === 'install') installUpdate();
  else if (u === 'page') { e.preventDefault(); navigator.clipboard?.writeText(upd?.page ?? 'https://github.com/Edgar-Mendonca/Datum-Plan/releases'); toast('Releases link copied to the clipboard.', true); }
};

document.addEventListener('keydown', e => {
  if (editor || document.querySelector('.ov')) return;
  const k = e.key, ctrl = e.ctrlKey || e.metaKey;
  if (!$('#bs').hidden) { if (k === 'Escape') closeBs(); return; }
  const go = (dr: number, dc: number) => { e.preventDefault(); select(cur.r + dr, cur.c + dc, { shiftKey: e.shiftKey && dr !== 0, ctrlKey: false }); };
  if (ctrl && k.toLowerCase() === 's') { e.preventDefault(); save(e.shiftKey); }
  else if (ctrl && k.toLowerCase() === 'o') { e.preventDefault(); openPlan(); }
  else if (ctrl && k.toLowerCase() === 'n') { e.preventDefault(); newPlan(); }
  else if (ctrl && k.toLowerCase() === 'z') { e.preventDefault(); undoRedo(false); }
  else if (ctrl && k.toLowerCase() === 'y') { e.preventDefault(); undoRedo(true); }
  else if ($('#ganttView').hidden) { /* pages take no grid keys */ }
  else if (ctrl && k === 'F2') op(e.shiftKey ? { Unlink: { uids: picked() } } : { Link: { uids: picked() } });
  else if (ctrl && e.shiftKey && k === 'F5') scrollToTask();
  else if (e.altKey && e.shiftKey && (k === 'ArrowRight' || k === 'ArrowLeft')) { e.preventDefault(); op(k === 'ArrowRight' ? { Indent: { uids: picked() } } : { Outdent: { uids: picked() } }); }
  else if (k === 'F2' && e.shiftKey) taskInfo();
  else if (k === 'ArrowDown') go(1, 0);
  else if (k === 'ArrowUp') go(-1, 0);
  else if (k === 'ArrowRight') go(0, 1);
  else if (k === 'ArrowLeft') go(0, -1);
  else if (k === 'Tab') go(0, e.shiftKey ? -1 : 1);
  else if (k === 'PageDown') go(Math.floor(gbody.clientHeight / ROW), 0);
  else if (k === 'PageUp') go(-Math.floor(gbody.clientHeight / ROW), 0);
  else if (k === 'Enter' || k === 'F2') { e.preventDefault(); startEdit(); }
  else if (k === 'Insert') insert(false);
  else if (k === 'Delete') del();
  else if (k === 'F1') { e.preventDefault(); shortcuts(); }
  else if (k.length === 1 && !ctrl && !e.altKey) { e.preventDefault(); startEdit(k); }
});
window.onresize = () => { renderRows(); draw(); };
TAURI.event.listen('close-requested', async () => { if (await guard()) inv('quit'); });
TAURI.event.listen('update-progress', (e: { payload: number }) => { updPct = e.payload; if (!$('#bs').hidden) renderBs(); });

// ---------- start ----------
(async () => {
  version = await inv<string>('version');
  tpls = await inv<Tpl[]>('templates');
  renderViews();
  ribbon();
  $('#grid').style.width = Math.min(gridW() + 18, innerWidth * 0.55) + 'px';
  show(await inv<View>('state'));
  setZoom(16, today + 20);
  const f = await inv<string | null>('startup_file');
  if (f) await openPlan(f); else openBs('home');
  checkUpdate();
})();
