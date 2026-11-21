//! Hours typed per day (0.9.39): Task and Resource Usage by day, against hand figures.
//! Plans start Monday 2 Nov 2026 on the standard calendar (480 working minutes a day).
use chrono::NaiveDate;
use datum_core::{doc::{Doc, Op}, fields::Calc, *};

fn d(y: i32, m: u32, day: u32) -> NaiveDate { NaiveDate::from_ymd_opt(y, m, day).unwrap() }
const F: &str = "%Y-%m-%d";

/// One task of `dur` with R (100 %, 10 an hour) on it.
fn plan(dur: &str) -> Doc {
    let mut p = Project::new("A", d(2026, 11, 2));
    let t = p.new_task("T1", 0, 0);
    p.tasks.push(t);
    p.resources.push(Resource { uid: 1, name: "R".into(), max: 1.0, rate: 10.0, ..Default::default() });
    let mut doc = Doc::new(p);
    set(&mut doc, "dur", dur);
    set(&mut doc, "res", "R");
    doc
}
fn set(doc: &mut Doc, f: &str, v: &str) { let uid = doc.p.tasks[0].uid; doc.apply(Op::Set { uid, field: f.into(), value: v.into() }, F).unwrap_or_else(|e| panic!("{f}={v}: {e}")) }
fn hours(doc: &mut Doc, day: &str, h: &str) -> Result<(), String> { let uid = doc.p.tasks[0].uid; doc.apply(Op::DayWork { uid, res: 1, day: day.into(), hours: h.into() }, F).map(|_| ()) }
fn calc(doc: &Doc) -> Calc<'static> { Calc::new(Box::leak(Box::new(doc.p.clone())), F, d(2026, 11, 2)) }
fn daily(c: &Calc) -> Vec<f64> { c.l.task_daily.values().filter(|x| x.0 > 0.0).map(|x| (x.0 * 1e6).round() / 1e6).collect() }

/// 4 h on Tuesday of a 5-day task: 480, 240, 480, 480, 480 = 36 h, 360 at 10 an hour; then 10 h on
/// Friday: 38 h. The dates do not move.
#[test]
fn typed_hours_land_on_their_days_and_make_the_work() {
    let mut x = plan("5d");
    let finish = calc(&x).text(0, "finish");
    hours(&mut x, "2026-11-03", "4").unwrap();
    let c = calc(&x);
    assert_eq!(daily(&c), vec![480.0, 240.0, 480.0, 480.0, 480.0]);
    assert_eq!((c.val(0, "work").num(), c.val(0, "cost").num()), (36.0, 360.0));
    assert_eq!(c.text(0, "contour"), "Edited");
    hours(&mut x, "2026-11-06", "10h").unwrap();
    let c = calc(&x);
    assert_eq!(daily(&c), vec![480.0, 240.0, 480.0, 480.0, 600.0]);
    assert_eq!(c.val(0, "work").num(), 38.0);
    assert_eq!(c.text(0, "finish"), finish, "the dates stay");
    // Resource Usage by day shows the same.
    let u = report::usage(&c, "resource", "work", "day");
    let row = u.rows.iter().find(|r| r.of == Some(1)).unwrap();
    assert_eq!(row.vals.iter().copied().filter(|v| *v > 0.0).collect::<Vec<_>>(), vec![8.0, 4.0, 8.0, 8.0, 10.0]);
}

/// Wrong days and amounts are refused and change nothing.
#[test]
fn wrong_days_and_hours_are_refused() {
    let mut x = plan("5d");
    let before = x.p.clone();
    for (day, h) in [("2026-11-07", "4"), ("2026-11-10", "4"), ("2026-11-03", "25"), ("2026-11-03", "-1"), ("2026-11-03", "four"), ("3 Nov", "4")] {
        assert!(hours(&mut x, day, h).is_err(), "{day} {h}");
    }
    let mut one = plan("1d");
    assert!(hours(&mut one, "2026-11-02", "0").is_err(), "a task needs some hours");
    assert_eq!(x.p.tasks, before.tasks);
}

/// Choosing a contour clears the typed hours (back to 40 h flat); typing the names again keeps them;
/// a longer task stretches the pattern with the same total.
#[test]
fn contour_names_and_duration() {
    let mut x = plan("5d");
    hours(&mut x, "2026-11-02", "0").unwrap();
    assert_eq!(calc(&x).val(0, "work").num(), 32.0);
    set(&mut x, "res", "R");
    assert_eq!(calc(&x).val(0, "work").num(), 32.0, "the names typed again keep the hours");
    set(&mut x, "dur", "10d");
    let c = calc(&x);
    assert_eq!(c.val(0, "work").num(), 32.0, "stretched, same total");
    assert_eq!(daily(&c).len(), 8, "the empty first part stays empty: Mon and Tue of the first week carry none");
    set(&mut x, "contour", "Flat");
    assert_eq!(calc(&x).val(0, "work").num(), 80.0, "a contour clears the typed hours");
}

/// Under way: the work done is its share of the typed total, all of it is booked once.
#[test]
fn progress_on_typed_hours() {
    let mut x = plan("5d");
    hours(&mut x, "2026-11-06", "16").unwrap();
    x.apply(Op::ProjectSet { field: "status".into(), value: serde_json::json!("2026-11-03") }, F).unwrap();
    set(&mut x, "astart", "2026-11-02");
    set(&mut x, "pct", "40");
    let c = calc(&x);
    let total: f64 = c.l.task_daily.values().map(|v| v.0).sum();
    assert!((total - 2880.0).abs() < 1e-6, "48 h once: {total}");
    assert!((c.val(0, "work").num() - 48.0).abs() < 1e-9);
}
