// The Workload view: people against weeks or months, by trade (workload.rs). Each cell shows the
// hours booked against the hours the person can work, coloured by how full it is; a person opens to
// the tasks that make up their hours. Drag a task to another person to hand it over, or to a later
// period of the same person to start it then: one change each, worked out by the engine.
import { For, Show, createMemo, createResource, createSignal } from "solid-js";
import { act, call, out, tick, setSel, setView, kept } from "./store";
import { Page } from "./views";
import { Kpi } from "./reports";
import { Seg } from "./ui";
import { dt } from "./regrid";

type WTask = { uid: number; name: string; hours: number[]; total: number; crit: boolean; start: string | null; finish: string | null };
type WPerson = { uid: number; name: string; trade: string; booked: number[]; cap: number[]; over: boolean[]; peak: number; tasks: WTask[] };
type WTrade = { name: string; booked: number[]; cap: number[]; people: WPerson[] };
type WOut = { periods: string[]; unit: string; trades: WTrade[]; over_people: number; over_cells: number; booked: number; free: number };

const [unit, setUnit] = kept("wl-unit", "week");
const [show, setShow] = kept("wl-show", "hours");
const [onlyOver, setOnlyOver] = kept("wl-over", false);
const pct = (b: number, c: number) => (c > 0 ? (b / c) * 100 : b > 0 ? 999 : 0);
/** A cell's colour: free (white) to full (blue), over (red). A person's cell is red when they are over on any day of it (the engine's flag). */
const tone = (b: number, c: number) => { const v = pct(b, c); return v > 100.5 ? "over" : v >= 85 ? "full" : v >= 50 ? "half" : v > 0 ? "low" : ""; };

export const WorkloadView = () => {
  const [data] = createResource(() => [tick(), out()?.tab, unit()] as const, () => call<WOut>("workload", { unit: unit() }));
  const [open, setOpen] = createSignal<Set<number>>(new Set());
  const [drag, setDrag] = createSignal<{ task: WTask; from: WPerson } | null>(null), [over, setOver] = createSignal("");
  const label = (d: string) => unit() === "month" ? new Date(d + "T00:00").toLocaleDateString("en-GB", { month: "short", year: "2-digit" }) : dt(d).slice(0, 6);
  const cell = (b: number, c: number) => show() === "pct" ? (c > 0 ? `${Math.round(pct(b, c))}%` : b ? "∞" : "") : show() === "free" ? (c - b > 0.05 ? (c - b).toFixed(0) : b > c + 0.05 ? `−${(b - c).toFixed(0)}` : "") : b > 0.05 ? b.toFixed(0) : "";
  const trades = createMemo(() => (data()?.trades ?? []).map(t => ({ ...t, people: onlyOver() ? t.people.filter(p => p.over.some(Boolean)) : t.people })).filter(t => t.people.length));
  const toggle = (uid: number) => { const s = new Set(open()); if (s.has(uid)) s.delete(uid); else s.add(uid); setOpen(s); };
  const drop = async (to: WPerson, j: number) => {
    const d = drag(); setDrag(null); setOver("");
    if (!d || !data()) return;
    const first = d.task.hours.findIndex(h => h > 0);
    const later = to.uid === d.from.uid && j > first ? data()!.periods[j] : "";
    if (to.uid === d.from.uid && !later) return;
    await act({ WorkMove: { uid: d.task.uid, from: d.from.uid, to: to.uid === d.from.uid ? 0 : to.uid, start: later } });
  };
  const goTask = (t: WTask) => { setSel([t.uid]); setView("gantt"); };
  return (
    <Page title="Workload" help="workload" tools={<>
      <Seg value={unit()} set={setUnit} options={[["week", "Weeks"], ["month", "Months"]]} />
      <Seg value={show()} set={setShow} options={[["hours", "Hours booked"], ["pct", "% of capacity"], ["free", "Hours free"]]} />
      <label class="chk"><input type="checkbox" checked={onlyOver()} onChange={e => setOnlyOver(e.currentTarget.checked)} /> Only people over capacity</label>
      <button class="btn" onClick={() => setOpen(new Set((data()?.trades ?? []).flatMap(t => t.people.map(p => p.uid))))}>Open all</button>
      <button class="btn" onClick={() => setOpen(new Set<number>())}>Close all</button>
    </>}>
      <Show when={data()}>{d => <>
        <div class="kpis">
          <Kpi v={`${d().over_people}`} l={`people over capacity · in ${d().over_cells} ${unit()}s`} bad={d().over_people > 0} good={d().over_people === 0 && d().booked > 0} />
          <Kpi v={`${Math.round(d().booked).toLocaleString("en-GB")} h`} l="booked" />
          <Kpi v={`${Math.round(d().free).toLocaleString("en-GB")} h`} l="free (capacity not booked)" />
        </div>
        <Show when={d().trades.length} fallback={<p class="empty">No work resources with work yet: add people in the Resource Sheet and assign them to tasks.</p>}>
          <div class="wlwrap">
            <table class="wl">
              <thead><tr><th class="wln">Trade · person · task</th><th class="r" title="Highest share of capacity booked in a period">Peak</th><For each={d().periods}>{p => <th>{label(p)}</th>}</For></tr></thead>
              <tbody><For each={trades()}>{t => <>
                <tr class="wltrade"><td class="wln"><b>{t.name}</b> <span class="muted">· {t.people.length}</span></td><td />
                  <For each={d().periods}>{(_, j) => <td class={`wc ${tone(t.booked[j()], t.cap[j()])}`} title={`${t.booked[j()].toFixed(1)} h of ${t.cap[j()].toFixed(1)} h`}>{cell(t.booked[j()], t.cap[j()])}</td>}</For></tr>
                <For each={t.people}>{p => <>
                  <tr class="wlperson" classList={{ isover: p.over.some(Boolean) }}>
                    <td class="wln"><button class="wltog" onClick={() => toggle(p.uid)}>{open().has(p.uid) ? "▾" : "▸"}</button>{p.name} <span class="muted">· {p.tasks.length}</span></td>
                    <td class="r" classList={{ late: p.peak > 100 }}>{p.peak ? `${Math.round(p.peak)}%` : ""}</td>
                    <For each={d().periods}>{(_, j) => <td class={`wc ${p.over[j()] ? "over" : tone(p.booked[j()], p.cap[j()])}`} classList={{ drop: over() === `${p.uid}|${j()}` }} title={`${p.name}: ${p.booked[j()].toFixed(1)} h of ${p.cap[j()].toFixed(1)} h${p.over[j()] ? " · over capacity on some days" : ""}`}
                      onDragOver={e => { if (drag()) { e.preventDefault(); setOver(`${p.uid}|${j()}`); } }} onDragLeave={() => setOver("")} onDrop={e => { e.preventDefault(); drop(p, j()); }}>{cell(p.booked[j()], p.cap[j()])}</td>}</For>
                  </tr>
                  <Show when={open().has(p.uid)}><For each={p.tasks}>{k => <tr class="wltask" classList={{ crit: k.crit }} draggable={true} onDragStart={e => { setDrag({ task: k, from: p }); e.dataTransfer?.setData("text/plain", String(k.uid)); }} onDragEnd={() => { setDrag(null); setOver(""); }}
                    onDblClick={() => goTask(k)} title={`Drag to another person to hand it over, or to a later ${unit()} of ${p.name} to start it then. Double-click to show it in the Gantt chart.`}>
                    <td class="wln"><span class="wlgrip">⠿</span>{k.name} <span class="muted">{dt(k.start)} – {dt(k.finish)}</span></td><td class="r muted">{k.total.toFixed(0)} h</td>
                    <For each={d().periods}>{(_, j) => <td class="wt">{k.hours[j()] > 0.05 ? <i style={{ opacity: Math.min(1, 0.35 + k.hours[j()] / 40) }}>{k.hours[j()].toFixed(0)}</i> : ""}</td>}</For>
                  </tr>}</For></Show>
                </>}</For>
              </>}</For></tbody>
            </table>
          </div>
          <p class="muted wlkey"><span class="wc low">‎</span> under half <span class="wc half">‎</span> half full <span class="wc full">‎</span> nearly full <span class="wc over">‎</span> over capacity · open a person (▸) and drag a task to another person, or to a later {unit()}.</p>
        </Show>
      </>}</Show>
    </Page>
  );
};
