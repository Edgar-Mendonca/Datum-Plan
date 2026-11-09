//! Checked cases for delay analysis (0.8.2): what the plan allows, as-planned against as-built, windows.
use crate::{delay, doc::{Doc, Op}, fields::Calc, parse, *};
use chrono::NaiveDate;

fn d(y: i32, m: u32, day: u32) -> NaiveDate { NaiveDate::from_ymd_opt(y, m, day).unwrap() }
const D: i64 = 480;
const F: &str = "%Y-%m-%d";

/// A chain of three tasks (5, 3 and 2 days from Monday 2 November) and a side task, then a milestone.
fn plan() -> Doc {
    let mut p = Project::new("Delay", d(2026, 11, 2));
    for (n, dur) in [("Design", 5), ("Build", 3), ("Test", 2), ("Manuals", 2), ("Handover", 0)] { let t = p.new_task(n, 0, dur * D); p.tasks.push(t) }
    let ids: Vec<u32> = p.tasks.iter().map(|t| t.uid).collect();
    for (k, pr) in ["", "1", "2", "1", "3,4"].iter().enumerate() { p.tasks[k].preds = parse::preds(pr, &ids, D).unwrap() }
    Doc::new(p)
}
fn op(doc: &mut Doc, o: Op) { doc.apply(o, F).unwrap() }
fn status(doc: &mut Doc, day: &str) { op(doc, Op::ProjectSet { field: "status".into(), value: serde_json::json!(day) }) }
fn set(doc: &mut Doc, k: usize, f: &str, v: &str) { let uid = doc.p.tasks[k].uid; op(doc, Op::Set { uid, field: f.into(), value: v.into() }) }
fn calc(p: &Project) -> Calc<'_> { Calc::new(p, F, d(2026, 11, 2)) }

// Without a baseline or snapshots, the methods say what they need; with them, they can run.
#[test]
fn methods_follow_the_data() {
    let mut doc = plan();
    let a = delay::available(&calc(&doc.p));
    assert!(a.methods.iter().all(|m| !m.ok), "nothing to analyse yet");
    assert!(a.methods.iter().find(|m| m.key == "apab").unwrap().why.contains("No baseline"));
    op(&mut doc, Op::Baseline { uids: vec![], clear: false, n: 0 });
    status(&mut doc, "2026-11-06");
    op(&mut doc, Op::Snapshot { label: "W1".into(), comment: String::new() });
    status(&mut doc, "2026-11-13");
    op(&mut doc, Op::Snapshot { label: "W2".into(), comment: String::new() });
    let a = delay::available(&calc(&doc.p));
    let ok: Vec<&str> = a.methods.iter().filter(|m| m.ok).map(|m| m.key).collect();
    assert_eq!(ok, ["apab", "windows"]);
    assert_eq!((a.baseline, a.snapshots), (5, 2), "every task and the milestone have a baseline");
}

// Design took 8 days instead of 5: the critical chain and the finish move 3 working days; Design is
// the driver; the side task (Manuals) keeps its dates.
#[test]
fn as_planned_against_as_built() {
    let mut doc = plan();
    op(&mut doc, Op::Baseline { uids: vec![], clear: false, n: 0 });
    set(&mut doc, 0, "astart", "2026-11-02");
    set(&mut doc, 0, "afinish", "2026-11-11");
    status(&mut doc, "2026-11-11");
    let c = calc(&doc.p);
    let r = delay::apab(&c);
    let row = |n: &str| r.rows.iter().find(|x| x.name == n).unwrap();
    assert_eq!((row("Design").finish_var, row("Design").dur, row("Design").planned_dur), (3.0, 8.0, 5.0));
    assert_eq!(row("Build").start_var, 3.0);
    assert_eq!(row("Manuals").finish_var, 3.0, "Manuals waits for Design too");
    assert_eq!(r.delay, 3.0);
    assert!(!r.drivers.contains(&doc.p.tasks[0].uid), "a finished task is not on the critical path any more");
    assert!(r.drivers.contains(&doc.p.tasks[2].uid), "Test, on the critical path, finishes 3 days late");
    assert!(row("Design").finished && row("Design").started);
}

// Windows: the finish holds in the first window (Design on pace), slips 3 days in the second (Build
// re-estimated from 3 days to 6) and recovers 1 day in the last, to today's plan (Test cut to 1 day);
// the slip names Build, critical at the window's start, first.
#[test]
fn windows_between_snapshots() {
    let mut doc = plan();
    op(&mut doc, Op::Baseline { uids: vec![], clear: false, n: 0 });
    set(&mut doc, 0, "astart", "2026-11-02");
    set(&mut doc, 0, "pct", "40");
    status(&mut doc, "2026-11-03");
    op(&mut doc, Op::Snapshot { label: "Day 2".into(), comment: String::new() });
    set(&mut doc, 0, "pct", "60");
    status(&mut doc, "2026-11-04");
    op(&mut doc, Op::Snapshot { label: "Day 3".into(), comment: "Steady".into() });
    set(&mut doc, 1, "dur", "6");
    op(&mut doc, Op::Snapshot { label: "Day 3 again".into(), comment: "Build needs 3 more days".into() });
    set(&mut doc, 2, "dur", "1");
    set(&mut doc, 0, "pct", "80");
    status(&mut doc, "2026-11-05");
    let c = calc(&doc.p);
    let w = delay::windows(&c);
    assert_eq!(w.iter().map(|x| x.moved).collect::<Vec<_>>(), [0.0, 3.0, -1.0], "the last window runs to today's plan");
    assert_eq!(w.last().unwrap().total, 2.0);
    assert_eq!(w[1].comment, "Build needs 3 more days");
    assert_eq!(w[1].slips.first().map(|x| x.name.as_str()), Some("Build"), "{:?}", w[1].slips.iter().map(|x| (&x.name, x.critical, x.moved)).collect::<Vec<_>>());
    assert!(w[1].slips[0].critical && w[1].slips[0].moved == 3.0, "Build was critical at the window's start");
    assert_eq!(w[2].label, "Now");
}
