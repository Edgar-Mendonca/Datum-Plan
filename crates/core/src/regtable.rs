//! The registers as spreadsheet tables, for any sheet formula: `Documents[…]` (one row per vendor
//! document) and `Transmittals[…]` (one row per submission) from the Document Register, `Orders[…]`
//! (one row per order) and `Expediting[…]` (one row per note) from the Procurement Register, the
//! registers' own columns included; their own formula columns worked out per row
//! (`=[Review due]-TODAY()`); and the tidying both registers share (lists, own columns, words).
use crate::docreg::RegOut;
use crate::fields::Calc;
use crate::model::*;
use crate::procure::ProcOut;
use crate::sheet::{col_name, formula_cell, table, translate, EPOCH};
use chrono::NaiveDate;
use gridcraft_core::{CellRef, Value};
use gridcraft_model::{Cell, Sheet, Workbook};
use std::collections::BTreeMap;
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
/// The Orders table's columns: the order, each milestone planned and actual, and what the app works out.
pub const ORDER_COLS: [(&str, &str); 35] = [
    ("tag", "Item"), ("name", "Order"), ("vendor", "Sub-vendor"), ("origin", "Origin"), ("po_no", "PO No."), ("value", "Value"), ("state", "State"), ("long_lead", "Long lead"),
    ("d:p0", "RFQ planned"), ("d:a0", "RFQ issued"), ("d:p1", "PO planned"), ("d:a1", "PO placed"), ("d:p2", "Mfg start planned"), ("d:a2", "Mfg start"), ("mfg", "Mfg %"),
    ("d:p3", "IRN planned"), ("d:a3", "IRN"), ("d:p4", "Delivery planned"), ("d:a4", "Delivered"), ("d:promised", "Promised"), ("d:forecast", "Forecast"), ("var", "Variance"),
    ("d:required", "Required"), ("float", "Float"), ("rag", "RAG"), ("status", "Status"), ("next", "Next"), ("d:next_date", "Next due"), ("days_late", "Days late"),
    ("earned", "Earned %"), ("planned", "Planned %"), ("task", "Task"), ("need", "Needed by"), ("docs", "Documents accepted"), ("remarks", "Remarks"),
];
/// The Expediting table's columns.
pub const NOTE_COLS: [(&str, &str); 5] = [("tag", "Item"), ("name", "Order"), ("d:date", "Date"), ("note", "Note"), ("by", "By")];
/// The Contract Register's tables: name, what it holds, its columns (dates marked `d:`).
/// A table: its name, what it holds, its columns (key, name).
pub type RegTable = (&'static str, &'static str, &'static [(&'static str, &'static str)]);
pub const CONTRACT_TABLES: [RegTable; 5] = [
    ("Variations", "Every variation of the Contract Register", &[("no", "VO No."), ("desc", "Description"), ("raised_by", "Raised by"), ("status", "Status"), ("d:submitted", "Submitted"), ("d:decided", "Decided"),
        ("claimed", "Claimed"), ("approved", "Approved"), ("days_claimed", "Days claimed"), ("days_granted", "Days granted"), ("delay_event", "Delay event"), ("remarks", "Remarks")]),
    ("Notices", "Every notice and its time bar", &[("no", "Notice"), ("event", "Event"), ("kind", "Kind"), ("d:event_date", "Event date"), ("days", "Days allowed"), ("d:due", "Due by"), ("d:sent", "Sent"),
        ("our_ref", "Our ref."), ("status", "Status"), ("left", "Days left"), ("linked", "Linked")]),
    ("Guarantees", "Every guarantee and insurance", &[("name", "Guarantee"), ("basis", "Basis"), ("amount", "Amount"), ("issuer", "Issuer"), ("number", "Number"), ("d:from", "From"), ("d:expires", "Expires"),
        ("left", "Days left"), ("d:due", "Release"), ("d:released", "Released"), ("status", "Status")]),
    ("Invoices", "Every invoice", &[("no", "Invoice"), ("payment", "Payment"), ("amount", "Amount"), ("d:invoiced", "Invoiced"), ("d:due", "Due"), ("certified", "Certified"), ("retention", "Retention"),
        ("d:paid", "Paid"), ("status", "Status"), ("left", "Days left"), ("remarks", "Remarks")]),
    ("Obligations", "Every obligation of the contract", &[("clause", "Clause"), ("what", "Obligation"), ("party", "Party"), ("owner", "Owner"), ("every", "Every"), ("d:done", "Last done"), ("d:due", "Next due"),
        ("left", "Days left"), ("status", "Status"), ("remarks", "Remarks")]),
];

fn date(d: Option<NaiveDate>) -> Value {
    d.map_or(Value::Empty, |d| Value::number((d - NaiveDate::from_ymd_opt(1970, 1, 1).unwrap_or_default()).num_days() as f64 + EPOCH))
}
fn text(s: &str) -> Value { if s.is_empty() { Value::Empty } else { Value::text(s) } }
fn num(v: Option<i64>) -> Value { v.map_or(Value::Empty, |x| Value::number(x as f64)) }

/// An own column's value as the sheets read it: a number or a date when the column holds one.
fn own_value(cols: &[OwnCol], own: &BTreeMap<String, String>, name: &str) -> Value {
    let v = own.get(name).map(String::as_str).unwrap_or("");
    match cols.iter().find(|c| c.name == name).map(|c| c.kind.as_str()) {
        Some("number") => v.trim().parse::<f64>().map_or(text(v), Value::number),
        Some("date") => NaiveDate::parse_from_str(v.trim(), "%Y-%m-%d").map_or(text(v), |d| date(Some(d))),
        _ => text(v),
    }
}

/// One document's value in a column.
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
        own => own_value(&r.columns, &d.own, own),
    }
}

/// One order's value in a column.
fn order_value(c: &Calc, o: &ProcOut, k: usize, key: &str) -> Value {
    let (r, it) = (&c.p.procurement, &c.p.procurement.items[k]);
    let Some(x) = o.items.get(k) else { return Value::Empty };
    let k5 = |s: &str| s[1..].parse::<usize>().ok().filter(|i| *i < 5);
    match key.trim_start_matches("d:") {
        "tag" => text(&it.tag), "name" => text(&it.name), "vendor" => text(&it.vendor), "origin" => text(&it.origin), "po_no" => text(&it.po_no), "value" => Value::number(it.value),
        "state" => text(if it.state.is_empty() { "Active" } else { &it.state }), "long_lead" => Value::Bool(it.long_lead), "mfg" => Value::number(it.mfg_pct),
        "promised" => date(it.promised), "forecast" => date(x.forecast), "var" => num(x.var), "required" => date(x.required), "float" => num(x.float), "rag" => text(&x.rag),
        "status" => text(&x.status), "next" => text(&x.next), "next_date" => date(x.next_date), "days_late" => Value::number(x.days_late as f64),
        "earned" => Value::number(x.earned), "planned" => Value::number(x.planned_pct), "task" => text(&x.task_name), "need" => text(&x.need_name),
        "docs" => if x.docs > 0 { text(&format!("{}/{}", x.docs_ok, x.docs)) } else { Value::Empty }, "remarks" => text(&it.remarks),
        s if s.len() == 2 && s.starts_with('p') && k5(s).is_some() => date(x.planned[k5(s).unwrap_or(0)]),
        s if s.len() == 2 && s.starts_with('a') && k5(s).is_some() => date(x.actual[k5(s).unwrap_or(0)]),
        own => own_value(&r.columns, &it.own, own),
    }
}

/// A register as rows of values: the standard columns, then the own ones (formula columns as
/// worked out in `own`, when given).
struct Rows { names: Vec<String>, dates: Vec<bool>, rows: Vec<Vec<Value>> }
fn rows(std: &[(&str, &str)], cols: &[OwnCol], n: usize, value: impl Fn(usize, &str) -> Value, own: impl Fn(usize, usize) -> Option<String>) -> Rows {
    let mut names: Vec<String> = std.iter().map(|x| x.1.to_string()).collect();
    names.extend(cols.iter().map(|c| c.name.clone()));
    let mut dates: Vec<bool> = std.iter().map(|x| x.0.starts_with("d:")).collect();
    dates.extend(cols.iter().map(|c| c.kind == "date"));
    let rows = (0..n).map(|k| {
        let mut v: Vec<Value> = std.iter().map(|(key, _)| value(k, key)).collect();
        v.extend(cols.iter().enumerate().map(|(i, c)| if c.kind == "formula" { own(k, i).map_or(Value::Empty, |s| s.parse::<f64>().map_or_else(|_| text(&s), Value::number)) } else { value(k, &c.name) }));
        v
    }).collect();
    Rows { names, dates, rows }
}
fn sheet(t: &Rows, name: &str) -> Sheet {
    let mut sh = Sheet::new(name);
    for (j, n) in t.names.iter().enumerate() { sh.set_value(CellRef::new(0, j as u32), Value::text(n.as_str())) }
    for (k, r) in t.rows.iter().enumerate() { for (j, v) in r.iter().enumerate() { sh.set_value(CellRef::new(k as u32 + 1, j as u32), v.clone()) } }
    sh
}

/// The register's own formula columns, per row: what each shows. `[Column]` reads that column of
/// the same row; a result that moves with the dates it reads is shown as a date.
fn own_eval(c: &Calc, t: &Rows, cols: &[OwnCol]) -> Vec<Vec<String>> {
    let base = t.names.len() - cols.len();
    let f: Vec<(usize, &OwnCol)> = cols.iter().enumerate().filter(|(_, x)| x.kind == "formula").collect();
    let mut out = vec![vec![String::new(); cols.len()]; t.rows.len()];
    if f.is_empty() || t.rows.is_empty() { return out }
    // Twice: as it is, and with every date column 1000 days later; a result 1000 days later is a date.
    let run = |shift: f64| -> Vec<Vec<Value>> {
        let mut sh = sheet(t, "R");
        if shift != 0.0 {
            for (j, _) in t.dates.iter().enumerate().filter(|(_, d)| **d) {
                for k in 0..t.rows.len() {
                    let at = CellRef::new(k as u32 + 1, j as u32);
                    if let Value::Number(n) = sh.value(at) { sh.set_value(at, Value::number(n + shift)) }
                }
            }
        }
        for &(i, x) in &f {
            let tr = translate(&x.formula, |name| t.names.iter().position(|n| n.eq_ignore_ascii_case(name.trim())).map(|j| format!("{}{{ROW}}", col_name(j as u32))));
            for k in 0..t.rows.len() {
                let cell = match &tr { Ok(s) => formula_cell(&format!("={}", s.replace("{ROW}", &(k + 2).to_string())), &c.fmt), Err(_) => Cell::value(Value::Error(gridcraft_core::CellError::Name)) };
                sh.set_cell(CellRef::new(k as u32 + 1, (base + i) as u32), cell);
            }
        }
        let mut wb = Workbook::new();
        wb.sheets[0] = Arc::new(sh);
        gridcraft_calc::Calc::new().recalc_all(&mut wb);
        let sh = &wb.sheets[0];
        (0..t.rows.len()).map(|k| f.iter().map(|&(i, _)| sh.value(CellRef::new(k as u32 + 1, (base + i) as u32))).collect()).collect()
    };
    let (a, b) = (run(0.0), run(1000.0));
    for k in 0..t.rows.len() {
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

fn doc_rows(c: &Calc, o: &RegOut, own: bool) -> Rows {
    let r = &c.p.documents;
    rows(&DOC_COLS, &r.columns, r.docs.len(), |k, key| doc_value(c, o, k, key), |k, i| own.then(|| o.docs.get(k).and_then(|x| x.own.get(i)).cloned()).flatten())
}
fn order_rows(c: &Calc, o: &ProcOut, own: bool) -> Rows {
    let r = &c.p.procurement;
    rows(&ORDER_COLS, &r.columns, r.items.len(), |k, key| order_value(c, o, k, key), |k, i| own.then(|| o.items.get(k).and_then(|x| x.own.get(i)).cloned()).flatten())
}

/// The Document Register's own formula columns, per document.
pub fn own_formulas(c: &Calc, o: &RegOut) -> Vec<Vec<String>> { own_eval(c, &doc_rows(c, o, false), &c.p.documents.columns) }
/// The Procurement Register's own formula columns, per order.
pub fn order_formulas(c: &Calc, o: &ProcOut) -> Vec<Vec<String>> { own_eval(c, &order_rows(c, o, false), &c.p.procurement.columns) }

/// One row of a Contract Register table: the row's own fields (by its JSON) and what the app works out.
fn contract_row(cols: &[(&str, &str)], item: &serde_json::Value, o: Option<&crate::contract::RowOut>, extra: impl Fn(&str) -> Option<Value>) -> Vec<Value> {
    cols.iter().map(|(key, _)| {
        let k = key.trim_start_matches("d:");
        if let Some(v) = extra(k) { return v }
        match k {
            "due" => date(o.and_then(|x| x.due)), "left" => num(o.and_then(|x| x.left)), "status" if o.is_some() => text(o.map_or("", |x| x.status.as_str())),
            _ => match &item[k] {
                serde_json::Value::String(s) if key.starts_with("d:") => date(NaiveDate::parse_from_str(s, "%Y-%m-%d").ok()),
                serde_json::Value::String(s) => text(s),
                serde_json::Value::Number(n) => n.as_f64().map_or(Value::Empty, Value::number),
                _ => Value::Empty,
            },
        }
    }).collect()
}

/// Adds the registers' sheets and tables to a workbook (for the registers that have rows).
pub fn add_tables(c: &Calc, wb: &mut Workbook) {
    let p = c.p;
    let k = &p.contract;
    if !(k.variations.is_empty() && k.notices.is_empty() && k.guarantees.is_empty() && k.invoices.is_empty() && k.obligations.is_empty()) {
        let o = crate::contract::status(c);
        fn js<T: serde::Serialize>(v: &[T]) -> Vec<serde_json::Value> { v.iter().map(|x| serde_json::to_value(x).unwrap_or_default()).collect() }
        let pay = |id: u32| k.payments.iter().find(|x| x.id == id).map(|x| text(&x.name));
        let lists: [(Vec<serde_json::Value>, &[crate::contract::RowOut]); 5] = [
            (js(&k.variations), &[]), (js(&k.notices), &o.notices), (js(&k.guarantees), &o.guarantees),
            (js(&k.invoices), &o.invoices), (js(&k.obligations), &o.obligations),
        ];
        for (n, ((name, _, cols), (items, outs))) in CONTRACT_TABLES.iter().zip(lists.iter()).enumerate() {
            let t = Rows { names: cols.iter().map(|x| x.1.to_string()).collect(), dates: vec![], rows: items.iter().enumerate().map(|(i, it)| contract_row(cols, it, outs.get(i), |key| match (n, key) {
                (3, "payment") => Some(pay(it["payment"].as_u64().unwrap_or(0) as u32).unwrap_or(Value::Empty)),
                _ => None,
            })).collect() };
            let mut sh = sheet(&t, &format!("_{name}"));
            sh.tables.push(table(7 + n as u32, name, t.names.clone(), items.len() as u32));
            wb.sheets.push(Arc::new(sh));
        }
    }
    if !p.documents.docs.is_empty() {
        let o = crate::docreg::status(c);
        let t = doc_rows(c, &o, true);
        let mut sh = sheet(&t, "_Documents");
        sh.tables.push(table(3, "Documents", t.names.clone(), p.documents.docs.len() as u32));
        wb.sheets.push(Arc::new(sh));
        let r = &p.documents;
        let log = Rows { names: LOG_COLS.iter().map(|x| x.1.to_string()).collect(), dates: vec![], rows: r.log.iter().map(|s| {
            let k = r.docs.iter().position(|d| d.id == s.doc);
            let x = o.subs.iter().find(|x| x.id == s.id);
            LOG_COLS.iter().map(|(key, _)| match key.trim_start_matches("d:") {
                "number" | "title" | "vendor" | "class" => k.map_or(Value::Empty, |k| doc_value(c, &o, k, key)),
                "rev" => text(&s.rev), "purpose" => text(&s.purpose), "transmittal" => text(&s.transmittal), "sent" => date(s.sent),
                "due" => date(x.and_then(|x| x.due)), "ret_transmittal" => text(&s.ret_transmittal), "returned" => date(s.returned), "code" => text(&s.code), "crs" => text(&s.crs),
                "days" => x.map_or(Value::Empty, |x| Value::number(x.days as f64)), "late" => x.map_or(Value::Empty, |x| Value::number(x.late as f64)),
                "state" => x.map_or(Value::Empty, |x| text(&x.state)), _ => Value::Bool(x.is_some_and(|x| x.accepted)),
            }).collect()
        }).collect() };
        let mut lg = sheet(&log, "_Transmittals");
        lg.tables.push(table(4, "Transmittals", log.names.clone(), r.log.len() as u32));
        wb.sheets.push(Arc::new(lg));
    }
    if !p.procurement.items.is_empty() {
        let o = crate::procure::status(c);
        let t = order_rows(c, &o, true);
        let mut sh = sheet(&t, "_Orders");
        sh.tables.push(table(5, "Orders", t.names.clone(), p.procurement.items.len() as u32));
        wb.sheets.push(Arc::new(sh));
        let notes = Rows { names: NOTE_COLS.iter().map(|x| x.1.to_string()).collect(), dates: vec![], rows: p.procurement.items.iter().flat_map(|it| it.log.iter().map(move |n| {
            vec![text(&it.tag), text(&it.name), date(n.date), text(&n.note), text(&n.by)]
        })).collect() };
        let n = notes.rows.len() as u32;
        let mut ns = sheet(&notes, "_Expediting");
        ns.tables.push(table(6, "Expediting", notes.names.clone(), n));
        wb.sheets.push(Arc::new(ns));
    }
}

/// Tidies what both registers share: lists named, without repeats and holding every value the rows
/// use (`used(field)`), own columns of a known kind, and words only for known status keys.
pub fn tidy(lists: &mut Vec<RegList>, columns: &mut Vec<OwnCol>, words: &mut BTreeMap<String, String>, keys: &[(&str, &str)], used: impl Fn(&str) -> Vec<String>) {
    let t = |s: &mut String, n: usize| { if s.chars().count() > n { *s = s.chars().take(n).collect() } };
    lists.truncate(40);
    for l in lists.iter_mut() {
        t(&mut l.name, 60); t(&mut l.field, 60);
        let mut seen = std::collections::HashSet::new();
        l.items.retain(|i| !i.value.trim().is_empty() && seen.insert(i.value.trim().to_lowercase()));
        l.items.truncate(2000);
        for i in &mut l.items { t(&mut i.value, 120); t(&mut i.label, 200); }
        for v in used(&l.field) {
            let v = v.trim();
            if !v.is_empty() && !l.items.iter().any(|i| i.value.trim().eq_ignore_ascii_case(v)) && l.items.len() < 2000 { l.items.push(ListItem { value: v.into(), ..Default::default() }) }
        }
    }
    columns.retain(|c| !c.name.trim().is_empty());
    columns.truncate(40);
    for c in columns.iter_mut() { t(&mut c.name, 60); t(&mut c.formula, 2000); if !["text", "number", "date", "list", "formula"].contains(&c.kind.as_str()) { c.kind = "text".into() } }
    words.retain(|k, v| keys.iter().any(|x| x.0 == k) && !v.trim().is_empty());
    for v in words.values_mut() { t(v, 80) }
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

    /// The Procurement Register: states, own columns (a formula per order), the client's words, and
    /// Variations[…], Notices[…], Invoices[…] in sheet formulas.
    #[test]
    fn the_contract_register_in_sheet_formulas() {
        let mut p = Project::new("K", NaiveDate::from_ymd_opt(2026, 11, 2).unwrap());
        p.contract.variations = vec![Variation { id: 1, no: "VO-1".into(), status: "Approved".into(), approved: 100.0, ..Default::default() }, Variation { id: 2, no: "VO-2".into(), status: "Submitted".into(), claimed: 40.0, ..Default::default() }];
        p.contract.notices = vec![Notice { id: 1, no: "N1".into(), event_date: NaiveDate::from_ymd_opt(2026, 11, 1), days: 10, ..Default::default() }];
        p.contract.payments = vec![Payment { id: 4, name: "Advance".into(), ..Default::default() }];
        p.contract.invoices = vec![Invoice { id: 1, no: "I1".into(), payment: 4, amount: 250.0, ..Default::default() }];
        let mut sd = SheetData { name: "S".into(), ..Default::default() };
        for (a, f) in [("A1", "=SUMIFS(Variations[Approved],Variations[Status],\"Approved\")"), ("A2", "=INDEX(Notices[Due by],1)-DATE(2026,11,11)"), ("A3", "=INDEX(Invoices[Payment],1)"), ("A4", "=COUNTA(Variations[VO No.])")] { sd.cells.insert(a.into(), f.into()); }
        p.sheets.push(sd);
        let c = Calc::new(&p, "%Y-%m-%d", NaiveDate::from_ymd_opt(2026, 11, 20).unwrap());
        let v = crate::sheet::sheet_view(&c, p.sheets.len() - 1).unwrap();
        let at = |a: &str| v.cells.iter().find(|x| x.0 == a).map(|x| x.2.clone()).unwrap_or_default();
        assert_eq!((at("A1"), at("A2"), at("A3"), at("A4")), ("100".into(), "0".into(), "Advance".into(), "2".into()));
    }

    /// Orders[…] / Expediting[…] in sheet formulas.
    #[test]
    fn orders_in_sheet_formulas_states_words_and_own_columns() {
        let d = |m, day| NaiveDate::from_ymd_opt(2026, m, day).unwrap();
        let mut doc = Doc::new(crate::templates::build(&crate::templates::TEMPLATES[0], d(11, 2)));
        let mut r = doc.p.procurement.clone();
        let it = |id: u32, tag: &str, v: &str| PItem { id, tag: tag.into(), name: format!("Order {tag}"), vendor: v.into(), value: 100.0, manual_w: 1.0,
            planned: [Some(d(11, 2)), Some(d(11, 16)), Some(d(12, 1)), None, Some(d(12, 31))], ..Default::default() };
        r.items = vec![it(1, "1.1", "Forge Co"), it(2, "1.2", "Forge Co"), it(3, "1.3", "Cast Ltd")];
        r.items[0].actual[0] = Some(d(11, 3));
        r.items[0].actual[1] = Some(d(11, 18));
        r.items[0].log.push(Expedite { date: Some(d(11, 20)), note: "Chased the forgings".into(), by: "EM".into() });
        r.items[2].state = "Cancelled".into();
        r.words.insert("po".into(), "PO issued".into());
        r.columns = vec![OwnCol { name: "Inspector".into(), kind: "list".into(), formula: String::new() }, OwnCol { name: "Chase".into(), kind: "formula".into(), formula: "=[Delivery planned]-14".into() }];
        r.items[0].own.insert("Inspector".into(), "TPI A".into());
        doc.apply(Op::Procurement { r: Box::new(r) }, "%Y-%m-%d").unwrap();
        let p = &doc.p.procurement;
        assert!(p.list("vendor").is_some_and(|l| l.items.len() == 2) && p.list("Inspector").is_none(), "the sub-vendors used join their list (a list column's list comes from the window)");
        let c = Calc::new(&doc.p, "%Y-%m-%d", d(11, 25));
        let o = crate::procure::status(&c);
        assert_eq!((o.items[0].status.as_str(), o.items[0].skey.as_str()), ("PO issued", "po"), "the client's words for a status");
        assert_eq!((o.items[2].skey.as_str(), o.items[2].weight), ("off", 0.0), "a cancelled order is left out of progress");
        assert_eq!(o.items[0].own, ["TPI A", "2026-12-17"], "a formula column per order: a date less 14 days is a date");
        let mut sd = SheetData { name: "S".into(), ..Default::default() };
        for (a, f) in [("A1", "=COUNTIFS(Orders[Sub-vendor],\"Forge Co\")"), ("A2", "=COUNTA(Orders[PO placed])"), ("A3", "=INDEX(Expediting[Note],1)"), ("A4", "=INDEX(Orders[Inspector],1)")] { sd.cells.insert(a.into(), f.into()); }
        doc.p.sheets.push(sd);
        let c = Calc::new(&doc.p, "%Y-%m-%d", d(11, 25));
        let v = crate::sheet::sheet_view(&c, doc.p.sheets.len() - 1).unwrap();
        let at = |a: &str| v.cells.iter().find(|x| x.0 == a).map(|x| x.2.clone()).unwrap_or_default();
        assert_eq!((at("A1"), at("A2"), at("A3"), at("A4")), ("2".into(), "1".into(), "Chased the forgings".into(), "TPI A".into()));
    }
}
