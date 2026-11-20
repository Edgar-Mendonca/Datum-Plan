//! The AI tools on plan files, without the window (Model Context Protocol, JSON-RPC on stdio):
//! `datum-cli mcp [FILE]` and `Datum Plan.exe --mcp-file [FILE]`, for assistants such as Claude Code.
//! Every tool of `assist` that works on one plan is served; the window's own tools (views, tabs) and
//! the many-plans tools are left out, and opening, starting and saving work on files instead.
use crate::{assist, doc::Doc, io, model::Project, templates};
use chrono::NaiveDate;
use serde_json::{json, Value};
use std::{io::{BufRead, Write}, path::Path};

const FMT: &str = "%Y-%m-%d";

pub const INSTRUCTIONS: &str = "Datum Plan schedules projects (CPM): tasks, links, resources, baselines, progress, sheets, registers. \
You work on plan files, with no window: open_plan (.dplan, Project XML or XER) or new_plan, then read with plan_summary, get_tasks (uids identify tasks) and get_task. \
Change only with the tools; each change is one undo step. Dates are YYYY-MM-DD, durations like 5d, 3w, 4h. \
Nothing is written until save_plan (to the plan's .dplan file) or save_plan_as; ask before saving over a file. export_plan writes other formats.";

/// The file tools, in place of the window's.
fn file_tools() -> Value {
    let obj = |props: Value, req: &[&str]| json!({ "type": "object", "properties": props, "required": req });
    let s = || json!({ "type": "string" });
    json!([
        { "name": "open_plan", "description": "Open a plan file to work on: .dplan, Project XML or XER. Returns its summary.", "inputSchema": obj(json!({ "path": s() }), &["path"]) },
        { "name": "new_plan", "description": "Start a new plan: blank, or from a template id (list_templates).", "inputSchema": obj(json!({ "name": s(), "start": { "type": "string", "description": "YYYY-MM-DD" }, "template": s() }), &["name", "start"]) },
        { "name": "list_templates", "description": "The templates a new plan can start from (id, name, what it holds).", "inputSchema": obj(json!({}), &[]) },
        { "name": "save_plan", "description": "Save the plan to its .dplan file (a plan opened from XML or XER, or new, needs save_plan_as). Only when the user asks.", "inputSchema": obj(json!({}), &[]) },
        { "name": "save_plan_as", "description": "Save the plan to a .dplan file; an existing file is replaced only with overwrite true.", "inputSchema": obj(json!({ "path": s(), "overwrite": { "type": "boolean" } }), &["path"]) },
        { "name": "export_plan", "description": "Write a copy in another format, by the path's extension: .xlsx .csv .xml (Project XML) .xer .pmxml .mpx .json .html .ics.", "inputSchema": obj(json!({ "path": s(), "overwrite": { "type": "boolean" } }), &["path"]) },
    ])
}

/// Every tool served on files: the file tools, then each one-plan tool of `assist`.
pub fn tools() -> Value {
    let mut out = file_tools().as_array().cloned().unwrap_or_default();
    let mine: Vec<String> = out.iter().map(|t| t["name"].as_str().unwrap_or("").to_string()).collect();
    for t in assist::tools().as_array().into_iter().flatten() {
        let n = t["name"].as_str().unwrap_or("");
        if !assist::WINDOW_TOOLS.contains(&n) && !assist::MANY_PLANS.contains(&n) && !mine.iter().any(|m| m == n) { out.push(t.clone()) }
    }
    Value::Array(out)
}

fn load(path: &str) -> Result<Doc, String> {
    let p = Path::new(path);
    if p.extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case("dplan")) { return Doc::open(p) }
    match io::import(p)? {
        io::Imported::Plan(p, _) => Ok(Doc::new(*p)),
        io::Imported::Table(_) => Err("Spreadsheets need their columns mapped: import them in the app.".into()),
    }
}

fn free(path: &Path, a: &Value) -> Result<(), String> {
    if path.exists() && !a["overwrite"].as_bool().unwrap_or(false) { return Err(format!("{} exists; ask the user, then give overwrite true.", path.display())) }
    Ok(())
}

/// Runs one tool on the open plan (or opens, starts or saves one).
pub fn call(doc: &mut Option<Doc>, today: NaiveDate, name: &str, a: &Value) -> Result<Value, String> {
    let s = |k: &str| a[k].as_str().map(String::from).ok_or(format!("{k} is needed"));
    let opened = |d: &mut Doc| assist::call(d, FMT, today, "plan_summary", &json!({}));
    match name {
        "list_templates" => return Ok(json!(templates::TEMPLATES.iter().map(|t| json!({ "id": t.id, "name": t.name, "about": t.desc })).collect::<Vec<_>>())),
        "open_plan" => { let mut d = load(&s("path")?)?; let v = opened(&mut d)?; *doc = Some(d); return Ok(v) }
        "new_plan" => {
            let start = NaiveDate::parse_from_str(&s("start")?, FMT).map_err(|_| "The start date is like 2026-11-02.")?;
            let mut p = match a["template"].as_str().filter(|t| !t.is_empty()) {
                Some(id) => templates::build(templates::TEMPLATES.iter().find(|t| t.id == id).ok_or(format!("No template {id}; see list_templates."))?, start),
                None => Project::new(&s("name")?, start),
            };
            p.name = s("name")?;
            let mut d = Doc::new(p);
            let v = opened(&mut d)?;
            *doc = Some(d);
            return Ok(v);
        }
        _ => {}
    }
    let d = doc.as_mut().ok_or("Open or start a plan first (open_plan or new_plan).")?;
    // The names of datum-cli before 0.9.31 still work: plan_info, get_schedule, save_plan with a path.
    match name {
        "plan_info" => assist::call(d, FMT, today, "plan_summary", a),
        "get_schedule" => assist::call(d, FMT, today, "get_tasks", &json!({ "limit": 500 })),
        "save_plan" if a["path"].is_string() => {
            let out = s("path")?;
            let ext = Path::new(&out).extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
            let tool = if ext == "dplan" { "save_plan_as" } else { "export_plan" };
            call(doc, today, tool, &json!({ "path": out, "overwrite": true }))
        }
        "save_plan" => {
            let path = d.path.clone().ok_or("This plan has no .dplan file yet: use save_plan_as.")?;
            d.save(&path)?;
            Ok(json!({ "saved": path.display().to_string() }))
        }
        "save_plan_as" => {
            let out = s("path")?;
            let path = Path::new(&out);
            if !path.extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case("dplan")) { return Err("save_plan_as writes .dplan; export_plan writes other formats.".into()) }
            free(path, a)?;
            d.save(path)?;
            Ok(json!({ "saved": out }))
        }
        "export_plan" => {
            let out = s("path")?;
            let path = Path::new(&out);
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
            if !["xml", "xer", "csv", "xlsx", "json", "html", "ics", "mpx", "pmxml"].contains(&ext.as_str()) { return Err(format!("export_plan writes .xlsx .csv .xml .xer .pmxml .mpx .json .html or .ics, not \"{out}\".")) }
            free(path, a)?;
            io::export(&d.p, path, FMT, today, &[])?;
            Ok(json!({ "saved": out }))
        }
        n if assist::WINDOW_TOOLS.contains(&n) || assist::MANY_PLANS.contains(&n) => Err(format!("{n} needs the Datum Plan window (its AI Link), not plan files.")),
        n => assist::call(d, FMT, today, n, a),
    }
}

/// Serves the tools: one JSON-RPC message per line in, one per line out. `file` is opened first.
pub fn serve(input: impl BufRead, mut out: impl Write, file: Option<&str>, today: NaiveDate) {
    let mut doc: Option<Doc> = None;
    let mut first = file.map(|f| call(&mut doc, today, "open_plan", &json!({ "path": f })).err()).unwrap_or(None);
    for line in input.lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() { continue }
        let msg: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(e) => { let _ = writeln!(out, "{}", json!({"jsonrpc": "2.0", "id": null, "error": {"code": -32700, "message": format!("Parse error: {e}")}})); let _ = out.flush(); continue }
        };
        let Some(id) = msg.get("id").cloned() else { continue }; // a notification: no answer
        let p = &msg["params"];
        let result = match msg["method"].as_str().unwrap_or("") {
            "initialize" => {
                let note = match (file, first.take()) { (_, Some(e)) => format!(" The plan file given could not be opened: {e}"), (Some(f), None) => format!(" {f} is open."), _ => String::new() };
                Ok(json!({ "protocolVersion": p["protocolVersion"].as_str().unwrap_or("2025-06-18"), "capabilities": { "tools": {} },
                    "serverInfo": { "name": "datum-plan", "version": env!("CARGO_PKG_VERSION") }, "instructions": format!("{INSTRUCTIONS}{note}") }))
            }
            "ping" => Ok(json!({})),
            "tools/list" => Ok(json!({ "tools": tools() })),
            "tools/call" => {
                let name = p["name"].as_str().unwrap_or("");
                let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| call(&mut doc, today, name, &p["arguments"]))).unwrap_or_else(|_| Err("Internal error; the plan is unchanged.".into()));
                Ok(match r {
                    Ok(v) => json!({ "content": [{ "type": "text", "text": serde_json::to_string_pretty(&v).unwrap_or_default() }], "isError": false }),
                    Err(e) => json!({ "content": [{ "type": "text", "text": e }], "isError": true }),
                })
            }
            m => Err(json!({ "code": -32601, "message": format!("Method not found: {m}") })),
        };
        let reply = match result { Ok(r) => json!({"jsonrpc": "2.0", "id": id, "result": r}), Err(e) => json!({"jsonrpc": "2.0", "id": id, "error": e}) };
        let _ = writeln!(out, "{reply}");
        let _ = out.flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session(file: Option<&str>, lines: &[Value]) -> Vec<Value> {
        let input: String = lines.iter().map(|l| l.to_string() + "\n").collect::<String>() + "not json\n";
        let mut out = vec![];
        serve(input.as_bytes(), &mut out, file, NaiveDate::from_ymd_opt(2026, 11, 20).unwrap());
        String::from_utf8_lossy(&out).lines().map(|l| serde_json::from_str(l).unwrap_or(Value::Null)).collect()
    }
    fn tool(id: u64, name: &str, args: Value) -> Value { json!({"jsonrpc": "2.0", "id": id, "method": "tools/call", "params": {"name": name, "arguments": args}}) }
    fn text(v: &Value) -> String { v["result"]["content"][0]["text"].as_str().unwrap_or("").to_string() }
    fn err(v: &Value) -> bool { v["result"]["isError"].as_bool().unwrap_or(true) }

    #[test]
    fn the_full_tool_set_works_on_plan_files_without_the_window() {
        let names: Vec<String> = tools().as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap().to_string()).collect();
        for n in ["open_plan", "new_plan", "save_plan", "save_plan_as", "export_plan", "add_tasks", "link_tasks", "document_register", "procurement_status", "contract_status", "set_sheet_cells", "earned_value"] { assert!(names.contains(&n.to_string()), "{n}") }
        for n in ["show_view", "list_plans", "switch_plan", "portfolio", "compare_plans"] { assert!(!names.contains(&n.to_string()), "{n}") }
        let mut seen = std::collections::HashSet::new();
        assert!(names.iter().all(|n| seen.insert(n)), "a tool is listed twice");

        let dir = std::env::temp_dir().join(format!("datum-mcpfile-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let plan = dir.join("p.dplan").display().to_string();
        let _ = std::fs::remove_file(&plan);
        let r = session(None, &[
            json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}),
            tool(2, "get_tasks", json!({})),
            tool(3, "new_plan", json!({"name": "Pump house", "start": "2026-11-02"})),
            tool(4, "add_tasks", json!({"tasks": [{"name": "Dig", "dur": "5d"}, {"name": "Pour", "dur": "3d"}]})),
            tool(5, "save_plan", json!({})),
            tool(6, "save_plan_as", json!({"path": plan})),
            tool(7, "show_view", json!({"view": "gantt"})),
            tool(8, "save_plan_as", json!({"path": plan})),
        ]);
        assert!(r[0]["result"]["instructions"].as_str().unwrap().contains("plan files"));
        assert!(err(&r[1]) && text(&r[1]).contains("Open or start"));
        assert!(!err(&r[2]) && text(&r[2]).contains("Pump house"));
        assert!(!err(&r[3]), "{}", text(&r[3]));
        assert!(err(&r[4]) && text(&r[4]).contains("save_plan_as"));
        assert!(!err(&r[5]) && Path::new(&plan).exists());
        assert!(err(&r[6]) && text(&r[6]).contains("window"));
        assert!(err(&r[7]) && text(&r[7]).contains("overwrite"));
        assert_eq!(r.last().unwrap()["error"]["code"], -32700);

        // Opened at start, changed, saved back to the same file.
        let r = session(Some(&plan), &[
            json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}),
            tool(2, "get_tasks", json!({})),
            tool(3, "set_task_fields", json!({"uid": 1, "fields": {"dur": "8d"}})),
            tool(4, "save_plan", json!({})),
            tool(5, "export_plan", json!({"path": dir.join("p.xml").display().to_string(), "overwrite": true})),
        ]);
        assert!(r[0]["result"]["instructions"].as_str().unwrap().contains("is open"));
        assert!(text(&r[1]).contains("Dig"));
        assert!(!err(&r[2]), "{}", text(&r[2]));
        assert!(!err(&r[3]) && !err(&r[4]), "{} {}", text(&r[3]), text(&r[4]));
        let mut back = Doc::open(Path::new(&plan)).unwrap();
        let t = assist::call(&mut back, FMT, NaiveDate::from_ymd_opt(2026, 11, 20).unwrap(), "get_tasks", &json!({})).unwrap().to_string();
        assert!(t.contains("8d") || t.contains("8 days"), "{t}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
