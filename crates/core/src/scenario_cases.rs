//! Checked cases for what-if scenarios (0.8.4): a copy that knows its plan, and merging back by kind.
use crate::{doc::{Doc, Op}, parse, scenario, *};
use chrono::NaiveDate;

fn d(y: i32, m: u32, day: u32) -> NaiveDate { NaiveDate::from_ymd_opt(y, m, day).unwrap() }
const D: i64 = 480;

fn plan() -> Project {
    let mut p = Project::new("Plan", d(2026, 11, 2));
    for (n, dur) in [("Design", 5), ("Build", 3), ("Test", 2), ("Handover", 0)] { let t = p.new_task(n, 0, dur * D); p.tasks.push(t) }
    let ids: Vec<u32> = p.tasks.iter().map(|t| t.uid).collect();
    for (k, pr) in ["", "1", "2", "3"].iter().enumerate() { p.tasks[k].preds = parse::preds(pr, &ids, D).unwrap() }
    p.subplans = vec![SubPlan { file: "x.dplan".into(), name: "x".into(), level: 0 }];
    p
}

// A scenario is the plan under another name that knows where it came from, standing alone.
#[test]
fn a_scenario_is_a_copy_that_knows_its_plan() {
    let p = plan();
    let s = scenario::make(&p, "Two shifts", "Plan.dplan");
    assert_eq!((s.name.as_str(), s.scenario_of.as_str()), ("Two shifts", "Plan.dplan"));
    assert_eq!(s.tasks, p.tasks);
    assert!(s.subplans.is_empty() && s.master.is_empty());
    assert!(!serde_json::to_string(&p).unwrap().contains("scenario_of"), "a plain plan saves as before");
}

// Merge takes only the chosen kinds: durations without links, then links, then added and deleted tasks.
#[test]
fn merge_takes_the_chosen_kinds() {
    let p = plan();
    let mut s = scenario::make(&p, "Faster", "Plan.dplan");
    s.tasks[1].dur = 2 * D; // Build shorter
    s.tasks[2].preds[0].lag = D; // Test waits a day more
    s.tasks[0].name = "Design (fast-track)".into();
    let extra = s.new_task("Extra crew mobilise", 0, D);
    let extra_uid = extra.uid;
    s.tasks.insert(1, extra);
    let gone = s.tasks.remove(4).uid; // Handover dropped
    let mut doc = Doc::new(p.clone());
    let f = "%Y-%m-%d";
    doc.apply(Op::Merge { from: Box::new(s.clone()), kinds: vec!["dur".into()] }, f).unwrap();
    assert_eq!(doc.p.tasks[1].dur, 2 * D);
    assert_eq!(doc.p.tasks[2].preds[0].lag, 0, "links not taken");
    assert_eq!(doc.p.tasks[0].name, "Design", "names not taken");
    assert_eq!(doc.p.tasks.len(), 4, "tasks not added or deleted");
    doc.apply(Op::Merge { from: Box::new(s.clone()), kinds: vec!["links".into(), "tasks".into(), "names".into()] }, f).unwrap();
    let names: Vec<&str> = doc.p.tasks.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(names, ["Design (fast-track)", "Extra crew mobilise", "Build", "Test"], "added after the task before it in the scenario; Handover gone");
    assert!(doc.p.idx(gone).is_none() && doc.p.idx(extra_uid).is_some());
    assert!(doc.p.next_uid > extra_uid, "new tasks never clash with later ones");
    assert_eq!(doc.p.tasks[3].preds[0].lag, D, "Test's longer lag came with the links");
    assert_eq!(doc.p.subplans, p.subplans, "what a merge does not cover stays as it was");
    assert!(doc.apply(Op::Merge { from: Box::new(s), kinds: vec!["everything".into()] }, f).is_err());
    doc.undo();
    doc.undo();
    assert_eq!(doc.p.tasks, p.tasks, "a merge undoes like any edit");
}
