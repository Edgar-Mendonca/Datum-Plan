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
use super::{guard, out, regroup, show_tab, tab_list, today, St};
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
    datum_core::doc::write_atomic(&f, json!({ "port": port, "key": k, "pid": std::process::id() }).to_string().as_bytes())?;
    let on = Arc::new(AtomicBool::new(true));
    let app2 = app.clone();
    listen(l, k, on.clone(), move |n, a| run(&app2, n, a));
    *g = Some(Link { port, on });
    Ok(())
}

/// Switches the link off: forgets the key, removes the file and stops listening.
pub fn stop() {
    if let Some(l) = LINK.lock().ok().and_then(|mut g| g.take()) {
        l.on.store(false, Ordering::SeqCst);
        let _ = TcpStream::connect(("127.0.0.1", l.port)); // wakes the listener so it ends
    }
    if let Some(f) = link_file() { let _ = std::fs::remove_file(f); }
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
            "save_plan" => {
                let p = ap.doc.path.clone().filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("dplan"))).ok_or("This plan has no .dplan file yet: ask the user to save it once (File › Save).")?;
                ap.doc.save(&p)?;
                Ok(json!({ "saved": p.display().to_string() }))
            }
            _ => {
                let fmt = ap.opts.date_fmt.clone();
                let r = assist::call(&mut ap.doc, &fmt, today(), name, a);
                if r.is_ok() && assist::changes(name) { regroup(&mut ap) }
                r
            }
        });
        let shown = (r.is_ok() && (assist::changes(name) || matches!(name, "switch_plan" | "save_plan"))).then(|| out(&ap));
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
    const OFF: &str = "Datum Plan is not open, or its AI Link is off. Ask the user to open Datum Plan and click Home › AI Link.";
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
        assert!(list["result"]["tools"].as_array().unwrap().len() >= 76);
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
        assert!(z.by_name("icon.png").is_ok());
        assert_eq!(z.by_name(&bin).unwrap().size(), std::fs::metadata(std::env::current_exe().unwrap()).unwrap().len());
        let _ = std::fs::remove_dir_all(&dir);
    }
    #[test]
    fn keys_differ() { assert_ne!(key(), key()); assert_eq!(key().len(), 32); }
}
