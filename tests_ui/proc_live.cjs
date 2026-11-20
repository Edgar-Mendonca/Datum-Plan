// Live Procurement Register tests: the built window (ui/dist) in Chromium, with every change answered by
// the real engine (crates/core/examples/sheetbridge.rs), so each check is what a planner would see.
// Run: npm run test:proc (builds the window and the bridge first).
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
  await call("apply", { op: { Modules: { on: ["procurement"] } } });
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
      if (c === "procurement" || c === "documents") return bridge(c, a);
      if (c === "set_options") { m.options = a.opts; return null; }
      if (c in m) return m[c];
      return null;
    } }, event: { listen: async () => () => {} } };
  }, [fix, port]);
  await p.goto(`http://localhost:${srv.address().port}/`); await p.waitForTimeout(700);
  await p.click(".bs aside .back"); await p.waitForTimeout(200);
  await p.click('.side button[title="Procurement"]'); await p.waitForTimeout(700);
  await p.click(".regtabs button:text-is('Register')"); await p.waitForTimeout(300);
  const R = async () => (await call("state")).project.procurement;
  const W = ms => p.waitForTimeout(ms);
  const cell = (i, j) => `.rg td[data-rc="${i}:${j}"]`;
  const colOf = async name => p.$$eval(".rg thead th span", (s, n) => s.findIndex(x => x.textContent.trim() === n) + 1, name);
  const typeIn = async (i, j, text, key = "Enter") => { await p.click(cell(i, j)); await W(80); await p.keyboard.type(text); await W(120); await p.keyboard.press(key); await W(450); };
  const paste = async text => { await p.evaluate(t => { const dt = new DataTransfer(); dt.setData("text/plain", t); document.dispatchEvent(new ClipboardEvent("paste", { clipboardData: dt, bubbles: true, cancelable: true })); }, text); await W(600); };
  const T = async (name, fn) => { try { await fn(); } catch (e) { check(name, false, `threw: ${e.message ?? e}`); } };

  await T("typing an item number in the empty row adds an order", async () => {
    await typeIn(0, await colOf("Item"), "2.1", "Tab");
    await p.keyboard.type("Rotor forgings"); await p.keyboard.press("Tab"); await W(400);
    await p.keyboard.type("Forge Co"); await W(200); await p.keyboard.press("Enter"); await W(500);
    const r = await R();
    check("the register is a spreadsheet: an item number, Tab, the order, Tab, a new sub-vendor (added to its list)", r.items.length === 1 && r.items[0].tag === "2.1" && r.items[0].name === "Rotor forgings" && r.items[0].vendor === "Forge Co" && r.lists.find(l => l.field === "vendor").items.some(i => i.value === "Forge Co"), JSON.stringify(r.items));
  });
  await T("paste orders from a workbook", async () => {
    await p.click(cell(1, await colOf("Item"))); await W(100);
    await paste("2.2\tStator forgings\tForge Co\n2.3\tBearings\tBall Works\n2.4\tCouplings\tBall Works");
    const r = await R();
    check("pasting three rows (item, order, sub-vendor) adds three orders", r.items.length === 4 && r.items[3].name === "Couplings" && r.items[2].vendor === "Ball Works", JSON.stringify(r.items.map(x => [x.tag, x.vendor])));
  });
  await T("planned dates typed; the engine works out forecast and status", async () => {
    await typeIn(0, await colOf("PO planned"), "16/11/2026");
    await typeIn(0, await colOf("Delivery planned"), "29 Jan 2027");
    const r = await R(), fc = await p.textContent(cell(0, await colOf("Forecast")));
    check("planned PO and delivery dates typed in any style; the forecast (grey) follows the planned delivery", r.items[0].planned[1] === "2026-11-16" && r.items[0].planned[4] === "2027-01-29" && fc.includes("29 Jan 27"), JSON.stringify([r.items[0].planned, fc]));
  });
  await T("record a milestone for many", async () => {
    await p.click(".rg td.rh >> nth=0"); await p.keyboard.down("Shift"); await p.click(".rg td.rh >> nth=1"); await p.keyboard.up("Shift"); await W(150);
    await p.click(".rgbar button:has-text('Record milestone')"); await W(300);
    await p.selectOption(".dlg label:has-text('Milestone') select", "1"); await p.fill(".dlg label:has-text('Done on') input", "2026-11-18");
    await p.click(".dlg .btn.pri"); await W(600);
    const r = await R(), st = await p.textContent(cell(0, await colOf("Status")));
    check("select two orders, Record milestone (PO placed, 18 Nov): both get it, and the status says PO placed", r.items[0].actual[1] === "2026-11-18" && r.items[1].actual[1] === "2026-11-18" && !r.items[2].actual[1] && st.includes("PO placed"), JSON.stringify([r.items.map(x => x.actual[1]), st]));
  });
  await T("an expediting note for many, and the log", async () => {
    await p.click(".rgbar button:text-is('Note (2)')"); await W(300);
    await p.fill(".dlg textarea", "Vendor confirms the forgings ship in week 5"); await p.fill(".dlg label:has-text('By') input", "EM");
    await p.click(".dlg .btn.pri"); await W(700);
    const r = await R(), rows = await p.$$eval(".rg td.rh", t => t.filter(x => /^\d+$/.test(x.textContent)).length);
    check("Note for two orders adds the note to each, and the expediting log (opened after) lists them", r.items[0].log.length === 1 && r.items[1].log[0].note.includes("week 5") && rows === 2 && (await p.textContent(".regtabs .on")).includes("Expediting"), JSON.stringify([r.items.map(x => x.log.length), rows]));
    const c = await colOf("Item");
    await p.click(cell(2, c)); await p.keyboard.type("2.3"); await W(200);
    const drop = await p.textContent(".rgdrop");
    await p.keyboard.press("Tab"); await W(400);
    await p.click(cell(2, await colOf("Note"))); await p.keyboard.type("RFQ answered"); await p.keyboard.press("Enter"); await W(500);
    const r2 = await R();
    check("in the log's empty row, an item number (offered from the register) starts a note for that order", drop.includes("Bearings") && r2.items[2].log.length === 1 && r2.items[2].log[0].note === "RFQ answered", JSON.stringify([drop, r2.items[2].log]));
  });
  await T("the order card", async () => {
    await p.click(".regtabs button:text-is('Register')"); await W(300);
    await p.dblclick(".rg td.rh >> nth=0"); await W(400);
    const t = await p.textContent(".dcard");
    check("double-click an order: its card shows the milestones and the expediting notes", t.includes("Rotor forgings") && t.includes("EXPEDITING") && t.includes("week 5") && t.includes("PO placed"), t.slice(0, 160));
    await p.click(".dcard button[title='Close']"); await W(200);
  });
  await T("Settings › Lists, Columns, Status words", async () => {
    await p.click(".page button:has-text('Settings…')"); await W(300);
    await p.click(".dlg .stabs2 button:has-text('Lists')"); await W(200);
    const box = ".dlg .dlist:has(h4:has-text('Sub-vendors'))";
    await p.fill(`${box} .li >> nth=0 >> input.v`, "Forge Company"); await p.press(`${box} .li >> nth=0 >> input.v`, "Tab");
    await p.click(".dlg .stabs2 button:has-text('Columns')"); await W(200);
    await p.click(".dlg button:has-text('+ Column')"); await W(100);
    await p.fill(".dlg .tbl tbody tr >> nth=0 >> td >> nth=0 >> input", "Chase from"); await p.press(".dlg .tbl tbody tr >> nth=0 >> td >> nth=0 >> input", "Tab");
    await p.selectOption(".dlg .tbl tbody tr >> nth=0 >> select", "formula");
    await p.fill(".dlg .tbl tbody tr >> nth=0 >> td >> nth=2 >> input", "=[Delivery planned]-14"); await p.press(".dlg .tbl tbody tr >> nth=0 >> td >> nth=2 >> input", "Tab");
    await p.click(".dlg .stabs2 button:has-text('Status words')"); await W(200);
    await p.fill(".dlg tr:has-text('Order placed, manufacturing not started') input", "PO issued"); await p.press(".dlg tr:has-text('Order placed, manufacturing not started') input", "Tab");
    await p.click(".dlg .btn.pri:text-is('Save')"); await W(800);
    const r = await R(), c = await colOf("Chase from"), v = c ? await p.textContent(cell(0, c)) : "", st = await p.textContent(cell(0, await colOf("Status")));
    check("renaming a sub-vendor in Lists renames it in every order", r.items.filter(x => x.vendor === "Forge Company").length === 2, JSON.stringify(r.items.map(x => x.vendor)));
    check("a formula column of your own (=[Delivery planned]-14) shows a date", /15-Jan-27/.test(v), `${c} ${v}`);
    check("the client's own status words show", st.includes("PO issued"), st);
  });
  await T("a Cancelled order is left out", async () => {
    await typeIn(3, await colOf("State"), "Cancelled");
    const st = await p.textContent(cell(3, await colOf("Status")));
    check("an order set to Cancelled stays in the register, struck through and not counted", st.includes("Not counted") && !!(await p.$(".rg tr.off")), st);
  });
  await T("the formula bar", async () => {
    await p.click(cell(1, await colOf("Order / equipment"))); await W(150);
    const v = await p.inputValue(".rgfx input");
    await p.click(".rgfx input"); await p.fill(".rgfx input", "Stator forgings (2 halves)"); await p.keyboard.press("Enter"); await W(500);
    check("the formula bar shows the order's name and keeps a change typed in it", v === "Stator forgings" && (await R()).items[1].name === "Stator forgings (2 halves)", v);
  });
  await T("delete, then delete the register", async () => {
    await p.click(".rg td.rh >> nth=3"); await W(100);
    await p.click(".rgbar button.danger"); await W(500);
    check("Delete removes the selected order", (await R()).items.length === 3);
    await p.click(".page button:has-text('Settings…')"); await W(300);
    await p.click(".dlg .stabs2 button:has-text('Delete register')"); await W(200);
    await p.click(".dlg button:has-text('Delete the whole register')"); await W(300);
    await p.click(".ov:last-child .dlg .btn.pri"); await W(700);
    const r = await R();
    check("Settings › Delete register empties it after asking", !r || !(r.items ?? []).length, String(JSON.stringify(r)).slice(0, 100));
  });

  check("no page errors", errors.length === 0, errors.slice(0, 3).join(" | "));
  console.log(`\n${passed} passed, ${failed} failed`);
  await b.close(); srv.close(); br.kill();
  process.exit(failed ? 1 : 0);
})();
