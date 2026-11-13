//! The tools an AI assistant uses on an open plan (the window's AI Link, Model Context Protocol):
//! read the schedule, its checks, curves and sheets, and change it only through the engine's named
//! operations, so every change is one Undo step and is checked like the user's own. The window adds
//! the tools about its tabs (list, switch, save); this module works on one plan.
use crate::{check, doc::{Doc, Op}, fields::{task_fields, Calc}, ops, progress, report, risk, sheet};
use chrono::NaiveDate;
use serde_json::{json, Value};

/// The tool list (name, description, input schema), the window's own tools included.
pub fn tools() -> Value {
    let obj = |props: Value, req: &[&str]| json!({ "type": "object", "properties": props, "required": req });
    let none = || obj(json!({}), &[]);
    let uid = || json!({ "type": "integer", "description": "The task's unique id (uid), from get_tasks" });
    json!([
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
        { "name": "list_operations", "description": "Every named operation the engine has (with an example), for apply_operation.", "inputSchema": none() },
        { "name": "apply_operation", "description": "Any other change, as one named operation (see list_operations). A failed operation changes nothing.", "inputSchema": obj(json!({ "operation": { "type": "object" } }), &["operation"]) },
        { "name": "undo", "description": "Undo the last change in the plan shown (the user's or the assistant's).", "inputSchema": none() },
        { "name": "redo", "description": "Redo the change undone last.", "inputSchema": none() },
        { "name": "save_plan", "description": "Save the plan shown to its file. Only when the user asks for it.", "inputSchema": none() },
    ])
}

/// What the assistant is told when it connects.
pub const INSTRUCTIONS: &str = "You are connected to the Datum Plan window the user has open: a project schedule (CPM) with tasks, links, resources, baselines, progress and sheets. \
Read before you change: plan_summary, get_tasks (uids identify tasks), get_task. Change only with the tools; each change is one Undo step the user sees at once. \
Dates are YYYY-MM-DD. Durations like 5d, 3w, 4h. Ask before deleting tasks, setting a baseline or saving; never save unless asked.";

/// Does a tool change the plan?
pub fn changes(name: &str) -> bool {
    matches!(name, "add_tasks" | "set_task_fields" | "link_tasks" | "unlink_tasks" | "delete_tasks" | "update_progress" | "set_status_date" | "set_baseline" | "set_sheet_cells" | "apply_operation" | "undo" | "redo")
}

fn s<'a>(a: &'a Value, k: &str) -> Option<&'a str> { a[k].as_str().filter(|x| !x.trim().is_empty()) }
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

/// Runs one tool on `doc`. `fmt` is the user's date format (typed dates are read in it or as YYYY-MM-DD).
pub fn call(doc: &mut Doc, fmt: &str, today: NaiveDate, name: &str, a: &Value) -> Result<Value, String> {
    let apply = |doc: &mut Doc, op: Op| -> Result<(), String> { doc.apply(op, fmt) };
    match name {
        "plan_summary" => Ok(summary(&Calc::new(&doc.p, fmt, today))),
        "list_fields" => Ok(json!(task_fields().iter().map(|f| json!({ "key": f.key, "name": f.name, "group": f.group, "editable": f.edit })).collect::<Vec<_>>())),
        "list_operations" => Ok(json!(ops::catalog())),
        "get_tasks" => {
            let c = Calc::new(&doc.p, fmt, today);
            if let Some(e) = &c.error { return Err(e.clone()) }
            let keys: Vec<String> = a["fields"].as_array().map(|v| v.iter().filter_map(|x| x.as_str().map(String::from)).collect()).filter(|v: &Vec<String>| !v.is_empty()).unwrap_or_else(|| DEFAULT_FIELDS.iter().map(|k| k.to_string()).collect());
            if let Some(bad) = keys.iter().find(|k| c.key_of(k).is_none() && !task_fields().iter().any(|f| &f.key == *k)) { return Err(format!("No field \"{bad}\"; see list_fields.")) }
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
            let all: Vec<usize> = (0..p.tasks.len()).filter(|&i| keep(i)).collect();
            let off = a["offset"].as_u64().unwrap_or(0) as usize;
            let lim = a["limit"].as_u64().unwrap_or(200).clamp(1, 500) as usize;
            let rows: Vec<Value> = all.iter().skip(off).take(lim).map(|&i| task_row(&c, i, &keys)).collect();
            Ok(json!({ "total": all.len(), "offset": off, "tasks": rows, "more": all.len() > off + rows.len() }))
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
                r.extend(o.iter().filter(|(k, _)| k.as_str() != "name").map(|(k, v)| (k.clone(), v.as_str().map(String::from).unwrap_or_else(|| v.to_string()))));
                r
            }).collect();
            let first = doc.p.next_uid;
            apply(doc, Op::InsertRows { at, rows })?;
            Ok(json!({ "added": doc.p.tasks.iter().filter(|t| t.uid >= first).map(|t| json!({ "uid": t.uid, "name": t.name })).collect::<Vec<_>>() }))
        }
        "set_task_fields" => {
            let uid = u(a, "uid")?;
            let f = a["fields"].as_object().filter(|m| !m.is_empty()).ok_or("fields is needed, like {\"dur\":\"5d\"}.")?;
            let ops: Vec<Op> = f.iter().map(|(k, v)| Op::Set { uid, field: k.clone(), value: v.as_str().map(String::from).unwrap_or_else(|| v.to_string()) }).collect();
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
            let cells: Vec<(String, String)> = a["cells"].as_object().map(|m| m.iter().map(|(c, v)| (c.to_uppercase(), v.as_str().map(String::from).unwrap_or_else(|| v.to_string()))).collect()).unwrap_or_default();
            if cells.is_empty() { return Err("cells is needed, like {\"A1\":\"Vendor\"}.".into()) }
            apply(doc, Op::SheetCells { k, cells, fmt: None })?;
            Ok(json!({ "sheet": doc.p.sheets[k].name }))
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
}
