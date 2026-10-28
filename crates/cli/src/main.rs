//! datum-cli: Datum Plan without the window. Every change is one of the engine's named
//! operations (`datum-cli ops`), the same ones the window sends. `datum-cli mcp` serves the same
//! tools to AI assistants over the Model Context Protocol (JSON-RPC on stdin/stdout).
use datum_core::{doc::Doc, fields::Calc, io, ops, progress, risk, templates, Project};
use serde_json::{json, Value};
use std::{io::{BufRead, Write}, path::Path};

const HELP: &str = "datum-cli: Datum Plan on the command line

  datum-cli info FILE                     plan facts: dates, tasks, cost, critical tasks
  datum-cli schedule FILE [--csv]         every task with its dates and float
  datum-cli ops                           the named operations, with an example of each
  datum-cli apply FILE --op JSON [--op JSON…] [--save OUT.dplan]
                                          change a plan, e.g. --op '{\"Set\":{\"uid\":2,\"field\":\"dur\",\"value\":\"4d\"}}'
  datum-cli export FILE OUT               save as .dplan .xml .xer .csv .xlsx .json .html .ics .mpx .pmxml
  datum-cli risk FILE [--runs N]          schedule risk: P50, P80, P90 finish
  datum-cli lookahead FILE [--weeks N]    the next weeks' work and open constraints
  datum-cli new NAME START [TEMPLATE]     start a plan (START like 2026-11-02; templates: see `datum-cli templates`)
  datum-cli templates                     the plan templates
  datum-cli mcp                           serve the tools above to an AI assistant (Model Context Protocol, stdio)

FILE is a .dplan, Project XML or XER file. Nothing is saved unless you say --save or export.";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = match run(&args) {
        Ok(text) => { println!("{text}"); 0 }
        Err(e) => { eprintln!("datum-cli: {e}"); 1 }
    };
    std::process::exit(code);
}

fn today() -> chrono::NaiveDate { chrono::Local::now().date_naive() }
const FMT: &str = "%Y-%m-%d";

fn load(path: &str) -> Result<Project, String> {
    match io::import(Path::new(path))? {
        io::Imported::Plan(p, _) => Ok(*p),
        io::Imported::Table(_) => Err("Spreadsheets need their columns mapped: import them in the app.".into()),
    }
}

fn flag<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    args.iter().position(|a| a == name).and_then(|k| args.get(k + 1)).map(String::as_str)
}

fn run(args: &[String]) -> Result<String, String> {
    let file = || args.get(1).map(String::as_str).ok_or_else(|| "Give the plan file.".to_string());
    match args.first().map(String::as_str) {
        None | Some("help" | "-h" | "--help") => Ok(HELP.into()),
        Some("--version" | "version") => Ok(format!("datum-cli {}", env!("CARGO_PKG_VERSION"))),
        Some("info") => Ok(info(&load(file()?)?)?.to_string()),
        Some("schedule") => table(&load(file()?)?, args.iter().any(|a| a == "--csv")),
        Some("ops") => Ok(ops::catalog().iter().map(|o| format!("{}\n  {}\n  {}\n", o.name, o.about, o.example)).collect::<Vec<_>>().join("\n")),
        Some("templates") => Ok(templates::TEMPLATES.iter().map(|t| format!("{:<14} {}", t.id, t.name)).collect::<Vec<_>>().join("\n")),
        Some("apply") => {
            let mut d = Doc::new(load(file()?)?);
            let list: Vec<&String> = args.windows(2).filter(|w| w[0] == "--op").map(|w| &w[1]).collect();
            if list.is_empty() { return Err("Give at least one --op JSON (see `datum-cli ops`).".into()) }
            for (k, j) in list.iter().enumerate() {
                let op = serde_json::from_str(j).map_err(|e| format!("--op {}: not an operation ({e})", k + 1))?;
                d.apply(op, FMT).map_err(|e| format!("--op {}: {e}", k + 1))?;
            }
            if let Some(out) = flag(args, "--save") { save(&mut d, out)? }
            Ok(info(&d.p)?.to_string())
        }
        Some("export") => {
            let out = args.get(2).ok_or("Give the file to write.")?;
            let mut d = Doc::new(load(file()?)?);
            save(&mut d, out)?;
            Ok(format!("Saved {out}"))
        }
        Some("risk") => {
            let runs = flag(args, "--runs").and_then(|n| n.parse().ok()).unwrap_or(1000);
            Ok(sim(&load(file()?)?, runs)?.to_string())
        }
        Some("lookahead") => {
            let p = load(file()?)?;
            let weeks = flag(args, "--weeks").and_then(|n| n.parse().ok()).unwrap_or(3);
            Ok(serde_json::to_string_pretty(&progress::lookahead(&Calc::new(&p, FMT, today()), weeks)).unwrap_or_default())
        }
        Some("new") => {
            let mut d = Doc::new(new_plan(args.get(1).ok_or("Give a name.")?, args.get(2).ok_or("Give a start date.")?, args.get(3).map(String::as_str))?);
            let out = format!("{}.dplan", d.p.name.replace(['/', '\\', ':'], "-"));
            save(&mut d, &out)?;
            Ok(format!("Saved {out}"))
        }
        Some("mcp") => { mcp(std::io::stdin().lock(), std::io::stdout().lock()); Ok(String::new()) }
        Some(x) => Err(format!("Unknown command {x}.\n\n{HELP}")),
    }
}

fn new_plan(name: &str, start: &str, template: Option<&str>) -> Result<Project, String> {
    let start = chrono::NaiveDate::parse_from_str(start, FMT).map_err(|_| "The start date is like 2026-11-02.")?;
    let mut p = match template {
        Some(id) => templates::build(templates::TEMPLATES.iter().find(|t| t.id == id).ok_or(format!("No template {id}; see `datum-cli templates`."))?, start),
        None => Project::new(name, start),
    };
    p.name = name.into();
    Ok(p)
}

/// Writes a .dplan or any export format, by extension. Other extensions are refused.
fn save(d: &mut Doc, out: &str) -> Result<(), String> {
    let path = Path::new(out);
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    match ext.as_str() {
        "dplan" => d.save(path),
        "xml" | "xer" | "csv" | "xlsx" | "json" | "html" | "ics" | "mpx" | "pmxml" => io::export(&d.p, path, FMT, today(), &[]),
        _ => Err(format!("Datum Plan writes .dplan, .xml, .xer, .csv, .xlsx, .json, .html, .ics, .mpx or .pmxml, not \"{out}\".")),
    }
}

fn info(p: &Project) -> Result<Value, String> {
    let c = Calc::new(p, FMT, today());
    if let Some(e) = &c.error { return Err(e.clone()) }
    let s = c.summary();
    let crit: Vec<String> = (0..p.tasks.len()).filter(|&i| !c.s.summary[i] && !c.s.off[i] && c.s.critical(p, i)).map(|i| p.tasks[i].name.clone()).take(30).collect();
    Ok(json!({ "name": s.name, "start": s.start, "finish": s.finish, "duration": s.dur, "status_date": s.status, "tasks": s.tasks, "done": s.done, "late": s.late,
        "complete_pct": (s.pct * 10.0).round() / 10.0, "work": s.work, "cost": s.cost, "spi": s.spi, "cpi": s.cpi, "critical_tasks": crit, "error": s.error }))
}

fn rows(p: &Project) -> Result<Vec<Value>, String> {
    let c = Calc::new(p, FMT, today());
    if let Some(e) = &c.error { return Err(e.clone()) }
    Ok((0..p.tasks.len()).map(|i| json!({ "uid": p.tasks[i].uid, "id": i + 1, "code": p.tasks[i].code, "name": p.tasks[i].name, "level": p.tasks[i].level, "summary": c.s.summary[i],
        "dur": c.text(i, "dur"), "start": c.text(i, "start"), "finish": c.text(i, "finish"), "preds": c.text(i, "preds"), "res": c.text(i, "res"), "pct": c.pct[i],
        "total_float": c.text(i, "tf"), "critical": c.s.critical(p, i) })).collect())
}

fn table(p: &Project, csv: bool) -> Result<String, String> {
    let r = rows(p)?;
    let cols = ["id", "code", "name", "dur", "start", "finish", "total_float", "pct", "critical"];
    let cell = |v: &Value| match v { Value::String(s) => s.clone(), Value::Null => String::new(), x => x.to_string() };
    if csv {
        let q = |s: String| if s.contains([',', '"']) { format!("\"{}\"", s.replace('"', "\"\"")) } else { s };
        return Ok(std::iter::once(cols.join(",")).chain(r.iter().map(|x| cols.iter().map(|k| q(cell(&x[k]))).collect::<Vec<_>>().join(","))).collect::<Vec<_>>().join("\n"));
    }
    let lines: Vec<String> = r.iter().map(|x| {
        let name = format!("{}{}", "  ".repeat(x["level"].as_u64().unwrap_or(0) as usize), cell(&x["name"]));
        format!("{:>4} {:<8} {:<40} {:>10} {:>10} {:>10} {:>9} {:>4}% {}", cell(&x["id"]), cell(&x["code"]), name.chars().take(40).collect::<String>(), cell(&x["dur"]), cell(&x["start"]), cell(&x["finish"]), cell(&x["total_float"]), cell(&x["pct"]), if x["critical"].as_bool() == Some(true) { "critical" } else { "" })
    }).collect();
    Ok(format!("{:>4} {:<8} {:<40} {:>10} {:>10} {:>10} {:>9} {:>5}\n{}", "ID", "Code", "Task", "Duration", "Start", "Finish", "Float", "Done", lines.join("\n")))
}

fn sim(p: &Project, runs: u32) -> Result<Value, String> {
    let r = risk::simulate(p, &risk::SimOpts { iterations: runs, ..Default::default() })?;
    let d = |t: i64| datum_core::cal::date_of((t - 1).max(0)).format(FMT).to_string();
    Ok(json!({ "runs": r.runs, "planned_finish": d(r.planned), "chance_of_planned": (r.planned_chance * 100.0).round(), "p50": d(r.p50), "p80": d(r.p80), "p90": d(r.p90),
        "cost_p50": r.cost_p50, "cost_p80": r.cost_p80, "drivers": r.drivers.iter().map(|x| json!({"task": x.name, "correlation": (x.value * 100.0).round() / 100.0})).collect::<Vec<_>>(), "notes": r.notes }))
}

// ---------- MCP (Model Context Protocol) over stdio ----------

fn tools() -> Value {
    let obj = |props: Value, req: &[&str]| json!({ "type": "object", "properties": props, "required": req });
    json!([
        { "name": "open_plan", "description": "Open a plan file (.dplan, Project XML or XER) to work on. Returns its facts.", "inputSchema": obj(json!({ "path": { "type": "string" } }), &["path"]) },
        { "name": "new_plan", "description": "Start a new plan (optionally from a template: see list_templates).", "inputSchema": obj(json!({ "name": { "type": "string" }, "start": { "type": "string", "description": "YYYY-MM-DD" }, "template": { "type": "string" } }), &["name", "start"]) },
        { "name": "list_templates", "description": "The plan templates.", "inputSchema": obj(json!({}), &[]) },
        { "name": "plan_info", "description": "Facts about the open plan: dates, tasks, progress, cost, critical tasks.", "inputSchema": obj(json!({}), &[]) },
        { "name": "get_schedule", "description": "Every task with uid, dates, links, resources, progress, float and whether it is critical.", "inputSchema": obj(json!({}), &[]) },
        { "name": "list_operations", "description": "The named operations that change a plan, each with an example.", "inputSchema": obj(json!({}), &[]) },
        { "name": "apply_operation", "description": "Change the open plan with one named operation (see list_operations), e.g. {\"Set\":{\"uid\":2,\"field\":\"dur\",\"value\":\"4d\"}}. A failed operation changes nothing.", "inputSchema": obj(json!({ "operation": { "type": "object" } }), &["operation"]) },
        { "name": "undo", "description": "Undo the last change.", "inputSchema": obj(json!({}), &[]) },
        { "name": "schedule_risk", "description": "Monte Carlo schedule risk: P50, P80 and P90 finish, and the tasks that drive it.", "inputSchema": obj(json!({ "runs": { "type": "integer" } }), &[]) },
        { "name": "lookahead", "description": "The work in the next weeks from the status date, with open constraints.", "inputSchema": obj(json!({ "weeks": { "type": "integer" } }), &[]) },
        { "name": "save_plan", "description": "Save the plan: .dplan, or export as .xml .xer .csv .xlsx .json .html .ics .mpx .pmxml (by extension).", "inputSchema": obj(json!({ "path": { "type": "string" } }), &["path"]) },
    ])
}

fn call(doc: &mut Option<Doc>, name: &str, a: &Value) -> Result<Value, String> {
    let s = |k: &str| a[k].as_str().map(String::from);
    let need = |d: &Option<Doc>| d.is_some().then_some(()).ok_or_else(|| "Open or start a plan first (open_plan or new_plan).".to_string());
    match name {
        "open_plan" => { let p = load(&s("path").ok_or("path is needed")?)?; let v = info(&p)?; *doc = Some(Doc::new(p)); Ok(v) }
        "new_plan" => { let p = new_plan(&s("name").ok_or("name is needed")?, &s("start").ok_or("start is needed")?, s("template").as_deref())?; let v = info(&p)?; *doc = Some(Doc::new(p)); Ok(v) }
        "list_templates" => Ok(json!(templates::TEMPLATES.iter().map(|t| json!({"id": t.id, "name": t.name, "about": t.desc})).collect::<Vec<_>>())),
        "list_operations" => Ok(json!(ops::catalog())),
        _ => {
            need(doc)?;
            let d = doc.as_mut().ok_or("no plan")?;
            match name {
                "plan_info" => info(&d.p),
                "get_schedule" => Ok(json!(rows(&d.p)?)),
                "apply_operation" => {
                    let op = serde_json::from_value(a["operation"].clone()).map_err(|e| format!("Not an operation ({e}). See list_operations."))?;
                    d.apply(op, FMT)?;
                    info(&d.p)
                }
                "undo" => { d.undo(); info(&d.p) }
                "schedule_risk" => sim(&d.p, a["runs"].as_u64().unwrap_or(1000).min(10_000) as u32),
                "lookahead" => Ok(json!(progress::lookahead(&Calc::new(&d.p, FMT, today()), a["weeks"].as_u64().unwrap_or(3).min(12) as u32))),
                "save_plan" => { let out = s("path").ok_or("path is needed")?; save(d, &out)?; Ok(json!({ "saved": out })) }
                x => Err(format!("Unknown tool {x}.")),
            }
        }
    }
}

/// One JSON-RPC message per line in, one per line out.
fn mcp(input: impl BufRead, mut out: impl Write) {
    let mut doc: Option<Doc> = None;
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
            "initialize" => Ok(json!({ "protocolVersion": p["protocolVersion"].as_str().unwrap_or("2025-06-18"), "capabilities": { "tools": {} },
                "serverInfo": { "name": "datum-plan", "version": env!("CARGO_PKG_VERSION") },
                "instructions": "Datum Plan schedules projects. Open or start a plan, read it with get_schedule, change it only with apply_operation (see list_operations; tasks are named by uid), and save it with save_plan. Nothing is written to disk unless save_plan is called." })),
            "ping" => Ok(json!({})),
            "tools/list" => Ok(json!({ "tools": tools() })),
            "tools/call" => {
                let name = p["name"].as_str().unwrap_or("");
                let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| call(&mut doc, name, &p["arguments"]))).unwrap_or_else(|_| Err("Internal error; the plan is unchanged.".into()));
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

    fn session(lines: &[Value]) -> Vec<Value> {
        let input: String = lines.iter().map(|l| l.to_string() + "\n").collect::<String>() + "not json at all\n";
        let mut out = vec![];
        mcp(input.as_bytes(), &mut out);
        String::from_utf8_lossy(&out).lines().map(|l| serde_json::from_str(l).unwrap_or(Value::Null)).collect()
    }
    fn tool(id: u64, name: &str, args: Value) -> Value { json!({"jsonrpc": "2.0", "id": id, "method": "tools/call", "params": {"name": name, "arguments": args}}) }

    #[test]
    fn an_assistant_can_build_change_and_save_a_plan() {
        let dir = std::env::temp_dir().join(format!("datum-cli-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("p.dplan").display().to_string();
        let r = session(&[
            json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2025-06-18"}}),
            json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
            json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}),
            tool(3, "get_schedule", json!({})),
            tool(4, "new_plan", json!({"name": "Pump house", "start": "2026-11-02"})),
            tool(5, "apply_operation", json!({"operation": {"InsertRows": {"at": 0, "rows": [[["name", "Dig"], ["dur", "5d"]], [["name", "Pour"], ["dur", "3d"]]]}}})),
            tool(51, "apply_operation", json!({"operation": {"LinkSet": {"pred": 1, "succ": 2, "kind": "FS", "lag": ""}}})),
            tool(6, "apply_operation", json!({"operation": {"Set": {"uid": 999, "field": "dur", "value": "1d"}}})),
            tool(7, "save_plan", json!({"path": path})),
            tool(8, "save_plan", json!({"path": dir.join("x.exe").display().to_string()})),
            json!({"jsonrpc": "2.0", "id": 9, "method": "nope"}),
        ]);
        assert_eq!(r[0]["result"]["serverInfo"]["name"], "datum-plan");
        assert_eq!(r.len(), 11, "the notification gets no answer");
        assert!(r[1]["result"]["tools"].as_array().is_some_and(|t| t.len() == 11));
        assert_eq!(r[2]["result"]["isError"], true);
        assert_eq!(r.iter().find(|x| x["id"] == 4).map(|x| x["result"]["isError"].clone()), Some(json!(false)));
        let facts: Value = serde_json::from_str(r[5]["result"]["content"][0]["text"].as_str().unwrap_or("")).unwrap_or_default();
        assert_eq!((facts["tasks"].as_u64(), facts["finish"].as_str(), facts["work"].as_str()), (Some(2), Some("2026-11-11"), Some("0 hrs")));
        assert_eq!(r[6]["result"]["isError"], true);
        assert_eq!(r[7]["result"]["isError"], false);
        assert!(Doc::open(Path::new(&path)).is_ok_and(|d| d.p.tasks.len() == 2));
        assert_eq!(r[8]["result"]["isError"], true);
        assert_eq!(r[9]["error"]["code"], -32601);
        assert_eq!(r[10]["error"]["code"], -32700);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_command_line_reads_changes_and_exports() {
        let dir = std::env::temp_dir().join(format!("datum-cli2-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let f = dir.join("t.dplan").display().to_string();
        let mut d = Doc::new(new_plan("T", "2026-11-02", Some(templates::TEMPLATES[0].id)).unwrap_or_default());
        save(&mut d, &f).unwrap_or_else(|e| panic!("{e}"));
        let a = |v: &[&str]| run(&v.iter().map(|s| s.to_string()).collect::<Vec<_>>());
        assert!(a(&["info", &f]).is_ok_and(|t| t.contains("\"critical_tasks\"")));
        assert!(a(&["schedule", &f, "--csv"]).is_ok_and(|t| t.starts_with("id,code,name")));
        let out = dir.join("u.dplan").display().to_string();
        assert!(a(&["apply", &f, "--op", r#"{"Set":{"uid":2,"field":"dur","value":"9d"}}"#, "--save", &out]).is_ok());
        assert!(a(&["apply", &f, "--op", "{bad"]).is_err());
        assert!(a(&["export", &out, &dir.join("u.xer").display().to_string()]).is_ok());
        assert!(a(&["export", &out, &dir.join("u.sh").display().to_string()]).is_err());
        assert!(a(&["risk", &f, "--runs", "50"]).is_ok_and(|t| t.contains("p80")));
        assert!(a(&["ops"]).is_ok_and(|t| t.contains("ApplyCredit")));
        assert!(a(&["nonsense"]).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
