//! The tools an AI assistant uses on an open plan (the window's AI Link, Model Context Protocol):
//! read the schedule, its checks, curves and sheets, and change it only through the engine's named
//! operations, so every change is one Undo step and is checked like the user's own. The window adds
//! the tools about its tabs (list, switch, save); this module works on one plan.
use crate::{check, doc::{Doc, Op}, fields::{res_text, task_fields, Calc}, model::*, ops, parse, progress, query::{builtin_filters, Filter, Rule}, report, risk, sheet};
use chrono::NaiveDate;
use serde_json::{json, Value};

/// The tool list (name, description, input schema), the window's own tools included.
pub fn tools() -> Value {
    let obj = |props: Value, req: &[&str]| json!({ "type": "object", "properties": props, "required": req });
    let none = || obj(json!({}), &[]);
    let uid = || json!({ "type": "integer", "description": "The task's unique id (uid), from get_tasks" });
    let uids = || json!({ "type": "array", "items": { "type": "integer" }, "description": "Task uids, from get_tasks" });
    let ruid = || json!({ "type": "integer", "description": "The resource's uid, from list_resources" });
    let week = || json!({ "type": "object", "description": "Hours per weekday (mon … sun), like \"08:00-12:00, 13:00-17:00\"; empty for a day off" });
    let base = json!([
        { "name": "list_plans", "description": "The plans open in Datum Plan (one per tab) and which one is shown. The tools below work on the plan shown.", "inputSchema": none() },
        { "name": "switch_plan", "description": "Show another open plan (by its tab number from list_plans, or its name), so the tools work on it.", "inputSchema": obj(json!({ "tab": { "type": "integer" }, "name": { "type": "string" } }), &[]) },
        { "name": "plan_summary", "description": "Facts about the plan shown: name, start, finish, status date, % complete and planned %, baseline finish and variance, SPI and CPI, tasks done, late and critical, cost.", "inputSchema": none() },
        { "name": "get_tasks", "description": "Tasks of the plan shown, in order, with uid, ID, WBS, name and the fields asked for (default: duration, start, finish, % complete, total float, predecessors, resources). Filter: all, critical, late, milestones, in_progress, not_started, complete, summaries; or search by name; or uids.", "inputSchema": obj(json!({ "filter": { "type": "string" }, "search": { "type": "string" }, "uids": { "type": "array", "items": { "type": "integer" } }, "fields": { "type": "array", "items": { "type": "string" }, "description": "Field keys from list_fields" }, "offset": { "type": "integer" }, "limit": { "type": "integer", "description": "At most 500 (default 200)" } }), &[]) },
        { "name": "get_task", "description": "One task in full: dates, baseline, progress, links in and out, resources, constraint, notes.", "inputSchema": obj(json!({ "uid": uid() }), &["uid"]) },
        { "name": "list_fields", "description": "The task fields (key, name, group, whether it can be typed in), for get_tasks and set_task_fields.", "inputSchema": none() },
        { "name": "check_schedule", "description": "The schedule quality checks (missing logic, leads, lags, constraints, high float, negative float, critical path test, CPLI, BEI…) with the failing tasks, and the health score.", "inputSchema": none() },
        { "name": "progress_curve", "description": "The S-curve: planned, actual and forecast cumulative % at each week or month end.", "inputSchema": obj(json!({ "unit": { "type": "string", "enum": ["week", "month"] }, "weight": { "type": "string", "enum": ["duration", "cost", "work", "physical"] } }), &[]) },
        { "name": "schedule_risk", "description": "Monte Carlo schedule risk: P50, P80 and P90 finish and the tasks that drive it (uses the tasks' optimistic/pessimistic ranges and the risk register).", "inputSchema": obj(json!({ "runs": { "type": "integer" } }), &[]) },
        { "name": "lookahead", "description": "The work in the next weeks from the status date, with open constraints.", "inputSchema": obj(json!({ "weeks": { "type": "integer" } }), &[]) },
        { "name": "get_sheet", "description": "A sheet of the plan (by name): every cell's address, what was typed and what it shows. Without a name, the list of sheets.", "inputSchema": obj(json!({ "name": { "type": "string" } }), &[]) },
        { "name": "add_tasks", "description": "Add tasks after a task (or at the end). Each task: name, and any fields by key, e.g. {\"name\":\"FAT\",\"dur\":\"3d\",\"preds\":\"12FS+2d\"}. preds uses task IDs (row numbers) as in the task sheet. Returns the new uids.", "inputSchema": obj(json!({ "after_uid": uid(), "tasks": { "type": "array", "items": { "type": "object" } } }), &["tasks"]) },
        { "name": "set_task_fields", "description": "Change fields of a task by key (see list_fields), e.g. {\"dur\":\"5d\",\"name\":\"…\",\"pct\":\"50\",\"astart\":\"2026-11-20\"}. All or nothing.", "inputSchema": obj(json!({ "uid": uid(), "fields": { "type": "object" } }), &["uid", "fields"]) },
        { "name": "link_tasks", "description": "Link two tasks: type FS, SS, FF or SF, lag like 2d or -1d (empty for none).", "inputSchema": obj(json!({ "pred_uid": uid(), "succ_uid": uid(), "type": { "type": "string", "enum": ["FS", "SS", "FF", "SF"] }, "lag": { "type": "string" } }), &["pred_uid", "succ_uid"]) },
        { "name": "unlink_tasks", "description": "Remove the link between two tasks.", "inputSchema": obj(json!({ "pred_uid": uid(), "succ_uid": uid() }), &["pred_uid", "succ_uid"]) },
        { "name": "delete_tasks", "description": "Delete tasks (and their subtasks).", "inputSchema": obj(json!({ "uids": { "type": "array", "items": { "type": "integer" } } }), &["uids"]) },
        { "name": "update_progress", "description": "Record progress on a task: % complete, and optionally actual start and finish (YYYY-MM-DD).", "inputSchema": obj(json!({ "uid": uid(), "pct": { "type": "integer" }, "actual_start": { "type": "string" }, "actual_finish": { "type": "string" } }), &["uid"]) },
        { "name": "set_status_date", "description": "Set the plan's status (data) date, YYYY-MM-DD.", "inputSchema": obj(json!({ "date": { "type": "string" } }), &["date"]) },
        { "name": "set_baseline", "description": "Save the current dates as a baseline (0 to 10), for the whole plan or some tasks. Ask the user first: it replaces that baseline.", "inputSchema": obj(json!({ "n": { "type": "integer" }, "uids": { "type": "array", "items": { "type": "integer" } } }), &[]) },
        { "name": "set_sheet_cells", "description": "Type into cells of a sheet (by name): {\"A1\":\"Vendor\",\"B2\":\"=SUM(C2:C9)\"}; an empty value clears a cell.", "inputSchema": obj(json!({ "name": { "type": "string" }, "cells": { "type": "object" } }), &["name", "cells"]) },
    ]);
    // Resources, calendars, codes, custom fields, constraints, filters, outline and notes (0.9.2).
    let more = json!([
        { "name": "list_resources", "description": "The plan's resources: uid, name, kind (Work, Material, Cost), group, max units, rate, calendar, work, cost and whether over-allocated.", "inputSchema": none() },
        { "name": "add_resources", "description": "Add resources, each with a name and any of: kind (work, material, cost), group, max (e.g. 200% or 2), rate (per hour, or per unit for material), per_use, ovt, label (material unit), initials, email, code, country, notes, calendar. Returns the new uids.", "inputSchema": obj(json!({ "resources": { "type": "array", "items": { "type": "object" } } }), &["resources"]) },
        { "name": "set_resource_fields", "description": "Change fields of a resource (same keys as add_resources), e.g. {\"rate\":\"85\",\"max\":\"300%\",\"calendar\":\"Night shift\"}. All or nothing.", "inputSchema": obj(json!({ "resource_uid": ruid(), "fields": { "type": "object" } }), &["resource_uid", "fields"]) },
        { "name": "delete_resources", "description": "Delete resources (their assignments go with them). Ask the user first.", "inputSchema": obj(json!({ "resource_uids": { "type": "array", "items": { "type": "integer" } } }), &["resource_uids"]) },
        { "name": "assign_resource", "description": "Assign a resource (by name) to tasks, or change its units: units like 50% or 2 for work (people), a quantity for material, an amount for cost. Other assignments stay.", "inputSchema": obj(json!({ "uids": uids(), "resource": { "type": "string" }, "units": { "type": "string" } }), &["uids", "resource"]) },
        { "name": "unassign_resource", "description": "Remove a resource (by name) from tasks.", "inputSchema": obj(json!({ "uids": uids(), "resource": { "type": "string" } }), &["uids", "resource"]) },
        { "name": "get_assignments", "description": "Assignments of a task (uid) or of a resource (resource name): resource or task, units, dates.", "inputSchema": obj(json!({ "uid": uid(), "resource": { "type": "string" } }), &[]) },
        { "name": "list_calendars", "description": "The plan's calendars: working hours per weekday, number of exceptions, which is the project calendar, and how many tasks and resources use each.", "inputSchema": none() },
        { "name": "get_calendar", "description": "One calendar in full (the project calendar when no name): working hours per weekday and every exception (holiday, shutdown, extra working day).", "inputSchema": obj(json!({ "name": { "type": "string" } }), &[]) },
        { "name": "add_calendar", "description": "Add a calendar: a copy of another (copy_from) or a standard Monday–Friday 8-hour week, then week hours per day, e.g. {\"sat\":\"08:00-12:00\",\"sun\":\"\"} (empty = not working).", "inputSchema": obj(json!({ "name": { "type": "string" }, "copy_from": { "type": "string" }, "week": week() }), &["name"]) },
        { "name": "set_calendar", "description": "Change a calendar: week hours per day, add exceptions [{\"name\":\"Eid holiday\",\"from\":\"2026-03-20\",\"to\":\"2026-03-22\"}] (with hours for a working exception, e.g. \"08:00-12:00\"), remove exceptions by name, or rename it.", "inputSchema": obj(json!({ "name": { "type": "string" }, "week": week(), "add_exceptions": { "type": "array", "items": { "type": "object" } }, "remove_exceptions": { "type": "array", "items": { "type": "string" } }, "rename": { "type": "string" } }), &["name"]) },
        { "name": "delete_calendar", "description": "Delete a calendar no task or resource uses (not the project calendar).", "inputSchema": obj(json!({ "name": { "type": "string" } }), &["name"]) },
        { "name": "list_codes", "description": "The activity codes (e.g. Area, Discipline, Phase) with their values and how many tasks have each value.", "inputSchema": none() },
        { "name": "define_code", "description": "Add or change an activity code and its values: [{\"value\":\"CIV\",\"desc\":\"Civil\"}] or [\"CIV\",\"MECH\"]. rename_from renames a code (tasks keep their values); remove deletes it.", "inputSchema": obj(json!({ "name": { "type": "string" }, "values": { "type": "array" }, "rename_from": { "type": "string" }, "remove": { "type": "boolean" } }), &["name"]) },
        { "name": "set_task_codes", "description": "Give tasks a value of an activity code (empty value clears it).", "inputSchema": obj(json!({ "uids": uids(), "code": { "type": "string" }, "value": { "type": "string" } }), &["uids", "code", "value"]) },
        { "name": "list_custom_fields", "description": "The custom fields in use (key like text1, number2, flag1, date1, cost1, duration1, outline1): alias, formula, value list, and how many tasks have a value. Values are set with set_task_fields and the key c:text1.", "inputSchema": none() },
        { "name": "define_custom_field", "description": "Name a custom field (alias), give it a formula (e.g. =[Duration]*2 or a sheet formula) or a value list (lookup, restrict to the list).", "inputSchema": obj(json!({ "key": { "type": "string", "description": "text1–30, number1–20, flag1–20, date1–10, cost1–10, duration1–10, start1–10, finish1–10, outline1–10" }, "alias": { "type": "string" }, "formula": { "type": "string" }, "lookup": { "type": "array", "items": { "type": "string" } }, "restrict": { "type": "boolean" } }), &["key"]) },
        { "name": "delete_custom_field", "description": "Remove a custom field's definition and its values on every task. Ask the user first.", "inputSchema": obj(json!({ "key": { "type": "string" } }), &["key"]) },
        { "name": "set_constraint", "description": "Set a date constraint on tasks: ASAP, ALAP, SNET, SNLT, FNET, FNLT, MSO or MFO, with a date (YYYY-MM-DD) for the dated ones.", "inputSchema": obj(json!({ "uids": uids(), "type": { "type": "string", "enum": ["ASAP", "ALAP", "SNET", "SNLT", "FNET", "FNLT", "MSO", "MFO"] }, "date": { "type": "string" } }), &["uids", "type"]) },
        { "name": "set_deadline", "description": "Set a deadline on tasks (YYYY-MM-DD; empty clears it). A deadline does not move the task; a finish after it shows as late.", "inputSchema": obj(json!({ "uids": uids(), "date": { "type": "string" } }), &["uids"]) },
        { "name": "list_filters", "description": "The built-in filters and the filters saved with the plan, with their rules.", "inputSchema": none() },
        { "name": "filter_tasks", "description": "Tasks that pass a filter: a name from list_filters, or rules [{\"field\":\"tf\",\"test\":\"is less than\",\"value\":\"5d\"},{\"field\":\"code:Area\",\"test\":\"equals\",\"value\":\"U-100\",\"or\":false}]. Tests: equals, does not equal, is greater than, is less than (or equal to), is within, is not within (value and value2), contains, does not contain.", "inputSchema": obj(json!({ "name": { "type": "string" }, "rules": { "type": "array", "items": { "type": "object" } }, "fields": { "type": "array", "items": { "type": "string" } }, "offset": { "type": "integer" }, "limit": { "type": "integer" } }), &[]) },
        { "name": "indent_tasks", "description": "Indent tasks one level: they become subtasks of the task above.", "inputSchema": obj(json!({ "uids": uids() }), &["uids"]) },
        { "name": "outdent_tasks", "description": "Outdent tasks one level.", "inputSchema": obj(json!({ "uids": uids() }), &["uids"]) },
        { "name": "move_tasks", "description": "Move tasks (with their subtasks) up or down by some rows.", "inputSchema": obj(json!({ "uids": uids(), "direction": { "type": "string", "enum": ["up", "down"] }, "rows": { "type": "integer" } }), &["uids", "direction"]) },
        { "name": "get_columns", "description": "The columns of the task sheet (Entry table), in order, with their width; hidden ones say so.", "inputSchema": none() },
        { "name": "set_columns", "description": "Show columns in the task sheet (Entry table), by field key or name from list_fields, e.g. [\"Baseline Start\",\"b0finish\"], placed after a column (after) or at the end; or hide columns. The window shows the change at once.", "inputSchema": obj(json!({ "show": { "type": "array", "items": { "type": "string" } }, "after": { "type": "string" }, "hide": { "type": "array", "items": { "type": "string" } } }), &[]) },
        { "name": "set_task_notes", "description": "Write a task's notes, or add a line to them (append).", "inputSchema": obj(json!({ "uid": uid(), "notes": { "type": "string" }, "append": { "type": "boolean" } }), &["uid", "notes"]) },
        { "name": "list_operations", "description": "Every named operation the engine has (with an example), for apply_operation.", "inputSchema": none() },
        { "name": "apply_operation", "description": "Any other change, as one named operation (see list_operations). A failed operation changes nothing.", "inputSchema": obj(json!({ "operation": { "type": "object" } }), &["operation"]) },
        { "name": "undo", "description": "Undo the last change in the plan shown (the user's or the assistant's).", "inputSchema": none() },
        { "name": "redo", "description": "Redo the change undone last.", "inputSchema": none() },
        { "name": "save_plan", "description": "Save the plan shown to its file. Only when the user asks for it.", "inputSchema": none() },
    ]);
    // Risks, delays, snapshots and reports, health settings, levelling, updating (0.9.4).
    let ids = |what: &str| json!({ "type": "array", "items": { "type": "integer" }, "description": format!("{what} uids, from get_tasks") });
    let third = json!([
        { "name": "list_risks", "description": "The risk register: id, name, cause, probability and impact (1–5), score, owner, response, status, kind, linked tasks, and the schedule delay in working days (with its range and own chance) used by schedule_risk.", "inputSchema": none() },
        { "name": "set_risk", "description": "Add a risk (no id) or change one (id). Fields: name, cause, prob and impact (1–5), owner, response, status (Open, Closed…), kind (Threat, Opportunity), task_uids, days (delay on each linked task when it occurs), pct (own chance %), days_min, days_max.", "inputSchema": obj(json!({ "id": { "type": "integer" }, "name": { "type": "string" }, "cause": { "type": "string" }, "prob": { "type": "integer" }, "impact": { "type": "integer" }, "owner": { "type": "string" }, "response": { "type": "string" }, "status": { "type": "string" }, "kind": { "type": "string" }, "task_uids": ids("Task"), "days": { "type": "number" }, "pct": { "type": "number" }, "days_min": { "type": "number" }, "days_max": { "type": "number" } }), &[]) },
        { "name": "delete_risk", "description": "Remove a risk from the register. Ask the user first.", "inputSchema": obj(json!({ "id": { "type": "integer" } }), &["id"]) },
        { "name": "list_delay_events", "description": "The delay events for delay analysis: what happened, when, and what it added (extra working days on tasks, tasks held to a date, new activities).", "inputSchema": none() },
        { "name": "set_delay_event", "description": "Add a delay event (no id) or change one: name, date, description, on (counted in the analyses), extend [{\"uid\":12,\"days\":5}], hold [{\"uid\":14,\"date\":\"2027-01-10\"}], frag [{\"name\":\"Redesign\",\"days\":10,\"after\":[12],\"before\":[14]}].", "inputSchema": obj(json!({ "id": { "type": "integer" }, "name": { "type": "string" }, "date": { "type": "string" }, "description": { "type": "string" }, "on": { "type": "boolean" }, "extend": { "type": "array" }, "hold": { "type": "array" }, "frag": { "type": "array" } }), &[]) },
        { "name": "delete_delay_event", "description": "Remove a delay event. Ask the user first.", "inputSchema": obj(json!({ "id": { "type": "integer" } }), &["id"]) },
        { "name": "add_fragnet", "description": "Put a delay event's new activities into the plan, linked as the event says.", "inputSchema": obj(json!({ "id": { "type": "integer" } }), &["id"]) },
        { "name": "delay_analysis", "description": "Delay analysis on the plan's data: available (what the plan holds and which methods can run), apab (as-planned against as-built), windows (between snapshots), tia (time impact), iap (impacted as-planned), cab (collapsed as-built). It measures; it does not say who is responsible.", "inputSchema": obj(json!({ "method": { "type": "string", "enum": ["available", "apab", "windows", "tia", "iap", "cab"] } }), &["method"]) },
        { "name": "list_snapshots", "description": "The snapshots (period records): date, label, comment, finish, % complete, cost and health score.", "inputSchema": none() },
        { "name": "take_snapshot", "description": "Record the plan as it stands at the status date (closes a reporting period), with a label and comment.", "inputSchema": obj(json!({ "label": { "type": "string" }, "comment": { "type": "string" } }), &[]) },
        { "name": "period_report", "description": "The progress report for a week or month (the latest at the status date, or the one ending on a date): planned and actual %, finish and its variance, SPI, CPI, tasks completed, started, slipped and coming up, milestones, progress by WBS.", "inputSchema": obj(json!({ "period": { "type": "string", "enum": ["week", "month"] }, "end": { "type": "string" } }), &[]) },
        { "name": "milestone_trend", "description": "Each milestone's forecast over the snapshots and today, against its baseline: which milestones keep slipping.", "inputSchema": none() },
        { "name": "earned_value", "description": "Earned value at the status date: PV, EV, AC, BAC, SV, CV, SPI, CPI, and earned schedule (SPI(t), SV(t) in working days, forecast finish).", "inputSchema": none() },
        { "name": "get_health_settings", "description": "The schedule check limits (percent allowed, high float, long duration, long lag, minimum index) and each check's weight in the health score, with the current score.", "inputSchema": none() },
        { "name": "set_health_settings", "description": "Change check limits ({\"high_float\":44,\"pct\":5}) and weights ({\"logic\":3,\"resources\":0}) of the health score.", "inputSchema": obj(json!({ "limits": { "type": "object" }, "weights": { "type": "object" } }), &[]) },
        { "name": "level_preview", "description": "What resource levelling would do (all tasks, or some), without changing anything: tasks that move, the finish before and after, over-allocated days before and after.", "inputSchema": obj(json!({ "uids": ids("Task") }), &[]) },
        { "name": "level_resources", "description": "Level resources (all tasks, or some): delays tasks so no one is booked over capacity. Run level_preview first and ask the user.", "inputSchema": obj(json!({ "uids": ids("Task") }), &[]) },
        { "name": "clear_levelling", "description": "Remove the levelling delays (all tasks, or some).", "inputSchema": obj(json!({ "uids": ids("Task") }), &[]) },
        { "name": "update_project", "description": "Update Project: mark work complete as scheduled through a date, or (reschedule) move unfinished work to start after it.", "inputSchema": obj(json!({ "through": { "type": "string" }, "reschedule": { "type": "boolean" } }), &["through"]) },
        { "name": "mark_on_track", "description": "Mark tasks on track: progress as scheduled up to the status date.", "inputSchema": obj(json!({ "uids": uids() }), &["uids"]) },
        { "name": "split_task", "description": "Split a task: a pause (gap, like 3d) after some of its work (after, like 2d).", "inputSchema": obj(json!({ "uid": uid(), "after": { "type": "string" }, "gap": { "type": "string" } }), &["uid", "after", "gap"]) },
        { "name": "save_interim", "description": "Save the current start and finish of every task as interim plan 1–10 (Start1/Finish1 …), a lighter record than a baseline.", "inputSchema": obj(json!({ "n": { "type": "integer" } }), &["n"]) },
        { "name": "list_constraint_log", "description": "The constraint log of the look-ahead: what must be in place before a task can start (drawings, materials, permits…), its owner, need date and status.", "inputSchema": none() },
        { "name": "set_constraint_log_item", "description": "Add (no id) or change an entry of the constraint log: uid (the task held up), kind, text, owner, need (date), status (Open, Closed); remove deletes it.", "inputSchema": obj(json!({ "id": { "type": "integer" }, "uid": uid(), "kind": { "type": "string" }, "text": { "type": "string" }, "owner": { "type": "string" }, "need": { "type": "string" }, "status": { "type": "string" }, "remove": { "type": "boolean" } }), &[]) },
        { "name": "resource_usage", "description": "Hours or cost per week or month, by resource or by task (over-allocated periods marked), or people per period by trade (by: trade).", "inputSchema": obj(json!({ "by": { "type": "string", "enum": ["resource", "task", "trade"] }, "what": { "type": "string", "enum": ["work", "cost"] }, "unit": { "type": "string", "enum": ["week", "month"] } }), &[]) },
    ]);
    // Registers, baselines, exports, sheets and dashboards (0.9.5).
    let rid = |what: &str| json!({ "type": "integer", "description": format!("The {what}'s id (0 or none adds one)") });
    let item = |what: &str, fields: &str| json!({ "type": "object", "description": format!("{what} fields to set: {fields}. Dates YYYY-MM-DD.") });
    let style = || json!({ "type": "object", "description": "bold, italic, underline, strike (true/false), color and fill (#RRGGBB), size (px), font, align (left, center, right), border (\"thin #000000\": thin, med, thick, dbl, dash, dot); an empty value removes it" });
    let fourth = json!([
        { "name": "contract_status", "description": "The contract: value, contract dates (forecast against the date, float, delay damages) and payment milestones (due, invoiced, paid).", "inputSchema": none() },
        { "name": "set_contract", "description": "The contract's number, value and warning days (a contract date with less float is at risk).", "inputSchema": obj(json!({ "number": { "type": "string" }, "value": { "type": "number" }, "warn_days": { "type": "integer" } }), &[]) },
        { "name": "set_contract_date", "description": "Add or change a contract date (met by a task's finish), or remove it.", "inputSchema": obj(json!({ "id": rid("contract date"), "fields": item("Contract date", "name, task (uid), date, value, ld_rate (% per week), ld_cap (%), grace (days), note"), "remove": { "type": "boolean" } }), &[]) },
        { "name": "set_payment", "description": "Add or change a payment milestone (a share or amount, due some days after its task finishes), or remove it.", "inputSchema": obj(json!({ "id": rid("payment"), "fields": item("Payment", "name, pct, amount, task (uid), terms (days), invoiced, paid"), "remove": { "type": "boolean" } }), &[]) },
        { "name": "document_register", "description": "The vendor document register: each document's planned, actual and forecast dates of its six steps, revisions and resubmissions, and the progress earned against planned.", "inputSchema": none() },
        { "name": "set_vendor_document", "description": "Add or change a vendor document, or remove it.", "inputSchema": obj(json!({ "id": rid("document"), "fields": item("Document", "number, title, vendor, package, disc, kind, class (1–3), key, offset (P + n), first (date), weight, task (uid that needs it), remarks"), "remove": { "type": "boolean" } }), &[]) },
        { "name": "log_submission", "description": "Record a submission of a vendor document and the client's return (or change one by id).", "inputSchema": obj(json!({ "id": rid("submission"), "fields": item("Submission", "doc (document id), rev, purpose (ICR, IFA, IFC1…), transmittal, sent, ret_transmittal, returned, code (Code 1…), crs") }), &[]) },
        { "name": "procurement_status", "description": "The procurement register: each order's planned and actual RFQ, PO, manufacturing, inspection release and delivery, promised date, float to need, and progress.", "inputSchema": none() },
        { "name": "set_order", "description": "Add or change an order (equipment item or sub-order), or remove it.", "inputSchema": obj(json!({ "id": rid("order"), "fields": item("Order", "tag, name, vendor, origin, po_no, value, long_lead, task (uid), need (uid), required, planned and actual ([RFQ, PO, manufacturing, IRN, delivery] dates or null), mfg_pct, promised, remarks"), "remove": { "type": "boolean" } }), &[]) },
        { "name": "log_expediting", "description": "Add a note to an order's expediting log.", "inputSchema": obj(json!({ "id": { "type": "integer" }, "note": { "type": "string" }, "date": { "type": "string" }, "by": { "type": "string" } }), &["id", "note"]) },
        { "name": "list_baselines", "description": "Which baselines (0–10) are set: how many tasks, and the project finish in each.", "inputSchema": none() },
        { "name": "baseline_variance", "description": "Tasks against a baseline (default 0): start and finish variance in days, slipped most first.", "inputSchema": obj(json!({ "n": { "type": "integer" }, "limit": { "type": "integer" } }), &[]) },
        { "name": "clear_baseline", "description": "Clear a baseline (0–10) for the whole plan or some tasks. Ask the user first.", "inputSchema": obj(json!({ "n": { "type": "integer" }, "uids": uids() }), &["n"]) },
        { "name": "export_plan", "description": "Save a copy of the plan in another format to the Downloads folder: xlsx, ods, csv, xml (Project XML), xer, pmxml, mpx, json, html, ics.", "inputSchema": obj(json!({ "format": { "type": "string", "enum": ["xlsx", "ods", "csv", "xml", "xer", "pmxml", "mpx", "json", "html", "ics"] } }), &["format"]) },
        { "name": "export_register", "description": "Save a register as a workbook to the Downloads folder: documents, procurement or delays (the delay analysis).", "inputSchema": obj(json!({ "register": { "type": "string", "enum": ["documents", "procurement", "delays"] } }), &["register"]) },
        { "name": "add_sheet", "description": "Add a sheet (spreadsheet tab), optionally with cells: {\"A1\":\"Vendor\",\"B1\":\"=SUM(B2:B9)\"}.", "inputSchema": obj(json!({ "name": { "type": "string" }, "cells": { "type": "object" } }), &["name"]) },
        { "name": "rename_sheet", "description": "Rename a sheet (formulas that refer to it follow).", "inputSchema": obj(json!({ "name": { "type": "string" }, "to": { "type": "string" } }), &["name", "to"]) },
        { "name": "delete_sheet", "description": "Delete a sheet. Ask the user first.", "inputSchema": obj(json!({ "name": { "type": "string" } }), &["name"]) },
        { "name": "format_sheet_cells", "description": "Format cells of a sheet (a range like A1:D1): bold, colours, size, alignment, borders; and column widths in px ({\"A\":180}).", "inputSchema": obj(json!({ "name": { "type": "string" }, "range": { "type": "string" }, "style": style(), "widths": { "type": "object" } }), &["name"]) },
        { "name": "set_number_format", "description": "Number format of sheet cells (a range): #,##0.00, 0.0%, dd-mmm-yy, \"$\"#,##0 …; empty for General.", "inputSchema": obj(json!({ "name": { "type": "string" }, "range": { "type": "string" }, "format": { "type": "string" } }), &["name", "range", "format"]) },
        { "name": "define_name", "description": "Add, change or (empty formula) remove a defined name for the sheets' formulas, e.g. Rate =Inputs!$B$2.", "inputSchema": obj(json!({ "name": { "type": "string" }, "formula": { "type": "string" }, "comment": { "type": "string" } }), &["name"]) },
        { "name": "format_tasks", "description": "Format rows of the task sheet (or one column of them): bold, italic, colours, size…", "inputSchema": obj(json!({ "uids": uids(), "column": { "type": "string", "description": "A field key (name, start…); none for the whole row" }, "style": style() }), &["uids", "style"]) },
        { "name": "list_dashboards", "description": "The plan's own dashboards and their tiles.", "inputSchema": none() },
        { "name": "add_dashboard", "description": "Add a dashboard (or replace one of the same name) with tiles: [{\"kind\":\"number\",\"title\":\"SPI\",\"formula\":\"=SPI\",\"size\":3}, {\"kind\":\"ring\",\"title\":\"Progress\",\"formula\":\"=PercentComplete\"}, {\"kind\":\"chart\",\"title\":\"S-curve\",\"plan\":\"scurve\",\"size\":12}, {\"kind\":\"table\",\"title\":\"Late\",\"plan\":\"late\"}, {\"kind\":\"text\",\"text\":\"Notes\"}]. Plan charts: scurve, periodbars, phases, costev, manpower; lists: milestones, late, critical, upcoming, completed. Size 3, 4, 6, 8 or 12 (of 12).", "inputSchema": obj(json!({ "name": { "type": "string" }, "tiles": { "type": "array", "items": { "type": "object" } } }), &["name", "tiles"]) },
        { "name": "delete_dashboard", "description": "Delete one of the plan's dashboards.", "inputSchema": obj(json!({ "name": { "type": "string" } }), &["name"]) },
    ]);
    // Task Path, project details, recurring tasks, rules of credit and steps (0.9.6).
    let fifth = json!([
        { "name": "task_path", "description": "A task's path: its driving predecessors (the chain that sets its dates), other predecessors, the successors it drives, and other successors.", "inputSchema": obj(json!({ "uid": uid() }), &["uid"]) },
        { "name": "set_project_info", "description": "The project's details: name, code, description, client, manager, location, company.", "inputSchema": obj(json!({ "name": { "type": "string" }, "code": { "type": "string" }, "description": { "type": "string" }, "client": { "type": "string" }, "manager": { "type": "string" }, "location": { "type": "string" }, "company": { "type": "string" } }), &[]) },
        { "name": "add_recurring_task", "description": "Add a recurring task (a summary with one occurrence every day, week or month, e.g. a weekly progress meeting) after a task or at the end.", "inputSchema": obj(json!({ "name": { "type": "string" }, "duration": { "type": "string" }, "every": { "type": "string", "enum": ["day", "week", "month"] }, "interval": { "type": "integer" }, "count": { "type": "integer" }, "start": { "type": "string" }, "after_uid": uid() }), &["name", "every", "count", "start"]) },
        { "name": "list_credit_rules", "description": "The rules of credit (steps of a kind of work and the share of progress each earns): the plan's own and the built-in ones.", "inputSchema": none() },
        { "name": "apply_credit_rule", "description": "Give tasks the steps of a rule of credit; their progress is then measured by the steps done (physical %).", "inputSchema": obj(json!({ "uids": uids(), "rule": { "type": "string" } }), &["uids", "rule"]) },
        { "name": "set_task_steps", "description": "A task's steps (name, weight, done), replacing its list; with weights, the steps done set its physical %.", "inputSchema": obj(json!({ "uid": uid(), "steps": { "type": "array", "items": { "type": "object" } } }), &["uid", "steps"]) },
        { "name": "mark_step_done", "description": "Tick (or untick) one step of a task by its name; the task's physical % follows.", "inputSchema": obj(json!({ "uid": uid(), "step": { "type": "string" }, "done": { "type": "boolean" } }), &["uid", "step"]) },
    ]);
    let mut all: Vec<Value> = [base, more, third, fourth, fifth].into_iter().flat_map(|v| match v { Value::Array(v) => v, _ => vec![] }).collect();
    // How Claude may treat each tool: reading only, or changing something it cannot easily get back.
    for t in &mut all {
        let n = t["name"].as_str().unwrap_or("").to_string();
        t["annotations"] = json!({ "readOnlyHint": !changes(&n) && !matches!(n.as_str(), "switch_plan" | "save_plan" | "export_plan" | "export_register"),
            "destructiveHint": n.starts_with("delete_") || matches!(n.as_str(), "set_baseline" | "clear_baseline" | "save_plan" | "apply_operation"), "openWorldHint": false });
    }
    Value::Array(all)
}

/// What the assistant is told when it connects.
pub const INSTRUCTIONS: &str = "You are connected to the Datum Plan window the user has open: a project schedule (CPM) with tasks, links, resources, baselines, progress and sheets. \
Read before you change: plan_summary, get_tasks (uids identify tasks), get_task. Change only with the tools; each change is one Undo step the user sees at once. \
Dates are YYYY-MM-DD. Durations like 5d, 3w, 4h. Ask before deleting tasks, setting a baseline or saving; never save unless asked. \
Work on Datum Plan only through these tools: do not control its window through the screen, and do not start or install another copy of Datum Plan (a second copy takes over the link). \
If a tool says the link is off, ask the user to turn the AI Link off and on in Datum Plan (Home › AI Link), then try again.";

/// Ready-made requests the assistant can offer (MCP prompts): name, description, arguments.
pub fn prompts() -> Value {
    let arg = |n: &str, d: &str, req: bool| json!({ "name": n, "description": d, "required": req });
    json!([
        { "name": "plan_project", "description": "Build a plan from a goal: phases, tasks, durations, links and resources.", "arguments": [arg("goal", "What the project must deliver", true), arg("start", "Start date (YYYY-MM-DD)", false), arg("team", "People or crews available, e.g. 'Ana (design), 2 welders'", false)] },
        { "name": "status_report", "description": "A short status report of the plan shown: progress, finish against baseline, late and critical work, risks.", "arguments": [arg("period", "week or month", false)] },
        { "name": "schedule_review", "description": "Review the schedule's quality and propose fixes (missing logic, constraints, float, lags), without changing it.", "arguments": [] },
    ])
}

/// The text of a prompt with its arguments filled in.
pub fn prompt(name: &str, a: &Value) -> Result<String, String> {
    let g = |k: &str| a[k].as_str().map(str::trim).filter(|x| !x.is_empty());
    Ok(match name {
        "plan_project" => format!("Build a plan in Datum Plan for: {}.{}{} Read plan_summary first. Add phases as summary tasks and their tasks with realistic durations (add_tasks, indent_tasks), link them (link_tasks), add milestones, add and assign resources (add_resources, assign_resource). Then run check_schedule, fix what fails, and tell me the finish date and the critical path.",
            g("goal").ok_or("goal is needed.")?, g("start").map(|s| format!(" Start on {s}.")).unwrap_or_default(), g("team").map(|s| format!(" Team: {s}.")).unwrap_or_default()),
        "status_report" => format!("Write a short status report of the plan open in Datum Plan for this {}: use plan_summary, period_report, earned_value, get_tasks (late, critical) and list_risks. Progress against plan, the finish against the baseline, what slipped and why, what comes next, and the top risks. Change nothing.", g("period").unwrap_or("month")),
        "schedule_review" => "Review the schedule open in Datum Plan: run check_schedule and get_tasks, then list each problem (missing logic, hard constraints, high or negative float, long lags, out-of-sequence progress) with the tasks involved and the fix you propose. Change nothing until I agree.".into(),
        x => return Err(format!("No prompt called {x}.")),
    })
}

/// Turns day numbers (days since 1970-01-01) under the given keys into dates, anywhere in `v`.
fn dates(v: &mut Value, keys: &[&str]) {
    match v {
        Value::Object(m) => for (k, x) in m.iter_mut() {
            if keys.contains(&k.as_str()) {
                if let Some(d) = x.as_i64() { *x = json!(crate::delay::date(d)); continue }
                if let Some(a) = x.as_array_mut() { for y in a.iter_mut() { if let Some(d) = y.as_i64() { *y = json!(crate::delay::date(d)) } } continue }
            }
            dates(x, keys)
        },
        Value::Array(a) => for x in a { dates(x, keys) },
        _ => {}
    }
}
/// `old` with the given fields set (by their names in the plan file); unknown fields are refused.
fn patch<T: serde::Serialize + serde::de::DeserializeOwned>(old: &T, f: &Value, fixed: &[&str]) -> Result<T, String> {
    let mut v = serde_json::to_value(old).map_err(|e| e.to_string())?;
    for (k, x) in f.as_object().into_iter().flatten() {
        if fixed.contains(&k.as_str()) || v.get(k).is_none() { return Err(format!("{k} cannot be set here.")) }
        v[k] = x.clone();
    }
    serde_json::from_value(v).map_err(|e| format!("A value has the wrong kind ({e}). Dates are YYYY-MM-DD."))
}
/// The cells of a range like A1:C3 (or one cell), at most 10,000.
fn cells_of(r: &str) -> Result<Vec<String>, String> {
    let one = |x: &str| -> Option<(u32, u32)> {
        let x = x.trim().trim_matches('$').replace('$', "").to_uppercase();
        let cut = x.find(|c: char| c.is_ascii_digit())?;
        let col = x[..cut].chars().try_fold(0u32, |n, c| c.is_ascii_uppercase().then(|| n * 26 + (c as u32 - 64)))?;
        Some((col.checked_sub(1)?, x[cut..].parse::<u32>().ok()?.checked_sub(1)?))
    };
    let bad = || format!("\"{r}\" is not a range like A1:C3.");
    let (a, b) = r.split_once(':').unwrap_or((r, r));
    let ((c0, r0), (c1, r1)) = (one(a).ok_or_else(bad)?, one(b).ok_or_else(bad)?);
    let (c0, c1, r0, r1) = (c0.min(c1), c0.max(c1), r0.min(r1), r0.max(r1));
    if (c1 - c0 + 1) as u64 * (r1 - r0 + 1) as u64 > 10_000 { return Err("Format at most 10,000 cells at a time.".into()) }
    Ok((r0..=r1).flat_map(|r| (c0..=c1).map(move |c| format!("{}{}", sheet::col_name(c), r + 1))).collect())
}
/// A cell style ("b;c:#C0392B;z:12") with the given changes.
fn restyle(old: &str, st: &Value) -> Result<String, String> {
    let mut m: Vec<(String, String)> = old.split(';').filter(|t| !t.is_empty()).map(|t| { let (k, v) = t.split_once(':').unwrap_or((t, "")); (k.to_string(), v.to_string()) }).collect();
    for (k, v) in st.as_object().ok_or("style is needed, like {\"bold\":true,\"fill\":\"#FFF2CC\"}.")? {
        let flag = |c: &str| (c.to_string(), String::new());
        let on = v.as_bool().unwrap_or(!txt(v).trim().is_empty() && txt(v) != "false");
        let val = txt(v).trim().to_string();
        let (key, set) = match k.as_str() {
            "bold" => ("b", on.then(|| flag("b"))), "italic" => ("i", on.then(|| flag("i"))), "underline" => ("u", on.then(|| flag("u"))), "strike" => ("s", on.then(|| flag("s"))), "wrap" => ("w", on.then(|| flag("w"))),
            "color" | "fill" => {
                if !val.is_empty() && !(val.len() == 7 && val.starts_with('#') && val[1..].chars().all(|c| c.is_ascii_hexdigit())) { return Err(format!("{k} is a colour like #C0392B.")) }
                (if k == "color" { "c" } else { "f" }, (!val.is_empty()).then(|| (if k == "color" { "c" } else { "f" }.to_string(), val.clone())))
            }
            "size" => ("z", match v.as_f64() { Some(z) if (6.0..=72.0).contains(&z) => Some(("z".into(), z.to_string())), None if val.is_empty() => None, _ => return Err("size is from 6 to 72.".into()) }),
            "font" => ("n", (!val.is_empty()).then(|| ("n".into(), val.replace([';', ':'], "")))),
            "align" => ("a", match val.as_str() { "" => None, "left" | "l" => Some(("a".into(), "l".into())), "center" | "c" => Some(("a".into(), "c".into())), "right" | "r" => Some(("a".into(), "r".into())), _ => return Err("align is left, center or right.".into()) }),
            "border" => ("", { for b in ["bt", "bb", "bl", "br"] { m.retain(|x| x.0 != b); if !val.is_empty() { m.push((b.into(), val.replace([';', ':'], ""))) } } None }),
            x => return Err(format!("No style {x}: bold, italic, underline, strike, wrap, color, fill, size, font, align, border.")),
        };
        if key.is_empty() { continue }
        m.retain(|x| x.0 != key);
        if let Some(x) = set { m.push(x) }
    }
    Ok(m.iter().map(|(k, v)| if v.is_empty() { k.clone() } else { format!("{k}:{v}") }).collect::<Vec<_>>().join(";"))
}
fn sheet_k(p: &Project, a: &Value) -> Result<usize, String> {
    let n = s(a, "name").ok_or("name is needed (the sheet; see get_sheet).")?;
    p.sheets.iter().position(|x| x.name.eq_ignore_ascii_case(n.trim())).ok_or_else(|| format!("No sheet called {n}."))
}
/// The task sheet's columns (the Entry table): the plan's own, or the window's first layout.
fn entry_columns(p: &Project) -> Vec<(String, i32)> {
    if !p.columns.is_empty() { return p.columns.clone() }
    [("mode", 34), ("name", 260), ("dur", 74), ("start", 92), ("finish", 92), ("preds", 90), ("res", 140)].iter().map(|(k, w)| (k.to_string(), *w)).collect()
}
/// A date from an instant (minutes; a finish is the end of its last minute).
fn inst(t: i64) -> String { crate::cal::date_of((t - 1).max(0)).to_string() }

/// Does a tool change the plan?
pub fn changes(name: &str) -> bool {
    matches!(name, "add_tasks" | "set_task_fields" | "link_tasks" | "unlink_tasks" | "delete_tasks" | "update_progress" | "set_status_date" | "set_baseline" | "set_sheet_cells" | "apply_operation"
        | "add_resources" | "set_resource_fields" | "delete_resources" | "assign_resource" | "unassign_resource" | "add_calendar" | "set_calendar" | "delete_calendar"
        | "define_code" | "set_task_codes" | "define_custom_field" | "delete_custom_field" | "set_constraint" | "set_deadline" | "indent_tasks" | "outdent_tasks" | "move_tasks" | "set_task_notes" | "set_columns"
        | "set_risk" | "delete_risk" | "set_delay_event" | "delete_delay_event" | "add_fragnet" | "take_snapshot" | "set_health_settings" | "level_resources" | "clear_levelling"
        | "update_project" | "mark_on_track" | "split_task" | "save_interim" | "set_constraint_log_item"
        | "set_contract" | "set_contract_date" | "set_payment" | "set_vendor_document" | "log_submission" | "set_order" | "log_expediting" | "clear_baseline"
        | "add_sheet" | "rename_sheet" | "delete_sheet" | "format_sheet_cells" | "set_number_format" | "define_name" | "format_tasks" | "add_dashboard" | "delete_dashboard"
        | "set_project_info" | "add_recurring_task" | "apply_credit_rule" | "set_task_steps" | "mark_step_done" | "undo" | "redo")
}

fn s<'a>(a: &'a Value, k: &str) -> Option<&'a str> { a[k].as_str().filter(|x| !x.trim().is_empty()) }
fn txt(v: &Value) -> String { v.as_str().map(String::from).unwrap_or_else(|| v.to_string()) }
fn uid_list(a: &Value, k: &str) -> Result<Vec<u32>, String> {
    let v: Vec<u32> = match &a[k] { Value::Array(v) => v.iter().filter_map(|x| x.as_u64().map(|x| x as u32)).collect(), x => x.as_u64().map(|x| vec![x as u32]).unwrap_or_default() };
    if v.is_empty() { Err(format!("{k} is needed (a list of uids).")) } else { Ok(v) }
}
const DAYS: [&str; 7] = ["mon", "tue", "wed", "thu", "fri", "sat", "sun"];
/// Working hours as text: "08:00-12:00, 13:00-17:00".
fn hours_text(h: &Hours) -> String { h.iter().map(|(a, b)| format!("{:02}:{:02}-{:02}:{:02}", a / 60, a % 60, b / 60, b % 60)).collect::<Vec<_>>().join(", ") }
fn hours(t: &str) -> Result<Hours, String> {
    let m = |x: &str| -> Option<u16> { let (h, mm) = x.trim().split_once(':').unwrap_or((x.trim(), "0")); Some(h.trim().parse::<u16>().ok()? * 60 + mm.trim().parse::<u16>().ok()?).filter(|v| *v <= 1440) };
    t.split([',', ';']).map(str::trim).filter(|x| !x.is_empty() && !x.eq_ignore_ascii_case("off")).map(|x| {
        x.split_once('-').and_then(|(a, b)| Some((m(a)?, m(b)?))).filter(|(a, b)| a < b).ok_or_else(|| format!("\"{x}\" is not a time range like 08:00-17:00."))
    }).collect()
}
fn week_json(c: &Calendar) -> Value { Value::Object(DAYS.iter().zip(&c.week).map(|(d, h)| (d.to_string(), json!(hours_text(h)))).collect()) }
fn set_week(c: &mut Calendar, w: &Value) -> Result<(), String> {
    for (k, v) in w.as_object().into_iter().flatten() {
        let d = DAYS.iter().position(|d| k.to_lowercase().starts_with(d)).ok_or_else(|| format!("\"{k}\" is not a weekday (mon … sun)."))?;
        c.week[d] = hours(&txt(v))?;
    }
    Ok(())
}
fn res_named(p: &Project, a: &Value) -> Result<usize, String> {
    let n = s(a, "resource").ok_or("resource is needed (a name from list_resources).")?;
    p.resources.iter().position(|r| r.name.eq_ignore_ascii_case(n.trim())).ok_or_else(|| format!("No resource called {n}; add it with add_resources."))
}
fn units_text(p: &Project, x: &Assign) -> String {
    let Some(r) = p.res_idx(x.res).map(|k| &p.resources[k]) else { return String::new() };
    if r.kind == ResKind::Work { format!("{}%", (x.units * 1000.0).round() / 10.0) } else { format!("{} {}", (x.units * 1000.0).round() / 1000.0, r.label).trim().into() }
}
/// Resource fields: the calendar goes through its own operation.
fn res_ops(p: &Project, uid: u32, f: &serde_json::Map<String, Value>) -> Result<Vec<Op>, String> {
    f.iter().map(|(k, v)| {
        let v = txt(v);
        if matches!(k.as_str(), "cal" | "calendar") {
            if !v.is_empty() && !p.calendars.iter().any(|c| c.name == v) { return Err(format!("There is no calendar called \"{v}\".")) }
            return Ok(Op::ResJson { uid, field: "cal".into(), value: json!(v) })
        }
        Ok(Op::ResSet { uid, field: k.clone(), value: v })
    }).collect()
}
/// A custom field key: text1–30, number1–20 … (r in front for resources).
fn custom_key(k: &str) -> bool {
    let k = k.strip_prefix('r').unwrap_or(k); // no task prefix starts with r
    let n = k.trim_start_matches(|c: char| c.is_ascii_alphabetic());
    let max = match &k[..k.len() - n.len()] { "text" => 30, "number" | "flag" => 20, "date" | "cost" | "duration" | "start" | "finish" | "outline" => 10, _ => 0 };
    n.parse::<u32>().is_ok_and(|n| (1..=max).contains(&n))
}
fn u(a: &Value, k: &str) -> Result<u32, String> { a[k].as_u64().map(|x| x as u32).ok_or_else(|| format!("{k} is needed (a task uid from get_tasks).")) }

/// Plan facts.
pub fn summary(c: &Calc) -> Value {
    let p = c.p;
    let sm = c.summary();
    let crit = (0..p.tasks.len()).filter(|&i| !c.s.summary[i] && !c.s.off[i] && c.s.critical(p, i)).count();
    json!({ "name": sm.name, "start": sm.start, "finish": sm.finish, "duration": sm.dur, "status_date": p.status.map(|d| d.to_string()), "measured_at": sm.measured_at,
        "complete_pct": (sm.pct * 100.0).round() / 100.0, "planned_pct": (sheet::planned_pct(c) * 100.0).round() / 100.0,
        "baseline_finish": sm.bl_finish, "finish_variance": sm.var_finish, "spi": sm.spi, "cpi": sm.cpi, "tasks": sm.tasks, "done": sm.done, "late": sm.late, "critical": crit,
        "work": sm.work, "cost": sm.cost, "baseline_locked": !p.baseline_unlocked, "error": sm.error })
}

const DEFAULT_FIELDS: [&str; 7] = ["dur", "start", "finish", "pct", "tf", "preds", "res"];

fn task_row(c: &Calc, i: usize, keys: &[String]) -> Value {
    let t = &c.p.tasks[i];
    let mut m = serde_json::Map::new();
    m.insert("uid".into(), json!(t.uid));
    m.insert("id".into(), json!(i + 1));
    m.insert("wbs".into(), json!(c.text(i, "wbs")));
    m.insert("name".into(), json!(t.name));
    m.insert("level".into(), json!(t.level));
    if c.s.summary[i] { m.insert("summary".into(), json!(true)); }
    if c.s.critical(c.p, i) && !c.s.summary[i] { m.insert("critical".into(), json!(true)); }
    for k in keys { m.insert(k.clone(), json!(c.text(i, k))); }
    Value::Object(m)
}

/// The fields asked for (or the default ones), checked.
fn field_keys(c: &Calc, a: &Value) -> Result<Vec<String>, String> {
    if let Some(e) = &c.error { return Err(e.clone()) }
    let keys: Vec<String> = a["fields"].as_array().map(|v| v.iter().filter_map(|x| x.as_str().map(String::from)).collect()).filter(|v: &Vec<String>| !v.is_empty()).unwrap_or_else(|| DEFAULT_FIELDS.iter().map(|k| k.to_string()).collect());
    if let Some(bad) = keys.iter().find(|k| c.key_of(k).is_none() && !task_fields().iter().any(|f| &f.key == *k)) { return Err(format!("No field \"{bad}\"; see list_fields.")) }
    Ok(keys)
}

/// One page of task rows.
fn page(c: &Calc, all: Vec<usize>, keys: &[String], a: &Value) -> Value {
    let off = a["offset"].as_u64().unwrap_or(0) as usize;
    let lim = a["limit"].as_u64().unwrap_or(200).clamp(1, 500) as usize;
    let rows: Vec<Value> = all.iter().skip(off).take(lim).map(|&i| task_row(c, i, keys)).collect();
    json!({ "total": all.len(), "offset": off, "tasks": rows, "more": all.len() > off + rows.len() })
}

/// Runs one tool on `doc`. `fmt` is the user's date format (typed dates are read in it or as YYYY-MM-DD).
pub fn call(doc: &mut Doc, fmt: &str, today: NaiveDate, name: &str, a: &Value) -> Result<Value, String> {
    let apply = |doc: &mut Doc, op: Op| -> Result<(), String> { doc.apply(op, fmt) };
    match name {
        "plan_summary" => Ok(summary(&Calc::new(&doc.p, fmt, today))),
        "list_fields" => Ok(json!(task_fields().iter().map(|f| json!({ "key": f.key, "name": f.name, "group": f.group, "editable": f.edit })).collect::<Vec<_>>())),
        "list_operations" => Ok(json!(ops::catalog())),
        "get_tasks" => {
            let c = Calc::new(&doc.p, fmt, today);
            let keys = field_keys(&c, a)?;
            let uids: Vec<u64> = a["uids"].as_array().map(|v| v.iter().filter_map(Value::as_u64).collect()).unwrap_or_default();
            let search = s(a, "search").map(str::to_lowercase);
            let filter = s(a, "filter").unwrap_or("all");
            let p = c.p;
            let keep = |i: usize| -> bool {
                if !uids.is_empty() { return uids.contains(&(p.tasks[i].uid as u64)) }
                if search.as_ref().is_some_and(|q| !p.tasks[i].name.to_lowercase().contains(q.as_str())) { return false }
                let (sm, pct) = (c.s.summary[i], c.pct[i]);
                match filter {
                    "critical" => !sm && c.s.critical(p, i),
                    "late" => !sm && c.val(i, "late").truth(),
                    "milestones" => !sm && p.tasks[i].dur == 0,
                    "in_progress" => !sm && pct > 0 && pct < 100,
                    "not_started" => !sm && pct == 0,
                    "complete" => !sm && pct == 100,
                    "summaries" => sm,
                    _ => true,
                }
            };
            Ok(page(&c, (0..p.tasks.len()).filter(|&i| keep(i)).collect(), &keys, a))
        }
        "get_task" => {
            let c = Calc::new(&doc.p, fmt, today);
            let uid = u(a, "uid")?;
            let i = doc.p.idx(uid).ok_or("No task with that uid.")?;
            let keys: Vec<String> = ["dur", "start", "finish", "b0start", "b0finish", "pct", "plannedpct", "astart", "afinish", "tf", "ff", "preds", "succs", "res", "work", "cost", "constraint", "cdate", "deadline", "cal", "notes"]
                .iter().filter(|k| task_fields().iter().any(|f| f.key == **k)).map(|k| k.to_string()).collect();
            Ok(task_row(&c, i, &keys))
        }
        "check_schedule" => {
            let c = Calc::new(&doc.p, fmt, today);
            let checks = check::run(&c, &doc.p.health.limits);
            let sc = check::score(&checks, &doc.p.health);
            let name = |uid: &u32| doc.p.idx(*uid).map(|i| json!({ "uid": uid, "id": i + 1, "name": doc.p.tasks[i].name }));
            Ok(json!({ "score": sc.score, "grade": sc.grade, "checks": checks.iter().map(|k| json!({ "check": k.name, "about": k.about, "pass": k.pass, "count": k.count, "of": k.total, "value": k.value, "limit": k.limit,
                "tasks": k.uids.iter().take(25).filter_map(name).collect::<Vec<_>>() })).collect::<Vec<_>>() }))
        }
        "progress_curve" => {
            let c = Calc::new(&doc.p, fmt, today);
            let unit = s(a, "unit").unwrap_or("month");
            let sc = report::scurve(&c, if unit == "week" { "week" } else { "month" }, s(a, "weight").unwrap_or(&doc.p.periods.weight), None);
            let d = |x: i64| crate::cal::date_of(x * crate::cal::DAY).to_string();
            Ok(json!({ "weighted_by": sc.weight, "status_date": d(sc.status_day), "points": (0..sc.days.len()).map(|i| json!({ "date": d(sc.days[i]), "planned": (sc.planned[i] * 10.0).round() / 10.0, "actual": sc.actual[i].map(|x| (x * 10.0).round() / 10.0), "forecast": (sc.forecast[i] * 10.0).round() / 10.0 })).collect::<Vec<_>>() }))
        }
        "schedule_risk" => {
            let r = risk::simulate(&doc.p, &risk::SimOpts { iterations: a["runs"].as_u64().unwrap_or(1000).clamp(100, 10_000) as u32, ..Default::default() })?;
            let d = |t: i64| crate::cal::date_of((t - 1).max(0)).to_string();
            Ok(json!({ "runs": r.runs, "planned_finish": d(r.planned), "chance_of_planned_pct": (r.planned_chance * 100.0).round(), "p50": d(r.p50), "p80": d(r.p80), "p90": d(r.p90),
                "drivers": r.drivers.iter().take(10).map(|x| json!({ "task": x.name, "correlation": (x.value * 100.0).round() / 100.0 })).collect::<Vec<_>>(), "notes": r.notes }))
        }
        "lookahead" => Ok(json!(progress::lookahead(&Calc::new(&doc.p, fmt, today), a["weeks"].as_u64().unwrap_or(3).clamp(1, 12) as u32))),
        "get_sheet" => {
            let Some(n) = s(a, "name") else { return Ok(json!(doc.p.sheets.iter().map(|x| x.name.clone()).collect::<Vec<_>>())) };
            let k = doc.p.sheets.iter().position(|x| x.name.eq_ignore_ascii_case(n)).ok_or_else(|| format!("No sheet called {n}."))?;
            let v = sheet::sheet_view(&Calc::new(&doc.p, fmt, today), k).ok_or("That sheet cannot be read.")?;
            Ok(json!({ "name": v.name, "cells": v.cells.iter().take(3000).map(|x| json!({ "cell": x.0, "typed": x.1, "shows": x.2 })).collect::<Vec<_>>() }))
        }
        "add_tasks" => {
            let list = a["tasks"].as_array().filter(|v| !v.is_empty()).ok_or("tasks is needed: a list like [{\"name\":\"FAT\",\"dur\":\"3d\"}].")?;
            if list.len() > 500 { return Err("Add at most 500 tasks at a time.".into()) }
            let at = match a["after_uid"].as_u64() { Some(x) => doc.p.idx(x as u32).ok_or("No task with after_uid.")? + 1, None => doc.p.tasks.len() };
            let rows: Vec<Vec<(String, String)>> = list.iter().map(|t| {
                let o = t.as_object().cloned().unwrap_or_default();
                let mut r: Vec<(String, String)> = vec![("name".into(), o.get("name").and_then(Value::as_str).unwrap_or("New task").into())];
                r.extend(o.iter().filter(|(k, _)| k.as_str() != "name").map(|(k, v)| (k.clone(), txt(v))));
                r
            }).collect();
            let first = doc.p.next_uid;
            apply(doc, Op::InsertRows { at, rows })?;
            Ok(json!({ "added": doc.p.tasks.iter().filter(|t| t.uid >= first).map(|t| json!({ "uid": t.uid, "name": t.name })).collect::<Vec<_>>() }))
        }
        "set_task_fields" => {
            let uid = u(a, "uid")?;
            let f = a["fields"].as_object().filter(|m| !m.is_empty()).ok_or("fields is needed, like {\"dur\":\"5d\"}.")?;
            let ops: Vec<Op> = f.iter().map(|(k, v)| Op::Set { uid, field: k.clone(), value: txt(v) }).collect();
            apply(doc, if ops.len() == 1 { ops.into_iter().next().ok_or("no field")? } else { Op::Batch(ops) })?;
            Ok(json!({ "changed": uid }))
        }
        "link_tasks" => {
            let kind = s(a, "type").unwrap_or("FS");
            let kind = serde_json::from_value(json!(kind)).map_err(|_| "type is FS, SS, FF or SF.".to_string())?;
            apply(doc, Op::LinkSet { pred: u(a, "pred_uid")?, succ: u(a, "succ_uid")?, kind: Some(kind), lag: s(a, "lag").unwrap_or("").into() })?;
            Ok(json!({ "linked": true }))
        }
        "unlink_tasks" => {
            apply(doc, Op::LinkSet { pred: u(a, "pred_uid")?, succ: u(a, "succ_uid")?, kind: None, lag: String::new() })?;
            Ok(json!({ "unlinked": true }))
        }
        "delete_tasks" => {
            let uids: Vec<u32> = a["uids"].as_array().map(|v| v.iter().filter_map(|x| x.as_u64().map(|x| x as u32)).collect()).unwrap_or_default();
            if uids.is_empty() { return Err("uids is needed.".into()) }
            apply(doc, Op::Delete { uids: uids.clone() })?;
            Ok(json!({ "deleted": uids }))
        }
        "update_progress" => {
            let uid = u(a, "uid")?;
            let mut ops = vec![];
            if let Some(x) = s(a, "actual_start") { ops.push(Op::Set { uid, field: "astart".into(), value: x.into() }) }
            if let Some(x) = a["pct"].as_u64() { ops.push(Op::Set { uid, field: "pct".into(), value: x.min(100).to_string() }) }
            if let Some(x) = s(a, "actual_finish") { ops.push(Op::Set { uid, field: "afinish".into(), value: x.into() }) }
            if ops.is_empty() { return Err("Give pct, actual_start or actual_finish.".into()) }
            apply(doc, Op::Batch(ops))?;
            Ok(json!({ "updated": uid }))
        }
        "set_status_date" => {
            let d = crate::parse::date_in(s(a, "date").ok_or("date is needed (YYYY-MM-DD).")?, fmt)?;
            apply(doc, Op::ProjectSet { field: "status".into(), value: json!(d.to_string()) })?;
            Ok(json!({ "status_date": d.to_string() }))
        }
        "set_baseline" => {
            let uids: Vec<u32> = a["uids"].as_array().map(|v| v.iter().filter_map(|x| x.as_u64().map(|x| x as u32)).collect()).unwrap_or_default();
            let n = a["n"].as_u64().unwrap_or(0).min(10) as u8;
            apply(doc, Op::Baseline { uids, clear: false, n })?;
            Ok(json!({ "baseline": n }))
        }
        "set_sheet_cells" => {
            let n = s(a, "name").ok_or("name is needed (the sheet).")?;
            let k = doc.p.sheets.iter().position(|x| x.name.eq_ignore_ascii_case(n)).ok_or_else(|| format!("No sheet called {n}."))?;
            let cells: Vec<(String, String)> = a["cells"].as_object().map(|m| m.iter().map(|(c, v)| (c.to_uppercase(), txt(v))).collect()).unwrap_or_default();
            if cells.is_empty() { return Err("cells is needed, like {\"A1\":\"Vendor\"}.".into()) }
            apply(doc, Op::SheetCells { k, cells, fmt: None })?;
            Ok(json!({ "sheet": doc.p.sheets[k].name }))
        }
        "list_resources" => {
            let c = Calc::new(&doc.p, fmt, today);
            Ok(json!(doc.p.resources.iter().enumerate().map(|(k, r)| json!({ "uid": r.uid, "id": k + 1, "name": r.name, "kind": res_text(&c, k, "kind"), "group": r.group, "max": res_text(&c, k, "max"),
                "rate": res_text(&c, k, "rate"), "calendar": r.cal, "work": res_text(&c, k, "work"), "cost": res_text(&c, k, "cost"), "overallocated": c.l.over.get(k).copied().unwrap_or(false) })).collect::<Vec<_>>()))
        }
        "add_resources" => {
            let list = a["resources"].as_array().filter(|v| !v.is_empty()).ok_or("resources is needed: a list like [{\"name\":\"Welder\",\"max\":\"400%\",\"rate\":\"60\"}].")?;
            let mut next = doc.p.resources.iter().map(|r| r.uid).max().unwrap_or(0);
            let (mut ops, mut added) = (vec![], vec![]);
            for r in list {
                let o = r.as_object().cloned().unwrap_or_default();
                let name = o.get("name").map(txt).filter(|n| !n.trim().is_empty()).ok_or("Each resource needs a name.")?;
                next += 1;
                ops.push(Op::ResInsert { name: name.clone() });
                ops.extend(res_ops(&doc.p, next, &o.into_iter().filter(|(k, _)| k != "name").collect())?);
                added.push(json!({ "uid": next, "name": name.trim() }));
            }
            apply(doc, Op::Batch(ops))?;
            Ok(json!({ "added": added }))
        }
        "set_resource_fields" => {
            let uid = u(a, "resource_uid")?;
            doc.p.res_idx(uid).ok_or("No resource with that uid.")?;
            let f = a["fields"].as_object().filter(|m| !m.is_empty()).ok_or("fields is needed, like {\"rate\":\"85\"}.")?;
            apply(doc, Op::Batch(res_ops(&doc.p, uid, f)?))?;
            Ok(json!({ "changed": uid }))
        }
        "delete_resources" => {
            let uids = uid_list(a, "resource_uids")?;
            apply(doc, Op::ResDelete { uids: uids.clone() })?;
            Ok(json!({ "deleted": uids }))
        }
        "assign_resource" | "unassign_resource" => {
            let tasks = uid_list(a, "uids")?;
            let k = res_named(&doc.p, a)?;
            let (r, add) = (&doc.p.resources[k], name == "assign_resource");
            let units = a.get("units").map(txt).filter(|x| !x.trim().is_empty());
            let mut ops = vec![];
            for uid in tasks {
                let i = doc.p.idx(uid).ok_or_else(|| format!("No task with uid {uid}."))?;
                let rest: Vec<Assign> = doc.p.tasks[i].res.iter().filter(|x| x.res != r.uid).cloned().collect();
                let mut v = parse::fmt_res(&rest, &doc.p);
                if add { v = [v, units.as_ref().map_or(r.name.clone(), |x| format!("{}[{x}]", r.name))].into_iter().filter(|x| !x.is_empty()).collect::<Vec<_>>().join(", ") }
                ops.push(Op::Set { uid, field: "res".into(), value: v });
            }
            apply(doc, Op::Batch(ops))?;
            Ok(json!({ "resource": doc.p.resources[k].name, if add { "assigned" } else { "removed" }: true }))
        }
        "get_assignments" => {
            let (c, p) = (Calc::new(&doc.p, fmt, today), &doc.p);
            if let Ok(uid) = u(a, "uid") {
                let i = p.idx(uid).ok_or("No task with that uid.")?;
                return Ok(json!(p.tasks[i].res.iter().filter_map(|x| Some(json!({ "resource": p.resources[p.res_idx(x.res)?].name, "units": units_text(p, x) }))).collect::<Vec<_>>()))
            }
            let r = p.resources[res_named(p, a).map_err(|_| "Give a task uid, or a resource name.".to_string())?].uid;
            Ok(json!((0..p.tasks.len()).filter_map(|i| p.tasks[i].res.iter().find(|x| x.res == r).map(|x| json!({ "uid": p.tasks[i].uid, "id": i + 1, "name": p.tasks[i].name, "units": units_text(p, x), "start": c.text(i, "start"), "finish": c.text(i, "finish"), "work": c.text(i, "work") }))).collect::<Vec<_>>()))
        }
        "list_calendars" => {
            let p = &doc.p;
            Ok(json!(p.calendars.iter().map(|c| json!({ "name": c.name, "project_calendar": c.name == p.calendar, "week": week_json(c), "exceptions": c.exceptions.len(),
                "tasks": p.tasks.iter().filter(|t| t.cal.as_deref() == Some(&c.name)).count(), "resources": p.resources.iter().filter(|r| r.cal.as_deref() == Some(&c.name)).count() })).collect::<Vec<_>>()))
        }
        "get_calendar" => {
            let n = s(a, "name").unwrap_or(&doc.p.calendar);
            let c = doc.p.calendars.iter().find(|c| c.name.eq_ignore_ascii_case(n)).ok_or_else(|| format!("No calendar called {n}."))?;
            Ok(json!({ "name": c.name, "project_calendar": c.name == doc.p.calendar, "week": week_json(c),
                "exceptions": c.exceptions.iter().map(|e| json!({ "name": e.name, "from": e.from.to_string(), "to": e.to.to_string(), "hours": if e.hours.is_empty() { "not working".into() } else { hours_text(&e.hours) } })).collect::<Vec<_>>() }))
        }
        "add_calendar" => {
            let n = s(a, "name").ok_or("name is needed.")?.trim();
            if doc.p.calendars.iter().any(|c| c.name.eq_ignore_ascii_case(n)) { return Err(format!("There is already a calendar called {n}; change it with set_calendar.")) }
            let mut cal = match s(a, "copy_from") { Some(f) => doc.p.calendars.iter().find(|c| c.name.eq_ignore_ascii_case(f)).cloned().ok_or_else(|| format!("No calendar called {f}."))?, None => Calendar::standard(n) };
            cal.name = n.into();
            set_week(&mut cal, &a["week"])?;
            apply(doc, Op::CalPut { old: None, cal })?;
            Ok(json!({ "added": n }))
        }
        "set_calendar" => {
            let n = s(a, "name").ok_or("name is needed.")?;
            let mut cal = doc.p.calendars.iter().find(|c| c.name.eq_ignore_ascii_case(n)).cloned().ok_or_else(|| format!("No calendar called {n}."))?;
            let old = cal.name.clone();
            set_week(&mut cal, &a["week"])?;
            let gone: Vec<String> = a["remove_exceptions"].as_array().into_iter().flatten().map(|x| txt(x).to_lowercase()).collect();
            cal.exceptions.retain(|e| !gone.contains(&e.name.to_lowercase()));
            for e in a["add_exceptions"].as_array().into_iter().flatten() {
                let from = parse::date_in(s(e, "from").ok_or("Each exception needs from (YYYY-MM-DD).")?, fmt)?;
                let to = s(e, "to").map(|x| parse::date_in(x, fmt)).transpose()?.unwrap_or(from);
                cal.exceptions.push(Exception { name: s(e, "name").unwrap_or("Holiday").into(), from, to, hours: hours(s(e, "hours").unwrap_or(""))? });
            }
            if let Some(r) = s(a, "rename") { cal.name = r.trim().into() }
            let name = cal.name.clone();
            apply(doc, Op::CalPut { old: Some(old), cal })?;
            Ok(json!({ "calendar": name }))
        }
        "delete_calendar" => {
            let n = s(a, "name").ok_or("name is needed.")?;
            let p = &doc.p;
            let c = p.calendars.iter().find(|c| c.name.eq_ignore_ascii_case(n)).ok_or_else(|| format!("No calendar called {n}."))?.name.clone();
            let used = p.tasks.iter().filter(|t| t.cal.as_deref() == Some(&c)).count() + p.resources.iter().filter(|r| r.cal.as_deref() == Some(&c)).count();
            if used > 0 { return Err(format!("{used} tasks or resources use {c}; give them another calendar first.")) }
            apply(doc, Op::CalDelete { name: c.clone() })?;
            Ok(json!({ "deleted": c }))
        }
        "list_codes" => {
            let p = &doc.p;
            Ok(json!(p.codes.iter().map(|c| json!({ "name": c.name, "values": c.values.iter().map(|v| json!({ "value": v.value, "desc": v.desc,
                "tasks": p.tasks.iter().filter(|t| t.codes.get(&c.name) == Some(&v.value)).count() })).collect::<Vec<_>>() })).collect::<Vec<_>>()))
        }
        "define_code" => {
            let n = s(a, "name").ok_or("name is needed.")?.trim().to_string();
            if a["remove"].as_bool() == Some(true) {
                let c = doc.p.codes.iter().find(|c| c.name.eq_ignore_ascii_case(&n)).ok_or_else(|| format!("No code called {n}."))?.name.clone();
                apply(doc, Op::CodeDef { old: Some(c.clone()), def: None })?;
                return Ok(json!({ "removed": c }))
            }
            let from = s(a, "rename_from").unwrap_or(&n).to_string();
            let old = doc.p.codes.iter().find(|c| c.name.eq_ignore_ascii_case(&from)).cloned();
            if s(a, "rename_from").is_some() && old.is_none() { return Err(format!("No code called {from}.")) }
            let mut def = old.clone().unwrap_or_default();
            def.name = n.clone();
            if let Some(v) = a["values"].as_array() {
                def.values = v.iter().map(|x| {
                    let (value, desc) = if x.is_object() { (s(x, "value").unwrap_or("").trim().to_string(), s(x, "desc").unwrap_or("").to_string()) } else { (txt(x).trim().to_string(), String::new()) };
                    let color = old.as_ref().and_then(|o| o.values.iter().find(|w| w.value == value)).map(|w| w.color.clone()).unwrap_or_default();
                    CodeValue { value, desc, color }
                }).filter(|v| !v.value.is_empty()).collect();
            }
            apply(doc, Op::CodeDef { old: old.map(|o| o.name), def: Some(def) })?;
            Ok(json!({ "code": n }))
        }
        "set_task_codes" => {
            let uids = uid_list(a, "uids")?;
            let c = s(a, "code").ok_or("code is needed (from list_codes).")?;
            let c = doc.p.codes.iter().find(|x| x.name.eq_ignore_ascii_case(c)).ok_or_else(|| format!("No code called {c}; add it with define_code."))?.name.clone();
            let v = a.get("value").map(txt).unwrap_or_default();
            apply(doc, Op::Batch(uids.iter().map(|&uid| Op::Set { uid, field: format!("code:{c}"), value: v.trim().into() }).collect()))?;
            Ok(json!({ "code": c, "tasks": uids.len() }))
        }
        "list_custom_fields" => {
            let p = &doc.p;
            Ok(json!(p.fields.iter().map(|d| json!({ "key": d.key, "alias": d.alias, "formula": d.formula, "lookup": d.lookup.iter().map(|l| &l.value).collect::<Vec<_>>(), "restrict": d.restrict,
                "tasks_with_value": p.tasks.iter().filter(|t| t.custom.contains_key(&d.key)).count() })).collect::<Vec<_>>()))
        }
        "define_custom_field" => {
            let k = s(a, "key").ok_or("key is needed, like text1 or number2.")?.trim().trim_start_matches("c:").to_lowercase();
            if !custom_key(&k) { return Err(format!("\"{k}\" is not a custom field: text1–30, number1–20, flag1–20, date1–10, cost1–10, duration1–10, start1–10, finish1–10, outline1–10.")) }
            let mut def = doc.p.fields.iter().find(|d| d.key == k).cloned().unwrap_or_else(|| CustomDef { key: k.clone(), ..Default::default() });
            if let Some(x) = a.get("alias") { def.alias = txt(x).trim().into() }
            if let Some(x) = a.get("formula") { def.formula = txt(x).trim().into() }
            if let Some(v) = a["lookup"].as_array() { def.lookup = v.iter().map(|x| LookupItem { value: txt(x), desc: String::new() }).collect() }
            if let Some(x) = a["restrict"].as_bool() { def.restrict = x }
            apply(doc, Op::FieldDef { def })?;
            Ok(json!({ "field": k, "set_with": format!("c:{k}") }))
        }
        "delete_custom_field" => {
            let k = s(a, "key").ok_or("key is needed.")?.trim().trim_start_matches("c:").to_lowercase();
            if !doc.p.fields.iter().any(|d| d.key == k) && !doc.p.tasks.iter().any(|t| t.custom.contains_key(&k)) { return Err(format!("{k} is not in use.")) }
            apply(doc, Op::DeleteField { key: k.clone() })?;
            Ok(json!({ "deleted": k }))
        }
        "set_constraint" => {
            let uids = uid_list(a, "uids")?;
            let t = parse::constraint(s(a, "type").ok_or("type is needed (ASAP, SNET, MFO…).")?)?;
            let dated = !matches!(t, Constraint::ASAP | Constraint::ALAP);
            let d = s(a, "date").map(|x| parse::date_in(x, fmt)).transpose()?;
            if dated && d.is_none() { return Err("This constraint needs a date (YYYY-MM-DD).".into()) }
            let ty = format!("{t:?}");
            apply(doc, Op::Batch(uids.iter().flat_map(|&uid| [Some(Op::Set { uid, field: "ctype".into(), value: ty.clone() }), d.filter(|_| dated).map(|d| Op::Set { uid, field: "cdate".into(), value: d.to_string() })]).flatten().collect()))?;
            Ok(json!({ "constraint": ty, "tasks": uids.len() }))
        }
        "set_deadline" => {
            let uids = uid_list(a, "uids")?;
            let d = s(a, "date").map(|x| parse::date_in(x, fmt)).transpose()?.map(|d| d.to_string()).unwrap_or_default();
            apply(doc, Op::Batch(uids.iter().map(|&uid| Op::Set { uid, field: "deadline".into(), value: d.clone() }).collect()))?;
            Ok(json!({ "deadline": d, "tasks": uids.len() }))
        }
        "list_filters" => {
            let saved: Vec<Filter> = serde_json::from_value(doc.p.ui["filters"].clone()).unwrap_or_default();
            let f = |x: &Filter, kind: &str| json!({ "name": x.name, "kind": kind, "rules": x.rules.iter().map(|r| json!({ "field": r.field, "test": r.test, "value": r.value, "value2": r.value2, "or": r.or })).collect::<Vec<_>>() });
            Ok(json!(builtin_filters().iter().map(|x| f(x, "built-in")).chain(saved.iter().map(|x| f(x, "saved"))).collect::<Vec<_>>()))
        }
        "filter_tasks" => {
            let c = Calc::new(&doc.p, fmt, today);
            let keys = field_keys(&c, a)?;
            let f = match (s(a, "name"), a["rules"].as_array()) {
                (_, Some(r)) if !r.is_empty() => Filter { rules: r.iter().map(|x| serde_json::from_value::<Rule>(x.clone()).map_err(|e| format!("A rule is {{field, test, value}}: {e}"))).collect::<Result<_, _>>()?, ..Default::default() },
                (Some(n), _) => {
                    let saved: Vec<Filter> = serde_json::from_value(doc.p.ui["filters"].clone()).unwrap_or_default();
                    builtin_filters().into_iter().chain(saved).find(|x| x.name.eq_ignore_ascii_case(n)).ok_or_else(|| format!("No filter called {n}; see list_filters."))?
                }
                _ => return Err("Give a filter name (list_filters) or rules.".into()),
            };
            if let Some(r) = f.rules.iter().find(|r| c.key_of(&r.field).is_none()) { return Err(format!("No field \"{}\"; see list_fields.", r.field)) }
            Ok(page(&c, (0..doc.p.tasks.len()).filter(|&i| c.matches(i, &f)).collect(), &keys, a))
        }
        "indent_tasks" | "outdent_tasks" => {
            let uids = uid_list(a, "uids")?;
            apply(doc, if name == "indent_tasks" { Op::Indent { uids: uids.clone() } } else { Op::Outdent { uids: uids.clone() } })?;
            Ok(json!({ "tasks": uids.len() }))
        }
        "move_tasks" => {
            let uids = uid_list(a, "uids")?;
            let up = match s(a, "direction") { Some("up") => true, Some("down") => false, _ => return Err("direction is up or down.".into()) };
            let n = a["rows"].as_u64().unwrap_or(1).clamp(1, 200) as usize;
            apply(doc, Op::Batch(vec![Op::Move { uids: uids.clone(), up }; n]))?;
            Ok(json!({ "moved": uids.len(), "rows": n }))
        }
        "get_columns" => {
            let c = Calc::new(&doc.p, fmt, today);
            Ok(json!(entry_columns(&doc.p).iter().map(|(k, w)| json!({ "key": k, "name": c.name(k), "width": w.abs(), "hidden": *w < 0 })).collect::<Vec<_>>()))
        }
        "set_columns" => {
            let c = Calc::new(&doc.p, fmt, today);
            let key = |x: &Value| -> Result<String, String> { let n = txt(x); c.key_of(&n).ok_or_else(|| format!("No field \"{n}\"; see list_fields.")) };
            let mut cols = entry_columns(&doc.p);
            for k in a["hide"].as_array().into_iter().flatten().map(key).collect::<Result<Vec<_>, _>>()? {
                if k == "name" { return Err("The Task Name column stays.".into()) }
                if let Some(x) = cols.iter_mut().find(|x| x.0 == k) { x.1 = -x.1.abs() }
            }
            let show = a["show"].as_array().into_iter().flatten().map(key).collect::<Result<Vec<_>, _>>()?;
            let mut at = match s(a, "after") { Some(x) => { let k = key(&json!(x))?; cols.iter().position(|c| c.0 == k).ok_or_else(|| format!("{x} is not a column of the task sheet."))? + 1 } None => cols.len() };
            for k in show {
                match cols.iter().position(|x| x.0 == k) {
                    Some(i) if s(a, "after").is_none() => cols[i].1 = cols[i].1.abs(),
                    Some(i) => { let mut x = cols.remove(i); x.1 = x.1.abs(); if i < at { at -= 1 } cols.insert(at, x); at += 1 }
                    None => { cols.insert(at.min(cols.len()), (k, 100)); at += 1 }
                }
            }
            if a["show"].as_array().is_none_or(|v| v.is_empty()) && a["hide"].as_array().is_none_or(|v| v.is_empty()) { return Err("Give show or hide, a list of fields.".into()) }
            apply(doc, Op::Columns { cols: cols.clone() })?;
            Ok(json!(cols.iter().filter(|x| x.1 > 0).map(|x| x.0.clone()).collect::<Vec<_>>()))
        }
        "set_task_notes" => {
            let uid = u(a, "uid")?;
            let i = doc.p.idx(uid).ok_or("No task with that uid.")?;
            let n = a.get("notes").map(txt).ok_or("notes is needed.")?;
            let old = &doc.p.tasks[i].notes;
            let v = if a["append"].as_bool() == Some(true) && !old.is_empty() { format!("{old}\n{n}") } else { n };
            apply(doc, Op::Set { uid, field: "notes".into(), value: v })?;
            Ok(json!({ "notes": doc.p.tasks[i].notes }))
        }
        "list_risks" => Ok(json!(doc.p.risks.iter().map(|r| json!({ "id": r.id, "name": r.name, "cause": r.cause, "prob": r.prob, "impact": r.impact, "score": r.prob as u32 * r.impact as u32,
            "owner": r.owner, "response": r.response, "status": r.status, "kind": r.kind, "days": r.days, "pct": r.pct, "days_min": r.days_min, "days_max": r.days_max,
            "tasks": r.tasks.iter().filter_map(|u| doc.p.idx(*u).map(|i| json!({ "uid": u, "id": i + 1, "name": doc.p.tasks[i].name }))).collect::<Vec<_>>() })).collect::<Vec<_>>())),
        "set_risk" => {
            let id = a["id"].as_u64().unwrap_or(0) as u32;
            let mut r = if id == 0 { Risk { prob: 3, impact: 3, status: "Open".into(), kind: "Threat".into(), ..Default::default() } } else { doc.p.risks.iter().find(|r| r.id == id).cloned().ok_or("No risk with that id; see list_risks.")? };
            for k in ["name", "cause", "owner", "response", "status", "kind"] {
                if let Some(v) = a.get(k).filter(|v| !v.is_null()).map(txt) {
                    let v = v.trim().to_string();
                    match k { "name" => r.name = v, "cause" => r.cause = v, "owner" => r.owner = v, "response" => r.response = v, "status" => r.status = v, _ => r.kind = v }
                }
            }
            let score = |k: &str, d: u8| -> Result<u8, String> { match a[k].as_u64() { Some(x) if (1..=5).contains(&x) => Ok(x as u8), Some(_) => Err(format!("{k} is from 1 to 5.")), None => Ok(d) } };
            (r.prob, r.impact) = (score("prob", r.prob)?, score("impact", r.impact)?);
            for (k, f) in [("days", &mut r.days), ("pct", &mut r.pct), ("days_min", &mut r.days_min), ("days_max", &mut r.days_max)] {
                if let Some(x) = a[k].as_f64() { if !(0.0..=100_000.0).contains(&x) || (k == "pct" && x > 100.0) { return Err(format!("{k} is out of range.")) } *f = x }
            }
            if a.get("task_uids").is_some() { r.tasks = uid_list(a, "task_uids").unwrap_or_default(); if let Some(u) = r.tasks.iter().find(|u| doc.p.idx(**u).is_none()) { return Err(format!("No task with uid {u}.")) } }
            if r.name.trim().is_empty() { return Err("A risk needs a name.".into()) }
            apply(doc, Op::Risk { id, risk: Some(r) })?;
            Ok(json!({ "risk": if id == 0 { doc.p.risks.iter().map(|r| r.id).max().unwrap_or(0) } else { id } }))
        }
        "delete_risk" => {
            let id = a["id"].as_u64().ok_or("id is needed (from list_risks).")? as u32;
            if !doc.p.risks.iter().any(|r| r.id == id) { return Err("No risk with that id.".into()) }
            apply(doc, Op::Risk { id, risk: None })?;
            Ok(json!({ "deleted": id }))
        }
        "list_delay_events" => {
            let p = &doc.p;
            let name = |u: &u32| p.idx(*u).map(|i| p.tasks[i].name.clone()).unwrap_or_default();
            Ok(json!(p.delays.iter().map(|e| json!({ "id": e.id, "name": e.name, "date": e.date.map(|d| d.to_string()), "description": e.description, "on": e.on,
                "extend": e.extend.iter().map(|(u, m)| json!({ "uid": u, "task": name(u), "days": *m as f64 / p.day_min.max(1) as f64 })).collect::<Vec<_>>(),
                "hold": e.hold.iter().map(|(u, d)| json!({ "uid": u, "task": name(u), "date": d.to_string() })).collect::<Vec<_>>(),
                "frag": e.frag.iter().map(|f| json!({ "name": f.name, "days": f.dur as f64 / p.day_min.max(1) as f64, "after": f.after, "before": f.before })).collect::<Vec<_>>() })).collect::<Vec<_>>()))
        }
        "set_delay_event" => {
            let id = a["id"].as_u64().unwrap_or(0) as u32;
            let p = &doc.p;
            let mut e = if id == 0 { DelayEvent { on: true, ..Default::default() } } else { p.delays.iter().find(|e| e.id == id).cloned().ok_or("No delay event with that id; see list_delay_events.")? };
            if let Some(v) = s(a, "name") { e.name = v.trim().into() }
            if let Some(v) = a.get("description").map(txt) { e.description = v }
            if let Some(v) = a["on"].as_bool() { e.on = v }
            if let Some(v) = s(a, "date") { e.date = Some(parse::date_in(v, fmt)?) }
            let mins = |x: &Value| -> Result<i64, String> { x.as_f64().filter(|d| (0.0..=100_000.0).contains(d)).map(|d| (d * p.day_min as f64).round() as i64).ok_or_else(|| "days is a number from 0 up.".to_string()) };
            let task = |x: &Value| -> Result<u32, String> { x["uid"].as_u64().map(|u| u as u32).filter(|u| p.idx(*u).is_some()).ok_or_else(|| "Each entry needs the uid of a task in the plan.".to_string()) };
            if let Some(v) = a["extend"].as_array() { e.extend = v.iter().map(|x| Ok((task(x)?, mins(&x["days"])?))).collect::<Result<_, String>>()? }
            if let Some(v) = a["hold"].as_array() { e.hold = v.iter().map(|x| Ok((task(x)?, parse::date_in(s(x, "date").ok_or("Each hold needs a date.")?, fmt)?))).collect::<Result<_, String>>()? }
            if let Some(v) = a["frag"].as_array() {
                let list = |x: &Value| -> Vec<u32> { x.as_array().map(|v| v.iter().filter_map(|u| u.as_u64().map(|u| u as u32)).collect()).unwrap_or_default() };
                e.frag = v.iter().map(|x| Ok(FragTask { name: s(x, "name").ok_or("Each new activity needs a name.")?.trim().into(), dur: mins(&x["days"])?, after: list(&x["after"]), before: list(&x["before"]) })).collect::<Result<_, String>>()?;
            }
            if e.name.is_empty() { return Err("A delay event needs a name.".into()) }
            apply(doc, Op::Delay { id, ev: Some(e) })?;
            Ok(json!({ "event": if id == 0 { doc.p.delays.iter().map(|e| e.id).max().unwrap_or(0) } else { id } }))
        }
        "delete_delay_event" | "add_fragnet" => {
            let id = a["id"].as_u64().ok_or("id is needed (from list_delay_events).")? as u32;
            if !doc.p.delays.iter().any(|e| e.id == id) { return Err("No delay event with that id.".into()) }
            apply(doc, if name == "add_fragnet" { Op::DelayFrag { id } } else { Op::Delay { id, ev: None } })?;
            Ok(json!({ "event": id }))
        }
        "delay_analysis" => {
            let c = Calc::new(&doc.p, fmt, today);
            if let Some(e) = &c.error { return Err(e.clone()) }
            let mut v = match s(a, "method").unwrap_or("available") {
                "available" => json!(crate::delay::available(&c)),
                "apab" => json!(crate::delay::apab(&c)),
                "windows" => json!(crate::delay::windows(&c)),
                "tia" => json!(crate::delay::tia(&c)?),
                "iap" => json!(crate::delay::iap(&c)?),
                "cab" => json!(crate::delay::cab(&c)?),
                m => return Err(format!("No method {m}: available, apab, windows, tia, iap or cab.")),
            };
            dates(&mut v, &["bs", "bf", "s", "f", "planned_finish", "finish", "finish_before", "finish_after", "was", "now", "start_finish", "end_finish", "before", "after"]);
            Ok(v)
        }
        "list_snapshots" => Ok(json!(doc.p.snapshots.iter().map(|x| json!({ "date": x.date.map(|d| d.to_string()), "label": x.label, "comment": x.comment, "finish": inst(x.finish),
            "complete_pct": (x.pct * 100.0).round() / 100.0, "cost": x.cost, "health": x.health })).collect::<Vec<_>>())),
        "take_snapshot" => {
            if doc.p.status.is_none() { return Err("Set the status date first (set_status_date).".into()) }
            apply(doc, Op::Snapshot { label: s(a, "label").unwrap_or("").into(), comment: s(a, "comment").unwrap_or("").into() })?;
            Ok(json!({ "snapshots": doc.p.snapshots.len() }))
        }
        "period_report" => {
            let c = Calc::new(&doc.p, fmt, today);
            if let Some(e) = &c.error { return Err(e.clone()) }
            let end = s(a, "end").map(|x| parse::date_in(x, fmt)).transpose()?.map(|d| crate::cal::at(d).div_euclid(crate::cal::DAY));
            let mut v = json!(report::period_report(&c, if s(a, "period") == Some("week") { "week" } else { "month" }, end));
            if let Some(h) = v["hours"].as_array_mut() { for x in h { if let Some(d) = x[0].as_i64() { x[0] = json!(crate::delay::date(d)) } } }
            dates(&mut v, &["from", "to", "prev_from"]);
            Ok(v)
        }
        "milestone_trend" => {
            let c = Calc::new(&doc.p, fmt, today);
            if let Some(e) = &c.error { return Err(e.clone()) }
            Ok(json!(report::milestone_trend(&c).iter().map(|t| json!({ "uid": t.uid, "name": t.name, "baseline": t.baseline.map(crate::delay::date),
                "forecasts": t.points.iter().map(|(at, f)| json!({ "on": crate::delay::date(*at), "finish": crate::delay::date(*f) })).collect::<Vec<_>>() })).collect::<Vec<_>>()))
        }
        "earned_value" => {
            let c = Calc::new(&doc.p, fmt, today);
            if let Some(e) = &c.error { return Err(e.clone()) }
            let leaf = |i: &usize| !c.s.summary[*i] && !c.s.off[*i];
            let sum = |v: &[f64]| (0..v.len()).filter(leaf).map(|i| v[i]).sum::<f64>();
            let (pv, ev, ac, bac) = (sum(&c.ev.pv), sum(&c.ev.ev), sum(&c.ev.ac), sum(&c.ev.bac));
            let r2 = |x: f64| (x * 100.0).round() / 100.0;
            let es = progress::earned_schedule(&c);
            Ok(json!({ "status_date": doc.p.status.map(|d| d.to_string()), "pv": r2(pv), "ev": r2(ev), "ac": r2(ac), "bac": r2(bac), "sv": r2(ev - pv), "cv": r2(ev - ac),
                "spi": (pv > 0.0).then(|| r2(ev / pv)), "cpi": (ac > 0.0).then(|| r2(ev / ac)), "eac": (ev > 0.0 && ac > 0.0).then(|| r2(bac / (ev / ac))),
                "earned_schedule": es.map(|e| json!({ "spi_t": r2(e.spi_t), "sv_t_days": r2(e.sv_t), "planned_finish": inst(e.planned_finish), "forecast_finish": inst(e.forecast_finish) })) }))
        }
        "get_health_settings" => {
            let h = &doc.p.health;
            let sc = check::health(&Calc::new(&doc.p, fmt, today));
            Ok(json!({ "limits": h.limits, "weights": check::WEIGHTS.iter().map(|(k, _)| (k.to_string(), json!(check::weight(h, k)))).collect::<serde_json::Map<_, _>>(), "score": sc.score, "grade": sc.grade }))
        }
        "set_health_settings" => {
            let mut h = doc.p.health.clone();
            if let Some(m) = a["limits"].as_object() {
                let mut l = serde_json::to_value(&h.limits).map_err(|e| e.to_string())?;
                for (k, v) in m { if l.get(k).is_none() { return Err(format!("No limit {k}: pct, high_float, high_dur, long_lag, index.")) } l[k] = v.clone() }
                h.limits = serde_json::from_value(l).map_err(|e| format!("Limits are numbers: {e}"))?;
            }
            for (k, v) in a["weights"].as_object().into_iter().flatten() {
                if !check::WEIGHTS.iter().any(|w| w.0 == k) { return Err(format!("No check {k}; see get_health_settings.")) }
                h.weights.insert(k.clone(), v.as_f64().filter(|x| (0.0..=100.0).contains(x)).ok_or("A weight is a number from 0 to 100.")?);
            }
            apply(doc, Op::Health { h })?;
            Ok(json!({ "score": check::health(&Calc::new(&doc.p, fmt, today)).score }))
        }
        "level_preview" => {
            let only: Vec<u32> = uid_list(a, "uids").unwrap_or_default();
            let r = crate::level::preview(&doc.p, &only)?;
            Ok(json!({ "finish": inst(r.finish), "new_finish": inst(r.new_finish), "over_allocated_days_before": r.over_before, "over_allocated_days_after": r.over_after,
                "moved": r.moved.iter().take(200).map(|m| json!({ "uid": m.uid, "name": m.name, "start": crate::cal::date_of(m.start).to_string(), "new_start": crate::cal::date_of(m.new_start).to_string(), "finish": inst(m.finish), "new_finish": inst(m.new_finish) })).collect::<Vec<_>>(), "moved_count": r.moved.len() }))
        }
        "level_resources" | "clear_levelling" => {
            let uids: Vec<u32> = uid_list(a, "uids").unwrap_or_default();
            apply(doc, Op::Level { uids, clear: name == "clear_levelling" })?;
            Ok(summary(&Calc::new(&doc.p, fmt, today)))
        }
        "update_project" => {
            let d = parse::date_in(s(a, "through").ok_or("through is needed (YYYY-MM-DD).")?, fmt)?;
            apply(doc, Op::UpdateProject { through: d.to_string(), reschedule: a["reschedule"].as_bool() == Some(true) })?;
            Ok(summary(&Calc::new(&doc.p, fmt, today)))
        }
        "mark_on_track" => {
            if doc.p.status.is_none() { return Err("Set the status date first (set_status_date).".into()) }
            let uids = uid_list(a, "uids")?;
            apply(doc, Op::MarkOnTrack { uids: uids.clone() })?;
            Ok(json!({ "tasks": uids.len() }))
        }
        "split_task" => {
            let uid = u(a, "uid")?;
            let m = |k: &str| -> Result<i64, String> { parse::dur(s(a, k).ok_or_else(|| format!("{k} is needed, like 2d."))?, doc.p.day_min).map(|x| x.0) };
            apply(doc, Op::Split { uid, after: m("after")?, gap: m("gap")? })?;
            Ok(json!({ "split": uid }))
        }
        "save_interim" => {
            let n = a["n"].as_u64().filter(|n| (1..=10).contains(n)).ok_or("n is from 1 to 10.")? as u8;
            apply(doc, Op::Interim { n })?;
            Ok(json!({ "saved_as": format!("Start{n} and Finish{n}") }))
        }
        "list_constraint_log" => Ok(json!(doc.p.log.iter().map(|x| json!({ "id": x.id, "uid": x.uid, "task": doc.p.idx(x.uid).map(|i| doc.p.tasks[i].name.clone()), "kind": x.kind, "text": x.text,
            "owner": x.owner, "need": x.need.map(|d| d.to_string()), "status": x.status })).collect::<Vec<_>>())),
        "set_constraint_log_item" => {
            let id = a["id"].as_u64().unwrap_or(0) as u32;
            let old = doc.p.log.iter().find(|x| x.id == id).cloned();
            if id != 0 && old.is_none() { return Err("No entry with that id; see list_constraint_log.".into()) }
            if a["remove"].as_bool() == Some(true) { if id == 0 { return Err("id is needed to remove an entry.".into()) } apply(doc, Op::Log { id, item: None })?; return Ok(json!({ "removed": id })) }
            let mut x = old.unwrap_or_default();
            if let Some(v) = a["uid"].as_u64() { x.uid = v as u32 }
            for k in ["kind", "text", "owner", "status"] { if let Some(v) = a.get(k).filter(|v| !v.is_null()).map(txt) { match k { "kind" => x.kind = v, "text" => x.text = v, "owner" => x.owner = v, _ => x.status = v } } }
            if let Some(v) = a.get("need").map(txt) { x.need = if v.trim().is_empty() { None } else { Some(parse::date_in(&v, fmt)?) } }
            apply(doc, Op::Log { id, item: Some(x) })?;
            Ok(json!({ "entry": if id == 0 { doc.p.log.iter().map(|x| x.id).max().unwrap_or(0) } else { id } }))
        }
        "resource_usage" => {
            let c = Calc::new(&doc.p, fmt, today);
            if let Some(e) = &c.error { return Err(e.clone()) }
            let unit = if s(a, "unit") == Some("week") { "week" } else { "month" };
            let r1 = |x: f64| (x * 10.0).round() / 10.0;
            if s(a, "by") == Some("trade") {
                let m = progress::manpower(&c, unit);
                return Ok(json!({ "unit": "people", "periods": m.buckets.iter().map(|d| crate::delay::date(*d)).collect::<Vec<_>>(),
                    "trades": m.trades.iter().map(|t| json!({ "trade": t.name, "planned": t.planned.iter().map(|x| r1(*x)).collect::<Vec<_>>(), "forecast": t.forecast.iter().map(|x| x.map(r1)).collect::<Vec<_>>() })).collect::<Vec<_>>() }))
            }
            let g = report::usage(&c, if s(a, "by") == Some("task") { "task" } else { "resource" }, if s(a, "what") == Some("cost") { "cost" } else { "work" }, unit);
            Ok(json!({ "unit": g.unit, "periods": g.buckets.iter().map(|d| crate::delay::date(*d)).collect::<Vec<_>>(),
                "rows": g.rows.iter().take(300).map(|r| json!({ "name": r.name, "total": r1(r.total), "values": r.vals.iter().map(|x| r1(*x)).collect::<Vec<_>>(), "over": r.over.iter().any(|x| *x) })).collect::<Vec<_>>() }))
        }
        "contract_status" => {
            let c = Calc::new(&doc.p, fmt, today);
            if let Some(e) = &c.error { return Err(e.clone()) }
            Ok(json!({ "number": doc.p.contract.number, "status": crate::contract::status(&c) }))
        }
        "set_contract" => {
            let f = json!({ "number": a.get("number"), "value": a.get("value"), "warn_days": a.get("warn_days") });
            let f = Value::Object(f.as_object().cloned().unwrap_or_default().into_iter().filter(|(_, v)| !v.is_null()).collect());
            if f.as_object().is_none_or(|m| m.is_empty()) { return Err("Give number, value or warn_days.".into()) }
            let c = patch(&doc.p.contract, &f, &["dates", "payments"])?;
            apply(doc, Op::Contract { c: Box::new(c) })?;
            Ok(json!({ "number": doc.p.contract.number, "value": doc.p.contract.value }))
        }
        "set_contract_date" | "set_payment" | "set_vendor_document" | "log_submission" | "set_order" => {
            let id = a["id"].as_u64().unwrap_or(0) as u32;
            let remove = a["remove"].as_bool() == Some(true);
            let f = &a["fields"];
            let p = &doc.p;
            let task_ok = |v: &Value, k: &str| -> Result<(), String> { match v[k].as_u64() { Some(u) if u > 0 && p.idx(u as u32).is_none() => Err(format!("No task with uid {u}.")), _ => Ok(()) } };
            for k in ["task", "need"] { task_ok(f, k)? }
            fn put<T: serde::Serialize + serde::de::DeserializeOwned + Default + Clone>(list: &mut Vec<T>, id: u32, idof: impl Fn(&T) -> u32, f: &Value, remove: bool) -> Result<(), String> {
                let k = list.iter().position(|x| idof(x) == id);
                if id != 0 && k.is_none() { return Err("No entry with that id.".into()) }
                match (k, remove) {
                    (Some(k), true) => { list.remove(k); }
                    (None, true) => return Err("id is needed to remove an entry.".into()),
                    (Some(k), false) => list[k] = patch(&list[k], f, &["id"])?,
                    (None, false) => list.push(patch(&T::default(), f, &["id"])?),
                }
                Ok(())
            }
            let op = match name {
                "set_contract_date" | "set_payment" => {
                    let mut c = p.contract.clone();
                    if name == "set_payment" { put(&mut c.payments, id, |x| x.id, f, remove)? } else { put(&mut c.dates, id, |x| x.id, f, remove)? }
                    Op::Contract { c: Box::new(c) }
                }
                "set_vendor_document" | "log_submission" => {
                    let mut r = p.documents.clone();
                    if name == "log_submission" {
                        let doc_id = f["doc"].as_u64().map(|d| d as u32).or_else(|| r.log.iter().find(|x| x.id == id).map(|x| x.doc));
                        if !doc_id.is_some_and(|d| r.docs.iter().any(|x| x.id == d)) { return Err("doc is needed: a document id from document_register.".into()) }
                        put(&mut r.log, id, |x| x.id, f, remove)?
                    } else { put(&mut r.docs, id, |x| x.id, f, remove)? }
                    Op::Documents { r: Box::new(r) }
                }
                _ => { let mut r = p.procurement.clone(); put(&mut r.items, id, |x| x.id, f, remove)?; Op::Procurement { r: Box::new(r) } }
            };
            apply(doc, op)?;
            let p = &doc.p;
            let last = match name { "set_contract_date" => p.contract.dates.last().map(|x| x.id), "set_payment" => p.contract.payments.last().map(|x| x.id),
                "set_vendor_document" => p.documents.docs.last().map(|x| x.id), "log_submission" => p.documents.log.last().map(|x| x.id), _ => p.procurement.items.last().map(|x| x.id) };
            Ok(json!({ if remove { "removed" } else { "id" }: if id == 0 { last.unwrap_or(0) } else { id } }))
        }
        "log_expediting" => {
            let id = a["id"].as_u64().ok_or("id is needed (an order id from procurement_status).")? as u32;
            let mut r = doc.p.procurement.clone();
            let it = r.items.iter_mut().find(|x| x.id == id).ok_or("No order with that id.")?;
            let date = Some(s(a, "date").map(|x| parse::date_in(x, fmt)).transpose()?.unwrap_or(today));
            it.log.push(Expedite { date, note: s(a, "note").ok_or("note is needed.")?.into(), by: s(a, "by").unwrap_or("").into() });
            apply(doc, Op::Procurement { r: Box::new(r) })?;
            Ok(json!({ "order": id }))
        }
        "document_register" | "procurement_status" => {
            let c = Calc::new(&doc.p, fmt, today);
            if let Some(e) = &c.error { return Err(e.clone()) }
            let mut v = if name == "document_register" { json!({ "documents": doc.p.documents.docs, "status": crate::docreg::status(&c) }) } else { json!({ "orders": doc.p.procurement.items, "status": crate::procure::status(&c) }) };
            if let Some(m) = v["status"].as_object_mut() { m.remove("weekly"); m.remove("monthly"); }
            Ok(v)
        }
        "list_baselines" => {
            let p = &doc.p;
            Ok(json!((0..=10u8).filter_map(|n| {
                let b: Vec<&Baseline> = p.tasks.iter().filter_map(|t| t.bls.get(&n)).collect();
                (!b.is_empty()).then(|| json!({ "n": n, "tasks": b.len(), "finish": inst(b.iter().map(|x| x.finish).max().unwrap_or(0)) }))
            }).collect::<Vec<_>>()))
        }
        "baseline_variance" => {
            let c = Calc::new(&doc.p, fmt, today);
            if let Some(e) = &c.error { return Err(e.clone()) }
            let n = a["n"].as_u64().unwrap_or(0).min(10) as u8;
            let day = crate::cal::DAY as f64;
            let mut rows: Vec<(f64, Value)> = (0..doc.p.tasks.len()).filter(|&i| !c.s.summary[i] && !c.s.off[i]).filter_map(|i| {
                let (t, b) = (&doc.p.tasks[i], doc.p.tasks[i].bls.get(&n)?);
                let (sv, fv) = ((c.s.es[i] - b.start) as f64 / day, (c.s.ef[i] - b.finish) as f64 / day);
                Some((fv, json!({ "uid": t.uid, "id": i + 1, "name": t.name, "baseline_start": crate::cal::date_of(b.start).to_string(), "start": crate::cal::date_of(c.s.es[i]).to_string(), "start_var_days": (sv * 10.0).round() / 10.0,
                    "baseline_finish": inst(b.finish), "finish": inst(c.s.ef[i]), "finish_var_days": (fv * 10.0).round() / 10.0 })))
            }).collect();
            if rows.is_empty() { return Err(format!("Baseline {n} is not set; see list_baselines.")) }
            rows.sort_by(|x, y| y.0.total_cmp(&x.0));
            let lim = a["limit"].as_u64().unwrap_or(50).clamp(1, 500) as usize;
            Ok(json!({ "baseline": n, "tasks": rows.len(), "late": rows.iter().filter(|r| r.0 > 0.0).count(), "rows": rows.into_iter().take(lim).map(|r| r.1).collect::<Vec<_>>() }))
        }
        "clear_baseline" => {
            let n = a["n"].as_u64().filter(|n| *n <= 10).ok_or("n is from 0 to 10.")? as u8;
            apply(doc, Op::Baseline { uids: uid_list(a, "uids").unwrap_or_default(), clear: true, n })?;
            Ok(json!({ "cleared": n }))
        }
        "add_sheet" => {
            let n = s(a, "name").ok_or("name is needed.")?.trim().to_string();
            let cells = a["cells"].as_object().map(|m| m.iter().map(|(c, v)| (c.to_uppercase(), txt(v))).filter(|x| !x.1.is_empty()).collect()).unwrap_or_default();
            apply(doc, Op::Batch(vec![Op::Sheet { k: None, sheet: Some(SheetData { name: n.clone(), ..Default::default() }) }, Op::SheetCells { k: doc.p.sheets.len(), cells, fmt: None }]))?;
            Ok(json!({ "sheet": n }))
        }
        "rename_sheet" => {
            let k = sheet_k(&doc.p, a)?;
            let mut sh = doc.p.sheets[k].clone();
            sh.name = s(a, "to").ok_or("to is needed (the new name).")?.trim().into();
            apply(doc, Op::Sheet { k: Some(k), sheet: Some(sh) })?;
            Ok(json!({ "sheet": doc.p.sheets[k].name }))
        }
        "delete_sheet" => {
            let k = sheet_k(&doc.p, a)?;
            let n = doc.p.sheets[k].name.clone();
            apply(doc, Op::Sheet { k: Some(k), sheet: None })?;
            Ok(json!({ "deleted": n }))
        }
        "format_sheet_cells" => {
            let k = sheet_k(&doc.p, a)?;
            let mut ops = vec![];
            if let Some(r) = s(a, "range") {
                let sh = &doc.p.sheets[k];
                ops.push(Op::SheetStyle { k, cells: cells_of(r)?.into_iter().map(|c| { let v = restyle(sh.style.get(&c).map_or("", |x| x.as_str()), &a["style"]); v.map(|v| (c, v)) }).collect::<Result<_, _>>()? });
            }
            if let Some(w) = a["widths"].as_object() {
                let mut sh = doc.p.sheets[k].clone();
                for (c, v) in w {
                    let col = cells_of(&format!("{c}1"))?.first().cloned().ok_or("A column is a letter like B.")?;
                    let i = col.trim_end_matches('1').chars().fold(0u32, |n, ch| n * 26 + (ch as u32 - 64)) - 1;
                    sh.widths.insert(i, v.as_u64().filter(|x| (8..=2000).contains(x)).ok_or("A width is in px, from 8 to 2000.")? as u16);
                }
                ops.push(Op::Sheet { k: Some(k), sheet: Some(sh) });
            }
            if ops.is_empty() { return Err("Give a range with a style, or widths.".into()) }
            apply(doc, Op::Batch(ops))?;
            Ok(json!({ "sheet": doc.p.sheets[k].name }))
        }
        "set_number_format" => {
            let k = sheet_k(&doc.p, a)?;
            let cells = cells_of(s(a, "range").ok_or("range is needed, like B2:B20.")?)?.into_iter().map(|c| (c, String::new())).collect();
            apply(doc, Op::SheetCells { k, cells, fmt: Some(a.get("format").map(txt).unwrap_or_default().trim().into()) })?;
            Ok(json!({ "sheet": doc.p.sheets[k].name }))
        }
        "define_name" => {
            let n = s(a, "name").ok_or("name is needed.")?.trim().to_string();
            let mut names = doc.p.names.clone();
            let f = a.get("formula").map(txt).unwrap_or_default().trim().to_string();
            let at = names.iter().position(|x| x.name.eq_ignore_ascii_case(&n));
            match (at, f.is_empty()) {
                (Some(i), true) => { names.remove(i); }
                (None, true) => return Err(format!("No name {n} to remove.")),
                (Some(i), false) => { names[i].formula = f; if let Some(c) = s(a, "comment") { names[i].comment = c.into() } }
                (None, false) => names.push(DefName { name: n.clone(), formula: f, comment: s(a, "comment").unwrap_or("").into(), sheet: String::new() }),
            }
            apply(doc, Op::Names { names })?;
            Ok(json!({ "names": doc.p.names.len() }))
        }
        "format_tasks" => {
            let key = s(a, "column").unwrap_or("*").to_string();
            let cells = uid_list(a, "uids")?.into_iter().map(|uid| {
                let i = doc.p.idx(uid).ok_or_else(|| format!("No task with uid {uid}."))?;
                let old = doc.p.tasks[i].style.get(&key).and_then(Value::as_str).unwrap_or("");
                Ok((uid, key.clone(), restyle(old, &a["style"])?))
            }).collect::<Result<Vec<_>, String>>()?;
            apply(doc, Op::Style { cells })?;
            Ok(json!({ "tasks": uid_list(a, "uids")?.len() }))
        }
        "list_dashboards" => Ok(json!(doc.p.ui["dashboards"].as_array().into_iter().flatten().map(|d| json!({ "name": d["name"],
            "tiles": d["tiles"].as_array().into_iter().flatten().map(|t| json!({ "kind": t["kind"], "title": t["title"], "formula": t["formula"], "plan": t["plan"], "src": t["src"] })).collect::<Vec<_>>() })).collect::<Vec<_>>())),
        "add_dashboard" | "delete_dashboard" => {
            let n = s(a, "name").ok_or("name is needed.")?.trim().to_string();
            let mut list: Vec<Value> = doc.p.ui["dashboards"].as_array().cloned().unwrap_or_default();
            let had = list.len();
            list.retain(|d| !d["name"].as_str().is_some_and(|x| x.eq_ignore_ascii_case(&n)));
            if name == "delete_dashboard" {
                if list.len() == had { return Err(format!("No dashboard called {n}.")) }
            } else {
                const KINDS: [&str; 7] = ["number", "ring", "chart", "table", "countries", "text", "image"];
                let tiles = a["tiles"].as_array().filter(|v| !v.is_empty() && v.len() <= 60).ok_or("tiles is needed: 1 to 60 tiles.")?.iter().enumerate().map(|(k, t)| {
                    let kind = t["kind"].as_str().filter(|x| KINDS.contains(x)).ok_or("Each tile has a kind: number, ring, chart, table, countries, text or image.")?;
                    let w = t["size"].as_u64().filter(|w| [3, 4, 6, 8, 12].contains(w)).unwrap_or(if matches!(kind, "chart" | "table") { 6 } else { 3 });
                    let mut o = json!({ "id": format!("ai{}{k}", doc.p.next_uid), "kind": kind, "title": t["title"].as_str().unwrap_or(""), "w": w, "h": if matches!(kind, "chart" | "table") { 2 } else { 1 } });
                    for f in ["formula", "fmt", "sub", "plan", "src", "chart", "text"] { if let Some(x) = t[f].as_str() { o[f] = json!(x) } }
                    Ok(o)
                }).collect::<Result<Vec<_>, String>>()?;
                list.push(json!({ "name": n, "tiles": tiles }));
            }
            apply(doc, Op::Ui { key: "dashboards".into(), value: json!(list) })?;
            Ok(json!({ "dashboards": list.len() }))
        }
        "task_path" => {
            let i = doc.p.idx(u(a, "uid")?).ok_or("No task with that uid.")?;
            let sc = crate::sched::schedule(&doc.p)?;
            let path = crate::sched::task_path(&sc, i);
            let list = |k: u8| (0..path.len()).filter(|&j| path[j] == k).map(|j| json!({ "uid": doc.p.tasks[j].uid, "id": j + 1, "name": doc.p.tasks[j].name })).collect::<Vec<_>>();
            Ok(json!({ "task": doc.p.tasks[i].name, "driving_predecessors": list(2), "predecessors": list(3), "driven_successors": list(4), "successors": list(5) }))
        }
        "set_project_info" => {
            let mut ops = vec![];
            for k in ["name", "code", "description", "client", "manager", "location", "company"] {
                if let Some(v) = a.get(k).filter(|v| !v.is_null()) { ops.push(Op::ProjectSet { field: k.into(), value: json!(txt(v).trim()) }) }
            }
            if ops.is_empty() { return Err("Give name, code, description, client, manager, location or company.".into()) }
            if a.get("name").is_some_and(|v| txt(v).trim().is_empty()) { return Err("A project needs a name.".into()) }
            apply(doc, Op::Batch(ops))?;
            Ok(json!({ "name": doc.p.name, "client": doc.p.client, "manager": doc.p.manager }))
        }
        "add_recurring_task" => {
            let at = match a["after_uid"].as_u64() { Some(x) => doc.p.idx(x as u32).ok_or("No task with after_uid.")? + 1, None => doc.p.tasks.len() };
            let every = s(a, "every").filter(|e| ["day", "week", "month"].contains(e)).ok_or("every is day, week or month.")?.to_string();
            let start = parse::date_in(s(a, "start").ok_or("start is needed (YYYY-MM-DD).")?, fmt)?;
            let first = doc.p.next_uid;
            apply(doc, Op::Recurring { at, name: s(a, "name").ok_or("name is needed.")?.trim().into(), dur: s(a, "duration").unwrap_or("1d").into(), every,
                interval: a["interval"].as_u64().unwrap_or(1).clamp(1, 365) as u32, count: a["count"].as_u64().unwrap_or(0).min(10_000) as u32, start: start.to_string() })?;
            Ok(json!({ "summary_uid": first, "occurrences": doc.p.tasks.iter().filter(|t| t.uid > first).count() }))
        }
        "list_credit_rules" => {
            let f = |r: &CreditRule, kind: &str| json!({ "name": r.name, "kind": kind, "steps": r.steps.iter().map(|(n, w)| json!({ "step": n, "weight": w })).collect::<Vec<_>>() });
            Ok(json!(doc.p.credit.iter().map(|r| f(r, "plan")).chain(progress::credit_library().iter().map(|r| f(r, "built-in"))).collect::<Vec<_>>()))
        }
        "apply_credit_rule" => {
            let uids = uid_list(a, "uids")?;
            let r = s(a, "rule").ok_or("rule is needed (see list_credit_rules).")?;
            let name = doc.p.credit.iter().map(|x| x.name.clone()).chain(progress::credit_library().into_iter().map(|x| x.name)).find(|n| n.eq_ignore_ascii_case(r.trim())).ok_or_else(|| format!("No rule of credit called {r}."))?;
            apply(doc, Op::ApplyCredit { uids: uids.clone(), rule: name.clone() })?;
            Ok(json!({ "rule": name, "tasks": uids.len() }))
        }
        "set_task_steps" | "mark_step_done" => {
            let uid = u(a, "uid")?;
            let i = doc.p.idx(uid).ok_or("No task with that uid.")?;
            let steps: Vec<Step> = if name == "set_task_steps" {
                a["steps"].as_array().ok_or("steps is needed: [{\"name\":\"Fabricated\",\"weight\":60,\"done\":false}].")?.iter().map(|x| {
                    let n = s(x, "name").ok_or("Each step needs a name.")?.trim().to_string();
                    let w = x["weight"].as_f64().unwrap_or(0.0);
                    if !(0.0..=1e6).contains(&w) { return Err("A weight is from 0 up.".to_string()) }
                    Ok(Step { name: n, weight: w, done: x["done"].as_bool() == Some(true) })
                }).collect::<Result<_, String>>()?
            } else {
                let st = s(a, "step").ok_or("step is needed (its name).")?;
                let mut v = doc.p.tasks[i].steps.clone();
                v.iter_mut().find(|x| x.name.eq_ignore_ascii_case(st.trim())).ok_or_else(|| format!("This task has no step called {st}."))?.done = a["done"].as_bool() != Some(false);
                v
            };
            apply(doc, Op::TaskList { uid, field: "steps".into(), value: json!(steps) })?;
            let t = &doc.p.tasks[i];
            Ok(json!({ "steps": t.steps.iter().map(|x| json!({ "name": x.name, "weight": x.weight, "done": x.done })).collect::<Vec<_>>(), "physical_pct": t.phys }))
        }
        "apply_operation" => {
            let op = serde_json::from_value(a["operation"].clone()).map_err(|e| format!("Not an operation ({e}). See list_operations."))?;
            apply(doc, op)?;
            Ok(summary(&Calc::new(&doc.p, fmt, today)))
        }
        "undo" => { if !doc.can_undo() { return Err("Nothing to undo.".into()) } doc.undo(); Ok(json!({ "undone": true })) }
        "redo" => { if !doc.can_redo() { return Err("Nothing to redo.".into()) } doc.redo(); Ok(json!({ "redone": true })) }
        x => Err(format!("Unknown tool {x}.")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn d(y: i32, m: u32, dd: u32) -> NaiveDate { NaiveDate::from_ymd_opt(y, m, dd).unwrap() }
    fn plan() -> Doc { Doc::new(crate::templates::build(&crate::templates::TEMPLATES[0], d(2026, 11, 2))) }
    fn run(doc: &mut Doc, name: &str, a: Value) -> Result<Value, String> { call(doc, "%d-%b-%y", d(2026, 11, 13), name, &a) }

    #[test]
    fn every_listed_tool_answers_or_says_what_it_needs() {
        let mut doc = plan();
        for t in tools().as_array().unwrap() {
            let n = t["name"].as_str().unwrap();
            if matches!(n, "list_plans" | "switch_plan" | "save_plan" | "export_plan" | "export_register") { continue } // the window's own
            let r = run(&mut doc, n, json!({}));
            assert!(r.is_ok() || r.as_ref().unwrap_err().len() > 5, "{n}: {r:?}");
        }
    }
    #[test]
    fn an_assistant_reads_and_changes_the_plan_with_undo() {
        let mut doc = plan();
        let s0 = run(&mut doc, "plan_summary", json!({})).unwrap();
        assert!(s0["tasks"].as_u64().unwrap() > 3);
        let t = run(&mut doc, "get_tasks", json!({ "filter": "all", "limit": 3 })).unwrap();
        assert_eq!(t["tasks"].as_array().unwrap().len(), 3);
        let first = t["tasks"][1]["uid"].as_u64().unwrap();
        // Add two linked tasks after the second row: each change is one Undo step.
        let undo0 = doc.undo_len();
        let r = run(&mut doc, "add_tasks", json!({ "after_uid": first, "tasks": [{ "name": "Pump test", "dur": "3d" }, { "name": "Ship", "dur": "2d" }] })).unwrap();
        let added: Vec<u64> = r["added"].as_array().unwrap().iter().map(|x| x["uid"].as_u64().unwrap()).collect();
        assert_eq!(added.len(), 2);
        assert_eq!(doc.undo_len(), undo0 + 1);
        run(&mut doc, "link_tasks", json!({ "pred_uid": added[0], "succ_uid": added[1], "type": "FS", "lag": "1d" })).unwrap();
        let ship = run(&mut doc, "get_task", json!({ "uid": added[1] })).unwrap();
        assert!(ship["preds"].as_str().unwrap().contains("FS+1"), "{ship}");
        run(&mut doc, "update_progress", json!({ "uid": added[0], "pct": 40, "actual_start": "2026-11-09" })).unwrap();
        assert_eq!(run(&mut doc, "get_task", json!({ "uid": added[0] })).unwrap()["pct"], "40%");
        assert!(run(&mut doc, "set_task_fields", json!({ "uid": added[0], "fields": { "dur": "not a duration" } })).is_err(), "a bad value changes nothing");
        // A locked baseline stays locked for the assistant too.
        run(&mut doc, "set_baseline", json!({})).unwrap();
        assert!(run(&mut doc, "set_task_fields", json!({ "uid": added[0], "fields": { "b0start": "2026-11-20" } })).unwrap_err().contains("locked"));
        run(&mut doc, "undo", json!({})).unwrap();
        run(&mut doc, "redo", json!({})).unwrap();
        run(&mut doc, "delete_tasks", json!({ "uids": added })).unwrap();
        assert!(run(&mut doc, "get_tasks", json!({ "search": "pump test" })).unwrap()["total"] == 0);
        let c = run(&mut doc, "check_schedule", json!({})).unwrap();
        assert!(c["score"].as_f64().is_some() && c["checks"].as_array().unwrap().len() > 5);
        assert!(run(&mut doc, "get_tasks", json!({ "fields": ["nosuchfield"] })).unwrap_err().contains("list_fields"));
        run(&mut doc, "set_status_date", json!({ "date": "2026-11-13" })).unwrap();
        assert_eq!(doc.p.status, Some(d(2026, 11, 13)));
    }
    #[test]
    fn task_path_project_details_recurring_tasks_and_steps() {
        let mut doc = plan();
        assert!(tools().as_array().unwrap().len() >= 108);
        let t: Vec<u64> = run(&mut doc, "get_tasks", json!({ "filter": "not_started" })).unwrap()["tasks"].as_array().unwrap().iter().map(|x| x["uid"].as_u64().unwrap()).collect();
        let linked = t.iter().copied().find(|&u| !run(&mut doc, "task_path", json!({ "uid": u })).unwrap()["driving_predecessors"].as_array().unwrap().is_empty()).expect("a task with a driving predecessor");
        let p = run(&mut doc, "task_path", json!({ "uid": linked })).unwrap();
        assert!(p["driving_predecessors"][0]["name"].is_string());
        run(&mut doc, "set_project_info", json!({ "client": "Acme Utilities", "manager": "A. Planner" })).unwrap();
        assert_eq!(doc.p.client, "Acme Utilities");
        assert!(run(&mut doc, "set_project_info", json!({ "name": " " })).is_err());
        let n = doc.p.tasks.len();
        let r = run(&mut doc, "add_recurring_task", json!({ "name": "Progress meeting", "duration": "2h", "every": "week", "count": 4, "start": "2026-11-16" })).unwrap();
        assert_eq!((doc.p.tasks.len(), r["occurrences"].as_u64()), (n + 5, Some(4)));
        assert!(run(&mut doc, "list_credit_rules", json!({})).unwrap().as_array().unwrap().iter().any(|r| r["name"] == "Document"));
        run(&mut doc, "apply_credit_rule", json!({ "uids": [t[0]], "rule": "document" })).unwrap();
        let s = run(&mut doc, "mark_step_done", json!({ "uid": t[0], "step": "drafted" })).unwrap();
        assert_eq!(s["physical_pct"], 50);
        let s = run(&mut doc, "set_task_steps", json!({ "uid": t[1], "steps": [{ "name": "Cut", "weight": 1, "done": true }, { "name": "Weld", "weight": 3 }] })).unwrap();
        assert_eq!(s["physical_pct"], 25);
        assert!(run(&mut doc, "mark_step_done", json!({ "uid": t[1], "step": "Paint" })).is_err());
    }
    #[test]
    fn registers_baselines_sheets_formats_and_dashboards() {
        let mut doc = plan();
        let names: Vec<String> = tools().as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap().to_string()).collect();
        assert!(names.len() >= 101 && names.iter().collect::<std::collections::HashSet<_>>().len() == names.len(), "{} tools", names.len());
        let ann = |n: &str| tools().as_array().unwrap().iter().find(|t| t["name"] == n).unwrap()["annotations"]["readOnlyHint"].as_bool();
        assert_eq!((ann("export_plan"), ann("list_baselines")), (Some(false), Some(true)), "an export writes a file");
        let t: Vec<u64> = run(&mut doc, "get_tasks", json!({ "filter": "not_started" })).unwrap()["tasks"].as_array().unwrap().iter().map(|x| x["uid"].as_u64().unwrap()).collect();
        let (a, b) = (t[0], t[1]);
        // Contract, documents, procurement.
        run(&mut doc, "set_contract", json!({ "number": "C-100", "value": 2_000_000 })).unwrap();
        let cd = run(&mut doc, "set_contract_date", json!({ "fields": { "name": "Mechanical completion", "task": a, "date": "2027-01-15", "ld_rate": 0.5, "ld_cap": 10 } })).unwrap()["id"].as_u64().unwrap();
        run(&mut doc, "set_payment", json!({ "fields": { "name": "Advance", "pct": 10, "task": a, "terms": 30 } })).unwrap();
        assert!(run(&mut doc, "set_contract_date", json!({ "fields": { "nosuch": 1 } })).is_err());
        assert!(run(&mut doc, "set_contract_date", json!({ "fields": { "task": 999999 } })).unwrap_err().contains("999999"));
        let cs = run(&mut doc, "contract_status", json!({})).unwrap();
        assert_eq!(cs["number"], "C-100");
        run(&mut doc, "set_contract_date", json!({ "id": cd, "remove": true })).unwrap();
        assert!(doc.p.contract.dates.is_empty() && doc.p.contract.payments.len() == 1);
        let d = run(&mut doc, "set_vendor_document", json!({ "fields": { "number": "V-001", "title": "GA drawing", "class": 1, "task": b } })).unwrap()["id"].as_u64().unwrap();
        run(&mut doc, "log_submission", json!({ "fields": { "doc": d, "rev": "A", "purpose": "ICR", "sent": "2026-11-10", "returned": "2026-11-20", "code": "Code 2" } })).unwrap();
        assert!(run(&mut doc, "log_submission", json!({ "fields": { "doc": 999 } })).is_err());
        let dr = run(&mut doc, "document_register", json!({})).unwrap();
        assert_eq!(dr["documents"][0]["number"], "V-001");
        assert!(dr["status"].get("weekly").is_none());
        let o = run(&mut doc, "set_order", json!({ "fields": { "tag": "P-101", "name": "Feed pump", "vendor": "Acme", "value": 120000, "task": a, "need": b, "planned": ["2026-11-01", "2026-11-15", null, null, "2027-02-01"] } })).unwrap()["id"].as_u64().unwrap();
        run(&mut doc, "log_expediting", json!({ "id": o, "note": "Casting poured" })).unwrap();
        assert_eq!(doc.p.procurement.items[0].log[0].note, "Casting poured");
        assert!(run(&mut doc, "procurement_status", json!({})).unwrap()["orders"].as_array().unwrap().len() == 1);
        // Baselines.
        assert!(run(&mut doc, "baseline_variance", json!({})).unwrap_err().contains("not set"));
        run(&mut doc, "set_baseline", json!({ "n": 1 })).unwrap();
        assert_eq!(run(&mut doc, "list_baselines", json!({})).unwrap()[0]["n"], 1);
        run(&mut doc, "set_task_fields", json!({ "uid": a, "fields": { "dur": "20d" } })).unwrap();
        let v = run(&mut doc, "baseline_variance", json!({ "n": 1 })).unwrap();
        assert!(v["late"].as_u64().unwrap() >= 1 && v["rows"][0]["finish_var_days"].as_f64().unwrap() > 0.0, "{v}");
        run(&mut doc, "clear_baseline", json!({ "n": 1 })).unwrap();
        assert!(run(&mut doc, "list_baselines", json!({})).unwrap().as_array().unwrap().is_empty());
        // Sheets, names and formats.
        run(&mut doc, "add_sheet", json!({ "name": "Costs", "cells": { "A1": "Item", "B1": "Cost", "B2": "100", "B3": "=B2*2" } })).unwrap();
        assert!(run(&mut doc, "add_sheet", json!({ "name": "costs" })).is_err(), "names are unique");
        run(&mut doc, "format_sheet_cells", json!({ "name": "Costs", "range": "A1:B1", "style": { "bold": true, "fill": "#FFF2CC", "align": "center" }, "widths": { "A": 180 } })).unwrap();
        let k = doc.p.sheets.iter().position(|x| x.name == "Costs").unwrap();
        assert_eq!(doc.p.sheets[k].style["B1"], "a:c;b;f:#FFF2CC");
        assert_eq!(doc.p.sheets[k].widths[&0], 180);
        run(&mut doc, "format_sheet_cells", json!({ "name": "Costs", "range": "B1", "style": { "bold": false } })).unwrap();
        assert_eq!(doc.p.sheets[k].style["B1"], "a:c;f:#FFF2CC");
        assert!(run(&mut doc, "format_sheet_cells", json!({ "name": "Costs", "range": "A1", "style": { "fill": "yellow" } })).is_err());
        run(&mut doc, "set_number_format", json!({ "name": "Costs", "range": "B2:B3", "format": "#,##0.00" })).unwrap();
        assert_eq!(doc.p.sheets[k].fmt["B3"], "#,##0.00");
        run(&mut doc, "define_name", json!({ "name": "Rate", "formula": "=Costs!$B$2" })).unwrap();
        run(&mut doc, "rename_sheet", json!({ "name": "Costs", "to": "Cost Plan" })).unwrap();
        assert!(doc.p.names[0].formula.contains("Cost Plan"), "{}", doc.p.names[0].formula);
        run(&mut doc, "define_name", json!({ "name": "Rate", "formula": "" })).unwrap();
        run(&mut doc, "delete_sheet", json!({ "name": "Cost Plan" })).unwrap();
        assert!(!doc.p.sheets.iter().any(|x| x.name == "Cost Plan"));
        assert_eq!(cells_of("B2:C3").unwrap(), ["B2", "C2", "B3", "C3"]);
        // Task formatting and dashboards.
        run(&mut doc, "format_tasks", json!({ "uids": [a], "style": { "bold": true, "color": "#C0392B" } })).unwrap();
        let ia = doc.p.idx(a as u32).unwrap();
        assert_eq!(doc.p.tasks[ia].style["*"], "b;c:#C0392B");
        run(&mut doc, "add_dashboard", json!({ "name": "Client", "tiles": [{ "kind": "number", "title": "SPI", "formula": "=SPI" }, { "kind": "chart", "title": "S-curve", "plan": "scurve", "size": 12 }] })).unwrap();
        assert!(run(&mut doc, "add_dashboard", json!({ "name": "Bad", "tiles": [{ "kind": "pie" }] })).is_err());
        assert_eq!(run(&mut doc, "list_dashboards", json!({})).unwrap()[0]["tiles"][1]["plan"], "scurve");
        run(&mut doc, "delete_dashboard", json!({ "name": "client" })).unwrap();
        assert!(run(&mut doc, "list_dashboards", json!({})).unwrap().as_array().unwrap().is_empty());
    }
    #[test]
    fn risks_delays_snapshots_health_levelling_and_updates() {
        let mut doc = plan();
        let names: Vec<String> = tools().as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap().to_string()).collect();
        assert!(names.len() >= 76, "{} tools", names.len());
        assert!(tools().as_array().unwrap().iter().all(|t| t["annotations"]["readOnlyHint"].is_boolean()));
        let t: Vec<u64> = run(&mut doc, "get_tasks", json!({ "filter": "not_started" })).unwrap()["tasks"].as_array().unwrap().iter().map(|x| x["uid"].as_u64().unwrap()).collect();
        let (a, b) = (t[0], t[1]);
        // Risk register.
        let r = run(&mut doc, "set_risk", json!({ "name": "Late vendor drawings", "prob": 4, "impact": 3, "task_uids": [a], "days": 10 })).unwrap()["risk"].as_u64().unwrap();
        assert!(run(&mut doc, "set_risk", json!({ "id": r, "prob": 9 })).unwrap_err().contains("1 to 5"));
        run(&mut doc, "set_risk", json!({ "id": r, "status": "Closed" })).unwrap();
        let l = run(&mut doc, "list_risks", json!({})).unwrap();
        assert_eq!((l[0]["score"].as_u64(), l[0]["status"].as_str(), l[0]["tasks"][0]["uid"].as_u64()), (Some(12), Some("Closed"), Some(a)));
        run(&mut doc, "delete_risk", json!({ "id": r })).unwrap();
        assert!(doc.p.risks.is_empty());
        // Delay events and their fragnet.
        let e = run(&mut doc, "set_delay_event", json!({ "name": "Client change", "date": "2026-11-20", "extend": [{ "uid": a, "days": 3 }], "frag": [{ "name": "Redesign", "days": 5, "after": [a], "before": [b] }] })).unwrap()["event"].as_u64().unwrap();
        assert_eq!(run(&mut doc, "list_delay_events", json!({})).unwrap()[0]["extend"][0]["days"], 3.0);
        let n = doc.p.tasks.len();
        run(&mut doc, "add_fragnet", json!({ "id": e })).unwrap();
        assert_eq!(doc.p.tasks.len(), n + 1, "the new activity is in the plan");
        assert!(run(&mut doc, "delay_analysis", json!({ "method": "available" })).unwrap()["methods"].as_array().unwrap().len() >= 2);
        assert!(run(&mut doc, "delay_analysis", json!({ "method": "nope" })).is_err());
        run(&mut doc, "delete_delay_event", json!({ "id": e })).unwrap();
        // Status, snapshots, reports and earned value.
        assert!(run(&mut doc, "take_snapshot", json!({})).unwrap_err().contains("status date"));
        run(&mut doc, "set_baseline", json!({})).unwrap();
        run(&mut doc, "set_status_date", json!({ "date": "2026-11-20" })).unwrap();
        run(&mut doc, "mark_on_track", json!({ "uids": [a] })).unwrap();
        run(&mut doc, "take_snapshot", json!({ "label": "Week 3" })).unwrap();
        assert_eq!(run(&mut doc, "list_snapshots", json!({})).unwrap()[0]["label"], "Week 3");
        let pr = run(&mut doc, "period_report", json!({ "period": "week" })).unwrap();
        assert!(pr["from"].as_str().unwrap().starts_with("2026-"), "{}", pr["from"]);
        assert!(run(&mut doc, "milestone_trend", json!({})).unwrap().is_array());
        let ev = run(&mut doc, "earned_value", json!({})).unwrap();
        assert!(ev["bac"].as_f64().is_some() && ev["status_date"] == "2026-11-20", "{ev}");
        // Health settings.
        run(&mut doc, "set_health_settings", json!({ "limits": { "high_float": 30 }, "weights": { "resources": 1 } })).unwrap();
        let h = run(&mut doc, "get_health_settings", json!({})).unwrap();
        assert_eq!((h["limits"]["high_float"].as_f64(), h["weights"]["resources"].as_f64()), (Some(30.0), Some(1.0)));
        assert!(run(&mut doc, "set_health_settings", json!({ "weights": { "nosuch": 1 } })).is_err());
        // Levelling, Update Project, split, interim, the constraint log and usage.
        assert!(run(&mut doc, "level_preview", json!({})).unwrap()["finish"].is_string());
        run(&mut doc, "level_resources", json!({})).unwrap();
        run(&mut doc, "clear_levelling", json!({})).unwrap();
        run(&mut doc, "update_project", json!({ "through": "2026-11-20" })).unwrap();
        let long = { let c = Calc::new(&doc.p, "%d-%b-%y", d(2026, 11, 13)); (0..doc.p.tasks.len()).find(|&i| !c.s.summary[i] && doc.p.tasks[i].pct == 0 && doc.p.tasks[i].dur >= 3 * doc.p.day_min).unwrap() };
        let lu = doc.p.tasks[long].uid;
        run(&mut doc, "split_task", json!({ "uid": lu, "after": "1d", "gap": "2d" })).unwrap();
        assert_eq!(doc.p.tasks[long].splits.len(), 1);
        run(&mut doc, "save_interim", json!({ "n": 1 })).unwrap();
        assert!(doc.p.tasks[0].custom.contains_key("start1"));
        let li = run(&mut doc, "set_constraint_log_item", json!({ "uid": b, "kind": "Material", "text": "Pipe spools on site", "need": "2026-12-01" })).unwrap()["entry"].as_u64().unwrap();
        assert_eq!(run(&mut doc, "list_constraint_log", json!({})).unwrap()[0]["status"], "Open");
        run(&mut doc, "set_constraint_log_item", json!({ "id": li, "remove": true })).unwrap();
        assert!(doc.p.log.is_empty());
        assert!(run(&mut doc, "resource_usage", json!({ "by": "resource", "unit": "week" })).unwrap()["periods"][0].is_string());
        assert!(run(&mut doc, "resource_usage", json!({ "by": "trade" })).unwrap()["trades"].is_array());
        assert!(prompt("status_report", &json!({})).unwrap().contains("month") && prompt("nope", &json!({})).is_err());
    }
    #[test]
    fn resources_calendars_codes_fields_filters_and_outline() {
        let mut doc = plan();
        let names: Vec<String> = tools().as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap().to_string()).collect();
        assert!(names.len() >= 50 && names.iter().collect::<std::collections::HashSet<_>>().len() == names.len(), "{} tools, names unique", names.len());
        let t: Vec<u64> = run(&mut doc, "get_tasks", json!({ "filter": "not_started" })).unwrap()["tasks"].as_array().unwrap().iter().map(|x| x["uid"].as_u64().unwrap()).collect();
        let (a, b) = (t[0], t[1]);
        // Resources and assignments: one Undo step each.
        let r = run(&mut doc, "add_resources", json!({ "resources": [{ "name": "Rigger", "max": "300%", "rate": 55 }, { "name": "Grout", "kind": "material", "label": "m3" }] })).unwrap();
        let rig = r["added"][0]["uid"].as_u64().unwrap();
        let k = doc.p.res_idx(rig as u32).unwrap();
        assert_eq!((doc.p.resources[k].max, doc.p.resources[k].rate), (3.0, 55.0));
        assert!(run(&mut doc, "add_resources", json!({ "resources": [{ "name": "rigger" }] })).is_err(), "names are unique");
        run(&mut doc, "assign_resource", json!({ "uids": [a, b], "resource": "Rigger", "units": "200%" })).unwrap();
        run(&mut doc, "assign_resource", json!({ "uids": [a], "resource": "Grout", "units": "4" })).unwrap();
        let on_a = run(&mut doc, "get_assignments", json!({ "uid": a })).unwrap();
        assert!(on_a.as_array().unwrap().iter().any(|x| x["resource"] == "Rigger" && x["units"] == "200%"), "{on_a}");
        assert_eq!(run(&mut doc, "get_assignments", json!({ "resource": "rigger" })).unwrap().as_array().unwrap().len(), 2);
        run(&mut doc, "unassign_resource", json!({ "uids": [b], "resource": "Rigger" })).unwrap();
        assert_eq!(run(&mut doc, "get_assignments", json!({ "resource": "Rigger" })).unwrap().as_array().unwrap().len(), 1);
        assert!(run(&mut doc, "list_resources", json!({})).unwrap().as_array().unwrap().iter().any(|x| x["name"] == "Grout"));
        // Calendars and exceptions.
        run(&mut doc, "add_calendar", json!({ "name": "Six days", "week": { "sat": "07:00-12:00" } })).unwrap();
        run(&mut doc, "set_calendar", json!({ "name": "six days", "add_exceptions": [{ "name": "Shutdown", "from": "2026-12-24", "to": "2026-12-26" }] })).unwrap();
        let cal = run(&mut doc, "get_calendar", json!({ "name": "Six days" })).unwrap();
        assert_eq!((cal["week"]["sat"].as_str().unwrap(), cal["exceptions"][0]["to"].as_str().unwrap()), ("07:00-12:00", "2026-12-26"));
        run(&mut doc, "set_resource_fields", json!({ "resource_uid": rig, "fields": { "calendar": "Six days" } })).unwrap();
        assert!(run(&mut doc, "delete_calendar", json!({ "name": "Six days" })).unwrap_err().contains("use"));
        assert!(run(&mut doc, "set_calendar", json!({ "name": "Six days", "week": { "mon": "8-5" } })).is_err(), "a bad time changes nothing");
        // Activity codes, custom fields, constraints, deadlines and notes.
        run(&mut doc, "define_code", json!({ "name": "Area", "values": [{ "value": "U-100", "desc": "Unit 100" }, "U-200"] })).unwrap();
        run(&mut doc, "set_task_codes", json!({ "uids": [a, b], "code": "area", "value": "U-100" })).unwrap();
        assert!(run(&mut doc, "set_task_codes", json!({ "uids": [a], "code": "Area", "value": "U-999" })).is_err());
        run(&mut doc, "define_code", json!({ "name": "Zone", "rename_from": "Area" })).unwrap();
        assert_eq!(run(&mut doc, "list_codes", json!({})).unwrap()[0]["values"][0]["tasks"], 2);
        assert!(run(&mut doc, "define_custom_field", json!({ "key": "text31" })).is_err());
        run(&mut doc, "define_custom_field", json!({ "key": "text1", "alias": "Vendor", "lookup": ["Acme", "Beta"], "restrict": true })).unwrap();
        run(&mut doc, "set_task_fields", json!({ "uid": a, "fields": { "c:text1": "Acme" } })).unwrap();
        assert_eq!(run(&mut doc, "list_custom_fields", json!({})).unwrap()[0]["tasks_with_value"], 1);
        run(&mut doc, "set_constraint", json!({ "uids": [b], "type": "SNET", "date": "2027-01-04" })).unwrap();
        run(&mut doc, "set_deadline", json!({ "uids": [b], "date": "2027-02-01" })).unwrap();
        let ib = doc.p.idx(b as u32).unwrap();
        assert_eq!((doc.p.tasks[ib].constraint, doc.p.tasks[ib].cdate, doc.p.tasks[ib].deadline), (Constraint::SNET, Some(d(2027, 1, 4)), Some(d(2027, 2, 1))));
        assert!(run(&mut doc, "set_constraint", json!({ "uids": [b], "type": "MFO" })).is_err(), "a dated constraint needs its date");
        run(&mut doc, "set_task_notes", json!({ "uid": a, "notes": "Vendor confirmed." })).unwrap();
        // Columns of the task sheet: by name or key, after a column, hidden again.
        let shown = run(&mut doc, "set_columns", json!({ "show": ["Baseline Start", "b0finish"], "after": "finish" })).unwrap();
        let v: Vec<&str> = shown.as_array().unwrap().iter().map(|x| x.as_str().unwrap()).collect();
        let f = v.iter().position(|x| *x == "finish").unwrap();
        assert_eq!(&v[f..f + 3], ["finish", "b0start", "b0finish"], "{v:?}");
        run(&mut doc, "set_columns", json!({ "hide": ["b0finish"] })).unwrap();
        assert!(run(&mut doc, "get_columns", json!({})).unwrap().as_array().unwrap().iter().any(|x| x["key"] == "b0finish" && x["hidden"] == true));
        assert!(run(&mut doc, "set_columns", json!({ "show": ["nosuch"] })).is_err() && run(&mut doc, "set_columns", json!({ "hide": ["name"] })).is_err());
        assert_eq!(run(&mut doc, "set_task_notes", json!({ "uid": a, "notes": "Crane booked.", "append": true })).unwrap()["notes"], "Vendor confirmed.\nCrane booked.");
        // Filters: by name, and by rules on any field (codes and custom fields too).
        assert!(run(&mut doc, "list_filters", json!({})).unwrap().as_array().unwrap().iter().any(|f| f["name"] == "Critical"));
        assert!(run(&mut doc, "filter_tasks", json!({ "name": "Critical" })).unwrap()["total"].as_u64().unwrap() > 0);
        let f = run(&mut doc, "filter_tasks", json!({ "rules": [{ "field": "code:Zone", "test": "equals", "value": "U-100" }], "fields": ["c:text1"] })).unwrap();
        assert_eq!(f["total"], 2, "{f}");
        assert!(run(&mut doc, "filter_tasks", json!({ "rules": [{ "field": "nosuch", "test": "equals", "value": "1" }] })).is_err());
        // Outline: indent, outdent and move, each one Undo step.
        let i = (1..doc.p.tasks.len()).find(|&i| doc.p.tasks[i - 1].level == doc.p.tasks[i].level && doc.p.tasks.get(i + 1).is_none_or(|n| n.level <= doc.p.tasks[i].level)).unwrap();
        let (x, lvl) = (doc.p.tasks[i].uid, doc.p.tasks[i].level);
        run(&mut doc, "indent_tasks", json!({ "uids": [x] })).unwrap();
        assert_eq!(doc.p.tasks[i].level, lvl + 1);
        run(&mut doc, "outdent_tasks", json!({ "uids": [x] })).unwrap();
        assert_eq!(doc.p.tasks[i].level, lvl);
        let n = doc.undo_len();
        run(&mut doc, "move_tasks", json!({ "uids": [b], "direction": "up", "rows": 1 })).unwrap();
        assert_eq!(doc.undo_len(), n + 1);
        run(&mut doc, "undo", json!({})).unwrap();
        assert_eq!(doc.p.idx(b as u32).unwrap(), ib);
        run(&mut doc, "delete_resources", json!({ "resource_uids": [rig] })).unwrap();
        assert!(doc.p.res_idx(rig as u32).is_none());
        assert!(run(&mut doc, "define_code", json!({ "name": "Zone", "remove": true })).is_ok() && doc.p.codes.is_empty());
        run(&mut doc, "delete_custom_field", json!({ "key": "text1" })).unwrap();
        assert!(doc.p.tasks.iter().all(|t| !t.custom.contains_key("text1")));
    }
}
