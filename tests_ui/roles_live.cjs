// Live Roles tests (a role and Assign People): the built window (ui/dist) in Chromium, with every change answered by
// the real engine (crates/core/examples/sheetbridge.rs), so each check is what a planner would see.
// Run: npm run test:roles (builds the window and the bridge first).
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
  // The resource on the most tasks becomes a role (a generic resource) before the window opens.
  { const s0 = await call("state"), n = new Map(); s0.project.tasks.forEach(t => (t.res ?? []).forEach(a => n.set(a.res, (n.get(a.res) ?? 0) + 1)));
    const work = s0.project.resources.filter(r => r.kind === "Work"), role = [...work].sort((a, b) => (n.get(b.uid) ?? 0) - (n.get(a.uid) ?? 0))[0];
    globalThis.ROLE = role; await call("apply", { op: { ResSet: { uid: role.uid, field: "role", value: "Yes" } } }); }
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
      if (c === "procurement" || c === "documents" || c === "contract" || c === "board" || c === "workload" || c === "usage" || c === "role_candidates") return bridge(c, a);
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
  const role = globalThis.ROLE;
  await p.click('.side button[title="Resources"]'); await W(900);
  await T("the Role column", async () => {
    const heads = await p.$$eval(".sheet thead th", hs => hs.map(h => h.textContent.trim()));
    check("the Resource Sheet has a Role column", heads.includes("Role"), heads.join("|"));
    check("the role shows an Assign people button", (await p.$$('button:has-text("Assign people")')).length === 1);
  });
  let before;
  await T("the dialog", async () => {
    before = await call("role_candidates", { role: role.uid, uids: [] });
    await p.click('button:has-text("Assign people")'); await W(900);
    check("it lists the role's tasks", (await p.$$(".dlg tbody tr input[type=checkbox], dialog tbody tr input[type=checkbox], .dbody tbody tr input[type=checkbox]")).length === before.tasks.length && before.tasks.length > 0, `${before.tasks.length}`);
    check("and the people who could take them", (await p.$$('.dbody button:has-text("Give to")')).length === before.people.length && before.people.length > 0, `${before.people.length}`);
  });
  await T("give the work", async () => {
    const first = before.tasks[0].uid;
    await p.locator(".dbody tbody tr").first().locator("input[type=checkbox]").uncheck(); await W(700);
    const person = before.people[0];
    await p.locator(`.dbody button:has-text("Give to ${person.name}")`).first().click(); await W(900);
    const pr = await S();
    const has = (uid, r) => pr.tasks.find(t => t.uid === uid).res.some(a => a.res === r);
    check("the unticked task keeps the role", has(first, role.uid));
    check("the other tasks go to the person", before.tasks.slice(1).every(t => has(t.uid, person.uid) && !has(t.uid, role.uid)), JSON.stringify(before.tasks.map(t => t.uid)));
  });
  if (process.env.SHOT) { await p.screenshot({ path: process.env.SHOT }); }

  check("no page errors", errors.length === 0, errors.slice(0, 3).join(" | "));
  console.log(`\n${passed} passed, ${failed} failed`);
  await b.close(); srv.close(); br.kill();
  process.exit(failed ? 1 : 0);
})();
