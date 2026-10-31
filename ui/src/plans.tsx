// Several plans at once: a tab per open plan (above the ribbon), links from a task to tasks in
// other plans, and the Linked Plans panel that shows them and brings their dates up to date.
import { For, Show, createMemo, createResource, createSignal } from "solid-js";
import { Out, fmtAt } from "./api";
import { act, call, out, P, setOut, openMenu, openDialog, closeDialog, toast, useTable, table, tick } from "./store";
import { Dialog, Row } from "./ui";
import { C } from "./cmds";

type ExtLink = { file: string; uid: number; kind: string; lag: number; plan: string; code: string; name: string; start: number; finish: number };
type Target = { tab: number; name: string; path: string | null; tasks: [number, string, string, number, boolean, number, number][] };
type LinkState = { uid: number; task: string; k: number; link: ExtLink; now_start: number | null; now_finish: number | null; state: string };

/** Shows another tab's plan (the view stays as it is). */
export const showTab = async (k: number) => { const o = await call<Out>("switch_tab", { k }); if (o) { setOut(o); useTable(table()); } };
/** Closes a tab, offering to save it first when it has unsaved changes. */
export const closeTab = async (k: number) => {
  const t = out()!.tabs[k];
  const close = async () => { const o = await call<Out>("close_tab", { k }); if (o) { setOut(o); useTable(table()); } };
  if (!t.dirty) return close();
  await showTab(k);
  openDialog(() => (
    <Dialog title="Save changes?" okText="Save" ok={async () => { const o = await call<Out | null>("save", { asNew: false }); if (o) { setOut(o); await close(); } return !!o; }}
      extra={<button class="btn" onClick={() => { closeDialog(); close(); }}>Don't save</button>}>
      <p>Do you want to save the changes to “{t.name}” before closing it?</p>
    </Dialog>
  ));
};
/** Ctrl+Tab / Ctrl+Shift+Tab. */
export const nextTab = (d: number) => { const n = out()?.tabs.length ?? 1; if (n > 1) showTab((out()!.tab + d + n) % n); };
const plusMenu = (e: MouseEvent) => openMenu(e, [
  { label: "New plan…", run: () => import("./store").then(s => s.setBackstage("new")) },
  { label: "Open…", run: C.open },
  "-",
  { label: "Example: a main plan with three linked subplans", run: example },
]);
export const example = async () => {
  const o = await call<Out | null>("linked_example");
  if (o) { setOut(o); useTable(table()); import("./store").then(s => s.setBackstage(null)); toast("Four linked plans were saved in a new folder and opened in tabs: the main schedule, engineering, procurement and construction.", true); }
};

/** The tab strip: one tab per open plan, with unsaved changes marked; + adds one. */
export const PlanTabs = () => (
  <div class="plantabs">
    <For each={out()?.tabs ?? []}>{(t, k) => (
      <div class="ptab" classList={{ on: k() === out()!.tab }} title={t.path ?? "Not saved yet"} onClick={() => k() !== out()!.tab && showTab(k())} onAuxClick={e => { if (e.button === 1) closeTab(k()); }}>
        <span class="nm">{t.name || "Untitled"}</span><Show when={t.dirty}><i class="dot" title="Unsaved changes">•</i></Show>
        <button class="x" title="Close (Ctrl+W)" onClick={e => { e.stopPropagation(); closeTab(k()); }}>×</button>
      </div>
    )}</For>
    <button class="padd" title="New plan, open a plan, or the linked-plans example" onClick={plusMenu}>＋</button>
  </div>
);

const KINDS = ["FS", "SS", "FF", "SF"];
const lagText = (m: number) => (m ? `${m > 0 ? "+" : ""}${Math.round((m / P().day_min) * 10) / 10}d` : "");
const linkText = (x: ExtLink) => `${x.plan} › ${x.code ? `${x.code} ` : ""}${x.name}`;

/** Link the task `uid` to tasks in other open plans: pick a plan, a task, the type and the lag. */
export const linkDialog = (uid: number) => openDialog(() => {
  const [targets] = createResource(() => call<Target[]>("link_targets"));
  const [info] = createResource(() => [tick(), uid], () => call<{ task: { name: string; ext?: ExtLink[] } }>("task_info", { uid }));
  const [links, setLinks] = createSignal<ExtLink[] | null>(null);
  const cur = () => links() ?? info()?.task.ext ?? [];
  const [tab, setTab] = createSignal(-1), [pick, setPick] = createSignal(0), [kind, setKind] = createSignal("FS"), [lag, setLag] = createSignal("0"), [q, setQ] = createSignal("");
  const target = () => targets()?.find(t => t.tab === tab()) ?? targets()?.[0];
  const rows = createMemo(() => (target()?.tasks ?? []).filter(t => !q() || `${t[1]} ${t[2]}`.toLowerCase().includes(q().toLowerCase())));
  const add = () => {
    const t = target(), x = t?.tasks.find(r => r[0] === pick());
    if (!t || !x) return toast("Choose a task in the other plan.");
    if (!t.path) return toast(`Save “${t.name}” first, so the link knows its file.`);
    const lagMin = Math.round((parseFloat(lag()) || 0) * P().day_min);
    setLinks([...cur().filter(l => !(l.file === t.path && l.uid === x[0])), { file: t.path, uid: x[0], kind: kind(), lag: lagMin, plan: t.name, code: x[1], name: x[2], start: x[5], finish: x[6] }]);
  };
  return (
    <Dialog title="Link to Plan" wide help="linkedplans" okText="OK" ok={() => (links() ? act({ ExtSet: { uid, links: links() } }) : undefined)}>
      <p>Predecessors of <b>{info()?.task.name}</b> in other plans. The task waits for them like for a normal link; <b>Update Links</b> keeps their dates current.</p>
      <table class="tbl"><thead><tr><th>Plan › task</th><th>Type</th><th>Last known finish</th><th /></tr></thead><tbody>
        <For each={cur()} fallback={<tr><td colspan="4" class="muted">No links to other plans yet.</td></tr>}>{(x, k) => <tr><td>{linkText(x)}</td><td>{x.kind}{lagText(x.lag)}</td><td>{x.finish ? fmtAt(x.finish - 1) : "—"}</td>
          <td><button class="x" title="Remove this link" onClick={() => setLinks(cur().filter((_, j) => j !== k()))}>×</button></td></tr>}</For>
      </tbody></table>
      <Show when={targets()?.length} fallback={<p class="muted">Open the other plan in a tab first (File › Open, or ＋ beside the tabs), then come back here.</p>}>
        <h4 class="sub">Add a link</h4>
        <div class="form">
          <Row label="Plan"><select onChange={e => { setTab(+e.currentTarget.value); setPick(0); }}><For each={targets()}>{t => <option value={t.tab} selected={t.tab === target()?.tab}>{t.name}{t.path ? "" : " (not saved yet)"}</option>}</For></select></Row>
          <Row label="Find"><input value={q()} placeholder="Part of the task's name or ID" onInput={e => setQ(e.currentTarget.value)} /></Row>
        </div>
        <div class="linkpick"><For each={rows()}>{t => <button classList={{ on: t[0] === pick(), sum: t[4] }} style={{ "padding-left": `${8 + t[3] * 14}px` }} onClick={() => setPick(t[0])} onDblClick={add}>
          <span class="mono">{t[1]}</span><span>{t[2]}</span><span class="muted">{fmtAt(t[6] - 1)}</span></button>}</For></div>
        <div class="form">
          <Row label="Type and lag"><span class="linklag"><select onChange={e => setKind(e.currentTarget.value)}><For each={KINDS}>{k => <option selected={k === kind()}>{k}</option>}</For></select>
            <input type="number" step="0.5" style={{ width: "90px" }} value={lag()} onInput={e => setLag(e.currentTarget.value)} /><span class="muted">days (negative for a lead)</span>
            <button class="btn sm pri" onClick={add}>Add Link</button></span></Row>
        </div>
      </Show>
    </Dialog>
  );
});

/** Brings links up to date (this plan, or every open plan) and says what changed. */
export const updateLinks = async (all: boolean) => {
  const r = await call<[Out, string]>("update_links", { all });
  if (r) { setOut(r[0]); toast(r[1], true); }
  return r;
};
/** Link to Plan for the selected task. */
export const linkSelected = () => { const u = C.ids()[0]; if (u) linkDialog(u); else toast("Select a task first, then link it to a task in another plan."); };

/** The Linked Plans panel: every link of this plan, whether the other plan has moved, and Update Links. */
export const linkedPlans = () => openDialog(() => {
  const [states, { refetch }] = createResource(() => tick(), () => call<LinkState[]>("link_states"));
  const [users] = createResource(() => tick(), () => call<{ uid: number; task: string; tab: number; by: string }[]>("link_users"));
  const update = (all: boolean) => updateLinks(all).then(() => refetch());
  const label = (s: string) => (s === "ok" ? "Up to date" : s === "changed" ? "Moved – update" : `Not found: ${s}`);
  return (
    <Dialog title="Linked Plans" wide help="linkedplans" extra={<><button class="btn" onClick={() => update(false)}>Update This Plan</button><button class="btn pri" onClick={() => update(true)}>Update All Open Plans</button></>}>
      <h4 class="sub"><span class="xin">⇠</span> This plan waits for</h4>
      <p class="muted">Tasks of “{P().name}” that wait for tasks in other plans. When another plan moves, its link shows <b>Moved</b> until you update. Plans that are not open are read from their files.</p>
      <table class="tbl linkstates"><thead><tr><th>Task here</th><th>Waits for</th><th>Type</th><th>Last known finish</th><th>Finish now</th><th>State</th><th /></tr></thead><tbody>
        <For each={states() ?? []} fallback={<tr><td colspan="7" class="muted">No links to other plans. Select a task and use Project › Link to Plan.</td></tr>}>{s => <tr classList={{ warn: s.state !== "ok" }}>
          <td>{s.task}</td><td>{linkText(s.link)}</td><td>{s.link.kind}{lagText(s.link.lag)}</td><td>{s.link.finish ? fmtAt(s.link.finish - 1) : "—"}</td>
          <td>{s.now_finish ? fmtAt(s.now_finish - 1) : "—"}</td><td><span class="pill" classList={{ good: s.state === "ok", bad: s.state !== "ok" }}>{label(s.state)}</span></td>
          <td><button class="btn sm" onClick={() => { closeDialog(); linkDialog(s.uid); }}>Edit…</button></td></tr>}</For>
      </tbody></table>
      <h4 class="sub"><span class="xout">⇢</span> Other open plans wait for</h4>
      <table class="tbl linkstates"><thead><tr><th>Task here</th><th>Waited for by</th><th /></tr></thead><tbody>
        <For each={users() ?? []} fallback={<tr><td colspan="3" class="muted">No other open plan waits for this plan's tasks.</td></tr>}>{u => <tr>
          <td>{u.task}</td><td>{u.by}</td><td><button class="btn sm" onClick={() => { closeDialog(); showTab(u.tab); }}>Show plan</button></td></tr>}</For>
      </tbody></table>
    </Dialog>
  );
});
