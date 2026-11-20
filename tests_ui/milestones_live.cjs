// Live Milestones tests: the built window (ui/dist) in Chromium, with every change answered by
// the real engine (crates/core/examples/sheetbridge.rs), so each check is what a planner would see.
// Run: npm run test:milestones (builds the window and the bridge first).
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
  // Baseline 0, a period close, then the first task takes 10 days longer and baseline 1 is saved.
  await call("apply", { op: { Baseline: { uids: [], clear: false, n: 0 } } });
  await call("apply", { op: { Snapshot: { label: "Week 45", comment: "" } } });
  const s0 = await call("state"), tk = s0.project.tasks;
  const first = tk.find((t, i) => t.dur > 0 && !(tk[i + 1] && tk[i + 1].level > t.level));
  await call("apply", { op: { Set: { uid: first.uid, field: "dur", value: "20d" } } });
  await call("apply", { op: { Baseline: { uids: [], clear: false, n: 1 } } });
  const lastMs = [...tk].reverse().find(t => t.dur === 0);
  await call("apply", { op: { Set: { uid: lastMs.uid, field: "deadline", value: "01-Nov-26" } } });
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
      if (c === "procurement" || c === "documents" || c === "contract" || c === "board" || c === "milestones") return bridge(c, a);
      if (c === "set_options") { m.options = a.opts; return null; }
      if (c in m) return m[c];
      return null;
    } }, event: { listen: async () => () => {} } };
  }, [fix, port]);
  await p.goto(`http://localhost:${srv.address().port}/`); await p.waitForTimeout(700);
  await p.click(".bs aside .back"); await p.waitForTimeout(200);
  await p.click('.side button[title="Milestones"]'); await p.waitForTimeout(900);
  const W = ms => p.waitForTimeout(ms);
  const cell = (i, j) => `.rg td[data-rc="${i}:${j}"]`;
  const colOf = async name => p.$$eval(".rg thead th span", (s, n) => s.findIndex(x => x.textContent.trim() === n) + 1, name);
  const typeIn = async (i, j, text, key = "Enter") => { await p.click(cell(i, j)); await W(80); await p.keyboard.type(text); await W(120); await p.keyboard.press(key); await W(450); };
  const paste = async text => { await p.evaluate(t => { const dt = new DataTransfer(); dt.setData("text/plain", t); document.dispatchEvent(new ClipboardEvent("paste", { clipboardData: dt, bubbles: true, cancelable: true })); }, text); await W(600); };
  const T = async (name, fn) => { try { await fn(); } catch (e) { check(name, false, `threw: ${e.message ?? e}`); } };

  const M = async (base = 0) => call("milestones", { base });
  await T("the table", async () => {
    const o = await M();
    const heads = await p.$$eval(".mstable thead th", t => t.map(x => x.textContent.trim()));
    check("every milestone, a column per baseline (Baseline, Baseline 1)", (await p.$$(".mstable tbody tr")).length === o.rows.length && o.rows.length >= 2 && heads.includes("Baseline") && heads.includes("Baseline 1"), JSON.stringify(heads));
    const late = o.rows.find(r => r.status === "Late" && (r.var ?? 0) > 0);
    check("a milestone pushed by the longer task is late against the baseline and slipping since Week 45", !!late && late.moved > 0 && (await p.textContent(".mstable tbody")).includes(`+${late.var}`), JSON.stringify(late));
    check("the KPIs count the late milestones", (await p.textContent(".kpis")).includes(String(o.late)) && o.late >= 1);
    check("a deadline before the forecast makes the last milestone late and shows in red", (await p.$$(".mstable td.late")).length > 0);
  });
  await T("compare with another baseline", async () => {
    await p.selectOption(".ptools label:has-text('Compare with') select", "1"); await W(700);
    const o = await M(1);
    const r = o.rows.find(x => x.baselines.length === 2 && x.baselines[0][1] !== x.baselines[1][1]);
    check("compared with Baseline 1, the milestones that moved are on time again", !!r && r.var === 0 && (await p.textContent(".mstable th.on")).includes("Baseline 1"), JSON.stringify(r));
    await p.selectOption(".ptools label:has-text('Compare with') select", "0"); await W(500);
  });
  await T("filters", async () => {
    const all = (await p.$$(".mstable tbody tr")).length;
    await p.click(".ptools .seg button:has-text('Late or slipping')"); await W(400);
    const some = (await p.$$(".mstable tbody tr")).length;
    check("Late or slipping keeps only those", some > 0 && some <= all, `${some} of ${all}`);
    await p.click(".ptools .seg button:has-text('All')"); await W(300);
  });
  await T("the chart", async () => {
    const o = await M();
    check("the chart: a row per milestone, a diamond per baseline, the forecast and the status date", (await p.$$(".msrow")).length === o.rows.length && (await p.$$(".msbl")).length === o.rows.reduce((a, r) => a + r.baselines.length, 0) && (await p.$$(".msdia")).length >= o.rows.length && !!(await p.$(".mstodayl")));
    check("late ones have a red forecast and a red slip line", (await p.$$(".mslane .msdia.late")).length >= 1 && (await p.$$(".msslip.late")).length >= 1);
  });
  await T("open in the Gantt", async () => {
    await p.dblclick(".mstable tbody tr >> nth=0"); await W(500);
    check("double-click shows the milestone in the Gantt chart", !!(await p.$('.side button.on[title="Gantt"]')));
  });
  if (process.env.SHOT) { await p.click('.side button[title="Milestones"]'); await W(700); await p.screenshot({ path: process.env.SHOT }); }

  check("no page errors", errors.length === 0, errors.slice(0, 3).join(" | "));
  console.log(`\n${passed} passed, ${failed} failed`);
  await b.close(); srv.close(); br.kill();
  process.exit(failed ? 1 : 0);
})();
