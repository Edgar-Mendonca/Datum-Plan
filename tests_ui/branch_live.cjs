// Live Branch tests (a risk's branch in the Risk Register): the built window (ui/dist) in Chromium, with every change answered by
// the real engine (crates/core/examples/sheetbridge.rs), so each check is what a planner would see.
// Run: npm run test:branch (builds the window and the bridge first).
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
  // A risk to give a branch to, before the window opens.
  { const s0 = await call("state"); globalThis.T1 = s0.project.tasks.find(t => !s0.project.tasks.some(x => x.level > t.level) && t.dur > 0);
    await call("apply", { op: { Risk: { id: 1, risk: { id: 1, name: "Weld test fails", cause: "", prob: 3, impact: 3, owner: "", response: "", status: "Open", tasks: [], kind: "Threat", days: 0 } } } }); }
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
  await p.click('.side button[title="Risks"]'); await W(900);
  await T("the Branch button", async () => {
    check("each risk has a Branch button", (await p.$$('.riskreg button:has-text("Branch")')).length === 1);
  });
  await T("edit the branch", async () => {
    await p.click('.riskreg button:has-text("Branch")'); await W(700);
    await p.click('button:has-text("+ Add activity")'); await W(200);
    await p.click('button:has-text("+ Add activity")'); await W(200);
    const rows = p.locator(".ov table tbody tr").filter({ has: p.locator("input") });
    check("two activities to fill in", (await rows.count()) >= 2, String(await rows.count()));
    await p.locator(".ov select").first().selectOption(String(globalThis.T1.uid)); await W(150);
    await p.click('button:has-text("Save")'); await W(900);
    const r = (await S()).risks.find(x => x.id === 1);
    check("the branch is saved with the risk", r.branch?.length === 2 && r.after === globalThis.T1.uid && (r.before ?? 0) === 0, JSON.stringify(r));
    check("the button counts the activities", (await p.textContent('.riskreg')).includes("Branch: 2"));
  });
  await T("without a task to follow it is refused", async () => {
    await p.click('.riskreg button:has-text("Branch")'); await W(700);
    await p.locator(".ov select").first().selectOption("0"); await W(150);
    await p.click('button:has-text("Save")'); await W(700);
    const r = (await S()).risks.find(x => x.id === 1);
    check("the saved branch is unchanged", r.after === globalThis.T1.uid && r.branch.length === 2);
    await p.keyboard.press("Escape"); await W(300);
  });
  if (process.env.SHOT) { await p.screenshot({ path: process.env.SHOT }); }

  check("no page errors", errors.length === 0, errors.slice(0, 3).join(" | "));
  console.log(`\n${passed} passed, ${failed} failed`);
  await b.close(); srv.close(); br.kill();
  process.exit(failed ? 1 : 0);
})();
