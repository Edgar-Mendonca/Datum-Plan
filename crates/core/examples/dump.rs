//! Prints engine output for a template as JSON fixtures for the UI tests (tests_ui/run.cjs).
//! `dump guide` prints a plan in progress (baseline, two closed periods, risks, constraints) for
//! the guide's screenshots (tests_ui/guide_shots.cjs).
use datum_core::{doc::{Doc, Op}, fields::Calc, query::Query, *};
use chrono::NaiveDate;

fn guide_plan() -> (Project, NaiveDate) {
    let d = |m, day| NaiveDate::from_ymd_opt(2026, m, day).unwrap();
    let mut doc = Doc::new(templates::build(&templates::TEMPLATES[0], d(11, 2)));
    let f = "%Y-%m-%d";
    let mut op = |o: Op| doc.apply(o, f).unwrap_or_else(|e| panic!("{e}"));
    op(Op::Baseline { uids: vec![], clear: false, n: 0 });
    op(Op::UpdateProject { through: "2026-11-20".into(), reschedule: false });
    op(Op::ProjectSet { field: "status".into(), value: serde_json::json!("2026-11-20") });
    op(Op::Snapshot { label: "Week ending 20 Nov".into(), comment: String::new() });
    op(Op::UpdateProject { through: "2026-12-04".into(), reschedule: false });
    op(Op::ProjectSet { field: "status".into(), value: serde_json::json!("2026-12-04") });
    op(Op::Snapshot { label: "Week ending 4 Dec".into(), comment: String::new() });
    let uid = |name: &str, p: &Project| p.tasks.iter().find(|t| t.name.contains(name)).map(|t| t.uid).unwrap_or(1);
    // The 3D CAD model is running late: less done than planned.
    let cad = uid("3D CAD", &doc.p);
    doc.apply(Op::Set { uid: cad, field: "pct".into(), value: "45".into() }, f).unwrap_or_else(|e| panic!("{e}"));
    let (parts, tests) = (uid("Parts lead", &doc.p), uid("Environmental", &doc.p));
    for o in [
        Op::Risk { id: 0, risk: Some(Risk { name: "Supplier delivers parts late".into(), cause: "Single supplier".into(), prob: 3, impact: 4, owner: "Buyer".into(), response: "Qualify a second supplier".into(), status: "Open".into(), tasks: vec![parts], kind: "Threat".into(), days: 10.0, ..Default::default() }) },
        Op::Risk { id: 0, risk: Some(Risk { name: "Test lab not available".into(), cause: "Shared lab".into(), prob: 2, impact: 3, owner: "Test lead".into(), response: "Book the lab now".into(), status: "Open".into(), tasks: vec![tests], kind: "Threat".into(), days: 5.0, ..Default::default() }) },
        Op::Log { id: 0, item: Some(LogItem { uid: uid("Calculations", &doc.p), kind: "Drawing or information".into(), text: "Load cases from the client".into(), owner: "Client".into(), need: Some(d(12, 3)), status: "Open".into(), ..Default::default() }) },
        Op::Log { id: 0, item: Some(LogItem { uid: uid("Drawings", &doc.p), kind: "Labour".into(), text: "Second CAD designer".into(), owner: "Engineering manager".into(), need: Some(d(12, 9)), status: "Open".into(), ..Default::default() }) },
    ] { doc.apply(o, f).unwrap_or_else(|e| panic!("{e}")) }
    // Three milestones on the timeline.
    let on: Vec<(u32, bool)> = doc.p.tasks.iter().map(|t| (t.uid, ["Requirements signed off", "Critical design review", "Released to manufacturing"].contains(&t.name.as_str()))).collect();
    for (uid, yes) in on { doc.apply(Op::Set { uid, field: "timeline".into(), value: if yes { "Yes" } else { "No" }.into() }, f).unwrap_or_else(|e| panic!("{e}")) }
    // A weekly report on a sheet: plan numbers by name, a live milestone list and the vendors with their flags.
    let cells = [
        ("A1", "Weekly report"), ("A3", "Project finish"), ("B3", "=ProjectFinish"), ("A4", "Baseline finish"), ("B4", "=BaselineFinish"), ("A5", "% complete"), ("B5", "=PercentComplete"),
        ("A6", "Late tasks"), ("B6", "=COUNTIF(Tasks[Late],TRUE)"), ("A7", "SPI"), ("B7", "=SPI"),
        ("A9", "Milestone"), ("B9", "Finish"), ("C9", "% Complete"), ("A10", "=FILTER(CHOOSECOLS(Tasks,3,8,9),Tasks[Milestone]=TRUE,\"None\")"),
        ("E3", "Vendor"), ("F3", "Country"), ("G3", "Scope"), ("E4", "Northwind Castings"), ("F4", "DE"), ("G4", "Housings"), ("E5", "Pacific Circuits"), ("F5", "TW"), ("G5", "Control boards"),
        ("E6", "Kestrel Test Labs"), ("F6", "GB"), ("G6", "Environmental tests"), ("E7", "Total orders"), ("G7", "=COUNTA(G4:G6)"),
    ];
    let mut fmt: Vec<(String, String)> = [("B7", "0.00"), ("F4", "flag"), ("F5", "flag"), ("F6", "flag")].iter().map(|(a, b)| (a.to_string(), b.to_string())).collect();
    for r in 10..20 { fmt.push((format!("B{r}"), "date".into())); fmt.push((format!("C{r}"), "0\"%\"".into())); }
    let style = ["A1", "A9", "B9", "C9", "E3", "F3", "G3"].iter().map(|a| (a.to_string(), "b".to_string())).collect();
    doc.p.sheets.push(SheetData { name: "Weekly report".into(), cells: cells.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect(), fmt: fmt.into_iter().collect(), widths: [(0, 230), (4, 170), (5, 150), (6, 160)].into_iter().collect(), style });
    (doc.p, d(12, 4))
}

fn main() {
    let (p, today) = if std::env::args().nth(1).as_deref() == Some("guide") { guide_plan() } else {
        let t = &templates::TEMPLATES[2];
        let mut p = templates::build(t, NaiveDate::from_ymd_opt(2026, 10, 26).unwrap());
        // A vendor list on a sheet: flags, a total and a live count from the plan.
        let cells = [("A1", "Vendor"), ("B1", "Country"), ("C1", "Order"), ("A2", "Steel Co"), ("B2", "DE"), ("C2", "1200"), ("A3", "Pipe Works"), ("B3", "cn"), ("C3", "800"), ("C4", "=SUM(C2:C3)"), ("E1", "=ROWS(Tasks)")];
        let fmt = [("B2", "flag"), ("B3", "flag")];
        p.sheets.push(SheetData { name: "Vendors".into(), cells: cells.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect(), fmt: fmt.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect(), ..Default::default() });
        (p, NaiveDate::from_ymd_opt(2026, 10, 24).unwrap())
    };
    let c = Calc::new(&p, "%d-%b-%y", today);
    let q = Query { cols: p.columns.iter().filter(|c| c.1 > 0).map(|c| c.0.clone()).collect(), ..Query::default() };
    let g = c.grid(&q);
    let rows: Vec<usize> = g.rows.iter().filter(|r| r.kind == "task").filter_map(|r| p.idx(r.uid)).collect();
    let mut tl = serde_json::to_value(&g).unwrap();
    if let Some(r) = tl["rows"].as_array_mut() { r.retain(|x| x["uid"].as_u64().and_then(|u| p.idx(u as u32)).is_some_and(|i| p.tasks[i].timeline)) }
    let tq = Query { cols: ["name", "astart", "afinish", "pct", "phys", "adur", "rem", "acost", "awork"].map(String::from).to_vec(), ..Query::default() };
    let tracking = c.grid(&tq);
    let mut proj = serde_json::to_value(&p).unwrap();
    proj.as_object_mut().unwrap().remove("tasks");
    println!("{}", serde_json::json!({
        "state": { "project": proj, "summary": c.summary(), "path": null, "dirty": false, "undo": false, "redo": false, "clip": 0, "rev": 1 },
        "query": g, "query:timeline": tl, "query:tracking": tracking, "scurve": report::scurve(&c, "week", "duration", None), "period_report": report::period_report(&c, "week", None),
        "periods": report::period_ends(&c, "week"), "curve": report::curve(&c, "week"), "usage": report::usage(&c, "resource", "work", "week"), "usage:group": report::usage(&c, "group", "work", "week"),
        "risk_run": risk::simulate(&p, &risk::SimOpts { iterations: 300, ..Default::default() }).unwrap(), "level_preview": level::preview(&p, &[]).unwrap(),
        "manpower": progress::manpower(&c, "week"), "earned_schedule": progress::earned_schedule(&c), "lookahead": progress::lookahead(&c, 3), "credit_library": progress::credit_library(),
        "sheet_view": sheet::sheet_view(&c, 0), "formula_shift": ["=C3*2"],
        "sheet_catalog": { "functions": sheet::functions(), "names": sheet::project_names(&c).into_iter().map(|(n, _, d)| (n, d)).collect::<Vec<_>>(), "tasks": sheet::task_columns(&c).into_iter().map(|x| x.1).collect::<Vec<_>>(), "resources": sheet::RES_COLS.iter().map(|x| x.1).collect::<Vec<_>>() },
        "resources": c.res_grid(&["name", "kind", "label", "initials", "group", "max", "rate", "ovt", "per_use", "cal", "code", "country", "email", "work", "cost"].map(String::from)), "network": network::layout(&c, &rows), "field_list": { "task": fields::task_fields(), "resource": fields::resource_fields(), "filters": query::builtin_filters() },
    }));
}
