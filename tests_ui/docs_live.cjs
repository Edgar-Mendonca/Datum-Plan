// Live Document Register tests: the built window (ui/dist) in Chromium, with every change answered by
// the real engine (crates/core/examples/sheetbridge.rs), so each check is what a planner would see.
// Run: npm run test:docs (builds the window and the bridge first).
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
  await call("apply", { op: { Modules: { on: ["documents"] } } });
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
      if (c === "documents") return bridge(c, a);
      if (c === "set_options") { m.options = a.opts; return null; }
      if (c in m) return m[c];
      return null;
    } }, event: { listen: async () => () => {} } };
  }, [fix, port]);
  await p.goto(`http://localhost:${srv.address().port}/`); await p.waitForTimeout(700);
  await p.click(".bs aside .back"); await p.waitForTimeout(200);
  await p.click('.side button[title="Documents"]'); await p.waitForTimeout(700);
  const R = async () => (await call("state")).project.documents;
  const W = ms => p.waitForTimeout(ms);
  /** A cell of the grid on screen (row i from 0, column j from 1). */
  const cell = (i, j) => `.rg td[data-rc="${i}:${j}"]`;
  const colOf = async name => p.$$eval(".rg thead th span", (s, n) => s.findIndex(x => x.textContent.trim() === n) + 1, name);
  const typeIn = async (i, j, text, key = "Enter") => { await p.click(cell(i, j)); await W(80); await p.keyboard.type(text); await W(120); await p.keyboard.press(key); await W(450); };
  const paste = async text => { await p.evaluate(t => { const dt = new DataTransfer(); dt.setData("text/plain", t); document.dispatchEvent(new ClipboardEvent("paste", { clipboardData: dt, bubbles: true, cancelable: true })); }, text); await W(600); };
  const T = async (name, fn) => { try { await fn(); } catch (e) { check(name, false, `threw: ${e.message ?? e}`); } };

  await T("the register is a spreadsheet: typing a number in the empty row adds a document", async () => {
    await typeIn(0, await colOf("Document No."), "DOC-001");
    const r = await R();
    check("the register is a spreadsheet: typing a number in the empty row adds a document", r.docs.length === 1 && r.docs[0].number === "DOC-001", JSON.stringify(r.docs));
  });
  await T("Tab moves right; a title and a vendor typed in", async () => {
    await typeIn(0, await colOf("Title"), "General arrangement", "Tab");
    await p.keyboard.type("Northwind"); await W(200);
    const drop = await p.textContent(".rgdrop").catch(() => "");
    await p.keyboard.press("Enter"); await W(450);
    const r = await R();
    check("Tab moves right; a vendor not in the list is offered as new, kept and added to the Vendors list", r.docs[0].title === "General arrangement" && r.docs[0].vendor === "Northwind" && drop.includes("new") && r.lists.find(l => l.field === "vendor").items.some(i => i.value === "Northwind"), JSON.stringify([r.docs[0], drop]));
  });
  await T("a list cell: typing part of a value and Enter picks it", async () => {
    await typeIn(0, await colOf("State"), "hol");
    const r = await R();
    check("a list cell: typing part of a value (hol) and Enter picks it from the list (Hold)", r.docs[0].state === "Hold", r.docs[0].state);
    await typeIn(0, await colOf("State"), "Active");
  });
  await T("paste rows from a workbook", async () => {
    await p.click(cell(1, await colOf("Document No."))); await W(100);
    await paste("DOC-002\tCompressor datasheet\tNorthwind\nDOC-003\tLube oil P&ID\tPacific\nDOC-004\tNoise study\tPacific");
    const r = await R();
    check("pasting three rows (number, title, vendor) from a workbook adds three documents", r.docs.length === 4 && r.docs[3].number === "DOC-004" && r.docs[2].vendor === "Pacific", JSON.stringify(r.docs.map(d => [d.number, d.vendor])));
  });
  await T("P + n or a date in one column; the engine plans it", async () => {
    const c = await colOf("P +");
    await typeIn(0, c, "2"); await typeIn(1, c, "4"); await typeIn(2, c, "18/01/2027");
    const r = await R(), planned = await p.textContent(cell(0, await colOf("1st planned")));
    check("P + n (a number) or a first-issue date (18/01/2027) go in the same column; the 1st planned date is worked out (grey)", r.docs[0].offset === 2 && r.docs[2].first === "2027-01-18" && r.docs[2].offset === null && /\d{2} \w{3} \d{2}/.test(planned), JSON.stringify([r.docs[0].offset, r.docs[2], planned]));
  });
  await T("fill down", async () => {
    const c = await colOf("Disc.");
    await typeIn(0, c, "MR");
    await p.click(cell(0, c)); await p.keyboard.down("Shift"); await p.click(cell(3, c)); await p.keyboard.up("Shift");
    await p.keyboard.press("Control+d"); await W(500);
    const r = await R();
    check("Ctrl+D fills the discipline down four rows", r.docs.every(d => d.disc === "MR"), JSON.stringify(r.docs.map(d => d.disc)));
  });
  await T("the formula bar shows and changes the cell", async () => {
    const c = await colOf("Title");
    await p.click(cell(2, c)); await W(150);
    const shown = await p.inputValue(".rgfx input"), where = await p.textContent(".rgfx .rgwhere");
    await p.click(".rgfx input"); await p.fill(".rgfx input", "Lube oil system P&ID, all skids"); await p.keyboard.press("Enter"); await W(500);
    const r = await R();
    check("the formula bar shows the selected cell whole (Title · row 3) and Enter keeps a change typed in it", shown === "Lube oil P&ID" && where.includes("Title") && where.includes("row 3") && r.docs[2].title === "Lube oil system P&ID, all skids", JSON.stringify([shown, where, r.docs[2].title]));
    await p.click(cell(0, await colOf("Status"))); await W(150);
    check("…on a worked-out cell it shows the value, read-only", (await p.inputValue(".rgfx input")).length > 0 && (await p.$eval(".rgfx input", i => i.readOnly)));
  });
  await T("a calculated cell cannot be typed over", async () => {
    await p.click(cell(0, await colOf("Status"))); await p.keyboard.type("x"); await W(200);
    check("a grey (worked-out) cell cannot be typed in", !(await p.$(".rg .rgin")));
  });
  await T("log one transmittal for three selected documents", async () => {
    await p.click(".rg td.rh >> nth=0"); await p.keyboard.down("Shift"); await p.click(".rg td.rh >> nth=2"); await p.keyboard.up("Shift"); await W(150);
    const label = await p.textContent(".rgbar button:has-text('Log submission')");
    await p.click(".rgbar button:has-text('Log submission')"); await W(300);
    const tr = await p.inputValue(".dlg label:has-text('Transmittal no.') input");
    await p.click(".dlg .btn.pri"); await W(600);
    const r = await R();
    check("select three rows, Log submission (3): one transmittal (TR-0001), a row each with revision A and the first-issue purpose", label.includes("(3)") && tr === "TR-0001" && r.log.length === 3 && r.log.every(s => s.transmittal === "TR-0001" && s.rev === "A" && s.purpose === "ICR"), JSON.stringify([label, tr, r.log]));
  });
  await T("log a return", async () => {
    await p.click(".dtabs button:has-text('Register')"); await W(300);
    await p.click(".rg td.rh >> nth=1"); await W(100);
    await p.click(".rgbar button:has-text('Log return')"); await W(300);
    await p.selectOption(".dlg label:has-text('Code') select", "Code 3"); await p.click(".dlg .btn.pri"); await W(600);
    const r = await R(), st = await p.textContent(cell(1, await colOf("Status")));
    check("Log return puts the date and code on the open submission; Code 3 makes it Resubmit", r.log.find(s => s.doc === r.docs[1].id).code === "Code 3" && /Resubmit/.test(st), JSON.stringify([r.log, st]));
  });
  await T("the transmittal log fills itself from the document number", async () => {
    await p.click(".dtabs button:has-text('Transmittal log')"); await W(400);
    const n = (await R()).log.length, c = await colOf("Document No.");
    await p.click(cell(n, c)); await p.keyboard.type("DOC-002"); await W(200);
    const drop = await p.textContent(".rgdrop");
    await p.keyboard.press("Enter"); await W(600);
    const r = await R(), s = r.log[r.log.length - 1];
    const title = await p.textContent(cell(n, await colOf("Title")));
    check("typing a document number in the log's empty row offers it from the register, then fills title, class and vendor, and proposes its next revision (B) and purpose", drop.includes("Compressor datasheet") && r.log.length === n + 1 && s.rev === "B" && s.purpose === "ICR" && title.includes("Compressor datasheet"), JSON.stringify([drop, s, title]));
    await p.click(cell(n + 1, c)); await p.keyboard.type("NOPE-9"); await p.keyboard.press("Enter"); await W(400);
    check("…a number that is not in the register is refused", (await R()).log.length === n + 1);
    await p.keyboard.press("Escape"); await W(100);
  });
  await T("the transmittals list", async () => {
    await p.click(".dtabs button:has-text('Transmittals')"); await W(300);
    check("Transmittals lists each transmittal number with its documents", (await p.textContent(".page")).includes("TR-0001") && (await p.$$(".page .tbl tbody tr")).length >= 1);
  });
  await T("the document card", async () => {
    await p.click(".dtabs button:has-text('Register')"); await W(300);
    await p.dblclick(".rg td.rh >> nth=1"); await W(400);
    const t = await p.textContent(".dcard");
    check("double-click a row: the card shows its steps and history (sent, returned Code 3, next issue)", t.includes("HISTORY") && t.includes("Returned Code 3") && t.includes("Compressor datasheet"), t.slice(0, 200));
    await p.click(".dcard button[title='Close']"); await W(200);
  });
  await T("Settings › Lists: rename a vendor everywhere", async () => {
    await p.click(".page button:has-text('Settings…')"); await W(300);
    await p.click(".dlg .stabs2 button:has-text('Lists')"); await W(200);
    const box = ".dlg .dlist:has(h4:has-text('Vendors'))";
    await p.fill(`${box} .li >> nth=0 >> input.v`, "Northwind Castings"); await p.press(`${box} .li >> nth=0 >> input.v`, "Tab");
    await p.click(".dlg .btn.pri:text-is('Save')"); await W(600);
    const r = await R();
    check("renaming a vendor in Settings › Lists renames it in every document", r.docs.filter(d => d.vendor === "Northwind Castings").length === 2 && !r.docs.some(d => d.vendor === "Northwind"), JSON.stringify(r.docs.map(d => d.vendor)));
  });
  await T("Settings › Columns: a formula column of your own", async () => {
    await p.click(".page button:has-text('Settings…')"); await W(300);
    await p.click(".dlg .stabs2 button:has-text('Columns')"); await W(200);
    await p.click(".dlg button:has-text('+ Column')"); await W(100);
    await p.fill(".dlg .tbl tbody tr >> nth=0 >> td >> nth=0 >> input", "Chase on"); await p.press(".dlg .tbl tbody tr >> nth=0 >> td >> nth=0 >> input", "Tab");
    await p.selectOption(".dlg .tbl tbody tr >> nth=0 >> select", "formula");
    await p.fill(".dlg .tbl tbody tr >> nth=0 >> td >> nth=2 >> input", "=[Review due]+2"); await p.press(".dlg .tbl tbody tr >> nth=0 >> td >> nth=2 >> input", "Tab");
    await p.click(".dlg .btn.pri:text-is('Save')"); await W(700);
    const c = await colOf("Chase on"), v = c ? await p.textContent(cell(0, c)) : "";
    check("a formula column (=[Review due]+2) shows a date per document", c > 0 && /\d{2}-\w{3}-\d{2}/.test(v), `${c} ${v}`);
  });
  await T("Settings › Status words", async () => {
    await p.click(".page button:has-text('Settings…')"); await W(300);
    await p.click(".dlg .stabs2 button:has-text('Status words')"); await W(200);
    await p.fill(".dlg tr:has-text('With the client, within the review period') input", "Under Company Review"); await p.press(".dlg tr:has-text('With the client, within the review period') input", "Tab");
    await p.click(".dlg .btn.pri:text-is('Save')"); await W(600);
    const st = await p.textContent(cell(1, await colOf("Status"))), r = await R();
    check("the client's own status words show in the register", st.includes("Under Company Review"), JSON.stringify([st, r.words, await colOf("Status")]));
  });
  await T("a Deleted state leaves a document out", async () => {
    await typeIn(3, await colOf("State"), "Deleted");
    const st = await p.textContent(cell(3, await colOf("Status")));
    check("a document set to Deleted stays in the register, struck through and not counted", st.includes("Not counted") && !!(await p.$(".rg tr.off")), st);
  });
  await T("delete selected rows", async () => {
    await p.click(".rg td.rh >> nth=3"); await W(100);
    await p.click(".rgbar button.danger"); await W(500);
    const r = await R();
    check("Delete removes the selected document (no submissions: no question asked)", r.docs.length === 3, String(r.docs.length));
  });
  await T("delete the whole register", async () => {
    await p.click(".page button:has-text('Settings…')"); await W(300);
    await p.click(".dlg .stabs2 button:has-text('Delete register')"); await W(200);
    await p.click(".dlg button:has-text('Delete the whole register')"); await W(300);
    await p.click(".ov:last-child .dlg .btn.pri"); await W(700);
    const r = await R();
    check("Settings › Delete register empties it after asking", !r || (!(r.docs ?? []).length && !(r.log ?? []).length), JSON.stringify(r).slice(0, 120));
  });

  check("no page errors", errors.length === 0, errors.slice(0, 3).join(" | "));
  console.log(`\n${passed} passed, ${failed} failed`);
  await b.close(); srv.close(); br.kill();
  process.exit(failed ? 1 : 0);
})();
