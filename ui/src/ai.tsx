// AI Link (Home › AI Link): Claude works on the plan open in this window. The engine has the tools
// (crates/core/src/assist.rs); the window switches the link on, sets up Claude Desktop, and shows
// Claude's changes as they happen (each one an Undo step).
import { Show, createSignal } from "solid-js";
import { call, openDialog, opts, refresh, setOpts, setOut, toast } from "./store";
import { Dialog } from "./ui";
import { Out, listen } from "./api";

type Info = { on: boolean; exe: string; claude_config: string | null; claude_set: boolean; snippet: string };
export const [aiOn, setAiOn] = createSignal(false);

/** What Claude just did, in words for the note at the bottom. */
const DID: Record<string, string> = {
  add_tasks: "added tasks", set_task_fields: "changed a task", link_tasks: "linked tasks", unlink_tasks: "removed a link", delete_tasks: "deleted tasks",
  update_progress: "recorded progress", set_status_date: "set the status date", set_baseline: "saved a baseline", set_sheet_cells: "typed in a sheet",
  add_resources: "added resources", set_resource_fields: "changed a resource", delete_resources: "deleted resources", assign_resource: "assigned a resource", unassign_resource: "removed a resource from tasks",
  add_calendar: "added a calendar", set_calendar: "changed a calendar", delete_calendar: "deleted a calendar", define_code: "changed an activity code", set_task_codes: "coded tasks",
  define_custom_field: "set up a custom field", delete_custom_field: "removed a custom field", set_constraint: "set a constraint", set_deadline: "set a deadline",
  indent_tasks: "indented tasks", outdent_tasks: "outdented tasks", move_tasks: "moved tasks", set_task_notes: "wrote a note",
  set_risk: "changed the risk register", delete_risk: "removed a risk", set_delay_event: "changed a delay event", delete_delay_event: "removed a delay event", add_fragnet: "added a delay event's activities",
  take_snapshot: "took a snapshot", set_health_settings: "changed the health settings", level_resources: "levelled resources", clear_levelling: "cleared levelling",
  update_project: "updated progress through a date", mark_on_track: "marked tasks on track", split_task: "split a task", save_interim: "saved an interim plan", set_constraint_log_item: "changed the constraint log",
  apply_operation: "changed the plan", undo: "undid a change", redo: "redid a change", switch_plan: "showed another plan", save_plan: "saved the plan",
};

/** At start: the link's state, and Claude's changes shown as they come. */
export const initAi = async () => {
  const i = await call<Info>("ai_info");
  if (i) setAiOn(i.on);
  listen("ai-change", async (p: { out: Out; tool: string }) => {
    setOut(p.out);
    await refresh();
    toast(`Claude ${DID[p.tool] ?? "changed the plan"}. Undo (Ctrl+Z) takes it back.`, true);
  });
};

export const aiLink = () => openDialog(() => {
  const [i, setI] = createSignal<Info | null>(null);
  const [busy, setBusy] = createSignal(false);
  const load = async () => { const x = await call<Info>("ai_info"); if (x) { setI(x); setAiOn(x.on); } };
  const turn = async (on: boolean) => { setBusy(true); const x = await call<Info>("ai_link", { on }); setBusy(false); if (x) { setI(x); setAiOn(x.on); setOpts({ ...opts(), aiLink: x.on }); toast(on ? "AI Link on: Claude can work on the plan you have open." : "AI Link off.", true); } };
  const [added, setAdded] = createSignal<{ file: string; opened: boolean } | null>(null);
  const add = async () => { setBusy(true); const x = await call<{ file: string; opened: boolean }>("ai_add_to_claude"); setBusy(false); if (x) { setAdded(x); toast(x.opened ? "Claude Desktop is asking to install Datum Plan: click Install." : "Drag Datum Plan.mcpb onto Claude Desktop › Settings › Extensions.", true); } };
  const setup = async () => { const f = await call<string>("ai_setup_claude"); if (f) { toast(`Claude Desktop knows Datum Plan now (${f}). Restart Claude Desktop once.`, true); load(); } };
  const copy = (t: string, what: string) => navigator.clipboard?.writeText(t).then(() => toast(`${what} copied.`, true));
  load();
  return (
    <Dialog title="AI Link: Claude" help="ailink" cancel="Close">
      <div class="ailink">
        <p>Ask Claude, in Claude Desktop, to read or change the plan you have open here: <i>“Add a 3-day pump test after task 24 and link handover to it”</i>, <i>“Which tasks are late and why?”</i>. You see each change at once, and <b>Undo</b> takes it back. No API key is needed: Claude uses your own Claude account.</p>
        <Show when={i()}>{x => <>
          <div class="ailink-step" classList={{ done: x().on }}>
            <b>1</b><div><div class="t">Switch the AI Link on</div><div class="muted">Only Claude on this computer can reach it, with a key that changes each time. Datum Plan never saves your file unless you ask.</div></div>
            <button class="btn" classList={{ pri: !x().on }} disabled={busy()} onClick={() => turn(!x().on)}>{x().on ? "Turn off" : "Turn on"}</button>
          </div>
          <div class="ailink-step" classList={{ done: !!added() || x().claude_set }}>
            <b>2</b><div><div class="t">Add Datum Plan to Claude Desktop</div><div class="muted">Once. Claude Desktop opens and asks to install Datum Plan: click <b>Install</b>. Nothing to edit.
              <Show when={added()}>{a => <> {a().opened ? "" : <>If Claude did not open, drag <b>Datum Plan.mcpb</b> (shown in its folder, {a().file}) onto Claude Desktop › Settings › Extensions.</>}</>}</Show></div></div>
            <button class="btn" classList={{ pri: x().on && !added() }} disabled={busy()} onClick={add}>Add to Claude</button>
          </div>
          <div class="ailink-step">
            <b>3</b><div><div class="t">Ask Claude</div><div class="muted">In Claude Desktop, check Settings › Extensions shows Datum Plan switched on, then start a new chat: <i>“What plan is open in Datum Plan, and what is its finish date?”</i></div></div>
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
            <p class="muted">Another AI app on this computer that can use local MCP servers (tools): add a server with this command and the argument <code>--mcp</code>. Datum Plan is made and tested for Claude; other apps may work differently.</p>
            <pre>{`"${x().exe}" --mcp`}</pre>
            <button class="btn sm" onClick={() => copy(`"${x().exe}" --mcp`, "The command")}>Copy</button>
          </details>
        </>}</Show>
      </div>
    </Dialog>
  );
});
