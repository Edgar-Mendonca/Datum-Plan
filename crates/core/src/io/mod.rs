//! File exchange: MS Project XML, Primavera XER and PMXML, Excel, CSV, MPX, JSON, HTML and iCalendar.
pub mod formats;
pub mod mspdi;
pub mod table;
pub mod xer;
pub(crate) mod xml;

use crate::{fields::Calc, load, model::Project, sched};
use chrono::NaiveDate;
use std::path::Path;

pub enum Imported {
    Plan(Box<Project>),
    /// A spreadsheet that needs its columns mapped first.
    Table(table::Table),
}

fn ext(path: &Path) -> String {
    path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase()
}

pub fn import(path: &Path) -> Result<Imported, String> {
    let read = || std::fs::read(path).map(|b| String::from_utf8_lossy(&b).into_owned()).map_err(|e| format!("Could not open {}: {e}", path.display()));
    match ext(path).as_str() {
        "xml" => mspdi::read(&read()?).map(|p| Imported::Plan(Box::new(p))),
        "xer" => xer::read(&read()?).map(|p| Imported::Plan(Box::new(p))),
        "json" | "dplan" => Project::from_json(&read()?).map(|p| Imported::Plan(Box::new(p))),
        "csv" | "txt" | "xlsx" | "xlsm" | "xls" | "ods" => table::read(path).map(Imported::Table),
        "mpp" => Err("MS Project .mpp files are a closed format. In MS Project, use File › Save As › XML, then import the XML file.".into()),
        e => Err(format!("Datum Plan cannot import .{e} files. Use XML, XER, XLSX, CSV or JSON.")),
    }
}

/// Writes the plan to `path` in the format its extension names. `cols` chooses the columns of
/// table-like formats (CSV, Excel, JSON, HTML); empty means the standard set.
pub fn export(p: &Project, path: &Path, fmt: &str, today: NaiveDate, cols: &[String]) -> Result<(), String> {
    let s = sched::schedule(p)?;
    let write = |text: String| std::fs::write(path, text).map_err(|e| format!("Could not save {}: {e}", path.display()));
    let std_cols: Vec<String> = table::OUT.iter().map(|c| c.to_string()).collect();
    let cols = if cols.is_empty() { &std_cols[..] } else { cols };
    let calc = || Calc::new(p, fmt, today);
    match ext(path).as_str() {
        "xml" => write(mspdi::write(p, &s, &load::compute(p, &s).cost)),
        "xer" => write(xer::write(p, &s, today)),
        "csv" => table::write_csv(&calc(), cols, path),
        "xlsx" => table::write_xlsx(&calc(), cols, path),
        "json" => write(formats::json(&calc(), cols)),
        "html" | "htm" => write(formats::html(&calc(), cols)),
        "ics" => write(formats::ics(&calc())),
        "mpx" => write(formats::mpx(&calc())),
        "pmxml" => write(formats::pmxml(&calc())),
        e => Err(format!("Datum Plan cannot export .{e} files.")),
    }
}
