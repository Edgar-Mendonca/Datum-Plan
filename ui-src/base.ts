// Datum Plan window. The Rust core owns the plan and computes every value; these files draw what it
// returns and send edits back as operations. Files share one global scope (see tsconfig "files").

type LinkT = 'FS' | 'SS' | 'FF' | 'SF';
interface Row {
  uid: number; id: number; wbs: string; code: string; name: string; lvl: number; sum: boolean; ms: boolean; crit: boolean; late: boolean;
  manual: boolean; off: boolean; tl: boolean; dur: string; start: string; finish: string; preds: string; succs: string; res: string; pct: number;
  tf: string; ff: string; work: string; cost: string; ctype: string; cdate: string; deadline: string; cal: string; mode: string; active: string;
  astart: string; afinish: string; rem: string; bstart: string; bfinish: string; bdur: string; vstart: string; vfinish: string; notes: string;
  s: number; f: number; bs: number | null; bf: number | null; dl: number | null; links: [number, LinkT][];
}
interface ResRow { uid: number; id: number; name: string; kind: string; group: string; max: string; rate: string; perUse: string; label: string; work: string; cost: string; over: boolean }
type Hours = [number, number][];
interface Cal { name: string; week: Hours[]; exceptions: { name: string; from: string; to: string; hours: Hours }[] }
interface View {
  name: string; start: string; startIso: string; finish: string; status: string; statusIso: string; statusDay: number | null; s: number; f: number;
  calendar: string; dayHours: number; manualDefault: boolean; week: boolean[]; offDays: number[]; columns: [string, number][]; calendars: Cal[];
  resources: ResRow[]; work: string; cost: string; blFinish: string; varFinish: string; rows: Row[]; error: string | null;
  path: string | null; dirty: boolean; undo: boolean; redo: boolean; clip: number;
}
interface Rel { uid: number; id: number; name: string; kind: LinkT; lag: string }
interface Detail { preds: Rel[]; succs: Rel[]; assigns: { res: number; name: string; kind: string; units: string; work: string; cost: string }[] }
interface Recent { path: string; name: string; modified: string }
interface Upd { current: string; latest: string | null; available: boolean; notes: string; published: string; url: string | null; sums: string | null; size: number; page: string }
interface Tpl { id: string; name: string; desc: string; color: string; tasks: number; days: number; bars: [number, number, number][] }
interface Opts { dateFmt: string; autosave: number }

const TAURI = (window as any).__TAURI__;
const inv = <T>(cmd: string, args?: object): Promise<T> => TAURI.core.invoke(cmd, args);
const $ = <E extends HTMLElement = HTMLElement>(s: string) => document.querySelector(s) as E;
const esc = (s: string) => String(s).replace(/[&<>"]/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[c]!));
const DAY = 864e5, ROW = 24, EXTRA = 40;
const today = Math.floor((Date.now() - new Date().getTimezoneOffset() * 6e4) / DAY);

// Every field the grid can show: title, width, editable, cell class, choices for the editor.
const FIELDS: Record<string, [string, number, boolean, string?]> = {
  wbs: ['WBS', 60, false], code: ['Activity ID', 84, true], name: ['Task Name', 250, true, 'nm'], dur: ['Duration', 78, true],
  start: ['Start', 86, true], finish: ['Finish', 86, true], preds: ['Predecessors', 100, true], succs: ['Successors', 100, false],
  res: ['Resource Names', 140, true], pct: ['% Complete', 78, true, 'r'], tf: ['Total Float', 78, false, 'r'], ff: ['Free Float', 78, false, 'r'],
  work: ['Work', 74, false, 'r'], cost: ['Cost', 90, false, 'r'], ctype: ['Constraint Type', 140, true], cdate: ['Constraint Date', 96, true],
  deadline: ['Deadline', 86, true], cal: ['Calendar', 100, true], mode: ['Task Mode', 76, true], active: ['Active', 56, true],
  astart: ['Actual Start', 90, true], afinish: ['Actual Finish', 90, true], rem: ['Remaining Duration', 90, true],
  bstart: ['Baseline Start', 96, false], bfinish: ['Baseline Finish', 96, false], bdur: ['Baseline Duration', 96, false],
  vstart: ['Start Variance', 86, false, 'r'], vfinish: ['Finish Variance', 86, false, 'r'], crit: ['Critical', 56, false], late: ['Late', 50, false], notes: ['Notes', 160, true],
};
const CONSTRAINTS = ['As Soon As Possible', 'As Late As Possible', 'Start No Earlier Than', 'Start No Later Than', 'Finish No Earlier Than', 'Finish No Later Than', 'Must Start On', 'Must Finish On'];
const choices = (k: string): string[] | null =>
  k === 'mode' ? ['Auto', 'Manual'] : k === 'active' ? ['Yes', 'No'] : k === 'ctype' ? CONSTRAINTS : k === 'cal' ? ['', ...V.calendars.map(c => c.name)] : k === 'res' ? V.resources.map(r => r.name) : null;

// ---------- state ----------
let V: View;
let rows: Row[] = [];
let version = '', tpls: Tpl[] = [], opts: Opts = { dateFmt: '%d-%b-%y', autosave: 5 };
let upd: Upd | null = null, updErr = '', updPct = -1;
const collapsed = new Set<number>();
const sel = new Set<number>();
let cur = { r: 0, c: 1 }, anchor = 0;
let pd = 16, d0 = 0, showCrit = true, showBase = false, showTl = true, showDet = false, tab = 'task', view = 'gantt';
let editor: HTMLInputElement | null = null;

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
const set = (uid: number, field: string, value: string) => op({ Set: { uid, field, value } });

function show(v: View) {
  V = v;
  rows = visible();
  const t = `${v.name}${v.dirty ? ' *' : ''} - Datum Plan`;
  $('#title').textContent = t;
  TAURI.window.getCurrentWindow().setTitle(t).catch(() => {});
  $<HTMLButtonElement>('[data-a=undo]').disabled = !v.undo;
  $<HTMLButtonElement>('[data-a=redo]').disabled = !v.redo;
  const n = v.rows.filter(r => !r.sum).length;
  $('#stInfo').textContent = v.rows.length ? `${n} tasks · ${v.start} → ${v.finish}${v.status ? ` · status date ${v.status}` : ''}${v.cost !== '0.00' ? ` · cost ${v.cost}` : ''}` : 'Type a task name to begin';
  $('#stMode').textContent = `New tasks: ${v.manualDefault ? 'Manually Scheduled' : 'Auto Scheduled'}`;
  setStatus(v.error ?? 'Ready', !!v.error);
  cur.r = Math.min(cur.r, rows.length + EXTRA - 1);
  layout();
  renderDetails();
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

/** Selected task uids in row order, or the current row. */
function picked(): number[] {
  const u = rows.filter(r => sel.has(r.uid)).map(r => r.uid);
  if (u.length) return u;
  const r = rows[cur.r];
  return r ? [r.uid] : [];
}
const curRow = (): Row | undefined => rows[cur.r];
/** Index in the full task list for inserting at the current row. */
const insertAt = () => { const r = curRow(); return r ? V.rows.findIndex(x => x.uid === r.uid) : V.rows.length; };

// ---------- dates ----------
const MON = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'];
const dowMon = (d: number) => (new Date(Math.floor(d) * DAY).getUTCDay() + 6) % 7;
const ymd = (d: number) => { const t = new Date(Math.floor(d) * DAY); return [t.getUTCFullYear(), t.getUTCMonth(), t.getUTCDate()]; };
const iso = (d: number) => new Date(Math.floor(d) * DAY).toISOString().slice(0, 10);
const fmtD = (d: number) => { const [y, m, dd] = ymd(d); return `${String(dd).padStart(2, '0')}-${MON[m]}-${String(y).slice(2)}`; };
/** Last calendar day a task works on. */
const endDay = (r: Row) => r.ms ? Math.floor(r.s) : Math.ceil(r.f) - 1;
const offDay = (d: number) => !V.week[dowMon(d)] || V.offDays.includes(d);

// ---------- dialogs, menus, toast, status ----------
function dialog(title: string, body: string, buttons: string[], focus?: string, wide = false): Promise<number> {
  return new Promise(res => {
    const ov = document.createElement('div');
    ov.className = 'ov';
    ov.innerHTML = `<div class="dlg${wide ? ' wide' : ''}"><header>${title}</header><div class="bd">${body}</div><footer>${buttons.map((b, i) => `<button class="btn${i ? '' : ' pri'}" data-i="${i}">${b}</button>`).join('')}</footer></div>`;
    document.body.appendChild(ov);
    // Resolve first and remove later, so callers can still read the inputs after the dialog closes.
    const done = (i: number) => { ov.style.display = 'none'; res(i); setTimeout(() => ov.remove(), 0); gbody.focus(); };
    ov.onclick = e => { const i = (e.target as HTMLElement).closest('[data-i]') as HTMLElement | null; if (i?.parentElement?.tagName === 'FOOTER') done(+i.dataset.i!); };
    ov.onkeydown = e => {
      e.stopPropagation();
      if (e.key === 'Escape') done(buttons.length - 1);
      if (e.key === 'Enter' && !['TEXTAREA', 'SELECT', 'BUTTON'].includes((e.target as HTMLElement).tagName)) { e.preventDefault(); done(0); }
    };
    (ov.querySelector(focus ?? '.btn') as HTMLElement).focus();
  });
}
const val = (id: string) => $<HTMLInputElement>('#' + id).value;
const checked = (id: string) => $<HTMLInputElement>('#' + id).checked;

type MenuItem = [string, () => void, boolean?, string?] | '-';
function menu(items: MenuItem[], x: number, y: number) {
  document.querySelector('.menu')?.remove();
  const m = document.createElement('div');
  m.className = 'menu';
  const fns: (() => void)[] = [];
  m.innerHTML = items.map(it => it === '-' ? '<hr>' : `<button data-m="${fns.push(it[1]) - 1}"${it[2] === false ? ' disabled' : ''}><span>${it[0]}</span><kbd>${it[3] ?? ''}</kbd></button>`).join('');
  document.body.appendChild(m);
  m.style.left = Math.min(x, innerWidth - m.offsetWidth - 6) + 'px';
  m.style.top = Math.min(y, innerHeight - m.offsetHeight - 6) + 'px';
  const close = () => { m.remove(); document.removeEventListener('mousedown', away, true); };
  const away = (e: MouseEvent) => { if (!m.contains(e.target as Node)) close(); };
  document.addEventListener('mousedown', away, true);
  m.onclick = e => { const b = (e.target as HTMLElement).closest('[data-m]') as HTMLElement | null; if (b) { close(); fns[+b.dataset.m!](); } };
}

let toastT = 0;
function toast(msg: string, ok = false) {
  const t = $('#toast');
  t.textContent = msg; t.className = ok ? 'ok' : ''; t.hidden = false;
  clearTimeout(toastT);
  toastT = setTimeout(() => (t.hidden = true), ok ? 2500 : 6000);
}
function setStatus(msg: string, err = false) { const s = $('#stMsg'); s.textContent = msg; s.className = err ? 'err' : ''; }
