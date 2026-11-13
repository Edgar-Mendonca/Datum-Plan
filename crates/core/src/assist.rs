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
        { "name": "set_task_notes", "description": "Write a task's notes, or add a line to them (append).", "inputSchema": obj(json!({ "uid": uid(), "notes": { "type": "string" }, "append": { "type": "boolean" } }), &["uid", "notes"]) },
        { "name": "list_operations", "description": "Every named operation the engine has (with an example), for apply_operation.", "inputSchema": none() },
        { "name": "apply_operation", "description": "Any other change, as one named operation (see list_operations). A failed operation changes nothing.", "inputSchema": obj(json!({ "operation": { "type": "object" } }), &["operation"]) },
        { "name": "undo", "description": "Undo the last change in the plan shown (the user's or the assistant's).", "inputSchema": none() },
        { "name": "redo", "description": "Redo the change undone last.", "inputSchema": none() },
        { "name": "save_plan", "description": "Save the plan shown to its file. Only when the user asks for it.", "inputSchema": none() },
    ]);
    match (base, more) { (Value::Array(mut v), Value::Array(m)) => { v.extend(m); Value::Array(v) } _ => Value::Null }
}

/// What the assistant is told when it connects.
pub const INSTRUCTIONS: &str = "You are connected to the Datum Plan window the user has open: a project schedule (CPM) with tasks, links, resources, baselines, progress and sheets. \
Read before you change: plan_summary, get_tasks (uids identify tasks), get_task. Change only with the tools; each change is one Undo step the user sees at once. \
Dates are YYYY-MM-DD. Durations like 5d, 3w, 4h. Ask before deleting tasks, setting a baseline or saving; never save unless asked.";

/// Does a tool change the plan?
pub fn changes(name: &str) -> bool {
    matches!(name, "add_tasks" | "set_task_fields" | "link_tasks" | "unlink_tasks" | "delete_tasks" | "update_progress" | "set_status_date" | "set_baseline" | "set_sheet_cells" | "apply_operation"
        | "add_resources" | "set_resource_fields" | "delete_resources" | "assign_resource" | "unassign_resource" | "add_calendar" | "set_calendar" | "delete_calendar"
        | "define_code" | "set_task_codes" | "define_custom_field" | "delete_custom_field" | "set_constraint" | "set_deadline" | "indent_tasks" | "outdent_tasks" | "move_tasks" | "set_task_notes" | "undo" | "redo")
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
        "set_task_notes" => {
            let uid = u(a, "uid")?;
            let i = doc.p.idx(uid).ok_or("No task with that uid.")?;
            let n = a.get("notes").map(txt).ok_or("notes is needed.")?;
            let old = &doc.p.tasks[i].notes;
            let v = if a["append"].as_bool() == Some(true) && !old.is_empty() { format!("{old}\n{n}") } else { n };
            apply(doc, Op::Set { uid, field: "notes".into(), value: v })?;
            Ok(json!({ "notes": doc.p.tasks[i].notes }))
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
            if matches!(n, "list_plans" | "switch_plan" | "save_plan") { continue } // the window's own
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
