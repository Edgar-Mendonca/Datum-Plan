//! AI Link: Claude works on the plan open in this window (Model Context Protocol).
//!
//! Claude Desktop starts `Datum Plan.exe --mcp` (the bridge, no window). The bridge answers Claude's
//! questions about the tools itself and passes each tool call to the open window over
//! 127.0.0.1, with the key that the window wrote to `ai-link.json` in its settings folder when the
//! link was switched on. The window runs the tool on the plan shown (datum_core::assist), so every
//! change is an Undo step and appears at once. Nothing listens unless the link is on.
//!
//! Claude learns about Datum Plan from an extension file (`Datum Plan.mcpb`, made by `extension`):
//! one click on Install in Claude Desktop, no settings to edit. Writing Claude's settings file
//! (`setup_claude`) is the fallback.
use super::{add_tab, backup_old, close_at, docs, fresh, guard, out, regroup, remember, show_tab, tab_list, today, App, St};
use datum_core::{doc::Doc, fields::Calc, query::{builtin_filters, Filter, Rule}};
use datum_core::assist;
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

const IDENT: &str = "com.edgarmendonca.datumplan";
/// A request larger than this is refused (a long add_tasks list is far below it).
const MAX_LINE: u64 = 8 << 20;

/// A plan the assistant names: a tab number, an open plan's name or file, or a .dplan file on disk.
pub(crate) fn plan_ref(ap: &App, v: &Value) -> Result<(String, datum_core::Project), String> {
    let ds = docs(ap);
    let hit = match v {
        Value::Number(n) => n.as_u64().and_then(|k| ds.get(k as usize).copied()),
        Value::String(s) => ds.iter().find(|d| d.p.name.eq_ignore_ascii_case(s.trim()) || d.path.as_ref().is_some_and(|p| p.display().to_string().eq_ignore_ascii_case(s.trim()))).copied(),
        _ => return Err("plan is a tab number (list_plans), an open plan's name, or a .dplan file.".into()),
    };
    if let Some(d) = hit { return Ok((d.path.as_ref().ok_or("That plan is not saved yet: a link names the other plan's file, so ask the user to save it first.")?.display().to_string(), d.p.clone())) }
    let path = PathBuf::from(v.as_str().unwrap_or("").trim());
    if !path.is_file() { return Err(format!("No open plan or file {v}; see list_plans.")) }
    Ok((path.display().to_string(), Doc::open(&path)?.p))
}

/// What changed between two plans, for the assistant (at most 300 lines).
fn changes_json(c: datum_core::report::Changes) -> Value {
    let more = c.items.len().saturating_sub(300);
    json!({ "finish_was": c.finish[0], "finish_now": c.finish[1], "finish_variance": c.finish_var, "counts": c.counts, "changes": c.items.into_iter().take(300).collect::<Vec<_>>(), "more": more })
}

/// Many plans (0.9.13): links between plans, subplans, scenarios, compare, portfolio, pool, import.
fn many(app: &AppHandle, ap: &mut App, name: &str, a: &Value) -> Result<Value, String> {
    use datum_core::cal::date_of;
    let d0 = |m: i64| date_of(m).to_string();
    let d1 = |m: i64| date_of(m - 1).to_string();
    let txt = |k: &str| a[k].as_str().map(str::trim).filter(|s| !s.is_empty()).ok_or_else(|| format!("{k} is needed."));
    match name {
        "list_plan_links" => {
            let day = ap.doc.p.day_min;
            let links: Vec<Value> = super::link_states_in(ap).into_iter().map(|x| json!({ "uid": x.uid, "task": x.task, "waits_for": datum_core::links::label(&x.link, day), "file": x.link.file, "other_uid": x.link.uid,
                "type": datum_core::links::kind_lag(&x.link, day), "state": x.state, "other_start": x.now_start.map(d0), "other_finish": x.now_finish.map(d1) })).collect();
            let ds = docs(ap);
            let waiting: Vec<Value> = super::waited_on(ap).into_iter().flat_map(|(uid, v)| v.into_iter().map(move |(k, label, other)| (uid, k, label, other)))
                .map(|(uid, k, label, other)| json!({ "uid": uid, "tab": k, "plan": ds[k].p.name, "their_uid": other, "link": label })).collect();
            Ok(json!({ "links": links, "waiting_on_this_plan": waiting, "note": "state changed = the other plan moved since the last update_links." }))
        }
        "list_link_targets" => {
            if a["plan"].is_null() {
                return Ok(json!(docs(ap).iter().enumerate().filter(|(k, _)| *k != ap.cur).map(|(k, d)| json!({ "tab": k, "name": d.p.name, "file": d.path.as_ref().map(|p| p.display().to_string()) })).collect::<Vec<_>>()));
            }
            let (file, p) = plan_ref(ap, &a["plan"])?;
            let s = datum_core::sched::schedule(&p).ok();
            let tasks: Vec<Value> = p.tasks.iter().enumerate().map(|(i, t)| json!({ "uid": t.uid, "code": t.code, "name": t.name, "level": t.level, "summary": s.as_ref().is_some_and(|s| s.summary[i]),
                "start": s.as_ref().map(|s| d0(s.es[i])), "finish": s.as_ref().map(|s| d1(s.ef[i])) })).collect();
            Ok(json!({ "plan": p.name, "file": file, "tasks": tasks }))
        }
        "link_to_plan" => {
            let uid = a["uid"].as_u64().ok_or("uid is needed.")? as u32;
            let i = ap.doc.p.idx(uid).ok_or("No task with that uid.")?;
            let here = ap.doc.path.clone().ok_or("Save this plan first: the other plan finds it by its file.")?;
            let mut list = if a["append"].as_bool() == Some(true) { ap.doc.p.tasks[i].ext.clone() } else { vec![] };
            for x in a["links"].as_array().ok_or("links is needed (a list; empty removes them).")? {
                let (file, p) = plan_ref(ap, &x["plan"])?;
                if std::path::Path::new(&file) == here { return Err("That is this plan: use link_tasks for links inside it.".into()) }
                let other = x["task_uid"].as_u64().ok_or("Each link needs task_uid (list_link_targets).")? as u32;
                let k = p.idx(other).ok_or_else(|| format!("{} has no task {other}; see list_link_targets.", p.name))?;
                let kind = serde_json::from_value(json!(x["type"].as_str().unwrap_or("FS"))).map_err(|_| "type is FS, SS, FF or SF.".to_string())?;
                let (mut lag, pct, mut el) = datum_core::parse::lag(x["lag"].as_str().unwrap_or(""), ap.doc.p.day_min)?;
                if pct { return Err("A link to another plan takes a lag in time (2d, 7ed), not a percent.".into()) }
                if x["elapsed"].as_bool() == Some(true) && !el { lag = lag * 1440 / ap.doc.p.day_min.max(1); el = true }
                list.retain(|y: &datum_core::ExtLink| !(y.uid == other && y.file.eq_ignore_ascii_case(&file)));
                list.push(datum_core::ExtLink { file, uid: other, kind, lag, elapsed: el, plan: p.name.clone(), code: p.tasks[k].code.clone(), name: p.tasks[k].name.clone(), start: 0, finish: 0 });
            }
            let fmt = ap.opts.date_fmt.clone();
            ap.doc.apply(datum_core::doc::Op::ExtSet { uid, links: list.clone() }, &fmt)?;
            let msg = super::update_links_in(ap, false).map(|x| x.1).unwrap_or_default();
            Ok(json!({ "linked": list.len(), "update": msg }))
        }
        "update_links" => Ok(json!({ "result": super::update_links_in(ap, a["all"].as_bool().unwrap_or(false))?.1 })),
        "list_subplans" => {
            let role = if !ap.doc.p.subplans.is_empty() { "master" } else if !ap.doc.p.master.is_empty() { "subplan" } else { "plan" };
            let rows: Vec<Value> = super::subplan_rows_in(ap).into_iter().map(|r| json!({ "name": r.name, "file": r.file, "level": r.level, "found": r.found, "tab": (r.tab >= 0).then_some(r.tab),
                "start": (r.finish > 0).then(|| d0(r.start)), "finish": (r.finish > 0).then(|| d1(r.finish)), "pct": (r.pct * 10.0).round() / 10.0, "tasks": r.tasks })).collect();
            Ok(json!({ "this_plan": role, "master": ap.doc.p.master, "subplans": rows }))
        }
        "new_subplan" => {
            if !ap.doc.p.master.is_empty() { return Err("This plan is a subplan: show its master (switch_plan) to add one.".into()) }
            let at = a["at"].as_u64().map_or(usize::MAX, |x| x as usize);
            super::new_subplan_in(app, ap, txt("name")?.to_string(), a["level"].as_u64().unwrap_or(0).min(8) as u8, at)?;
            Ok(json!({ "opened": ap.doc.p.name, "tab": ap.cur }))
        }
        "add_subplan" => {
            if !ap.doc.p.master.is_empty() { return Err("This plan is a subplan: show its master (switch_plan) first.".into()) }
            let path = plan_path(a)?;
            if !path.is_file() { return Err(format!("There is no file {}.", path.display())) }
            super::add_subplan_in(app, ap, path)?;
            Ok(json!({ "subplans": ap.doc.p.subplans.len() }))
        }
        "remove_subplan" => {
            let f = txt("file")?;
            let key = datum_core::links::file_name(f).to_lowercase();
            let sp = ap.doc.p.subplans.iter().find(|s| datum_core::links::file_name(&s.file).to_lowercase() == key || s.name.eq_ignore_ascii_case(f)).cloned().ok_or("No such subplan; see list_subplans.")?;
            super::remove_subplan_in(ap, sp.file)?;
            Ok(json!({ "removed": sp.name, "note": "Its file is kept as it was." }))
        }
        "open_subplans" => {
            if ap.doc.p.subplans.is_empty() { return Err("This plan has no subplans.".into()) }
            let (_, missing) = super::open_subplans_in(app, ap);
            Ok(json!({ "open": tab_list(ap).len(), "not_found": missing }))
        }
        "apply_setup_to_subplans" => {
            let parts: Vec<String> = a["parts"].as_array().ok_or("parts is needed.")?.iter().filter_map(|x| x.as_str().map(String::from)).collect();
            let tabs: Vec<usize> = match a["tabs"].as_array() { Some(t) => t.iter().filter_map(|x| x.as_u64().map(|k| k as usize)).collect(), None => (0..tab_list(ap).len()).collect() };
            Ok(json!({ "result": super::share_setup_in(ap, parts, tabs)?.1 }))
        }
        "list_scenarios" => {
            let (plan, finish, cards) = super::scenarios_in(ap)?;
            Ok(json!({ "plan": plan, "finish": finish, "scenarios": cards.iter().map(|c| json!({ "name": c.name, "file": c.path, "finish": c.finish, "variance_days": c.var, "health": c.health, "tasks": c.tasks, "open": c.open })).collect::<Vec<_>>() }))
        }
        "new_scenario" => {
            super::new_scenario_in(app, ap, txt("name")?.to_string())?;
            Ok(json!({ "opened": ap.doc.p.name, "tab": ap.cur, "note": "The scenario is a copy saved next to the plan; change it freely, then compare_scenario and merge_scenario from the plan." }))
        }
        "compare_scenario" | "merge_scenario" => {
            let s = txt("scenario")?;
            let path = super::scenarios_in(ap)?.2.into_iter().find(|c| c.name.eq_ignore_ascii_case(s) || c.path.eq_ignore_ascii_case(s)).map(|c| c.path).ok_or("No such scenario; see list_scenarios.")?;
            if name == "compare_scenario" { return Ok(changes_json(super::scenario_compare_in(ap, path)?)) }
            let kinds: Vec<String> = a["kinds"].as_array().ok_or("kinds is needed.")?.iter().filter_map(|x| x.as_str().map(String::from)).collect();
            if let Some(k) = kinds.iter().find(|k| !datum_core::scenario::KINDS.iter().any(|(x, _)| x == k)) { return Err(format!("Unknown kind {k}: {}.", datum_core::scenario::KINDS.map(|x| x.0).join(", "))) }
            super::scenario_merge_in(ap, path, kinds)?;
            Ok(json!({ "merged": s }))
        }
        "compare_plans" => {
            let path = PathBuf::from(txt("path")?);
            let old = match datum_core::io::import(&path)? { datum_core::io::Imported::Plan(p, _) => *p, _ => return Err("Compare needs a plan file (.dplan, .xml or .xer).".into()) };
            Ok(changes_json(super::compare_with(ap, old)))
        }
        "portfolio" => {
            let folder = a["folder"].as_str().map(str::trim).filter(|s| !s.is_empty()).map(String::from).unwrap_or_else(|| ap.opts.workspace.clone());
            if folder.is_empty() { return Err("No workspace folder yet: give folder, or ask the user to choose one (Report › Portfolio).".into()) }
            let cards = super::workspace_cards(ap, &folder)?;
            let cards: Vec<Value> = cards.into_iter().map(|c| { let mut v = serde_json::to_value(c).unwrap_or_default(); if let Some(o) = v.as_object_mut() { for k in ["s", "f", "bf"] { o.remove(k); } } v }).collect();
            Ok(json!({ "folder": folder, "plans": cards }))
        }
        "resource_pool" => {
            let unit = a["unit"].as_str().unwrap_or("week").to_string();
            let v = super::pool_view_in(ap, unit)?;
            let b = &v.view.buckets;
            let rows: Vec<Value> = v.view.rows.iter().map(|r| json!({ "name": r.name, "group": r.group, "in_pool": r.in_pool, "periods_over": r.over, "peak_load": (r.peak * 100.0).round() / 100.0,
                "hours_by_plan": v.view.plans.iter().zip(&r.plans).map(|(n, h)| (n.clone(), json!((h.iter().sum::<f64>() * 10.0).round() / 10.0))).collect::<serde_json::Map<_, _>>(),
                "over": (0..b.len()).filter(|&j| r.load[j] > r.cap[j] + 1e-6).take(20).map(|j| json!({ "from": date_of(b[j] * 1440).to_string(), "hours": (r.load[j] * 10.0).round() / 10.0, "capacity": (r.cap[j] * 10.0).round() / 10.0 })).collect::<Vec<_>>() })).collect();
            Ok(json!({ "folder": v.folder, "pool_plan": v.pool, "plans": v.view.plans, "resources": rows }))
        }
        "set_pool_plan" => {
            let path = a["path"].as_str().map(str::trim).unwrap_or("").to_string();
            if !path.is_empty() && !Path::new(&path).is_file() { return Err(format!("There is no file {path}.")) }
            super::set_pool_in(app, ap, path.clone())?;
            Ok(json!({ "pool": path }))
        }
        "take_from_pool" => Ok(json!({ "result": super::pool_take_in(ap, a["add"].as_bool().unwrap_or(false))?.1 })),
        "import_plan" => {
            let path = PathBuf::from(txt("path")?);
            if !path.is_file() { return Err(format!("There is no file {}.", path.display())) }
            match datum_core::io::import(&path)? {
                datum_core::io::Imported::Plan(p, notes) => {
                    let p = fresh(ap, *p);
                    let mut d = Doc::new(p);
                    d.dirty = true;
                    add_tab(ap, d);
                    Ok(json!({ "opened": ap.doc.p.name, "tab": ap.cur, "notes": notes }))
                }
                _ => Err("That file is a table: its columns must be matched by the user (File › Import).".into()),
            }
        }
        _ => Err(format!("Unknown tool {name}.")),
    }
}

/// The settings folder (the same one Tauri uses for options.json), found without the app running.
pub fn settings_dir() -> Option<PathBuf> {
    let home = std::env::var_os(if cfg!(windows) { "APPDATA" } else { "HOME" }).map(PathBuf::from)?;
    Some(if cfg!(windows) { home.join(IDENT) } else if cfg!(target_os = "macos") { home.join("Library/Application Support").join(IDENT) } else {
        std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from).unwrap_or_else(|| home.join(".config")).join(IDENT)
    })
}
fn link_file() -> Option<PathBuf> { settings_dir().map(|d| d.join("ai-link.json")) }

/// Claude Desktop's settings file. On Windows, Claude Desktop installed as a packaged app keeps it in
/// its own folder (%LOCALAPPDATA%\Packages\Claude_…\LocalCache\Roaming\Claude), not in %APPDATA%.
pub fn claude_config() -> Option<PathBuf> {
    if cfg!(windows) {
        let packages = std::env::var_os("LOCALAPPDATA").map(|d| PathBuf::from(d).join("Packages"));
        let own = packages.and_then(|d| std::fs::read_dir(d).ok()).into_iter().flatten().flatten()
            .find(|e| e.file_name().to_string_lossy().starts_with("Claude_")).map(|e| e.path().join("LocalCache").join("Roaming").join("Claude"));
        if let Some(d) = own.filter(|d| d.is_dir()) { return Some(d.join("claude_desktop_config.json")) }
    }
    let home = std::env::var_os(if cfg!(windows) { "APPDATA" } else { "HOME" }).map(PathBuf::from)?;
    Some(if cfg!(windows) { home.join("Claude") } else if cfg!(target_os = "macos") { home.join("Library/Application Support/Claude") } else { home.join(".config/Claude") }.join("claude_desktop_config.json"))
}

/// A fresh key: 128 bits from the system's random hashing seeds and the time.
fn key() -> String {
    use std::hash::{BuildHasher, Hasher};
    let part = |salt: u64| { let mut h = std::collections::hash_map::RandomState::new().build_hasher(); h.write_u64(salt); h.write_u128(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0)); h.finish() };
    format!("{:016x}{:016x}", part(std::process::id() as u64), part(0x5eed))
}

struct Link { port: u16, on: Arc<AtomicBool> }
static LINK: Mutex<Option<Link>> = Mutex::new(None);

pub fn is_on() -> bool { LINK.lock().map(|l| l.is_some()).unwrap_or(false) }

/// Switches the link on: listens on 127.0.0.1 (a free port) and writes the port and key for the bridge.
pub fn start(app: &AppHandle) -> Result<(), String> {
    let mut g = LINK.lock().map_err(|_| "The AI Link is busy.")?;
    if g.is_some() { return Ok(()) }
    let l = TcpListener::bind("127.0.0.1:0").map_err(|e| format!("The AI Link could not start: {e}"))?;
    let port = l.local_addr().map_err(|e| e.to_string())?.port();
    let k = key();
    let f = link_file().ok_or("No settings folder for the AI Link.")?;
    if let Some(d) = f.parent() { let _ = std::fs::create_dir_all(d); }
    let mine = json!({ "port": port, "key": k, "pid": std::process::id() }).to_string();
    datum_core::doc::write_atomic(&f, mine.as_bytes())?;
    let on = Arc::new(AtomicBool::new(true));
    let app2 = app.clone();
    listen(l, k, on.clone(), move |n, a| run(&app2, n, a));
    keep(f, mine, on.clone());
    *g = Some(Link { port, on });
    Ok(())
}

/// Keeps the link file while the link is on: another Datum Plan window (a second copy started by
/// hand, or by an assistant using the screen) may write its own and remove it when it closes; then
/// this window writes its own again, so the assistant can still reach it.
fn keep(f: PathBuf, mine: String, on: Arc<AtomicBool>) {
    std::thread::spawn(move || while on.load(Ordering::SeqCst) {
        std::thread::sleep(Duration::from_secs(2));
        if on.load(Ordering::SeqCst) && !f.exists() { let _ = datum_core::doc::write_atomic(&f, mine.as_bytes()); }
    });
}

/// Whether the link file is this window's (by its process id).
fn ours(f: &Path) -> bool {
    std::fs::read_to_string(f).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()).is_some_and(|v| v["pid"].as_u64() == Some(std::process::id() as u64))
}

/// Switches the link off: forgets the key, removes the file and stops listening.
pub fn stop() {
    if let Some(l) = LINK.lock().ok().and_then(|mut g| g.take()) {
        l.on.store(false, Ordering::SeqCst);
        let _ = TcpStream::connect(("127.0.0.1", l.port)); // wakes the listener so it ends
    }
    // Only this window's file: another window's link stays reachable.
    if let Some(f) = link_file().filter(|f| ours(f)) { let _ = std::fs::remove_file(f); }
}

/// Serves tool calls one after the other until `on` goes false.
fn listen(l: TcpListener, k: String, on: Arc<AtomicBool>, run: impl Fn(&str, &Value) -> Result<Value, String> + Send + 'static) {
    std::thread::spawn(move || {
        for s in l.incoming() {
            if !on.load(Ordering::SeqCst) { break }
            if let Ok(s) = s { serve(s, &k, &run) }
        }
    });
}

/// One tool call from the bridge: {"key", "name", "arguments"} in, {"ok": …} or {"err": …} out.
fn serve(s: TcpStream, k: &str, run: &impl Fn(&str, &Value) -> Result<Value, String>) {
    let _ = s.set_read_timeout(Some(Duration::from_secs(10)));
    let mut line = String::new();
    if BufReader::new((&s).take(MAX_LINE)).read_line(&mut line).is_err() { return }
    let req: Value = serde_json::from_str(&line).unwrap_or(Value::Null);
    let given = req["key"].as_str().unwrap_or("");
    // Same time whatever the key: compare every byte.
    let same = given.len() == k.len() && given.bytes().zip(k.bytes()).fold(0u8, |d, (a, b)| d | (a ^ b)) == 0;
    let reply = if !same { json!({ "err": "The AI Link key does not match: switch the AI Link off and on in Datum Plan, then try again." }) } else {
        match run(req["name"].as_str().unwrap_or(""), &req["arguments"]) { Ok(v) => json!({ "ok": v }), Err(e) => json!({ "err": e }) }
    };
    let mut w = s;
    let _ = writeln!(w, "{reply}");
}

/// What the window shows, as it last said (for get_view).
static VIEW: Mutex<Option<Value>> = Mutex::new(None);
pub fn view_state(v: Value) { if let Ok(mut g) = VIEW.lock() { *g = Some(v) } }

const VIEWS: [&str; 29] = ["setup", "gantt", "tracking", "network", "calendar", "taskusage", "ressheet", "resusage", "resgraph", "team", "dashboard", "reports", "portfolio", "risks", "simrisk", "checks",
    "lookahead", "manpower", "timeline", "progress", "scurve", "sheets", "update", "relations", "builder", "contract", "documents", "procurement", "delay"];
const TABLES: [&str; 10] = ["Entry", "Schedule", "Tracking", "Cost", "Work", "Variance", "Earned Value", "Summary", "Baseline", "Activities"];
const PANES: [&str; 13] = ["general", "status", "resources", "predecessors", "successors", "codes", "steps", "notebook", "expenses", "docs", "custom", "risks", "notes"];

/// Asks the window to change what it shows.
fn ui(app: &AppHandle, action: &str, a: Value) { let _ = app.emit("ai-ui", json!({ "action": action, "a": a })); }

/// A .dplan path the user gave.
fn plan_path(a: &Value) -> Result<PathBuf, String> {
    let p = PathBuf::from(a["path"].as_str().map(str::trim).filter(|x| !x.is_empty()).ok_or("path is needed (a .dplan file).")?);
    if !p.extension().is_some_and(|e| e.eq_ignore_ascii_case("dplan")) { return Err("The path must end in .dplan.".into()) }
    Ok(p)
}

/// Runs a tool on the plan shown and tells the window what changed.
fn run(app: &AppHandle, name: &str, a: &Value) -> Result<Value, String> {
    let st = app.state::<St>();
    let (r, shown) = {
        let mut ap = st.lock();
        let r = guard(|| match name {
            "_tools" => Ok(assist::tools()),
            "list_plans" => Ok(json!({ "shown": ap.cur, "plans": tab_list(&ap).iter().enumerate().map(|(k, t)| json!({ "tab": k, "name": t.name, "file": t.path, "unsaved": t.dirty })).collect::<Vec<_>>() })),
            "switch_plan" => {
                let tabs = tab_list(&ap);
                let k = a["tab"].as_u64().map(|k| k as usize).or_else(|| a["name"].as_str().and_then(|n| tabs.iter().position(|t| t.name.eq_ignore_ascii_case(n)))).filter(|&k| k < tabs.len()).ok_or("No such plan; see list_plans.")?;
                show_tab(&mut ap, k);
                Ok(json!({ "shown": tabs[k].name }))
            }
            "get_view" => Ok(json!({ "plan": tab_list(&ap).get(ap.cur).map(|t| t.name.clone()), "window": VIEW.lock().ok().and_then(|g| g.clone()) })),
            "show_view" => {
                let v = a["view"].as_str().filter(|v| VIEWS.contains(v)).ok_or_else(|| format!("view is one of: {}.", VIEWS.join(", ")))?;
                ui(app, "view", json!(v));
                Ok(json!({ "shown": v }))
            }
            "use_table" => {
                let t = a["table"].as_str().map(str::trim).ok_or("table is needed.")?;
                let saved: Vec<String> = ap.doc.p.ui["tables"].as_object().map(|m| m.keys().cloned().collect()).unwrap_or_default();
                let t = TABLES.iter().map(|x| x.to_string()).chain(saved).find(|x| x.eq_ignore_ascii_case(t)).ok_or_else(|| format!("No table {t}: {}.", TABLES.join(", ")))?;
                ui(app, "table", json!(t));
                Ok(json!({ "table": t }))
            }
            "filter_view" => {
                let c = Calc::new(&ap.doc.p, &ap.opts.date_fmt, today());
                let mut f = if a["clear"].as_bool() == Some(true) { Filter { name: "All Tasks".into(), summaries: true, ..Default::default() } }
                    else if let Some(r) = a["rules"].as_array().filter(|r| !r.is_empty()) {
                        let rules: Vec<Rule> = r.iter().map(|x| serde_json::from_value(x.clone()).map_err(|e| format!("A rule is {{field, test, value}}: {e}"))).collect::<Result<_, _>>()?;
                        if let Some(x) = rules.iter().find(|x| c.key_of(&x.field).is_none()) { return Err(format!("No field \"{}\"; see list_fields.", x.field)) }
                        Filter { name: "Assistant's filter".into(), rules, summaries: true, highlight: false }
                    } else {
                        let n = a["name"].as_str().ok_or("Give a filter name, rules, or clear.")?;
                        let saved: Vec<Filter> = serde_json::from_value(ap.doc.p.ui["filters"].clone()).unwrap_or_default();
                        builtin_filters().into_iter().chain(saved).find(|x| x.name.eq_ignore_ascii_case(n)).ok_or_else(|| format!("No filter called {n}; see list_filters."))?
                    };
                if let Some(h) = a["highlight"].as_bool() { f.highlight = h }
                let name = f.name.clone();
                ui(app, "filter", serde_json::to_value(&f).map_err(|e| e.to_string())?);
                Ok(json!({ "filter": name }))
            }
            "group_view" | "sort_view" => {
                let c = Calc::new(&ap.doc.p, &ap.opts.date_fmt, today());
                let key = |x: &str| c.key_of(x).ok_or_else(|| format!("No field \"{x}\"; see list_fields."));
                let list = a["fields"].as_array().ok_or("fields is needed (a list; empty clears it).")?;
                if name == "group_view" {
                    let keys = list.iter().map(|x| key(x.as_str().unwrap_or(""))).collect::<Result<Vec<_>, _>>()?;
                    ui(app, "group", json!(keys));
                    Ok(json!({ "grouped_by": keys }))
                } else {
                    let keys = list.iter().map(|x| Ok(json!([key(x["field"].as_str().or(x.as_str()).unwrap_or(""))?, x["desc"].as_bool().unwrap_or(false)]))).collect::<Result<Vec<_>, String>>()?;
                    ui(app, "sort", json!(keys));
                    Ok(json!({ "sorted_by": keys }))
                }
            }
            "outline_view" => {
                if let Some(all) = a["all"].as_str() {
                    if !matches!(all, "expand" | "collapse") { return Err("all is expand or collapse.".into()) }
                    let sums: Vec<u32> = if all == "collapse" { (0..ap.doc.p.tasks.len()).filter(|&i| ap.doc.p.is_summary(i)).map(|i| ap.doc.p.tasks[i].uid).collect() } else { vec![] };
                    ui(app, "collapse", json!(sums));
                    return Ok(json!({ "summaries": all }))
                }
                let l = a["level"].as_u64().filter(|l| *l <= 20).ok_or("Give level (0 = all) or all (expand, collapse).")?;
                ui(app, "level", json!(l));
                Ok(json!({ "level": l }))
            }
            "select_task" => {
                let uid = a["uid"].as_u64().ok_or("uid is needed.")? as u32;
                ap.doc.p.idx(uid).ok_or("No task with that uid.")?;
                ui(app, "select", json!(uid));
                Ok(json!({ "selected": uid }))
            }
            "zoom_view" => {
                let to = a["to"].as_str().filter(|t| matches!(*t, "in" | "out" | "entire")).ok_or("to is in, out or entire.")?;
                ui(app, "zoom", json!(to));
                Ok(json!({ "zoom": to }))
            }
            "gantt_options" => {
                if let Some(b) = a["baseline"].as_u64() { if b > 10 { return Err("baseline is 0 to 10.".into()) } }
                let o: serde_json::Map<String, Value> = ["critical", "slack", "late", "labels", "baseline_bars", "baseline"].iter().filter_map(|k| a.get(*k).filter(|v| !v.is_null()).map(|v| (k.to_string(), v.clone()))).collect();
                if o.is_empty() { return Err("Give critical, slack, late, labels, baseline_bars or baseline.".into()) }
                ui(app, "gantt", Value::Object(o.clone()));
                Ok(Value::Object(o))
            }
            "task_path_view" => {
                if let Some(u) = a["uid"].as_u64() { ap.doc.p.idx(u as u32).ok_or("No task with that uid.")?; }
                ui(app, "path", json!({ "on": a["on"].as_bool().unwrap_or(true), "uid": a["uid"] }));
                Ok(json!({ "task_path": a["on"].as_bool().unwrap_or(true) }))
            }
            "show_panes" => {
                if let Some(t) = a["tab"].as_str() { if !PANES.contains(&t) { return Err(format!("tab is one of: {}.", PANES.join(", "))) } }
                ui(app, "panes", json!({ "details": a["details"], "tab": a["tab"], "timeline": a["timeline"] }));
                Ok(json!({ "panes": "changed" }))
            }
            "list_templates" => Ok(json!(datum_core::templates::TEMPLATES.iter().map(|t| json!({ "id": t.id, "name": t.name, "about": t.desc })).collect::<Vec<_>>())),
            "new_plan" => {
                let p = match a["template"].as_str().filter(|x| !x.is_empty()) {
                    Some(id) => datum_core::templates::build(datum_core::templates::TEMPLATES.iter().find(|t| t.id.eq_ignore_ascii_case(id)).ok_or("No such template; see list_templates.")?, super::next_monday()),
                    None => datum_core::Project::new("Project1", today()),
                };
                let p = fresh(&ap, p);
                add_tab(&mut ap, Doc::new(p));
                Ok(json!({ "opened": ap.doc.p.name, "tab": ap.cur }))
            }
            "open_plan" => {
                let path = plan_path(a)?;
                if !path.is_file() { return Err(format!("There is no file {}.", path.display())) }
                let same = |d: &Doc| d.path.as_ref().is_some_and(|x| x == &path || x.canonicalize().ok() == path.canonicalize().ok());
                if let Some(k) = docs(&ap).iter().position(|d| same(d)) { show_tab(&mut ap, k); return Ok(json!({ "shown": k })) }
                backup_old(&path);
                let d = Doc::open(&path)?;
                remember(app, &path);
                add_tab(&mut ap, d);
                Ok(json!({ "opened": ap.doc.p.name, "tab": ap.cur }))
            }
            "save_plan_as" => {
                let path = plan_path(a)?;
                if !path.parent().is_some_and(|d| d.as_os_str().is_empty() || d.is_dir()) { return Err("That folder does not exist.".into()) }
                if path.exists() && a["overwrite"].as_bool() != Some(true) { return Err(format!("{} exists already: ask the user, then use overwrite true.", path.display())) }
                ap.doc.save(&path)?;
                remember(app, &path);
                Ok(json!({ "saved": path.display().to_string() }))
            }
            "close_plan" => {
                let k = a["tab"].as_u64().map(|k| k as usize).unwrap_or(ap.cur);
                let d = *docs(&ap).get(k).ok_or("No such tab; see list_plans.")?;
                if d.dirty && a["discard"].as_bool() != Some(true) { return Err("That plan has unsaved changes: ask the user to save it, or to close it without saving (discard true).".into()) }
                let n = d.p.name.clone();
                close_at(app, &mut ap, k);
                Ok(json!({ "closed": n }))
            }
            "export_plan" | "export_register" => {
                let dir = app.path().download_dir().map_err(|_| "No Downloads folder.")?;
                let p = &ap.doc.p;
                let clean = |s: &str| s.chars().map(|c| if c.is_alphanumeric() || " -_().".contains(c) { c } else { ' ' }).collect::<String>().split_whitespace().collect::<Vec<_>>().join(" ");
                let file = |stem: &str, ext: &str| { let base = clean(stem).chars().take(100).collect::<String>(); let mut f = dir.join(format!("{base}.{ext}")); for n in 2..1000 { if !f.exists() { break } f = dir.join(format!("{base} ({n}).{ext}")) } f };
                let fmt = ap.opts.date_fmt.clone();
                let f = if name == "export_plan" {
                    let ext = a["format"].as_str().filter(|x| ["xlsx", "ods", "csv", "xml", "xer", "pmxml", "mpx", "json", "html", "ics"].contains(x)).ok_or("format is xlsx, ods, csv, xml, xer, pmxml, mpx, json, html or ics.")?;
                    let f = file(&p.name, ext);
                    if ext == "xer" { datum_core::io::export_xer(p, &f, today(), datum_core::io::xer::DEFAULT_VERSION)?; } else { datum_core::io::export(p, &f, &fmt, today(), &[])?; }
                    f
                } else {
                    let c = datum_core::fields::Calc::new(p, &fmt, today());
                    if let Some(e) = &c.error { return Err(e.clone()) }
                    match a["register"].as_str() {
                        Some("documents") => { let f = file(&format!("{} - Document Register", p.name), "xlsx"); datum_core::docreg::write_book(p, &datum_core::docreg::status(&c), &f)?; f }
                        Some("procurement") => { let f = file(&format!("{} - Procurement", p.name), "xlsx"); datum_core::procure::write_book(p, &datum_core::procure::status(&c), &f)?; f }
                        Some("delays") => { let f = file(&format!("{} - Delay Analysis", p.name), "xlsx"); datum_core::delay::write_book(&c, &f)?; f }
                        _ => return Err("register is documents, procurement or delays.".into()),
                    }
                };
                Ok(json!({ "saved": f.display().to_string() }))
            }
            "save_plan" => {
                let p = ap.doc.path.clone().filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("dplan"))).ok_or("This plan has no .dplan file yet: ask the user to save it once (File › Save).")?;
                ap.doc.save(&p)?;
                Ok(json!({ "saved": p.display().to_string() }))
            }
            n if assist::MANY_PLANS.contains(&n) => many(app, &mut ap, n, a),
            _ => {
                let fmt = ap.opts.date_fmt.clone();
                let r = assist::call(&mut ap.doc, &fmt, today(), name, a);
                if r.is_ok() && assist::changes(name) { regroup(&mut ap) }
                r
            }
        });
        let shown = (r.is_ok() && (assist::changes(name) || matches!(name, "switch_plan" | "save_plan" | "new_plan" | "open_plan" | "save_plan_as" | "close_plan") || assist::MANY_PLANS_CHANGE.contains(&name))).then(|| out(&ap));
        (r, shown)
    };
    if let Some(o) = shown { let _ = app.emit("ai-change", json!({ "out": o, "tool": name })); }
    r
}

/// What the window shows about the link.
pub fn info() -> Value {
    let exe = std::env::current_exe().map(|p| p.display().to_string()).unwrap_or_default();
    let cfg = claude_config();
    let set = cfg.as_ref().and_then(|c| std::fs::read_to_string(c).ok()).and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .is_some_and(|v| v["mcpServers"]["datum-plan"]["command"].as_str() == Some(exe.as_str()));
    json!({ "on": is_on(), "exe": exe, "claude_config": cfg.map(|c| c.display().to_string()), "claude_set": set,
        "snippet": serde_json::to_string_pretty(&json!({ "mcpServers": { "datum-plan": { "command": exe, "args": ["--mcp"] } } })).unwrap_or_default() })
}

/// Adds Datum Plan to Claude Desktop's settings (keeping everything else there, with a backup).
pub fn setup_claude() -> Result<String, String> {
    let f = claude_config().ok_or("Claude Desktop's settings folder was not found.")?;
    let exe = std::env::current_exe().map_err(|e| e.to_string())?.display().to_string();
    let old = std::fs::read_to_string(&f).ok();
    let mut v: Value = match &old { Some(t) if !t.trim().is_empty() => serde_json::from_str(t).map_err(|_| format!("{} is not valid JSON, so it was left as it is. Add Datum Plan by hand (the text in this dialog).", f.display()))?, _ => json!({}) };
    if !v.is_object() { return Err(format!("{} has an unexpected layout; it was left as it is.", f.display())) }
    if !v["mcpServers"].is_object() { v["mcpServers"] = json!({}); }
    v["mcpServers"]["datum-plan"] = json!({ "command": exe, "args": ["--mcp"] });
    if let Some(d) = f.parent() { std::fs::create_dir_all(d).map_err(|e| e.to_string())?; }
    if let Some(t) = old { let _ = std::fs::write(f.with_extension("json.bak"), t); }
    datum_core::doc::write_atomic(&f, serde_json::to_string_pretty(&v).unwrap_or_default().as_bytes())?;
    Ok(f.display().to_string())
}

/// The extension's manifest (MCPB): Claude runs `bin` inside the extension with --mcp.
fn manifest(bin: &str) -> Value {
    let platform = if cfg!(windows) { "win32" } else if cfg!(target_os = "macos") { "darwin" } else { "linux" };
    json!({ "manifest_version": "0.3", "name": "datum-plan", "display_name": "Datum Plan", "version": env!("CARGO_PKG_VERSION"),
        "description": "Read and change the plan open in Datum Plan: tasks, links, progress, resources, calendars, codes, checks, curves and sheets.",
        "long_description": "Works with the plan you have open in Datum Plan, on this computer only. Switch it on in Datum Plan (Home › AI Link). Every change shows at once and is one Undo step in Datum Plan; nothing is saved unless you ask.",
        "author": { "name": "Datum", "url": "https://github.com/Edgar-Mendonca/Datum-Plan" }, "homepage": "https://github.com/Edgar-Mendonca/Datum-Plan", "license": "MIT",
        "icon": "icon.png", "keywords": ["schedule", "planning", "gantt", "project"],
        "server": { "type": "binary", "entry_point": bin, "mcp_config": { "command": format!("${{__dirname}}/{bin}"), "args": ["--mcp"] } },
        "tools": assist::tools().as_array().into_iter().flatten().map(|t| json!({ "name": t["name"], "description": t["description"] })).collect::<Vec<_>>(),
        // Newer Datum Plan versions add tools: the client takes the list the server gives, not only these.
        "tools_generated": true, "prompts_generated": true,
        "compatibility": { "platforms": [platform] } })
}

/// Makes `Datum Plan.mcpb` in `dir`: the manifest, the icon and a copy of this program as the bridge.
/// Claude Desktop installs it with one click (Install), and it keeps working after Datum Plan updates:
/// the bridge only passes calls on, and asks the open window for the current tool list.
pub fn extension(dir: &Path) -> Result<PathBuf, String> {
    use zip::write::SimpleFileOptions;
    let bin = if cfg!(windows) { "server/datum-plan.exe" } else { "server/datum-plan" };
    let exe = std::fs::read(std::env::current_exe().map_err(|e| e.to_string())?).map_err(|e| format!("Datum Plan could not read its own program: {e}"))?;
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let f = dir.join("Datum Plan.mcpb");
    let tmp = f.with_extension("mcpb.part");
    let e = |x: zip::result::ZipError| x.to_string();
    let mut z = zip::ZipWriter::new(std::fs::File::create(&tmp).map_err(|e| e.to_string())?);
    let o = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    for (name, bytes, mode) in [("manifest.json", serde_json::to_vec_pretty(&manifest(bin)).unwrap_or_default(), 0o644), ("icon.png", include_bytes!("../icons/icon.png").to_vec(), 0o644), (bin, exe, 0o755)] {
        z.start_file(name, o.unix_permissions(mode)).map_err(e)?;
        z.write_all(&bytes).map_err(|e| e.to_string())?;
    }
    z.finish().map_err(e)?;
    std::fs::rename(&tmp, &f).map_err(|e| e.to_string())?;
    Ok(f)
}

// ---------- the bridge (`Datum Plan.exe --mcp`, started by Claude) ----------

/// Passes one tool call to the open window, or says why it cannot.
fn forward(name: &str, args: &Value) -> Result<Value, String> {
    const OFF: &str = "Datum Plan is not open, or its AI Link is off. Ask the user to open Datum Plan and click Home › AI Link (off and on again if it says on). Do not use the screen or start another copy of Datum Plan instead.";
    let link: Value = link_file().and_then(|f| std::fs::read_to_string(f).ok()).and_then(|t| serde_json::from_str(&t).ok()).ok_or(OFF)?;
    let port = link["port"].as_u64().filter(|&p| p > 0 && p < 65536).ok_or(OFF)? as u16;
    let mut s = TcpStream::connect_timeout(&std::net::SocketAddr::from(([127, 0, 0, 1], port)), Duration::from_secs(3)).map_err(|_| OFF.to_string())?;
    let _ = s.set_read_timeout(Some(Duration::from_secs(600)));
    writeln!(s, "{}", json!({ "key": link["key"], "name": name, "arguments": args })).map_err(|_| OFF.to_string())?;
    let mut line = String::new();
    BufReader::new(s.take(MAX_LINE * 4)).read_line(&mut line).map_err(|e| format!("Datum Plan did not answer: {e}"))?;
    let r: Value = serde_json::from_str(&line).map_err(|_| "Datum Plan's answer could not be read.".to_string())?;
    match (r.get("ok"), r["err"].as_str()) { (Some(v), _) => Ok(v.clone()), (_, Some(e)) => Err(e.into()), _ => Err("Datum Plan's answer was empty.".into()) }
}

/// Answers one MCP message (None for a notification).
pub fn answer(msg: &Value, call: impl Fn(&str, &Value) -> Result<Value, String>) -> Option<Value> {
    let id = msg.get("id")?.clone();
    let p = &msg["params"];
    let result = match msg["method"].as_str().unwrap_or("") {
        "initialize" => Ok(json!({ "protocolVersion": p["protocolVersion"].as_str().unwrap_or("2025-06-18"), "capabilities": { "tools": {}, "prompts": {} },
            "serverInfo": { "name": "datum-plan", "version": env!("CARGO_PKG_VERSION") }, "instructions": assist::INSTRUCTIONS })),
        "ping" => Ok(json!({})),
        "prompts/list" => Ok(json!({ "prompts": assist::prompts() })),
        "prompts/get" => assist::prompt(p["name"].as_str().unwrap_or(""), &p["arguments"])
            .map(|t| json!({ "messages": [{ "role": "user", "content": { "type": "text", "text": t } }] }))
            .map_err(|e| json!({ "code": -32602, "message": e })),
        // The open window's list (a newer Datum Plan may have more tools than this copy), else this one's.
        "tools/list" => Ok(json!({ "tools": call("_tools", &json!({})).ok().filter(Value::is_array).unwrap_or_else(assist::tools) })),
        "tools/call" => Ok(match call(p["name"].as_str().unwrap_or(""), &p["arguments"]) {
            Ok(v) => json!({ "content": [{ "type": "text", "text": serde_json::to_string_pretty(&v).unwrap_or_default() }], "isError": false }),
            Err(e) => json!({ "content": [{ "type": "text", "text": e }], "isError": true }),
        }),
        m => Err(json!({ "code": -32601, "message": format!("Method not found: {m}") })),
    };
    Some(match result { Ok(r) => json!({ "jsonrpc": "2.0", "id": id, "result": r }), Err(e) => json!({ "jsonrpc": "2.0", "id": id, "error": e }) })
}

/// The bridge: MCP messages on stdin and stdout, one per line.
pub fn bridge() {
    let (stdin, mut stdout) = (std::io::stdin(), std::io::stdout());
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() { continue }
        let reply = match serde_json::from_str::<Value>(&line) {
            Ok(msg) => answer(&msg, forward),
            Err(e) => Some(json!({ "jsonrpc": "2.0", "id": null, "error": { "code": -32700, "message": format!("Parse error: {e}") } })),
        };
        if let Some(r) = reply { let _ = writeln!(stdout, "{r}"); let _ = stdout.flush(); }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn the_bridge_lists_tools_and_says_when_the_window_is_closed() {
        let init = answer(&json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": { "protocolVersion": "2025-06-18" } }), |_, _| Ok(json!(null))).unwrap();
        assert_eq!(init["result"]["serverInfo"]["name"], "datum-plan");
        assert!(answer(&json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }), |_, _| Ok(json!(null))).is_none());
        let list = answer(&json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" }), |_, _| Ok(json!(null))).unwrap();
        assert!(list["result"]["tools"].as_array().unwrap().len() >= 101);
        let off = answer(&json!({ "jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": { "name": "plan_summary", "arguments": {} } }), |_, _| Err("closed".into())).unwrap();
        assert_eq!(off["result"]["isError"], true);
        assert_eq!(answer(&json!({ "jsonrpc": "2.0", "id": 4, "method": "nope" }), |_, _| Ok(json!(null))).unwrap()["error"]["code"], -32601);
        // Ready-made requests, and tools marked as reading only or changing the plan.
        let pr = answer(&json!({ "jsonrpc": "2.0", "id": 5, "method": "prompts/get", "params": { "name": "plan_project", "arguments": { "goal": "a pump station" } } }), |_, _| Ok(json!(null))).unwrap();
        assert!(pr["result"]["messages"][0]["content"]["text"].as_str().unwrap().contains("a pump station"));
        assert!(answer(&json!({ "jsonrpc": "2.0", "id": 6, "method": "prompts/get", "params": { "name": "plan_project", "arguments": {} } }), |_, _| Ok(json!(null))).unwrap()["error"].is_object());
        let tools = list["result"]["tools"].as_array().unwrap();
        let ann = |n: &str| tools.iter().find(|t| t["name"] == n).unwrap()["annotations"].clone();
        assert_eq!((ann("get_tasks")["readOnlyHint"].as_bool(), ann("add_tasks")["readOnlyHint"].as_bool(), ann("delete_tasks")["destructiveHint"].as_bool()), (Some(true), Some(false), Some(true)));
    }
    #[test]
    fn claude_changes_the_open_plan_through_the_bridge() {
        // A settings folder of its own; the "window" is a plan behind the real listener.
        let dir = std::env::temp_dir().join(format!("dp-ai-{}", std::process::id()));
        std::env::set_var("XDG_CONFIG_HOME", &dir);
        std::env::set_var("HOME", &dir);
        let off = forward("plan_summary", &json!({})).unwrap_err();
        assert!(off.contains("AI Link is off"), "{off}");
        let doc = Arc::new(Mutex::new(datum_core::doc::Doc::new(datum_core::templates::build(&datum_core::templates::TEMPLATES[0], chrono::NaiveDate::from_ymd_opt(2026, 11, 2).unwrap()))));
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = l.local_addr().unwrap().port();
        let k = key();
        let f = link_file().unwrap();
        std::fs::create_dir_all(f.parent().unwrap()).unwrap();
        std::fs::write(&f, json!({ "port": port, "key": k }).to_string()).unwrap();
        let d2 = doc.clone();
        listen(l, k.clone(), Arc::new(AtomicBool::new(true)), move |n, a| assist::call(&mut d2.lock().unwrap(), "%d-%b-%y", chrono::NaiveDate::from_ymd_opt(2026, 11, 13).unwrap(), n, a));
        let n0 = doc.lock().unwrap().p.tasks.len();
        let r = forward("add_tasks", &json!({ "tasks": [{ "name": "Pump test", "dur": "3d" }] })).unwrap();
        assert_eq!(r["added"][0]["name"], "Pump test");
        assert_eq!(doc.lock().unwrap().p.tasks.len(), n0 + 1, "the plan behind the link changed");
        assert!(forward("set_task_fields", &json!({ "uid": 999999, "fields": { "dur": "1d" } })).is_err());
        // A wrong key is turned away.
        std::fs::write(&f, json!({ "port": port, "key": "not the key" }).to_string()).unwrap();
        assert!(forward("plan_summary", &json!({})).unwrap_err().contains("key does not match"));
        let _ = std::fs::remove_dir_all(&dir);
    }
    #[test]
    fn the_extension_holds_a_manifest_the_icon_and_the_bridge() {
        let dir = std::env::temp_dir().join(format!("dp-mcpb-{}", std::process::id()));
        let f = extension(&dir).unwrap();
        let mut z = zip::ZipArchive::new(std::fs::File::open(&f).unwrap()).unwrap();
        let mut m = String::new();
        z.by_name("manifest.json").unwrap().read_to_string(&mut m).unwrap();
        let m: Value = serde_json::from_str(&m).unwrap();
        let bin = m["server"]["entry_point"].as_str().unwrap().to_string();
        assert_eq!((m["name"].as_str(), m["server"]["type"].as_str()), (Some("datum-plan"), Some("binary")));
        assert_eq!(m["server"]["mcp_config"]["command"], format!("${{__dirname}}/{bin}"));
        assert_eq!(m["server"]["mcp_config"]["args"][0], "--mcp");
        assert!(m["tools"].as_array().unwrap().len() >= 50);
        assert_eq!(m["tools_generated"], true, "tools added by later versions reach the assistant");
        assert!(z.by_name("icon.png").is_ok());
        assert_eq!(z.by_name(&bin).unwrap().size(), std::fs::metadata(std::env::current_exe().unwrap()).unwrap().len());
        let _ = std::fs::remove_dir_all(&dir);
    }
    #[test]
    fn a_window_keeps_its_link_file_and_leaves_another_windows_alone() {
        let dir = std::env::temp_dir().join(format!("dp-keep-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("ai-link.json");
        let mine = json!({ "port": 1, "key": "k", "pid": std::process::id() }).to_string();
        let on = Arc::new(AtomicBool::new(true));
        keep(f.clone(), mine.clone(), on.clone());
        // Another window wrote its own file: it is not ours, and the keeper leaves it.
        std::fs::write(&f, json!({ "port": 2, "key": "x", "pid": 1 }).to_string()).unwrap();
        assert!(!ours(&f));
        std::thread::sleep(Duration::from_millis(2500));
        assert!(!ours(&f), "another window's file is left alone");
        // That window closed and removed it: this window writes its own again.
        std::fs::remove_file(&f).unwrap();
        std::thread::sleep(Duration::from_millis(2500));
        assert!(ours(&f), "the link file is back");
        on.store(false, Ordering::SeqCst);
        let _ = std::fs::remove_dir_all(&dir);
    }
    #[test]
    fn keys_differ() { assert_ne!(key(), key()); assert_eq!(key().len(), 32); }
}
