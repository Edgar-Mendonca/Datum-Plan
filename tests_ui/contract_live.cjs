// Live Contract Register tests: the built window (ui/dist) in Chromium, with every change answered by
// the real engine (crates/core/examples/sheetbridge.rs), so each check is what a planner would see.
// Run: npm run test:contract (builds the window and the bridge first).
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
  await call("apply", { op: { Modules: { on: ["contract"] } } });
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
      if (c === "procurement" || c === "documents" || c === "contract") return bridge(c, a);
      if (c === "set_options") { m.options = a.opts; return null; }
      if (c in m) return m[c];
      return null;
    } }, event: { listen: async () => () => {} } };
  }, [fix, port]);
  // The bridge works out every date at 10 Nov 2026 (sheetbridge.rs); the window keeps the same day, so what it stamps "today"
  // (Mark sent, Mark done) agrees with the engine whatever the real date is.
  await p.clock.setFixedTime(new Date(2026, 10, 10, 12));
  await p.goto(`http://localhost:${srv.address().port}/`); await p.waitForTimeout(700);
  await p.click(".bs aside .back"); await p.waitForTimeout(200);
  await p.click('.side button[title="Contract"]'); await p.waitForTimeout(700);
  const R = async () => (await call("state")).project.contract;
  const tabTo = async t => { await p.click(`.regtabs button:text-is('${t}')`); await p.waitForTimeout(300); };
  const W = ms => p.waitForTimeout(ms);
  const cell = (i, j) => `.rg td[data-rc="${i}:${j}"]`;
  const colOf = async name => p.$$eval(".rg thead th span", (s, n) => s.findIndex(x => x.textContent.trim() === n) + 1, name);
  const typeIn = async (i, j, text, key = "Enter") => { await p.click(cell(i, j)); await W(80); await p.keyboard.type(text); await W(120); await p.keyboard.press(key); await W(450); };
  const paste = async text => { await p.evaluate(t => { const dt = new DataTransfer(); dt.setData("text/plain", t); document.dispatchEvent(new ClipboardEvent("paste", { clipboardData: dt, bubbles: true, cancelable: true })); }, text); await W(600); };
  const T = async (name, fn) => { try { await fn(); } catch (e) { check(name, false, `threw: ${e.message ?? e}`); } };

  const rowText = async i => p.$eval(cell(i, 1), td => td.parentElement.textContent);
  const status = async i => p.textContent(cell(i, await colOf("Status")));
  await T("settings", async () => {
    await p.click(".page button:has-text('Settings…')"); await W(300);
    await p.fill(".dlg label:has-text('Contract value') input", "1000000"); await p.click(".dlg button:has-text('Add the usual ones')"); await p.click(".dlg .btn.pri:text-is('Save')"); await W(500);
    const k = await R();
    check("Settings keep the value and the kinds of notice with their days", k.value === 1000000 && k.notice_kinds.find(x => x.name === "Force majeure")?.days === 14, JSON.stringify(k.notice_kinds));
  });
  await T("notices", async () => {
    await tabTo("Notices");
    await p.click(".rgbar button:has-text('+ Notice')"); await W(500);
    await typeIn(0, await colOf("Event"), "Late free-issue cables"); await typeIn(0, await colOf("Kind"), "Delay"); await typeIn(0, await colOf("Event date"), "26-Oct-26");
    check("a notice's time bar: 28 days for a Delay notice after 26 Oct is 23 Nov, 13 days from today", (await status(0)).includes("13 days left") && (await rowText(0)).includes("23 Nov 26") && (await R()).notices[0].no === "NOT-001", await rowText(0));
    await p.click(cell(1, await colOf("Notice"))); await W(100);
    await paste("NOT-002\tPort closure\tForce majeure\t02-Sep-26");
    check("a pasted Force majeure notice from 2 Sep is overdue by 55 days, in red", (await status(1)).includes("Overdue 55 d") && !!(await p.$(`${cell(1, await colOf("Status"))} .st.bad`)), await status(1));
    check("the Notices tab counts the one due within a week (the overdue one; the other has 13 days)", (await p.textContent(".regtabs button:has-text('Notices')")) === "Notices1");
    await p.click(".rg td.rh >> nth=0"); await W(100); await p.click(".rgbar button:has-text('Mark sent')"); await W(500);
    if (process.env.SHOT) await p.screenshot({ path: process.env.SHOT.replace(".png", "-notices.png") });
    check("Mark sent dates it today and it was sent in time", (await status(0)).includes("Sent in time"), await status(0));
  });
  await T("variations", async () => {
    await tabTo("Variations");
    await p.click(cell(0, await colOf("VO No."))); await W(100);
    await paste("VO-001\tAdded instrument tappings\tClient\tApproved\t05-Oct-26\t21-Oct-26\t120000\t120000\nVO-002\tExtra FAT days\tUs\tSubmitted\t20-Oct-26\t\t80000");
    const k = await R();
    check("pasted variations keep their numbers, statuses and amounts", k.variations.length === 2 && k.variations[0].approved === 120000 && k.variations[1].status === "Submitted" && k.variations[1].submitted === "2026-10-20", JSON.stringify(k.variations.map(v => [v.no, v.status, v.claimed])));
    const tot = await p.textContent(".rgtot");
    check("the totals: approved, waiting and the revised contract value", tot.includes("approved $120,000") && tot.includes("waiting $80,000") && tot.includes("$1,120,000"), tot);
    check("a submitted variation counts the days it has waited (21 since 20 Oct)", (await p.textContent(cell(1, await colOf("Waiting (d)")))).trim() === "21");
  });
  await T("guarantees", async () => {
    await tabTo("Guarantees");
    await typeIn(0, await colOf("Guarantee / insurance"), "Advance payment guarantee"); await typeIn(0, await colOf("Number"), "APG-1102"); await typeIn(0, await colOf("Expires"), "30-Nov-26");
    check("a guarantee expiring in 20 days is flagged", (await status(0)).includes("Expiring") && (await p.textContent(cell(0, await colOf("Days left")))).trim() === "20", await rowText(0));
  });
  await T("invoices", async () => {
    await tabTo("Invoices");
    await p.click(".rgbar button:has-text('+ Invoice')"); await W(500);
    await typeIn(0, await colOf("Amount"), "50000"); await typeIn(0, await colOf("Invoiced"), "01-Oct-26");
    check("an invoice sent on 1 Oct is due by the contract's 30 days and 10 days overdue", (await status(0)).includes("Overdue 10 d") && (await rowText(0)).includes("31 Oct 26") && (await R()).invoices[0].no === "INV-001", await rowText(0));
    await typeIn(0, await colOf("Paid"), "08-Nov-26");
    check("…and paid, it says so", (await status(0)).includes("Paid"));
  });
  await T("obligations", async () => {
    await tabTo("Obligations");
    await typeIn(0, await colOf("Clause"), "12.3"); await typeIn(0, await colOf("Obligation"), "Monthly progress report"); await typeIn(0, await colOf("First due"), "05-Aug-26");
    await typeIn(0, await colOf("Every"), "month"); await typeIn(0, await colOf("Last done"), "06-Oct-26");
    check("a monthly obligation last done 6 Oct was next due 5 Nov: 5 days overdue", (await status(0)).includes("Overdue 5 d") && (await rowText(0)).includes("05 Nov 26"), await rowText(0));
    await p.click(".rg td.rh >> nth=0"); await W(100); await p.click(".rgbar button:has-text('Mark done')"); await W(500);
    check("Mark done moves it to the next month", (await rowText(0)).includes("05 Dec 26") && (await status(0)).includes("Open"), await rowText(0));
  });
  await T("dashboard", async () => {
    await tabTo("Dashboard");
    if (process.env.SHOT) await p.screenshot({ path: process.env.SHOT.replace(".png", "-dash.png") });
    const t = await p.textContent(".page");
    check("the Dashboard: revised value, the overdue notice to send and the guarantee to act on", t.includes("$1,120,000") && t.includes("Port closure") && t.includes("Advance payment guarantee") && (await p.$$(".page .attn")).length === 6, t.slice(0, 200));
    await p.click(".attnrow:has-text('Port closure')"); await W(300);
    check("a row in Needs attention opens its tab", (await p.textContent(".regtabs button.on")).startsWith("Notices"));
  });

  check("no page errors", errors.length === 0, errors.slice(0, 3).join(" | "));
  console.log(`\n${passed} passed, ${failed} failed`);
  await b.close(); srv.close(); br.kill();
  process.exit(failed ? 1 : 0);
})();
