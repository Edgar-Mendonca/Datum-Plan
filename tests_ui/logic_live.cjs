// Live Logic tests (the time-scaled logic diagram): the built window (ui/dist) in Chromium, with every change answered by
// the real engine (crates/core/examples/sheetbridge.rs), so each check is what a planner would see.
// Run: npm run test:logic (builds the window and the bridge first).
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
      if (c === "procurement" || c === "documents" || c === "contract" || c === "board" || c === "workload" || c === "usage" || c === "logic_diagram") return bridge(c, a);
      if (c === "set_options") { m.options = a.opts; return null; }
      if (c in m) return m[c];
      return null;
    } }, event: { listen: async () => () => {} } };
  }, [fix, port]);
  await p.goto(`http://localhost:${srv.address().port}/`); await p.waitForTimeout(700);
  await p.click(".bs aside .back"); await p.waitForTimeout(200);
  const W = ms => p.waitForTimeout(ms);
  const T = async (name, fn) => { try { await fn(); } catch (e) { check(name, false, `threw: ${e.message ?? e}`); } };
  await p.click('.side button[title="Logic"]'); await W(1200);
  await T("the diagram", async () => {
    const d = await call("logic_diagram", { phase: true, gap: Math.round(1440 * 8 / 12), per: Math.round(1440 * 6.2 / 12) });
    check("a bar per task", (await p.$$(".tsbar")).length === d.bars.length && d.bars.length > 0, `${d.bars.length}`);
    check("a line per link", (await p.$$(".tslink")).length === d.links.length, `${d.links.length}`);
    check("a band per phase", (await p.$$(".tsband")).length === d.bands.length && d.bands.length > 1, `${d.bands.length}`);
    check("the legend is above the diagram, as in the Network Diagram (issue #5)", await p.evaluate(() => { const l = document.querySelector(".tslegend"), w = document.querySelector(".tsldwrap"); return !!l && !!w && l.getBoundingClientRect().bottom <= w.getBoundingClientRect().top; }));
    check("critical work in red", (await p.$$(".tsrect.crit, .tsms.crit")).length === d.bars.filter(b => b.critical).length);
    check("fewer lanes than tasks", d.lanes < d.bars.length, `${d.lanes} lanes for ${d.bars.length} tasks`);
  });
  await T("one band", async () => {
    await p.locator('.seg button:visible', { hasText: "One band" }).click(); await W(900);
    check("one band when chosen", (await p.$$(".tsband")).length === 1);
  });
  await T("zoom", async () => {
    const w1 = +(await p.getAttribute("svg.tsld", "width"));
    await p.locator('.seg button:visible', { hasText: "Days" }).click(); await W(700);
    const w2 = +(await p.getAttribute("svg.tsld", "width"));
    check("Days is wider than Weeks", w2 > w1 * 2, `${w1} ${w2}`);
    await p.locator('.seg button:visible', { hasText: "Weeks" }).click(); await W(500);
    await p.locator('.seg button:visible', { hasText: "Bands by phase" }).click(); await W(800);
  });
  // Issue #7: large plans on a laptop screen: Fit, a ruler that stays at the top, drag to move, Ctrl+wheel to zoom.
  await T("large plans", async () => {
    await p.setViewportSize({ width: 1366, height: 600 }); await W(300);
    const wrap = () => p.$eval(".tsldwrap", e => ({ w: e.clientWidth, sl: e.scrollLeft, st: e.scrollTop, top: e.getBoundingClientRect().top }));
    await p.locator(".page .btn", { hasText: "Fit" }).click(); await W(900);
    check("Fit puts the whole plan across the window", +(await p.getAttribute("svg.tsld", "width")) <= (await wrap()).w + 1, `${await p.getAttribute("svg.tsld", "width")} ${(await wrap()).w}`);
    await p.$eval(".tsldwrap", e => { e.scrollTop = 150; }); await W(150);
    const hd = await p.$eval("svg.tsldhead", e => e.getBoundingClientRect().top), wr = await wrap();
    check("the date ruler stays at the top while scrolling down", wr.st > 0 && Math.abs(hd - wr.top - 1) <= 1, `${hd} ${wr.top} ${wr.st}`);
    await p.locator('.seg button:visible', { hasText: "Days" }).click(); await W(900);
    const b = await p.$eval(".tsldwrap", e => e.getBoundingClientRect());
    await p.mouse.move(b.x + b.width - 40, b.y + 60); await p.mouse.down(); await p.mouse.move(b.x + b.width - 340, b.y + 60, { steps: 5 }); await p.mouse.up(); await W(100);
    check("dragging the background moves the view", (await wrap()).sl >= 250, `${(await wrap()).sl}`);
    const w0 = +(await p.getAttribute("svg.tsld", "width"));
    await p.mouse.move(b.x + 900, b.y + 100); await p.keyboard.down("Control"); await p.mouse.wheel(0, 120); await p.keyboard.up("Control"); await W(900);
    check("Ctrl+wheel zooms out", +(await p.getAttribute("svg.tsld", "width")) < w0 * 0.9, `${w0} ${await p.getAttribute("svg.tsld", "width")} ${await p.evaluate(() => localStorage.getItem("tsld-zoom"))} ${JSON.stringify(b)}`);
    await p.locator('.seg button:visible', { hasText: "Weeks" }).click(); await W(700);
    await p.setViewportSize({ width: 1500, height: 950 }); await W(200);
  });
  if (process.env.SHOT) { await p.screenshot({ path: process.env.SHOT }); }
  await T("double-click", async () => {
    await p.locator(".tsbar rect, .tsbar path").first().dblclick({ force: true }); await W(800);
    check("double-clicking a bar shows the Gantt chart", (await p.$$(".tsld")).length === 0);
  });

  check("no page errors", errors.length === 0, errors.slice(0, 3).join(" | "));
  console.log(`\n${passed} passed, ${failed} failed`);
  await b.close(); srv.close(); br.kill();
  process.exit(failed ? 1 : 0);
})();
