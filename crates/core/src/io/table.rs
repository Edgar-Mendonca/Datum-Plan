//! Task lists in spreadsheets: CSV and Excel in, with a column mapping step; CSV and Excel out.
use crate::{doc, model::Project, view::View};
use calamine::{open_workbook_auto, Data, Reader};
use chrono::NaiveDate;
use serde::Serialize;
use std::path::Path;

#[derive(Serialize)]
pub struct Table {
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
    /// The field each column most likely holds ("" = skip).
    pub guess: Vec<String>,
}

/// Fields a column can map to: key, label, header words that suggest it.
pub const FIELDS: [(&str, &str, &[&str]); 15] = [
    ("id", "ID (for predecessors)", &["id", "unique id", "#"]),
    ("level", "Outline level", &["outline level", "level", "indent"]),
    ("wbs", "WBS / outline number", &["wbs", "outline number"]),
    ("code", "Activity ID", &["activity id", "task code", "code"]),
    ("name", "Task name", &["name", "task", "activity", "description", "title"]),
    ("dur", "Duration", &["duration", "dur", "days"]),
    ("start", "Start", &["start", "begin"]),
    ("finish", "Finish", &["finish", "end", "due"]),
    ("preds", "Predecessors", &["predecessor", "depends", "after"]),
    ("res", "Resources", &["resource", "assigned", "owner", "who"]),
    ("pct", "% complete", &["%", "complete", "progress", "done"]),
    ("deadline", "Deadline", &["deadline"]),
    ("astart", "Actual start", &["actual start"]),
    ("afinish", "Actual finish", &["actual finish"]),
    ("notes", "Notes", &["note", "comment", "remark"]),
];

fn guess(h: &str) -> String {
    let h = h.trim().to_lowercase();
    // Exact matches first, then the longest word that appears in the header.
    FIELDS.iter().find(|(k, l, _)| h == *k || h == l.to_lowercase()).or_else(|| {
        FIELDS.iter().filter_map(|f| f.2.iter().filter(|w| h.contains(*w)).map(|w| w.len()).max().map(|n| (f, n))).max_by_key(|x| x.1).map(|x| x.0)
    }).map(|f| f.0.to_string()).unwrap_or_default()
}

fn csv_rows(text: &str) -> Vec<Vec<String>> {
    let first = text.lines().next().unwrap_or("");
    let sep = [',', ';', '\t'].into_iter().max_by_key(|c| first.matches(*c).count()).unwrap_or(',');
    let (mut rows, mut row, mut cell, mut q) = (vec![], vec![], String::new(), false);
    let mut it = text.trim_start_matches('\u{feff}').chars().peekable();
    while let Some(c) = it.next() {
        match c {
            '"' if q && it.peek() == Some(&'"') => { cell.push('"'); it.next(); }
            '"' => q = !q,
            c if c == sep && !q => row.push(std::mem::take(&mut cell)),
            '\n' if !q => { row.push(std::mem::take(&mut cell)); rows.push(std::mem::take(&mut row)); }
            '\r' if !q => {}
            c => cell.push(c),
        }
    }
    if !cell.is_empty() || !row.is_empty() {
        row.push(cell);
        rows.push(row);
    }
    rows
}

fn cell(d: &Data) -> String {
    match d {
        Data::Empty => String::new(),
        Data::String(s) | Data::DateTimeIso(s) | Data::DurationIso(s) => s.clone(),
        Data::Float(f) if f.fract() == 0.0 => format!("{}", *f as i64),
        Data::Float(f) => f.to_string(),
        Data::Int(i) => i.to_string(),
        Data::Bool(b) => b.to_string(),
        Data::DateTime(x) => {
            let v = x.as_f64();
            (NaiveDate::from_ymd_opt(1899, 12, 30).unwrap() + chrono::Days::new(v.max(0.0) as u64)).to_string()
        }
        Data::Error(_) => String::new(),
    }
}

pub fn read(path: &Path) -> Result<Table, String> {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    let mut rows = if ext == "csv" || ext == "txt" {
        let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
        csv_rows(&String::from_utf8_lossy(&bytes))
    } else {
        let mut wb = open_workbook_auto(path).map_err(|e| format!("Could not read the workbook: {e}"))?;
        let range = wb.worksheet_range_at(0).ok_or("The workbook has no sheets.")?.map_err(|e| e.to_string())?;
        range.rows().map(|r| r.iter().map(cell).collect()).collect()
    };
    rows.retain(|r: &Vec<String>| r.iter().any(|c| !c.trim().is_empty()));
    if rows.is_empty() {
        return Err("The file has no rows.".into());
    }
    let headers: Vec<String> = rows.remove(0).into_iter().map(|h| h.trim().to_string()).collect();
    let guess = headers.iter().map(|h| guess(h)).collect();
    Ok(Table { headers, rows, guess })
}

/// Builds a plan from table rows, `map[c]` naming the field of column c. Returns the plan and
/// how many cells could not be used.
pub fn to_project(t: &Table, map: &[String], name: &str, today: NaiveDate, fmt: &str) -> Result<(Project, usize), String> {
    let col = |f: &str| map.iter().position(|m| m == f);
    let name_col = col("name").ok_or("Choose which column holds the task names.")?;
    let rows: Vec<&Vec<String>> = t.rows.iter().filter(|r| r.get(name_col).is_some_and(|c| !c.trim().is_empty())).collect();
    let get = |r: &Vec<String>, f: &str| col(f).and_then(|c| r.get(c)).map(|s| s.trim().to_string()).unwrap_or_default();
    let start = rows.iter().filter_map(|r| crate::parse::date_in(&get(r, "start"), fmt).ok()).min().unwrap_or(today);
    let mut p = Project::new(name, start);
    for r in &rows {
        let level = if col("level").is_some() {
            get(r, "level").parse::<i32>().map(|l| (l - 1).max(0)).unwrap_or(0)
        } else if col("wbs").is_some() {
            get(r, "wbs").matches('.').count() as i32
        } else { 0 };
        let task = p.new_task("", level.min(30) as u8, p.day_min);
        p.tasks.push(task);
    }
    p.normalize();
    // Predecessors refer to source IDs (or row numbers when there is no ID column).
    let ids: Vec<String> = rows.iter().enumerate().map(|(k, r)| if col("id").is_some() { get(r, "id") } else { (k + 1).to_string() }).collect();
    let mut bad = 0;
    for (i, r) in rows.iter().enumerate() {
        let has_preds = !get(r, "preds").is_empty();
        for f in ["name", "code", "dur", "start", "finish", "res", "pct", "deadline", "astart", "afinish", "notes"] {
            let v = get(r, f);
            if v.is_empty() || (f == "start" && has_preds) || (f == "finish" && col("dur").is_some()) {
                continue;
            }
            if doc::set(&mut p, i, f, &v, fmt).is_err() && !p.is_summary(i) {
                bad += 1;
            }
        }
    }
    for (i, r) in rows.iter().enumerate() {
        let v = get(r, "preds");
        if v.is_empty() || p.is_summary(i) {
            continue;
        }
        let mapped: Vec<String> = v.split([',', ';']).filter_map(|tok| {
            let tok = tok.trim();
            let cut = tok.find(|c: char| !c.is_ascii_digit()).unwrap_or(tok.len());
            ids.iter().position(|x| *x == tok[..cut]).map(|k| format!("{}{}", k + 1, &tok[cut..]))
        }).collect();
        if doc::set(&mut p, i, "preds", &mapped.join(","), fmt).is_err() {
            bad += 1;
        }
    }
    p.normalize();
    if crate::sched::schedule(&p).is_err() {
        p.tasks.iter_mut().for_each(|t| t.preds.clear());
        bad += 1;
    }
    Ok((p, bad))
}

const OUT: [(&str, &str); 13] = [
    ("ID", "id"), ("WBS", "wbs"), ("Outline Level", "lvl"), ("Activity ID", "code"), ("Task Name", "name"), ("Duration", "dur"), ("Start", "start"),
    ("Finish", "finish"), ("Predecessors", "preds"), ("Resource Names", "res"), ("% Complete", "pct"), ("Total Float", "tf"), ("Cost", "cost"),
];

fn row_values(v: &View) -> Vec<Vec<String>> {
    v.rows.iter().map(|r| {
        let j = serde_json::to_value(r).unwrap_or_default();
        OUT.iter().map(|(_, k)| match k { &"lvl" => (r.lvl + 1).to_string(), k => match &j[*k] { serde_json::Value::String(s) => s.clone(), x => x.to_string() } }).collect()
    }).collect()
}

pub fn write_csv(v: &View, path: &Path) -> Result<(), String> {
    let q = |s: &str| if s.contains([',', '"', '\n']) { format!("\"{}\"", s.replace('"', "\"\"")) } else { s.to_string() };
    let mut out = String::from("\u{feff}");
    out.push_str(&OUT.iter().map(|h| h.0).collect::<Vec<_>>().join(","));
    out.push('\n');
    for r in row_values(v) {
        out.push_str(&r.iter().map(|c| q(c)).collect::<Vec<_>>().join(","));
        out.push('\n');
    }
    std::fs::write(path, out).map_err(|e| format!("Could not save {}: {e}", path.display()))
}

pub fn write_xlsx(v: &View, path: &Path) -> Result<(), String> {
    use rust_xlsxwriter::{Format, Workbook};
    let mut wb = Workbook::new();
    let ws = wb.add_worksheet();
    let e = |x: rust_xlsxwriter::XlsxError| x.to_string();
    ws.set_name("Tasks").map_err(e)?;
    let head = Format::new().set_bold().set_background_color(0xE6F0FC);
    for (c, (h, _)) in OUT.iter().enumerate() {
        ws.write_string_with_format(0, c as u16, *h, &head).map_err(e)?;
        ws.set_column_width(c as u16, if *h == "Task Name" { 44.0 } else if *h == "Resource Names" { 28.0 } else { 13.0 }).map_err(e)?;
    }
    for (k, (r, vals)) in v.rows.iter().zip(row_values(v)).enumerate() {
        let mut f = Format::new();
        if r.sum {
            f = f.set_bold();
        }
        for (c, val) in vals.iter().enumerate() {
            let fmt = if OUT[c].1 == "name" { f.clone().set_indent(r.lvl) } else { f.clone() };
            match (OUT[c].1, val.parse::<f64>()) {
                ("id" | "lvl" | "pct", Ok(n)) => ws.write_number_with_format(k as u32 + 1, c as u16, n, &fmt).map(|_| ()),
                _ => ws.write_string_with_format(k as u32 + 1, c as u16, val, &fmt).map(|_| ()),
            }.map_err(e)?;
        }
    }
    ws.set_freeze_panes(1, 0).map_err(e)?;
    wb.save(path).map_err(|x| format!("Could not save {}: {x}", path.display()))
}
