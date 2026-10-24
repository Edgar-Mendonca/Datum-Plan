// Full-page views: Calendar, Resource Sheet, Workload, Dashboard and Reports.
const leaves = () => V.rows.filter(r => !r.sum && !r.off);
const span = (r: Row) => r.ms ? 0 : r.f - r.s;
const pctNow = (L: Row[]) => { const w = L.reduce((a, r) => a + span(r), 0); return w ? L.reduce((a, r) => a + span(r) * r.pct, 0) / w : 0; };
const planned = (L: Row[], d: number, base = false) => {
  const sp = (r: Row) => base ? (r.bf ?? r.f) - (r.bs ?? r.s) : span(r), st = (r: Row) => base ? r.bs ?? r.s : r.s;
  const w = L.reduce((a, r) => a + sp(r), 0);
  return w ? 100 * L.reduce((a, r) => a + (sp(r) > 0 ? sp(r) * Math.min(1, Math.max(0, (d - st(r)) / sp(r))) : 0), 0) / w : 0;
};
const tbl = (L: Row[]) => L.length
  ? `<table class="tbl"><tr><th>ID</th><th>Task</th><th>Start</th><th>Finish</th><th class="r">Done</th></tr>${L.map(r =>
    `<tr data-u="${r.uid}" class="link"><td>${r.id}</td><td${r.late ? ' class="late"' : ''}>${esc(r.name)}</td><td>${r.start}</td><td>${r.finish}</td><td class="r">${r.pct}%</td></tr>`).join('')}</table>`
  : '<div class="empty">None</div>';
const meter = (p: number) => `<div class="meter"><i style="width:${p}%"></i></div>`;

let calOff = 0, rep: { unit: 'week' | 'month'; off: number } = { unit: 'week', off: 0 }, wlWeekly = true;
async function renderPage() {
  const pg = $('#page');
  if (view === 'res') { pg.innerHTML = resourcePage(); return; }
  if (!V.rows.length) { pg.innerHTML = '<h1>Nothing to show yet</h1><p class="muted">Add tasks in the Gantt view, or start from a template under File › Home.</p>'; return; }
  if (view === 'load') { pg.innerHTML = await workloadPage(); return; }
  pg.innerHTML = view === 'cal' ? calendarPage() : view === 'dash' ? dashboardPage() : reportPage();
}

function calendarPage(): string {
  const [ty, tm] = ymd(today), first = Date.UTC(ty, tm + calOff, 1) / DAY, [y, m] = ymd(first), start = first - dowMon(first);
  const byDay = new Map<number, Row[]>();
  const add = (d: number, r: Row) => byDay.set(d, [...(byDay.get(d) ?? []), r]);
  for (const r of leaves()) { add(Math.floor(r.s), r); if (!r.ms && endDay(r) !== Math.floor(r.s)) add(endDay(r), r); }
  let cells = '';
  for (let k = 0; k < 42; k++) {
    const d = start + k, [, mm, dd] = ymd(d), it = byDay.get(d) ?? [];
    const chip = (r: Row) => `<div class="chip${r.ms ? ' ms' : ''}${showCrit && r.crit ? ' crit' : ''}" data-u="${r.uid}" title="${esc(r.name)} · ${r.start} → ${r.finish}">${r.ms ? '◆' : Math.floor(r.s) === d ? '▶' : '■'} ${esc(r.name)}</div>`;
    cells += `<div class="day${mm !== m ? ' out' : ''}${offDay(d) ? ' nw' : ''}${d === today ? ' today' : ''}"><span class="dn">${dd}</span>${it.slice(0, 4).map(chip).join('')}${it.length > 4 ? `<div class="chip more">+${it.length - 4} more</div>` : ''}</div>`;
  }
  const MONF = ['January', 'February', 'March', 'April', 'May', 'June', 'July', 'August', 'September', 'October', 'November', 'December'];
  return `<div class="ptools"><h1>${MONF[m]} ${y}</h1><span class="grow"></span><div class="seg"><button data-cal="-1">‹</button><button data-cal="0">Today</button><button data-cal="1">›</button></div></div>
    <div class="calgrid">${['Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat', 'Sun'].map(x => `<div class="dh">${x}</div>`).join('')}${cells}</div>
    <p class="muted">▶ starts · ■ finishes · ◆ milestone. Click a task to open it in the Gantt chart.</p>`;
}

function resourcePage(): string {
  const kinds = ['Work', 'Material', 'Cost'];
  const c = (r: ResRow, f: string, v: string, w = 90) => `<td><input data-rf="${f}" data-ru="${r.uid}" value="${esc(v)}" style="width:${w}px"></td>`;
  return `<div class="ptools"><h1>Resource Sheet</h1><span class="grow"></span><button class="btn" data-addres="1">Add resource</button></div>
    <div class="panel"><table class="tbl sheet"><tr><th>ID</th><th></th><th>Resource Name</th><th>Type</th><th>Group</th><th>Max Units</th><th>Std. Rate /h</th><th>Cost/Use</th><th>Material Label</th><th class="r">Work</th><th class="r">Cost</th><th></th></tr>
    ${V.resources.map(r => `<tr><td>${r.id}</td><td>${r.over ? '<span class="late" title="Overallocated: more work than capacity on some days">⚠</span>' : ''}</td>${c(r, 'name', r.name, 180)}
      <td><select data-rf="kind" data-ru="${r.uid}">${kinds.map(k => `<option${k === r.kind ? ' selected' : ''}>${k}</option>`).join('')}</select></td>
      ${c(r, 'group', r.group)}${r.kind === 'Work' ? c(r, 'max', r.max, 70) : '<td></td>'}${c(r, 'rate', r.rate, 80)}${c(r, 'per_use', r.perUse, 80)}${r.kind === 'Material' ? c(r, 'label', r.label, 70) : '<td></td>'}
      <td class="r">${r.work}</td><td class="r">${r.cost}</td><td><button data-delres="${r.uid}" title="Delete resource">✕</button></td></tr>`).join('')}
    </table>${V.resources.length ? '' : '<div class="empty">No resources yet. Add one here, or type names in the Resource Names column (e.g. <b>Engineer[50%]</b>).</div>'}</div>
    <p class="muted">Work resources are people and equipment (Max Units 100% = full time). Materials are charged per unit; cost resources take a fixed amount per assignment. Total project cost: <b>${V.cost}</b> · work ${V.work}.</p>`;
}

async function workloadPage(): Promise<string> {
  const w = await call<{ buckets: number[]; rows: { uid: number; name: string; load: number[]; cap: number[] }[] }>('workload', { weekly: wlWeekly });
  if (!w) return '';
  const head = `<div class="ptools"><h1>Workload</h1><div class="seg"><button data-wl="1" class="${wlWeekly ? 'on' : ''}">Weeks</button><button data-wl="0" class="${wlWeekly ? '' : 'on'}">Days</button></div>
    <span class="muted">Hours assigned per ${wlWeekly ? 'week' : 'day'}. Red cells are over capacity.</span></div>`;
  if (!w.rows.length) return head + '<div class="empty">Assign work resources to tasks to see their workload.</div>';
  const cells = (row: { load: number[]; cap: number[] }) => row.load.map((h, k) => {
    const over = h > row.cap[k] + 0.01, a = row.cap[k] ? Math.min(1, h / row.cap[k]) : h ? 1 : 0;
    return `<td class="wl${over ? ' over' : ''}" style="--a:${a.toFixed(2)}" title="${h.toFixed(1)} of ${row.cap[k].toFixed(1)} h">${h ? Math.round(h) : ''}</td>`;
  }).join('');
  return head + `<div class="panel wlwrap"><table class="tbl wltbl"><tr><th>Resource</th>${w.buckets.map(d => `<th>${fmtD(d).slice(0, 6)}</th>`).join('')}</tr>
    ${w.rows.map(r => `<tr><td class="nm">${esc(r.name)}</td>${cells(r)}</tr>`).join('')}</table></div>`;
}

function scurve(L: Row[]): string {
  const hasBase = L.some(r => r.bs !== null);
  const a = V.s, b = Math.max(V.f, ...L.map(r => r.bf ?? 0), a + 1), W = 600, H = 170, px = (d: number) => 30 + (d - a) / (b - a) * (W - 40), py = (p: number) => H - 20 - p / 100 * (H - 30);
  const curve = (base: boolean) => Array.from({ length: 61 }, (_, k) => { const d = a + (b - a) * k / 60; return `${px(d).toFixed(1)},${py(planned(L, d, base)).toFixed(1)}`; }).join(' ');
  const t = Math.min(Math.max(today, a), b), act = pctNow(L);
  return `<svg viewBox="0 0 ${W} ${H}" style="width:100%;height:auto">
    ${[0, 50, 100].map(p => `<line x1="30" x2="${W - 10}" y1="${py(p)}" y2="${py(p)}" stroke="#E3E9F2"/><text x="2" y="${py(p) + 4}" font-size="10" fill="#6B7686">${p}%</text>`).join('')}
    ${hasBase ? `<polyline points="${curve(true)}" fill="none" stroke="#AEB7C4" stroke-width="2" stroke-dasharray="5 3"/>` : ''}
    <polyline points="${curve(false)}" fill="none" stroke="#2F6FCF" stroke-width="2"/>
    <line x1="${px(t)}" x2="${px(t)}" y1="8" y2="${H - 20}" stroke="#F2B233" stroke-dasharray="4 3"/>
    <circle cx="${px(t)}" cy="${py(act)}" r="5" fill="#2E9D57"/>
    <text x="30" y="${H - 4}" font-size="10" fill="#6B7686">${V.start}</text><text x="${W - 10}" y="${H - 4}" font-size="10" fill="#6B7686" text-anchor="end">${fmtD(Math.ceil(b) - 1)}</text></svg>
    <div class="muted"><span style="color:#2F6FCF">━</span> current plan${hasBase ? ' · <span style="color:#8A94A3">╍</span> baseline' : ''} · <span style="color:#2E9D57">●</span> actual today (${act.toFixed(0)}%)</div>`;
}

function dashboardPage(): string {
  const L = leaves(), done = L.filter(r => r.pct === 100), prog = L.filter(r => r.pct > 0 && r.pct < 100), late = L.filter(r => r.late);
  const now = pctNow(L), plan = planned(L, today + 1), left = Math.max(0, Math.ceil(V.f) - 1 - today);
  const kpi = (v: string, l: string, c = '') => `<div class="kpi ${c}"><div class="v">${v}</div><div class="l">${l}</div></div>`;
  const minLvl = Math.min(...V.rows.filter(x => x.sum).map(x => x.lvl));
  const phases = V.rows.filter(r => r.sum && r.lvl === minLvl);
  const ms = L.filter(r => r.ms && r.s >= today).slice(0, 8);
  const over = V.resources.filter(r => r.over).length;
  return `<div class="ptools"><h1>${esc(V.name)}: dashboard</h1></div>
    <div class="kpis">${kpi(now.toFixed(0) + '%', `complete · planned ${plan.toFixed(0)}% by today`, now + 0.5 >= plan ? 'good' : 'bad')}
      ${kpi(V.finish, `finish · ${left} days to go${V.varFinish ? ` · ${V.varFinish} vs baseline` : ''}`, V.varFinish && !V.varFinish.startsWith('0') && !V.varFinish.startsWith('-') ? 'bad' : '')}
      ${kpi(String(L.length), 'tasks')}${kpi(String(done.length), 'completed', 'good')}${kpi(String(prog.length), 'in progress')}
      ${kpi(String(late.length), 'late', late.length ? 'bad' : 'good')}${kpi(String(L.filter(r => r.crit).length), 'critical tasks')}
      ${V.cost !== '0.00' ? kpi(V.cost, `cost · ${V.work}`) : ''}${V.resources.length ? kpi(String(over), 'overallocated resources', over ? 'bad' : 'good') : ''}</div>
    <div class="cols">
      <div class="panel"><h2 style="margin-top:0">Progress curve</h2>${scurve(L)}</div>
      <div class="panel"><h2 style="margin-top:0">Phases</h2>${phases.length ? `<table class="tbl"><tr><th>Phase</th><th>Finish</th><th>Done</th><th></th></tr>${phases.map(r => `<tr data-u="${r.uid}" class="link"><td>${esc(r.name)}</td><td>${r.finish}</td><td class="r">${r.pct}%</td><td>${meter(r.pct)}</td></tr>`).join('')}</table>` : '<div class="empty">Indent tasks under summary tasks to see phases.</div>'}</div>
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
  const starting = L.filter(r => !r.ms && inP(Math.floor(r.s))), active = L.filter(r => r.pct > 0 && r.pct < 100), late = L.filter(r => r.late);
  const ms = L.filter(r => r.ms && inP(Math.floor(r.s))), next = L.filter(r => inP(Math.floor(r.s), pe, ne));
  const slipped = L.filter(r => r.vfinish && !r.vfinish.startsWith('0') && !r.vfinish.startsWith('-'));
  const now = pctNow(L), plan = planned(L, pe), title = rep.unit === 'week' ? `Week of ${fmtD(ps)} to ${fmtD(pe - 1)}` : `${MON[ymd(ps)[1]]} ${ymd(ps)[0]}`;
  const sec = (h: string, L2: Row[]) => `<div class="panel"><h2 style="margin-top:0">${h} <span class="muted">(${L2.length})</span></h2>${tbl(L2)}</div>`;
  return `<div class="ptools"><div class="seg"><button data-rep="week" class="${rep.unit === 'week' ? 'on' : ''}">Weekly</button><button data-rep="month" class="${rep.unit === 'month' ? 'on' : ''}">Monthly</button></div>
      <div class="seg"><button data-rp="-1">‹ Previous</button><button data-rp="0">Current</button><button data-rp="1">Next ›</button></div><span class="grow"></span>
      <button class="btn" data-print="1">Print or save as PDF</button></div>
    <h1>${esc(V.name)}: progress report</h1><div class="muted">${title} · produced ${fmtD(today)}${V.status ? ` · status date ${V.status}` : ''}</div>
    <div class="kpis" style="margin-top:14px"><div class="kpi"><div class="v">${now.toFixed(0)}%</div><div class="l">complete now</div></div>
      <div class="kpi"><div class="v">${plan.toFixed(0)}%</div><div class="l">planned by end of period</div></div>
      <div class="kpi ${late.length ? 'bad' : 'good'}"><div class="v">${late.length}</div><div class="l">late tasks</div></div>
      <div class="kpi"><div class="v">${V.finish}</div><div class="l">forecast finish${V.blFinish ? ` · baseline ${V.blFinish}` : ''}</div></div></div>
    <div class="cols">${sec('Finished this period', finished)}${sec('Due to finish this period', due)}${sec('Starting this period', starting)}${sec('Milestones this period', ms)}
      ${sec('In progress', active)}${sec('Late', late)}${V.blFinish ? sec('Finishing later than baseline', slipped) : ''}${sec('Starting next period (look-ahead)', next)}</div>`;
}

$('#page').onclick = async e => {
  const el = e.target as HTMLElement, d = el.dataset;
  const u = (el.closest('[data-u]') as HTMLElement | null)?.dataset.u;
  if (d.cal) { calOff = d.cal === '0' ? 0 : calOff + +d.cal; renderPage(); }
  else if (d.rep) { rep = { unit: d.rep as 'week' | 'month', off: 0 }; renderPage(); }
  else if (d.rp) { rep.off = d.rp === '0' ? 0 : rep.off + +d.rp; renderPage(); }
  else if (d.wl) { wlWeekly = d.wl === '1'; renderPage(); }
  else if (d.print) print();
  else if (d.addres) { const v = await op({ ResInsert: { name: '' } }); if (v) setTimeout(() => $<HTMLInputElement>(`#page tr:last-child [data-rf=name]`)?.select(), 30); }
  else if (d.delres) {
    const r = V.resources.find(x => x.uid === +d.delres!);
    if (r && await dialog('Delete resource', `<p>Delete <b>${esc(r.name)}</b>? It is removed from every task it is assigned to.</p>`, ['Delete', 'Cancel']) === 0) op({ ResDelete: { uids: [r.uid] } });
  }
  else if (u) { setView('gantt'); collapsed.clear(); rows = visible(); layout(); const i = rows.findIndex(r => r.uid === +u); if (i >= 0) { select(i, 1); scrollToTask(); } }
};
$('#page').onchange = e => {
  const el = e.target as HTMLInputElement;
  if (el.dataset.rf) op({ ResSet: { uid: +el.dataset.ru!, field: el.dataset.rf, value: el.value } });
};
$('#page').onkeydown = e => { if (e.key === 'Enter' && (e.target as HTMLElement).tagName === 'INPUT') (e.target as HTMLInputElement).blur(); };
