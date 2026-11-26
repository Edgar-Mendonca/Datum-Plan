// Live Task Board tests: the built window (ui/dist) in Chromium, with every change answered by
// the real engine (crates/core/examples/sheetbridge.rs), so each check is what a planner would see.
// Run: npm run test:board (builds the window and the bridge first).
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
  // The plan with a vendor document that is late for a task, and an order that task needs.
  await call("apply", { op: { Modules: { on: ["documents", "procurement"] } } });
  const s0 = await call("state"), tk = s0.project.tasks, leaf = tk.filter((t, i) => !(tk[i + 1] && tk[i + 1].level > t.level));
  const need = leaf[Math.min(3, leaf.length - 1)].uid;
  await call("apply", { op: { Documents: { r: { ...s0.project.documents, docs: [{ id: 1, number: "V-001", title: "GA drawing", class: 1, task: need, offset: -20, weight: 1, progress: true }] } } } });
  await call("apply", { op: { Procurement: { r: { ...s0.project.procurement, items: [{ id: 1, tag: "2.1", name: "Rotor forgings", vendor: "Forge Co", need, manual_w: 1, planned: [null, null, null, null, null], actual: [null, null, null, null, null] }] } } } });
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
      if (c === "procurement" || c === "documents" || c === "contract" || c === "board") return bridge(c, a);
      if (c === "set_options") { m.options = a.opts; return null; }
      if (c in m) return m[c];
      return null;
    } }, event: { listen: async () => () => {} } };
  }, [fix, port]);
  // The bridge works out every date at 10 Nov 2026 (sheetbridge.rs); the window keeps the same day, so what it stamps "today"
  // agrees with the engine whatever the real date is.
  await p.clock.setFixedTime(new Date(2026, 10, 10, 12));
  await p.goto(`http://localhost:${srv.address().port}/`); await p.waitForTimeout(700);
  await p.click(".bs aside .back"); await p.waitForTimeout(200);
  await p.click('.side button[title="Board"]'); await p.waitForTimeout(900);
  const W = ms => p.waitForTimeout(ms);
  const cell = (i, j) => `.rg td[data-rc="${i}:${j}"]`;
  const colOf = async name => p.$$eval(".rg thead th span", (s, n) => s.findIndex(x => x.textContent.trim() === n) + 1, name);
  const typeIn = async (i, j, text, key = "Enter") => { await p.click(cell(i, j)); await W(80); await p.keyboard.type(text); await W(120); await p.keyboard.press(key); await W(450); };
  const paste = async text => { await p.evaluate(t => { const dt = new DataTransfer(); dt.setData("text/plain", t); document.dispatchEvent(new ClipboardEvent("paste", { clipboardData: dt, bubbles: true, cancelable: true })); }, text); await W(600); };
  const T = async (name, fn) => { try { await fn(); } catch (e) { check(name, false, `threw: ${e.message ?? e}`); } };

  const cols = async () => p.$$eval(".kbhead b", h => h.map(x => x.textContent));
  const inCol = async n => p.$$eval(".kbcol", (cs, n) => cs[n] ? [...cs[n].querySelectorAll(".kc .kcn")].map(x => x.textContent) : [], n);
  const colIx = async name => (await cols()).indexOf(name);
  const dragTo = async (name, colName) => {
    const src = p.locator(".kc", { has: p.locator(".kcn", { hasText: name }) }).first();
    const dst = p.locator(".kbcol").nth(await colIx(colName));
    await src.dragTo(dst, { targetPosition: { x: 60, y: 30 } }); await W(700);
  };
  const S = async () => (await call("state")).project;
  const B = async () => call("board");
  await T("columns and cards", async () => {
    const o = await B();
    check("the board opens on four status columns, the plan's tasks as cards", JSON.stringify(await cols()) === '["Not started","In progress","Blocked","Done"]' && (await p.$$(".kc")).length === o.cards.length && o.cards.length > 3, JSON.stringify(await cols()));
    check("critical tasks have the red edge, and float shows in days", (await p.$$(".kc.crit")).length === o.cards.filter(k => k.crit).length && (await p.$$(".kc .kfloat")).length > 0);
  });
  const name = (await B()).cards.find(k => k.col === "todo" && !k.blocked)?.name ?? "";
  const uid = (await B()).cards.find(k => k.name === name)?.id;
  await T("drag to In progress", async () => {
    await dragTo(name, "In progress");
    const t = (await S()).tasks.find(x => x.uid === uid);
    check("dragged to In progress, the task starts on the day (actual start)", t?.act_start === "2026-11-10" && (await inCol(1)).includes(name), JSON.stringify([t?.act_start, await inCol(1)]));
  });
  await T("drag to Blocked", async () => {
    await dragTo(name, "Blocked");
    await p.fill(".dlg input.asktext", "Waiting for scaffolding"); await p.click(".ov:last-child .dlg .btn.pri"); await W(700);
    check("dragged to Blocked, it asks why and shows the reason on the card", (await S()).board?.blocked?.[`t:${uid}`] === "Waiting for scaffolding" && (await p.textContent(".kbcol >> nth=2")).includes("Waiting for scaffolding"));
  });
  await T("drag to Done", async () => {
    await dragTo(name, "Done");
    const s = await S(), t = s.tasks.find(x => x.uid === uid);
    check("dragged to Done, it finishes on the day and is no longer blocked", t?.act_finish === "2026-11-10" && !s.board?.blocked?.[`t:${uid}`] && (await inCol(3)).includes(name), JSON.stringify([t?.act_finish, s.board]));
  });
  await T("waits for a late document and an order", async () => {
    const k = (await B()).cards.find(x => x.waits.length);
    check("a task waiting for a late vendor document shows it on its card, and is blocked by it", !!k && (await p.textContent(".kbcol >> nth=2")).includes("V-001") && k.blocked.startsWith("Waits for"), JSON.stringify(k));
  });
  await T("lanes, columns by a field, WIP limits, look-ahead", async () => {
    await p.selectOption(".ptools label:has-text('Lanes') select", "phase"); await W(700);
    check("swimlanes by phase", (await p.$$(".kblane")).length > 1, String((await p.$$(".kblane")).length));
    await p.selectOption(".ptools label:has-text('Lanes') select", ""); await W(500);
    await p.click(".ptools button:has-text('WIP limits')"); await W(300);
    await p.fill(".dlg tr:has-text('Not started') input", "2"); await p.click(".dlg .btn.pri"); await W(700);
    check("a WIP limit of 2 turns the Not started column red with its count", !!(await p.$(".kbhead.over")) && (await p.textContent(".kbhead.over")).includes("/ 2"));
    const all = (await p.$$(".kc")).length;
    await p.selectOption(".ptools label:has-text('Look-ahead') select", "1"); await W(700);
    const one = (await p.$$(".kc")).length;
    check("a one-week look-ahead keeps fewer cards (what starts soon, what is under way, what just finished)", one < all && one > 0, `${one} of ${all}`);
    await p.selectOption(".ptools label:has-text('Look-ahead') select", "0"); await W(500);
    await p.selectOption(".ptools label:has-text('Columns') select", "responsible"); await W(700);
    check("columns by a field: one per value", (await cols()).length >= 1 && !(await cols()).includes("Blocked"), JSON.stringify(await cols()));
    await p.selectOption(".ptools label:has-text('Columns') select", "status"); await W(500);
  });
  await T("documents on the same board", async () => {
    await p.click(".ptools .seg button:has-text('Documents')"); await W(700);
    check("the Document Register on the board: To issue, With the client, To resubmit, Blocked, Final", JSON.stringify(await cols()) === '["To issue","With the client","To resubmit","Blocked","Final"]' && (await inCol(0)).length === 1);
    await dragTo("GA drawing", "With the client");
    const s = await S();
    check("dragged to With the client, its next submission is logged (revision A, TR-0001)", s.documents.log.length === 1 && s.documents.log[0].rev === "A" && s.documents.log[0].transmittal === "TR-0001" && (await inCol(1)).length === 1, JSON.stringify(s.documents.log));
  });
  await T("orders on the same board", async () => {
    await p.click(".ptools .seg button:has-text('Orders')"); await W(700);
    await dragTo("Rotor forgings", "Ordered");
    const it = (await S()).procurement.items[0];
    check("an order dragged to Ordered has its RFQ and PO recorded on the day", it.actual[0] === "2026-11-10" && it.actual[1] === "2026-11-10" && !it.actual[2], JSON.stringify(it.actual));
    await p.click(".ptools .seg button:has-text('Tasks')"); await W(500);
  });
  if (process.env.SHOT) { await p.selectOption(".ptools label:has-text('Lanes') select", "phase"); await W(700); await p.screenshot({ path: process.env.SHOT }); }

  check("no page errors", errors.length === 0, errors.slice(0, 3).join(" | "));
  console.log(`\n${passed} passed, ${failed} failed`);
  await b.close(); srv.close(); br.kill();
  process.exit(failed ? 1 : 0);
})();
