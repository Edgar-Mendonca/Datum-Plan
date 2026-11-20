//! Audit of the multi-plan features: links between plans, scenarios, portfolio and pool, copying
//! tasks between plans. Each failing test proves a bug (expected values worked by hand).
use chrono::NaiveDate;
use datum_core::cal::{at, date_of, DAY};
use datum_core::doc::{self, Doc, Op};
use datum_core::*;

const D: i64 = 480;
const F: &str = "%Y-%m-%d";
fn d(y: i32, m: u32, dd: u32) -> NaiveDate { NaiveDate::from_ymd_opt(y, m, dd).unwrap() }
fn hm(day: NaiveDate, h: i64) -> i64 { at(day) + h * 60 }

/// A plan of independent tasks with these durations (days).
fn plan(name: &str, start: NaiveDate, durs: &[i64]) -> Project {
    let mut p = Project::new(name, start);
    for (k, &n) in durs.iter().enumerate() { let t = p.new_task(&format!("{name} {k}"), 0, n * D); p.tasks.push(t) }
    p
}
fn ext(file: &str, uid: u32, kind: LinkType, lag: i64) -> ExtLink {
    ExtLink { file: file.into(), uid, kind, lag, elapsed: false, plan: String::new(), code: String::new(), name: String::new(), start: 0, finish: 0 }
}
fn dates(p: &Project) -> Vec<(i64, i64)> { let s = sched::schedule(p).unwrap(); (0..p.tasks.len()).map(|i| (s.es[i], s.ef[i])).collect() }

// ---------- links between plans ----------

// A chain A -> B -> C across three files settles, and each plan's dates follow the one before.
#[test]
fn chain_of_three_plans_settles() {
    let a = plan("A", d(2026, 11, 2), &[5]); // Mon 2 - Fri 6 Nov
    let mut b = plan("B", d(2026, 11, 2), &[3]);
    let mut c = plan("C", d(2026, 11, 2), &[2]);
    b.tasks[0].ext.push(ext("C:/w/A.dplan", a.tasks[0].uid, LinkType::FS, 0));
    c.tasks[0].ext.push(ext("C:/w/B.dplan", b.tasks[0].uid, LinkType::FS, 0));
    // Listed so that the chain runs against the order of the list (worst case).
    let mut set = vec![("C:/w/C.dplan".to_string(), c), ("C:/w/B.dplan".to_string(), b), ("C:/w/A.dplan".to_string(), a)];
    let (_, missing) = links::update_all(&mut set).unwrap();
    assert!(missing.is_empty());
    // B: Mon 9 - Wed 11 Nov; C: Thu 12 - Fri 13 Nov.
    assert_eq!(dates(&set[1].1)[0], (hm(d(2026, 11, 9), 8), hm(d(2026, 11, 11), 17)));
    assert_eq!(dates(&set[0].1)[0], (hm(d(2026, 11, 12), 8), hm(d(2026, 11, 13), 17)));
}

// A real task-level loop across two plans is refused.
#[test]
fn loop_across_plans_is_refused() {
    let mut a = plan("A", d(2026, 11, 2), &[1]);
    let mut b = plan("B", d(2026, 11, 2), &[1]);
    a.tasks[0].ext.push(ext("A/B.dplan", b.tasks[0].uid, LinkType::FS, 0));
    b.tasks[0].ext.push(ext("A/A.dplan", a.tasks[0].uid, LinkType::FS, 0));
    let mut set = vec![("A/A.dplan".to_string(), a), ("A/B.dplan".to_string(), b)];
    assert!(links::update_all(&mut set).is_err());
}

// Plans link both ways without a task loop (main -> sub -> main): settles.
#[test]
fn both_ways_without_task_loop_settles() {
    let mut a = plan("A", d(2026, 11, 2), &[2, 1]);
    let mut b = plan("B", d(2026, 11, 2), &[3, 1]);
    b.tasks[0].ext.push(ext("w/A.dplan", a.tasks[0].uid, LinkType::FS, 0)); // B0 after A0
    a.tasks[1].ext.push(ext("w/B.dplan", b.tasks[0].uid, LinkType::FS, 0)); // A1 after B0
    let mut set = vec![("w/A.dplan".to_string(), a), ("w/B.dplan".to_string(), b)];
    links::update_all(&mut set).unwrap();
    // A0 Mon 2-Tue 3; B0 Wed 4 - Fri 6; A1 Mon 9.
    assert_eq!(dates(&set[0].1)[1].0, hm(d(2026, 11, 9), 8));
}

// Plans on different calendars: a 7-day plan finishing on Saturday holds a 5-day plan's task until Monday.
#[test]
fn different_calendars_and_start_dates() {
    let mut a = plan("A", d(2026, 11, 5), &[2]); // Thu 5, then Fri 6 ... on 7 days: Thu 5 - Fri 6
    let mut cal = Calendar::standard("Seven");
    cal.week[5] = cal.week[0].clone();
    cal.week[6] = cal.week[0].clone();
    a.calendars = vec![cal];
    a.calendar = "Seven".into();
    a.tasks[0].dur = 3 * D; // Thu 5 - Sat 7
    let mut b = plan("B", d(2026, 10, 1), &[1]);
    b.tasks[0].ext.push(ext("w/A.dplan", a.tasks[0].uid, LinkType::FS, 0));
    let mut set = vec![("w/A.dplan".to_string(), a), ("w/B.dplan".to_string(), b)];
    links::update_all(&mut set).unwrap();
    let x = &set[1].1.tasks[0].ext[0];
    assert_eq!(x.finish, hm(d(2026, 11, 7), 17), "A finishes Saturday 17:00");
    assert_eq!(dates(&set[1].1)[0].0, hm(d(2026, 11, 9), 8), "B's task waits until Monday");
}

// A link to a task deleted in the other plan keeps its last known dates and is reported.
#[test]
fn link_to_deleted_task_keeps_last_dates() {
    let a = plan("A", d(2026, 11, 2), &[5]);
    let mut b = plan("B", d(2026, 11, 2), &[1]);
    b.tasks[0].ext.push(ext("w/A.dplan", a.tasks[0].uid, LinkType::FS, 0));
    let mut set = vec![("w/A.dplan".to_string(), a), ("w/B.dplan".to_string(), b)];
    links::update_all(&mut set).unwrap();
    let kept = set[1].1.tasks[0].ext[0].finish;
    set[0].1.tasks.clear();
    let (_, missing) = links::update_all(&mut set).unwrap();
    assert_eq!(missing.len(), 1);
    assert_eq!(set[1].1.tasks[0].ext[0].finish, kept);
}

// BUG: when the other plan cannot be scheduled (say a circular link was just made in it),
// `update_all` copies 0/0 dates into every link that points at it. The scheduler skips links whose
// finish is 0 ("never read"), so the successor silently loses its hold and jumps back to the
// project start. Expected: the link keeps its last known dates and is reported as not followed.
#[test]
fn update_all_with_an_unschedulable_target_drops_the_link_silently() {
    let mut a = plan("A", d(2026, 11, 2), &[5, 1, 1]);
    let mut b = plan("B", d(2026, 11, 2), &[2]);
    b.tasks[0].ext.push(ext("w/A.dplan", a.tasks[0].uid, LinkType::FS, 0));
    let mut set = vec![("w/A.dplan".to_string(), a.clone()), ("w/B.dplan".to_string(), b)];
    links::update_all(&mut set).unwrap();
    let before = set[1].1.tasks[0].ext[0].clone();
    assert_eq!(before.finish, hm(d(2026, 11, 6), 17));
    assert_eq!(dates(&set[1].1)[0].0, hm(d(2026, 11, 9), 8), "B waits for A: Mon 9 Nov");
    // A gets a loop between its tasks 1 and 2 (it cannot be scheduled now).
    let (u1, u2) = (a.tasks[1].uid, a.tasks[2].uid);
    a.tasks[1].preds = vec![Link { pred: u2, kind: LinkType::FS, lag: 0, pct: false, elapsed: false }];
    a.tasks[2].preds = vec![Link { pred: u1, kind: LinkType::FS, lag: 0, pct: false, elapsed: false }];
    assert!(sched::schedule(&a).is_err());
    set[0].1 = a;
    let (_, missing) = links::update_all(&mut set).unwrap();
    let after = &set[1].1.tasks[0].ext[0];
    assert!(!missing.is_empty() || (after.start, after.finish) == (before.start, before.finish),
        "the link was overwritten with {}..{} and nothing was reported", after.start, after.finish);
    assert_eq!(dates(&set[1].1)[0].0, hm(d(2026, 11, 9), 8), "B must still wait for A (Mon 9 Nov)");
}

// BUG?: a link to a task that is inactive in the other plan. Inside a plan an inactive task holds
// nothing; through a link it still holds the successor by its (meaningless) computed dates.
#[test]
fn link_to_an_inactive_task_holds_nothing() {
    let mut a = plan("A", d(2026, 11, 2), &[10]);
    a.tasks[0].inactive = true;
    // Inside A: a successor of the inactive task is not held by it.
    let mut a2 = a.clone();
    let t = a2.new_task("after", 0, D);
    let u0 = a2.tasks[0].uid;
    a2.tasks.push(t);
    a2.tasks[1].preds = vec![Link { pred: u0, kind: LinkType::FS, lag: 0, pct: false, elapsed: false }];
    assert_eq!(dates(&a2)[1].0, hm(d(2026, 11, 2), 8), "in-plan: an inactive predecessor holds nothing");
    let mut b = plan("B", d(2026, 11, 2), &[1]);
    b.tasks[0].ext.push(ext("w/A.dplan", a.tasks[0].uid, LinkType::FS, 0));
    let mut set = vec![("w/A.dplan".to_string(), a), ("w/B.dplan".to_string(), b)];
    links::update_all(&mut set).unwrap();
    assert_eq!(dates(&set[1].1)[0].0, hm(d(2026, 11, 2), 8), "across plans: the inactive task should hold nothing either");
}

// A link to a summary in the other plan takes the summary's rolled-up dates.
#[test]
fn link_to_a_summary_takes_its_rolled_up_dates() {
    let mut a = plan("A", d(2026, 11, 2), &[0, 2, 3]);
    a.tasks[1].level = 1;
    a.tasks[2].level = 1;
    let (u1, u2) = (a.tasks[1].uid, a.tasks[2].uid);
    let _ = u1;
    a.tasks[2].preds = vec![Link { pred: u1, kind: LinkType::FS, lag: 0, pct: false, elapsed: false }];
    let _ = u2;
    let mut b = plan("B", d(2026, 11, 2), &[1]);
    b.tasks[0].ext.push(ext("w/A.dplan", a.tasks[0].uid, LinkType::FS, 0));
    let mut set = vec![("w/A.dplan".to_string(), a), ("w/B.dplan".to_string(), b)];
    links::update_all(&mut set).unwrap();
    // Summary Mon 2 - Fri 6 (2d + 3d): B after it on Mon 9.
    assert_eq!(set[1].1.tasks[0].ext[0].finish, hm(d(2026, 11, 6), 17));
    assert_eq!(dates(&set[1].1)[0].0, hm(d(2026, 11, 9), 8));
}

// FF and SS links from another plan with a lag on the successor's calendar.
#[test]
fn ext_ss_and_ff_with_lag() {
    let a = plan("A", d(2026, 11, 2), &[5]);
    let mut b = plan("B", d(2026, 11, 2), &[2, 2]);
    let ua = a.tasks[0].uid;
    b.tasks[0].ext.push(ext("w/A.dplan", ua, LinkType::SS, 2 * D)); // starts Wed 4
    b.tasks[1].ext.push(ext("w/A.dplan", ua, LinkType::FF, D)); // finishes Mon 9 -> starts Fri 6
    let mut set = vec![("w/A.dplan".to_string(), a), ("w/B.dplan".to_string(), b)];
    links::update_all(&mut set).unwrap();
    let s = dates(&set[1].1);
    assert_eq!(s[0].0, hm(d(2026, 11, 4), 8));
    assert_eq!((s[1].0, s[1].1), (hm(d(2026, 11, 6), 8), hm(d(2026, 11, 9), 17)));
}

// "Moved, update links": the state is "changed" exactly when the other task's dates moved.
#[test]
fn states_report_moved_targets() {
    let a = plan("A", d(2026, 11, 2), &[5]);
    let mut b = plan("B", d(2026, 11, 2), &[1]);
    b.tasks[0].ext.push(ext("w/A.dplan", a.tasks[0].uid, LinkType::FS, 0));
    let mut set = vec![("w/A.dplan".to_string(), a), ("w/B.dplan".to_string(), b)];
    links::update_all(&mut set).unwrap();
    let a_now = |p: &Project| { let s = sched::schedule(p).unwrap(); links::Found { plan: p.name.clone(), code: String::new(), name: p.tasks[0].name.clone(), start: s.es[0], finish: s.ef[0] } };
    let a = set[0].1.clone();
    let st = links::states(&set[1].1, &mut |_| Ok(a_now(&a)));
    assert_eq!(st[0].state, "ok");
    let mut a2 = a.clone();
    a2.tasks[0].dur = 6 * D;
    let st = links::states(&set[1].1, &mut |_| Ok(a_now(&a2)));
    assert_eq!(st[0].state, "changed");
}

// ---------- scenarios ----------

fn chain() -> Project {
    let mut p = plan("Plan", d(2026, 11, 2), &[5, 3, 2]);
    let ids: Vec<u32> = p.tasks.iter().map(|t| t.uid).collect();
    for (k, pr) in ["", "1", "2"].iter().enumerate() { p.tasks[k].preds = parse::preds(pr, &ids, D).unwrap() }
    p
}

// BUG: a task added to the plan after the scenario was made and a task added in the scenario get
// the same unique ID (both copies count on from the same `next_uid`). Merging "names" and "dur"
// then renames and resizes the plan's own new task with the scenario's, and merging "tasks" never
// brings the scenario's task in (its uid looks present already).
#[test]
fn merge_confuses_tasks_added_on_both_sides_after_the_copy() {
    let base = chain();
    let mut scen = scenario::make(&base, "What if", "Plan.dplan");
    let mut base = base;
    let t = base.new_task("Plan: permits", 0, 4 * D);
    base.tasks.push(t);
    let t = scen.new_task("Scenario: second crew", 0, D);
    scen.tasks.push(t);
    // Both new tasks got uid 4.
    let m1 = scenario::merge(&base, &scen, &["names".into(), "dur".into()]);
    assert_eq!((m1.tasks[3].name.as_str(), m1.tasks[3].dur), ("Plan: permits", 4 * D), "the plan's own new task was renamed/resized from an unrelated scenario task");
    let merged = scenario::merge(&base, &scen, &["names".into(), "dur".into(), "tasks".into()]);
    let names: Vec<&str> = merged.tasks.iter().map(|t| t.name.as_str()).collect();
    assert!(names.contains(&"Plan: permits") && names.contains(&"Scenario: second crew"), "got {names:?}");
}

// BUG: merging "tasks" deletes every task of the plan that the scenario does not have, including
// tasks added to the plan after the scenario was made (the scenario never deleted them).
#[test]
fn merge_tasks_deletes_plan_tasks_added_after_the_scenario() {
    let base = chain();
    let scen = scenario::make(&base, "What if", "Plan.dplan");
    let mut base = base;
    // The plan goes on: a new task (uid far above anything the scenario knows).
    let mut t = base.new_task("Plan: added later", 0, 2 * D);
    t.uid = 1000;
    base.next_uid = 1001;
    base.tasks.push(t);
    let merged = scenario::merge(&base, &scen, &["tasks".into()]);
    assert!(merged.tasks.iter().any(|t| t.uid == 1000), "the plan's own later task was deleted by the merge");
}

// Merge of tasks keeps the plan consistent: links to tasks the scenario deleted are gone.
#[test]
fn merge_tasks_drops_links_to_deleted_tasks() {
    let base = chain();
    let mut scen = scenario::make(&base, "Shorter", "Plan.dplan");
    scen.tasks.remove(1);
    let merged = scenario::merge(&base, &scen, &["tasks".into()]);
    assert_eq!(merged.tasks.len(), 2);
    let uids: Vec<u32> = merged.tasks.iter().map(|t| t.uid).collect();
    assert!(merged.tasks.iter().all(|t| t.preds.iter().all(|l| uids.contains(&l.pred))));
}

// A scenario task added as a child under a summary stays under it after the merge.
#[test]
fn merge_tasks_keeps_hierarchy_of_added_child() {
    let mut base = plan("Plan", d(2026, 11, 2), &[0, 2, 0, 3]);
    base.tasks[1].level = 1;
    base.tasks[3].level = 1;
    let mut scen = scenario::make(&base, "S", "Plan.dplan");
    let mut t = scen.new_task("new child", 1, D);
    t.level = 1;
    scen.tasks.insert(2, t); // last child of summary 0
    let merged = scenario::merge(&base, &scen, &["tasks".into()]);
    let v: Vec<(String, u8)> = merged.tasks.iter().map(|t| (t.name.clone(), t.level)).collect();
    assert_eq!(v[2], ("new child".to_string(), 1));
}

// ---------- copying tasks between plans ----------

// BUG: copying tasks from one plan and pasting them into another (the clipboard is shared by the
// tabs) keeps the resource uids of the first plan. In the second plan the same uid is another
// resource: the pasted task is silently assigned to the wrong person. Expected: matched by name
// (or dropped), never re-pointed at a different resource.
#[test]
fn paste_into_another_plan_keeps_foreign_resource_uids() {
    let mut a = Doc::new(Project::new("A", d(2026, 11, 2)));
    a.apply(Op::InsertRows { at: 0, rows: vec![vec![("name".into(), "Weld".into()), ("dur".into(), "2d".into()), ("res".into(), "Welder".into())]] }, F).unwrap();
    let mut b = Doc::new(Project::new("B", d(2026, 11, 2)));
    b.apply(Op::ResInsert { name: "Crane".into() }, F).unwrap();
    assert_eq!(a.p.resources[0].uid, b.p.resources[0].uid, "both plans number their first resource the same");
    let clip = doc::copy(&a.p, &[a.p.tasks[0].uid]);
    b.apply(Op::Paste { at: 0, tasks: clip }, F).unwrap();
    let t = &b.p.tasks[0];
    let names: Vec<&str> = t.res.iter().filter_map(|x| b.p.resources.iter().find(|r| r.uid == x.res)).map(|r| r.name.as_str()).collect();
    assert!(!names.contains(&"Crane"), "the pasted Weld task is now assigned to {names:?}");
}

// ---------- portfolio and the resource pool ----------

// BUG: the pool's last month bucket counts 31 days of capacity whatever the month, so a 30-day
// month ending the range gets a working day of the next month (and February three).
#[test]
fn pool_last_month_counts_the_month_only() {
    // One plan, one 10-day task Mon 16 - Fri 27 Nov 2026 by a full-time welder.
    let mut a = Doc::new(Project::new("Unit A", d(2026, 11, 16)));
    a.apply(Op::InsertRows { at: 0, rows: vec![vec![("name".into(), "Weld".into()), ("dur".into(), "10d".into()), ("res".into(), "Welder".into())]] }, F).unwrap();
    let o = portfolio::pool(&[("Unit A".into(), &a.p)], None, F, d(2026, 11, 1), "month");
    assert_eq!(o.buckets.len(), 1);
    // November 2026: 21 weekdays x 8 h = 168 h. Tue 1 Dec must not be counted.
    assert_eq!(o.rows[0].cap[0], 168.0);
}

// Pool load matches each plan's own work.
#[test]
fn pool_load_matches_plan_work() {
    let mut a = Doc::new(Project::new("Unit A", d(2026, 11, 2)));
    a.apply(Op::InsertRows { at: 0, rows: vec![vec![("name".into(), "Weld".into()), ("dur".into(), "3d".into()), ("res".into(), "Welder".into())]] }, F).unwrap();
    let o = portfolio::pool(&[("Unit A".into(), &a.p)], None, F, d(2026, 11, 1), "week");
    assert_eq!(o.rows[0].load.iter().sum::<f64>(), 24.0);
}

// Portfolio card: SPI is the plan's EV/PV (not an average of task SPIs); bf is a day after the last baseline day.
#[test]
fn card_finish_and_baseline_on_the_same_scale() {
    let mut p = plan("A", d(2026, 11, 2), &[5]);
    let s = sched::schedule(&p).unwrap();
    p.tasks[0].bls.insert(0, Baseline { start: s.es[0], finish: s.ef[0], dur: 5 * D, cost: 0.0, work: None });
    let c = portfolio::card(&p, F, d(2026, 11, 1), "x".into(), String::new());
    // On time: the window flags late when f > bf + 0.5.
    assert!(!(c.f > c.bf.unwrap() + 0.5), "f {} bf {:?}", c.f, c.bf);
    p.tasks[0].dur = 6 * D;
    let c = portfolio::card(&p, F, d(2026, 11, 1), "x".into(), String::new());
    assert!(c.f > c.bf.unwrap() + 0.5, "one day late is flagged: f {} bf {:?}", c.f, c.bf);
}

// The Subplans panel (shell `subplan_rows_in`) weighs % complete by each task's calendar span in
// minutes (nights and weekends included); the subplan's own project row (Calc::summary, used by
// the combined view) weighs by working duration. The same subplan shows two different figures.
// This repeats the shell's computation over the public API.
#[test]
fn subplan_panel_percent_matches_the_subplans_own_figure() {
    let mut p = plan("Sub", d(2026, 11, 2), &[1, 5]); // A Mon 2 (1d), B Mon 2 - Fri 6 (5d)
    p.tasks[1].pct = 100;
    let c = fields::Calc::new(&p, F, d(2026, 11, 20));
    let own = c.summary().pct; // 5/6 = 83.3 %
    assert!((own - 500.0 / 6.0).abs() < 0.01, "own {own}");
    // As in src-tauri/src/main.rs subplan_rows_in:
    let s = sched::schedule(&p).unwrap();
    let leaf: Vec<usize> = (0..p.tasks.len()).filter(|&i| !s.summary[i] && !s.off[i]).collect();
    let w = |i: usize| (s.ef[i] - s.es[i]).max(1) as f64;
    let total: f64 = leaf.iter().map(|&i| w(i)).sum();
    let panel = leaf.iter().map(|&i| w(i) * p.tasks[i].pct as f64).sum::<f64>() / total;
    assert!((panel - own).abs() < 0.5, "Subplans panel shows {panel:.1} %, the subplan itself {own:.1} %");
}

// ---------- apply setup to subplans ----------

// The master's print header names its picture 1; the subplan already has its own picture 1. The
// header must point at the master's picture under its new id, and the subplan keeps its title.
#[test]
fn setup_print_remaps_picture_ids() {
    let png = |c: &str| format!("data:image/png;base64,{c}");
    let mut m = Project::new("Master", d(2026, 11, 2));
    m.pictures = vec![Picture { id: 1, name: "Client".into(), role: "Client".into(), data: png("AAAA") }];
    m.ui = serde_json::json!({ "page": { "title": "Master title", "pics": { "left": [{ "v": "@pic:1", "h": 40 }] } } });
    let mut s = Project::new("Sub", d(2026, 11, 2));
    s.pictures = vec![Picture { id: 1, name: "Ours".into(), role: "Us".into(), data: png("BBBB") }];
    s.ui = serde_json::json!({ "page": { "title": "Sub title" } });
    setupshare::adopt(&mut s, &m, &["print".into()]).unwrap();
    let v = s.ui["page"]["pics"]["left"][0]["v"].as_str().unwrap().to_string();
    let id: u32 = v[5..].parse().unwrap();
    assert_eq!(s.pictures.iter().find(|x| x.id == id).unwrap().data, png("AAAA"));
    assert_eq!(s.ui["page"]["title"], "Sub title");
}

// The comment on `scenario::make` says a scenario "keeps no snapshots of the plan"; it keeps them all.
#[test]
fn scenario_keeps_no_snapshots() {
    let mut p = chain();
    p.snapshots.push(Snapshot { date: Some(d(2026, 11, 6)), label: "Week 1".into(), comment: String::new(), finish: 0, pct: 10.0, cost: 0.0, rows: vec![], health: None });
    let s = scenario::make(&p, "S", "Plan.dplan");
    assert!(s.snapshots.is_empty(), "the scenario carries {} snapshot(s) of the plan", s.snapshots.len());
}

#[allow(dead_code)]
fn _unused() -> (i64, NaiveDate) { (DAY, date_of(0)) }
