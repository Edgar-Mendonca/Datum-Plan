//! Checked cases for schedule quality (0.8.1): the health score, the logic change log, milestone trend.
use crate::{check, doc::{Doc, Op}, fields::Calc, parse, report, *};
use chrono::NaiveDate;

fn d(y: i32, m: u32, day: u32) -> NaiveDate { NaiveDate::from_ymd_opt(y, m, day).unwrap() }
const D: i64 = 480;

fn plan(rows: &[(i64, &str)]) -> Project {
    let mut p = Project::new("Q", d(2026, 11, 2));
    for (i, &(dur, _)) in rows.iter().enumerate() { let t = p.new_task(&format!("T{}", i + 1), 0, dur * D); p.tasks.push(t) }
    let ids: Vec<u32> = p.tasks.iter().map(|t| t.uid).collect();
    for (t, &(_, pr)) in p.tasks.iter_mut().zip(rows) { t.preds = parse::preds(pr, &ids, D).unwrap() }
    p
}
fn calc(p: &Project) -> Calc<'_> { Calc::new(p, "%Y-%m-%d", d(2026, 11, 2)) }

// A plan with sound logic scores 100; breaking logic and adding a hard constraint lowers it, by the
// weights; a weight of 0 leaves a check out; a check with nothing to look at is not counted.
#[test]
fn health_score_follows_the_checks_and_weights() {
    let good = plan(&[(5, ""), (3, "1"), (2, "2"), (0, "3")]);
    let s = check::health(&calc(&good));
    assert_eq!((s.score, s.grade), (100.0, "Good"), "{:?}", s.parts.iter().filter(|p| p.earned < 1.0).map(|p| p.key).collect::<Vec<_>>());
    assert!(s.parts.iter().any(|p| p.key == "missed" && !p.counted), "no baseline: missed tasks are left out");
    assert!(s.parts.iter().any(|p| p.key == "resources" && !p.counted), "resources weigh nothing by default");
    // Four open ends of eight tasks and a Must Finish On: logic and hard constraints fail.
    let mut bad = plan(&[(5, ""), (3, "1"), (2, ""), (4, ""), (1, ""), (2, "2"), (6, ""), (0, "6")]);
    bad.tasks[3].constraint = Constraint::MFO;
    bad.tasks[3].cdate = Some(d(2026, 11, 20));
    let c = calc(&bad);
    let sb = check::health(&c);
    assert!(sb.score < 85.0, "{}", sb.score);
    let logic = sb.parts.iter().find(|p| p.key == "logic").unwrap();
    assert!(logic.earned < 1.0 && logic.counted);
    // Taking the logic check out raises the score; the score is the weighted share of what was earned.
    let mut h = bad.health.clone();
    h.weights.insert("logic".into(), 0.0);
    let s2 = check::score(&check::run(&c, &h.limits), &h);
    assert!(s2.score > sb.score, "{} > {}", s2.score, sb.score);
    let (got, of) = s2.parts.iter().filter(|p| p.counted).fold((0.0, 0.0), |a, p| (a.0 + p.weight * p.earned, a.1 + p.weight));
    assert!((s2.score - (got / of * 1000.0).round() / 10.0).abs() < 1e-9);
}

// The settings are kept with the plan and undone like any edit; a snapshot keeps the score of its day.
#[test]
fn health_settings_saved_and_scored_in_snapshots() {
    let mut doc = Doc::new(plan(&[(5, ""), (3, ""), (2, "")]));
    let mut h = check::Health::default();
    h.weights.insert("logic".into(), 5.0);
    h.limits.pct = 10.0;
    doc.apply(Op::Health { h: h.clone() }, "%Y-%m-%d").unwrap();
    assert_eq!(doc.p.health, h);
    let json = serde_json::to_string(&doc.p).unwrap();
    assert_eq!(serde_json::from_str::<Project>(&json).unwrap().health, h);
    assert!(!serde_json::to_string(&Project::new("x", d(2026, 1, 1))).unwrap().contains("\"health\""), "a plan with the usual settings saves as before");
    let mut bad = h.clone();
    bad.weights.insert("logic".into(), f64::NAN);
    assert!(doc.apply(Op::Health { h: bad }, "%Y-%m-%d").is_err());
    doc.apply(Op::Snapshot { label: "Week 1".into(), comment: String::new() }, "%Y-%m-%d").unwrap();
    let score = doc.p.snapshots[0].health.unwrap();
    assert_eq!(score, check::health(&calc(&doc.p)).score);
    assert!(score < 100.0, "three unlinked tasks are not healthy: {score}");
    doc.undo();
    doc.undo();
    assert!(doc.p.health.is_default());
}

// The change log lists added and deleted tasks, duration, link (added, deleted, changed),
// constraint and progress changes, with the finish movement; ghost bars carry the old dates.
#[test]
fn change_log_lists_logic_changes() {
    let old = plan(&[(5, ""), (3, "1"), (2, "2"), (1, "")]);
    let mut new = old.clone();
    new.tasks[1].dur = 6 * D;
    new.tasks[2].preds[0].lag = 2 * D;
    new.tasks[3].preds = parse::preds("1SS", &new.tasks.iter().map(|t| t.uid).collect::<Vec<_>>(), D).unwrap();
    new.tasks[2].constraint = Constraint::SNET;
    new.tasks[2].cdate = Some(d(2026, 11, 30));
    new.tasks[0].pct = 50;
    let gone = new.tasks.remove(3).uid;
    let added = new.new_task("Extra", 0, D);
    let added_uid = added.uid;
    new.tasks.push(added);
    new.tasks[1].preds.push(Link { pred: gone, kind: LinkType::FS, lag: 0, pct: false, elapsed: false });
    new.tasks[1].preds.retain(|l| l.pred != gone);
    let (a, b) = (calc(&old), calc(&new));
    let ch = report::changes(&a, &b);
    let kinds: Vec<(&str, u32)> = ch.items.iter().map(|c| (c.kind.as_str(), c.uid)).collect();
    let has = |k: &str, u: u32| kinds.contains(&(k, u));
    assert!(has("Added", added_uid) && has("Deleted", gone), "{kinds:?}");
    assert!(has("Duration", new.tasks[1].uid) && has("Link changed", new.tasks[2].uid) && has("Constraint", new.tasks[2].uid) && has("% complete", new.tasks[0].uid), "{kinds:?}");
    let lc = ch.items.iter().find(|c| c.kind == "Link changed").unwrap();
    assert!(lc.was.ends_with(": FS") && lc.now.ends_with(": FS +2 days"), "{lc:?}");
    assert_eq!(ch.items.first().unwrap().kind, "Added", "listed in the order of CHANGE_KINDS");
    assert!(ch.counts.iter().any(|(k, n)| k == "Finish" && *n > 0));
    assert_ne!(ch.finish[0], ch.finish[1]);
    assert!(ch.finish_var.starts_with('7') || ch.finish_var.starts_with("1"), "{}", ch.finish_var);
    assert_eq!(ch.ghost.len(), 4);
    // The same plan: nothing changed.
    assert!(report::changes(&a, &calc(&old)).items.is_empty());
}

// Milestone trend: each milestone's forecast at each snapshot, then now.
#[test]
fn milestone_trend_from_snapshots() {
    let mut doc = Doc::new(plan(&[(5, ""), (0, "1"), (3, "2"), (0, "3")]));
    let f = "%Y-%m-%d";
    doc.apply(Op::ProjectSet { field: "status".into(), value: serde_json::json!("2026-11-06") }, f).unwrap();
    doc.apply(Op::Snapshot { label: "1".into(), comment: String::new() }, f).unwrap();
    let uid = doc.p.tasks[0].uid;
    doc.apply(Op::Set { uid, field: "dur".into(), value: "8".into() }, f).unwrap();
    doc.apply(Op::ProjectSet { field: "status".into(), value: serde_json::json!("2026-11-13") }, f).unwrap();
    let c = Calc::new(&doc.p, f, d(2026, 11, 13));
    let tr = report::milestone_trend(&c);
    assert_eq!(tr.iter().map(|t| t.name.as_str()).collect::<Vec<_>>(), ["T2", "T4"]);
    let day = |y, m, dd| crate::cal::at(d(y, m, dd)).div_euclid(crate::cal::DAY);
    // Unstarted work moves past the status date: on 6 Nov the milestone was forecast for 13 Nov; with
    // eight days of work from 16 Nov it is now 25 Nov.
    assert_eq!(tr[0].points, [(day(2026, 11, 6), day(2026, 11, 13)), (day(2026, 11, 13), day(2026, 11, 25))]);
}
