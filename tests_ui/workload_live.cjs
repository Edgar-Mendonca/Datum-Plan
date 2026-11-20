// Live Workload tests: the built window (ui/dist) in Chromium, with every change answered by
// the real engine (crates/core/examples/sheetbridge.rs), so each check is what a planner would see.
// Run: npm run test:workload (builds the window and the bridge first).
const { chromium } = require("playwright");
const fs = require("fs"), http = require("http"), path = require("path"), { spawn } = require("child_process");
const root = path.join(__dirname, "../ui/dist"), fix = JSON.parse(fs.readFileSync(path.join(__dirname, "fixture.json")));
const types = { ".js": "text/javascript", ".css": "text/css", ".svg": "image/svg+xml", ".woff2": "font/woff2", ".woff": "font/woff", ".html": "text/html" };
const srv = http.createServer((q, r) => {
  const f = path.join(root, q.url === "/" ? "index.html" : decodeURIComponent(q.url.split("?")[0]));
  fs.readFile(f, (e, b) => { if (e) { r.writeHead(404); return r.end(); } r.writeHead(200, { "content-type": types[path.extname(f)] ?? "application/octet-stream" }); r.end(b); });
});
let failed = 0, passed = 0;
const check = (name, ok, info = "") => { console.log(`${ok ? "ok  " : "FAIL"} ${name}${ok ? "" : "  " + info}`); if (ok) passed++; else failed++; };

(async () => {
  const exe = path.join(__dirname, "../target/debug/examples/sheetbridge");
  const br = spawn(exe, [], { stdio: ["ignore", "pipe", "inherit"] });
  const port = await new Promise(ok => br.stdout.once("data", d => ok(+String(d).trim())));
  const call = async (c, a = {}) => { const r = await fetch(`http://127.0.0.1:${port}/`, { method: "POST", body: JSON.stringify({ c, a }) }); const j = await r.json(); if (j.err) throw new Error(j.err); return j.ok; };
  // Overload the busiest person: half their usual units, so they are over capacity in some weeks.
  { const w = await call("workload", { unit: "week" }), ps = w.trades.flatMap(t => t.people).sort((a, b) => b.peak - a.peak);
    await call("apply", { op: { ResSet: { uid: ps[0].uid, field: "max", value: "40%" } } }); }
  await new Promise(ok => srv.listen(0, ok));
  const b = await chromium.launch(process.env.CHROMIUM ? { executablePath: process.env.CHROMIUM } : {});
  const p = await (await b.newContext({ viewport: { width: 1500, height: 950 } })).newPage();
  const errors = [];
  p.on("pageerror", e => errors.push(e.message));
  const start = await call("state");
  fix.state.project = { ...fix.state.project, ...start.project };
  await p.addInitScript(([f, port]) => {
    const opts = { dateFmt: "%d-%b-%y", autosave: 0, beta: false, currency: { code: "USD", symbol: "$", decimals: 2, after: false }, company: "", logo: "", workspace: "", setupPanel: false, theme: "light", docTemplates: {} };
    const m = { ...f, options: opts, startup_file: null, recovery: null, check_update: null, templates: [], recent: [], version: "test", resources: [], checks: { checks: [], score: { score: 100, grade: "Good", parts: [] }, trend: [] }, task_info: null };
    const bridge = async (c, a) => { const r = await fetch(`http://127.0.0.1:${port}/`, { method: "POST", body: JSON.stringify({ c, a }) }); const j = await r.json(); if (j.err) throw j.err; return j.ok; };
    window.__calls = [];
    window.__TAURI__ = { core: { invoke: async (c, a) => {
      window.__calls.push([c, a]);
      if (c === "apply" || c === "history") { const s = await bridge(c, a); f.state = { ...f.state, rev: f.state.rev + 1, project: { ...f.state.project, ...s.project }, undo: s.undo, redo: s.redo }; return f.state; }
      if (c === "procurement" || c === "documents" || c === "contract" || c === "board" || c === "workload") return bridge(c, a);
      if (c === "set_options") { m.options = a.opts; return null; }
      if (c in m) return m[c];
      return null;
    } }, event: { listen: async () => () => {} } };
  }, [fix, port]);
  await p.goto(`http://localhost:${srv.address().port}/`); await p.waitForTimeout(700);
  await p.click(".bs aside .back"); await p.waitForTimeout(200);
  await p.click('.side button[title="Workload"]'); await p.waitForTimeout(900);
  const W = ms => p.waitForTimeout(ms);
  const cell = (i, j) => `.rg td[data-rc="${i}:${j}"]`;
  const colOf = async name => p.$$eval(".rg thead th span", (s, n) => s.findIndex(x => x.textContent.trim() === n) + 1, name);
  const typeIn = async (i, j, text, key = "Enter") => { await p.click(cell(i, j)); await W(80); await p.keyboard.type(text); await W(120); await p.keyboard.press(key); await W(450); };
  const paste = async text => { await p.evaluate(t => { const dt = new DataTransfer(); dt.setData("text/plain", t); document.dispatchEvent(new ClipboardEvent("paste", { clipboardData: dt, bubbles: true, cancelable: true })); }, text); await W(600); };
  const T = async (name, fn) => { try { await fn(); } catch (e) { check(name, false, `threw: ${e.message ?? e}`); } };

  const WL = async () => call("workload", { unit: "week" });
  const people = o => o.trades.flatMap(t => t.people);
  const S = async () => (await call("state")).project;
  await T("the grid", async () => {
    const o = await WL();
    check("a row per trade and per person, a column per week", (await p.$$(".wltrade")).length === o.trades.length && (await p.$$(".wlperson")).length === people(o).length && (await p.$$(".wl thead th")).length === o.periods.length + 2, `${o.trades.length} ${people(o).length}`);
    check("people over capacity show in red, and the figure counts them", (await p.$$(".wlperson .wc.over")).length === o.over_cells && (await p.textContent(".kpis")).includes(String(o.over_people)) && o.over_people > 0);
  });
  await T("filters and units", async () => {
    await p.click(".ptools label:has-text('Only people over capacity') input"); await W(400);
    const o = await WL();
    check("Only people over capacity keeps those", (await p.$$(".wlperson")).length === o.over_people);
    await p.click(".ptools label:has-text('Only people over capacity') input"); await W(300);
    await p.click(".ptools .seg button:has-text('% of capacity')"); await W(300);
    check("% of capacity: the cells show percentages", (await p.textContent(".wlperson .wc.over >> nth=0")).includes("%"));
    await p.click(".ptools .seg button:has-text('Hours booked')"); await W(300);
    await p.click(".ptools .seg button:has-text('Months')"); await W(700);
    const m = await call("workload", { unit: "month" });
    check("Months: a column per month", (await p.$$(".wl thead th")).length === m.periods.length + 2 && m.periods.length < (await WL()).periods.length);
    await p.click(".ptools .seg button:has-text('Weeks')"); await W(700);
  });
  const o0 = await WL();
  const busy = people(o0).filter(x => x.over.some(Boolean)).sort((a, b) => b.peak - a.peak)[0];
  const task = [...busy.tasks].sort((a, b) => b.total - a.total)[0];
  const other = people(o0).find(x => x.uid !== busy.uid && !x.tasks.some(t => t.uid === task.uid));
  const rowOf = async name => (await p.$$eval(".wlperson .wln", (cs, n) => cs.findIndex(c => c.textContent.includes(n)), name));
  await T("hand a task over", async () => {
    await p.click(`.wlperson >> nth=${await rowOf(busy.name)} >> .wltog`); await W(300);
    check("a person opens to the tasks that make up their hours", (await p.$$(".wltask")).length === busy.tasks.length);
    const src = p.locator(".wltask", { hasText: task.name }).first();
    const dst = p.locator(".wlperson").nth(await rowOf(other.name)).locator("td.wc").nth(1);
    await src.dragTo(dst); await W(800);
    const o = await WL(), b2 = people(o).find(x => x.uid === busy.uid), o2 = people(o).find(x => x.uid === other.uid);
    check(`dragged onto ${other.name}, the task is theirs and ${busy.name} has less work`, !b2.tasks.some(t => t.uid === task.uid) && o2.tasks.some(t => t.uid === task.uid) && b2.booked.reduce((a, x) => a + x, 0) < busy.booked.reduce((a, x) => a + x, 0), JSON.stringify([b2.tasks.map(t => t.uid), o2.tasks.map(t => t.uid)]));
  });
  await T("start it later", async () => {
    const o = await WL(), me = people(o).find(x => x.uid === other.uid), t = me.tasks.find(x => x.uid === task.uid);
    const first = t.hours.findIndex(h => h > 0), later = Math.min(first + 3, o.periods.length - 1);
    await p.click(`.wlperson >> nth=${await rowOf(other.name)} >> .wltog`); await W(300);
    const src = p.locator(".wltask", { hasText: task.name }).first();
    const dst = p.locator(".wlperson").nth(await rowOf(other.name)).locator("td.wc").nth(later);
    await src.dragTo(dst); await W(800);
    const tk = (await S()).tasks.find(x => x.uid === task.uid);
    const o2 = await WL(), t2 = people(o2).find(x => x.uid === other.uid).tasks.find(x => x.uid === task.uid);
    check("dragged to a later week of the same person, it starts no earlier than that week", tk.constraint === "SNET" && t2.hours.findIndex(h => h > 0) >= later, JSON.stringify([tk.constraint, tk.cdate, o.periods[later], t2.hours.findIndex(h => h > 0), later]));
  });
  if (process.env.SHOT) { await p.screenshot({ path: process.env.SHOT }); }

  check("no page errors", errors.length === 0, errors.slice(0, 3).join(" | "));
  console.log(`\n${passed} passed, ${failed} failed`);
  await b.close(); srv.close(); br.kill();
  process.exit(failed ? 1 : 0);
})();
