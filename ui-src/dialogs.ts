// Dialogs: project information, working time (calendars), options, import mapping, command search,
// quick add and keyboard shortcuts.

async function projectInfo() {
  const b = await dialog('Project Information', `<div class="form">
    <label>Project name</label><input id="p-name" value="${esc(V.name)}">
    <label>Start date</label><input id="p-start" type="date" value="${V.startIso}">
    <label>Finish date</label><span>${V.finish} (calculated)</span>
    <label>Status date</label><span><input id="p-status" type="date" value="${V.statusIso}"> <span class="muted">progress is reported up to this day</span></span>
    <label>Project calendar</label>${sel1('p-cal', V.calendars.map(c => c.name), V.calendar)}
    <label>Hours per day</label><span><input id="p-hours" type="number" min="1" max="24" step="0.5" value="${V.dayHours}" style="width:80px"> <span class="muted">used when durations are typed in days</span></span>
    <label>New tasks</label><label class="chk"><input id="p-manual" type="checkbox"${V.manualDefault ? ' checked' : ''}> Manually scheduled</label>
    </div>`, ['OK', 'Cancel'], '#p-name');
  if (b !== 0) return;
  op({ Project: { name: val('p-name'), start: val('p-start'), status: val('p-status'), calendar: val('p-cal'), day_hours: +val('p-hours') || 8, manual_default: checked('p-manual') } });
}

// ---------- working time ----------
const hm = (m: number) => `${String(Math.floor(m / 60)).padStart(2, '0')}:${String(m % 60).padStart(2, '0')}`;
const fmtHours = (h: Hours) => h.map(([a, b]) => `${hm(a)}-${hm(b)}`).join(', ');
function parseHours(s: string): Hours | null {
  if (!s.trim()) return [];
  const out: Hours = [];
  for (const part of s.split(/[,;]/)) {
    const m = part.trim().match(/^(\d{1,2})(?::(\d\d))?\s*-\s*(\d{1,2})(?::(\d\d))?$/);
    if (!m) return null;
    const a = +m[1] * 60 + +(m[2] ?? 0), b = +m[3] * 60 + +(m[4] ?? 0) || 1440;
    if (b <= a || b > 1440) return null;
    out.push([a, b]);
  }
  return out;
}

async function workingTime() {
  const DAYS = ['Monday', 'Tuesday', 'Wednesday', 'Thursday', 'Friday', 'Saturday', 'Sunday'];
  const edits = new Map<string, { old: string | null; cal: Cal }>(V.calendars.map(c => [c.name, { old: c.name, cal: JSON.parse(JSON.stringify(c)) }]));
  const deleted: string[] = [];
  let curName = V.calendar;
  const p = dialog('Change Working Time', '<div id="wt"></div>', ['OK', 'Cancel'], undefined, true);
  const box = $('#wt');
  const render = () => {
    const c = edits.get(curName)!.cal;
    box.innerHTML = `<div class="ptools"><label>Calendar</label>${sel1('wt-cal', [...edits.keys()], curName)}
      ${curName === V.calendar ? '<span class="pill ok">project calendar</span>' : ''}<span class="grow"></span>
      <input id="wt-new" placeholder="New calendar name" style="width:150px">${sel1('wt-base', ['Copy of this calendar', 'Standard (Mon–Fri 8–5)', '24 hours, 7 days'], 'Copy of this calendar')}<button class="btn" data-wt="new">Create</button>
      <button class="btn" data-wt="rename">Rename</button><button class="btn" data-wt="del"${curName === V.calendar ? ' disabled' : ''}>Delete</button></div>
      <h4>Work week</h4><table class="tbl"><tr><th>Day</th><th>Working</th><th>Working hours (e.g. 08:00-12:00, 13:00-17:00)</th></tr>
      ${DAYS.map((d, i) => `<tr><td>${d}</td><td><input type="checkbox" data-wd="${i}"${c.week[i].length ? ' checked' : ''}></td><td><input data-wh="${i}" value="${fmtHours(c.week[i])}" style="width:260px"${c.week[i].length ? '' : ' disabled'}></td></tr>`).join('')}</table>
      <h4>Exceptions: holidays, shutdowns and special working days</h4><table class="tbl"><tr><th>Name</th><th>From</th><th>To</th><th>Working hours (blank = not working)</th><th></th></tr>
      ${c.exceptions.map((e, k) => `<tr><td><input data-en="${k}" value="${esc(e.name)}" style="width:150px"></td><td><input type="date" data-ef="${k}" value="${e.from}"></td><td><input type="date" data-et="${k}" value="${e.to}"></td><td><input data-eh="${k}" value="${fmtHours(e.hours)}" style="width:170px"></td><td><button data-ex="${k}">✕</button></td></tr>`).join('')}
      <tr><td colspan="5"><button class="btn" data-wt="exc">Add exception</button></td></tr></table>`;
  };
  render();
  box.onclick = e => {
    const el = e.target as HTMLElement, a = el.dataset.wt, c = edits.get(curName)!.cal;
    if (a === 'exc') { const d = iso(today); c.exceptions.push({ name: 'Holiday', from: d, to: d, hours: [] }); render(); }
    else if (el.dataset.ex) { c.exceptions.splice(+el.dataset.ex, 1); render(); }
    else if (a === 'new') {
      const n = val('wt-new').trim(), base = val('wt-base');
      if (!n || edits.has(n)) return toast(n ? `There is already a calendar called "${n}".` : 'Type a name for the new calendar.');
      const std: Hours = [[480, 720], [780, 1020]];
      const cal: Cal = base.startsWith('Copy') ? { ...JSON.parse(JSON.stringify(c)), name: n }
        : base.startsWith('24') ? { name: n, week: DAYS.map(() => [[0, 1440]] as Hours), exceptions: [] } : { name: n, week: DAYS.map((_, i) => i < 5 ? std : []), exceptions: [] };
      edits.set(n, { old: null, cal });
      curName = n; render();
    } else if (a === 'rename') {
      const n = val('wt-new').trim();
      if (!n || edits.has(n)) return toast('Type a new, unused name in the box first.');
      const ed = edits.get(curName)!;
      edits.delete(curName); ed.cal.name = n; edits.set(n, ed);
      curName = n; render();
    } else if (a === 'del') {
      const ed = edits.get(curName)!;
      if (ed.old) deleted.push(ed.old);
      edits.delete(curName); curName = [...edits.keys()][0]; render();
    }
  };
  box.onchange = e => {
    const el = e.target as HTMLInputElement, d = el.dataset, c = edits.get(curName)!.cal;
    if (el.id === 'wt-cal') { curName = el.value; render(); }
    else if (d.wd) { c.week[+d.wd] = el.checked ? [[480, 720], [780, 1020]] : []; render(); }
    else if (d.wh) { const h = parseHours(el.value); if (h) c.week[+d.wh] = h; else toast('Write hours like 08:00-12:00, 13:00-17:00.'); }
    else if (d.en) c.exceptions[+d.en].name = el.value;
    else if (d.ef) c.exceptions[+d.ef].from = el.value;
    else if (d.et) c.exceptions[+d.et].to = el.value;
    else if (d.eh) { const h = parseHours(el.value); if (h) c.exceptions[+d.eh].hours = h; else toast('Write hours like 08:00-12:00, or leave blank for a day off.'); }
  };
  if (await p !== 0) return;
  const ops: object[] = [...edits.values()].map(e => ({ CalPut: { old: e.old, cal: e.cal } }));
  deleted.forEach(n => ops.push({ CalDelete: { name: n } }));
  op({ Batch: ops });
}

async function optionsDialog() {
  const fmts: [string, string][] = [['%d-%b-%y', '24-Oct-26'], ['%d-%b-%Y', '24-Oct-2026'], ['%a %d-%b-%y', 'Sat 24-Oct-26'], ['%d/%m/%Y', '24/10/2026'], ['%m/%d/%Y', '10/24/2026'], ['%Y-%m-%d', '2026-10-24'], ['%d %b %Y', '24 Oct 2026']];
  const b = await dialog('Options', `<div class="form">
    <label>Date format</label><select id="o-fmt">${fmts.map(([f, x]) => `<option value="${f}"${f === opts.dateFmt ? ' selected' : ''}>${x}</option>`).join('')}</select>
    <label>Recovery copy</label><select id="o-auto">${[0, 2, 5, 10, 15].map(m => `<option value="${m}"${m === opts.autosave ? ' selected' : ''}>${m ? `Every ${m} minutes` : 'Off'}</option>`).join('')}</select>
    <label></label><span class="muted">Unsaved work is copied in the background. If Datum Plan closes unexpectedly, it offers the copy next time.</span>
    <label>This plan</label><span><button class="btn" id="o-proj">Project Information…</button> <button class="btn" id="o-cal">Working time…</button></span>
    </div>`, ['OK', 'Cancel'], '#o-fmt');
  if (b !== 0) return;
  opts = { dateFmt: val('o-fmt'), autosave: +val('o-auto') };
  const v = await call<View>('set_options', { opts });
  if (v) show(v);
  armAutosave();
}
document.addEventListener('click', e => {
  const id = (e.target as HTMLElement).id;
  if (id === 'o-proj' || id === 'o-cal') { (document.querySelector('.ov .btn:last-child') as HTMLElement)?.click(); setTimeout(id === 'o-proj' ? projectInfo : workingTime, 0); }
});

let autosaveT = 0;
function armAutosave() {
  clearInterval(autosaveT);
  if (opts.autosave > 0) autosaveT = setInterval(() => inv('autosave').catch(() => {}), opts.autosave * 60000);
}

// ---------- import and export ----------
async function importFile() {
  if (!(await guard())) return;
  const r = await call<{ plan: View | null; table: { headers: string[]; rows: string[][]; guess: string[] } | null; fields: [string, string][] } | null>('import');
  if (!r) return;
  if (r.plan) { afterLoad(r.plan); return toast(`Imported "${r.plan.name}". Save it as a .dplan file to keep it.`, true); }
  const t = r.table!;
  const options = (g: string) => `<option value="">(skip)</option>${r.fields.map(([k, l]) => `<option value="${k}"${k === g ? ' selected' : ''}>${l}</option>`).join('')}`;
  const b = await dialog('Import a task list', `<p>Match each column to a Datum Plan field. Predecessors are matched to the ID column when there is one, otherwise to row numbers.</p>
    <div class="mapwrap"><table class="tbl map"><tr>${t.headers.map((h, i) => `<th>${esc(h || `Column ${i + 1}`)}<br><select data-map="${i}">${options(t.guess[i])}</select></th>`).join('')}</tr>
    ${t.rows.map(row => `<tr>${t.headers.map((_, i) => `<td>${esc(row[i] ?? '')}</td>`).join('')}</tr>`).join('')}</table></div>`, ['Import', 'Cancel'], undefined, true);
  if (b !== 0) return;
  const map = t.headers.map((_, i) => (document.querySelector(`[data-map="${i}"]`) as HTMLSelectElement | null)?.value ?? t.guess[i]);
  const res = await call<{ plan: View; skipped: number }>('import_table', { map });
  if (!res) return;
  afterLoad(res.plan);
  toast(res.skipped ? `Imported, but ${res.skipped} cell${res.skipped === 1 ? '' : 's'} could not be read and were left blank.` : 'Imported. Save it as a .dplan file to keep it.', !res.skipped);
}

async function exportFile(kind: string) {
  const p = await call<string | null>('export', { kind });
  if (p) toast(`Exported to ${p}`, true);
}

// ---------- command search (Ctrl+K) and quick add (Ctrl+J) ----------
function palette() {
  const ov = document.createElement('div');
  ov.className = 'ov top';
  ov.innerHTML = '<div class="pal"><input placeholder="Type a command, view or task name…"><div class="pl"></div></div>';
  document.body.appendChild(ov);
  const inp = ov.querySelector('input')!, list = ov.querySelector('.pl') as HTMLElement;
  let items: [string, string, () => void][] = [], k = 0;
  const close = () => { ov.remove(); gbody.focus(); };
  const fill = () => {
    const q = inp.value.trim().toLowerCase();
    const cmds = Object.values(CMDS).filter(c => c.t.toLowerCase().replace('\n', ' ').includes(q)).map(c => [c.t.replace('\n', ' '), c.k ?? 'Command', c.run] as [string, string, () => void]);
    const tasks = q ? V.rows.filter(r => r.name.toLowerCase().includes(q)).slice(0, 8).map(r => [`${r.id} · ${r.name}`, 'Go to task', () => { setView('gantt'); collapsed.clear(); rows = visible(); layout(); select(rows.findIndex(x => x.uid === r.uid), 1); scrollToTask(); }] as [string, string, () => void]) : [];
    items = [...cmds.slice(0, 12), ...tasks];
    k = Math.min(k, items.length - 1);
    list.innerHTML = items.map(([t, kk], i) => `<button data-i="${i}" class="${i === k ? 'on' : ''}"><span>${esc(t)}</span><kbd>${esc(kk)}</kbd></button>`).join('') || '<div class="empty">No matches</div>';
  };
  inp.oninput = () => { k = 0; fill(); };
  inp.onkeydown = e => {
    e.stopPropagation();
    if (e.key === 'Escape') close();
    else if (e.key === 'ArrowDown' || e.key === 'ArrowUp') { e.preventDefault(); k = (k + (e.key === 'ArrowDown' ? 1 : -1) + items.length) % Math.max(1, items.length); fill(); }
    else if (e.key === 'Enter' && items[k]) { e.preventDefault(); const f = items[k][2]; close(); setTimeout(f, 0); }
  };
  ov.onmousedown = e => { const b = (e.target as HTMLElement).closest('[data-i]') as HTMLElement | null; if (b) { close(); items[+b.dataset.i!][2](); } else if (e.target === ov) close(); };
  fill();
  inp.focus();
}

async function quickAdd() {
  const b = await dialog('Quick add', `<p class="muted">One line per task: name, duration, links, people and dates.</p>
    <input id="qa" style="width:100%" placeholder="Pour slab 5d after 4 @Site Crew[50%] by 20-Nov-26">
    <p class="muted">Words: <b>5d</b>, <b>4h</b> or <b>2w</b> for duration · <b>after 3, 5SS</b> for predecessors · <b>@Name</b> for resources · <b>start</b> date · <b>by</b> deadline · <b>milestone</b>.</p>`, ['Add', 'Cancel'], '#qa');
  if (b !== 0) return;
  const text = val('qa').trim();
  if (!text) return;
  const at = curRow() ? insertAt() + 1 : V.rows.length;
  if (await op({ QuickAdd: { at, text } })) select(rows.findIndex(r => r.uid === V.rows[at]?.uid), 1);
}

const KEYS: [string, string][] = [
  ['Ctrl+K', 'Search commands and tasks'], ['Ctrl+J', 'Quick add a task in one line'], ['Ctrl+N / Ctrl+O / Ctrl+S', 'New, open, save'], ['Ctrl+Shift+S', 'Save as'],
  ['Ctrl+Z / Ctrl+Y', 'Undo, redo'], ['Ctrl+C / Ctrl+X / Ctrl+V', 'Copy, cut, paste tasks (paste rows from Excel too)'], ['Ctrl+D', 'Fill down'],
  ['Ctrl+F / F3 / Ctrl+H', 'Find, find next, replace'], ['Insert', 'New task above the selection'], ['Delete', 'Delete the selected tasks'],
  ['Enter / F2', 'Edit the cell'], ['Tab / Shift+Tab', 'Next / previous cell'], ['Alt+Shift+→ / ←', 'Indent / outdent'], ['Alt+Shift+↑ / ↓', 'Move tasks up / down'],
  ['Ctrl+F2 / Ctrl+Shift+F2', 'Link / unlink the selected tasks'], ['Shift+F2', 'Task details'], ['Ctrl+Shift+F5', 'Scroll to task'],
  ['Ctrl+wheel', 'Zoom the Gantt chart'], ['Right-click', 'Task and column menus'], ['Shift+click, Ctrl+click', 'Select several tasks'],
];
const shortcuts = () => dialog('Keyboard Shortcuts', `<table class="keys">${KEYS.map(([k, d]) => `<tr><td>${k}</td><td>${d}</td></tr>`).join('')}</table>`, ['Close']);
