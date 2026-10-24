//! Text in and out of grid cells: durations, dates and predecessor lists.
use crate::model::{Link, LinkType};
use chrono::NaiveDate;
use std::collections::HashMap;

/// "5", "5d", "2 wks", "1.5w", "1 mo" → working days.
pub fn dur(s: &str) -> Result<i32, String> {
    let s = s.trim().to_lowercase();
    let cut = s.find(|c: char| !(c.is_ascii_digit() || c == '.')).unwrap_or(s.len());
    let (num, unit) = s.split_at(cut);
    let v: f64 = num.parse().map_err(|_| format!("\"{s}\" is not a duration. Try 5d, 2w or 1mo."))?;
    let m = match unit.trim().trim_end_matches('?') {
        "" | "d" | "day" | "days" | "dy" | "dys" => 1.0,
        "w" | "wk" | "wks" | "week" | "weeks" => 5.0,
        "mo" | "mon" | "mons" | "month" | "months" => 20.0,
        u => return Err(format!("Unknown duration unit \"{u}\". Use d, w or mo.")),
    };
    let d = (v * m).round();
    if !(0.0..=100_000.0).contains(&d) {
        return Err("Duration is out of range.".into());
    }
    Ok(d as i32)
}

pub fn fmt_dur(d: i64) -> String {
    if d == 1 { "1 day".into() } else { format!("{d} days") }
}

const DATE_FORMATS: [&str; 7] = ["%Y-%m-%d", "%d-%b-%y", "%d-%b-%Y", "%d/%m/%Y", "%d/%m/%y", "%d %b %Y", "%d %b %y"];

pub fn date(s: &str) -> Result<NaiveDate, String> {
    let s = s.trim();
    DATE_FORMATS
        .iter()
        .find_map(|f| NaiveDate::parse_from_str(s, f).ok())
        .ok_or_else(|| format!("\"{s}\" is not a date. Try 24-Oct-26 or 2026-10-24."))
}

pub fn fmt_date(d: NaiveDate) -> String {
    d.format("%d-%b-%y").to_string()
}

/// "3, 5SS+2d, 7FF-1" with row IDs (1-based) → links. `ids[i]` is the uid of row i+1.
pub fn preds(s: &str, ids: &[u32]) -> Result<Vec<Link>, String> {
    let mut out = vec![];
    for tok in s.split([',', ';']).map(str::trim).filter(|t| !t.is_empty()) {
        let t = tok.to_uppercase().replace(' ', "");
        let cut = t.find(|c: char| !c.is_ascii_digit()).unwrap_or(t.len());
        let id: usize = t[..cut].parse().map_err(|_| format!("\"{tok}\" should start with a task ID, like 3 or 3SS+2d."))?;
        let uid = *ids.get(id.wrapping_sub(1)).ok_or_else(|| format!("There is no task {id}."))?;
        let mut rest = &t[cut..];
        let kind = match rest.get(..2) {
            Some("FS") => LinkType::FS,
            Some("SS") => LinkType::SS,
            Some("FF") => LinkType::FF,
            Some("SF") => LinkType::SF,
            _ => LinkType::FS,
        };
        if matches!(rest.get(..2), Some("FS" | "SS" | "FF" | "SF")) {
            rest = &rest[2..];
        }
        let lag = match rest.chars().next() {
            None => 0,
            Some('+') => dur(&rest[1..])?,
            Some('-') => -dur(&rest[1..])?,
            _ => return Err(format!("Write the lag in \"{tok}\" as +2d or -1d.")),
        };
        out.push(Link { pred: uid, kind, lag });
    }
    Ok(out)
}

pub fn fmt_preds(links: &[Link], id_of: &HashMap<u32, usize>) -> String {
    links
        .iter()
        .filter_map(|l| {
            let id = id_of.get(&l.pred)?;
            let k = if l.kind == LinkType::FS && l.lag == 0 { String::new() } else { format!("{:?}", l.kind) };
            let lag = match l.lag {
                0 => String::new(),
                v if v > 0 => format!("+{v}d"),
                v => format!("{v}d"),
            };
            Some(format!("{id}{k}{lag}"))
        })
        .collect::<Vec<_>>()
        .join(", ")
}
