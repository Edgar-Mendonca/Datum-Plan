//! Audit of the analysis features (risk, delay, checks, milestones, levelling).
//! Failing tests prove bugs; passing ones record what was verified.
use chrono::NaiveDate;
use datum_core::{check, delay, doc::{Doc, Op}, fields::Calc, level, milestones, parse, risk, sched::schedule, *};

const D: i64 = 480;
const F: &str = "%Y-%m-%d";
fn d(y: i32, m: u32, day: u32) -> NaiveDate { NaiveDate::from_ymd_opt(y, m, day).unwrap() }

/// Tasks (name, days, preds) from Monday 2 November 2026.
fn plan(rows: &[(&str, i64, &str)]) -> Project {
    let mut p = Project::new("A", d(2026, 11, 2));
    for &(n, dur, _) in rows { let t = p.new_task(n, 0, dur * D); p.tasks.push(t) }
    let ids: Vec<u32> = p.tasks.iter().map(|t| t.uid).collect();
    for (k, &(_, _, pr)) in rows.iter().enumerate() { p.tasks[k].preds = parse::preds(pr, &ids, D).unwrap() }
    p
}
fn calc(p: &Project) -> Calc<'_> { Calc::new(p, F, d(2026, 11, 2)) }
fn chk<'a>(v: &'a [check::Check], k: &str) -> &'a check::Check { v.iter().find(|c| c.key == k).unwrap() }
fn opts(n: u32) -> risk::SimOpts { risk::SimOpts { iterations: n, ..Default::default() } }
/// The finish instant of a single task of `mins` working minutes from the plan start.
fn finish_after(mins: i64) -> i64 { let mut p = plan(&[("X", 0, "")]); p.tasks[0].dur = mins; schedule(&p).unwrap().finish }

// ---------------------------------------------------------------- check.rs

// DCMA check 1 (missing logic): every incomplete task needs a predecessor and a successor; only the
// project's start and finish are excepted. Four independent 5-day tasks have neither: at least 3 of
// the 4 (all 4 by the DCMA definition) must be counted. The engine excuses any task without a
// predecessor that starts on the project start, and any task without a successor that finishes on
// the project finish, so unlinked parallel tasks are never flagged.
#[test]
fn check_missing_logic_counts_unlinked_parallel_tasks() {
    let p = plan(&[("A", 5, ""), ("B", 5, ""), ("C", 5, ""), ("D", 5, "")]);
    let c = calc(&p);
    let v = check::run(&c, &check::Limits::default());
    let l = chk(&v, "logic");
    assert!(l.count >= 3 && !l.pass, "four unlinked tasks: count {} value {} pass {}", l.count, l.value, l.pass);
}

// Same check, with one open start in a linked plan: A -> C and B -> C, B has no predecessor and is
// not the project start activity (A is first in the plan). DCMA counts B... but at least the check
// must count *something* among two open starts; the engine counts nothing.
#[test]
fn check_missing_logic_counts_a_second_open_start() {
    let p = plan(&[("A", 5, ""), ("B", 3, ""), ("C", 2, "1,2")]);
    let v = check::run(&calc(&p), &check::Limits::default());
    // Two activities without predecessors: at most one is the project start.
    assert!(chk(&v, "logic").count >= 1, "count {}", chk(&v, "logic").count);
}

// DCMA checks 2 and 3 (leads, lags) are ratios of RELATIONSHIPS: links with a lag / all links
// (≤ 5 %). 20 tasks, two links, one with a 2-day lag: 1/2 = 50 % of the links -> fail. The engine
// divides successor tasks with a lag by all incomplete tasks: 1/20 = 5 % -> pass.
#[test]
fn check_lags_is_a_share_of_relationships() {
    let mut rows: Vec<(String, i64, String)> = (1..=20).map(|k| (format!("T{k}"), 2, String::new())).collect();
    rows[1].2 = "1FS+2d".into();
    rows[3].2 = "3".into();
    let r: Vec<(&str, i64, &str)> = rows.iter().map(|(a, b, c)| (a.as_str(), *b, c.as_str())).collect();
    let p = plan(&r);
    let v = check::run(&calc(&p), &check::Limits::default());
    let lags = chk(&v, "lags");
    assert_eq!(chk(&v, "rel").total, 2, "two links");
    assert!((lags.value - 50.0).abs() < 1e-9 && !lags.pass, "lags value {} (total {}) pass {}", lags.value, lags.total, lags.pass);
}

// BEI and missed tasks on a worked example: chain A(5) B(3) C(2), baseline, A done on time, status
// Wed 11 Nov: baseline finishes A Fri 6, B Wed 11 -> 2 due; done 1 -> BEI 0.5; missed: B (not done) = 1/2.
#[test]
fn check_bei_and_missed_worked_example() {
    let mut doc = Doc::new(plan(&[("A", 5, ""), ("B", 3, "1"), ("C", 2, "2")]));
    doc.apply(Op::Baseline { uids: vec![], clear: false, n: 0 }, F).unwrap();
    let uid = doc.p.tasks[0].uid;
    for (f, v) in [("astart", "2026-11-02"), ("afinish", "2026-11-06")] { doc.apply(Op::Set { uid, field: f.into(), value: v.into() }, F).unwrap() }
    doc.apply(Op::ProjectSet { field: "status".into(), value: serde_json::json!("2026-11-11") }, F).unwrap();
    let v = check::run(&calc(&doc.p), &check::Limits::default());
    let bei = chk(&v, "bei");
    assert_eq!((bei.count, bei.total), (1, 2));
    assert!((bei.value - 0.5).abs() < 1e-9);
    let m = chk(&v, "missed");
    assert_eq!((m.count, m.total), (1, 2));
}

// ---------------------------------------------------------------- milestones.rs

// "At risk with `warn` working days of float or less" (doc comment of `list`). A milestone with 3
// days of float (B 7 d -> M, A 10 d -> End, M -> End) and warn = 5 is at risk. The engine also
// requires the milestone to be critical (float <= 0), so any positive float <= warn is "On track".
#[test]
fn milestone_with_float_below_warn_is_at_risk() {
    let mut doc = Doc::new(plan(&[("A", 10, ""), ("B", 7, ""), ("M", 0, "2"), ("End", 0, "1,3")]));
    doc.apply(Op::Baseline { uids: vec![], clear: false, n: 0 }, F).unwrap();
    let o = milestones::list(&calc(&doc.p), 0, 5);
    let m = o.rows.iter().find(|r| r.name == "M").unwrap();
    assert_eq!(m.float_days, Some(3));
    assert_eq!(m.status, "At risk", "{m:?}");
    assert_eq!(o.at_risk, 2, "M (3 days of float) and End (none, on the critical path)");
}

// Variance in calendar days: End's baseline Fri 13 Nov; A extended by 2 working days moves it to
// Tue 17 Nov = +4 calendar days; Late.
#[test]
fn milestone_variance_in_calendar_days() {
    let mut doc = Doc::new(plan(&[("A", 10, ""), ("End", 0, "1")]));
    doc.apply(Op::Baseline { uids: vec![], clear: false, n: 0 }, F).unwrap();
    let uid = doc.p.tasks[0].uid;
    doc.apply(Op::Set { uid, field: "dur".into(), value: "12d".into() }, F).unwrap();
    let o = milestones::list(&calc(&doc.p), 0, 5);
    let m = o.rows.iter().find(|r| r.name == "End").unwrap();
    assert_eq!((m.var, m.status.as_str(), m.forecast), (Some(4), "Late", Some(d(2026, 11, 17))));
}

// ---------------------------------------------------------------- delay.rs

// Collapsed as-built with a hold: Design (5 d) -> Build (3 d) -> Test (2 d). Event: Build could not
// start before Thu 12 Nov. As built: Design 2–6 Nov, Build 12–16 Nov (3 days, Thu Fri Mon), so the
// hold cost 3 working days (Mon 9, Tue 10, Wed 11). Taking the hold out must save those 3 days.
// as_built() keeps the gap that happened as a lag on Build's driving predecessor, so lifting the
// SNET constraint changes nothing: the engine reports 0.
#[test]
fn collapsed_as_built_takes_a_hold_out() {
    let mut doc = Doc::new(plan(&[("Design", 5, ""), ("Build", 3, "1"), ("Test", 2, "2")]));
    let (design, build) = (doc.p.tasks[0].uid, doc.p.tasks[1].uid);
    let set = |doc: &mut Doc, uid: u32, f: &str, v: &str| doc.apply(Op::Set { uid, field: f.into(), value: v.into() }, F).unwrap();
    set(&mut doc, design, "astart", "2026-11-02");
    set(&mut doc, design, "afinish", "2026-11-06");
    set(&mut doc, build, "astart", "2026-11-12");
    set(&mut doc, build, "afinish", "2026-11-16");
    doc.apply(Op::ProjectSet { field: "status".into(), value: serde_json::json!("2026-11-16") }, F).unwrap();
    let e = DelayEvent { name: "Site closed".into(), date: Some(d(2026, 11, 9)), on: true, hold: vec![(build, d(2026, 11, 12))], ..Default::default() };
    doc.apply(Op::Delay { id: 0, ev: Some(e) }, F).unwrap();
    let c = calc(&doc.p);
    let r = delay::cab(&c).unwrap();
    assert!(r.fidelity.as_deref().unwrap().contains("(0 working days apart)"), "{:?}", r.fidelity);
    assert_eq!(r.events.iter().map(|e| e.impact).collect::<Vec<_>>(), [3.0], "notes {:?}", r.events.iter().map(|e| &e.notes).collect::<Vec<_>>());
}

// Time impact of the same hold on the unprogressed plan: Build planned Mon 9; held to Thu 12 = +3.
#[test]
fn time_impact_of_a_hold() {
    let mut doc = Doc::new(plan(&[("Design", 5, ""), ("Build", 3, "1"), ("Test", 2, "2")]));
    let build = doc.p.tasks[1].uid;
    let e = DelayEvent { name: "Site closed".into(), date: Some(d(2026, 11, 9)), on: true, hold: vec![(build, d(2026, 11, 12))], ..Default::default() };
    doc.apply(Op::Delay { id: 0, ev: Some(e) }, F).unwrap();
    let r = delay::tia(&calc(&doc.p)).unwrap();
    assert_eq!(r.events[0].impact, 3.0);
    assert_eq!(r.total, 3.0);
}

// ---------------------------------------------------------------- risk.rs

// A lump-sum cost (a Cost resource, 10,000) does not depend on how long the task takes, yet the
// simulation scales every non-fixed cost with the drawn duration: cost P80 > 10,000. Expected:
// P50 = P80 = 10,000, and the chance of meeting the plan's cost 100 %.
#[test]
fn risk_lump_sum_cost_does_not_scale_with_duration() {
    let mut p = plan(&[("A", 10, "")]);
    p.resources.push(Resource { uid: 1, name: "Permit fee".into(), kind: ResKind::Cost, ..Default::default() });
    p.tasks[0].res = vec![Assign { res: 1, units: 10_000.0, table: 0 }];
    let r = risk::simulate(&p, &opts(500)).unwrap();
    assert_eq!(r.cost, 10_000.0);
    assert_eq!((r.cost_p50, r.cost_p80, r.cost_chance), (10_000.0, 10_000.0, 1.0), "a fee is the same however long the task takes");
}

// Same for material: 50 t at 100 = 5,000 whatever the duration.
#[test]
fn risk_material_cost_does_not_scale_with_duration() {
    let mut p = plan(&[("A", 10, "")]);
    p.resources.push(Resource { uid: 1, name: "Steel".into(), kind: ResKind::Material, rate: 100.0, ..Default::default() });
    p.tasks[0].res = vec![Assign { res: 1, units: 50.0, table: 0 }];
    let r = risk::simulate(&p, &opts(500)).unwrap();
    assert_eq!(r.cost, 5_000.0);
    assert_eq!((r.cost_p50, r.cost_p80), (5_000.0, 5_000.0));
}

// One 10-day task, triangular −10 %/+25 %: a = 9, m = 10, b = 12.5 days. c = (m−a)/(b−a) = 1/3.5.
// Quantile q >= c: b − sqrt((b−a)(b−m)(1−q)). P50 = 12.5 − sqrt(8.75·0.5) = 10.4083 d,
// P80 = 12.5 − sqrt(8.75·0.2) = 11.1771 d, P10 (q < c): a + sqrt((b−a)(m−a)·q) = 9 + sqrt(0.35) = 9.5916 d.
// P(X <= 10) = c = 0.2857. 5000 runs: SE(P50) ≈ 1/(2 f √n), f(P50) ≈ 0.478/day -> 0.015 d ≈ 7 min.
#[test]
fn risk_triangular_quantiles_match_theory() {
    let p = plan(&[("A", 10, "")]);
    let r = risk::simulate(&p, &opts(5000)).unwrap();
    let q = |days: f64| finish_after((days * D as f64).round() as i64);
    let tol = 40; // minutes
    for (got, want, name) in [(r.p10, q(9.5916), "P10"), (r.p50, q(10.4083), "P50"), (r.p80, q(11.1771), "P80")] {
        assert!((got - want).abs() <= tol, "{name}: got {got} want {want}");
    }
    assert!((r.planned_chance - 1.0 / 3.5).abs() < 0.03, "{}", r.planned_chance);
    assert_eq!(r.criticality[0].value, 1.0);
}

// Uniform −20/+20 on 10 days: P10 = 8.4 d, P90 = 11.6 d.
#[test]
fn risk_uniform_quantiles_match_theory() {
    let p = plan(&[("A", 10, "")]);
    let r = risk::simulate(&p, &risk::SimOpts { iterations: 5000, dist: "uniform".into(), low: 20.0, high: 20.0, ..Default::default() }).unwrap();
    let q = |days: f64| finish_after((days * D as f64).round() as i64);
    assert!((r.p10 - q(8.4)).abs() <= 40 && (r.p90 - q(11.6)).abs() <= 40, "{} {} / {} {}", r.p10, q(8.4), r.p90, q(11.6));
}

// Two identical parallel 10-day paths into a milestone: the finish is the max of two draws; each path
// is critical in about half the runs; P(max <= 10 d) = c² = 0.0816.
#[test]
fn risk_parallel_paths_criticality_and_merge_bias() {
    let p = plan(&[("A", 10, ""), ("B", 10, ""), ("End", 0, "1,2")]);
    let r = risk::simulate(&p, &opts(4000)).unwrap();
    let c = |n: &str| r.criticality.iter().find(|t| t.name == n).map_or(0.0, |t| t.value);
    assert!((c("A") - 0.5).abs() < 0.05 && (c("B") - 0.5).abs() < 0.05, "{} {}", c("A"), c("B"));
    assert!((r.planned_chance - (1.0f64 / 3.5).powi(2)).abs() < 0.02, "{}", r.planned_chance);
    // Drivers: corr(X_A, max(X_A, X_B)) is the same for both and well above 0.
    assert!(r.drivers.len() == 2 && (r.drivers[0].value - r.drivers[1].value).abs() < 0.08);
}

// A risk with 30 % chance adding 5 days to a 5-day task, no ranges: P50 = plan, P80 = plan + 5 d,
// rate ≈ 0.3.
#[test]
fn risk_event_percentiles() {
    let mut p = plan(&[("A", 5, "")]);
    p.risks.push(Risk { id: 1, name: "R".into(), pct: 30.0, status: "Open".into(), tasks: vec![p.tasks[0].uid], kind: "Threat".into(), days: 5.0, ..Default::default() });
    let r = risk::simulate(&p, &risk::SimOpts { iterations: 4000, low: 0.0, high: 0.0, ..Default::default() }).unwrap();
    assert_eq!((r.p50, r.p80), (r.planned, finish_after(10 * D)));
    assert!((r.risks[0].value - 0.3).abs() < 0.03);
    assert!((r.planned_chance - 0.7).abs() < 0.03);
}

// ---------------------------------------------------------------- level.rs

// Two 5-day tasks on one full-time person: levelling moves the second by 5 days; the finish moves
// from Fri 6 to Fri 13 Nov; 5 days over before, none after.
#[test]
fn level_preview_two_tasks_one_person() {
    let mut p = plan(&[("A", 5, ""), ("B", 5, "")]);
    p.resources.push(Resource { uid: 1, name: "Fitter".into(), max: 1.0, ..Default::default() });
    for t in &mut p.tasks { t.res = vec![Assign { res: 1, units: 1.0, table: 0 }] }
    let pv = level::preview(&p, &[]).unwrap();
    assert_eq!(pv.moved.len(), 1);
    assert_eq!(pv.moved[0].name, "B");
    assert_eq!(pv.new_finish, finish_after(10 * D));
    assert_eq!(pv.moved[0].new_start, pv.moved[0].start + 7 * 1440, "a calendar week later");
    assert_eq!((pv.over_before, pv.over_after), (5, 0));
}

// ---------------------------------------------------------------- in-progress tasks

/// Design (10 d) -> Test (2 d); Design started Mon 2 Nov, 50 % complete, status Fri 6 Nov.
fn under_way() -> Doc {
    let mut doc = Doc::new(plan(&[("Design", 10, ""), ("Test", 2, "1")]));
    let uid = doc.p.tasks[0].uid;
    for (f, v) in [("astart", "2026-11-02"), ("pct", "50")] { doc.apply(Op::Set { uid, field: f.into(), value: v.into() }, F).unwrap() }
    doc.apply(Op::ProjectSet { field: "status".into(), value: serde_json::json!("2026-11-06") }, F).unwrap();
    doc
}

// Time impact: an event adds 2 working days to Design, which is under way. The finish must move 2
// working days. add() does `dur += m` while the scheduler keeps `pct` and plans the remaining work
// as dur·(1 − pct), so only 2·(1 − 0.5) = 1 day is added.
#[test]
fn time_impact_extension_on_a_task_under_way() {
    let mut doc = under_way();
    let design = doc.p.tasks[0].uid;
    let e = DelayEvent { name: "Late data".into(), date: Some(d(2026, 11, 6)), on: true, extend: vec![(design, 2 * D)], ..Default::default() };
    doc.apply(Op::Delay { id: 0, ev: Some(e) }, F).unwrap();
    let r = delay::tia(&calc(&doc.p)).unwrap();
    assert_eq!(r.events[0].impact, 2.0, "an extension of 2 working days");
}

// Risk: a certain risk (100 %) adding 5 working days to Design, under way, no duration ranges. Every
// run must finish 5 working days after the plan (the same as adding 5 days to the unstarted Test).
// risk.rs adds the delay to `dur`, which the scheduler scales by (1 − pct): only 2.5 days.
#[test]
fn risk_event_on_a_task_under_way() {
    let mut doc = under_way();
    let design = doc.p.tasks[0].uid;
    doc.p.risks.push(Risk { id: 1, name: "R".into(), pct: 100.0, status: "Open".into(), tasks: vec![design], kind: "Threat".into(), days: 5.0, ..Default::default() });
    let r = risk::simulate(&doc.p, &risk::SimOpts { iterations: 50, low: 0.0, high: 0.0, ..Default::default() }).unwrap();
    let mut q = doc.p.clone();
    q.risks.clear();
    q.tasks[1].dur += 5 * D;
    let want = schedule(&q).unwrap().finish;
    assert_eq!(r.p50, want, "5 working days later");
}
