// Commands, ribbon, view sidebar, File backstage, updates, keyboard and start-up.
interface Cmd { t: string; ic: string; run: () => void; k?: string; on?: () => boolean; big?: boolean }

const setMode = (manual: boolean) => { const rs = rows.filter(r => !r.sum && picked().includes(r.uid)); if (rs.length) op({ Batch: rs.map(r => ({ Set: { uid: r.uid, field: 'mode', value: manual ? 'Manual' : 'Auto' } })) }); };
const toggleDetails = () => { showDet = !showDet; ribbon(); renderDetails(); draw(); };
async function setBaseline() {
  const b = await dialog('Baseline', `<p>A baseline keeps today's dates so you can compare progress against them later (Tracking Gantt, variance columns, dashboard and reports).</p>
    <div class="form"><label>For</label><span><label class="chk"><input type="radio" name="bl" id="bl-all" checked> Entire project</label>
    <label class="chk"><input type="radio" name="bl" id="bl-sel"> Selected tasks (${picked().length})</label></span></div>`, ['Set Baseline', 'Clear Baseline', 'Cancel']);
  if (b === 2) return;
  await op({ Baseline: { uids: checked('bl-all') ? [] : picked(), clear: b === 1 } });
  if (b === 0) { showBase = true; ribbon(); draw(); toast('Baseline saved. Grey bars under each task show it.', true); }
}

const CMDS: Record<string, Cmd> = {
  task: { t: 'Task', ic: 'task', run: () => insert(false), k: 'Insert', big: true },
  ms: { t: 'Milestone', ic: 'ms', run: () => insert(true), big: true },
  quick: { t: 'Quick\nAdd', ic: 'quick', run: quickAdd, k: 'Ctrl+J', big: true },
  indent: { t: 'Indent', ic: 'indent', run: () => op({ Indent: { uids: picked() } }), k: 'Alt+Shift+→' },
  outdent: { t: 'Outdent', ic: 'outdent', run: () => op({ Outdent: { uids: picked() } }), k: 'Alt+Shift+←' },
  up: { t: 'Move Up', ic: 'up', run: () => moveRows(true), k: 'Alt+Shift+↑' },
  down: { t: 'Move Down', ic: 'down', run: () => moveRows(false), k: 'Alt+Shift+↓' },
  link: { t: 'Link Tasks', ic: 'link', run: () => op({ Link: { uids: picked() } }), k: 'Ctrl+F2' },
  unlink: { t: 'Unlink Tasks', ic: 'unlink', run: () => op({ Unlink: { uids: picked() } }), k: 'Ctrl+Shift+F2' },
  cut: { t: 'Cut', ic: 'cut', run: () => copyRows(true), k: 'Ctrl+X' },
  copy: { t: 'Copy', ic: 'copy', run: () => copyRows(false), k: 'Ctrl+C' },
  paste: { t: 'Paste', ic: 'paste', run: () => run('paste', { at: insertAt() }), k: 'Ctrl+V', big: true },
  fill: { t: 'Fill Down', ic: 'fill', run: fillDown, k: 'Ctrl+D' },
  manual: { t: 'Manually\nSchedule', ic: 'manual', run: () => setMode(true), big: true },
  auto: { t: 'Auto\nSchedule', ic: 'auto', run: () => setMode(false), big: true },
  inactive: { t: 'Inactivate', ic: 'inact', run: () => toggle('active', r => !r.off) },
  ontrack: { t: 'Mark on\nTrack', ic: 'track', run: () => op({ MarkOnTrack: { uids: picked() } }), big: true },
  details: { t: 'Details', ic: 'detail', run: toggleDetails, on: () => showDet, k: 'Shift+F2', big: true },
  addtl: { t: 'Add to\nTimeline', ic: 'tl', run: () => toggle('timeline', r => r.tl), big: true },
  scroll: { t: 'Scroll\nto Task', ic: 'scroll', run: scrollToTask, k: 'Ctrl+Shift+F5', big: true },
  find: { t: 'Find', ic: 'find', run: () => find(false), k: 'Ctrl+F' },
  replace: { t: 'Replace', ic: 'find', run: () => find(true), k: 'Ctrl+H' },
  del: { t: 'Delete', ic: 'del', run: del, k: 'Delete' },
  assign: { t: 'Assign\nResources', ic: 'res', run: () => openDetails('res'), big: true },
  sheet: { t: 'Resource\nSheet', ic: 'table', run: () => setView('res'), big: true },
  workload: { t: 'Workload', ic: 'load', run: () => setView('load'), big: true },
  addres: { t: 'Add\nResource', ic: 'res', run: async () => { setView('res'); await op({ ResInsert: { name: '' } }); }, big: true },
  info: { t: 'Project\nInformation', ic: 'proj', run: projectInfo, big: true },
  worktime: { t: 'Change\nWorking Time', ic: 'calendar2', run: workingTime, big: true },
  baseline: { t: 'Set\nBaseline', ic: 'base', run: setBaseline, big: true },
  import: { t: 'Import', ic: 'import', run: importFile, big: true },
  export: { t: 'Export', ic: 'export', run: () => openBs('export'), big: true },
  zin: { t: 'Zoom In', ic: 'zin', run: () => setZoom(pd * 1.5), big: true },
  zout: { t: 'Zoom Out', ic: 'zout', run: () => setZoom(pd / 1.5), big: true },
  fit: { t: 'Entire\nProject', ic: 'fit', run: fit, big: true },
  crit: { t: 'Critical\nPath', ic: 'crit', run: () => { showCrit = !showCrit; ribbon(); draw(); }, on: () => showCrit, big: true },
  base: { t: 'Baseline\nBars', ic: 'base', run: () => { showBase = !showBase; ribbon(); draw(); }, on: () => showBase, big: true },
  tl: { t: 'Timeline', ic: 'tl', run: () => { showTl = !showTl; ribbon(); draw(); }, on: () => showTl, big: true },
  cols: { t: 'Add\nColumn', ic: 'cols', run: () => { const r = ghead.getBoundingClientRect(); fieldPicker(r.left + 40, r.bottom, V.columns.length); }, big: true },
  gantt: { t: 'Gantt\nChart', ic: 'gantt', run: () => setView('gantt'), on: () => view === 'gantt', big: true },
  list: { t: 'Task\nList', ic: 'list', run: () => setView('list'), on: () => view === 'list', big: true },
  cal: { t: 'Calendar', ic: 'cal', run: () => setView('cal'), on: () => view === 'cal', big: true },
  dash: { t: 'Dashboard', ic: 'dash', run: () => setView('dash'), on: () => view === 'dash', big: true },
  report: { t: 'Reports', ic: 'report', run: () => setView('report'), on: () => view === 'report', big: true },
  palette: { t: 'Search', ic: 'search', run: palette, k: 'Ctrl+K' },
  options: { t: 'Options', ic: 'opts', run: optionsDialog, big: true },
  updates: { t: 'Check for\nUpdates', ic: 'upd', run: () => openBs('about'), big: true },
  keys: { t: 'Keyboard\nShortcuts', ic: 'keys', run: shortcuts, k: 'F1', big: true },
  about: { t: 'About\nDatum Plan', ic: 'info', run: () => openBs('about'), big: true },
  save: { t: 'Save', ic: 'save', run: () => save(false), k: 'Ctrl+S' },
  saveas: { t: 'Save As', ic: 'save', run: () => save(true), k: 'Ctrl+Shift+S' },
  open: { t: 'Open', ic: 'open', run: () => openPlan(), k: 'Ctrl+O' },
  new: { t: 'New plan', ic: 'new', run: () => newPlan(), k: 'Ctrl+N' },
  print: { t: 'Print', ic: 'print', run: () => print(), k: 'Ctrl+P' },
};
for (let p = 0; p <= 100; p += 25) CMDS['p' + p] = { t: `${p}%`, ic: 'pct', run: () => pct(p) };

type Item = string | string[] | 'pct';
const RIBBON: Record<string, [string, Item[]][]> = {
  task: [
    ['View', ['gantt']], ['Clipboard', ['paste', ['cut', 'copy', 'fill']]],
    ['Schedule', ['pct', ['indent', 'outdent', 'up'], ['link', 'unlink', 'down']]], ['Tasks', ['manual', 'auto', ['inactive'], 'ontrack']],
    ['Insert', ['task', 'ms', 'quick']], ['Properties', ['details', 'addtl']], ['Editing', ['scroll', ['find', 'replace', 'del']]],
  ],
  resource: [['View', ['sheet', 'workload']], ['Assignments', ['assign', 'addres']]],
  project: [['Properties', ['info', 'worktime']], ['Schedule', ['baseline', 'ontrack']], ['Exchange', ['import', 'export']]],
  view: [
    ['Task Views', ['gantt', 'list', 'cal']], ['Reports', ['dash', 'report']], ['Zoom', ['zin', 'zout', 'fit']],
    ['Show', ['crit', 'base', 'tl', 'details']], ['Columns', ['cols']],
  ],
  help: [['Help', ['updates', 'keys', 'options', 'about']]],
};
function ribbon() {
  const btn = (id: string, big = false) => {
    const c = CMDS[id];
    return `<button class="rb${big && c.big ? ' big' : ''}${c.on?.() ? ' on' : ''}" data-r="${id}" title="${c.t.replace('\n', ' ')}${c.k ? ` (${c.k})` : ''}"><svg><use href="#i-${c.ic}"/></svg><span>${c.t.replace('\n', big && c.big ? '<br>' : ' ')}</span></button>`;
  };
  $('#ribbon').innerHTML = RIBBON[tab].map(([cap, items]) => `<div class="grp"><div class="items">${items.map(it =>
    it === 'pct' ? `<div class="pcts">${[0, 25, 50, 75, 100].map(p => btn('p' + p)).join('')}</div>`
      : Array.isArray(it) ? `<div class="stack">${it.map(i => btn(i)).join('')}</div>` : btn(it, true)).join('')}</div><div class="cap">${cap}</div></div>`).join('');
}

// ---------- view sidebar ----------
const VIEWS: ([string, string, string, string?] | '-')[] = [
  ['gantt', 'gantt', 'Gantt'], ['list', 'list', 'Task List'], ['cal', 'cal', 'Calendar'], '-',
  ['res', 'res', 'Resources'], ['load', 'load', 'Workload'], '-', ['dash', 'dash', 'Dashboard'], ['report', 'report', 'Reports'], '-',
  ['net', 'net', 'Network', '0.8'],
];
function renderViews() {
  $('#views').innerHTML = VIEWS.map(v => v === '-' ? '<hr>' : `<button data-v="${v[0]}" class="${view === v[0] ? 'on' : ''}${v[3] ? ' soon' : ''}"${v[3] ? ` title="${v[2]}: coming in ${v[3]}"` : ""}><svg><use href="#i-${v[1]}"/></svg>${v[2]}</button>`).join('');
}
function setView(v: string) {
  const def = VIEWS.find(x => x !== '-' && x[0] === v) as [string, string, string, string?];
  if (def[3]) return toast(`${def[2]} arrives in version ${def[3]}. See the roadmap under Help › About.`);
  view = v;
  renderViews();
  ribbon();
  const grid = v === 'gantt' || v === 'list';
  $('#ganttView').hidden = !grid;
  $('#page').hidden = grid;
  $('#ganttView').classList.toggle('list', v === 'list');
  if (grid) { layout(); gbody.focus(); } else renderPage();
}
$('#views').onclick = e => { const b = (e.target as HTMLElement).closest('[data-v]') as HTMLElement | null; if (b) setView(b.dataset.v!); };

// ---------- files ----------
async function guard(): Promise<boolean> {
  if (!V?.dirty) return true;
  const b = await dialog('Datum Plan', `<p>Do you want to save changes to "${esc(V.name)}"?</p>`, ['Save', "Don't Save", 'Cancel']);
  if (b === 0) return !!(await run('save', { asNew: false }));
  return b === 1;
}
function afterLoad(v: View) {
  collapsed.clear(); sel.clear(); cur = { r: 0, c: 1 };
  show(v);
  closeBs();
  setView('gantt');
  fit();
}
async function newPlan(template?: string) {
  if (!(await guard())) return;
  const v = await call<View>('new_plan', { template: template ?? null });
  if (!v) return;
  afterLoad(v);
  if (!template) setZoom(16, today + 20);
}
async function openPlan(path?: string) {
  if (!(await guard())) return;
  const v = await call<View | null>('open', { path: path ?? null });
  if (v) afterLoad(v);
}
async function save(asNew: boolean) {
  const v = await run('save', { asNew });
  if (v) toast('Saved ' + (v.path ?? ''), true);
}
const undoRedo = (redo: boolean) => run('history', { redo });

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
      <h2>Datum Plan</h2><p class="muted">Project scheduling with the strengths of Microsoft Project and Primavera P6. Plans are saved as .dplan files on your computer, and exchange with MS Project (XML), Primavera (XER) and Excel.<br>© 2026 Edgar Mendonca · MIT licence</p></div>`;
    return;
  }
  if (bsPage === 'export') {
    const x = (k: string, t: string, d: string) => `<button class="tile" data-x="${k}"><div class="pic"><svg class="ic"><use href="#i-export"/></svg></div><b>${t}</b><span>${d}</span></button>`;
    main.innerHTML = `<h1>Export</h1><div class="tiles" style="max-width:900px">${x('xlsx', 'Excel workbook (.xlsx)', 'The task list with outline, dates, links, resources and progress.')}
      ${x('csv', 'CSV (.csv)', 'Plain text for any spreadsheet or database.')}${x('xml', 'MS Project XML (.xml)', 'Opens in Microsoft Project, ProjectLibre and others, with calendars, resources and baselines.')}
      ${x('xer', 'Primavera P6 (.xer)', 'Summary tasks become WBS; tasks become activities with relationships and resources.')}</div>
      <p class="muted">To bring files in, use File › Import (XML, XER, XLSX or CSV).</p>`;
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
$('#split').onmousedown = e => {
  const g = $('#grid'), x = e.clientX, w0 = g.offsetWidth;
  const mv = (m: MouseEvent) => { g.style.width = Math.max(120, w0 + m.clientX - x) + 'px'; draw(); };
  document.addEventListener('mousemove', mv);
  document.addEventListener('mouseup', () => document.removeEventListener('mousemove', mv), { once: true });
};
$('#zoom').oninput = e => setZoom(0.3 * Math.pow(200, +(e.target as HTMLInputElement).value / 100));
$('#ribbon').onclick = e => { const b = (e.target as HTMLElement).closest('[data-r]') as HTMLElement | null; if (b) CMDS[b.dataset.r!].run(); };
$('#tabs').onclick = e => {
  const t = (e.target as HTMLElement).dataset.tab;
  if (!t) return;
  if (t === 'file') return openBs('home');
  tab = t;
  document.querySelectorAll('#tabs button').forEach(b => b.classList.toggle('on', (b as HTMLElement).dataset.tab === t));
  ribbon();
};
$('#stMode').onclick = () => op({ Project: { name: V.name, start: V.startIso, status: V.statusIso, calendar: V.calendar, day_hours: V.dayHours, manual_default: !V.manualDefault } });
const ACTIONS: Record<string, () => void> = {
  save: () => save(false), undo: () => undoRedo(false), redo: () => undoRedo(true), home: () => openBs('home'),
  zin: () => setZoom(pd * 1.5), zout: () => setZoom(pd / 1.5), palette,
};
document.querySelectorAll('[data-a]').forEach(b => ((b as HTMLElement).onclick = () => ACTIONS[(b as HTMLElement).dataset.a!]()));
$('#bs aside').onclick = e => {
  const b = ((e.target as HTMLElement).closest('[data-b]') as HTMLElement | null)?.dataset.b;
  if (b === 'back') closeBs();
  else if (b === 'new') newPlan();
  else if (b === 'open') openPlan();
  else if (b === 'import') importFile();
  else if (b === 'save' || b === 'saveas') save(b === 'saveas').then(closeBs);
  else if (b === 'print') { closeBs(); setTimeout(print, 50); }
  else if (b === 'options') optionsDialog();
  else if (b) { bsPage = b; renderBs(); }
};
$('#bsMain').onclick = e => {
  const el = e.target as HTMLElement, h = (el.closest('[data-h]') as HTMLElement | null)?.dataset.h;
  const p = (el.closest('[data-p]') as HTMLElement | null)?.dataset.p, u = el.dataset.u, x = (el.closest('[data-x]') as HTMLElement | null)?.dataset.x;
  if (h === 'blank') newPlan();
  else if (h?.startsWith('t:')) newPlan(h.slice(2));
  else if (h === 'open') openPlan();
  else if (p) openPlan(p);
  else if (x) exportFile(x);
  else if (u === 'check') checkUpdate();
  else if (u === 'install') installUpdate();
  else if (u === 'page') { e.preventDefault(); navigator.clipboard?.writeText(upd?.page ?? 'https://github.com/Edgar-Mendonca/Datum-Plan/releases'); toast('Releases link copied to the clipboard.', true); }
};

document.addEventListener('keydown', e => {
  if (editor || document.querySelector('.ov')) return;
  const k = e.key, ctrl = e.ctrlKey || e.metaKey, lk = k.toLowerCase();
  if (!$('#bs').hidden) { if (k === 'Escape') closeBs(); return; }
  const go = (dr: number, dc: number) => { e.preventDefault(); select(cur.r + dr, cur.c + dc, { shiftKey: e.shiftKey && dr !== 0, ctrlKey: false }); };
  const grid = !$('#ganttView').hidden && !(e.target as HTMLElement).closest('#details');
  if (ctrl && lk === 'k') { e.preventDefault(); palette(); }
  else if (ctrl && lk === 's') { e.preventDefault(); save(e.shiftKey); }
  else if (ctrl && lk === 'o') { e.preventDefault(); openPlan(); }
  else if (ctrl && lk === 'n') { e.preventDefault(); newPlan(); }
  else if (ctrl && lk === 'p') { e.preventDefault(); print(); }
  else if (ctrl && lk === 'z') { e.preventDefault(); undoRedo(false); }
  else if (ctrl && lk === 'y') { e.preventDefault(); undoRedo(true); }
  else if (ctrl && lk === 'j') { e.preventDefault(); quickAdd(); }
  else if (k === 'F1') { e.preventDefault(); shortcuts(); }
  else if (!grid) { /* pages and the details pane take no grid keys */ }
  else if (ctrl && lk === 'f') { e.preventDefault(); find(false); }
  else if (ctrl && lk === 'h') { e.preventDefault(); find(true); }
  else if (k === 'F3') { e.preventDefault(); findNext(); }
  else if (ctrl && lk === 'd') { e.preventDefault(); fillDown(); }
  else if (ctrl && k === 'F2') op(e.shiftKey ? { Unlink: { uids: picked() } } : { Link: { uids: picked() } });
  else if (ctrl && e.shiftKey && k === 'F5') scrollToTask();
  else if (e.altKey && e.shiftKey && k.startsWith('Arrow')) {
    e.preventDefault();
    if (k === 'ArrowRight') op({ Indent: { uids: picked() } }); else if (k === 'ArrowLeft') op({ Outdent: { uids: picked() } }); else moveRows(k === 'ArrowUp');
  }
  else if (k === 'F2' && e.shiftKey) openDetails('general');
  else if (ctrl) { /* leave Ctrl+C/X/V and others to the browser events */ }
  else if (k === 'ArrowDown') go(1, 0);
  else if (k === 'ArrowUp') go(-1, 0);
  else if (k === 'ArrowRight') go(0, 1);
  else if (k === 'ArrowLeft') go(0, -1);
  else if (k === 'Tab') go(0, e.shiftKey ? -1 : 1);
  else if (k === 'PageDown') go(Math.floor(gbody.clientHeight / ROW), 0);
  else if (k === 'PageUp') go(-Math.floor(gbody.clientHeight / ROW), 0);
  else if (k === 'Home') go(0, 1 - cur.c);
  else if (k === 'Enter' || k === 'F2') { e.preventDefault(); startEdit(); }
  else if (k === 'Insert') insert(false);
  else if (k === 'Delete') del();
  else if (k === 'Escape') { sel.clear(); renderRows(); draw(); }
  else if (k.length === 1 && !e.altKey) { e.preventDefault(); startEdit(k); }
});
window.onresize = () => { renderRows(); draw(); };
new ResizeObserver(() => { renderRows(); draw(); }).observe($('#cwrap'));
TAURI.event.listen('close-requested', async () => { if (await guard()) inv('quit'); });
TAURI.event.listen('update-progress', (e: { payload: number }) => { updPct = e.payload; if (!$('#bs').hidden) renderBs(); });

// ---------- start ----------
(async () => {
  version = await inv<string>('version');
  opts = await inv<Opts>('options');
  tpls = await inv<Tpl[]>('templates');
  renderViews();
  ribbon();
  $('#grid').style.width = Math.min(780, innerWidth * 0.5) + 'px';
  show(await inv<View>('state'));
  setZoom(16, today + 20);
  armAutosave();
  const f = await inv<string | null>('startup_file');
  const rec = f ? null : await inv<{ name: string; path: string | null; saved: string } | null>('recovery');
  if (f) await openPlan(f);
  else if (rec && await dialog('Recover unsaved work', `<p>Datum Plan closed before <b>${esc(rec.name)}</b> was saved. A copy from ${esc(rec.saved)} is available.</p>`, ['Recover', 'Discard']) === 0) {
    const v = await call<View>('recover', { keep: true });
    if (v) { afterLoad(v); toast('Recovered. Save the plan to keep it.', true); }
  } else {
    if (rec) inv('recover', { keep: false });
    openBs('home');
  }
  checkUpdate();
})();
