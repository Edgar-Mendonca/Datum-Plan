//! Prints engine output for a template as JSON fixtures for the UI tests (tests_ui/run.cjs).
use datum_core::{fields::Calc, query::Query, *};
fn main() {
    let t = &templates::TEMPLATES[2];
    let p = templates::build(t, chrono::NaiveDate::from_ymd_opt(2026, 10, 26).unwrap());
    let c = Calc::new(&p, "%d-%b-%y", chrono::NaiveDate::from_ymd_opt(2026, 10, 24).unwrap());
    let q = Query { cols: p.columns.iter().filter(|c| c.1 > 0).map(|c| c.0.clone()).collect(), ..Query::default() };
    let g = c.grid(&q);
    let rows: Vec<usize> = g.rows.iter().filter(|r| r.kind == "task").filter_map(|r| p.idx(r.uid)).collect();
    let mut proj = serde_json::to_value(&p).unwrap();
    proj.as_object_mut().unwrap().remove("tasks");
    println!("{}", serde_json::json!({
        "state": { "project": proj, "summary": c.summary(), "path": null, "dirty": false, "undo": false, "redo": false, "clip": 0, "rev": 1 },
        "query": g, "scurve": report::scurve(&c, "week", "duration", None), "period_report": report::period_report(&c, "week", None),
        "periods": report::period_ends(&c, "week"), "curve": report::curve(&c, "week"), "usage": report::usage(&c, "resource", "work", "week"), "network": network::layout(&c, &rows), "field_list": { "task": fields::task_fields(), "resource": fields::resource_fields(), "filters": query::builtin_filters() },
    }));
}
