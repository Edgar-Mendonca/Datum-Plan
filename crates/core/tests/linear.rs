//! The time-location view (0.11.1): chainage fields and the lines drawn from them.
use chrono::NaiveDate;
use datum_core::{cal::at, doc::{Doc, Op}, fields::Calc, *};

fn d(y: i32, m: u32, day: u32) -> NaiveDate { NaiveDate::from_ymd_opt(y, m, day).unwrap() }
const F: &str = "%Y-%m-%d";
fn plan(rows: &[(&str, &str, &str, &str)]) -> Doc {
    let mut p = Project::new("Pipeline", d(2026, 11, 2));
    for &(name, ..) in rows { let t = p.new_task(name, 0, 0); p.tasks.push(t) }
    let mut doc = Doc::new(p);
    for (k, &(_, dur, from, to)) in rows.iter().enumerate() {
        let uid = doc.p.tasks[k].uid;
        for (f, v) in [("dur", dur), ("locfrom", from), ("locto", to)].into_iter().filter(|x| !x.1.is_empty()) { doc.apply(Op::Set { uid, field: f.into(), value: v.into() }, F).unwrap() }
    }
    doc
}
fn calc(doc: &Doc) -> Calc<'static> { Calc::new(Box::leak(Box::new(doc.p.clone())), F, d(2026, 11, 2)) }

/// Welding moves from km 0 to km 10 over 10 days; a crossing stays at km 5; testing has no chainage.
#[test]
fn lines_in_time_and_place() {
    let x = plan(&[("Welding", "10d", "0", "10"), ("Road crossing", "2d", "5", ""), ("Hydrotest", "3d", "", "")]);
    let l = linear::linear(&calc(&x));
    assert_eq!((l.items.len(), l.missing, l.lo, l.hi), (2, 1, 0.0, 10.0));
    let w = &l.items[0];
    assert_eq!((w.from, w.to, w.start, w.finish), (0.0, 10.0, at(d(2026, 11, 2)) + 480, at(d(2026, 11, 13)) + 1020));
    let c = &l.items[1];
    assert_eq!((c.from, c.to), (5.0, 5.0), "one place: a vertical bar");
}

/// Chainages read as numbers or as station form (12+500 is 12.5); empty clears; words are refused.
#[test]
fn chainage_fields() {
    let mut x = plan(&[("Lowering-in", "4d", "12+500", "18,000.5")]);
    let c = calc(&x);
    assert_eq!((c.val(0, "locfrom").num(), c.val(0, "locto").num()), (12.5, 18000.5));
    let uid = x.p.tasks[0].uid;
    assert!(x.apply(Op::Set { uid, field: "locto".into(), value: "far".into() }, F).is_err());
    x.apply(Op::Set { uid, field: "locto".into(), value: "".into() }, F).unwrap();
    assert_eq!(x.p.tasks[0].loc_to, None);
    let back: Project = serde_json::from_str(&serde_json::to_string(&x.p).unwrap()).unwrap();
    assert_eq!(back.tasks[0].loc_from, Some(12.5));
}
