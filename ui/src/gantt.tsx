// The Gantt chart and the timeline strip: canvas drawing, timescale, zoom, and dragging bars
// to move, resize, change progress or link.
import { createSignal, createEffect, onMount, onCleanup, Show } from "solid-js";
import { Row, fmtDay, iso, dateDay } from "./api";
import { grid, out, sel, setSel, cur, setCur, zoom, setZoom, set, act, openMenu, view, timeline, tick, baseline, call, query, themeTick } from "./store";
import { RH } from "./grid";
import { C } from "./cmds";

export const [showCrit, setShowCrit] = createSignal(true);
export const [showBase, setShowBase] = createSignal(false);
export const [showSlack, setShowSlack] = createSignal(false);
export const [showLate, setShowLate] = createSignal(false);
export const [showLabels, setShowLabels] = createSignal(true);

export type Unit = "day" | "week" | "month" | "quarter" | "year";
const MON = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
export const ymd = (d: number) => { const x = new Date(Date.UTC(1970, 0, 1) + Math.floor(d) * 864e5); return [x.getUTCFullYear(), x.getUTCMonth(), x.getUTCDate()]; };
const dow = (d: number) => (((Math.floor(d) + 3) % 7) + 7) % 7; // 0 = Monday
export const floorU = (d: number, u: Unit) => {
  const [y, m] = ymd(d);
  return u === "day" ? Math.floor(d) : u === "week" ? Math.floor(d) - dow(d) : u === "month" ? Date.UTC(y, m, 1) / 864e5 : u === "quarter" ? Date.UTC(y, m - (m % 3), 1) / 864e5 : Date.UTC(y, 0, 1) / 864e5;
};
export const nextU = (d: number, u: Unit) => {
  const [y, m] = ymd(d);
  return u === "day" ? d + 1 : u === "week" ? d + 7 : u === "month" ? Date.UTC(y, m + 1, 1) / 864e5 : u === "quarter" ? Date.UTC(y, m + 3, 1) / 864e5 : Date.UTC(y + 1, 0, 1) / 864e5;
};
export const label = (d: number, u: Unit, top: boolean, pd: number) => {
  const [y, m, day] = ymd(d);
  if (u === "day") return pd >= 26 ? `${"MTWTFSS"[dow(d)]} ${day}` : "MTWTFSS"[dow(d)];
  if (u === "week") return top ? `${day} ${MON[m]} '${String(y).slice(2)}` : `${day} ${MON[m]}`;
  if (u === "month") return top ? `${MON[m]} ${y}` : MON[m];
  if (u === "quarter") return `Q${m / 3 + 1}${top ? " " + y : ""}`;
  return String(y);
};
const today = () => dateDay(new Date());
const fit = (cv: HTMLCanvasElement, w: number, h: number) => {
  const r = devicePixelRatio || 1;
  if (cv.width !== Math.round(w * r) || cv.height !== Math.round(h * r)) { cv.width = Math.round(w * r); cv.height = Math.round(h * r); cv.style.width = `${w}px`; cv.style.height = `${h}px`; }
  const ctx = cv.getContext("2d")!;
  ctx.setTransform(r, 0, 0, r, 0, 0);
  ctx.clearRect(0, 0, w, h);
  return ctx;
};
/** Gantt Chart Style gallery: task fill and edge, critical fill and edge, summary/milestone. */
export const STYLES: { name: string; fill: string; edge: string; cfill: string; crit: string; sum: string }[] = [
  { name: "Datum blue", fill: "#A9CAF6", edge: "#2F6FCF", cfill: "#F2B0B0", crit: "#C73B3B", sum: "#0C1A2B" },
  { name: "Ocean", fill: "#9FD3E8", edge: "#0072B2", cfill: "#F5C2A0", crit: "#D55E00", sum: "#123A54" },
  { name: "Forest", fill: "#B5DFC6", edge: "#1F7A4D", cfill: "#F2B0B0", crit: "#C0392B", sum: "#123B27" },
  { name: "Amber", fill: "#F9DDA0", edge: "#B7791F", cfill: "#F2B0B0", crit: "#C0392B", sum: "#4A3410" },
  { name: "Violet", fill: "#D7C6F0", edge: "#6C4AB6", cfill: "#F2B0B0", crit: "#C0392B", sum: "#2E1F55" },
  { name: "Graphite", fill: "#D3D7DB", edge: "#5B6878", cfill: "#F3B9B4", crit: "#C0392B", sum: "#1F2933" },
  { name: "Teal", fill: "#A6E3DC", edge: "#0E8C7F", cfill: "#F7C1C8", crit: "#C2185B", sum: "#0B3B36" },
  { name: "Solid blue", fill: "#2F6FCF", edge: "#1A4E9C", cfill: "#D64545", crit: "#9B2C2C", sum: "#0C1A2B" },
  { name: "Solid green", fill: "#2E9D57", edge: "#1E6B3C", cfill: "#D64545", crit: "#9B2C2C", sum: "#123B27" },
  { name: "Monochrome", fill: "#FFFFFF", edge: "#1A1A1A", cfill: "#E0E0E0", crit: "#1A1A1A", sum: "#1A1A1A" },
];
export const gstyle = () => ({ ...STYLES[0], grid: false, ...(out()?.project.ui?.gantt ?? {}) });

/** Canvas colours from the theme and the chosen Gantt Chart Style. */
let K: Record<string, string> = {};
const pal = () => {
  const v = (n: string) => getComputedStyle(document.documentElement).getPropertyValue(n).trim();
  const dark = document.documentElement.dataset.theme === "dark";
  K = { ink: dark ? "#9CC2F7" : "#0C1A2B", line: v("--line"), text: v("--text2"), bg: v("--bg2"), panel: v("--panel"), off: dark ? "#141E2A" : "#F2F4F2", pale: v("--pale"), muted: v("--muted"),
    base: dark ? "#5B6878" : "#AEB7C4", link: dark ? "#6B7F99" : "#7A8BA6", fill: dark ? "#24476E" : "#A9CAF6", blue: v("--blue"), cfill: dark ? "#5A2522" : "#F2B0B0", crit: v("--crit") };
  const g = gstyle();
  if (!dark || g.name !== STYLES[0].name) Object.assign(K, { fill: g.fill, blue: g.edge, cfill: g.cfill, crit: g.crit, ...(dark ? {} : { ink: g.sum }) });
  K.grid = g.grid ? (dark ? "#1E2A38" : "#E8EBEF") : "";
};
export const off = (d: number) => { const s = out()?.summary; if (!s) return dow(d) >= 5; return !s.week[dow(d)] || s.off_days.includes(Math.floor(d)); };

export const Gantt = () => {
  let body!: HTMLDivElement, scaleCv!: HTMLCanvasElement, barsCv!: HTMLCanvasElement;
  const [size, setSize] = createSignal({ w: 800, h: 600 });
  const [tip, setTip] = createSignal<{ x: number; y: number; html: string } | null>(null);
  const d0 = () => Math.floor((out()?.summary.s ?? today()) - 30);
  const d1 = () => Math.ceil((out()?.summary.f ?? today()) + 120);
  const X = (d: number) => (d - d0()) * zoom() - body.scrollLeft;
  type Drag = { r: Row; i: number; mode: "move" | "size" | "link" | "pct"; x: number; y: number; dd: number; mx: number; my: number; pct: number };
  let drag: Drag | null = null;
  let pending = false;
  const draw = () => { if (pending) return; pending = true; requestAnimationFrame(() => { pending = false; if (body) { pal(); drawScale(); drawBars(); } }); };

  const drawScale = () => {
    const w = size().w, x0 = body.scrollLeft, pd = zoom();
    const ctx = fit(scaleCv, w, 40);
    ctx.fillStyle = K.panel; ctx.fillRect(0, 0, w, 40);
    const [top, bot]: [Unit, Unit] = pd >= 16 ? ["week", "day"] : pd >= 4 ? ["month", "week"] : pd >= 1.2 ? ["quarter", "month"] : ["year", "quarter"];
    const first = d0() + Math.floor(x0 / pd) - 1, last = d0() + Math.ceil((x0 + w) / pd) + 1;
    ctx.font = "11px 'Segoe UI', sans-serif"; ctx.textBaseline = "middle";
    for (const [u, y, isTop] of [[top, 0, true], [bot, 20, false]] as [Unit, number, boolean][]) {
      for (let d = floorU(first, u); d < last; d = nextU(d, u)) {
        const x = (d - d0()) * pd - x0, x2 = (nextU(d, u) - d0()) * pd - x0;
        ctx.fillStyle = K.line; ctx.fillRect(Math.round(x), y + 2, 1, 18);
        ctx.fillStyle = u === "day" && off(d) ? K.muted : K.text;
        let t = label(d, u, isTop, pd), tw = ctx.measureText(t).width;
        if (tw >= x2 - x - 4 && u === "week") { t = String(ymd(d)[2]); tw = ctx.measureText(t).width; }
        if (tw < x2 - x - 4) ctx.fillText(t, u === "day" ? x + (x2 - x - tw) / 2 : Math.max(x + 4, Math.min(4, x2 - tw - 4)), y + 11);
      }
    }
    ctx.fillStyle = K.line; ctx.fillRect(0, 20, w, 1); ctx.fillRect(0, 39, w, 1);
  };

  const drawBars = () => {
    const { w, h } = size(), top = body.scrollTop, pd = zoom(), rows = grid().rows, crit = showCrit() || view() === "tracking", base = showBase() || view() === "tracking";
    const ctx = fit(barsCv, w, h);
    if (pd >= 6) { ctx.fillStyle = K.off; for (let d = d0() + Math.floor(body.scrollLeft / pd); X(d) < w; d++) if (off(d)) ctx.fillRect(X(d), 0, pd, h); }
    const a = Math.floor(top / RH), b = Math.min(rows.length, a + Math.ceil(h / RH) + 1);
    if (K.grid) { ctx.fillStyle = K.grid; for (let i = a; i <= b; i++) ctx.fillRect(0, (i + 1) * RH - top - 1, w, 1); }
    const ss = new Set(sel());
    for (let i = a; i < b; i++) {
      const r = rows[i];
      if (ss.has(r.uid)) { ctx.fillStyle = "rgba(47,111,207,.09)"; ctx.fillRect(0, i * RH - top, w, RH); }
      else if (r.hl) { ctx.fillStyle = "rgba(242,178,51,.14)"; ctx.fillRect(0, i * RH - top, w, RH); }
    }
    const line = (d: number, color: string, dash: number[]) => { ctx.strokeStyle = color; ctx.setLineDash(dash); ctx.beginPath(); ctx.moveTo(Math.round(X(d)) + 0.5, 0); ctx.lineTo(Math.round(X(d)) + 0.5, h); ctx.stroke(); ctx.setLineDash([]); };
    line(today(), "#F2B233", [4, 3]);
    const sd = out()?.summary.status_day;
    if (sd) line(sd, "#2E9D57", [6, 3]);
    // Links
    const at = new Map(rows.map((r, i) => [r.uid, i]));
    const ends = (r: Row) => (r.ms ? [X(r.bar.s) - 6, X(r.bar.s) + 6] : [X(r.bar.s), X(r.bar.f)]);
    ctx.lineWidth = 1;
    rows.forEach((r, i) => r.bar.links.forEach(([pu, k]) => {
      const pi = at.get(pu);
      if (pi === undefined || Math.max(i, pi) < a || Math.min(i, pi) > b) return;
      const p = rows[pi], [p1, p2] = ends(p), [s1, s2] = ends(r);
      const fx = k[0] === "F" ? p2 : p1, tx = k[1] === "S" ? s1 : s2, outD = k[0] === "F" ? 1 : -1, inn = k[1] === "S" ? 1 : -1;
      const y1 = pi * RH - top + RH / 2, y2 = i * RH - top + RH / 2, ax = fx + outD * 6;
      const pts: [number, number][] = (inn === 1 ? tx >= ax : tx <= ax) ? [[fx, y1], [ax, y1], [ax, y2], [tx, y2]]
        : (() => { const ym = y2 + (y2 > y1 ? -RH / 2 : RH / 2), bx = tx - inn * 6; return [[fx, y1], [ax, y1], [ax, ym], [bx, ym], [bx, y2], [tx, y2]] as [number, number][]; })();
      const col = crit && r.crit && p.crit ? K.crit : K.link;
      ctx.strokeStyle = col; ctx.beginPath();
      pts.forEach(([x, y], n) => (n ? ctx.lineTo(Math.round(x) + 0.5, Math.round(y) + 0.5) : ctx.moveTo(Math.round(x) + 0.5, Math.round(y) + 0.5)));
      ctx.stroke();
      ctx.fillStyle = col; ctx.beginPath(); ctx.moveTo(tx, y2); ctx.lineTo(tx - inn * 5, y2 - 3.5); ctx.lineTo(tx - inn * 5, y2 + 3.5); ctx.fill();
    }));
    // Bars
    ctx.font = "11px 'Segoe UI', sans-serif"; ctx.textBaseline = "middle";
    for (let i = a; i < b; i++) {
      const r = rows[i], B = r.bar, y = i * RH - top, x1 = X(B.s), x2 = X(B.f);
      const isCrit = crit && r.crit, hasBase = base && B.bs !== null, by = hasBase ? y + 3 : y + 4, bh = hasBase ? 9 : 11;
      if (hasBase && B.bf !== null) {
        ctx.fillStyle = K.base;
        if (B.bf === B.bs) { const bx = X(B.bs!); ctx.beginPath(); ctx.moveTo(bx, y + 12); ctx.lineTo(bx + 4, y + 16); ctx.lineTo(bx, y + 20); ctx.lineTo(bx - 4, y + 16); ctx.fill(); }
        else ctx.fillRect(X(B.bs!), y + 14, Math.max(X(B.bf) - X(B.bs!), 2), 4);
      }
      if (showSlack() && r.kind === "task" && B.lf > B.f) { ctx.fillStyle = "#7FA6DA"; ctx.fillRect(x2, y + 15, X(B.lf) - x2, 2); }
      if (showLate() && r.kind === "task" && B.ls > B.s) { ctx.strokeStyle = K.muted; ctx.setLineDash([2, 2]); ctx.strokeRect(X(B.ls) + 0.5, by + 0.5, X(B.lf) - X(B.ls), bh); ctx.setLineDash([]); }
      let lx = x2 + 6;
      ctx.globalAlpha = r.off ? 0.35 : 1;
      if (r.kind === "group") { ctx.fillStyle = K.muted; ctx.fillRect(x1, y + 7, x2 - x1, 5); }
      else if (r.ms) {
        ctx.fillStyle = isCrit ? K.crit : K.ink;
        ctx.beginPath(); ctx.moveTo(x1, by - 1); ctx.lineTo(x1 + 6, by + 5); ctx.lineTo(x1, by + 11); ctx.lineTo(x1 - 6, by + 5); ctx.fill();
        lx = x1 + 10;
      } else if (r.kind === "summary" || r.kind === "project") {
        ctx.fillStyle = r.kind === "project" ? K.muted : K.ink;
        ctx.fillRect(x1, y + 6, x2 - x1, 4);
        ctx.beginPath(); ctx.moveTo(x1, y + 6); ctx.lineTo(x1 + 5, y + 6); ctx.lineTo(x1, y + 14); ctx.fill();
        ctx.beginPath(); ctx.moveTo(x2, y + 6); ctx.lineTo(x2 - 5, y + 6); ctx.lineTo(x2, y + 14); ctx.fill();
        if (B.pct > 0) { ctx.fillStyle = "#2E9D57"; ctx.fillRect(x1, y + 10, (x2 - x1) * B.pct / 100, 2); }
      } else {
        const segs = B.segs.length ? B.segs : [[B.s, B.f] as [number, number]];
        const fill = r.manual ? "#BFE6D3" : isCrit ? K.cfill : K.fill, edge = r.manual ? "#2E9D6B" : isCrit ? K.crit : K.blue;
        const done = B.s + (B.f - B.s) * B.pct / 100;
        segs.forEach(([s, f], n) => {
          const a1 = X(s), wd = Math.max(X(f) - a1, 2);
          if (n) { ctx.strokeStyle = edge; ctx.setLineDash([2, 2]); ctx.beginPath(); ctx.moveTo(X(segs[n - 1][1]), by + bh / 2); ctx.lineTo(a1, by + bh / 2); ctx.stroke(); ctx.setLineDash([]); }
          ctx.fillStyle = fill; ctx.fillRect(a1, by, wd, bh);
          ctx.fillStyle = edge; ctx.fillRect(a1, by + bh / 2 - 2, Math.max(0, Math.min(X(f), X(done)) - a1), 4);
          ctx.strokeStyle = edge;
          if (r.manual) { ctx.fillRect(a1, by, 3, bh); ctx.fillRect(a1 + wd - 3, by, 3, bh); } else ctx.strokeRect(a1 + 0.5, by + 0.5, wd - 1, bh - 1);
        });
      }
      if (B.dl !== null) {
        const dx = X(B.dl);
        ctx.fillStyle = B.dl < B.f ? K.crit : "#2E9D57";
        ctx.beginPath(); ctx.moveTo(dx - 4, y + 1); ctx.lineTo(dx + 4, y + 1); ctx.lineTo(dx, y + 7); ctx.fill(); ctx.fillRect(dx - 0.5, y + 5, 1, 13);
      }
      if (showLabels() && r.kind !== "group") { ctx.fillStyle = K.text; ctx.fillText(r.ms ? `${fmtDay(B.s)}` : r.res, lx, y + RH / 2); }
      ctx.globalAlpha = 1;
    }
    if (drag) {
      const r = drag.r, y = drag.i * RH - top;
      ctx.setLineDash([4, 3]); ctx.strokeStyle = K.blue; ctx.lineWidth = 1.5;
      if (drag.mode === "link") { ctx.beginPath(); ctx.moveTo(X(r.bar.f), y + RH / 2); ctx.lineTo(drag.mx - body.scrollLeft, drag.my - top); ctx.stroke(); }
      else if (drag.mode === "pct") { ctx.fillStyle = "rgba(46,157,87,.55)"; ctx.fillRect(X(r.bar.s), y + 8, (X(r.bar.f) - X(r.bar.s)) * drag.pct / 100, 4); }
      else { const g1 = X(r.bar.s + (drag.mode === "move" ? drag.dd : 0)), g2 = X(r.bar.f + drag.dd); ctx.strokeRect(g1 + 0.5, y + 2.5, Math.max(g2 - g1, 6), RH - 5); }
      ctx.setLineDash([]); ctx.lineWidth = 1;
    }
  };

  const pos = (e: MouseEvent) => { const b = body.getBoundingClientRect(); return [e.clientX - b.left + body.scrollLeft, e.clientY - b.top + body.scrollTop]; };
  const hit = (e: MouseEvent) => {
    const [x, y] = pos(e), i = Math.floor(y / RH), r = grid().rows[i], pd = zoom();
    let part = "";
    if (r && r.kind === "task") {
      const x1 = (r.bar.s - d0()) * pd, x2 = (r.bar.f - d0()) * pd, xp = x1 + (x2 - x1) * r.bar.pct / 100, low = y - i * RH > 10;
      part = r.ms ? (Math.abs(x - x1) <= 8 ? "move" : "") : x >= x2 - 5 && x <= x2 + 3 ? "size" : low && x >= x1 - 2 && Math.abs(x - xp) <= 4 ? "pct" : x >= x1 - 2 && x < x2 - 5 ? "move" : "";
    }
    return { i, x, y, r, part };
  };
  const endDay = (r: Row) => (r.bar.f % 1 === 0 && !r.ms ? r.bar.f - 1 : Math.floor(r.bar.f));
  const tipAt = (e: MouseEvent, html: string) => setTip({ x: Math.min(e.clientX + 14, innerWidth - 300), y: e.clientY + 18, html });
  const esc = (s: string) => s.replace(/[&<>]/g, c => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;" })[c]!);

  const down = (e: MouseEvent) => {
    if (e.button !== 0) return;
    const h = hit(e);
    if (h.r) { setSel(e.ctrlKey ? [...sel(), h.r.uid] : [h.r.uid]); setCur({ row: h.i, col: cur().col }); }
    if (!h.r || !h.part || e.shiftKey || e.ctrlKey) return;
    drag = { r: h.r, i: h.i, mode: h.part as Drag["mode"], x: h.x, y: h.y, dd: 0, mx: h.x, my: h.y, pct: h.r.bar.pct };
    e.preventDefault();
  };
  const move = (e: MouseEvent) => {
    if (!drag) {
      if (!body.contains(e.target as Node)) return setTip(null);
      const h = hit(e), r = h.r;
      body.style.cursor = h.part === "size" ? "ew-resize" : h.part === "pct" ? "col-resize" : h.part ? "grab" : "default";
      if (r && (h.part || r.kind === "summary")) tipAt(e, `<b>${esc(r.name)}</b><br>${fmtDay(r.bar.s, "long")} → ${fmtDay(endDay(r), "long")}<br>${r.bar.pct}% complete${r.res ? "<br>" + esc(r.res) : ""}${r.bar.bs !== null ? `<br>Baseline ${fmtDay(r.bar.bs)} → ${fmtDay(r.bar.bf! - 0.01)}` : ""}`
        + (r.kind === "task" ? '<br><span class="muted">Drag to move · drag the end to resize · drag the progress · drag down to link</span>' : ""));
      else setTip(null);
      return;
    }
    const [x, y] = pos(e), d = drag, pd = zoom();
    d.mx = x; d.my = y; d.dd = Math.round((x - d.x) / pd);
    if (d.mode === "move" && Math.abs(y - d.y) > RH) d.mode = "link";
    if (d.mode === "pct") { const x1 = (d.r.bar.s - d0()) * pd, x2 = (d.r.bar.f - d0()) * pd; d.pct = Math.max(0, Math.min(100, Math.round((x - x1) / Math.max(1, x2 - x1) * 20) * 5)); }
    const t = grid().rows[Math.floor(y / RH)];
    tipAt(e, d.mode === "link" ? `Link <b>${d.r.id}</b> → <b>${t && t !== d.r && t.uid > 0 ? t.id + " " + esc(t.name) : "…"}</b>` : d.mode === "pct" ? `Progress <b>${d.pct}%</b>`
      : d.mode === "move" ? `Start <b>${fmtDay(d.r.bar.s + d.dd, "long")}</b>` : `Finish <b>${fmtDay(Math.max(Math.floor(d.r.bar.s), endDay(d.r) + d.dd), "long")}</b>`);
    draw();
  };
  const up = async () => {
    const d = drag;
    if (!d) return;
    drag = null; setTip(null); draw();
    if (d.mode === "link") { const t = grid().rows[Math.floor(d.my / RH)]; if (t && t !== d.r && t.uid > 0) await act({ LinkSet: { pred: d.r.uid, succ: t.uid, kind: "FS", lag: "" } }); }
    else if (d.mode === "pct") { if (d.pct !== d.r.bar.pct) await set(d.r.uid, "pct", String(d.pct)); }
    else if (d.dd && d.mode === "move") await set(d.r.uid, "start", iso(d.r.bar.s + d.dd));
    else if (d.dd) await set(d.r.uid, "finish", iso(Math.max(Math.floor(d.r.bar.s), endDay(d.r) + d.dd)));
  };
  const setZ = (v: number, keep?: number) => {
    const day = keep ?? d0() + (body.scrollLeft + body.clientWidth / 2) / zoom();
    setZoom(Math.max(0.3, Math.min(120, v)));
    body.scrollLeft = (day - d0()) * zoom() - body.clientWidth / 2;
  };
  const fitAll = () => { const s = out()?.summary; if (!s) return; setZoom(Math.max(0.3, (body.clientWidth - 160) / Math.max(7, s.f - s.s + 4))); body.scrollLeft = (s.s - d0() - 2) * zoom(); };
  const scrollDay = (e: Event) => { body.scrollLeft = Math.max(0, ((e as CustomEvent).detail - d0()) * zoom() - 60); };
  const syncY = (e: Event) => { const v = (e as CustomEvent).detail; if (Math.abs(body.scrollTop - v) > 1) body.scrollTop = v; };

  const ro = new ResizeObserver(() => setSize({ w: body.clientWidth, h: body.clientHeight }));
  onMount(() => {
    ro.observe(body);
    addEventListener("mousemove", move); addEventListener("mouseup", up);
    document.addEventListener("gantt-fit", fitAll); document.addEventListener("gantt-scroll", scrollDay); document.addEventListener("chart-scroll", syncY);
    const s = out()?.summary;
    if (s) body.scrollLeft = Math.max(0, (s.s - d0() - 2) * zoom());
  });
  onCleanup(() => {
    ro.disconnect(); removeEventListener("mousemove", move); removeEventListener("mouseup", up);
    document.removeEventListener("gantt-fit", fitAll); document.removeEventListener("gantt-scroll", scrollDay); document.removeEventListener("chart-scroll", syncY);
  });
  createEffect(() => { themeTick(); out(); grid(); sel(); zoom(); size(); showCrit(); showBase(); showSlack(); showLate(); showLabels(); view(); baseline(); draw(); });

  return (
    <div class="chart">
      <canvas class="scale" ref={scaleCv} onDblClick={() => fitAll()} />
      <div class="cwrap">
        <div class="cbody" ref={body} onMouseDown={down} onScroll={() => { draw(); document.dispatchEvent(new CustomEvent("grid-scroll", { detail: body.scrollTop })); document.dispatchEvent(new CustomEvent("tl-redraw")); }}
          onWheel={e => { if (e.ctrlKey) { e.preventDefault(); setZ(zoom() * (e.deltaY < 0 ? 1.2 : 1 / 1.2), d0() + (body.scrollLeft + e.offsetX) / zoom()); } }}
          onDblClick={() => C.info()} onContextMenu={e => { const h = hit(e); if (h.r && !sel().includes(h.r.uid)) { setSel([h.r.uid]); setCur({ row: h.i, col: cur().col }); } openMenu(e, [
            { label: "Task Information…", icon: "info", run: C.info }, { label: "Scroll to Task", icon: "scroll", run: C.scrollTo }, { label: "Split Task…", icon: "split", run: C.split }, { label: "Mark on Track", icon: "track", run: C.onTrack }, "-",
            { label: "Show Critical Path", run: () => setShowCrit(!showCrit()), on: showCrit() }, { label: "Show Baseline", run: () => setShowBase(!showBase()), on: showBase() },
            { label: "Show Slack", run: () => setShowSlack(!showSlack()), on: showSlack() }, { label: "Show Late Bars", run: () => setShowLate(!showLate()), on: showLate() },
            { label: "Show Bar Labels", run: () => setShowLabels(!showLabels()), on: showLabels() }, "-", { label: "Zoom In", run: C.zoomIn }, { label: "Zoom Out", run: C.zoomOut }, { label: "Entire Project", run: fitAll },
          ]); }}>
          <div style={{ width: `${(d1() - d0()) * zoom()}px`, height: `${(grid().rows.length + 3) * RH}px` }} />
        </div>
        <canvas class="bars" ref={barsCv} />
      </div>
      <Show when={tip()}>{t => <div id="tip" style={{ left: `${t().x}px`, top: `${t().y}px` }} innerHTML={t().html} />}</Show>
    </div>
  );
};

/** The MS Project timeline: the whole project with the tasks marked Display on Timeline. */
export const Timeline = () => {
  let cv!: HTMLCanvasElement;
  const [w, setW] = createSignal(800);
  const [items, setItems] = createSignal<Row[]>([]);
  createEffect(() => { tick(); grid(); (async () => { const g = await call<{ rows: Row[] }>("query", { q: { ...query(), cols: ["name"], filter: { name: "", rules: [{ field: "timeline", test: "equals", value: "Yes", value2: "", or: false }], summaries: false, highlight: false }, group: [], collapsed: [], level: 0, project_summary: false } }); setItems(g?.rows ?? []); })(); });
  const draw = () => {
    if (!cv || !timeline()) return;
    pal();
    const h = 64, ctx = fit(cv, w(), h), s = out()?.summary;
    ctx.fillStyle = K.bg; ctx.fillRect(0, 0, w(), h);
    ctx.font = "11px 'Segoe UI', sans-serif"; ctx.textBaseline = "middle";
    if (!s) return;
    const L = 76, R = w() - 76, a = s.s, b = Math.max(s.f, a + 1), px = (d: number) => L + (d - a) / (b - a) * (R - L);
    ctx.fillStyle = K.muted; ctx.fillText("Start", 10, 20); ctx.fillText(s.start, 10, 36); ctx.fillText("Finish", R + 10, 20); ctx.fillText(s.finish, R + 10, 36);
    ctx.fillStyle = K.pale; ctx.fillRect(L, 22, R - L, 20);
    const step: Unit = (b - a) / (R - L) > 3 ? "quarter" : "month";
    ctx.fillStyle = K.muted;
    for (let d = nextU(floorU(a, step), step); d < b; d = nextU(d, step)) { ctx.fillRect(px(d), 22, 1, 20); if (px(nextU(d, step)) - px(d) > 40) ctx.fillText(label(d, step, true, 1), px(d) + 3, 53); }
    const lanes: number[] = [];
    for (const r of items()) {
      const x1 = px(r.bar.s), x2 = Math.max(px(r.bar.f), x1 + 2);
      if (r.ms) { ctx.fillStyle = "#F2B233"; ctx.beginPath(); ctx.moveTo(x1, 24); ctx.lineTo(x1 + 7, 32); ctx.lineTo(x1, 40); ctx.lineTo(x1 - 7, 32); ctx.fill(); ctx.fillStyle = K.ink; ctx.fillText(r.name, x1 + 8, 12); continue; }
      let lane = lanes.findIndex(e => e <= x1); if (lane < 0) lane = lanes.length; lanes[lane] = x2 + 4;
      const y = 23 + (lane % 2) * 9;
      ctx.fillStyle = r.kind === "summary" ? K.blue : "#6E9FE6"; ctx.fillRect(x1, y, x2 - x1, 9);
      ctx.fillStyle = K.ink; if (ctx.measureText(r.name).width < x2 - x1 + 40) ctx.fillText(r.name, x1, lane % 2 ? 58 : 12);
    }
    if (!items().length) { ctx.fillStyle = K.muted; ctx.fillText("Right-click a task and choose Add to Timeline", L + 8, 32); }
    const t = today(); ctx.strokeStyle = "#F2B233"; ctx.beginPath(); ctx.moveTo(px(t), 18); ctx.lineTo(px(t), 46); ctx.stroke();
  };
  const ro = new ResizeObserver(() => setW(cv.parentElement!.clientWidth));
  onMount(() => { ro.observe(cv.parentElement!); document.addEventListener("tl-redraw", draw); });
  onCleanup(() => { ro.disconnect(); document.removeEventListener("tl-redraw", draw); });
  createEffect(() => { themeTick(); items(); w(); timeline(); out(); draw(); });
  const click = (e: MouseEvent) => {
    const s = out()?.summary; if (!s) return;
    const L = 76, R = w() - 76, d = s.s + (e.offsetX - L) / (R - L) * (Math.max(s.f, s.s + 1) - s.s);
    document.dispatchEvent(new CustomEvent("gantt-scroll", { detail: d - 7 }));
  };
  return <div class="tlwrap"><canvas ref={cv} onClick={click} onContextMenu={e => openMenu(e, [{ label: "Hide Timeline", run: C.timelineBar }])} /></div>;
};
