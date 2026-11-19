//! The Document Register as spreadsheet tables: `Documents[…]` (one row per document, its own
//! columns included) and `Transmittals[…]` (one row per submission), for any sheet formula, and the
//! register's own formula columns worked out per row (`=[Review due]-TODAY()`).
use crate::docreg::RegOut;
use crate::fields::Calc;
use crate::model::*;
use crate::sheet::{col_name, formula_cell, table, translate, EPOCH};
use chrono::NaiveDate;
use gridcraft_core::{CellRef, Value};
use gridcraft_model::{Cell, Sheet, Workbook};
use std::sync::Arc;

/// The Documents table's columns: key and name (dates are marked with `d:`).
pub const DOC_COLS: [(&str, &str); 26] = [
    ("number", "Document No."), ("title", "Title"), ("vendor", "Vendor"), ("package", "Package"), ("disc", "Discipline"), ("kind", "Type"),
    ("class", "Class"), ("state", "State"), ("key", "Key"), ("offset", "P offset"), ("d:first_planned", "First planned"), ("d:first_sent", "First sent"),
    ("subs", "Submissions"), ("rev", "Rev"), ("purpose", "Purpose"), ("d:sent", "Sent"), ("d:due", "Review due"), ("d:returned", "Returned"),
    ("code", "Code"), ("status", "Status"), ("earned", "Earned %"), ("planned", "Planned %"), ("client_late", "Client late"), ("d:final", "Final"),
    ("task", "Task"), ("remarks", "Remarks"),
];
/// The Transmittals table's columns.
pub const LOG_COLS: [(&str, &str); 17] = [
    ("number", "Document No."), ("title", "Title"), ("class", "Class"), ("vendor", "Vendor"), ("rev", "Rev"), ("purpose", "Purpose"),
    ("transmittal", "Transmittal"), ("d:sent", "Sent"), ("d:due", "Review due"), ("ret_transmittal", "Return transmittal"), ("d:returned", "Returned"),
    ("code", "Code"), ("crs", "Comments ref."), ("days", "Days with client"), ("late", "Days late"), ("state", "State"), ("accepted", "Accepted"),
];

fn date(d: Option<NaiveDate>) -> Value {
    d.map_or(Value::Empty, |d| Value::number((d - NaiveDate::from_ymd_opt(1970, 1, 1).unwrap_or_default()).num_days() as f64 + EPOCH))
}
fn text(s: &str) -> Value { if s.is_empty() { Value::Empty } else { Value::text(s) } }

/// One document's value in a column (its own columns by name, after the standard ones).
fn doc_value(c: &Calc, o: &RegOut, k: usize, key: &str) -> Value {
    let (r, d) = (&c.p.documents, &c.p.documents.docs[k]);
    let Some(x) = o.docs.get(k) else { return Value::Empty };
    let cls = o.classes.get((d.class.max(1) as usize).saturating_sub(1)).cloned().unwrap_or_default();
    match key.trim_start_matches("d:") {
        "number" => text(&d.number), "title" => text(&d.title), "vendor" => text(&d.vendor), "package" => text(&d.package), "disc" => text(&d.disc), "kind" => text(&d.kind),
        "class" => text(&cls), "state" => text(if d.state.is_empty() { "Active" } else { &d.state }), "key" => Value::Bool(d.key),
        "offset" => d.offset.map_or(Value::Empty, Value::number), "first_planned" => date(x.planned.first().copied().flatten()), "first_sent" => date(x.actual.first().copied().flatten()),
        "subs" => Value::number(x.subs as f64), "rev" => text(&x.rev), "purpose" => text(&x.purpose), "sent" => date(x.sent), "due" => date(x.due), "returned" => date(x.returned),
        "code" => text(&x.code), "status" => text(&x.status), "earned" => Value::number(x.earned), "planned" => Value::number(x.planned_pct),
        "client_late" => Value::number(x.client_late as f64), "final" => date(x.finish), "task" => text(&x.task_name), "remarks" => text(&d.remarks),
        own => {
            let col = r.columns.iter().find(|c| c.name == own);
            let v = d.own.get(own).map(String::as_str).unwrap_or("");
            match col.map(|c| c.kind.as_str()) {
                Some("number") => v.trim().parse::<f64>().map_or(text(v), Value::number),
                Some("date") => NaiveDate::parse_from_str(v.trim(), "%Y-%m-%d").map_or(text(v), |d| date(Some(d))),
                _ => text(v),
            }
        }
    }
}

/// The Documents sheet: the standard columns, then the register's own (formula columns as worked
/// out in `o`, or left empty when `o` has none yet).
fn doc_sheet(c: &Calc, o: &RegOut, name: &str) -> (Sheet, Vec<String>) {
    let r = &c.p.documents;
    let mut sh = Sheet::new(name);
    let mut cols: Vec<(String, String)> = DOC_COLS.iter().map(|(k, n)| (k.to_string(), n.to_string())).collect();
    cols.extend(r.columns.iter().map(|x| (x.name.clone(), x.name.clone())));
    for (j, (_, n)) in cols.iter().enumerate() { sh.set_value(CellRef::new(0, j as u32), Value::text(n.as_str())) }
    for k in 0..r.docs.len() {
        for (j, (key, _)) in cols.iter().enumerate() {
            let own = r.columns.iter().position(|x| &x.name == key);
            let v = match own.filter(|&i| r.columns[i].kind == "formula") {
                Some(i) => o.docs.get(k).and_then(|x| x.own.get(i)).map_or(Value::Empty, |s| s.parse::<f64>().map_or_else(|_| text(s), Value::number)),
                None => doc_value(c, o, k, key),
            };
            sh.set_value(CellRef::new(k as u32 + 1, j as u32), v);
        }
    }
    (sh, cols.into_iter().map(|x| x.1).collect())
}

/// The Transmittals sheet: every submission with its document's number, title, class and vendor.
fn log_sheet(c: &Calc, o: &RegOut, name: &str) -> Sheet {
    let r = &c.p.documents;
    let mut sh = Sheet::new(name);
    for (j, (_, n)) in LOG_COLS.iter().enumerate() { sh.set_value(CellRef::new(0, j as u32), Value::text(*n)) }
    for (i, s) in r.log.iter().enumerate() {
        let k = r.docs.iter().position(|d| d.id == s.doc);
        let x = o.subs.iter().find(|x| x.id == s.id);
        for (j, (key, _)) in LOG_COLS.iter().enumerate() {
            let v = match key.trim_start_matches("d:") {
                "number" | "title" | "vendor" | "class" => k.map_or(Value::Empty, |k| doc_value(c, o, k, key)),
                "rev" => text(&s.rev), "purpose" => text(&s.purpose), "transmittal" => text(&s.transmittal), "sent" => date(s.sent),
                "due" => date(x.and_then(|x| x.due)), "ret_transmittal" => text(&s.ret_transmittal), "returned" => date(s.returned), "code" => text(&s.code), "crs" => text(&s.crs),
                "days" => x.map_or(Value::Empty, |x| Value::number(x.days as f64)), "late" => x.map_or(Value::Empty, |x| Value::number(x.late as f64)),
                "state" => x.map_or(Value::Empty, |x| text(&x.state)), _ => Value::Bool(x.is_some_and(|x| x.accepted)),
            };
            sh.set_value(CellRef::new(i as u32 + 1, j as u32), v);
        }
    }
    sh
}

/// Adds the Documents and Transmittals sheets and tables to a workbook (when the plan has a register).
pub fn add_tables(c: &Calc, wb: &mut Workbook) {
    let r = &c.p.documents;
    if r.docs.is_empty() { return }
    let o = crate::docreg::status(c);
    let (sh, cols) = doc_sheet(c, &o, "_Documents");
    let mut sh = sh;
    sh.tables.push(table(3, "Documents", cols, r.docs.len() as u32));
    wb.sheets.push(Arc::new(sh));
    let mut lg = log_sheet(c, &o, "_Transmittals");
    lg.tables.push(table(4, "Transmittals", LOG_COLS.iter().map(|x| x.1.to_string()).collect(), r.log.len() as u32));
    wb.sheets.push(Arc::new(lg));
}

/// The register's own formula columns, per document: what each shows. `[Column]` reads that
/// column of the same document; a result that moves with the dates it reads is shown as a date.
pub fn own_formulas(c: &Calc, o: &RegOut) -> Vec<Vec<String>> {
    let r = &c.p.documents;
    let f: Vec<(usize, &OwnCol)> = r.columns.iter().enumerate().filter(|(_, x)| x.kind == "formula").collect();
    let mut out = vec![vec![String::new(); r.columns.len()]; r.docs.len()];
    if f.is_empty() || r.docs.is_empty() { return out }
    // Twice: as it is, and with every date column 1000 days later; a result 1000 days later is a date.
    let run = |shift: f64| -> Vec<Vec<Value>> {
        let (mut sh, names) = doc_sheet(c, o, "D");
        if shift != 0.0 {
            for (j, (key, _)) in DOC_COLS.iter().enumerate() {
                if !key.starts_with("d:") { continue }
                for k in 0..r.docs.len() {
                    let at = CellRef::new(k as u32 + 1, j as u32);
                    if let Value::Number(n) = sh.value(at) { sh.set_value(at, Value::number(n + shift)) }
                }
            }
            for (j, _) in r.columns.iter().enumerate().filter(|(_, x)| x.kind == "date") {
                let col = (DOC_COLS.len() + j) as u32;
                for k in 0..r.docs.len() {
                    let at = CellRef::new(k as u32 + 1, col);
                    if let Value::Number(n) = sh.value(at) { sh.set_value(at, Value::number(n + shift)) }
                }
            }
        }
        for &(i, x) in &f {
            let col = (DOC_COLS.len() + i) as u32;
            let t = translate(&x.formula, |name| names.iter().position(|n| n.eq_ignore_ascii_case(name.trim())).map(|j| format!("{}{{ROW}}", col_name(j as u32))));
            for k in 0..r.docs.len() {
                let cell = match &t { Ok(t) => formula_cell(&format!("={}", t.replace("{ROW}", &(k + 2).to_string())), &c.fmt), Err(_) => Cell::value(Value::Error(gridcraft_core::CellError::Name)) };
                sh.set_cell(CellRef::new(k as u32 + 1, col), cell);
            }
        }
        let mut wb = Workbook::new();
        wb.sheets[0] = Arc::new(sh);
        gridcraft_calc::Calc::new().recalc_all(&mut wb);
        let sh = &wb.sheets[0];
        (0..r.docs.len()).map(|k| f.iter().map(|&(i, _)| sh.value(CellRef::new(k as u32 + 1, (DOC_COLS.len() + i) as u32))).collect()).collect()
    };
    let (a, b) = (run(0.0), run(1000.0));
    for k in 0..r.docs.len() {
        for (n, &(i, _)) in f.iter().enumerate() {
            out[k][i] = match (&a[k][n], &b[k][n]) {
                (Value::Number(x), Value::Number(y)) if (y - x - 1000.0).abs() < 1e-6 => {
                    let d = NaiveDate::from_ymd_opt(1970, 1, 1).unwrap_or_default() + chrono::Duration::days((x - EPOCH).floor() as i64);
                    d.format(&c.fmt).to_string()
                }
                (Value::Number(x), _) => { let s = format!("{:.4}", x); s.trim_end_matches('0').trim_end_matches('.').to_string() }
                (v, _) => v.display(),
            };
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc::{Doc, Op};

    #[test]
    fn documents_and_transmittals_in_sheet_formulas_and_own_columns() {
        let d = |m, day| NaiveDate::from_ymd_opt(2026, m, day).unwrap();
        let mut doc = Doc::new(crate::templates::build(&crate::templates::TEMPLATES[0], d(11, 2)));
        let mut r = doc.p.documents.clone();
        let vd = |id: u32, n: &str, v: &str, off: f64| VDoc { id, number: n.into(), title: format!("Doc {n}"), vendor: v.into(), class: 1, offset: Some(off), weight: 1.0, progress: true, ..Default::default() };
        r.docs = vec![vd(1, "D-1", "North", 1.0), vd(2, "D-2", "North", 2.0), vd(3, "D-3", "South", 3.0)];
        r.docs[2].state = "Deleted".into();
        r.log = vec![Submission { id: 1, doc: 1, rev: "A".into(), purpose: "ICR".into(), sent: Some(d(11, 9)), returned: Some(d(11, 20)), code: "Code 3".into(), ..Default::default() }];
        r.columns = vec![OwnCol { name: "Area".into(), kind: "text".into(), formula: String::new() }, OwnCol { name: "Chase on".into(), kind: "formula".into(), formula: "=[Review due]+2".into() },
            OwnCol { name: "Left".into(), kind: "formula".into(), formula: "=[Review due]-[Sent]".into() }];
        r.docs[0].own.insert("Area".into(), "Unit 1".into());
        doc.apply(Op::Documents { r: Box::new(r) }, "%Y-%m-%d").unwrap();
        let c = Calc::new(&doc.p, "%Y-%m-%d", d(11, 25));
        let o = crate::docreg::status(&c);
        assert_eq!(o.docs[0].own, ["Unit 1", "2026-11-25", "14"], "a date column plus days is a date; a date minus a date is days");
        assert_eq!(o.docs[2].status, "Not counted", "a deleted document is left out");
        let mut sd = SheetData { name: "S".into(), ..Default::default() };
        for (a, f) in [("A1", "=COUNTIFS(Documents[Vendor],\"North\")"), ("A2", "=COUNTIF(Transmittals[Code],\"Code 3\")"), ("A3", "=INDEX(Documents[Area],1)"), ("A4", "=SUM(Documents[Submissions])")] { sd.cells.insert(a.into(), f.into()); }
        doc.p.sheets.push(sd);
        let c = Calc::new(&doc.p, "%Y-%m-%d", d(11, 25));
        let v = crate::sheet::sheet_view(&c, doc.p.sheets.len() - 1).unwrap();
        let at = |a: &str| v.cells.iter().find(|x| x.0 == a).map(|x| x.2.clone()).unwrap_or_default();
        assert_eq!((at("A1"), at("A2"), at("A3"), at("A4")), ("2".into(), "1".into(), "Unit 1".into(), "1".into()));
    }
}
