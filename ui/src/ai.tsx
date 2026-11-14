// AI Link (Home › AI Link): Claude works on the plan open in this window. The engine has the tools
// (crates/core/src/assist.rs); the window switches the link on, sets up Claude Desktop, and shows
// Claude's changes as they happen (each one an Undo step).
import { Show, createSignal, createEffect, createRoot } from "solid-js";
import { ALL, call, filter, group, level, openDialog, opts, refresh, sel, setBaseline, setCollapsed, setDetails, setDetailsTab, setFilter, setGroup, setLevel, setOpts, setOut, setSort, setTimeline, sort, table, toast, useTable, view, View } from "./store";
import { C, selectUid } from "./cmds";
import { setShowBase, setShowCrit, setShowLabels, setShowLate, setShowSlack } from "./gantt";
import { setPathOn } from "./path";
import { Dialog } from "./ui";
import { Filter, Out, invoke as _i, listen } from "./api";

type Info = { on: boolean; exe: string; claude_config: string | null; claude_set: boolean; snippet: string };
export const [aiOn, setAiOn] = createSignal(false);

/** What Claude just did, in words for the note at the bottom. */
const DID: Record<string, string> = {
  add_tasks: "added tasks", set_task_fields: "changed a task", link_tasks: "linked tasks", unlink_tasks: "removed a link", delete_tasks: "deleted tasks",
  update_progress: "recorded progress", set_status_date: "set the status date", set_baseline: "saved a baseline", set_sheet_cells: "typed in a sheet",
  add_resources: "added resources", set_resource_fields: "changed a resource", delete_resources: "deleted resources", assign_resource: "assigned a resource", unassign_resource: "removed a resource from tasks",
  add_calendar: "added a calendar", set_calendar: "changed a calendar", delete_calendar: "deleted a calendar", define_code: "changed an activity code", set_task_codes: "coded tasks",
  define_custom_field: "set up a custom field", delete_custom_field: "removed a custom field", set_constraint: "set a constraint", set_deadline: "set a deadline",
  indent_tasks: "indented tasks", outdent_tasks: "outdented tasks", move_tasks: "moved tasks", set_task_notes: "wrote a note", set_columns: "changed the task sheet's columns",
  set_risk: "changed the risk register", delete_risk: "removed a risk", set_delay_event: "changed a delay event", delete_delay_event: "removed a delay event", add_fragnet: "added a delay event's activities",
  take_snapshot: "took a snapshot", set_health_settings: "changed the health settings", level_resources: "levelled resources", clear_levelling: "cleared levelling",
  update_project: "updated progress through a date", mark_on_track: "marked tasks on track", split_task: "split a task", save_interim: "saved an interim plan", set_constraint_log_item: "changed the constraint log",
  set_contract: "changed the contract", set_contract_date: "changed a contract date", set_payment: "changed a payment milestone", set_vendor_document: "changed the document register",
  log_submission: "logged a document submission", set_order: "changed an order", log_expediting: "added an expediting note", clear_baseline: "cleared a baseline",
  add_sheet: "added a sheet", rename_sheet: "renamed a sheet", delete_sheet: "deleted a sheet", format_sheet_cells: "formatted a sheet", set_number_format: "set a number format",
  define_name: "changed a defined name", format_tasks: "formatted tasks", add_dashboard: "added a dashboard", delete_dashboard: "deleted a dashboard", export_plan: "saved a copy to Downloads", export_register: "saved a register to Downloads",
  set_project_info: "changed the project details", add_recurring_task: "added a recurring task", apply_credit_rule: "gave tasks a rule of credit", set_task_steps: "set a task's steps", mark_step_done: "ticked a step",
  new_plan: "opened a new plan", open_plan: "opened a plan", save_plan_as: "saved the plan as a new file", close_plan: "closed a plan",
  set_project_settings: "changed the project settings", set_schedule_options: "changed the scheduling options", set_reporting_periods: "changed the reporting periods",
  set_baseline_lock: "locked or unlocked the baseline", rename_column: "renamed a column", set_percent_decimals: "changed the percent decimals", set_modules: "switched registers on or off",
  set_bar_colours: "changed the bar colours", set_conditional_formatting: "changed the conditional formatting", set_period_notes: "wrote report notes", set_task_expenses: "set a task's expenses",
  set_task_documents: "set a task's documents", set_task_notebook: "wrote in a task's notebook", set_resource_rates: "set a resource's rates", set_resource_availability: "set a resource's availability",
  set_resource_leave: "changed a resource's leave", define_credit_rule: "changed a rule of credit", find_replace: "replaced text in tasks",
  insert_rows_columns: "inserted rows or columns in a sheet", delete_rows_columns: "deleted rows or columns in a sheet", shift_cells: "moved cells in a sheet", move_sheet: "moved a sheet",
  merge_cells: "merged or unmerged cells", freeze_panes: "froze panes in a sheet", set_sheet_conditional_formatting: "changed a sheet's conditional formatting", set_cell_notes: "wrote cell notes",
  hide_rows_columns: "hid or showed rows or columns", set_sheet_tab: "changed a sheet tab", set_row_heights: "changed row heights", sort_range: "sorted a range", remove_duplicates: "removed duplicate rows",
  place_picture: "placed or removed a picture",
  link_to_plan: "linked a task to another plan", update_links: "updated the links between plans", new_subplan: "made a new subplan", add_subplan: "added a subplan", remove_subplan: "took a subplan out of the project",
  open_subplans: "opened the subplans", apply_setup_to_subplans: "gave the master's setup to its subplans", new_scenario: "made a scenario", merge_scenario: "merged a scenario into the plan",
  set_pool_plan: "chose the resource pool", take_from_pool: "took the pool's resources", import_plan: "imported a plan",
  apply_operation: "changed the plan", undo: "undid a change", redo: "redid a change", switch_plan: "showed another plan", save_plan: "saved the plan",
};

/** At start: the link's state, and Claude's changes shown as they come. */
export const initAi = async () => {
  const i = await call<Info>("ai_info");
  if (i) setAiOn(i.on);
  listen("ai-change", async (p: { out: Out; tool: string }) => {
    setOut(p.out);
    await refresh();
    if (p.tool === "set_columns" || p.tool === "undo" || p.tool === "redo") useTable(table());
    toast(`AI Link: the assistant ${DID[p.tool] ?? "changed the plan"}. Undo (Ctrl+Z) takes it back.`, true);
  });
  // What the assistant asks the window to show (it changes the view only, never the plan).
  listen("ai-ui", async (p: { action: string; a: any }) => {
    const a = p.a, later = (f: () => void) => setTimeout(f, 250);
    switch (p.action) {
      case "view": C.view(a as View); break;
      case "table": useTable(a); break;
      case "filter": setFilter((a as Filter).rules.length ? a : ALL); break;
      case "group": setGroup(a); break;
      case "sort": setSort(a); break;
      case "level": setLevel(a); break;
      case "collapse": setCollapsed(a); break;
      case "select": {
        if (!["gantt", "tracking", "taskusage", "network", "calendar"].includes(view())) C.view("gantt");
        selectUid(a);
        // Hidden by a filter, an outline level or a collapsed summary: show all, then select.
        if (!sel().includes(a)) { setFilter(ALL); setLevel(0); setCollapsed([]); await refresh(); later(() => { selectUid(a); C.scrollTo(); }); } else later(() => C.scrollTo());
        break;
      }
      case "zoom": a === "in" ? C.zoomIn() : a === "out" ? C.zoomOut() : C.fit(); break;
      case "gantt": {
        if (a.critical !== undefined) setShowCrit(!!a.critical);
        if (a.slack !== undefined) setShowSlack(!!a.slack);
        if (a.late !== undefined) setShowLate(!!a.late);
        if (a.labels !== undefined) setShowLabels(!!a.labels);
        if (a.baseline_bars !== undefined) setShowBase(!!a.baseline_bars);
        if (typeof a.baseline === "number") { setBaseline(a.baseline); setShowBase(true); }
        break;
      }
      case "path": setPathOn(!!a.on); if (a.uid) selectUid(a.uid); break;
      case "panes": {
        if (typeof a.details === "boolean") setDetails(a.details);
        if (a.tab) { setDetails(true); setDetailsTab(a.tab); }
        if (typeof a.timeline === "boolean") setTimeline(a.timeline);
        break;
      }
    }
  });
  // The window tells the AI Link what it shows, for the assistant's get_view.
  createRoot(() => createEffect(() => {
    const s = { view: view(), table: table(), filter: filter().name, filter_rules: filter().rules, group: group(), sort: sort(), outline_level: level(), selected: sel().slice(0, 50) };
    _i("ai_view_state", { state: s }).catch(() => {});
  }));
};

export const aiLink = () => openDialog(() => {
  const [i, setI] = createSignal<Info | null>(null);
  const [busy, setBusy] = createSignal(false);
  const load = async () => { const x = await call<Info>("ai_info"); if (x) { setI(x); setAiOn(x.on); } };
  const turn = async (on: boolean) => { setBusy(true); const x = await call<Info>("ai_link", { on }); setBusy(false); if (x) { setI(x); setAiOn(x.on); setOpts({ ...opts(), aiLink: x.on }); toast(on ? "AI Link on: your AI assistant can work on the plan you have open." : "AI Link off.", true); } };
  const [added, setAdded] = createSignal<{ file: string; opened: boolean } | null>(null);
  const add = async () => { setBusy(true); const x = await call<{ file: string; opened: boolean }>("ai_add_to_claude"); setBusy(false); if (x) { setAdded(x); toast(x.opened ? "Claude Desktop is asking to install Datum Plan: click Install." : "Datum Plan.mcpb is in Downloads: drag it onto Claude Desktop › Settings › Extensions and click Install.", true); } };
  const setup = async () => { const f = await call<string>("ai_setup_claude"); if (f) { toast(`Claude Desktop knows Datum Plan now (${f}). Restart Claude Desktop once.`, true); load(); } };
  const copy = (t: string, what: string) => navigator.clipboard?.writeText(t).then(() => toast(`${what} copied.`, true));
  load();
  return (
    <Dialog title="AI Link" help="ailink" cancel="Close">
      <div class="ailink">
        <p>Ask your AI assistant to read or change the plan you have open here: <i>“Add a 3-day pump test after task 24 and link handover to it”</i>, <i>“Which tasks are late and why?”</i>. You see each change at once, and <b>Undo</b> takes it back. The AI Link works with Claude Desktop (and Claude Code) through MCP, with your own account: no API key is needed.</p>
        <Show when={i()}>{x => <>
          <div class="ailink-step" classList={{ done: x().on }}>
            <b>1</b><div><div class="t">Switch the AI Link on</div><div class="muted">Only an assistant on this computer can reach it, with a key that changes each time. Datum Plan never saves your file unless you ask.</div></div>
            <button class="btn" classList={{ pri: !x().on }} disabled={busy()} onClick={() => turn(!x().on)}>{x().on ? "Turn off" : "Turn on"}</button>
          </div>
          <div class="ailink-step" classList={{ done: !!added() || x().claude_set }}>
            <b>2</b><div><div class="t">Add Datum Plan to Claude Desktop</div><div class="muted">Once. Datum Plan saves <b>Datum Plan.mcpb</b> in your Downloads folder and shows it. Open Claude Desktop › Settings › Extensions, drag the file onto that page and click <b>Install</b>. Nothing to edit.
              <Show when={added()}>{a => <> {a().opened ? "If Claude Desktop opened by itself, just click Install." : <>The file: {a().file}</>}</>}</Show></div></div>
            <button class="btn" classList={{ pri: x().on && !added() }} disabled={busy()} onClick={add}>Add to Claude Desktop</button>
          </div>
          <div class="ailink-step">
            <b>3</b><div><div class="t">Ask</div><div class="muted">In Claude Desktop, check Settings › Extensions shows Datum Plan switched on, then start a new chat: <i>“What plan is open in Datum Plan, and what is its finish date?”</i></div></div>
          </div>
          <details class="ailink-more">
            <summary>Other ways: Claude's settings file, Claude Code, or another AI app</summary>
            <p class="muted">Write Datum Plan into Claude Desktop's settings file{x().claude_config ? ` (${x().claude_config})` : ""}, with a backup; close Claude Desktop first, then open it again: <button class="btn sm" onClick={setup}>{x().claude_set ? "Write again" : "Write settings"}</button></p>
            <p class="muted">Or by hand: add this to {x().claude_config ?? "claude_desktop_config.json"} (Settings › Developer › Edit Config):</p>
            <pre>{x().snippet}</pre>
            <button class="btn sm" onClick={() => copy(x().snippet, "The settings text")}>Copy</button>
            <p class="muted">Claude Code (in a terminal):</p>
            <pre>{`claude mcp add datum-plan -- "${x().exe}" --mcp`}</pre>
            <button class="btn sm" onClick={() => copy(`claude mcp add datum-plan -- "${x().exe}" --mcp`, "The command")}>Copy</button>
            <p class="muted">Another AI app on this computer that can use local MCP servers (tools): add a server with this command and the argument <code>--mcp</code>. The AI Link is made and tested with Claude Desktop and Claude Code; other apps may work differently.</p>
            <pre>{`"${x().exe}" --mcp`}</pre>
            <button class="btn sm" onClick={() => copy(`"${x().exe}" --mcp`, "The command")}>Copy</button>
          </details>
        </>}</Show>
      </div>
    </Dialog>
  );
});
