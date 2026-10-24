// The details pane under the Gantt (Primavera style): General, Relationships, Resources and Notes.
let detTab = 'general';

function openDetails(t = detTab) {
  detTab = t;
  showDet = true;
  ribbon();
  renderDetails();
}

function sel1(id: string, opts: string[], v: string, dis = false) {
  return `<select id="${id}"${dis ? ' disabled' : ''}>${opts.map(o => `<option${o === v ? ' selected' : ''}>${esc(o)}</option>`).join('')}</select>`;
}

async function renderDetails() {
  const pane = $('#details');
  pane.hidden = $('#dsplit').hidden = !showDet;
  if (!showDet) return;
  const r = curRow();
  const tabs = [['general', 'General'], ['rel', 'Relationships'], ['res', 'Resources'], ['notes', 'Notes']];
  const head = `<nav class="dtabs">${tabs.map(([k, t]) => `<button data-dt="${k}" class="${detTab === k ? 'on' : ''}">${t}</button>`).join('')}<span class="grow"></span>
    <span class="muted">${r ? `${r.id} · ${esc(r.name)}` : ''}</span><button data-dt="close" title="Close the details pane">✕</button></nav>`;
  if (!r) { pane.innerHTML = head + '<div class="dbody empty">Select a task to see its details.</div>'; return; }
  const ro = r.sum, inp = (f: string, v: string, dis = false, ph = '') => `<input data-f="${f}" value="${esc(v)}"${dis ? ' disabled' : ''}${ph ? ` placeholder="${ph}"` : ''}>`;
  let body = '';
  if (detTab === 'general') {
    body = `<div class="dgrid">
      <label>Name</label>${inp('name', r.name)}<label>Activity ID</label>${inp('code', r.code)}
      <label>Duration</label>${inp('dur', r.dur, ro)}<label>% Complete</label>${inp('pct', String(r.pct), ro)}
      <label>Start</label>${inp('start', r.start, ro)}<label>Finish</label>${inp('finish', r.finish, ro)}
      <label>Mode</label>${sel1('d-mode', ['Auto', 'Manual'], r.mode, ro)}<label>Calendar</label>${sel1('d-cal', ['(project calendar)', ...V.calendars.map(c => c.name)], r.cal || '(project calendar)', ro)}
      <label>Constraint</label>${sel1('d-ctype', CONSTRAINTS, r.ctype, ro)}<label>Constraint date</label>${inp('cdate', r.cdate, ro || r.ctype.startsWith('As '))}
      <label>Deadline</label>${inp('deadline', r.deadline, false, 'none')}<label>Floats</label><span class="ro">total ${r.tf} · free ${r.ff}</span>
      <label>Actual start</label>${inp('astart', r.astart, ro, 'not started')}<label>Actual finish</label>${inp('afinish', r.afinish, ro, 'not finished')}
      <label>Remaining</label>${inp('rem', r.rem, ro)}<label>Baseline</label><span class="ro">${r.bstart ? `${r.bstart} → ${r.bfinish} · variance ${r.vfinish}` : 'none saved'}</span>
      <label>Work · Cost</label><span class="ro">${r.work} · ${r.cost}</span><label>Options</label><span>
        <label class="chk"><input type="checkbox" data-c="active"${r.off ? '' : ' checked'}> Active</label>
        <label class="chk"><input type="checkbox" data-c="timeline"${r.tl ? ' checked' : ''}> Show on timeline</label></span>
    </div>`;
  } else if (detTab === 'notes') {
    body = `<textarea data-f="notes" class="dnotes" placeholder="Notes for this task">${esc(r.notes)}</textarea>`;
  } else {
    const d = await call<Detail>('detail', { uid: r.uid });
    if (!d || curRow()?.uid !== r.uid) return;
    const kinds = (v: string, attr: string) => `<select ${attr}>${['FS', 'SS', 'FF', 'SF'].map(k => `<option${k === v ? ' selected' : ''}>${k}</option>`).join('')}</select>`;
    if (detTab === 'rel') {
      const tbl = (title: string, list: Rel[], side: 'p' | 's') => `<div><h4>${title}</h4><table class="tbl"><tr><th>ID</th><th>Task</th><th>Type</th><th>Lag</th><th></th></tr>
        ${list.map(l => `<tr data-u="${l.uid}" data-side="${side}"><td>${l.id}</td><td>${esc(l.name)}</td><td>${kinds(l.kind, 'data-k')}</td><td><input data-lag value="${esc(l.lag)}" placeholder="0" size="5"></td><td><button data-x title="Remove">✕</button></td></tr>`).join('')}
        <tr data-side="${side}" class="add"><td><input data-id placeholder="ID" size="3"></td><td class="muted">Add a ${side === 'p' ? 'predecessor' : 'successor'}</td><td>${kinds('FS', 'data-k')}</td><td><input data-lag placeholder="0" size="5"></td><td><button data-add class="btn">Add</button></td></tr></table></div>`;
      body = `<div class="dcols">${tbl('Predecessors', d.preds, 'p')}${tbl('Successors', d.succs, 's')}</div>`;
    } else {
      body = ro ? '<div class="empty">Assign resources to the tasks inside this summary.</div>' : `<table class="tbl"><tr><th>Resource</th><th>Type</th><th>Units / quantity</th><th class="r">Work</th><th class="r">Cost</th><th></th></tr>
        ${d.assigns.map(a => `<tr data-r="${a.res}"><td>${esc(a.name)}</td><td>${a.kind}</td><td><input data-units value="${esc(a.units)}" size="7"></td><td class="r">${a.work}</td><td class="r">${a.cost}</td><td><button data-x title="Remove">✕</button></td></tr>`).join('')}
        <tr class="add"><td><input data-rname list="dl-res" placeholder="Resource name"><datalist id="dl-res">${V.resources.map(x => `<option value="${esc(x.name)}">`).join('')}</datalist></td><td></td><td><input data-units placeholder="100%" size="7"></td><td></td><td></td><td><button data-add class="btn">Assign</button></td></tr></table>
        <p class="muted">Work resources take a percentage (50% = half time). Materials take a quantity; cost resources take an amount. New names are added to the resource sheet.</p>`;
    }
  }
  pane.innerHTML = head + `<div class="dbody">${body}</div>`;
}

function assignString(rs: { name: string; units: string }[]) {
  return rs.filter(a => a.name.trim()).map(a => a.units.trim() ? `${a.name.trim()}[${a.units.trim()}]` : a.name.trim()).join(', ');
}

$('#details').onclick = async e => {
  const el = e.target as HTMLElement, t = el.dataset.dt;
  if (t === 'close') { showDet = false; ribbon(); renderDetails(); return; }
  if (t) { detTab = t; renderDetails(); return; }
  const r = curRow(), tr = el.closest('tr') as HTMLElement | null;
  if (!r || !tr) return;
  if (detTab === 'rel' && (el.dataset.x !== undefined || el.dataset.add !== undefined)) {
    const side = tr.dataset.side!, other = el.dataset.add !== undefined ? V.rows[+(tr.querySelector<HTMLInputElement>('[data-id]')!.value) - 1]?.uid : +tr.dataset.u!;
    if (other === undefined) return toast('Type the ID of an existing task.');
    const kind = el.dataset.x !== undefined ? null : tr.querySelector<HTMLSelectElement>('[data-k]')!.value;
    const lag = tr.querySelector<HTMLInputElement>('[data-lag]')?.value ?? '';
    await op({ LinkSet: side === 'p' ? { pred: other, succ: r.uid, kind, lag } : { pred: r.uid, succ: other, kind, lag } });
  } else if (detTab === 'res' && (el.dataset.x !== undefined || el.dataset.add !== undefined)) {
    const d = await call<Detail>('detail', { uid: r.uid });
    if (!d) return;
    let list = d.assigns.map(a => ({ name: a.name, units: a.units, res: a.res }));
    if (el.dataset.x !== undefined) list = list.filter(a => a.res !== +tr.dataset.r!);
    else list.push({ name: tr.querySelector<HTMLInputElement>('[data-rname]')!.value, units: tr.querySelector<HTMLInputElement>('[data-units]')!.value, res: 0 });
    await set(r.uid, 'res', assignString(list));
  }
};
$('#details').onchange = async e => {
  const el = e.target as HTMLInputElement, r = curRow();
  if (!r) return;
  const f = el.dataset.f, c = el.dataset.c, tr = el.closest('tr') as HTMLElement | null;
  if (f) return set(r.uid, f, el.value);
  if (c) return set(r.uid, c, el.checked ? 'Yes' : 'No');
  if (el.id === 'd-mode') return set(r.uid, 'mode', el.value);
  if (el.id === 'd-cal') return set(r.uid, 'cal', el.value.startsWith('(') ? '' : el.value);
  if (el.id === 'd-ctype') return set(r.uid, 'ctype', el.value);
  if (detTab === 'rel' && tr?.dataset.u) {
    const side = tr.dataset.side, other = +tr.dataset.u, kind = tr.querySelector<HTMLSelectElement>('[data-k]')!.value, lag = tr.querySelector<HTMLInputElement>('[data-lag]')!.value;
    return op({ LinkSet: side === 'p' ? { pred: other, succ: r.uid, kind, lag } : { pred: r.uid, succ: other, kind, lag } });
  }
  if (detTab === 'res' && tr?.dataset.r && el.dataset.units !== undefined) {
    const d = await call<Detail>('detail', { uid: r.uid });
    if (d) set(r.uid, 'res', assignString(d.assigns.map(a => ({ name: a.name, units: a.res === +tr.dataset.r! ? el.value : a.units }))));
  }
};
$('#details').onkeydown = e => { e.stopPropagation(); if (e.key === 'Enter' && (e.target as HTMLElement).tagName === 'INPUT') (e.target as HTMLInputElement).blur(); };
$('#dsplit').onmousedown = e => {
  const p = $('#details'), y = e.clientY, h0 = p.offsetHeight;
  const mv = (m: MouseEvent) => { p.style.height = Math.max(120, Math.min(innerHeight * 0.7, h0 - (m.clientY - y))) + 'px'; renderRows(); draw(); };
  document.addEventListener('mousemove', mv);
  document.addEventListener('mouseup', () => document.removeEventListener('mousemove', mv), { once: true });
};
