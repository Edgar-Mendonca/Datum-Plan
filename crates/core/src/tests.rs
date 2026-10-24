use crate::{cal::{at, date_of, WorkCal, DAY}, doc::{copy, Doc, Op}, load, parse, sched::schedule, view, *};
use chrono::NaiveDate;

fn d(y: i32, m: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, day).unwrap()
}
const H: i64 = 60;
const D: i64 = 480;

/// Leaf tasks with (working days, preds) on the standard calendar starting Monday 2 Nov 2026.
fn plan(rows: &[(i64, &str)]) -> Project {
    let mut p = Project::new("T", d(2026, 11, 2));
    for (i, &(dur, _)) in rows.iter().enumerate() {
        let t = p.new_task(&format!("T{}", i + 1), 0, dur * D);
        p.tasks.push(t);
    }
    let ids: Vec<u32> = p.tasks.iter().map(|t| t.uid).collect();
    for (t, &(_, pr)) in p.tasks.iter_mut().zip(rows) {
        t.preds = parse::preds(pr, &ids, D).unwrap();
    }
    p
}

/// "dd hh:mm" of an instant, for readable assertions.
fn when(t: i64) -> String {
    format!("{} {:02}:{:02}", date_of(t).format("%d"), t.rem_euclid(DAY) / 60, t.rem_euclid(60))
}

fn op(doc: &mut Doc, o: Op) -> Result<(), String> {
    doc.apply(o, "%d-%b-%y")
}

#[test]
fn calendar_arithmetic() {
    let c = WorkCal::new(&Calendar::standard("S"));
    let mon = at(d(2026, 11, 2));
    assert_eq!(when(c.next_start(mon)), "02 08:00");
    assert_eq!(when(c.add(mon + 8 * H, 5 * H)), "02 14:00"); // lunch skipped
    assert_eq!(when(c.add(mon + 8 * H, 5 * D)), "06 17:00");
    assert_eq!(when(c.add(at(d(2026, 11, 6)) + 17 * H, 1)), "09 08:01"); // over the weekend
    assert_eq!(when(c.sub(at(d(2026, 11, 9)) + 8 * H, H)), "06 16:00");
    assert_eq!(c.between(mon, mon + 7 * DAY), 5 * D);
    assert_eq!(when(c.prev_end(at(d(2026, 11, 9)))), "06 17:00");
    let mut hol = Calendar::standard("H");
    hol.exceptions.push(Exception { name: "Holiday".into(), from: d(2026, 11, 3), to: d(2026, 11, 4), hours: vec![] });
    let c = WorkCal::new(&hol);
    assert_eq!(when(c.add(mon + 8 * H, 2 * D)), "05 17:00");
}

#[test]
fn forward_backward_and_float() {
    // 1 → 2 → 4 and 1 → 3 → 4; 3 is shorter so it has float.
    let p = plan(&[(2, ""), (5, "1"), (2, "1"), (1, "2,3")]);
    let s = schedule(&p).unwrap();
    assert_eq!(when(s.es[1]), "04 08:00");
    assert_eq!(when(s.ef[1]), "10 17:00");
    assert_eq!(when(s.es[3]), "11 08:00");
    assert_eq!(s.tf, vec![0, 0, 3 * D, 0]);
    assert_eq!(s.ff[2], 3 * D);
}

#[test]
fn link_types_lags_and_hours() {
    let mut p = plan(&[(4, ""), (3, "1SS+2d"), (3, "1FF"), (2, "1FS-1d"), (0, "1")]);
    p.tasks[1].preds[0].lag = 4 * H; // SS+4h
    let s = schedule(&p).unwrap();
    assert_eq!(when(s.es[1]), "02 13:00");
    assert_eq!(when(s.ef[2]), "05 17:00");
    assert_eq!(when(s.es[3]), "05 08:00");
    assert_eq!(when(s.es[4]), "05 17:00"); // a milestone sits at its predecessor's finish
}

#[test]
fn constraints_deadlines_and_alap() {
    let mut p = plan(&[(2, ""), (2, "1"), (1, "")]);
    p.tasks[0].constraint = Constraint::SNET;
    p.tasks[0].cdate = Some(d(2026, 11, 4));
    p.tasks[1].deadline = Some(d(2026, 11, 6));
    p.tasks[2].constraint = Constraint::ALAP;
    let s = schedule(&p).unwrap();
    assert_eq!(when(s.es[0]), "04 08:00");
    assert_eq!(when(s.ef[1]), "09 17:00");
    assert_eq!(s.tf[1], -D); // a missed deadline gives negative float
    assert_eq!(when(s.es[2]), "09 08:00"); // as late as possible
    p.tasks[0].constraint = Constraint::MFO;
    p.tasks[0].cdate = Some(d(2026, 11, 3));
    let s = schedule(&p).unwrap();
    assert_eq!(when(s.ef[0]), "03 17:00");
}

#[test]
fn task_calendars_summary_links_and_inactive() {
    let mut p = plan(&[(0, ""), (2, ""), (2, "2"), (1, "1"), (3, "")]);
    p.calendars.push(Calendar::full("24 Hours"));
    p.tasks[1].level = 1;
    p.tasks[2].level = 1;
    p.tasks[2].cal = Some("24 Hours".into());
    p.tasks[4].inactive = true;
    let s = schedule(&p).unwrap();
    assert_eq!(when(s.ef[2]), "04 09:00"); // 16 working hours on a 24-hour calendar
    assert_eq!(when(s.es[3]), "04 09:00"); // the link from the summary waits for all its tasks
    assert_eq!(s.finish, s.ef[3]); // the inactive task does not extend the project
    p.tasks[1].preds = vec![Link { pred: p.tasks[0].uid, kind: LinkType::FS, lag: 0 }];
    assert!(schedule(&p).err().unwrap().contains("own summary"));
}

#[test]
fn progress_against_the_status_date() {
    let mut p = plan(&[(4, ""), (4, "1"), (2, "")]);
    p.status = Some(d(2026, 11, 4));
    p.tasks[0].act_start = Some(d(2026, 11, 2));
    p.tasks[0].pct = 50;
    p.tasks[2].act_start = Some(d(2026, 11, 2));
    p.tasks[2].act_finish = Some(d(2026, 11, 3));
    p.tasks[2].pct = 100;
    let s = schedule(&p).unwrap();
    assert_eq!(when(s.ef[0]), "06 17:00"); // 2 days remain after the status date
    assert_eq!(when(s.ef[2]), "03 17:00");
    let mut doc = Doc::new(p);
    let uid = doc.p.tasks[1].uid;
    op(&mut doc, Op::Set { uid, field: "pct".into(), value: "100".into() }).unwrap();
    assert!(doc.p.tasks[1].act_start.is_some() && doc.p.tasks[1].act_finish.is_some());
    op(&mut doc, Op::Set { uid, field: "pct".into(), value: "0".into() }).unwrap();
    assert!(doc.p.tasks[1].act_start.is_none());
    let u0 = doc.p.tasks[0].uid;
    op(&mut doc, Op::MarkOnTrack { uids: vec![u0, uid] }).unwrap();
    assert_eq!(doc.p.tasks[1].pct, 0);
}

#[test]
fn edits_undo_and_cycles() {
    let mut doc = Doc::new(plan(&[(1, ""), (1, "1"), (1, ""), (1, "")]));
    let u: Vec<u32> = doc.p.tasks.iter().map(|t| t.uid).collect();
    assert!(op(&mut doc, Op::Set { uid: u[0], field: "preds".into(), value: "2".into() }).unwrap_err().contains("Circular"));
    op(&mut doc, Op::Set { uid: u[0], field: "dur".into(), value: "2w".into() }).unwrap();
    assert_eq!((doc.p.tasks[0].dur, doc.p.tasks[0].unit), (10 * D, Unit::W));
    doc.undo();
    assert_eq!(doc.p.tasks[0].dur, D);
    doc.redo();
    op(&mut doc, Op::Indent { uids: vec![u[1], u[2]] }).unwrap();
    assert!(doc.p.is_summary(0));
    assert!(doc.p.tasks[1].preds.is_empty()); // a link to its own new summary is dropped
    assert!(doc.p.tasks[0].preds.is_empty());
    op(&mut doc, Op::Outdent { uids: vec![u[1], u[2]] }).unwrap();
    op(&mut doc, Op::Move { uids: vec![u[3]], up: true }).unwrap();
    assert_eq!(doc.p.tasks[2].uid, u[3]);
    op(&mut doc, Op::Move { uids: vec![u[3]], up: false }).unwrap();
    assert_eq!(doc.p.tasks[3].uid, u[3]);
    op(&mut doc, Op::Set { uid: u[1], field: "preds".into(), value: "1".into() }).unwrap();
    let clip = copy(&doc.p, &[u[0], u[1]]);
    op(&mut doc, Op::Paste { at: 4, tasks: clip }).unwrap();
    assert_eq!(doc.p.tasks.len(), 6);
    assert_eq!(doc.p.tasks[5].preds[0].pred, doc.p.tasks[4].uid); // links inside the copy follow it
    op(&mut doc, Op::Set { uid: u[2], field: "finish".into(), value: "2026-11-06".into() }).unwrap();
    assert_eq!(doc.p.tasks[2].dur, 5 * D);
    op(&mut doc, Op::Delete { uids: vec![u[0]] }).unwrap();
    assert!(doc.p.tasks.iter().all(|t| t.preds.iter().all(|l| l.pred != u[0])));
}

#[test]
fn resources_costs_and_baseline() {
    let mut doc = Doc::new(plan(&[(2, ""), (3, "")]));
    let u: Vec<u32> = doc.p.tasks.iter().map(|t| t.uid).collect();
    op(&mut doc, Op::ResInsert { name: "Concrete".into() }).unwrap();
    let m = doc.p.resources[0].uid;
    op(&mut doc, Op::ResSet { uid: m, field: "kind".into(), value: "Material".into() }).unwrap();
    op(&mut doc, Op::ResSet { uid: m, field: "rate".into(), value: "50".into() }).unwrap();
    op(&mut doc, Op::Set { uid: u[0], field: "res".into(), value: "Engineer[50%], Concrete[12]".into() }).unwrap();
    op(&mut doc, Op::Set { uid: u[1], field: "res".into(), value: "Engineer".into() }).unwrap();
    let e = doc.p.resources[1].uid;
    op(&mut doc, Op::ResSet { uid: e, field: "rate".into(), value: "100".into() }).unwrap();
    let s = schedule(&doc.p).unwrap();
    let l = load::compute(&doc.p, &s);
    assert_eq!(l.work[0], 8.0 * H as f64); // 2 days at 50%
    assert_eq!(l.cost[0], 800.0 + 600.0);
    assert!(l.over[1]); // both tasks run on the same days
    assert_eq!(parse::fmt_res(&doc.p.tasks[0].res, &doc.p), "Engineer[50%], Concrete[12]");
    op(&mut doc, Op::Baseline { uids: vec![], clear: false }).unwrap();
    op(&mut doc, Op::Set { uid: u[0], field: "dur".into(), value: "4d".into() }).unwrap();
    let v = view::build(&doc.p, "%d-%b-%y", d(2026, 11, 1));
    assert_eq!(v.rows[0].vfinish, "2 days");
    assert_eq!(v.rows[0].cost, "2,200.00");
    let w = load::workload(&doc.p, &schedule(&doc.p).unwrap(), true);
    assert_eq!(w.rows[0].load[0], 16.0 + 24.0);
}

#[test]
fn calendars_and_project_settings() {
    let mut doc = Doc::new(plan(&[(1, "")]));
    let mut c = Calendar::standard("Site");
    c.week[5] = vec![(480, 720)];
    op(&mut doc, Op::CalPut { old: None, cal: c.clone() }).unwrap();
    let u0 = doc.p.tasks[0].uid;
    op(&mut doc, Op::Set { uid: u0, field: "cal".into(), value: "Site".into() }).unwrap();
    c.name = "Site 6-day".into();
    op(&mut doc, Op::CalPut { old: Some("Site".into()), cal: c }).unwrap();
    assert_eq!(doc.p.tasks[0].cal.as_deref(), Some("Site 6-day"));
    assert!(op(&mut doc, Op::CalDelete { name: "Standard".into() }).is_err());
    op(&mut doc, Op::Project { name: "P".into(), start: "2026-11-02".into(), status: "".into(), calendar: "Site 6-day".into(), day_hours: 10.0, manual_default: true }).unwrap();
    assert_eq!(doc.p.day_min, 600);
    op(&mut doc, Op::Insert { at: 1, name: "M".into(), ms: false }).unwrap();
    assert!(doc.p.tasks[1].manual);
}

#[test]
fn quick_add_and_rows() {
    let mut doc = Doc::new(plan(&[(1, "")]));
    op(&mut doc, Op::QuickAdd { at: 1, text: "Pour slab 5d after 1 @Site Crew[50%] by 20-Nov-26".into() }).unwrap();
    let t = &doc.p.tasks[1];
    assert_eq!((t.name.as_str(), t.dur, t.preds.len(), t.deadline), ("Pour slab", 5 * D, 1, Some(d(2026, 11, 20))));
    assert_eq!(doc.p.resources[0].name, "Site Crew");
    let rows = vec![vec![("name".into(), "A".into()), ("dur".into(), "3h".into())], vec![("name".into(), "B".into()), ("preds".into(), "3".into())]];
    op(&mut doc, Op::InsertRows { at: 2, rows }).unwrap();
    assert_eq!(doc.p.tasks[2].dur, 3 * H);
    assert_eq!(doc.p.tasks[3].preds[0].pred, doc.p.tasks[2].uid);
}

#[test]
fn parsing() {
    assert_eq!(parse::dur("3", D), Ok((3 * D, Unit::D)));
    assert_eq!(parse::dur("2 wks", D), Ok((10 * D, Unit::W)));
    assert_eq!(parse::dur("4h", D), Ok((4 * H, Unit::H)));
    assert!(parse::dur("x", D).is_err());
    assert_eq!(parse::fmt_dur(12 * H, Unit::D, D), "1.5 days");
    assert_eq!(parse::date("24-Oct-26"), Ok(d(2026, 10, 24)));
    assert_eq!(parse::date_in("10/24/2026", "%m/%d/%Y"), Ok(d(2026, 10, 24)));
    let ls = parse::preds("2SS+3d, 1; 3ff-4h", &[10, 20, 30], D).unwrap();
    assert_eq!((ls[0].pred, ls[0].kind, ls[0].lag), (20, LinkType::SS, 3 * D));
    assert_eq!((ls[2].kind, ls[2].lag), (LinkType::FF, -4 * H));
    assert!(parse::preds("9", &[1], D).is_err());
    let ids = [(10, 1), (20, 2), (30, 3)].into_iter().collect();
    assert_eq!(parse::fmt_preds(&ls, &ids, D), "2SS+3d, 1, 3FF-4h");
    assert_eq!(parse::constraint("start no earlier than"), Ok(Constraint::SNET));
    assert_eq!(view::money(1234567.5), "1,234,567.50");
}

#[test]
fn templates_and_old_files() {
    for t in crate::templates::TEMPLATES {
        let p = crate::templates::build(t, d(2026, 11, 2));
        let v = view::build(&p, "%d-%b-%y", d(2026, 11, 1));
        assert!(v.error.is_none(), "{}", t.id);
        let back = Project::from_json(&serde_json::to_string(&p).unwrap()).unwrap();
        assert_eq!(back, p);
    }
    let v1 = r#"{"format":1,"name":"Old","start":"2026-11-02","calendar":{"name":"Standard","week":[true,true,true,true,true,false,false],"holidays":["2026-11-03"]},
        "tasks":[{"uid":1,"name":"A","dur":2,"snet":"2026-11-02"},{"uid":2,"name":"B","dur":1,"preds":[{"pred":1,"kind":"FS","lag":1}]}],"next_uid":3}"#;
    let p = Project::from_json(v1).map_err(|e| e.to_string()).unwrap();
    assert_eq!((p.tasks[0].dur, p.tasks[0].constraint, p.tasks[1].preds[0].lag), (2 * D, Constraint::SNET, D));
    let s = schedule(&p).unwrap();
    assert_eq!(when(s.ef[0]), "04 17:00"); // the holiday carried over
}
