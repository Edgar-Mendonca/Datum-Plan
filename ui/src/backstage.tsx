// File (backstage): home with templates and recent plans, export, print, options, updates.
import { For, Show, createSignal, createResource, onMount } from "solid-js";
import { Currency, Options, Out } from "./api";
import { backstage, setBackstage, call, loadOut, out, setOut, opts, setOpts, toast, setSetupOpen, act, setThemeRaw, applyTheme } from "./store";
import { C } from "./cmds";
import * as D from "./dialogs";
import { Icon, Row, Text, Pick, Check } from "./ui";
import { listen, invoke } from "./api";

type Tpl = { id: string; name: string; desc: string; color: string; tasks: number; days: number; bars: [number, number, number][] };
type Info = { current: string; latest: string | null; beta: boolean; available: boolean; notes: string; published: string; url: string | null; sums: string | null; size: number; page: string };
export const [update, setUpdate] = createSignal<Info | null>(null);
export const checkUpdate = async (quiet: boolean) => { try { setUpdate(await invoke<Info>("check_update")); } catch (e) { if (!quiet) toast(String(e)); } };

const CURRENCIES: [string, string, number][] = [
  ["USD", "$", 2], ["EUR", "€", 2], ["GBP", "£", 2], ["INR", "₹", 2], ["AED", "AED", 2], ["SAR", "SAR", 2], ["QAR", "QAR", 2], ["KWD", "KWD", 3], ["OMR", "OMR", 3], ["BHD", "BHD", 3],
  ["JPY", "¥", 0], ["CNY", "¥", 2], ["AUD", "A$", 2], ["CAD", "C$", 2], ["CHF", "CHF", 2], ["SGD", "S$", 2], ["HKD", "HK$", 2], ["NZD", "NZ$", 2], ["ZAR", "R", 2], ["BRL", "R$", 2],
  ["MXN", "MX$", 2], ["SEK", "kr", 2], ["NOK", "kr", 2], ["DKK", "kr", 2], ["PLN", "zł", 2], ["CZK", "Kč", 2], ["HUF", "Ft", 0], ["TRY", "₺", 2], ["RUB", "₽", 2], ["KRW", "₩", 0],
  ["IDR", "Rp", 0], ["MYR", "RM", 2], ["THB", "฿", 2], ["PHP", "₱", 2], ["VND", "₫", 0], ["PKR", "Rs", 2], ["BDT", "৳", 2], ["LKR", "Rs", 2], ["NGN", "₦", 2], ["KES", "KSh", 2],
  ["EGP", "E£", 2], ["MAD", "MAD", 2], ["ILS", "₪", 2], ["CLP", "CLP$", 0], ["COP", "COL$", 0], ["PEN", "S/", 2], ["ARS", "AR$", 2], ["UAH", "₴", 2], ["RON", "lei", 2], ["ISK", "kr", 0],
];
export const currencyFor = (code: string): Currency => { const c = CURRENCIES.find(x => x[0] === code.toUpperCase()); return { code: code.toUpperCase(), symbol: c?.[1] ?? code.toUpperCase(), decimals: c?.[2] ?? 2, after: !!c && c[1].length > 1 && /^[A-Z]{3}$/.test(c[1]) }; };
export const CurrencyPick = (p: { value: Currency; set: (c: Currency) => void }) => <>
  <input list="iso4217" value={p.value.code} onChange={e => p.set(currencyFor(e.currentTarget.value))} style={{ width: "90px" }} />
  <datalist id="iso4217"><For each={CURRENCIES}>{c => <option value={c[0]}>{c[1]}</option>}</For></datalist>
  <span class="muted"> symbol </span><input value={p.value.symbol} style={{ width: "60px" }} onChange={e => p.set({ ...p.value, symbol: e.currentTarget.value })} />
  <span class="muted"> decimals </span><input type="number" min="0" max="4" value={p.value.decimals} style={{ width: "50px" }} onChange={e => p.set({ ...p.value, decimals: +e.currentTarget.value })} />
  <label class="chk"><input type="checkbox" checked={p.value.after} onChange={e => p.set({ ...p.value, after: e.currentTarget.checked })} />after amount</label>
</>;
/** Release notes: headings, bullets and bold, enough for the changelog. */
const md = (s: string) => s.split("\n").map(l => {
  const e = l.replace(/[&<>]/g, c => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;" })[c]!).replace(/\*\*(.+?)\*\*/g, "<b>$1</b>").replace(/`(.+?)`/g, "<code>$1</code>");
  return l.startsWith("## ") ? `<h3>${e.slice(3)}</h3>` : l.startsWith("- ") ? `<li>${e.slice(2)}</li>` : e.trim() ? `<p>${e}</p>` : "";
}).join("");

const Mini = (p: { t: Tpl }) => (
  <svg viewBox="0 0 100 60" preserveAspectRatio="none">
    <For each={p.t.bars.slice(0, 14)}>{([k, a, b], i) => k === 2 ? <path d={`M${a * 92 + 4} ${i() * 4 + 3}l2 2l-2 2l-2 -2z`} fill="#F2B233" /> : <rect x={a * 92 + 4} y={i() * 4 + 2} width={Math.max(1.5, (b - a) * 92)} height={k ? 1.6 : 2.6} fill={k ? "#14325E" : p.t.color} />}</For>
  </svg>
);

const Home = () => {
  const [tpls] = createResource(() => call<Tpl[]>("templates"));
  const [recent] = createResource(() => call<{ path: string; name: string; modified: string }[]>("recent"));
  const neu = (template: string | null) => loadOut("new_plan", { template }).then(o => { if (o && !o.project.setup_done) setSetupOpen(true); });
  return (
    <div class="home">
      <div>
        <h1>Good {new Date().getHours() < 12 ? "morning" : new Date().getHours() < 18 ? "afternoon" : "evening"}</h1>
        <div class="tiles">
          <button class="tile" onClick={() => neu(null)}><div class="pic"><svg class="ic"><use href="#i-new" /></svg></div><b>Blank Project</b><span>Start with the Project Setup panel.</span></button>
          <For each={tpls() ?? []}>{t => <button class="tile" onClick={() => neu(t.id)}><div class="pic"><Mini t={t} /><small>{t.tasks} tasks · {t.days} days</small></div><b>{t.name}</b><span>{t.desc}</span></button>}</For>
          <button class="tile" onClick={() => C.importFile()}><div class="pic"><svg class="ic"><use href="#i-import" /></svg></div><b>Import</b><span>MS Project XML, Primavera XER, Excel, CSV or JSON.</span></button>
        </div>
        <h2>Recent</h2>
        <table class="recent"><tbody>
          <For each={recent() ?? []} fallback={<tr><td class="muted">Plans you open or save appear here.</td></tr>}>{r => <tr class="f" onClick={() => loadOut("open", { path: r.path })}><td><b>{r.name}</b><div class="p">{r.path}</div></td><td class="p">{r.modified}</td></tr>}</For>
        </tbody></table>
      </div>
      <UpdateCard />
    </div>
  );
};

const UpdateCard = () => {
  const [pct, setPct] = createSignal<number | null>(null);
  const [ver] = createResource(() => call<string>("version"));
  onMount(() => { listen("update-progress", (p: number) => setPct(p)); if (!update()) checkUpdate(true); });
  const install = async () => { const u = update(); if (!u?.url) return; setPct(0); await call("install_update", { url: u.url, sums: u.sums }); setPct(null); };
  return (
    <div class="card">
      <div class="brand"><img src="logo.svg" alt="" /><div><b>Datum Plan</b><span class="muted">Version {ver()}{opts().beta ? " · beta channel" : ""}</span></div></div>
      <Show when={update()} fallback={<><span class="pill">Checking for updates…</span><button class="btn wide" onClick={() => checkUpdate(false)}>Check for updates</button></>}>{u => <>
        <Show when={u().available} fallback={<span class="pill ok">You have the latest version</span>}><span class="pill warn">Version {u().latest}{u().beta ? " (beta)" : ""} is ready</span></Show>
        <Show when={u().available && u().url}><button class="btn pri wide" disabled={pct() !== null} onClick={install}>{pct() === null ? `Update now (${Math.round(u().size / 1048576)} MB)` : `Downloading… ${pct()}%`}</button></Show>
        <Show when={pct() !== null}><div class="prog"><i style={{ width: `${pct()}%` }} /></div></Show>
        <button class="btn wide" onClick={() => checkUpdate(false)}>Check again</button>
        <div class="notes"><h4>What's new{u().latest ? ` in ${u().latest}` : ""}</h4><div innerHTML={md(u().notes || "Release notes will appear here.")} /></div>
      </>}</Show>
    </div>
  );
};

const Export = () => (
  <div><h1>Export</h1>
    <div class="tiles">
      <For each={[["xlsx", "Excel workbook", "The current table's columns, with outline indent."], ["xml", "MS Project XML", "Open in MS Project, ProjectLibre and others."], ["xer", "Primavera P6 XER", "Import into P6 Professional or EPPM."],
        ["pmxml", "Primavera P6 XML", "P6 XML with WBS, activities, relationships and resources."], ["mpx", "MPX", "The classic exchange format."], ["csv", "CSV", "Plain text, the current table's columns."],
        ["html", "Web page", "A branded report of the current table."], ["ics", "iCalendar", "Tasks and milestones as calendar events."], ["json", "JSON", "For scripts and other tools."]]}>{([k, n, d]) =>
        <button class="tile" onClick={() => C.exportAs(k)}><div class="pic"><svg class="ic"><use href="#i-export" /></svg><small>.{k}</small></div><b>{n}</b><span>{d}</span></button>}</For>
    </div>
    <h2>Pictures and PDF</h2>
    <div class="tiles"><button class="tile" onClick={() => { setBackstage(null); C.print(); }}><div class="pic"><svg class="ic"><use href="#i-print" /></svg></div><b>Print or save as PDF</b><span>Choose “Save as PDF” as the printer to make a PDF of the current view.</span></button>
      <button class="tile" onClick={() => { setBackstage(null); copyPicture(); }}><div class="pic"><svg class="ic"><use href="#i-copy" /></svg></div><b>Save picture (PNG)</b><span>A picture of the Gantt chart as shown.</span></button></div>
  </div>
);
const copyPicture = async () => {
  const cv = document.querySelector(".chart canvas.bars") as HTMLCanvasElement | null, sc = document.querySelector(".chart canvas.scale") as HTMLCanvasElement | null;
  if (!cv || !sc) return toast("Open the Gantt chart first.");
  const out = document.createElement("canvas"); out.width = cv.width; out.height = cv.height + sc.height;
  const ctx = out.getContext("2d")!; ctx.fillStyle = "#fff"; ctx.fillRect(0, 0, out.width, out.height); ctx.drawImage(sc, 0, 0); ctx.drawImage(cv, 0, sc.height);
  const path = await call<string | null>("save_file", { name: "Gantt chart.png", data: out.toDataURL("image/png") });
  if (path) toast(`Saved ${path}`, true);
};

const OptionsPage = () => {
  const [o, setO] = createSignal<Options>({ ...opts() });
  const save = async () => { const r = await call<Out>("set_options", { opts: o() }); if (r) { setOpts(o()); setOut(r); toast("Options saved.", true); } };
  const put = <K extends keyof Options>(k: K, v: Options[K]) => setO({ ...o(), [k]: v });
  return (
    <div><h1>Options</h1>
      <div class="form wideform">
        <h4>General</h4><span />
        <Row label="Date format"><Pick value={o().dateFmt} set={v => put("dateFmt", v)} options={[["%d-%b-%y", "24-Oct-26"], ["%d/%m/%Y", "24/10/2026"], ["%m/%d/%Y", "10/24/2026"], ["%Y-%m-%d", "2026-10-24"], ["%d %b %Y", "24 Oct 2026"], ["%a %d-%b-%y", "Sat 24-Oct-26"]]} /></Row>
        <Row label="Theme"><Pick value={o().theme || "auto"} set={v => { put("theme", v); setThemeRaw(v); applyTheme(); }} options={[["auto", "Auto (follow Windows)"], ["light", "Light"], ["dark", "Dark"]]} /></Row>
        <Row label="Recovery save every"><span><input type="number" min="0" style={{ width: "60px" }} value={o().autosave} onChange={e => put("autosave", +e.currentTarget.value)} /> minutes (0 = off)</span></Row>
        <Row label=""><Check label="Show the Project Setup panel for new plans" value={o().setupPanel} set={v => put("setupPanel", v)} /></Row>
        <h4>Currency for new plans</h4><span />
        <Row label="Currency"><span><CurrencyPick value={o().currency} set={c => put("currency", c)} /></span></Row>
        <Show when={out()}><Row label="This plan"><span><CurrencyPick value={out()!.project.currency} set={c => act({ ProjectSet: { field: "currency", value: c } })} /></span></Row></Show>
        <h4>Report branding</h4><span />
        <Row label="Company"><Text value={o().company} set={v => put("company", v)} /></Row>
        <Row label="Logo"><span>{o().logo && <img src={o().logo} class="logoprev" />}<button class="btn sm" onClick={async () => { const l = await call<string | null>("pick_logo"); if (l) put("logo", l); }}>Choose picture…</button>{o().logo && <button class="btn sm" onClick={() => put("logo", "")}>Remove</button>}
          <Show when={out()}><button class="btn sm" onClick={() => act({ Batch: [{ ProjectSet: { field: "logo", value: o().logo } }, { ProjectSet: { field: "company", value: o().company } }] })}>Use for this plan</button></Show></span></Row>
        <h4>Updates</h4><span />
        <Row label=""><Check label="Get beta versions (early access, may have bugs)" value={o().beta} set={v => put("beta", v)} /></Row>
        <Row label="Workspace folder"><span class="muted">{o().workspace || "Choose in Report › Portfolio"}</span></Row>
      </div>
      <p><button class="btn pri" onClick={save}>Save options</button></p>
      <p class="muted">Datum Plan keeps everything on this computer. The only connection it makes is the update check to GitHub.</p>
    </div>
  );
};

export const Backstage = () => (
  <div class="bs">
    <aside>
      <button class="back" onClick={() => out() && setBackstage(null)} title="Back">←</button>
      <For each={[["home", "home", "Home"], ["new", "new", "New"], ["open", "open", "Open"], ["import", "import", "Import"], ["save", "save", "Save"], ["saveas", "save", "Save As"], ["export", "export", "Export"], ["print", "print", "Print"]]}>{([k, i, l]) =>
        <button classList={{ on: backstage() === k }} onClick={() => {
          if (k === "open") C.open(); else if (k === "import") C.importFile(); else if (k === "save") C.save(); else if (k === "saveas") C.save(true);
          else if (k === "print") { setBackstage(null); C.print(); } else setBackstage(k === "new" ? "home" : k);
        }}><Icon n={i} size={22} />{l}</button>}</For>
      <span class="grow" />
      <button classList={{ on: backstage() === "options" }} onClick={() => setBackstage("options")}><Icon n="opts" size={22} />Options</button>
      <button classList={{ on: backstage() === "about" }} onClick={() => setBackstage("about")}><Icon n="upd" size={22} />Updates</button>
    </aside>
    <section class="bsmain">
      <Show when={backstage() === "home"}><Home /></Show>
      <Show when={backstage() === "export"}><Export /></Show>
      <Show when={backstage() === "options"}><OptionsPage /></Show>
      <Show when={backstage() === "about"}><div class="about"><h1>Updates</h1><UpdateCard /><p><button class="btn" onClick={() => D.shortcuts()}>Keyboard shortcuts</button></p></div></Show>
    </section>
  </div>
);
