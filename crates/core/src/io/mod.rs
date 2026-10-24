//! File exchange: MS Project XML, Primavera XER, Excel and CSV.
pub mod mspdi;
pub mod table;
pub mod xer;
mod xml;

use crate::{load, model::Project, sched, view};
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
        "csv" | "txt" | "xlsx" | "xlsm" | "xls" | "ods" => table::read(path).map(Imported::Table),
        "mpp" => Err("MS Project .mpp files are a closed format. In MS Project, use File › Save As › XML, then import the XML file.".into()),
        e => Err(format!("Datum Plan cannot import .{e} files. Use XML, XER, XLSX or CSV.")),
    }
}

pub fn export(p: &Project, path: &Path, fmt: &str, today: NaiveDate) -> Result<(), String> {
    let s = sched::schedule(p)?;
    let write = |text: String| std::fs::write(path, text).map_err(|e| format!("Could not save {}: {e}", path.display()));
    match ext(path).as_str() {
        "xml" => write(mspdi::write(p, &s, &load::compute(p, &s).cost)),
        "xer" => write(xer::write(p, &s, today)),
        "csv" => table::write_csv(&view::build(p, fmt, today), path),
        "xlsx" => table::write_xlsx(&view::build(p, fmt, today), path),
        e => Err(format!("Datum Plan cannot export .{e} files.")),
    }
}
