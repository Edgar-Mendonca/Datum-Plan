//! A local server around the real engine for the live Sheets tests (tests_ui/sheets_live.cjs): the
//! built window runs in Chromium and sends its sheet commands here, so what it shows is what the
//! engine works out. It answers POST {"c": command, "a": arguments} with JSON, on 127.0.0.1.
//! Run: cargo run -p datum-core --example sheetbridge  (prints the port, then serves until killed)
use datum_core::{doc::{Doc, Op}, fields::Calc, *};
use chrono::NaiveDate;
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Read, Write};

const FMT: &str = "%d-%b-%y";

fn plan() -> Project {
    let mut p = templates::build(&templates::TEMPLATES[2], NaiveDate::from_ymd_opt(2026, 10, 26).unwrap());
    p.sheets.push(SheetData { name: "Sheet1".into(), ..Default::default() });
    p.sheets.push(SheetData { name: "Sheet2".into(), ..Default::default() });
    p
}

fn answer(doc: &mut Doc, c: &str, a: &Value) -> Result<Value, String> {
    let today = NaiveDate::from_ymd_opt(2026, 11, 10).unwrap();
    let state = |d: &Doc| json!({ "project": d.p, "undo": d.can_undo(), "redo": d.can_redo() });
    Ok(match c {
        "apply" => { let op: Op = serde_json::from_value(a["op"].clone()).map_err(|e| e.to_string())?; doc.apply(op, FMT)?; state(doc) }
        "history" => { if a["redo"].as_bool() == Some(true) { doc.redo() } else { doc.undo() } state(doc) }
        "reset" => { *doc = Doc::new(plan()); state(doc) }
        "state" => state(doc),
        "sheet_view" => json!(sheet::sheet_view(&Calc::new(&doc.p, FMT, today), a["k"].as_u64().unwrap_or(0) as usize)),
        "sheet_check" => json!(sheet::check_input(&Calc::new(&doc.p, FMT, today), a["k"].as_u64().unwrap_or(0) as usize, a["cell"].as_str().unwrap_or(""), a["input"].as_str().unwrap_or(""))),
        "formula_shift" => json!(a["items"].as_array().map(|v| v.iter().map(|x| sheet::shift(x[0].as_str().unwrap_or(""), x[1].as_i64().unwrap_or(0), x[2].as_i64().unwrap_or(0))).collect::<Vec<_>>()).unwrap_or_default()),
        "sheet_catalog" => {
            let calc = Calc::new(&doc.p, FMT, today);
            json!({ "functions": sheet::functions(), "names": sheet::project_names(&calc).into_iter().map(|(n, _, d)| (n.to_string(), d.to_string())).collect::<Vec<_>>(), "tasks": sheet::task_columns(&calc).into_iter().map(|x| x.1).collect::<Vec<_>>(), "resources": sheet::RES_COLS.iter().map(|x| x.1).collect::<Vec<_>>() })
        }
        _ => return Err(format!("not bridged: {c}")),
    })
}

fn main() {
    let l = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    println!("{}", l.local_addr().unwrap().port());
    std::io::stdout().flush().ok();
    let mut doc = Doc::new(plan());
    for s in l.incoming().flatten() {
        let mut r = BufReader::new(s.try_clone().unwrap());
        let (mut line, mut len, mut method) = (String::new(), 0usize, String::new());
        if r.read_line(&mut line).is_err() { continue }
        method.push_str(line.split(' ').next().unwrap_or(""));
        loop {
            line.clear();
            if r.read_line(&mut line).unwrap_or(0) == 0 || line == "\r\n" { break }
            if let Some(v) = line.to_ascii_lowercase().strip_prefix("content-length:") { len = v.trim().parse().unwrap_or(0) }
        }
        let mut body = vec![0; len];
        r.read_exact(&mut body).ok();
        let out = if method == "OPTIONS" { String::new() } else {
            let q: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
            match answer(&mut doc, q["c"].as_str().unwrap_or(""), &q["a"]) { Ok(v) => json!({ "ok": v }).to_string(), Err(e) => json!({ "err": e }).to_string() }
        };
        let mut w = s;
        let _ = write!(w, "HTTP/1.1 200 OK\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Headers: content-type\r\nAccess-Control-Allow-Methods: POST, OPTIONS\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{out}", out.len());
    }
}
