// Live Time-Location tests (chainage against time): the built window (ui/dist) in Chromium, with every change answered by
// the real engine (crates/core/examples/sheetbridge.rs), so each check is what a planner would see.
// Run: npm run test:linear (builds the window and the bridge first).
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
  // Chainages on four tasks (a moving crew on three, one place on the fourth).
  { const s0 = await call("state"), leaves = s0.project.tasks.filter((t, k, all) => !(all[k + 1] && all[k + 1].level > t.level) && t.dur > 0).slice(0, 4);
    const at = [["0", "4"], ["4", "9"], ["9", "12+500"], ["6", ""]];
    for (const [k, t] of leaves.entries()) { await call("apply", { op: { Set: { uid: t.uid, field: "locfrom", value: at[k][0] } } }); if (at[k][1]) await call("apply", { op: { Set: { uid: t.uid, field: "locto", value: at[k][1] } } }); } }
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
      if (c === "procurement" || c === "documents" || c === "contract" || c === "board" || c === "workload" || c === "usage" || c === "linear_view") return bridge(c, a);
      if (c === "set_options") { m.options = a.opts; return null; }
      if (c in m) return m[c];
      return null;
    } }, event: { listen: async () => () => {} } };
  }, [fix, port]);
  await p.goto(`http://localhost:${srv.address().port}/`); await p.waitForTimeout(700);
  await p.click(".bs aside .back"); await p.waitForTimeout(200);
  const W = ms => p.waitForTimeout(ms);
  const T = async (name, fn) => { try { await fn(); } catch (e) { check(name, false, `threw: ${e.message ?? e}`); } };
  await p.click('.side button[title="Location"]'); await W(1200);
  await T("the view", async () => {
    const d = await call("linear_view", {});
    check("four tasks with a chainage", d.items.length === 4 && d.lo === 0 && d.hi === 12.5, JSON.stringify([d.items.length, d.lo, d.hi]));
    check("three moving lines and one bar in one place", (await p.$$("line.linline")).length === 3 && (await p.$$(".linear rect.tsrect")).length === 1);
    check("the rest are counted as not shown", (await p.textContent(".tslegend")).includes(`${d.missing} tasks without a chainage`) && d.missing > 0);
    check("the legend is above the diagram", await p.evaluate(() => { const l = document.querySelector(".tslegend"), w = document.querySelector(".tsldwrap"); return !!l && !!w && l.getBoundingClientRect().bottom <= w.getBoundingClientRect().top; }));
    check("the chainage unit shows", (await p.textContent("svg.linear")).includes("Chainage (km)"));
  });
  await T("scale", async () => {
    const h1 = +(await p.getAttribute("svg.linear", "height"));
    await p.locator(".seg button:visible", { hasText: "Days" }).click(); await W(600);
    check("Days is taller than Weeks", +(await p.getAttribute("svg.linear", "height")) > h1 * 2);
    await p.locator(".seg button:visible", { hasText: "Weeks" }).click(); await W(500);
  });
  if (process.env.SHOT) { await p.screenshot({ path: process.env.SHOT }); }
  await T("double-click", async () => {
    await p.locator("line.linline").first().dblclick({ force: true }); await W(800);
    check("double-clicking shows the Gantt chart", (await p.$$("svg.linear")).length === 0);
  });

  check("no page errors", errors.length === 0, errors.slice(0, 3).join(" | "));
  console.log(`\n${passed} passed, ${failed} failed`);
  await b.close(); srv.close(); br.kill();
  process.exit(failed ? 1 : 0);
})();
