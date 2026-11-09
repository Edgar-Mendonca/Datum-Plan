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

fn event(name: &str, date: &str) -> DelayEvent { DelayEvent { name: name.into(), date: NaiveDate::parse_from_str(date, F).ok(), on: true, ..Default::default() } }

// Time impact: Build extended by 2 days moves the finish 2 days; a 3-day review between Design and
// Test runs beside Build and costs nothing more; an event switched off is left out.
#[test]
fn time_impact_adds_events_in_order() {
    let mut doc = plan();
    let (design, build, test) = (doc.p.tasks[0].uid, doc.p.tasks[1].uid, doc.p.tasks[2].uid);
    let mut a = event("Late drawings", "2026-11-05");
    a.extend = vec![(build, 2 * D)];
    let mut b = event("Client review", "2026-11-06");
    b.frag = vec![FragTask { name: "Review".into(), dur: 3 * D, after: vec![design], before: vec![test] }];
    let mut off = event("Not counted", "2026-11-07");
    off.extend = vec![(test, 10 * D)];
    off.on = false;
    for e in [b.clone(), a.clone(), off] { op(&mut doc, Op::Delay { id: 0, ev: Some(e) }) }
    assert_eq!(doc.p.delays.len(), 3);
    let c = calc(&doc.p);
    let r = delay::tia(&c).unwrap();
    assert_eq!(r.events.iter().map(|e| (e.name.as_str(), e.impact)).collect::<Vec<_>>(), [("Late drawings", 2.0), ("Client review", 0.0)], "in the order they arose");
    assert_eq!((r.total, r.events[1].total), (2.0, 2.0));
    assert!(r.basis.contains("copy of today's plan"));
    assert!(delay::available(&c).methods.iter().any(|m| m.key == "tia" && m.ok));
    // A finished task's extension is not added, and says so.
    set(&mut doc, 1, "astart", "2026-11-09");
    set(&mut doc, 1, "afinish", "2026-11-11");
    let r = delay::tia(&calc(&doc.p)).unwrap();
    assert!(r.events[0].notes[0].contains("finished"), "{:?}", r.events[0].notes);
}

// Impacted as-planned rebuilds the plan as agreed (baseline durations, no progress) even after the
// plan has moved on, then adds the events; collapsed as-built rebuilds the plan from actual dates
// and takes the events out: Design took 8 days, 3 of them the event's, so it saves 3.
#[test]
fn impacted_as_planned_and_collapsed_as_built() {
    let mut doc = plan();
    let (design, build) = (doc.p.tasks[0].uid, doc.p.tasks[1].uid);
    op(&mut doc, Op::Baseline { uids: vec![], clear: false, n: 0 });
    set(&mut doc, 0, "astart", "2026-11-02");
    set(&mut doc, 0, "afinish", "2026-11-11");
    status(&mut doc, "2026-11-11");
    let mut a = event("Late survey data", "2026-11-04");
    a.extend = vec![(design, 3 * D)];
    let mut b = event("Crane breakdown", "2026-11-12");
    b.extend = vec![(build, 2 * D)];
    for e in [a, b] { op(&mut doc, Op::Delay { id: 0, ev: Some(e) }) }
    let c = calc(&doc.p);
    let iap = delay::iap(&c).unwrap();
    assert!(iap.fidelity.as_deref().unwrap().contains("(0 working days apart)"), "{:?}", iap.fidelity);
    assert_eq!(iap.events.iter().map(|e| e.impact).collect::<Vec<_>>(), [3.0, 2.0]);
    let cab = delay::cab(&c).unwrap();
    assert!(cab.fidelity.as_deref().unwrap().contains("(0 working days apart)"), "{:?}", cab.fidelity);
    assert_eq!(cab.events.iter().map(|e| (e.name.as_str(), e.impact)).collect::<Vec<_>>(), [("Crane breakdown", 2.0), ("Late survey data", 3.0)], "latest first");
    assert_eq!(cab.total, 5.0);
    // The plan itself is not changed by any analysis.
    assert_eq!(doc.p.tasks[0].dur, 5 * D);
}

// "Add to plan" puts an event's new activities into the plan once (a second time adds nothing),
// and collapsed as-built then takes their time out again.
#[test]
fn fragnet_into_the_plan_and_collapsed() {
    let mut doc = plan();
    let (design, build) = (doc.p.tasks[0].uid, doc.p.tasks[1].uid);
    set(&mut doc, 0, "astart", "2026-11-02");
    set(&mut doc, 0, "afinish", "2026-11-06");
    status(&mut doc, "2026-11-06");
    let mut e = event("Client hold", "2026-11-06");
    e.frag = vec![FragTask { name: "Await approval".into(), dur: 4 * D, after: vec![design], before: vec![build] }];
    op(&mut doc, Op::Delay { id: 0, ev: Some(e) });
    let before = crate::sched::schedule(&doc.p).unwrap().finish;
    op(&mut doc, Op::DelayFrag { id: 1 });
    op(&mut doc, Op::DelayFrag { id: 1 });
    assert_eq!(doc.p.tasks.iter().filter(|t| t.name == "Client hold: Await approval").count(), 1);
    let after = crate::sched::schedule(&doc.p).unwrap().finish;
    assert!(after > before, "the approval wait now drives Build");
    let cab = delay::cab(&calc(&doc.p)).unwrap();
    assert_eq!(cab.events.iter().map(|x| x.impact).collect::<Vec<_>>(), [4.0], "{:?}", cab.events.iter().map(|x| &x.notes).collect::<Vec<_>>());
    doc.undo();
    doc.undo();
    assert!(!doc.p.tasks.iter().any(|t| t.name.starts_with("Client hold")), "undone like any edit");
}
