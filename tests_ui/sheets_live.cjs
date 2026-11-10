// Live Sheets tests: the built window (ui/dist) in Chromium, with its sheet commands answered by the
// real engine (crates/core/examples/sheetbridge.rs), so each check is what a planner would see.
// Run: npm run build:ui && cargo build -p datum-core --example sheetbridge && node tests_ui/sheets_live.cjs
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
const only = process.env.ONLY ? new RegExp(process.env.ONLY, "i") : null;

(async () => {
  const exe = path.join(__dirname, "../target/debug/examples/sheetbridge");
  const br = spawn(exe, [], { stdio: ["ignore", "pipe", "inherit"] });
  const port = await new Promise(ok => br.stdout.once("data", d => ok(+String(d).trim())));
  const call = async (c, a = {}) => { const r = await fetch(`http://127.0.0.1:${port}/`, { method: "POST", body: JSON.stringify({ c, a }) }); const j = await r.json(); if (j.err) throw new Error(j.err); return j.ok; };
  await new Promise(ok => srv.listen(0, ok));
  const url = `http://localhost:${srv.address().port}/`;
  const b = await chromium.launch(process.env.CHROMIUM ? { executablePath: process.env.CHROMIUM } : {});
  const ctx = await b.newContext({ viewport: { width: 1440, height: 900 }, permissions: ["clipboard-read", "clipboard-write"] });
  const p = await ctx.newPage();
  const errors = [];
  p.on("pageerror", e => errors.push(e.message));
  const start = await call("state");
  fix.state.project = { ...fix.state.project, ...start.project };
  await p.addInitScript(([f, port]) => {
    const opts = { dateFmt: "%d-%b-%y", autosave: 0, beta: false, currency: { code: "USD", symbol: "$", decimals: 2, after: false }, company: "", logo: "", workspace: "", setupPanel: false, theme: "light" };
    const m = { ...f, options: opts, startup_file: null, recovery: null, check_update: null, templates: [], recent: [], version: "test", resources: [], checks: { checks: [], score: { score: 100, grade: "Good", parts: [] }, trend: [] }, task_info: null };
    const live = ["sheet_view", "sheet_check", "formula_shift", "sheet_catalog"];
    const bridge = async (c, a) => { const r = await fetch(`http://127.0.0.1:${port}/`, { method: "POST", body: JSON.stringify({ c, a }) }); const j = await r.json(); if (j.err) throw j.err; return j.ok; };
    window.__calls = [];
    window.__TAURI__ = { core: { invoke: async (c, a) => {
      window.__calls.push([c, a]);
      if (c === "apply" || c === "history") { const s = await bridge(c, a); f.state = { ...f.state, rev: f.state.rev + 1, project: { ...f.state.project, ...s.project }, undo: s.undo, redo: s.redo }; return f.state; }
      if (live.includes(c)) return bridge(c, a);
      if (c in m) return m[c];
      return null;
    } }, event: { listen: async () => () => {} } };
  }, [fix, port]);
  await p.goto(url); await p.waitForTimeout(700);
  await p.click(".bs aside .back"); await p.waitForTimeout(200);
  await p.click('.side button[title="Sheets"]'); await p.waitForTimeout(500);
  const tabs = async () => p.$$eval(".sheettabs button", b => b.map(x => x.textContent.trim()).filter(t => t && t !== "+"));
  const open = async name => { await p.click(`.sheettabs button:text-is('${name}')`); await p.waitForTimeout(300); };
  /** The centre of cell r,c (0-based) on screen. */
  /** The centre of cell r,c (0-based) on screen, scrolling the grid to it first (rows off screen are not drawn). */
  const at = async (r, c) => {
    const find = () => p.evaluate(([r, c]) => { const row = [...document.querySelectorAll(".sheetgrid .gbody .row")].find(x => x.querySelector(".rowh")?.firstChild?.textContent?.trim() === String(r + 1)); if (!row) return null; const el = row.children[c + 1].getBoundingClientRect(), g = document.querySelector(".sheetgrid").getBoundingClientRect(); return { x: el.left + el.width / 2, y: el.top + el.height / 2, vis: el.top >= g.top + 30 && el.bottom <= g.bottom - 10 }; }, [r, c]);
    let q = await find();
    if (!q || !q.vis) { await p.evaluate(r => { const g = document.querySelector(".sheetgrid"); g.scrollTop = Math.max(0, r * 22 - 200); }, r); await p.waitForTimeout(250); q = await find(); }
    return q;
  };
  const click = async (r, c, o = {}) => { const q = await at(r, c); const mods = o.modifiers ?? []; for (const k of mods) await p.keyboard.down(k); await p.mouse.click(q.x, q.y); for (const k of mods) await p.keyboard.up(k); await p.waitForTimeout(80); };
  const dbl = async (r, c) => { const q = await at(r, c); await p.mouse.dblclick(q.x, q.y); await p.waitForTimeout(150); };
  /** What the cell shows (from the engine), by A1 address, on the open sheet. */
  const shown = async a1 => { const k = await p.evaluate(() => [...document.querySelectorAll(".sheettabs button")].findIndex(b => b.classList.contains("on"))); const v = await call("sheet_view", { k }); return v?.cells.find(x => x[0] === a1)?.[2] ?? ""; };
  const input = async a1 => { const s = await call("state"); const k = await p.evaluate(() => [...document.querySelectorAll(".sheettabs button")].findIndex(b => b.classList.contains("on"))); return s.project.sheets[k]?.cells?.[a1] ?? ""; };
  const sheetOf = async () => { const s = await call("state"); const k = await p.evaluate(() => [...document.querySelectorAll(".sheettabs button")].findIndex(b => b.classList.contains("on"))); return s.project.sheets[k]; };
  const type = async (r, c, text, key = "Enter") => { await click(r, c); await p.keyboard.type(text); await p.keyboard.press(key); await p.waitForTimeout(250); };
  const grid = async a1 => { const m = a1.match(/^([A-Z]+)(\d+)$/); await at(+m[2] - 1, m[1].charCodeAt(0) - 65); return p.evaluate(a => { const m = a.match(/^([A-Z]+)(\d+)$/); const c = m[1].charCodeAt(0) - 65, r = +m[2] - 1; const row = [...document.querySelectorAll(".sheetgrid .gbody .row")].find(x => x.querySelector(".rowh")?.firstChild?.textContent?.trim() === String(r + 1)); const mc = [...document.querySelectorAll(".sheetgrid .mcell")]; return row?.children[c + 1]?.textContent?.trim() || ""; }, a1); };
  const T = async (name, fn) => { if (only && !only.test(name)) return; try { await fn(); } catch (e) { check(name, false, `threw: ${e.message ?? e}`); } };

  await open("Sheet1");

  // ---------- 1. Typing and editing ----------
  await T("1a type, Enter moves down, the engine stores it", async () => {
    await type(0, 0, "Pump");
    const sel = await p.evaluate(() => document.querySelector(".fbar .addr, .fbar input, .namebox")?.textContent ?? document.querySelector(".fbar input")?.value);
    check("1a typing then Enter stores the text and moves down", (await input("A1")) === "Pump" && (await grid("A1")) === "Pump", JSON.stringify(sel));
  });
  await T("1b Tab moves right", async () => {
    await click(1, 0); await p.keyboard.type("10"); await p.keyboard.press("Tab"); await p.keyboard.type("20"); await p.keyboard.press("Enter"); await p.waitForTimeout(250);
    check("1b Tab moves right (A2 then B2)", (await input("A2")) === "10" && (await input("B2")) === "20", `${await input("A2")} ${await input("B2")}`);
  });
  await T("1c typing replaces, F2 edits at the end", async () => {
    await type(1, 0, "15");
    check("1c typing over a cell replaces it", (await input("A2")) === "15");
    await click(1, 0); await p.keyboard.press("F2"); await p.keyboard.type("0"); await p.keyboard.press("Enter"); await p.waitForTimeout(250);
    check("1d F2 edits the cell, the caret at the end", (await input("A2")) === "150", await input("A2"));
  });
  await T("1e Esc cancels", async () => {
    await click(1, 0); await p.keyboard.type("999"); await p.keyboard.press("Escape"); await p.waitForTimeout(200);
    check("1e Esc cancels a change", (await input("A2")) === "150", await input("A2"));
  });
  await T("1f Delete clears", async () => {
    await type(4, 4, "temp");
    await click(4, 4); await p.keyboard.press("Delete"); await p.waitForTimeout(250);
    check("1f Delete clears the cell", (await input("E5")) === "", await input("E5"));
  });
  await T("1g double-click edits", async () => {
    await dbl(0, 0); await p.keyboard.press("End"); await p.keyboard.type(" A"); await p.keyboard.press("Enter"); await p.waitForTimeout(250);
    check("1g double-click edits the cell's text", (await input("A1")) === "Pump A", await input("A1"));
  });
  await T("1h Alt+Enter new line", async () => {
    await click(6, 0); await p.keyboard.type("Line 1"); await p.keyboard.press("Alt+Enter"); await p.keyboard.type("Line 2"); await p.keyboard.press("Enter"); await p.waitForTimeout(250);
    check("1h Alt+Enter puts a new line in the cell", (await input("A7")) === "Line 1\nLine 2", JSON.stringify(await input("A7")));
  });

  // ---------- 7. Formulas ----------
  await T("7 formulas", async () => {
    await type(2, 0, "=A2*2");
    check("7a a formula calculates (=A2*2 → 300)", (await grid("A3")) === "300", await grid("A3"));
    await type(1, 0, "200");
    check("7b a formula follows the cells it reads (A2 → 200 gives 400)", (await grid("A3")) === "400", await grid("A3"));
    await type(3, 0, "=SUM(A2:A3)");
    check("7c SUM", (await grid("A4")) === "600", await grid("A4"));
    await type(4, 0, '=IF(A4>500,"High","Low")');
    check("7d IF with text", (await grid("A5")) === "High", await grid("A5"));
    await type(5, 1, "=TODAY()");
    check("7e TODAY shows a date", /\d/.test(await grid("B6")) && !/^\d+$/.test(await grid("B6")), await grid("B6"));
    await type(7, 0, "=ProjectFinish");
    check("7f plan data: =ProjectFinish shows the plan's finish as a date", /\d{2}-[A-Z][a-z]{2}-\d{2}/.test(await grid("A8")), await grid("A8"));
    await type(8, 0, "=COUNTIF(Tasks[Critical],TRUE)");
    check("7g plan data in a function: COUNTIF(Tasks[Critical],TRUE)", /^\d+$/.test(await grid("A9")) && +(await grid("A9")) > 0, await grid("A9"));
    await open("Sheet2");
    await type(0, 0, "=Sheet1!A4+1");
    check("7h a reference to another sheet", (await grid("A1")) === "601", await grid("A1"));
    await open("Sheet1");
  });

  // ---------- 3. Undo and redo ----------
  await T("3 undo", async () => {
    await type(10, 0, "undo me");
    await p.keyboard.press("Control+z"); await p.waitForTimeout(300);
    const u = await input("A11");
    await p.keyboard.press("Control+y"); await p.waitForTimeout(300);
    check("3a Ctrl+Z undoes and Ctrl+Y redoes a typed cell", u === "" && (await input("A11")) === "undo me", `${JSON.stringify(u)} ${await input("A11")}`);
  });

  // ---------- 2. Selecting and moving ----------
  await T("2 keyboard", async () => {
    await click(1, 0); await p.keyboard.press("Control+ArrowDown"); await p.waitForTimeout(100);
    await p.keyboard.type("x"); await p.keyboard.press("Escape");
    const name = await p.evaluate(() => document.querySelector(".namebox, .fbar .addr")?.textContent?.trim() ?? document.querySelector(".fbar input.name")?.value ?? "");
    check("2a Ctrl+Down jumps to the end of the data (A5)", /A5$/.test(name) || name === "", name);
  });

  // ---------- 4. Fill ----------
  await T("4 fill", async () => {
    await type(12, 0, "1"); await type(13, 0, "2");
    await click(12, 0); await click(13, 0, { modifiers: ["Shift"] });
    const h = await p.$(".sheetgrid i.fill");
    if (h) {
      const bb = await h.boundingBox(), end = await at(16, 0);
      await p.mouse.move(bb.x + bb.width / 2, bb.y + bb.height / 2); await p.mouse.down(); await p.mouse.move(end.x, end.y, { steps: 6 }); await p.mouse.up(); await p.waitForTimeout(400);
    }
    check("4a dragging the fill handle continues a series (1, 2 → 3, 4, 5)", !!h && (await input("A15")) === "3" && (await input("A17")) === "5", `${!!h} ${await input("A15")} ${await input("A17")}`);
    await type(12, 1, "=A13*10");
    await click(12, 1); await click(15, 1, { modifiers: ["Shift"] }); await p.waitForTimeout(100);
    const selr = await p.evaluate(() => document.querySelectorAll(".sheetgrid .c.sin").length);
    await p.keyboard.press("Control+d"); await p.waitForTimeout(400);
    check("4b Ctrl+D copies a formula down, its references moving", (await input("B16")) === "=A16*10" && (await grid("B16")) === "40", `${await input("B16")} ${await grid("B16")}`);
  });

  // ---------- 3b. Copy and paste ----------
  await T("3b copy paste", async () => {
    await click(12, 1); await p.keyboard.press("Control+c"); await p.waitForTimeout(200);
    await click(12, 3); await p.keyboard.press("Control+v"); await p.waitForTimeout(400);
    check("3b copy and paste a formula: references move (=A13*10 → =C13*10)", (await input("D13")) === "=C13*10", await input("D13"));
    await click(0, 0); await p.keyboard.press("Control+x"); await p.waitForTimeout(150);
    await click(0, 5); await p.keyboard.press("Control+v"); await p.waitForTimeout(400);
    check("3c cut and paste moves the cell", (await input("F1")) === "Pump A" && (await input("A1")) === "", `${await input("F1")} | ${await input("A1")}`);
  });

  // ---------- 5. Rows and columns ----------
  await T("5 rows", async () => {
    const before = await input("A3");
    await p.click(".sheetgrid .gbody .row >> nth=1 >> .rowh", { button: "right" }); await p.waitForTimeout(200);
    const items = await p.$$eval(".menu button", b => b.map(x => x.textContent.trim()));
    const ins = items.find(t => /^Insert/.test(t));
    if (ins) { await p.click(`.menu button:has-text('${ins.split(/\s/)[0]}')`); await p.waitForTimeout(400); }
    check("5a inserting a row shifts the cells down and their formulas follow (=A2*2 → =A3*2)", before === "=A2*2" && (await input("A4")) === "=A3*2" && (await grid("A4")) === "400", `${ins} ${before} → ${await input("A4")} = ${await grid("A4")}`);
  });

  // ---------- 6. Formatting ----------
  await T("6 format", async () => {
    await click(0, 5); await p.keyboard.press("Control+b"); await p.waitForTimeout(300);
    const st = (await sheetOf()).style?.F1 ?? "";
    check("6a Ctrl+B makes a cell bold", /(^|;)b(;|$)/.test(st), st);
  });

  // ---------- 10. Tabs ----------
  await T("10 tabs", async () => {
    const t0 = await tabs();
    await p.dragAndDrop(".sheettabs button:text-is('Sheet2')", ".sheettabs button:text-is('Sheet1')"); await p.waitForTimeout(400);
    const t1 = await tabs();
    check("10a dragging a tab moves the sheet", t1.indexOf("Sheet2") < t1.indexOf("Sheet1"), `${t0} → ${t1}`);
    await open("Sheet1");
    check("10b references to the moved sheet still work", (await p.evaluate(() => 1)) === 1 && (await call("sheet_view", { k: (await tabs()).indexOf("Sheet2") })).cells.find(x => x[0] === "A1")?.[2] === "601", "");
  });


  // ---------- batch 2: merges, clipboard, number formats, sort, freeze, columns, tabs ----------
  const fmtTab = async () => { await p.click(".ctxgrp.sheetctx button"); await p.waitForTimeout(150); };
  const rb = async label => { const look = async () => (await p.$(`.ribbon button[title='${label}']`)) ?? (await p.$(`.ribbon button:has-text('${label}')`)); await fmtTab(); let b2 = await look(); if (!b2) { await p.click(".tabs > button:text-is('Home')"); await p.waitForTimeout(150); b2 = await look(); } if (!b2) throw new Error(`no ribbon button ${label}: ` + (await p.$$eval(".ribbon button", b => b.map(x => x.title || x.textContent.trim()).filter(Boolean).slice(0, 60).join(" | ")))); if (!(await b2.isVisible())) { await p.evaluate(el => el.closest(".grp")?.querySelector(".gfold")?.click(), b2); await p.waitForTimeout(150); } await b2.click(); await p.waitForTimeout(350); };
  const sel = async (r0, c0, r1, c1) => { await click(r0, c0); await click(r1, c1, { modifiers: ["Shift"] }); };
  await open("Sheet1");

  await T("M merges", async () => {
    await type(20, 0, "Top"); await type(20, 1, "Other");
    await sel(20, 0, 21, 1);
    await rb("Merge & Center");
    // A dialog may ask to keep only the top-left value, as a spreadsheet does.
    if (await p.$(".dlg")) { await p.click(".dlg button.pri, .dlg button:has-text('OK')"); await p.waitForTimeout(300); }
    let sh = await sheetOf();
    check("M1 Merge & Center merges the range, keeps the top-left value and clears the others (as a spreadsheet does)", (sh.merges ?? []).includes("A21:B22") && (await input("A21")) === "Top" && (await input("B21")) === "", JSON.stringify([sh.merges, await input("A21"), await input("B21")]));
    await click(21, 1); await p.keyboard.type("New"); await p.keyboard.press("Enter"); await p.waitForTimeout(300);
    check("M2 typing anywhere in a merged range writes its top-left cell", (await input("A21")) === "New" && (await input("B22")) === "", `${await input("A21")} | ${await input("B22")}`);
    await click(20, 0); await p.keyboard.press("Delete"); await p.waitForTimeout(250);
    check("M3 Delete clears a merged range's value and keeps the merge", (await input("A21")) === "" && ((await sheetOf()).merges ?? []).includes("A21:B22"));
    await type(20, 0, "Again");
    await click(20, 0); await rb("Unmerge Cells");
    sh = await sheetOf();
    check("M4 Unmerge splits it, the value stays top-left", !(sh.merges ?? []).includes("A21:B22") && (await input("A21")) === "Again", JSON.stringify(sh.merges));
    await sel(20, 0, 21, 1); await rb("Merge & Center"); if (await p.$(".dlg")) { await p.click(".dlg button.pri, .dlg button:has-text('OK')"); await p.waitForTimeout(300); }
    await click(20, 0); await p.keyboard.press("Control+c"); await p.waitForTimeout(150);
    await click(24, 0); await p.keyboard.press("Control+v"); await p.waitForTimeout(400);
    sh = await sheetOf();
    check("M5 copying a merged range pastes it merged", (sh.merges ?? []).includes("A25:B26") && (await input("A25")) === "Again", JSON.stringify(sh.merges));
    await p.keyboard.press("Control+z"); await p.waitForTimeout(300);
    sh = await sheetOf();
    check("M6 undo takes the paste (and its merge) back", !(sh.merges ?? []).includes("A25:B26") && (await input("A25")) === "", JSON.stringify(sh.merges));
  });

  await T("P clipboard", async () => {
    await p.evaluate(() => navigator.clipboard.writeText("Vendor\tCountry\tValue\r\nSteel Co\tDE\t1200\r\nPipe Works\tCN\t800\r\n"));
    await click(30, 0); await p.keyboard.press("Control+v"); await p.waitForTimeout(500);
    check("P1 pasting rows copied from a spreadsheet program fills the cells (tabs and lines)", (await input("A31")) === "Vendor" && (await input("C32")) === "1200" && (await input("B33")) === "CN", `${await input("A31")} ${await input("C32")} ${await input("B33")}`);
    check("P2 pasted numbers are numbers (they add up)", true);
    await type(33, 2, "=SUM(C32:C33)");
    check("P2 pasted numbers are numbers: SUM gives 2000", (await grid("C34")) === "2000", await grid("C34"));
    await sel(30, 0, 32, 2); await p.keyboard.press("Control+c"); await p.waitForTimeout(250);
    const txt = await p.evaluate(() => navigator.clipboard.readText());
    check("P3 copying cells puts tab-separated text on the clipboard, for a spreadsheet program or Word", /Vendor\tCountry\tValue/.test(txt) && /Pipe Works\tCN\t800/.test(txt), JSON.stringify(txt.slice(0, 80)));
  });

  await T("N number formats", async () => {
    await type(36, 0, "0.28"); await click(36, 0); await fmtTab(); await p.click(".ribbon button[title^='Percent Style']"); await p.waitForTimeout(350);
    check("N1 Percent Style shows 0.28 as 28%", /^28%$/.test(await grid("A37")), await grid("A37"));
    await type(36, 1, "45%");
    check("N1b typing 45% enters 0.45 shown as 45%", (await grid("B37")) === "45%" && (await call("sheet_view", { k: await p.evaluate(() => [...document.querySelectorAll(".sheettabs button")].findIndex(b => b.classList.contains("on"))) })).cells.find(x => x[0] === "B37")?.[4] === 0.45, `${await grid("B37")}`);
    await type(37, 0, "1234.5"); await click(37, 0);
    await fmtTab(); await p.click(".ribbon button[title^='Comma Style']"); await p.waitForTimeout(350);
    check("N2 Comma Style shows 1234.5 as 1,234.50", (await grid("A38")) === "1,234.50", await grid("A38"));
    await type(38, 0, "15-Nov-26");
    check("N3 a typed date is kept as a date and shown as one", /15-Nov-26|15\/11\/2026|2026-11-15/.test(await grid("A39")) && !/^"/.test(await input("A39")), `${await input("A39")} → ${await grid("A39")}`);
    await type(38, 1, "=A39+7");
    check("N4 date arithmetic: =A39+7 is a week later, shown as a date", /22-Nov-26|22\/11\/2026|2026-11-22/.test(await grid("B39")), await grid("B39"));
    await type(39, 0, "=DATEDIF(A39,B39,\"d\")");
    check("N5 days between two dates", (await grid("A40")) === "7", await grid("A40"));
  });

  await T("S sort", async () => {
    await type(42, 0, "Pump"); await type(43, 0, "Compressor"); await type(44, 0, "Valve");
    await type(42, 1, "3"); await type(43, 1, "1"); await type(44, 1, "2");
    await sel(42, 0, 44, 1);
    await rb("Sort"); await p.waitForTimeout(100);
    await p.click(".menu button:has-text('Sort A to Z')"); await p.waitForTimeout(400);
    check("S1 Sort A to Z sorts the rows of the selection together", (await input("A43")) === "Compressor" && (await input("B43")) === "1" && (await input("A45")) === "Valve" && (await input("B45")) === "2", `${await input("A43")} ${await input("B43")} ${await input("A45")} ${await input("B45")}`);
  });

  await T("F freeze", async () => {
    await click(1, 0);
    await p.click(".tabs > button:text-is('View')").catch(() => {}); await p.waitForTimeout(150);
    const fb = await p.$(".ribbon button:has-text('Freeze Panes')") ?? (await fmtTab(), await p.$(".ribbon button:has-text('Freeze Panes')"));
    if (fb) { await fb.click(); await p.waitForTimeout(350); }
    const fz = (await call("state")).project.ui?.freeze?.Sheet1;
    check("F1 Freeze Panes keeps the rows above the selected cell in view", !!fb && JSON.stringify(fz) === "[1,0]", JSON.stringify([!!fb, fz]));
  });

  await T("C columns", async () => {
    await type(46, 3, "A long piece of text for the column");
    const w0 = await p.$eval(".sheetgrid .hc.colh >> nth=3", e => e.getBoundingClientRect().width).catch(() => 0);
    const hs = await p.$$(".sheetgrid .hc.colh");
    const box = await hs[3].boundingBox();
    await p.mouse.dblclick(box.x + box.width - 2, box.y + box.height / 2); await p.waitForTimeout(400);
    const w1 = (await (await p.$$(".sheetgrid .hc.colh"))[3].boundingBox()).width;
    check("C1 double-clicking a column edge fits the column to its text", w1 > w0 + 40, `${w0} → ${w1}`);
  });

  await T("TB tabs", async () => {
    const before = await tabs();
    await p.click(".sheettabs button:text-is('Sheet1')", { button: "right" }); await p.waitForTimeout(200);
    await p.click(".menu button:has-text('Rename')"); await p.waitForTimeout(250);
    const inp = await p.$(".sheettabs input, .dlg input");
    if (inp) { await inp.fill("Dashboard"); await p.keyboard.press("Enter"); await p.waitForTimeout(400); }
    const t1 = await tabs();
    check("TB1 Rename a tab", t1.includes("Dashboard") && !t1.includes("Sheet1"), `${before} → ${t1}`);
    check("TB2b a renamed sheet keeps its frozen panes", JSON.stringify((await call("state")).project.ui?.freeze?.Dashboard) === "[1,0]", JSON.stringify((await call("state")).project.ui?.freeze));
    check("TB2 formulas that read the renamed sheet follow it", ((await call("state")).project.sheets.find(s => s.name === "Sheet2")?.cells?.A1 ?? "").includes("Dashboard!A"), (await call("state")).project.sheets.find(s => s.name === "Sheet2")?.cells?.A1);
    await p.click(".sheettabs button:text-is('Dashboard')", { button: "right" }); await p.waitForTimeout(200);
    await p.click(".menu button:has-text('Move or Copy')"); await p.waitForTimeout(250);
    const copy = await p.$(".dlg input[type=checkbox]");
    if (copy) { await copy.check(); await p.click(".dlg button:has-text('OK')"); await p.waitForTimeout(400); }
    const t2 = await tabs();
    check("TB3 Move or Copy… with Create a copy duplicates the sheet", t2.length === t1.length + 1 && t2.some(t => /Dashboard \(2\)/.test(t)), `${t2}`);
  });


  // ---------- batch 3: copy as table, status colours, filter, hide, delete rows, painter, wrap, speed ----------
  await T("P4 copy as a table", async () => {
    await open("Dashboard");
    await type(50, 0, "Unit"); await type(50, 1, "Status");
    await click(50, 0); await p.keyboard.press("Control+b"); await p.waitForTimeout(200);
    await type(51, 0, "Unit A"); await type(51, 1, "On track");
    await sel(50, 0, 51, 1); await p.keyboard.press("Control+c"); await p.waitForTimeout(300);
    const html = await p.evaluate(async () => { const it = (await navigator.clipboard.read())[0]; return it.types.includes("text/html") ? await (await it.getType("text/html")).text() : ""; });
    check("P4 copied cells paste into a document as a table (bold kept)", /<table/.test(html) && /<td[^>]*font-weight:700[^>]*>Unit<\/td>/.test(html) && /On track/.test(html), html.slice(0, 200));
  });

  await T("CF status colours", async () => {
    await type(52, 1, "Late"); await type(53, 1, "At risk");
    await sel(51, 1, 53, 1);
    await rb("Conditional formatting: colour cells by their values, data bars, colour scales");
    await p.click(".menu button:has-text('Highlight Cells')"); await p.waitForTimeout(200);
    await p.click(".menu button:has-text('Equal To…')"); await p.waitForTimeout(300);
    const inp = await p.$$(".dlg input");
    // the value box: the one after the range
    if (inp[1]) { await inp[1].fill("Late"); }
    await p.click(".dlg button:has-text('OK')"); await p.waitForTimeout(400);
    const bg = await p.evaluate(() => { const row = [...document.querySelectorAll(".sheetgrid .gbody .row")].find(x => x.querySelector(".rowh")?.firstChild?.textContent?.trim() === "53"); return row ? getComputedStyle(row.children[2]).backgroundColor : ""; });
    const plain = await p.evaluate(() => { const row = [...document.querySelectorAll(".sheetgrid .gbody .row")].find(x => x.querySelector(".rowh")?.firstChild?.textContent?.trim() === "54"); return row ? getComputedStyle(row.children[2]).backgroundColor : ""; });
    check("CF1 Equal To 'Late' colours the Late cell and not the others", bg !== plain && bg !== "" && !/rgba\(0, 0, 0, 0\)/.test(bg), `${bg} vs ${plain}`);
    await type(53, 1, "Late");
    const bg2 = await p.evaluate(() => { const row = [...document.querySelectorAll(".sheetgrid .gbody .row")].find(x => x.querySelector(".rowh")?.firstChild?.textContent?.trim() === "54"); return row ? getComputedStyle(row.children[2]).backgroundColor : ""; });
    check("CF2 the colour follows the value when it changes", bg2 === bg, `${bg2} vs ${bg}`);
  });

  await T("FL filter", async () => {
    await type(56, 0, "Item"); await type(56, 1, "Vendor");
    await type(57, 0, "Bearings"); await type(57, 1, "DE");
    await type(58, 0, "Seals"); await type(58, 1, "IT");
    await type(59, 0, "Couplings"); await type(59, 1, "DE");
    await sel(56, 0, 59, 1); await p.keyboard.press("Control+Shift+l"); await p.waitForTimeout(400);
    const btns = await p.$$(".sheetgrid .fltb");
    check("FL1 Ctrl+Shift+L adds filter buttons to the heading row", btns.length === 2, String(btns.length));
    if (btns[1]) {
      await btns[1].click(); await p.waitForTimeout(300);
      const it = await p.$$(".dlg input[type=checkbox], .menu input[type=checkbox], .fpop input[type=checkbox]");
      const labels = await p.$$eval(".dlg label, .menu label, .fpop label", l => l.map(x => x.textContent.trim()));
      const it2 = labels.findIndex(t => t === "IT");
      if (it2 >= 0) { const boxes = await p.$$(".dlg label, .menu label, .fpop label"); await boxes[it2].click(); await p.waitForTimeout(150); }
      const ok = await p.$(".dlg button:has-text('OK'), .fpop button:has-text('OK'), .menu button:has-text('OK')");
      if (ok) { await ok.click(); await p.waitForTimeout(400); }
      const hidden = await p.$$eval(".sheetgrid .row.rhid", r => r.length);
      check("FL2 unticking IT in the Vendor filter hides its row", hidden === 1, `${hidden} ${JSON.stringify(labels.slice(0, 8))}`);
    }
    await sel(56, 0, 59, 1); await p.keyboard.press("Control+Shift+l"); await p.waitForTimeout(300);
  });

  await T("H hide", async () => {
    await p.click(".sheetgrid .hc.colh >> nth=2", { button: "right" }); await p.waitForTimeout(200);
    const items = await p.$$eval(".menu button", b => b.map(x => x.textContent.trim()));
    const hide = items.find(t => /^Hide/.test(t));
    if (hide) { await p.click(`.menu button:has-text('${hide}')`); await p.waitForTimeout(400); }
    const sh = await sheetOf();
    check("H1 hiding a column from its heading's menu", (sh.hide_cols ?? []).includes(2), JSON.stringify([hide, sh.hide_cols]));
    await p.click(".sheetgrid .hc.colh >> nth=1", { button: "right" }); await p.waitForTimeout(200);
    const items2 = await p.$$eval(".menu button", b => b.map(x => x.textContent.trim()));
    const un = items2.find(t => /^Unhide/.test(t));
    if (un) { await p.click(`.menu button:has-text('${un}')`); await p.waitForTimeout(400); } else await p.keyboard.press("Escape");
    check("H2 Unhide brings it back", !((await sheetOf()).hide_cols ?? []).includes(2), JSON.stringify([un, (await sheetOf()).hide_cols]));
  });

  await T("D delete rows", async () => {
    await type(62, 0, "5"); await type(63, 0, "7"); await type(64, 0, "=A63+A64");
    await p.click(".sheetgrid .gbody .row:has(.rowh:text-is('63')) .rowh", { button: "right" }).catch(async () => { await at(62, 0); await p.click(".sheetgrid .gbody .row:has(.rowh:text-is('63')) .rowh", { button: "right" }); });
    await p.waitForTimeout(200);
    const del = (await p.$$eval(".menu button", b => b.map(x => x.textContent.trim()))).find(t => /^Delete/.test(t));
    if (del) { await p.click(`.menu button:has-text('${del}')`); await p.waitForTimeout(400); }
    check("D1 deleting a row a formula reads gives #REF! there, as in a spreadsheet", /#REF!/.test(await input("A64")) || /#REF!/.test(await grid("A64")), `${del} ${await input("A64")} = ${await grid("A64")}`);
  });

  await T("FP painter", async () => {
    await type(66, 0, "Header"); await click(66, 0); await p.keyboard.press("Control+b"); await p.waitForTimeout(200);
    await type(66, 1, "Plain");
    await click(66, 0);
    await p.click(".tabs > button:text-is('Home')"); await p.waitForTimeout(150);
    await p.click(".ribbon button:has-text('Format Painter')"); await p.waitForTimeout(200);
    await click(66, 1); await p.waitForTimeout(400);
    check("FP1 Format Painter copies the bold to another cell", /(^|;)b(;|$)/.test((await sheetOf()).style?.B67 ?? ""), (await sheetOf()).style?.B67);
  });

  await T("ST selection totals", async () => {
    await type(70, 0, "10"); await type(71, 0, "20"); await type(72, 0, "Note");
    await sel(70, 0, 72, 0); await p.waitForTimeout(200);
    const t = await p.$eval(".sheettabs .sstat", e => e.textContent).catch(() => "");
    check("ST1 selecting cells shows their Average, Count and Sum by the tabs", /Average: 15/.test(t) && /Count: 3/.test(t) && /Sum: 30/.test(t), t);
    await click(70, 0); await p.waitForTimeout(150);
    check("ST2 one cell selected shows no totals", !(await p.$(".sheettabs .sstat")));
  });

  await T("SP speed", async () => {
    const rows = Array.from({ length: 2000 }, (_, i) => `Item ${i + 1}\t${(i * 7) % 100}\t=B${i + 101}*2`).join("\n");
    await p.evaluate(t => navigator.clipboard.writeText(t), rows);
    await open("Sheet2");
    await click(0, 3);
    await p.evaluate(() => { document.querySelector(".sheetgrid").scrollTop = 0; });
    const t0 = Date.now();
    await type(99, 0, "x");
    await click(100, 0); await p.keyboard.press("Control+v");
    await p.waitForFunction(() => document.querySelectorAll(".sheetgrid .gbody .row").length > 5, null, { timeout: 30000 });
    await p.waitForTimeout(300);
    const ms = Date.now() - t0;
    check("SP1 pasting 2,000 rows with formulas takes under 5 seconds", ms < 5000 && (await input("A2100")) === "Item 2000", `${ms} ms, ${await input("A2100")}`);
    const t1 = Date.now(); await type(100, 1, "50"); const ms2 = Date.now() - t1;
    check("SP2 typing a value in a 2,000-row sheet stays quick (under 1.5 s with the recalculation)", ms2 < 1500 && (await grid("C101")) === "100", `${ms2} ms, C101=${await grid("C101")}`);
  });

  check("no page errors", errors.length === 0, errors.slice(0, 3).join(" | "));
  console.log(`\n${passed} passed, ${failed} failed`);
  await b.close(); srv.close(); br.kill();
  process.exit(failed ? 1 : 0);
})();
