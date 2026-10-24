// Title bar, quick access toolbar, Tell Me and the ribbon.
import { For, Show, JSX, createSignal, createMemo, onMount } from "solid-js";
import { out, tab, setTab, setBackstage, view, openMenu, Item, fields, filter, setFilter, ALL, setGroup, group, setSort, sort, useTable, tables, table, setProjSum, projSum, setHideSum, hideSum, baseline, setBaseline, details, timeline, setupOpen, P, act, openDialog, closeDialog, setLevel, setZoom, saveCols, cols, theme, setTheme } from "./store";
import { C, ALL_CMDS } from "./cmds";
import * as D from "./dialogs";
import { Icon } from "./ui";
import { showCrit, setShowCrit, showBase, setShowBase, showSlack, setShowSlack, showLate, setShowLate, showLabels, setShowLabels } from "./gantt";
import { fieldMenu } from "./grid";

const B = (p: { i: string; l: string; run?: (e: MouseEvent) => void; big?: boolean; on?: boolean; off?: boolean; dd?: (e: MouseEvent) => void; title?: string }) => (
  <div class="rbw" classList={{ big: p.big }}>
    <button class="rb" classList={{ big: p.big, on: p.on }} disabled={p.off} title={p.title ?? p.l} onClick={e => (p.run ?? p.dd)?.(e)}><Icon n={p.i} />{p.big ? <span>{p.l}{p.dd && !p.run ? " ▾" : ""}</span> : <span>{p.l}</span>}{!p.big && p.dd && !p.run && <b class="car">▾</b>}</button>
    {p.run && p.dd && <button class="rb split" onClick={e => p.dd!(e)}>▾</button>}
  </div>
);
const G = (p: { l: string; children: JSX.Element }) => <div class="grp"><div class="items">{p.children}</div><div class="cap">{p.l}</div></div>;
const S = (p: { children: JSX.Element }) => <div class="stack">{p.children}</div>;

const views: [string, string, Parameters<typeof C.view>[0]][] = [
  ["Gantt Chart", "gantt", "gantt"], ["Tracking Gantt", "base", "tracking"], ["Network Diagram", "net", "network"], ["Calendar", "cal", "calendar"], ["Task Usage", "table", "taskusage"],
  ["Timeline", "tl", "timeline"], ["Resource Sheet", "res", "ressheet"], ["Resource Usage", "load", "resusage"], ["Resource Graph", "load", "resgraph"], ["Team Planner", "team", "team"],
];
const viewMenu = (): Item[] => views.map(([l, , v]) => ({ label: l, run: () => C.view(v), on: view() === v }));
const filterMenu = (): Item[] => [
  ...fields().filters.map(f => ({ label: f.name, run: () => setFilter(f.rules.length ? f : ALL), on: filter().name === f.name })),
  ...((P().ui?.filters ?? []) as any[]).map(f => ({ label: `${f.name} (saved)`, run: () => setFilter(f), on: filter().name === f.name })),
  "-", { label: "Clear Filter", run: () => setFilter(ALL) }, { label: "New Filter…", run: () => D.filterEditor() },
  { label: "Display Related Summary Tasks", run: () => setFilter({ ...filter(), summaries: !filter().summaries }), on: filter().summaries },
  { label: "Highlight Instead of Filter", run: () => setFilter({ ...filter(), highlight: !filter().highlight }), on: filter().highlight },
];
const GROUPS: [string, string[]][] = [["Critical", ["critical"]], ["Duration", ["dur"]], ["Milestones", ["milestone"]], ["Priority", ["priority"]], ["Resource", ["res"]], ["Status", ["status"]], ["Constraint Type", ["ctype"]], ["Activity Type", ["acttype"]], ["Responsible", ["responsible"]]];
const groupMenu = (): Item[] => [
  { label: "No Group", run: () => setGroup([]), on: !group().length }, ...GROUPS.map(([l, g]) => ({ label: l, run: () => setGroup(g), on: group().join() === g.join() })),
  ...P().codes.map(c => ({ label: c.name, run: () => setGroup([`code:${c.name}`]) })), "-", { label: "Group by Field", sub: fieldMenu(k => setGroup([k])) }, { label: "New Group By…", run: () => D.groupEditor() },
];
const sortMenu = (): Item[] => [
  ...[["ID", "id"], ["Start Date", "start"], ["Finish Date", "finish"], ["Priority", "priority"], ["Cost", "cost"], ["Total Float", "tf"], ["Activity ID", "code"]].map(([l, k]) => ({ label: `by ${l}`, run: () => setSort(k === "id" ? [] : [[k, k === "cost" || k === "priority"]]), on: (sort()[0]?.[0] ?? "id") === k })),
  "-", { label: "Sort by…", run: () => D.sortEditor() },
];
const tableMenu = (): Item[] => [...Object.keys(tables()).map(t => ({ label: t, run: () => useTable(t), on: table() === t })), "-", { label: "More Tables…", run: () => D.tablesDialog() }];
const outlineMenu = (): Item[] => [{ label: "Show Subtasks", run: C.expandAll }, { label: "Hide Subtasks", run: C.collapseAll }, "-", ...[1, 2, 3, 4, 5, 6, 7, 8, 9].map(n => ({ label: `Level ${n}`, run: () => C.outline(n) })), { label: "All Subtasks", run: () => setLevel(0) }];
const zoomMenu = (): Item[] => [
  ...([["Days", 40], ["Weeks", 14], ["Months", 4], ["Quarters", 1.6], ["Years", 0.5]] as [string, number][]).map(([l, z]) => ({ label: l, run: () => setZoom(z) })),
  "-", { label: "Zoom In", run: C.zoomIn }, { label: "Zoom Out", run: C.zoomOut }, { label: "Entire Project", run: C.fit },
];
const exportMenu = (): Item[] => [["Excel workbook (.xlsx)", "xlsx"], ["MS Project XML (.xml)", "xml"], ["Primavera P6 XER (.xer)", "xer"], ["Primavera P6 XML (.pmxml)", "pmxml"], ["MPX (.mpx)", "mpx"], ["CSV (.csv)", "csv"], ["Web page (.html)", "html"], ["iCalendar (.ics)", "ics"], ["JSON (.json)", "json"]].map(([l, k]) => ({ label: l, run: () => C.exportAs(k) }));
const baselineMenu = (): Item[] => Array.from({ length: 11 }, (_, k) => ({ label: k ? `Baseline ${k}` : "Baseline", run: () => { setBaseline(k); setShowBase(true); }, on: baseline() === k && showBase() }));

/** Tell Me / command search (Ctrl+K, Alt+Q). */
export const palette = () => openDialog(() => {
  const [q, setQ] = createSignal(""), [k, setK] = createSignal(0);
  const list = createMemo(() => { const s = q().toLowerCase(); return ALL_CMDS.filter(c => c[0].toLowerCase().includes(s)).slice(0, 14); });
  const go = (i: number) => { const c = list()[i]; if (c) { closeDialog(); setTimeout(() => c[1]()); } };
  let input!: HTMLInputElement;
  onMount(() => input.focus());
  return (
    <div class="ov top" onMouseDown={e => e.target === e.currentTarget && closeDialog()}>
      <div class="pal">
        <input ref={input} placeholder="Tell me what you want to do" value={q()} onInput={e => { setQ(e.currentTarget.value); setK(0); }}
          onKeyDown={e => { if (e.key === "ArrowDown") { e.preventDefault(); setK(Math.min(list().length - 1, k() + 1)); } else if (e.key === "ArrowUp") { e.preventDefault(); setK(Math.max(0, k() - 1)); } else if (e.key === "Enter") { e.preventDefault(); go(k()); } else if (e.key === "Escape") closeDialog(); }} />
        <div class="pl"><For each={list()}>{(c, i) => <button classList={{ on: i() === k() }} onMouseEnter={() => setK(i())} onClick={() => go(i())}><span>{c[0]}</span>{c[2] && <kbd>{c[2]}</kbd>}</button>}</For></div>
      </div>
    </div>
  );
});

export const TitleBar = (p: { update: string | null }) => (
  <header class="top">
    <div class="bar">
      <div class="qat">
        <img src="logo.svg" alt="" class="logo" />
        <button title="Save (Ctrl+S)" onClick={() => C.save()}><Icon n="save" /></button>
        <button title="Undo (Ctrl+Z)" disabled={!out()?.undo} onClick={C.undo}><Icon n="undo" /></button>
        <button title="Redo (Ctrl+Y)" disabled={!out()?.redo} onClick={C.redo}><Icon n="redo" /></button>
        <button title="Print (Ctrl+P)" onClick={C.print}><Icon n="print" /></button>
      </div>
      <div class="title">{out() ? `${out()!.project.name}${out()!.dirty ? " •" : ""} — Datum Plan` : "Datum Plan"}</div>
      <button class="search" onClick={palette} title="Tell me what you want to do (Alt+Q)"><Icon n="search" /><span>Tell me what you want to do</span><kbd>Ctrl+K</kbd></button>
      <Show when={p.update}><button class="upd" onClick={() => setBackstage("about")}>Update {p.update} available</button></Show>
      <button class="theme" title="Theme: Auto follows Windows" onClick={() => setTheme(theme() === "auto" ? "light" : theme() === "light" ? "dark" : "auto")}>Theme · {theme()}</button>
    </div>
    <nav class="tabs">
      <button class="file" onClick={() => setBackstage("home")}>File</button>
      <For each={[["task", "Task"], ["resource", "Resource"], ["report", "Report"], ["project", "Project"], ["view", "View"], ["format", "Format"], ["help", "Help"]]}>{([k, l]) =>
        <button classList={{ on: tab() === k, ctx: k === "format" }} onClick={() => setTab(k)}>{k === "format" ? <><small>Gantt Chart Tools</small> {l}</> : l}</button>}</For>
    </nav>
  </header>
);

export const Ribbon = () => (
  <section class="ribbon">
    <Show when={tab() === "task"}>
      <G l="View"><B big i="gantt" l="Gantt Chart" run={() => C.view("gantt")} dd={e => openMenu(e, viewMenu())} /></G>
      <G l="Clipboard"><B big i="paste" l="Paste" run={C.paste} /><S><B i="cut" l="Cut" run={C.cut} /><B i="copy" l="Copy" run={C.copy} /></S></G>
      <G l="Schedule">
        <div class="pcts"><For each={[0, 25, 50, 75, 100]}>{v => <button class="rb" title={`${v}% complete`} onClick={() => C.pct(v)}><Icon n="pct" /><span>{v}%</span></button>}</For></div>
        <S><B i="track" l="Mark on Track" run={C.onTrack} dd={e => openMenu(e, [{ label: "Mark on Track", run: C.onTrack }, { label: "Update Tasks…", run: C.info }, { label: "Update Project…", run: C.update }])} /><B i="link" l="Link" run={C.link} /><B i="unlink" l="Unlink" run={C.unlink} /></S>
        <S><B i="outdent" l="Outdent" run={C.outdent} /><B i="indent" l="Indent" run={C.indent} /><B i="split" l="Split" run={C.split} /></S>
        <S><B i="up" l="Move Up" run={() => C.move(true)} /><B i="down" l="Move Down" run={() => C.move(false)} /><B i="inact" l="Inactivate" run={C.inactive} /></S>
      </G>
      <G l="Tasks"><B big i="manual" l="Manually Schedule" run={() => C.mode(true)} /><B big i="auto" l="Auto Schedule" run={() => C.mode(false)} /></G>
      <G l="Insert"><B big i="task" l="Task" run={() => C.newTask()} dd={e => openMenu(e, [{ label: "Task", run: () => C.newTask() }, { label: "Blank Row", run: () => C.newTask() }, { label: "Summary Task", run: C.summary }, { label: "Milestone", run: () => C.newTask(true) }])} />
        <S><B i="proj" l="Summary" run={C.summary} /><B i="ms" l="Milestone" run={() => C.newTask(true)} /><B i="del" l="Delete" run={C.del} /></S></G>
      <G l="Properties"><B big i="info" l="Information" run={C.info} /><S><B i="detail" l="Notes" run={C.notes} /><B i="detail" l="Details" run={C.details} on={details()} /><B i="tl" l="Add to Timeline" run={C.timeline} /></S></G>
      <G l="Editing"><S><B i="scroll" l="Scroll to Task" run={C.scrollTo} /><B i="find" l="Find" run={palette} /><B i="fill" l="Inspect" run={() => D.inspect(C.ids()[0])} /></S></G>
    </Show>
    <Show when={tab() === "resource"}>
      <G l="View"><B big i="res" l="Resource Sheet" run={() => C.view("ressheet")} dd={e => openMenu(e, viewMenu())} /><B big i="team" l="Team Planner" run={() => C.view("team")} /></G>
      <G l="Assignments"><B big i="res" l="Assign Resources" run={() => C.info()} /><S><B i="res" l="Resource Pool" run={() => C.view("ressheet")} /><B i="load" l="Resource Usage" run={() => C.view("resusage")} /><B i="load" l="Resource Graph" run={() => C.view("resgraph")} /></S></G>
      <G l="Insert"><B big i="task" l="Add Resource" run={() => act({ ResInsert: { name: `Resource ${P().resources.length + 1}` } }).then(() => C.view("ressheet"))} /></G>
      <G l="Level"><B big i="level" l="Level All" run={() => C.level()} /><S><B i="level" l="Level Selection" run={() => C.level(false)} /><B i="del" l="Clear Leveling" run={C.clearLevel} /><B i="opts" l="Leveling Options" run={() => D.scheduleOptions()} /></S></G>
    </Show>
    <Show when={tab() === "report"}>
      <G l="View Reports"><B big i="dash" l="Dashboard" run={() => C.view("dashboard")} /><B big i="track" l="Progress Report" run={() => C.view("progress")} /><B big i="ev" l="S-Curves" run={() => C.view("scurve")} /><B big i="report" l="Reports" run={() => C.view("reports")} /></G>
      <G l="Health"><B big i="check" l="Check Schedule" run={() => C.view("checks")} /><B big i="risk" l="Risks" run={() => C.view("risks")} /></G>
      <G l="Portfolio"><B big i="folder" l="Portfolio" run={() => C.view("portfolio")} /><B big i="copy" l="Compare Projects" run={() => D.compare()} /></G>
      <G l="Export"><B big i="print" l="Print / PDF" run={C.print} /><B big i="export" l="Export" dd={e => openMenu(e, exportMenu())} /></G>
    </Show>
    <Show when={tab() === "project"}>
      <G l="Properties"><B big i="proj" l="Project Information" run={() => D.projectInfo()} /><S><B i="cols" l="Custom Fields" run={() => D.customFields()} /><B i="group" l="Activity Codes" run={() => D.activityCodes()} /><B i="calendar2" l="Change Working Time" run={() => D.calendars()} /></S>
        <S><B i="detail" l="Project Setup" run={C.setup} on={setupOpen()} /><B i="opts" l="Schedule Options" run={() => D.scheduleOptions()} /></S></G>
      <G l="Schedule"><B big i="base" l="Set Baseline" run={C.baseline} /><S><B i="track" l="Update Project" run={C.update} /><B i="calendar2" l="Status Date" run={() => D.projectInfo()} /><B i="report" l="Store Snapshot" run={() => D.snapshot()} /></S><S><B i="calendar2" l="Reporting Periods" run={() => D.periodSettings()} /></S></G>
      <G l="Status"><S><B i="crit" l="Critical Path" run={() => { setShowCrit(true); setFilter(fields().filters.find(f => f.name === "Critical") ?? ALL); }} /><B i="crit" l="Longest Path" run={() => setFilter(fields().filters.find(f => f.name === "Longest Path") ?? ALL)} /><B i="check" l="Check Schedule" run={() => C.view("checks")} /></S></G>
      <G l="Proofing"><B big i="copy" l="Compare Projects" run={() => D.compare()} /></G>
    </Show>
    <Show when={tab() === "view"}>
      <G l="Task Views"><B big i="gantt" l="Gantt Chart" run={() => C.view("gantt")} dd={e => openMenu(e, viewMenu())} /><S><B i="base" l="Tracking Gantt" run={() => C.view("tracking")} /><B i="net" l="Network Diagram" run={() => C.view("network")} /><B i="cal" l="Calendar" run={() => C.view("calendar")} /></S><S><B i="table" l="Task Usage" run={() => C.view("taskusage")} /><B i="tl" l="Timeline" run={C.timelineBar} on={timeline()} /></S></G>
      <G l="Resource Views"><S><B i="res" l="Resource Sheet" run={() => C.view("ressheet")} /><B i="load" l="Resource Usage" run={() => C.view("resusage")} /><B i="team" l="Team Planner" run={() => C.view("team")} /></S></G>
      <G l="Data">
        <S><B i="sort" l={`Sort${sort().length ? ": on" : ""}`} dd={e => openMenu(e, sortMenu())} /><B i="list" l="Outline" dd={e => openMenu(e, outlineMenu())} /><B i="table" l={`Tables: ${table()}`} dd={e => openMenu(e, tableMenu())} /></S>
        <S><B i="filter" l={`Filter: ${filter().name || "Custom"}`} dd={e => openMenu(e, filterMenu())} on={filter().rules.length > 0} /><B i="filter" l="Highlight" dd={e => openMenu(e, filterMenu().map(x => x === "-" ? x : { ...x, run: () => { x.run?.(); setFilter({ ...filter(), highlight: true }); } }))} /><B i="group" l={`Group by: ${group().length ? group().join(", ") : "None"}`} dd={e => openMenu(e, groupMenu())} on={group().length > 0} /></S>
      </G>
      <G l="Zoom"><B big i="zin" l="Timescale" dd={e => openMenu(e, zoomMenu())} /><S><B i="zin" l="Zoom In" run={C.zoomIn} /><B i="zout" l="Zoom Out" run={C.zoomOut} /><B i="fit" l="Entire Project" run={C.fit} /></S></G>
      <G l="Split View"><S><B i="detail" l="Details" run={C.details} on={details()} /><B i="tl" l="Timeline" run={C.timelineBar} on={timeline()} /></S></G>
    </Show>
    <Show when={tab() === "format"}>
      <G l="Columns"><B big i="cols" l="Formula Column" run={() => D.formulaColumn()} /><B big i="cols" l="Insert Column" dd={e => openMenu(e, fieldMenu(k => saveCols([...cols(), [k, 90]])))} /><S><B i="table" l="Column Settings" run={() => D.fieldSettings(0)} /><B i="cols" l="Custom Fields" run={() => D.customFields()} /></S></G>
      <G l="Bar Styles"><S><label class="chk"><input type="checkbox" checked={showCrit()} onChange={e => setShowCrit(e.currentTarget.checked)} />Critical Tasks</label><label class="chk"><input type="checkbox" checked={showSlack()} onChange={e => setShowSlack(e.currentTarget.checked)} />Slack</label><label class="chk"><input type="checkbox" checked={showLate()} onChange={e => setShowLate(e.currentTarget.checked)} />Late Tasks</label></S>
        <B big i="base" l="Baseline" dd={e => openMenu(e, baselineMenu())} on={showBase()} /></G>
      <G l="Show/Hide"><S><label class="chk"><input type="checkbox" checked={projSum()} onChange={e => setProjSum(e.currentTarget.checked)} />Project Summary Task</label><label class="chk"><input type="checkbox" checked={!hideSum()} onChange={e => setHideSum(!e.currentTarget.checked)} />Summary Tasks</label><label class="chk"><input type="checkbox" checked={showLabels()} onChange={e => setShowLabels(e.currentTarget.checked)} />Bar Labels</label></S></G>
    </Show>
    <Show when={tab() === "help"}>
      <G l="Help"><B big i="keys" l="Shortcuts" run={() => D.shortcuts()} /><B big i="upd" l="Check for Updates" run={() => setBackstage("about")} /><B big i="search" l="Tell Me" run={palette} /></G>
    </Show>
  </section>
);
