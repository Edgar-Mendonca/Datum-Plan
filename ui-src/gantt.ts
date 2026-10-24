// The Gantt chart and the timeline strip: canvas drawing, zoom, and dragging bars to move, resize,
// change progress or link.
const cbody = $('#cbody'), cspace = $('#cspace');
const scaleCv = $<HTMLCanvasElement>('#scale'), barsCv = $<HTMLCanvasElement>('#bars'), tlCv = $<HTMLCanvasElement>('#tl');

type Unit = 'day' | 'week' | 'month' | 'quarter' | 'year';
function floorU(d: number, u: Unit): number {
  const [y, m] = ymd(d);
  return u === 'day' ? Math.floor(d) : u === 'week' ? Math.floor(d) - dowMon(d) : u === 'month' ? Date.UTC(y, m, 1) / DAY : u === 'quarter' ? Date.UTC(y, m - m % 3, 1) / DAY : Date.UTC(y, 0, 1) / DAY;
}
function nextU(d: number, u: Unit): number {
  const [y, m] = ymd(d);
  return u === 'day' ? d + 1 : u === 'week' ? d + 7 : u === 'month' ? Date.UTC(y, m + 1, 1) / DAY : u === 'quarter' ? Date.UTC(y, m + 3, 1) / DAY : Date.UTC(y + 1, 0, 1) / DAY;
}
function label(d: number, u: Unit, top: boolean): string {
  const [y, m, day] = ymd(d);
  if (u === 'day') return pd >= 26 ? `${'MTWTFSS'[dowMon(d)]} ${day}` : 'MTWTFSS'[dowMon(d)];
  if (u === 'week') return top ? `${day} ${MON[m]} '${String(y).slice(2)}` : `${day} ${MON[m]}`;
  if (u === 'month') return top ? `${MON[m]} ${y}` : MON[m];
  if (u === 'quarter') return `Q${m / 3 + 1}${top ? ' ' + y : ''}`;
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
  requestAnimationFrame(() => { drawPending = false; if (!$('#ganttView').hidden && view === 'gantt') { drawScale(); drawBars(); drawTimeline(); } });
}

function drawScale() {
  const w = cbody.clientWidth, x0 = cbody.scrollLeft;
  const ctx = fitCanvas(scaleCv, scaleCv.parentElement!.clientWidth, 40);
  ctx.fillStyle = '#F7F9FC'; ctx.fillRect(0, 0, w + 40, 40);
  const [top, bot]: [Unit, Unit] = pd >= 16 ? ['week', 'day'] : pd >= 4 ? ['month', 'week'] : pd >= 1.2 ? ['quarter', 'month'] : ['year', 'quarter'];
  const first = d0 + Math.floor(x0 / pd) - 1, last = d0 + Math.ceil((x0 + w) / pd) + 1;
  ctx.font = '11px "Segoe UI", sans-serif'; ctx.textBaseline = 'middle';
  for (const [u, y, isTop] of [[top, 0, true], [bot, 20, false]] as [Unit, number, boolean][]) {
    for (let d = floorU(first, u); d < last; d = nextU(d, u)) {
      const x = (d - d0) * pd - x0, x2 = (nextU(d, u) - d0) * pd - x0;
      ctx.fillStyle = '#DCE2EA'; ctx.fillRect(Math.round(x), y + 2, 1, 18);
      ctx.fillStyle = u === 'day' && offDay(d) ? '#97A1AF' : '#3B4657';
      let t = label(d, u, isTop), tw = ctx.measureText(t).width;
      if (tw >= x2 - x - 4 && u === 'week') { t = String(ymd(d)[2]); tw = ctx.measureText(t).width; }
      if (tw < x2 - x - 4) ctx.fillText(t, u === 'day' ? x + (x2 - x - tw) / 2 : Math.max(x + 4, Math.min(4, x2 - tw - 4)), y + 11);
    }
  }
  ctx.fillStyle = '#DCE2EA'; ctx.fillRect(0, 20, w + 40, 1); ctx.fillRect(0, 39, w + 40, 1);
}

const X = (d: number) => (d - d0) * pd - cbody.scrollLeft;
function drawBars() {
  const w = cbody.clientWidth, h = cbody.clientHeight, top = cbody.scrollTop;
  const ctx = fitCanvas(barsCv, w, h);
  if (pd >= 8) {
    ctx.fillStyle = '#F4F6FA';
    for (let d = d0 + Math.floor(cbody.scrollLeft / pd); X(d) < w; d++) if (offDay(d)) ctx.fillRect(X(d), 0, pd, h);
  }
  const a = Math.floor(top / ROW), b = Math.min(rows.length, a + Math.ceil(h / ROW) + 1);
  for (let i = a; i < b; i++) if (sel.has(rows[i].uid)) { ctx.fillStyle = 'rgba(31,78,154,.08)'; ctx.fillRect(0, i * ROW - top, w, ROW); }
  const line = (d: number, color: string, dash: number[]) => { ctx.strokeStyle = color; ctx.setLineDash(dash); ctx.beginPath(); ctx.moveTo(Math.round(X(d)) + .5, 0); ctx.lineTo(Math.round(X(d)) + .5, h); ctx.stroke(); ctx.setLineDash([]); };
  line(today, '#F2B233', [4, 3]);
  if (V.statusDay) line(V.statusDay, '#2E9D57', [6, 3]);

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
    const crit = showCrit && r.crit, by = showBase && r.bs !== null ? y + 4 : y + 6, bh = showBase && r.bs !== null ? 10 : 12;
    if (showBase && r.bs !== null && r.bf !== null) {
      ctx.fillStyle = '#AEB7C4';
      r.ms || r.bf === r.bs ? (ctx.beginPath(), ctx.moveTo(X(r.bs), y + 15), ctx.lineTo(X(r.bs) + 4, y + 19), ctx.lineTo(X(r.bs), y + 23), ctx.lineTo(X(r.bs) - 4, y + 19), ctx.fill())
        : ctx.fillRect(X(r.bs), y + 16, Math.max(X(r.bf) - X(r.bs), 2), 4);
    }
    let lx = x2 + 6;
    ctx.globalAlpha = r.off ? 0.35 : 1;
    if (r.ms) {
      ctx.fillStyle = crit ? '#D64545' : '#14325E';
      ctx.beginPath(); ctx.moveTo(x1, by - 1); ctx.lineTo(x1 + 7, by + 6); ctx.lineTo(x1, by + 13); ctx.lineTo(x1 - 7, by + 6); ctx.fill();
      lx = x1 + 11;
    } else if (r.sum) {
      ctx.fillStyle = '#14325E';
      ctx.fillRect(x1, y + 7, x2 - x1, 5);
      ctx.beginPath(); ctx.moveTo(x1, y + 7); ctx.lineTo(x1 + 5, y + 7); ctx.lineTo(x1, y + 16); ctx.fill();
      ctx.beginPath(); ctx.moveTo(x2, y + 7); ctx.lineTo(x2 - 5, y + 7); ctx.lineTo(x2, y + 16); ctx.fill();
      if (r.pct > 0) { ctx.fillStyle = '#2E9D57'; ctx.fillRect(x1, y + 12, (x2 - x1) * r.pct / 100, 2); }
    } else {
      const wd = Math.max(x2 - x1, 2);
      ctx.fillStyle = r.manual ? '#BFE6D3' : crit ? '#F2B0B0' : '#A9CAF6';
      ctx.fillRect(x1, by, wd, bh);
      ctx.fillStyle = r.manual ? '#2E9D6B' : crit ? '#C73B3B' : '#2F6FCF';
      ctx.fillRect(x1, by + bh / 2 - 2, wd * r.pct / 100, 4);
      ctx.strokeStyle = ctx.fillStyle;
      if (r.manual) { ctx.fillRect(x1, by, 3, bh); ctx.fillRect(x1 + wd - 3, by, 3, bh); }
      else ctx.strokeRect(x1 + .5, by + .5, wd - 1, bh - 1);
    }
    if (r.dl !== null) {
      const dx = X(r.dl);
      ctx.fillStyle = r.dl < r.f ? '#D64545' : '#2E9D57';
      ctx.beginPath(); ctx.moveTo(dx - 4, y + 2); ctx.lineTo(dx + 4, y + 2); ctx.lineTo(dx, y + 8); ctx.fill(); ctx.fillRect(dx - .5, y + 6, 1, 14);
    }
    ctx.fillStyle = '#3B4657';
    ctx.fillText(r.ms ? `${r.name}  ${r.start}` : r.name + (r.res ? `  [${r.res}]` : ''), lx, y + 12);
    ctx.globalAlpha = 1;
  }
  if (drag) {
    const r = drag.r, y = drag.i * ROW - top;
    ctx.setLineDash([4, 3]); ctx.strokeStyle = '#2F6FCF'; ctx.lineWidth = 1.5;
    if (drag.mode === 'link') { ctx.beginPath(); ctx.moveTo(X(r.f), y + 12); ctx.lineTo(drag.mx - cbody.scrollLeft, drag.my - top); ctx.stroke(); }
    else if (drag.mode === 'pct') { ctx.fillStyle = 'rgba(46,157,87,.5)'; ctx.fillRect(X(r.s), y + 10, (X(r.f) - X(r.s)) * drag.pct / 100, 4); }
    else { const g1 = X(r.s + (drag.mode === 'move' ? drag.dd : 0)), g2 = X(r.f + drag.dd); ctx.strokeRect(g1 + .5, y + 3.5, Math.max(g2 - g1, 6), 17); }
    ctx.setLineDash([]); ctx.lineWidth = 1;
  }
}

// ---------- timeline strip (MS Project style): the whole project, with tasks you choose ----------
function drawTimeline() {
  $('#tlWrap').hidden = !showTl;
  if (!showTl) return;
  const w = tlCv.parentElement!.clientWidth, h = 62, ctx = fitCanvas(tlCv, w, h);
  ctx.fillStyle = '#FFFFFF'; ctx.fillRect(0, 0, w, h);
  ctx.font = '11px "Segoe UI", sans-serif'; ctx.textBaseline = 'middle';
  if (!V.rows.length) { ctx.fillStyle = '#6B7686'; ctx.fillText('Timeline: add tasks to see the project at a glance.', 12, h / 2); return; }
  const L = 70, R = w - 70, a = V.s, b = Math.max(V.f, a + 1), px = (d: number) => L + (d - a) / (b - a) * (R - L);
  ctx.fillStyle = '#6B7686';
  ctx.fillText('Start', 10, 18); ctx.fillText(V.start, 10, 34);
  ctx.fillText('Finish', R + 10, 18); ctx.fillText(V.finish, R + 10, 34);
  ctx.fillStyle = '#EEF4FD'; ctx.fillRect(L, 22, R - L, 18);
  // month ticks
  ctx.fillStyle = '#9AA4B2';
  const step: Unit = (b - a) / (R - L) > 3 ? 'quarter' : 'month';
  for (let d = nextU(floorU(a, step), step); d < b; d = nextU(d, step)) { ctx.fillRect(px(d), 22, 1, 18); if (px(nextU(d, step)) - px(d) > 40) ctx.fillText(label(d, step, true), px(d) + 3, 50); }
  const items = V.rows.filter(r => r.tl && !r.off);
  const lanes: number[] = [];
  for (const r of items) {
    const x1 = px(r.s), x2 = Math.max(px(r.f), x1 + 2);
    if (r.ms) {
      ctx.fillStyle = '#F2B233'; ctx.beginPath(); ctx.moveTo(x1, 23); ctx.lineTo(x1 + 6, 31); ctx.lineTo(x1, 39); ctx.lineTo(x1 - 6, 31); ctx.fill();
      ctx.fillStyle = '#14325E'; ctx.fillText(r.name, x1 + 8, 12);
      continue;
    }
    let lane = lanes.findIndex(e => e <= x1);
    if (lane < 0) lane = lanes.length;
    lanes[lane] = x2 + 4;
    const y = 23 + (lane % 2) * 8;
    ctx.fillStyle = r.sum ? '#2F6FCF' : '#6E9FE6'; ctx.fillRect(x1, y, x2 - x1, 8);
    ctx.fillStyle = '#14325E';
    if (ctx.measureText(r.name).width < x2 - x1 + 30) ctx.fillText(r.name, x1, lane % 2 ? 57 : 12);
  }
  if (!items.length) { ctx.fillStyle = '#8A94A3'; ctx.fillText('Right-click a task and choose Add to Timeline', L + 8, 31); }
  ctx.strokeStyle = '#F2B233'; ctx.beginPath(); ctx.moveTo(px(today), 18); ctx.lineTo(px(today), 44); ctx.stroke();
  // the part of the plan the Gantt chart shows
  const v1 = d0 + cbody.scrollLeft / pd, v2 = v1 + cbody.clientWidth / pd;
  ctx.strokeStyle = '#2F6FCF'; ctx.lineWidth = 1.5; ctx.strokeRect(Math.max(L, px(v1)), 20, Math.max(4, Math.min(R, px(v2)) - Math.max(L, px(v1))), 22); ctx.lineWidth = 1;
}
tlCv.onclick = e => {
  if (!V.rows.length) return;
  const w = tlCv.clientWidth, L = 70, R = w - 70, x = e.offsetX;
  const d = V.s + (x - L) / (R - L) * (Math.max(V.f, V.s + 1) - V.s);
  cbody.scrollLeft = (d - d0) * pd - cbody.clientWidth / 2;
};

// ---------- zoom ----------
function setZoom(v: number, keepDay?: number) {
  const day = keepDay ?? d0 + (cbody.scrollLeft + cbody.clientWidth / 2) / pd;
  pd = Math.max(0.3, Math.min(60, v));
  $<HTMLInputElement>('#zoom').value = String(Math.round(Math.log(pd / 0.3) / Math.log(200) * 100));
  layout();
  cbody.scrollLeft = (day - d0) * pd - cbody.clientWidth / 2;
}
function fit() {
  if (!V.rows.length) return;
  setZoom((cbody.clientWidth - 160) / Math.max(7, V.f - V.s + 4));
  cbody.scrollLeft = (V.s - d0 - 2) * pd;
}
function scrollToTask() {
  const r = curRow();
  if (r) cbody.scrollLeft = Math.max(0, (r.s - d0) * pd - 60);
}

// ---------- mouse: select, drag to move, drag the end to resize, drag the progress, drag down to link ----------
interface Drag { r: Row; i: number; mode: 'move' | 'size' | 'link' | 'pct'; x: number; y: number; dd: number; mx: number; my: number; pct: number }
let drag: Drag | null = null;
const tip = $('#tip');
const pos = (e: MouseEvent) => { const b = cbody.getBoundingClientRect(); return [e.clientX - b.left + cbody.scrollLeft, e.clientY - b.top + cbody.scrollTop]; };
function hit(e: MouseEvent) {
  const [x, y] = pos(e), i = Math.floor(y / ROW), r = rows[i];
  let part = '';
  if (r && !r.sum) {
    const x1 = (r.s - d0) * pd, x2 = (r.f - d0) * pd, xp = x1 + (x2 - x1) * r.pct / 100, low = y - i * ROW > 12;
    part = r.ms ? (Math.abs(x - x1) <= 8 ? 'move' : '')
      : x >= x2 - 5 && x <= x2 + 3 ? 'size'
      : low && x >= x1 - 2 && Math.abs(x - xp) <= 4 ? 'pct'
      : x >= x1 - 2 && x < x2 - 5 ? 'move' : '';
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
  gbody.focus();
  if (!h.r || !h.part || e.shiftKey || e.ctrlKey) return;
  drag = { r: h.r, i: h.i, mode: h.part as Drag['mode'], x: h.x, y: h.y, dd: 0, mx: h.x, my: h.y, pct: h.r.pct };
  e.preventDefault();
};
cbody.oncontextmenu = e => { e.preventDefault(); const h = hit(e); if (rows[h.i] && !sel.has(rows[h.i].uid)) select(h.i, cur.c); rowMenu(e.clientX, e.clientY); };
cbody.ondblclick = () => openDetails('general');
cbody.onwheel = e => { if (e.ctrlKey) { e.preventDefault(); setZoom(pd * (e.deltaY < 0 ? 1.2 : 1 / 1.2), d0 + (cbody.scrollLeft + e.offsetX) / pd); } };
document.addEventListener('mousemove', e => {
  if (!drag) {
    if (!cbody.contains(e.target as Node)) return void (tip.hidden = true);
    const h = hit(e), r = h.r;
    cbody.style.cursor = h.part === 'size' ? 'ew-resize' : h.part === 'pct' ? 'col-resize' : h.part ? 'grab' : 'default';
    if (r && (h.part || r.sum)) showTip(e, `<b>${esc(r.name)}</b><br>${r.start} → ${r.finish} · ${r.dur}<br>${r.pct}% complete · total float ${r.tf}${r.res ? '<br>' + esc(r.res) : ''}${r.bstart ? `<br>Baseline ${r.bstart} → ${r.bfinish}` : ''}`
      + (r.sum ? '' : '<br><span class="muted">Drag to move · drag the end to resize · drag the progress line · drag down to link</span>'));
    else tip.hidden = true;
    return;
  }
  const [x, y] = pos(e), d = drag;
  d.mx = x; d.my = y; d.dd = Math.round((x - d.x) / pd);
  if (d.mode === 'move' && Math.abs(y - d.y) > ROW) d.mode = 'link';
  if (d.mode === 'pct') { const x1 = (d.r.s - d0) * pd, x2 = (d.r.f - d0) * pd; d.pct = Math.max(0, Math.min(100, Math.round((x - x1) / Math.max(1, x2 - x1) * 20) * 5)); }
  const t = rows[Math.floor(y / ROW)];
  showTip(e, d.mode === 'link' ? `Link <b>${d.r.id}</b> → <b>${t && t !== d.r ? t.id + ' ' + esc(t.name) : '…'}</b>`
    : d.mode === 'pct' ? `Progress <b>${d.pct}%</b>`
    : d.mode === 'move' ? `Start <b>${fmtD(d.r.s + d.dd)}</b>` : `Finish <b>${fmtD(Math.max(Math.floor(d.r.s), endDay(d.r) + d.dd))}</b>`);
  draw();
});
document.addEventListener('mouseup', async () => {
  const d = drag;
  if (!d) return;
  drag = null; tip.hidden = true; draw();
  if (d.mode === 'link') {
    const t = rows[Math.floor(d.my / ROW)];
    if (t && t !== d.r) await op({ LinkSet: { pred: d.r.uid, succ: t.uid, kind: 'FS', lag: '' } });
  } else if (d.mode === 'pct') { if (d.pct !== d.r.pct) await set(d.r.uid, 'pct', String(d.pct)); }
  else if (d.dd && d.mode === 'move') await set(d.r.uid, 'start', iso(d.r.s + d.dd));
  else if (d.dd) await set(d.r.uid, 'finish', iso(Math.max(Math.floor(d.r.s), endDay(d.r) + d.dd)));
});
