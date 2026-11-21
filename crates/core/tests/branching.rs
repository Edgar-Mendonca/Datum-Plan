//! Probabilistic branching (0.10.0): a risk whose activities (a fragnet) happen only in the runs
//! where it does. Plans start Monday 2 Nov 2026 on the standard calendar (8 h, Mon–Fri).
use chrono::NaiveDate;
use datum_core::{cal::{at, DAY}, risk::{simulate, SimOpts}, *};

fn d(y: i32, m: u32, day: u32) -> NaiveDate { NaiveDate::from_ymd_opt(y, m, day).unwrap() }
/// Durations exactly as planned (no ranges), so only the branch varies.
fn opts(runs: u32) -> SimOpts { SimOpts { iterations: runs, low: 0.0, high: 0.0, seed: 7, ..Default::default() } }
/// Tasks (working days, predecessors by row number).
fn plan(rows: &[(i64, &str)]) -> Project {
    let mut p = Project::new("B", d(2026, 11, 2));
    for (i, &(days, _)) in rows.iter().enumerate() { let t = p.new_task(&format!("T{}", i + 1), 0, days * 480); p.tasks.push(t) }
    let ids: Vec<u32> = p.tasks.iter().map(|t| t.uid).collect();
    for (t, &(_, pr)) in p.tasks.iter_mut().zip(rows) { t.preds = parse::preds(pr, &ids, 480).unwrap() }
    p
}
fn step(name: &str, days: f64, cost: f64) -> BranchStep { BranchStep { name: name.into(), days, cost, ..Default::default() } }
fn ends(day: NaiveDate) -> i64 { at(day) + 17 * 60 }

/// T1 (5 d) → T2 (3 d) finishes Wed 11 Nov. A certain rework branch after T1 (2 d, then 1 d of
/// retest) puts 3 working days before T2 in every run: Mon 16 Nov.
#[test]
fn a_certain_branch_is_in_every_run() {
    let mut p = plan(&[(5, ""), (3, "1")]);
    p.risks.push(Risk { id: 1, name: "Weld rework".into(), status: "Open".into(), kind: "Threat".into(), pct: 100.0, after: p.tasks[0].uid,
        branch: vec![step("Rework", 2.0, 0.0), step("Retest", 1.0, 0.0)], ..Default::default() });
    let r = simulate(&p, &opts(50)).unwrap();
    assert_eq!(r.planned, ends(d(2026, 11, 11)));
    assert_eq!((r.p10, r.p50, r.p90), (ends(d(2026, 11, 16)), ends(d(2026, 11, 16)), ends(d(2026, 11, 16))));
    assert_eq!((r.risks[0].value, r.risks[0].effect), (1.0, None), "in every run, so nothing to compare with");
}

/// At 50 %, about half the runs take the branch. Those finish 5 calendar days later (Mon 16 instead
/// of Wed 11, over the weekend); its cost (2,000) shows in the upper cost percentile.
#[test]
fn a_likely_branch_and_its_effect() {
    let mut p = plan(&[(5, ""), (3, "1")]);
    p.risks.push(Risk { id: 1, name: "Weld rework".into(), status: "Open".into(), kind: "Threat".into(), pct: 50.0, after: p.tasks[0].uid,
        branch: vec![step("Rework", 2.0, 1500.0), step("Retest", 1.0, 500.0)], ..Default::default() });
    let r = simulate(&p, &opts(400)).unwrap();
    let share = r.risks[0].value;
    assert!((0.4..0.6).contains(&share), "about half: {share}");
    assert_eq!(r.risks[0].effect, Some(5.0));
    assert_eq!((r.p10, r.p90), (ends(d(2026, 11, 11)), ends(d(2026, 11, 16))));
    assert!((r.cost_p80 - r.cost - 2000.0).abs() < 1e-6, "{} vs {}", r.cost_p80, r.cost);
    // Closed, it is left out.
    p.risks[0].status = "Closed".into();
    let r = simulate(&p, &opts(100)).unwrap();
    assert!(r.risks.is_empty() && r.p90 == ends(d(2026, 11, 11)));
}

/// A branch that holds up one task only: T1 → T2 (3 d) and T1 → T3 (1 d); the branch (4 d) after
/// T1 holds T3: T3 ends Fri 13 Nov, later than T2 (Wed 11 Nov), so the finish moves to Fri 13.
#[test]
fn a_branch_before_one_task() {
    let mut p = plan(&[(5, ""), (3, "1"), (1, "1")]);
    p.risks.push(Risk { id: 1, name: "Late spool".into(), status: "Open".into(), kind: "Threat".into(), pct: 100.0, after: p.tasks[0].uid, before: p.tasks[2].uid,
        branch: vec![step("Re-fabricate", 4.0, 0.0)], ..Default::default() });
    let r = simulate(&p, &opts(20)).unwrap();
    assert_eq!(r.p50, ends(d(2026, 11, 13)));
    // Without the target, it holds T2 as well: T2 then runs Fri 13, Mon 16, Tue 17.
    p.risks[0].before = 0;
    assert_eq!(simulate(&p, &opts(20)).unwrap().p50, ends(d(2026, 11, 17)));
    let _ = DAY;
}

/// A branch step with a range draws its days between the least and the most.
#[test]
fn a_branch_step_with_a_range() {
    let mut p = plan(&[(5, ""), (3, "1")]);
    p.risks.push(Risk { id: 1, name: "Weld rework".into(), status: "Open".into(), kind: "Threat".into(), pct: 100.0, after: p.tasks[0].uid,
        branch: vec![BranchStep { name: "Rework".into(), days: 2.0, days_min: 1.0, days_max: 6.0, ..Default::default() }], ..Default::default() });
    let r = simulate(&p, &opts(300)).unwrap();
    // 1 to 6 extra days after Fri 6 Nov: T2 ends between Thu 12 and Thu 19 Nov.
    assert!(r.p10 >= ends(d(2026, 11, 12)) && r.p90 <= ends(d(2026, 11, 19)) && r.p10 < r.p90, "{} {}", r.p10, r.p90);
}
