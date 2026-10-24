use crate::{cal::WorkCal, doc::{Doc, Op}, parse, sched::schedule, view, *};
use chrono::NaiveDate;

fn d(y: i32, m: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, day).unwrap()
}

/// Leaf tasks with (dur, preds) on a Mon–Fri calendar starting Monday 2 Nov 2026.
fn plan(rows: &[(i32, &str)]) -> Project {
    let mut p = Project::new("T", d(2026, 11, 2));
    for (i, &(dur, _)) in rows.iter().enumerate() {
        let t = p.new_task(&format!("T{}", i + 1), 0, dur);
        p.tasks.push(t);
    }
    let ids: Vec<u32> = p.tasks.iter().map(|t| t.uid).collect();
    for (t, &(_, pr)) in p.tasks.iter_mut().zip(rows) {
        t.preds = parse::preds(pr, &ids).unwrap();
    }
    p
}

fn run(p: &Project) -> crate::sched::Sched {
    schedule(p, &mut WorkCal::new(&p.calendar, p.start)).unwrap()
}

#[test]
fn calendar_skips_weekends() {
    let mut c = WorkCal::new(&Calendar::default(), d(2026, 10, 31)); // Saturday
    assert_eq!(c.date(0), d(2026, 11, 2));
    assert_eq!(c.date(5), d(2026, 11, 9));
    assert_eq!(c.ord(d(2026, 11, 7)), 5); // Saturday rolls to Monday
    assert_eq!(c.date(-1), d(2026, 10, 30));
    assert_eq!(c.ord(d(2026, 10, 29)), -2);
}

#[test]
fn forward_backward_and_float() {
    // 1 → 2 → 4 and 1 → 3 → 4; 3 is shorter so it has float.
    let p = plan(&[(2, ""), (5, "1"), (2, "1"), (1, "2,3")]);
    let s = run(&p);
    assert_eq!(s.es, vec![0, 2, 2, 7]);
    assert_eq!(s.ef, vec![2, 7, 4, 8]);
    assert_eq!(s.tf, vec![0, 0, 3, 0]);
    assert_eq!(s.ff[2], 3);
    assert_eq!(s.finish, 8);
}

#[test]
fn link_types_and_lags() {
    let p = plan(&[(4, ""), (3, "1SS+2d"), (3, "1FF"), (2, "1SF+1d"), (1, "1FS-1d")]);
    let s = run(&p);
    assert_eq!(s.es[1], 2); // SS+2
    assert_eq!(s.ef[2], 4); // FF: finishes with 1
    assert_eq!(s.es[3], 0); // SF+1 would start before the project: clamped
    assert_eq!(s.es[4], 3); // FS-1
}

#[test]
fn constraint_and_summary_rollup() {
    let mut p = plan(&[(0, ""), (3, ""), (2, "2")]);
    p.tasks[1].level = 1;
    p.tasks[2].level = 1;
    p.tasks[1].snet = Some(d(2026, 11, 4));
    let s = run(&p);
    assert!(s.summary[0]);
    assert_eq!((s.es[0], s.ef[0]), (2, 7));
}

#[test]
fn cycles_are_rejected_and_undo_works() {
    let mut doc = Doc::new(plan(&[(1, ""), (1, "1")]));
    let uid = doc.p.tasks[0].uid;
    let err = doc.apply(Op::Set { uid, field: "preds".into(), value: "2".into() }).unwrap_err();
    assert!(err.contains("Circular"));
    assert!(!doc.can_undo());
    doc.apply(Op::Set { uid, field: "dur".into(), value: "2w".into() }).unwrap();
    assert_eq!(doc.p.tasks[0].dur, 10);
    doc.undo();
    assert_eq!(doc.p.tasks[0].dur, 1);
    doc.redo();
    assert_eq!(doc.p.tasks[0].dur, 10);
}

#[test]
fn indent_outdent_link_delete() {
    let mut doc = Doc::new(plan(&[(1, ""), (1, ""), (1, ""), (1, "")]));
    let u: Vec<u32> = doc.p.tasks.iter().map(|t| t.uid).collect();
    doc.apply(Op::Indent { uids: vec![u[1], u[2]] }).unwrap();
    assert!(doc.p.is_summary(0));
    doc.apply(Op::Link { uids: vec![u[1], u[2], u[3]] }).unwrap();
    assert_eq!(doc.p.tasks[2].preds[0].pred, u[1]);
    // Indenting under a linked task drops its links: summaries are not linked in 0.1.
    doc.apply(Op::Indent { uids: vec![u[2]] }).unwrap();
    assert!(doc.p.tasks[1].preds.is_empty());
    doc.apply(Op::Outdent { uids: vec![u[2]] }).unwrap();
    doc.apply(Op::Delete { uids: vec![u[0]] }).unwrap(); // takes its children too
    assert_eq!(doc.p.tasks.len(), 1);
    assert!(doc.p.tasks[0].preds.is_empty());
}

#[test]
fn parsing() {
    assert_eq!(parse::dur("3"), Ok(3));
    assert_eq!(parse::dur("2 wks"), Ok(10));
    assert_eq!(parse::dur("1.5w"), Ok(8));
    assert!(parse::dur("x").is_err());
    assert_eq!(parse::date("24-Oct-26"), Ok(d(2026, 10, 24)));
    assert_eq!(parse::date("2026-10-24"), Ok(d(2026, 10, 24)));
    let ls = parse::preds("2SS+3d, 1; 3ff-1", &[10, 20, 30]).unwrap();
    assert_eq!((ls[0].pred, ls[0].kind, ls[0].lag), (20, LinkType::SS, 3));
    assert_eq!((ls[2].kind, ls[2].lag), (LinkType::FF, -1));
    assert!(parse::preds("9", &[1]).is_err());
    let ids = [(10, 1), (20, 2), (30, 3)].into_iter().collect();
    assert_eq!(parse::fmt_preds(&ls, &ids), "2SS+3d, 1, 3FF-1d");
}

#[test]
fn templates_schedule_and_round_trip() {
    for t in crate::templates::TEMPLATES {
        let p = crate::templates::build(t, d(2026, 11, 2));
        let v = view::build(&p);
        assert!(v.error.is_none(), "{}", t.id);
        assert!(v.rows[0].sum && v.rows.iter().any(|r| r.ms), "{}", t.id);
        let back: Project = serde_json::from_str(&serde_json::to_string(&p).unwrap()).unwrap();
        assert_eq!(back, p);
    }
}

#[test]
fn finish_sets_duration() {
    let mut doc = Doc::new(plan(&[(2, "")]));
    let uid = doc.p.tasks[0].uid;
    doc.apply(Op::Set { uid, field: "finish".into(), value: "2026-11-06".into() }).unwrap();
    assert_eq!(doc.p.tasks[0].dur, 5);
    doc.apply(Op::Set { uid, field: "finish".into(), value: "2026-11-08".into() }).unwrap(); // Sunday
    assert_eq!(doc.p.tasks[0].dur, 5);
}
