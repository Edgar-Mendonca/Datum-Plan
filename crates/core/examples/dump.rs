//! Prints engine output for a template as JSON fixtures for UI tests.
use datum_core::{fields::Calc, query::Query, *};
fn main() {
    let t = &templates::TEMPLATES[2];
    let p = templates::build(t, chrono::NaiveDate::from_ymd_opt(2026, 10, 26).unwrap());
    let c = Calc::new(&p, "%d-%b-%y", chrono::NaiveDate::from_ymd_opt(2026, 10, 24).unwrap());
    let q = Query { cols: ["name", "dur", "start", "finish", "preds", "res", "pct"].map(String::from).to_vec(), ..Query::default() };
    let g = c.grid(&q);
    let rows: Vec<usize> = g.rows.iter().filter(|r| r.kind == "task").filter_map(|r| p.idx(r.uid)).collect();
    let mut proj = serde_json::to_value(&p).unwrap();
    proj.as_object_mut().unwrap().remove("tasks");
    println!("{}", serde_json::json!({
        "state": { "project": proj, "summary": c.summary(), "path": null, "dirty": false, "undo": false, "redo": false, "clip": 0, "rev": 1 },
        "query": g, "network": network::layout(&c, &rows), "field_list": { "task": fields::task_fields(), "resource": fields::resource_fields(), "filters": query::builtin_filters() },
    }));
}
