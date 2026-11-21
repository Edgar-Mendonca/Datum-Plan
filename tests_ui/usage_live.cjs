// Live Usage tests (hours typed per day): the built window (ui/dist) in Chromium, with every change answered by
// the real engine (crates/core/examples/sheetbridge.rs), so each check is what a planner would see.
// Run: npm run test:usage (builds the window and the bridge first).
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
      if (c === "procurement" || c === "documents" || c === "contract" || c === "board" || c === "workload" || c === "usage") return bridge(c, a);
      if (c === "set_options") { m.options = a.opts; return null; }
      if (c in m) return m[c];
      return null;
    } }, event: { listen: async () => () => {} } };
  }, [fix, port]);
  await p.goto(`http://localhost:${srv.address().port}/`); await p.waitForTimeout(700);
  await p.click(".bs aside .back"); await p.waitForTimeout(200);
  const W = ms => p.waitForTimeout(ms);
  const T = async (name, fn) => { try { await fn(); } catch (e) { check(name, false, `threw: ${e.message ?? e}`); } };
  const S = async () => (await call("state")).project;
  await p.click('.side button[title="Task Usage"]'); await W(900);
  await T("weeks: read only, with a hint", async () => {
    check("no typing boxes by week", (await p.$$(".wlin")).length === 0);
    check("the hint says to show days", (await p.textContent(".wlwrap")).includes("to type a person's hours"));
  });
  await T("type hours on a day", async () => {
    await p.click('.seg button:has-text("Days")'); await W(900);
    const boxes = await p.$$(".wlin");
    check("by day, a person's cells take typing", boxes.length > 0, String(boxes.length));
    const u = await call("usage", { by: "task", what: "work", unit: "day" });
    const ri = u.rows.findIndex(r => r.of !== null && !r.sum && r.vals.some(v => v > 0));
    const r = u.rows[ri], j = r.vals.findIndex(v => v > 0);
    const row = p.locator(".wltbl tbody tr").nth(ri), box = row.locator("td").nth(2 + j).locator("input");
    await box.fill("1.5"); await box.press("Enter"); await W(900);
    const u2 = await call("usage", { by: "task", what: "work", unit: "day" });
    const r2 = u2.rows[ri];
    check("the engine takes the hours for that day", Math.abs(r2.vals[j] - 1.5) < 1e-9, String(r2.vals[j]));
    check("the person's total changes by the difference", Math.abs(r2.total - (r.total - r.vals[j] + 1.5)) < 1e-6, `${r.total} -> ${r2.total}`);
    const task = (await S()).tasks.find(t => t.uid === r.of);
    check("the typed hours are saved with the assignment", task.res.some(a => a.res === r.uid && Array.isArray(a.daily) && a.daily.length > 0), JSON.stringify(task.res));
    check("the window shows the new value", (await box.inputValue()) === "1.5", await box.inputValue());
  });
  await T("a day after the task is refused", async () => {
    const u = await call("usage", { by: "task", what: "work", unit: "day" });
    const ri = u.rows.findIndex(r => r.of !== null && !r.sum && r.vals.some(v => v > 0));
    const r = u.rows[ri], last = r.vals.length - 1 - [...r.vals].reverse().findIndex(v => v > 0);
    if (last >= r.vals.length - 1) { check("a free day after the task", true); return; }
    const box = p.locator(".wltbl tbody tr").nth(ri).locator("td").nth(2 + last + 1).locator("input");
    await box.fill("3"); await box.press("Enter"); await W(900);
    const u2 = await call("usage", { by: "task", what: "work", unit: "day" });
    check("no hours after the task's finish", u2.rows[ri].vals[last + 1] === 0 && Math.abs(u2.rows[ri].total - r.total) < 1e-9, `${u2.rows[ri].vals[last + 1]} ${u2.rows[ri].total} ${r.total}`);
  });
  await T("Resource Usage too", async () => {
    await p.click('.side button[title="Res. Usage"]'); await W(900);
    const segDay = await p.$('.seg button:has-text("Days")'); if (segDay) { await segDay.click(); await W(900); }
    check("Resource Usage by day takes typing too", (await p.$$(".wlin")).length > 0);
  });
  if (process.env.SHOT) { await p.screenshot({ path: process.env.SHOT }); }

  check("no page errors", errors.length === 0, errors.slice(0, 3).join(" | "));
  console.log(`\n${passed} passed, ${failed} failed`);
  await b.close(); srv.close(); br.kill();
  process.exit(failed ? 1 : 0);
})();
