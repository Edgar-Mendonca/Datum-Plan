//! Expected Finish and constraints on days off (0.9.41), against hand figures.
//! Plans start Monday 2 Nov 2026 on the standard calendar (8–12, 13–17, Mon–Fri).
use chrono::NaiveDate;
use datum_core::{doc::{Doc, Op}, fields::Calc, *};

fn d(y: i32, m: u32, day: u32) -> NaiveDate { NaiveDate::from_ymd_opt(y, m, day).unwrap() }
const F: &str = "%Y-%m-%d";

fn plan(rows: &[(&str, &str)]) -> Doc {
    let mut p = Project::new("A", d(2026, 11, 2));
    for k in 0..rows.len() { let t = p.new_task(&format!("T{}", k + 1), 0, 0); p.tasks.push(t) }
    p.resources.push(Resource { uid: 1, name: "R".into(), max: 1.0, rate: 10.0, ..Default::default() });
    let mut doc = Doc::new(p);
    for (k, (dur, preds)) in rows.iter().enumerate() {
        let uid = doc.p.tasks[k].uid;
        for (f, v) in [("dur", *dur), ("preds", *preds)] { doc.apply(Op::Set { uid, field: f.into(), value: v.into() }, F).unwrap() }
    }
    doc
}
fn set(doc: &mut Doc, k: usize, f: &str, v: &str) { let uid = doc.p.tasks[k].uid; doc.apply(Op::Set { uid, field: f.into(), value: v.into() }, F).unwrap_or_else(|e| panic!("{f}={v}: {e}")) }
fn calc(doc: &Doc) -> Calc<'static> { Calc::new(Box::leak(Box::new(doc.p.clone())), F, d(2026, 11, 2)) }

/// A 10-day task started Mon 2 Nov, 30 % done at the status date Wed 4 Nov; expected to finish
/// Fri 20 Nov instead of Fri 13 Nov. Its successor moves with it; the work stays 80 h.
#[test]
fn expected_finish_moves_a_task_under_way() {
    let mut x = plan(&[("10d", ""), ("2d", "1")]);
    set(&mut x, 0, "res", "R");
    x.apply(Op::ProjectSet { field: "status".into(), value: serde_json::json!("2026-11-04") }, F).unwrap();
    set(&mut x, 0, "astart", "2026-11-02");
    set(&mut x, 0, "pct", "30");
    let c = calc(&x);
    assert_eq!(c.text(0, "finish"), "2026-11-13");
    set(&mut x, 0, "expfinish", "2026-11-20");
    let c = calc(&x);
    assert_eq!((c.text(0, "expfinish"), c.text(0, "finish"), c.text(1, "start"), c.text(1, "finish")),
        ("2026-11-20".to_string(), "2026-11-20".to_string(), "2026-11-23".to_string(), "2026-11-24".to_string()));
    assert_eq!(c.val(0, "work").num(), 80.0, "the same work, spread over more days");
    // Earlier than the work allows is fine too: the remaining work shrinks to fit.
    set(&mut x, 0, "expfinish", "2026-11-10");
    assert_eq!(calc(&x).text(0, "finish"), "2026-11-10");
    // Never before it resumes: an expected finish before the status date finishes on the resume day.
    set(&mut x, 0, "expfinish", "2026-11-01");
    assert_eq!(calc(&x).text(0, "finish"), "2026-11-05");
}

/// Before it starts and once it is finished, the expected finish is not used.
#[test]
fn expected_finish_only_while_under_way() {
    let mut x = plan(&[("5d", "")]);
    set(&mut x, 0, "expfinish", "2026-11-20");
    assert_eq!(calc(&x).text(0, "finish"), "2026-11-06", "not started");
    set(&mut x, 0, "astart", "2026-11-02");
    set(&mut x, 0, "afinish", "2026-11-05");
    assert_eq!(calc(&x).text(0, "finish"), "2026-11-05", "finished");
    set(&mut x, 0, "expfinish", "");
    assert_eq!(calc(&x).text(0, "expfinish"), "");
}

/// A finish no earlier than Saturday finishes on Monday (as a start no earlier than Saturday starts on Monday).
#[test]
fn finish_no_earlier_than_a_day_off() {
    let mut x = plan(&[("1d", ""), ("1d", "")]);
    set(&mut x, 0, "ctype", "Finish No Earlier Than");
    set(&mut x, 0, "cdate", "2026-11-07");
    set(&mut x, 1, "ctype", "Start No Earlier Than");
    set(&mut x, 1, "cdate", "2026-11-07");
    let c = calc(&x);
    assert_eq!((c.text(0, "start"), c.text(0, "finish")), ("2026-11-09".to_string(), "2026-11-09".to_string()));
    assert_eq!(c.text(1, "start"), "2026-11-09");
    // On a working day it is unchanged: finish no earlier than Thu 5 Nov ends Thu 5 Nov.
    set(&mut x, 0, "cdate", "2026-11-05");
    assert_eq!(calc(&x).text(0, "finish"), "2026-11-05");
}

/// XER files carry the expected finish both ways.
#[test]
fn xer_keeps_the_expected_finish() {
    let mut x = plan(&[("10d", "")]);
    x.apply(Op::ProjectSet { field: "status".into(), value: serde_json::json!("2026-11-04") }, F).unwrap();
    set(&mut x, 0, "astart", "2026-11-02");
    set(&mut x, 0, "pct", "30");
    set(&mut x, 0, "expfinish", "2026-11-20");
    let s = sched::schedule(&x.p).unwrap();
    let text = io::xer::write(&x.p, &s, d(2026, 11, 4));
    assert!(text.contains("2026-11-20 17:00"), "written at the end of the day");
    let back = io::xer::read(&text).unwrap();
    assert_eq!(back.tasks.iter().find(|t| t.name == "T1").unwrap().exp_finish, Some(d(2026, 11, 20)));
}
