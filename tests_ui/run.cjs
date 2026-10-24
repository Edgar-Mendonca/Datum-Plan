// UI tests: the built window (ui/dist) in Chromium with the engine's real output for a template
// (cargo run -p datum-core --example dump > tests_ui/fixture.json) behind a mocked Tauri bridge.
// Run: npm run build:ui && node tests_ui/run.cjs
const { chromium } = require("playwright");
const fs = require("fs"), http = require("http"), path = require("path");
const root = path.join(__dirname, "../ui/dist"), fix = JSON.parse(fs.readFileSync(path.join(__dirname, "fixture.json")));
const types = { ".js": "text/javascript", ".css": "text/css", ".svg": "image/svg+xml", ".woff2": "font/woff2", ".woff": "font/woff", ".html": "text/html" };
const srv = http.createServer((q, r) => {
  const f = path.join(root, q.url === "/" ? "index.html" : decodeURIComponent(q.url.split("?")[0]));
  fs.readFile(f, (e, b) => { if (e) { r.writeHead(404); return r.end(); } r.writeHead(200, { "content-type": types[path.extname(f)] ?? "application/octet-stream" }); r.end(b); });
});
let failed = 0;
const check = (name, ok, info = "") => { console.log(`${ok ? "ok  " : "FAIL"} ${name}${ok ? "" : "  " + info}`); if (!ok) failed++; };

(async () => {
  await new Promise(ok => srv.listen(0, ok));
  const url = `http://localhost:${srv.address().port}/`;
  const b = await chromium.launch(process.env.CHROMIUM ? { executablePath: process.env.CHROMIUM } : {});
  const p = await b.newPage({ viewport: { width: 1440, height: 900 } });
  const errors = [];
  p.on("pageerror", e => errors.push(e.message));
  await p.addInitScript(f => {
    const opts = { dateFmt: "%d-%b-%y", autosave: 0, beta: false, currency: { code: "USD", symbol: "$", decimals: 2, after: false }, company: "", logo: "", workspace: "", setupPanel: false, theme: "light" };
    const m = { ...f, options: opts, startup_file: null, recovery: null, check_update: null, templates: [], recent: [], version: "test", resources: [], checks: [], sheet_view: null, task_info: null };
    window.__calls = [];
    window.__TAURI__ = { core: { invoke: async (c, a) => { window.__calls.push([c, a]); if (c in m) return m[c]; if (c === "apply") return f.state; return null; } }, event: { listen: async () => () => {} } };
  }, fix);
  await p.goto(url);
  await p.waitForTimeout(700);
  await p.click(".bs aside .back");
  await p.waitForTimeout(500);

  // 1. Headings line up with the cells under them (the 0.3.0 bug).
  const offsets = await p.evaluate(() => {
    const heads = [...document.querySelectorAll(".ghead .hc")].slice(1, -1), cells = [...document.querySelector(".gbody .row").children].slice(1);
    return heads.map((h, i) => Math.abs(h.getBoundingClientRect().left - cells[i].getBoundingClientRect().left));
  });
  check("column headings line up with cells", offsets.length > 3 && Math.max(...offsets) < 1, JSON.stringify(offsets));
  await p.evaluate(() => { document.querySelector(".gscroll").scrollLeft = 300; });
  await p.waitForTimeout(200);
  const after = await p.evaluate(() => {
    const heads = [...document.querySelectorAll(".ghead .hc")].slice(1, -1), cells = [...document.querySelector(".gbody .row").children].slice(1);
    return heads.map((h, i) => Math.abs(h.getBoundingClientRect().left - cells[i].getBoundingClientRect().left));
  });
  check("headings still line up after scrolling sideways", Math.max(...after) < 1, JSON.stringify(after));
  await p.evaluate(() => { document.querySelector(".gscroll").scrollLeft = 0; });

  // 2. Dragging selects exactly the cells dragged over.
  const cellBox = (row, col) => p.evaluate(([r, c]) => { const el = document.querySelectorAll(".gbody .row")[r].children[c + 1].getBoundingClientRect(); return { x: el.left + el.width / 2, y: el.top + el.height / 2 }; }, [row, col]);
  const a = await cellBox(2, 2), z = await cellBox(4, 4);
  await p.mouse.move(a.x, a.y); await p.mouse.down(); await p.mouse.move(z.x, z.y, { steps: 6 }); await p.mouse.up();
  const picked = await p.evaluate(() => [...document.querySelectorAll(".gbody .row")].slice(0, 6).map(r => [...r.children].slice(1).map(c => (c.classList.contains("in") ? 1 : 0)).join("")));
  check("drag selects a 3×3 block", picked[2] === picked[3] && picked[3] === picked[4] && picked[2].slice(2, 5) === "111" && !picked[1].includes("1") && !picked[5].includes("1") && (picked[2].match(/1/g) || []).length === 3, JSON.stringify(picked));
  check("status shows Sum/Count of the selection", (await p.textContent(".fbar")).includes("Count"));

  // 2b. Clicking cell after cell moves the selection each time (0.3.2 kept the first row), and double-click edits.
  const curAt = () => p.evaluate(() => { const rs = [...document.querySelectorAll(".gbody .row")]; for (let i = 0; i < rs.length; i++) { const j = [...rs[i].children].slice(1).findIndex(c => c.classList.contains("at")); if (j >= 0) return [i, j]; } return null; });
  const seq = [[1, 1], [4, 2], [6, 1], [3, 3]], got = [];
  for (const [r, c] of seq) { const q = await cellBox(r, c); await p.mouse.click(q.x, q.y); await p.waitForTimeout(80); got.push(await curAt()); }
  check("clicking cells selects each one", JSON.stringify(got) === JSON.stringify(seq), JSON.stringify(got));
  const e1 = await cellBox(2, 1); await p.mouse.dblclick(e1.x, e1.y); await p.waitForTimeout(150);
  await p.keyboard.press("Control+A"); await p.keyboard.type("Renamed task"); await p.keyboard.press("Enter"); await p.waitForTimeout(150);
  check("double-click edits a cell", JSON.stringify(await p.evaluate(() => window.__calls.filter(c => c[0] === "apply").pop())).includes("Renamed task"));

  // 3. Right-click menu commands run (they did nothing in 0.3.0) and show icons.
  const head = await p.$(".ghead .hc:nth-child(3)");
  await head.click({ button: "right" });
  check("heading menu has icons", (await p.$$(".menu button .tick svg")).length > 5);
  await p.getByText("New Formula Column…", { exact: true }).click();
  await p.waitForTimeout(300);
  check("menu command opens the Formula Column dialog", (await p.textContent(".dlg header").catch(() => "")).includes("Formula"));
  await p.keyboard.press("Escape");

  // 4. Ribbon: split buttons and the separate contextual Format tab.
  check("Gantt Chart is a split button", (await p.$$(".splitbig")).length >= 2);
  check("Format tab carries its Gantt Chart Tools label", (await p.textContent(".ctxgrp")).includes("Gantt Chart Tools"));
  await p.click(".ctxgrp button");
  check("Gantt Chart Style gallery", (await p.$$(".swatch")).length >= 8);

  // 5. The File sidebar is blue, not black.
  await p.click(".tabs .file");
  await p.waitForTimeout(200);
  const bg = await p.$eval(".bs aside", el => getComputedStyle(el).backgroundColor);
  check("File sidebar is blue", bg === "rgb(11, 92, 173)", bg);
  await p.click(".bs aside .back");

  // 5b. Options: each label sits left of its control (0.3.3 had them swapped).
  await p.click(".tabs .file"); await p.waitForTimeout(150);
  await p.click(".bs aside button:has-text('Options')"); await p.waitForTimeout(200);
  const bad = await p.$$eval(".opt", os => os.filter(o => o.children[0].getBoundingClientRect().right > o.children[1].getBoundingClientRect().left + 1).length);
  check("Options labels sit left of their controls", bad === 0 && (await p.$$(".opt")).length > 8, String(bad));
  await p.click(".bs aside .back"); await p.waitForTimeout(200);

  // 5c. Text formatting: Ctrl+B on a cell sends a Style edit; the Font group is on the Task tab.
  const fc = await cellBox(3, 1); await p.mouse.click(fc.x, fc.y); await p.keyboard.press("Control+b"); await p.waitForTimeout(150);
  check("Ctrl+B formats the selected cells", JSON.stringify(await p.evaluate(() => window.__calls.filter(c => c[0] === "apply").pop())).includes('"Style"'));
  await p.click(".tabs > button:text-is('Task')"); await p.waitForTimeout(100);
  check("Font group on the Task tab", (await p.$$(".fontg .fb")).length >= 8);

  // 5d. Print: Ctrl+P opens the print page with a preview; printing lays out the same pages with the paper size.
  await p.keyboard.press("Control+p"); await p.waitForTimeout(400);
  const nprev = (await p.$$(".pprev .ppage")).length;
  check("Print page shows a page preview", nprev >= 1 && (await p.textContent(".pprev .phead")).includes(fix.state.project.name), String(nprev));
  await p.evaluate(() => { window.print = () => { window.__printed = [document.querySelectorAll("#printroot .ppage").length, [...document.querySelectorAll("style")].map(s => s.textContent).find(t => t.includes("@page")) || ""]; }; });
  await p.click(".pbtns .btn"); await p.waitForTimeout(400);
  const pr = await p.evaluate(() => window.__printed);
  check("Print renders every page with its paper size", pr && pr[0] === nprev && /size: 297mm 210mm/.test(pr[1]), JSON.stringify(pr));
  await p.click(".bs aside .back"); await p.waitForTimeout(200);

  // 0.3.5: nested Insert Column menu, hide/unhide/delete, every column printed, ribbon fits, Setup in the sidebar.
  await p.keyboard.press("Escape");
  const hd = await p.$(".ghead .hc:nth-child(4)"); await hd.click({ button: "right" }); await p.waitForTimeout(150);
  await p.hover(".menu button:has-text('Insert Column')"); await p.waitForTimeout(120);
  await p.hover(".menu button:has-text('Schedule')"); await p.waitForTimeout(150);
  check("Insert Column categories open their fields", (await p.$$(".menu")).length === 3 && (await p.$$eval(".menu:nth-of-type(3) button", b => b.length)) > 10);
  await p.mouse.click(5, 5);
  const hd2 = await p.$(".ghead .hc:nth-child(4)"); const hname = (await hd2.textContent()).trim(); await hd2.click({ button: "right" }); await p.click(".menu button:has-text('Hide Column')"); await p.waitForTimeout(150);
  const hc = JSON.stringify(await p.evaluate(() => window.__calls.filter(c => c[0] === "apply").pop()));
  check("Hide Column keeps the column, hidden", /"Columns".*-\d+/.test(hc), hc.slice(0, 200));
  const hd3 = await p.$(".ghead .hc:nth-child(4)"); await hd3.click({ button: "right" });
  check("Unhide Columns lists the hidden column", !(await p.$eval(".menu button:has-text('Unhide Columns')", b => b.disabled)) && (await p.$$(".menu button:has-text('Delete Column')")).length === 1, hname);
  await p.mouse.click(5, 5);
  await p.evaluate(() => document.querySelector(".gscroll").focus()); await p.keyboard.press("Control+p"); await p.waitForTimeout(400);
  const heads = await p.$$eval(".pprev svg text", t => t.map(x => x.textContent));
  check("Print includes every column of the table", ["Task Name", "Duration", "Start", "Finish"].every(h => heads.includes(h)), heads.slice(0, 12).join("|"));
  await p.click(".bs aside .back"); await p.waitForTimeout(150);
  for (const w of [1280, 1600]) { await p.setViewportSize({ width: w, height: 900 }); for (const t of ["Task", "Schedule", "Progress", "Report", "View", "Format"]) { await p.click(`.tabs button:text-is('${t}')`); await p.waitForTimeout(120); const o = await p.$eval(".ribbon", r => r.scrollWidth - r.clientWidth); if (o > 1) check(`ribbon fits at ${w}px (${t})`, false, String(o)); } }
  check("ribbon never scrolls sideways (1280/1600 px)", true);
  await p.setViewportSize({ width: 1440, height: 900 });
  await p.click('.side button[title="Setup"]'); await p.waitForTimeout(200);
  check("Setup opens as a page from the sidebar", !!(await p.$(".setuppage .setup")));
  await p.click('.side button[title="Gantt"]'); await p.waitForTimeout(150);

  // 0.3.6: every view prints: exact pages for Gantt, Network, Calendar and tables; reports as a flowing document.
  const noPages = [];
  for (const v of ["Gantt", "Network", "Calendar", "Resources", "Task Usage", "Res. Usage", "Team", "Dashboard", "Progress", "S-Curves", "Builder"]) {
    if (!(await p.$(`.side button[title="${v}"]`))) continue;
    await p.click(`.side button[title="${v}"]`); await p.waitForTimeout(300);
    await p.click(".tabs .file"); await p.waitForTimeout(100); await p.click(".bs aside button:has-text('Print')"); await p.waitForTimeout(700);
    const n = await p.evaluate(() => document.querySelectorAll(".pprev .ppage, .pprev .pflow").length);
    if (!n) noPages.push(v);
    await p.click(".bs aside .back"); await p.waitForTimeout(150);
  }
  check("every view gives print pages", noPages.length === 0, noPages.join(", "));
  await p.click('.side button[title="Network"]'); await p.waitForTimeout(400);
  check("Network Diagram has a legend and link labels explained", (await p.textContent(".netleg")).includes("Start-to-Start") && (await p.$$(".net .nbox")).length > 3);
  await p.click('.side button[title="Gantt"]'); await p.waitForTimeout(400);
  check("timeline shows the zoom box", await p.evaluate(() => !!document.querySelector(".tlwrap canvas")));

  // 0.3.6: Help tab links and glossary; font list.
  await p.click(".tabs > button:text-is('Help')"); await p.waitForTimeout(150);
  check("Help tab has GitHub, Releases and Report an Issue", (await p.$$eval(".ribbon .rb span", s => s.map(x => x.textContent))).filter(t => ["GitHub", "Releases", "Report an Issue", "Glossary", "Getting Started"].includes(t)).length === 5);
  await p.click(".ribbon button[title*='FS, SS']"); await p.waitForTimeout(200);
  await p.fill(".dlg .find", "start-to-start"); await p.waitForTimeout(100);
  check("Glossary explains link types", (await p.textContent(".dlg .gloss")).includes("Start-to-Start"));
  await p.keyboard.press("Escape");
  await p.click(".tabs > button:text-is('Task')"); await p.waitForTimeout(100);
  check("Font list on the Task tab", (await p.$$eval(".ffam option", o => o.map(x => x.textContent))).includes("Times New Roman"));

  // 0.3.7: Tracking Gantt is its own view (banner + Tracking table), network WBS bands.
  await p.click('.side button[title="Tracking"]'); await p.waitForTimeout(400);
  const trkCols = await p.evaluate(() => window.__calls.filter(c => c[0] === "query").map(c => c[1]?.q?.cols ?? []).filter(c => c.length > 2).pop() ?? []);
  check("Tracking Gantt explains itself and uses the Tracking table", !!(await p.$(".trkbar")) && trkCols.includes("astart") && trkCols.includes("pct"), JSON.stringify(trkCols));
  await p.click('.side button[title="Gantt"]'); await p.waitForTimeout(300);
  const ganttCols = await p.evaluate(() => window.__calls.filter(c => c[0] === "query").map(c => c[1]?.q?.cols ?? []).filter(c => c.length > 2).pop() ?? []);
  check("back on the Gantt chart the Entry table returns", !ganttCols.includes("astart") && !(await p.$(".trkbar")), JSON.stringify(ganttCols));
  await p.click('.side button[title="Network"]'); await p.waitForTimeout(300);
  await p.click("label.chk:has-text('By WBS') input"); await p.waitForTimeout(300);
  check("Network by WBS asks the engine for bands", await p.evaluate(() => window.__calls.some(c => c[0] === "network" && c[1]?.wbs === true)));
  check("Network has the whiteboard canvas", (await p.$eval(".netwrap", e => getComputedStyle(e).backgroundImage)).includes("radial-gradient"));
  await p.click('.side button[title="Gantt"]'); await p.waitForTimeout(200);

  // 6. Every view opens without script errors.
  for (const v of await p.$$eval(".side button", bs => bs.map(x => x.title))) {
    await p.click(`.side button[title="${v}"]`);
    await p.waitForTimeout(250);
  }
  check("every view opens without errors", errors.length === 0, errors.join(" | "));

  await b.close();
  srv.close();
  console.log(failed ? `${failed} UI test(s) failed` : "All UI tests passed");
  process.exit(failed ? 1 : 0);
})();
