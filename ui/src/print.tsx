// File › Print: MS Project-style print settings with a live page preview. Pages are laid out
// here (table + vector Gantt per page, headers, footers, logos, legend, page numbers) and printed
// with the WebView's print dialog, where "Save as PDF" makes a PDF.
import { For, Show, createMemo, createSignal, onCleanup } from "solid-js";
import { Portal } from "solid-js/web";
import { createStore } from "solid-js/store";
import { Row, fmtDay, iso, dateDay } from "./api";
import { grid, out, opts, P, act, cols, zoom, call, view, setBackstage, toast } from "./store";
import { gstyle, floorU, nextU, label, off, Unit } from "./gantt";
import * as F from "./fmt";
import { Pick, Check, Seg } from "./ui";

/** Paper sizes in millimetres (portrait). */
export const PAPERS: [string, number, number][] = [["A4", 210, 297], ["A3", 297, 420], ["A2", 420, 594], ["A1", 594, 841], ["Letter", 215.9, 279.4], ["Legal", 215.9, 355.6], ["Tabloid", 279.4, 431.8]];
const MM = 96 / 25.4, RHP = 18, SCALE_H = 34, IDW = 30;
type Slots = [string, string, string];
export type PageSetup = {
  paper: string; landscape: boolean; margins: [number, number, number, number]; scale: number; fit: number;
  range: "all" | "dates"; from: string; to: string; pages: string; cols: number; repeat: boolean; unit: string;
  header: Slots; footer: Slots; legend: "every" | "last" | "none"; legendText: string; clientLogo: string; grid: boolean; idCol: boolean;
};
export const CODES: [string, string][] = [["&[Page]", "Page number"], ["&[Pages]", "Total pages"], ["&[Date]", "Today's date"], ["&[Project]", "Project name"], ["&[Company]", "Company"], ["&[Client]", "Client"],
  ["&[Manager]", "Manager"], ["&[Status]", "Status date"], ["&[Start]", "Project start"], ["&[Finish]", "Project finish"], ["&[Logo]", "Company logo"], ["&[Client Logo]", "Client logo"]];
const defaults = (): PageSetup => ({
  paper: opts().paper || "A4", landscape: opts().landscape !== false, margins: [12, 10, 12, 10], scale: 100, fit: 1, range: "all", from: "", to: "", pages: "", cols: 3, repeat: true, unit: "auto",
  header: ["&[Logo]", "&[Project]", "&[Client Logo]"], footer: ["Printed &[Date]", "Page &[Page] of &[Pages]", "&[Company]"], legend: "every", legendText: "&[Project]\nStatus date: &[Status]", clientLogo: "", grid: true, idCol: true,
});
const [ps, setPs] = createStore<PageSetup>(defaults());
let loadedFor = "";
/** The plan's saved page setup (project.ui.page) over the defaults. */
const load = () => { const key = `${out()?.path}|${out()?.rev}`; if (key === loadedFor) return; loadedFor = key; setPs({ ...defaults(), ...(P().ui?.page ?? {}) }); };
const save = () => { if (JSON.stringify(P().ui?.page ?? {}) !== JSON.stringify(ps)) act({ Ui: { key: "page", value: JSON.parse(JSON.stringify(ps)) } }); };

const trunc = (t: string, w: number, px = 11) => { const n = Math.floor((w - 6) / (px * 0.56)); return t.length > n ? t.slice(0, Math.max(0, n - 1)) + "…" : t; };
const codes = (t: string, page: number, pages: number) => {
  const p = P(), s = out()!.summary;
  return t.replace(/&\[Page\]/g, String(page)).replace(/&\[Pages\]/g, String(pages)).replace(/&\[Date\]/g, fmtDay(dateDay(new Date()), "long")).replace(/&\[Project\]/g, p.name)
    .replace(/&\[Company\]/g, p.company).replace(/&\[Client\]/g, p.client).replace(/&\[Manager\]/g, p.manager).replace(/&\[Status\]/g, s.status || "not set").replace(/&\[Start\]/g, s.start).replace(/&\[Finish\]/g, s.finish);
};
/** Header/footer text with codes; logo codes become pictures. */
const Slot = (p: { t: string; page: number; pages: number; h: number }) => (
  <For each={p.t.split(/(&\[(?:Client )?Logo\])/)}>{part => part === "&[Logo]" ? (P().logo && <img src={P().logo} style={{ "max-height": `${p.h}px`, "max-width": "45%" }} />)
    : part === "&[Client Logo]" ? (ps.clientLogo && <img src={ps.clientLogo} style={{ "max-height": `${p.h}px`, "max-width": "45%" }} />)
    : <span style={{ "white-space": "pre-line" }}>{codes(part, p.page, p.pages)}</span>}</For>
);

type Page = { rows: [number, number]; t: [number, number]; table: boolean; n: number };
/** Splits the rows and the time range into pages, across then down, like MS Project. */
const layout = createMemo(() => {
  const s = out()?.summary, g = grid();
  if (!s) return null;
  const [pw, ph] = (() => { const p = PAPERS.find(x => x[0] === ps.paper) ?? PAPERS[0]; return ps.landscape ? [p[2], p[1]] : [p[1], p[2]]; })();
  const k = ps.scale / 100, W = (pw - ps.margins[1] - ps.margins[3]) * MM / k, H = (ph - ps.margins[0] - ps.margins[2]) * MM / k;
  const hasH = ps.header.some(x => x.trim()), hasF = ps.footer.some(x => x.trim());
  const headH = hasH ? 52 : 0, footH = hasF ? 30 : 0, legH = ps.legend === "none" ? 0 : 58;
  const all = g.cols.map((c, j) => ({ c, j, w: cols()[j]?.[1] ?? 90 })).filter(x => x.c.key !== "mode"), shown = ps.cols > 0 ? all.slice(0, ps.cols) : all;
  const TW = (ps.idCol ? IDW : 0) + shown.reduce((a, c) => a + c.w, 0);
  const d0 = ps.range === "dates" && ps.from ? dateDay(new Date(ps.from)) : Math.floor(s.s) - 3;
  const d1 = ps.range === "dates" && ps.to ? dateDay(new Date(ps.to)) + 1 : Math.ceil(s.f) + 7;
  const rowsIn = (leg: boolean) => Math.max(1, Math.floor((H - headH - footH - (leg ? legH : 0) - SCALE_H - 6) / RHP));
  const cw0 = Math.max(80, W - TW), cwN = ps.repeat ? cw0 : W;
  const pd = ps.fit > 0 ? (cw0 + cwN * (ps.fit - 1)) / Math.max(1, d1 - d0) : zoom();
  const slices: [number, number, boolean][] = [];
  for (let x = d0, first = true; x < d1; first = false) { const w = (first ? cw0 : cwN) / pd; slices.push([x, Math.min(d1, x + w), first || ps.repeat]); x += w; }
  const rows = g.rows, per = rowsIn(ps.legend === "every"), pages: Page[] = [];
  for (let r = 0; r < Math.max(1, rows.length); r += per) for (const [a, b, tb] of slices) pages.push({ rows: [r, Math.min(rows.length, r + per)], t: [a, b], table: tb, n: pages.length + 1 });
  return { pw, ph, k, W, H, headH, footH, legH, shown, TW, pd, pages, rows };
});
/** Pages chosen in "Pages from–to" (e.g. 1-3,5), or all. */
const chosen = () => {
  const L = layout(); if (!L) return [];
  const want = ps.pages.trim(); if (!want) return L.pages;
  const ok = new Set<number>(); want.split(",").forEach(x => { const [a, b] = x.split("-").map(v => +v.trim()); for (let n = a; n <= (b || a); n++) ok.add(n); });
  return L.pages.filter(p => ok.has(p.n));
};

/** One printed page at real size (CSS px at 96 dpi). */
const Sheet = (p: { pg: Page; pages: number }) => {
  const L = layout()!, st = gstyle(), rows = () => L.rows.slice(p.pg.rows[0], p.pg.rows[1]);
  const tw = () => (p.pg.table ? L.TW : 0), cw = () => (p.pg.t[1] - p.pg.t[0]) * L.pd;
  const X = (d: number) => tw() + (d - p.pg.t[0]) * L.pd;
  const unit = (): [Unit, Unit] => { const u = ps.unit, pd = L.pd; if (u === "day") return ["week", "day"]; if (u === "week") return ["month", "week"]; if (u === "month") return ["quarter", "month"]; if (u === "quarter") return ["year", "quarter"]; return pd >= 16 ? ["week", "day"] : pd >= 4 ? ["month", "week"] : pd >= 1.2 ? ["quarter", "month"] : ["year", "quarter"]; };
  const last = () => p.pg.n === layout()!.pages.length;
  const showLeg = () => ps.legend === "every" || (ps.legend === "last" && last());
  const bodyH = () => SCALE_H + rows().length * RHP + 2;
  const Bar = (q: { r: Row; y: number }) => {
    const B = q.r.bar, x1 = Math.max(tw(), X(B.s)), x2 = Math.min(tw() + cw(), X(B.f)), y = q.y, crit = q.r.crit;
    if (X(B.f) < tw() || X(B.s) > tw() + cw()) return null;
    if (q.r.ms) { const x = X(B.s); return x < tw() || x > tw() + cw() ? null : <g><path d={`M${x} ${y + 3}l6 6l-6 6l-6-6z`} fill={crit ? st.crit : st.sum} /><text x={x + 9} y={y + 13} font-size="9" fill="#333">{fmtDay(B.s)}</text></g>; }
    if (q.r.kind === "summary" || q.r.kind === "project") return <g fill={st.sum}><rect x={x1} y={y + 5} width={Math.max(1, x2 - x1)} height={4} />{X(B.s) >= tw() && <path d={`M${x1} ${y + 5}h5l-5 8z`} />}{X(B.f) <= tw() + cw() && <path d={`M${x2} ${y + 5}h-5l5 8z`} />}</g>;
    const done = X(B.s + (B.f - B.s) * B.pct / 100);
    return <g><rect x={x1} y={y + 4} width={Math.max(1.5, x2 - x1)} height={10} fill={crit ? st.cfill : st.fill} stroke={crit ? st.crit : st.edge} stroke-width=".8" />
      {B.pct > 0 && <rect x={x1} y={y + 7} width={Math.max(0, Math.min(x2, done) - x1)} height={4} fill={crit ? st.crit : st.edge} />}
      {X(B.f) + 4 < tw() + cw() - 30 && q.r.res && <text x={X(B.f) + 4} y={y + 13} font-size="9" fill="#333">{trunc(q.r.res, tw() + cw() - X(B.f) - 4, 9)}</text>}</g>;
  };
  return (
    <div class="ppage" style={{ width: `${L.pw * MM}px`, height: `${L.ph * MM}px` }}>
      <div class="pin" style={{ left: `${ps.margins[3] * MM}px`, top: `${ps.margins[0] * MM}px`, width: `${L.W}px`, height: `${L.H}px`, transform: `scale(${L.k})` }}>
        <Show when={L.headH}><div class="phead" style={{ height: `${L.headH - 8}px` }}><For each={[0, 1, 2]}>{i => <div class={`ps${i}`}><Slot t={ps.header[i]} page={p.pg.n} pages={p.pages} h={L.headH - 12} /></div>}</For></div></Show>
        <svg width={tw() + cw()} height={bodyH()} class="psvg" text-rendering="geometricPrecision" font-family="'IBM Plex Sans', 'Segoe UI', sans-serif">
          {/* timescale */}
          <rect x={tw()} y={0} width={cw()} height={SCALE_H} fill="#F3F5F8" />
          <For each={unit()}>{(u, ti) => { const ticks: number[] = []; for (let d = floorU(p.pg.t[0], u); d < p.pg.t[1]; d = nextU(d, u)) ticks.push(d); return <For each={ticks}>{d => { const a = Math.max(tw(), X(d)), b = Math.min(tw() + cw(), X(nextU(d, u))), y = ti() * 17; const t = label(d, u, ti() === 0, L.pd); return <g><line x1={X(d)} x2={X(d)} y1={y} y2={y + 17} stroke="#C9CFD8" stroke-width=".6" />{b - a > t.length * 5.6 && <text x={u === "day" ? (a + b) / 2 : a + 3} y={y + 12} font-size="9.5" fill="#33404F" text-anchor={u === "day" ? "middle" : "start"}>{t}</text>}</g>; }}</For>; }}</For>
          <line x1={tw()} x2={tw() + cw()} y1={17} y2={17} stroke="#C9CFD8" stroke-width=".6" />
          {/* non-working days */}
          <Show when={L.pd >= 5}><For each={Array.from({ length: Math.ceil(p.pg.t[1] - p.pg.t[0]) + 1 }, (_, i) => Math.floor(p.pg.t[0]) + i).filter(d => off(d))}>{d => <rect x={Math.max(tw(), X(d))} y={SCALE_H} width={Math.max(0, Math.min(tw() + cw(), X(d + 1)) - Math.max(tw(), X(d)))} height={rows().length * RHP} fill="#F2F4F6" />}</For></Show>
          {/* table */}
          <Show when={p.pg.table}>
            <rect x={0} y={0} width={L.TW} height={SCALE_H} fill="#EAF0F8" />
            {ps.idCol && <text x={IDW / 2} y={SCALE_H - 10} font-size="9.5" font-weight="600" text-anchor="middle" fill="#0C1A2B">ID</text>}
            <For each={L.shown}>{(c, i) => { const x = (ps.idCol ? IDW : 0) + L.shown.slice(0, i()).reduce((a, q) => a + q.w, 0); return <g><line x1={x} x2={x} y1={0} y2={bodyH()} stroke="#D5DAE1" stroke-width=".6" /><text x={x + 4} y={SCALE_H - 10} font-size="9.5" font-weight="600" fill="#0C1A2B">{trunc(c.c.name, c.w, 9.5)}</text></g>; }}</For>
            <line x1={L.TW} x2={L.TW} y1={0} y2={bodyH()} stroke="#8D99A8" stroke-width=".8" />
          </Show>
          <line x1={0} x2={tw() + cw()} y1={SCALE_H} y2={SCALE_H} stroke="#8D99A8" stroke-width=".8" />
          <For each={rows()}>{(r, i) => {
            const y = SCALE_H + i() * RHP, rs = r.style ?? {}, sum = r.kind !== "task";
            return <g>
              {ps.grid && <line x1={0} x2={tw() + cw()} y1={y + RHP} y2={y + RHP} stroke="#E6E9EE" stroke-width=".5" />}
              <Show when={p.pg.table}>
                {ps.idCol && <text x={IDW / 2} y={y + 12.5} font-size="9.5" text-anchor="middle" fill="#55606E">{r.uid > 0 ? r.id : ""}</text>}
                <For each={L.shown}>{(c, k) => {
                  const x = (ps.idCol ? IDW : 0) + L.shown.slice(0, k()).reduce((a, q) => a + q.w, 0), f = F.parse([rs["*"], rs[c.c.key]].filter(Boolean).join(";")), ind = c.c.key === "name" ? r.lvl * 10 : 0;
                  const t = r.cells[c.j] ?? "", z = f.z ? f.z * 0.85 : 10, al = f.a || (["Num", "Int", "Dur", "Cost", "Pct", "Work"].includes(c.c.kind) ? "r" : "");
                  return <g>{f.f && <rect x={x} y={y} width={c.w} height={RHP} fill={f.f} />}
                    <text x={al === "r" ? x + c.w - 4 : al === "c" ? x + c.w / 2 : x + 4 + ind} y={y + 12.5} font-size={String(z)} text-anchor={al === "r" ? "end" : al === "c" ? "middle" : "start"} fill={f.c || (r.off ? "#9AA3AE" : "#1B2733")}
                      style={{ "font-weight": f.b || sum ? 600 : 400, "font-style": f.i ? "italic" : "normal", "text-decoration": [f.u && "underline", f.s && "line-through"].filter(Boolean).join(" ") || "none" }}>{trunc(t, c.w - ind, z)}</text></g>;
                }}</For>
              </Show>
              <Bar r={r} y={y} />
            </g>;
          }}</For>
          {/* status date */}
          {out()!.summary.status_day && X(out()!.summary.status_day!) > tw() && X(out()!.summary.status_day!) < tw() + cw() && <line x1={X(out()!.summary.status_day!)} x2={X(out()!.summary.status_day!)} y1={SCALE_H} y2={bodyH()} stroke="#2E9D57" stroke-dasharray="4 3" stroke-width=".8" />}
          <rect x={0} y={0} width={tw() + cw()} height={bodyH()} fill="none" stroke="#8D99A8" stroke-width=".8" />
        </svg>
        <Show when={showLeg()}>
          <div class="plegend" style={{ height: `${L.legH - 8}px` }}>
            <div class="plt"><Slot t={ps.legendText} page={p.pg.n} pages={p.pages} h={40} /></div>
            <svg width="430" height="44" font-size="9.5" fill="#1B2733" font-family="'IBM Plex Sans', 'Segoe UI', sans-serif">
              <rect x="4" y="6" width="30" height="9" fill={st.fill} stroke={st.edge} stroke-width=".8" /><text x="40" y="14">Task</text>
              <rect x="4" y="26" width="30" height="9" fill={st.cfill} stroke={st.crit} stroke-width=".8" /><text x="40" y="34">Critical</text>
              <rect x="110" y="6" width="30" height="9" fill={st.fill} stroke={st.edge} stroke-width=".8" /><rect x="110" y="8.5" width="16" height="4" fill={st.edge} /><text x="146" y="14">Progress</text>
              <path d="M125 24l6 6l-6 6l-6-6z" fill={st.sum} /><text x="146" y="34">Milestone</text>
              <g fill={st.sum}><rect x="220" y="7" width="30" height="4" /><path d="M220 7h5l-5 8z" /><path d="M250 7h-5l5 8z" /></g><text x="256" y="14">Summary</text>
              <line x1="220" x2="250" y1="31" y2="31" stroke="#2E9D57" stroke-dasharray="4 3" /><text x="256" y="34">Status date</text>
            </svg>
          </div>
        </Show>
        <Show when={L.footH}><div class="pfoot" style={{ height: `${L.footH - 6}px` }}><For each={[0, 1, 2]}>{i => <div class={`ps${i}`}><Slot t={ps.footer[i]} page={p.pg.n} pages={p.pages} h={L.footH - 8} /></div>}</For></div></Show>
      </div>
    </div>
  );
};

/** The task views print through the page layout; other views print as shown on screen. */
export const printable = () => ["gantt", "tracking", "timeline"].includes(view());
const [printing, setPrinting] = createSignal(false);
export const printNow = () => {
  save();
  const L = layout(); if (!L) return;
  setPrinting(true);
  const css = document.createElement("style");
  css.textContent = `@page { size: ${L.pw}mm ${L.ph}mm; margin: 0 }`;
  document.head.append(css);
  document.documentElement.classList.add("printing");
  setTimeout(() => { window.print(); document.documentElement.classList.remove("printing"); css.remove(); setPrinting(false); }, 120);
};

const Slots = (p: { k: "header" | "footer"; label: string }) => (
  <div class="pslots"><b>{p.label}</b>
    <For each={["Left", "Center", "Right"]}>{(l, i) => <label><span>{l}</span><textarea rows="2" value={ps[p.k][i()]} onInput={e => setPs(p.k, i(), e.currentTarget.value)} /></label>}</For>
  </div>
);
const Codes = () => <p class="muted pcodes">Codes: <For each={CODES}>{([c, l]) => <code title={l} onClick={() => navigator.clipboard?.writeText(c).then(() => toast(`${c} copied: paste it into a box.`, true))}>{c}</code>}</For></p>;

/** File › Print: settings on the left, the pages on the right. */
export const PrintPage = () => {
  load();
  const [tab, setTab] = createSignal("page"), [zoomP, setZoomP] = createSignal(0.55);
  onCleanup(save);
  const pickClient = async () => { const l = await call<string | null>("pick_logo"); if (l) setPs("clientLogo", l); };
  const L = layout;
  return (
    <div class="printpage">
      <div class="pset">
        <h1>Print</h1>
        <Show when={printable()} fallback={<><p>This view prints as shown on screen.</p><button class="btn pri" onClick={() => { setBackstage(null); setTimeout(() => window.print(), 80); }}>Print</button><p class="muted">Page Setup, headers, footers and page ranges apply to the Gantt Chart and Tracking Gantt. Open one of them to use them.</p></>}>
          <div class="pbtns"><button class="btn pri big" onClick={printNow}><svg class="ic"><use href="#i-print" /></svg> Print / Save as PDF</button></div>
          <p class="muted">In the print window choose your printer, or <b>Save as PDF</b>. {L()?.pages.length ?? 0} {L()?.pages.length === 1 ? "page" : "pages"} · {ps.paper} {ps.landscape ? "landscape" : "portrait"}.</p>
          <h4>Settings</h4>
          <label class="pl">What to print</label><Pick value={ps.range} set={v => setPs("range", v as "all")} options={[["all", "Entire project"], ["dates", "Specific dates"]]} />
          <Show when={ps.range === "dates"}><div class="prow"><label>From<input type="date" value={ps.from || iso(out()!.summary.s)} onChange={e => setPs("from", e.currentTarget.value)} /></label><label>To<input type="date" value={ps.to || iso(out()!.summary.f)} onChange={e => setPs("to", e.currentTarget.value)} /></label></div></Show>
          <label class="pl">Pages <span class="muted">(e.g. 1-3, 5; empty = all)</span></label><input value={ps.pages} placeholder={`1-${L()?.pages.length ?? 1}`} onInput={e => setPs("pages", e.currentTarget.value)} />
          <div class="prow"><label>Paper<Pick value={ps.paper} set={v => setPs("paper", v)} options={PAPERS.map(p => p[0])} /></label><label>Orientation<Pick value={ps.landscape ? "1" : "0"} set={v => setPs("landscape", v === "1")} options={[["1", "Landscape"], ["0", "Portrait"]]} /></label></div>
          <h4>Page Setup</h4>
          <Seg value={tab()} set={setTab} options={[["page", "Page"], ["margins", "Margins"], ["header", "Header"], ["footer", "Footer"], ["legend", "Legend"], ["view", "View"]]} />
          <div class="ptab">
            <Show when={tab() === "page"}>
              <label class="pl">Scaling</label>
              <div class="prow"><label><input type="radio" checked={!ps.fit} onChange={() => setPs("fit", 0)} /> Adjust to <input type="number" min="20" max="200" style={{ width: "62px" }} value={ps.scale} onChange={e => setPs("scale", Math.min(200, Math.max(20, +e.currentTarget.value)))} /> % size</label></div>
              <div class="prow"><label><input type="radio" checked={ps.fit > 0} onChange={() => setPs("fit", 1)} /> Fit timescale to <input type="number" min="1" max="20" style={{ width: "52px" }} value={ps.fit || 1} onChange={e => setPs("fit", Math.max(1, +e.currentTarget.value))} /> page(s) wide</label></div>
              <p class="muted">Adjust uses the Gantt chart's current zoom. Fit stretches or squeezes the dates to the number of pages.</p>
            </Show>
            <Show when={tab() === "margins"}>
              <div class="prow">{(["Top", "Right", "Bottom", "Left"] as const).map((l, i) => <label>{l} (mm)<input type="number" min="0" max="60" value={ps.margins[i]} onChange={e => setPs("margins", i, +e.currentTarget.value)} /></label>)}</div>
            </Show>
            <Show when={tab() === "header"}><Slots k="header" label="Header" /><Codes /><div class="prow"><button class="btn sm" onClick={pickClient}>Client logo…</button>{ps.clientLogo && <><img src={ps.clientLogo} class="logoprev" /><button class="btn sm" onClick={() => setPs("clientLogo", "")}>Remove</button></>}<span class="muted">Company logo: File › Options.</span></div></Show>
            <Show when={tab() === "footer"}><Slots k="footer" label="Footer" /><Codes /></Show>
            <Show when={tab() === "legend"}>
              <label class="pl">Legend on</label><Seg value={ps.legend} set={v => setPs("legend", v as "every")} options={[["every", "Every page"], ["last", "Last page"], ["none", "None"]]} />
              <label class="pl">Legend text</label><textarea rows="3" value={ps.legendText} onInput={e => setPs("legendText", e.currentTarget.value)} /><Codes />
            </Show>
            <Show when={tab() === "view"}>
              <label class="pl">Columns</label>
              <div class="prow"><label><input type="radio" checked={ps.cols === 0} onChange={() => setPs("cols", 0)} /> All sheet columns</label></div>
              <div class="prow"><label><input type="radio" checked={ps.cols > 0} onChange={() => setPs("cols", 3)} /> First <input type="number" min="1" max="30" style={{ width: "52px" }} value={ps.cols || 3} onChange={e => setPs("cols", Math.max(1, +e.currentTarget.value))} /> columns</label></div>
              <Check label="Repeat the columns on every page" value={ps.repeat} set={v => setPs("repeat", v)} />
              <Check label="Show the ID column" value={ps.idCol} set={v => setPs("idCol", v)} />
              <Check label="Gridlines between rows" value={ps.grid} set={v => setPs("grid", v)} />
              <label class="pl">Timescale</label><Pick value={ps.unit} set={v => setPs("unit", v)} options={[["auto", "Automatic"], ["day", "Days"], ["week", "Weeks"], ["month", "Months"], ["quarter", "Quarters"]]} />
            </Show>
          </div>
          <p><button class="btn sm" onClick={() => setPs(defaults())}>Reset to defaults</button></p>
        </Show>
      </div>
      <Show when={printable() && L()}>
        <div class="pprev">
          <div class="pzoom"><span class="muted">Preview</span><input type="range" min="0.25" max="1.2" step="0.05" value={zoomP()} onInput={e => setZoomP(+e.currentTarget.value)} /><span class="muted">{Math.round(zoomP() * 100)}%</span></div>
          <For each={chosen()}>{pg => <div class="pwrap" style={{ width: `${L()!.pw * MM * zoomP()}px`, height: `${L()!.ph * MM * zoomP()}px` }}><div style={{ transform: `scale(${zoomP()})`, "transform-origin": "0 0" }}><Sheet pg={pg} pages={L()!.pages.length} /></div><small>Page {pg.n} of {L()!.pages.length}</small></div>}</For>
        </div>
      </Show>
      <Show when={printing()}><Portal mount={document.body}><div id="printroot"><For each={chosen()}>{pg => <Sheet pg={pg} pages={L()!.pages.length} />}</For></div></Portal></Show>
    </div>
  );
};
