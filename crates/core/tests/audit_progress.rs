//! Audit of progress, cost and earned-value figures against hand calculations.
//! Every plan starts Monday 2 Nov 2026 on the standard calendar (8-12, 13-17, Mon-Fri; 480 min/day).
//! Failing tests here are confirmed bugs (see the comment above each for the hand-worked figure).
use chrono::NaiveDate;
use datum_core::{doc::{Doc, Op}, fields::Calc, *};

fn d(y: i32, m: u32, day: u32) -> NaiveDate { NaiveDate::from_ymd_opt(y, m, day).unwrap() }
const F: &str = "%Y-%m-%d";
const DAYM: i64 = 1440;

fn doc(rows: &[&str]) -> Doc {
    let mut p = Project::new("A", d(2026, 11, 2));
    for i in 0..rows.len() { let t = p.new_task(&format!("T{}", i + 1), 0, 0); p.tasks.push(t) }
    let mut doc = Doc::new(p);
    for (i, r) in rows.iter().enumerate() {
        let mut parts = r.split('|');
        let (dur, preds, extra) = (parts.next().unwrap_or(""), parts.next().unwrap_or(""), parts.next().unwrap_or(""));
        let uid = doc.p.tasks[i].uid;
        for (k, v) in [("dur", dur), ("preds", preds)].into_iter().chain(extra.split(';').filter(|s| !s.is_empty()).map(|kv| kv.split_once('=').unwrap_or((kv, "")))) {
            doc.apply(Op::Set { uid, field: k.into(), value: v.into() }, F).unwrap_or_else(|e| panic!("row {} {k}={v}: {e}", i + 1));
        }
    }
    doc
}
fn op(doc: &mut Doc, o: Op) { doc.apply(o, F).unwrap_or_else(|e| panic!("{e}")) }
fn set(doc: &mut Doc, k: usize, f: &str, v: &str) { let uid = doc.p.tasks[k].uid; op(doc, Op::Set { uid, field: f.into(), value: v.into() }) }
fn baseline(doc: &mut Doc) { op(doc, Op::Baseline { uids: vec![], clear: false, n: 0 }) }
fn status(doc: &mut Doc, s: &str) { op(doc, Op::ProjectSet { field: "status".into(), value: serde_json::json!(s) }) }
fn calc(doc: &Doc) -> Calc<'static> { Calc::new(Box::leak(Box::new(doc.p.clone())), F, d(2026, 11, 2)) }
fn close(a: f64, b: f64) -> bool { (a - b).abs() < 1e-6 }
fn num(c: &Calc, i: usize, k: &str) -> f64 { c.val(i, k).num() }
fn add_res(doc: &mut Doc, r: Resource) { doc.p.resources.push(r) }
fn worker(uid: u32, name: &str, rate: f64) -> Resource { Resource { uid, name: name.into(), max: 1.0, rate, ..Default::default() } }
/// Day number (days since 1970-01-01) of a date.
fn dn(x: NaiveDate) -> i64 { cal::at(x) / DAYM }
/// Work minutes of task `i` per day.
fn daily(c: &Calc, i: usize) -> Vec<(NaiveDate, f64)> {
    c.l.task_daily.range((i, i64::MIN)..=(i, i64::MAX)).filter(|x| x.1 .0 > 0.0).map(|(&(_, dd), &(m, _))| (cal::date_of(dd * DAYM), m)).collect()
}

// ---------------------------------------------------------------- splits

/// A 2-day task (R at 100 %) split after 1 day with a 1-day pause: Mon work, Tue pause, Wed work.
/// Hand: work 16 h; daily loads Mon 480, Tue 0, Wed 480 min.
/// The engine spreads the 16 h evenly over Mon–Wed (320 min a day, work booked inside the pause).
#[test]
fn split_task_books_no_work_in_the_pause() {
    let mut x = doc(&["2d"]);
    add_res(&mut x, worker(1, "R", 10.0));
    set(&mut x, 0, "res", "R");
    let uid = x.p.tasks[0].uid;
    op(&mut x, Op::Split { uid, after: 480, gap: 480 });
    let c = calc(&x);
    assert!(close(num(&c, 0, "work"), 16.0));
    let dl = daily(&c, 0);
    assert_eq!(dl, vec![(d(2026, 11, 2), 480.0), (d(2026, 11, 4), 480.0)], "no work on Tue (the pause)");
}

/// Update Project through Tue 3 Nov on that split task: only Monday's day of work (of 2) is done.
/// Hand: 50 %. The engine counts the pause as work done: 960/1440 = 66 %.
#[test]
fn update_project_does_not_count_a_pause_as_work_done() {
    let mut x = doc(&["2d"]);
    let uid = x.p.tasks[0].uid;
    op(&mut x, Op::Split { uid, after: 480, gap: 480 });
    op(&mut x, Op::UpdateProject { through: "2026-11-03".into(), reschedule: false });
    assert_eq!(x.p.tasks[0].pct, 50);
}

// ---------------------------------------------------------------- level of effort

/// LOE T2 (typed 1d) spans T1 (10d) through an FF link: it is shown as 10 days long.
/// Hand: R at 100 % for 10 days = 80 h. The engine books 1d x 100 % = 8 h (the typed duration).
#[test]
fn level_of_effort_work_follows_its_span() {
    let mut x = doc(&["10d|2FF", "1d||acttype=Level of Effort"]);
    add_res(&mut x, worker(1, "R", 10.0));
    set(&mut x, 1, "res", "R");
    let c = calc(&x);
    assert!(close(num(&c, 1, "dur"), 4800.0), "LOE spans 10 days: {}", num(&c, 1, "dur"));
    assert!(close(num(&c, 1, "work"), 80.0), "work = 10 d x 8 h x 100 % = 80 h, got {}", num(&c, 1, "work"));
    assert!(close(num(&c, 1, "cost"), 800.0), "cost = 80 h x $10, got {}", num(&c, 1, "cost"));
}

// ---------------------------------------------------------------- rates

/// Table A has a row (as saved by the Resource Information dialog: [{from: null, std: rate}]).
/// Then the Std. Rate column is changed from 10 to 20/h. Hand: a 1-day task costs 8 h x 20 = 160.
/// The engine keeps pricing from table A's stale row: 80, while the Std. Rate column shows 20.
#[test]
fn std_rate_edit_still_prices_work_after_table_a_was_saved() {
    let mut x = doc(&["1d"]);
    add_res(&mut x, worker(1, "R", 10.0));
    set(&mut x, 0, "res", "R");
    op(&mut x, Op::ResJson { uid: 1, field: "tables".into(), value: serde_json::json!([[{ "from": null, "std": 10.0, "ovt": 0.0, "per_use": 0.0 }]]) });
    op(&mut x, Op::ResSet { uid: 1, field: "rate".into(), value: "20".into() });
    let c = calc(&x);
    assert!(close(x.p.resources[0].rate, 20.0));
    assert!(close(num(&c, 0, "cost"), 160.0), "cost {}", num(&c, 0, "cost"));
}

/// Rate table rows entered out of date order: [{from 4 Nov, 20/h}, {from none, 10/h}].
/// A 5-day task Mon 2 – Fri 6 Nov. Hand: Mon, Tue at 10 (160), Wed–Fri at 20 (480) = 640.
/// The engine takes the last row whose date has passed in list order (the undated one): 400.
#[test]
fn rate_table_rows_out_of_date_order() {
    let mut x = doc(&["5d"]);
    add_res(&mut x, worker(1, "R", 10.0));
    set(&mut x, 0, "res", "R");
    op(&mut x, Op::ResJson { uid: 1, field: "tables".into(), value: serde_json::json!([[{ "from": "2026-11-04", "std": 20.0 }, { "from": null, "std": 10.0 }]]) });
    let c = calc(&x);
    assert!(close(num(&c, 0, "cost"), 640.0), "cost {}", num(&c, 0, "cost"));
}

/// Rate table sorted, the rate changes on Wed 4 Nov: 2 days at 10, 3 at 20 = 160 + 480 = 640 (checks the happy path).
#[test]
fn rate_table_change_mid_task_is_phased_by_day() {
    let mut x = doc(&["5d"]);
    add_res(&mut x, worker(1, "R", 10.0));
    set(&mut x, 0, "res", "R");
    op(&mut x, Op::ResJson { uid: 1, field: "tables".into(), value: serde_json::json!([[{ "from": null, "std": 10.0 }, { "from": "2026-11-04", "std": 20.0 }]]) });
    let c = calc(&x);
    assert!(close(num(&c, 0, "cost"), 640.0), "cost {}", num(&c, 0, "cost"));
}

/// Per-use cost accrues at the start of the assignment (it is spent once the task starts).
/// T1 2d, R $0/h + $100 per use, 50 % done at the end of Mon 2 Nov. Hand: actual cost 100.
/// The engine prorates it by % complete: 50 — while its own cost curve puts the full 100 on day 1.
#[test]
fn per_use_cost_is_actual_once_started() {
    let mut x = doc(&["2d"]);
    add_res(&mut x, Resource { uid: 1, name: "R".into(), max: 1.0, per_use: 100.0, ..Default::default() });
    set(&mut x, 0, "res", "R");
    set(&mut x, 0, "pct", "50");
    status(&mut x, "2026-11-02");
    let c = calc(&x);
    assert!(close(num(&c, 0, "cost"), 100.0));
    assert!(close(num(&c, 0, "acost"), 100.0), "actual cost {}", num(&c, 0, "acost"));
}

// ---------------------------------------------------------------- usage grid, over-allocation

/// R on T1 (Mon) alone, then on T2 and T3 both on Tue: 16 h against 8 h on Tue: over-allocated.
/// A weekly Resource Usage grid has one bucket (that week): its over flag must be set.
/// The engine checks only the first day of the last bucket (`unwrap_or(d0 + 1)`).
#[test]
fn usage_over_flag_checks_every_day_of_the_last_bucket() {
    let mut x = doc(&["1d", "1d|1", "1d|1"]);
    add_res(&mut x, worker(1, "R", 10.0));
    for k in 0..3 { set(&mut x, k, "res", "R") }
    let c = calc(&x);
    assert!(c.l.over[0], "the engine's own flag: R is over on Tue");
    let u = report::usage(&c, "resource", "work", "week");
    let r = u.rows.iter().find(|r| r.of.is_none() && r.uid == 1).unwrap();
    assert_eq!(u.buckets.len(), 1);
    assert!(r.over[0], "over in week 1 (Tue 16 h of 8 h)");
    // The Workload view counts the same flags.
    let w = workload::workload(&c, "week");
    assert_eq!((w.over_people, w.over_cells), (1, 1));
}

/// A task that actually started a week before the plan's start date (progress entered late):
/// 5d, act start Mon 26 Oct, 100 % (finished Fri 30 Oct), R at 100 %: 40 h.
/// Hand: the Task Usage grid's row total = the task's work = 40 h. The engine's buckets start at
/// the project start, so the hours before it are dropped.
#[test]
fn usage_grid_keeps_work_before_the_project_start() {
    let mut x = doc(&["5d", "1d|1"]);
    add_res(&mut x, worker(1, "R", 10.0));
    set(&mut x, 0, "res", "R");
    set(&mut x, 0, "astart", "2026-10-26");
    set(&mut x, 0, "afinish", "2026-10-30");
    set(&mut x, 0, "pct", "100");
    let c = calc(&x);
    assert!(close(num(&c, 0, "work"), 40.0));
    let u = report::usage(&c, "task", "work", "week");
    let row = u.rows.iter().find(|r| r.uid == x.p.tasks[0].uid && r.of.is_none()).unwrap();
    assert!(close(row.total, 40.0), "usage total {} vs work 40", row.total);
}

// ---------------------------------------------------------------- resource usage cost

/// T1 1d: R work $10/h + $50 per use, M material 10 units at $5. Hand: task cost 80 + 50 + 50 = 180;
/// Resource Usage (cost) R = 130, M = 50, the two rows adding up to the task's 180.
/// The engine prices the usage cost grid from work hours x rate only: R 80, M 0 (no row values).
#[test]
fn resource_usage_cost_includes_per_use_and_material() {
    let mut x = doc(&["1d"]);
    add_res(&mut x, Resource { uid: 1, name: "R".into(), max: 1.0, rate: 10.0, per_use: 50.0, ..Default::default() });
    add_res(&mut x, Resource { uid: 2, name: "M".into(), kind: ResKind::Material, rate: 5.0, ..Default::default() });
    set(&mut x, 0, "res", "R, M[10]");
    let c = calc(&x);
    assert!(close(num(&c, 0, "cost"), 180.0), "task cost {}", num(&c, 0, "cost"));
    assert!(close(c.l.res_cost[0], 130.0) && close(c.l.res_cost[1], 50.0), "{:?}", c.l.res_cost);
    let u = report::usage(&c, "resource", "cost", "day");
    let tot = |uid: u32| u.rows.iter().find(|r| r.of.is_none() && r.uid == uid).unwrap().total;
    assert!(close(tot(1), 130.0) && close(tot(2), 50.0), "R {} M {}", tot(1), tot(2));
    // Task Usage (cost): the assignment rows under T1 add up to T1's row.
    let t = report::usage(&c, "task", "cost", "day");
    let row = t.rows.iter().find(|r| r.of.is_none()).unwrap().total;
    let kids: f64 = t.rows.iter().filter(|r| r.of.is_some()).map(|r| r.total).sum();
    assert!(close(row, 180.0) && close(kids, row), "task row {row}, assignments {kids}");
}

// ---------------------------------------------------------------- earned value

/// T1 10d, R $10/h: BAC 800. Baseline, then 20 % done, an expense of $100 actually spent (none
/// planned), status end of Wed 4 Nov (3 of 10 days). Hand (PMI):
/// PV = 800 x 3/10 = 240; EV = 800 x 20 % = 160; AC = 160 + 100 = 260; SV = -80; CV = -100;
/// SPI = 0.6667; CPI = 160/260 = 0.6154; EAC = AC + (BAC-EV)/CPI = 260 + 640 x 260/160 = 1300;
/// VAC = -500; TCPI = (800-160)/(800-260) = 1.1852.
#[test]
fn earned_value_figures_of_one_task() {
    let mut x = doc(&["10d"]);
    add_res(&mut x, worker(1, "R", 10.0));
    set(&mut x, 0, "res", "R");
    baseline(&mut x);
    set(&mut x, 0, "pct", "20");
    let uid = x.p.tasks[0].uid;
    op(&mut x, Op::TaskList { uid, field: "expenses".into(), value: serde_json::json!([{ "name": "Hire", "planned": 0.0, "actual": 100.0, "accrual": "Start" }]) });
    status(&mut x, "2026-11-04");
    let c = calc(&x);
    let v = |k: &str| num(&c, 0, k);
    assert!(close(v("bac"), 800.0) && close(v("bcws"), 240.0) && close(v("bcwp"), 160.0) && close(v("acwp"), 260.0), "bac {} pv {} ev {} ac {}", v("bac"), v("bcws"), v("bcwp"), v("acwp"));
    assert!(close(v("sv"), -80.0) && close(v("cv"), -100.0));
    assert!(close(v("spi"), 160.0 / 240.0) && close(v("cpi"), 160.0 / 260.0));
    assert!(close(v("eac"), 1300.0) && close(v("vac"), -500.0) && close(v("tcpi"), 640.0 / 540.0), "eac {} vac {} tcpi {}", v("eac"), v("vac"), v("tcpi"));
}

/// T1 10d with a fixed cost of 1,000 charged at the start (accrual Start); baselined; started on
/// time and 10 % done (on plan) at the status date, end of Mon 2 Nov. The budget is spent on day 1, so the planned value
/// there is 1,000 (time-phased baseline). Hand: PV = 1000 and the planned cost curve on day 1 =
/// the forecast cost curve on day 1 = 1000 (the plan has not changed since the baseline).
/// The engine spreads every baseline cost evenly over the baseline dates: PV = 100.
#[test]
fn planned_value_follows_the_cost_accrual() {
    let mut x = doc(&["10d||fixedcost=1000;accrual=Start"]);
    baseline(&mut x);
    set(&mut x, 0, "pct", "10");
    status(&mut x, "2026-11-02");
    let c = calc(&x);
    let cv = report::curve(&c, "day");
    assert!(close(cv.forecast[0], 1000.0), "forecast day 1 {}", cv.forecast[0]);
    assert!(close(cv.planned[0], cv.forecast[0]), "planned day 1 {} vs forecast {}", cv.planned[0], cv.forecast[0]);
    assert!(close(num(&c, 0, "bcws"), 1000.0), "PV {}", num(&c, 0, "bcws"));
}

/// T1 5d done; T2 5d after it, not started, with an expense paid in advance ($40 actual, $100
/// planned). Status end of Fri 6 Nov. Hand: AC = T1's 400 + 40 = 440, and the cumulative actual
/// cost curve at the status date equals AC. The engine scales actuals onto forecast costs dated
/// before the status date; T2 has none, so its $40 disappears from the curve.
#[test]
fn actual_cost_curve_adds_up_to_ac() {
    let mut x = doc(&["5d", "5d|1"]);
    add_res(&mut x, worker(1, "R", 10.0));
    set(&mut x, 0, "res", "R");
    let uid = x.p.tasks[1].uid;
    op(&mut x, Op::TaskList { uid, field: "expenses".into(), value: serde_json::json!([{ "name": "Deposit", "planned": 100.0, "actual": 40.0, "accrual": "End" }]) });
    baseline(&mut x);
    set(&mut x, 0, "pct", "100");
    status(&mut x, "2026-11-06");
    let c = calc(&x);
    let ac: f64 = (0..2).map(|i| num(&c, i, "acwp")).sum();
    assert!(close(ac, 440.0), "AC {ac}");
    let cv = report::curve(&c, "week");
    let sum: f64 = cv.actual.iter().sum();
    assert!(close(sum, ac), "actual curve {sum} vs AC {ac}");
}

// ---------------------------------------------------------------- planned % of a summary

/// S over T1 9d and T2 1d (parallel), baselined, status end of Mon 2 Nov, on plan:
/// T1 1/9 done (11 %), T2 done. The summary's % complete is duration-weighted
/// ((11 x 9 + 100 x 1)/10 = 19.9 %), so its planned % must be weighted the same way:
/// (9 x 1/9 + 1 x 1)/10 = 20 %. The engine takes the summary's own span linearly: 1/9 = 11.1 %,
/// and the "% complete variance" reports a task exactly on plan as 8.8 points ahead.
#[test]
fn summary_planned_pct_is_weighted_like_its_pct() {
    let mut x = doc(&["1d", "9d", "1d"]);
    let uids = vec![x.p.tasks[1].uid, x.p.tasks[2].uid];
    op(&mut x, Op::Indent { uids });
    baseline(&mut x);
    set(&mut x, 1, "pct", "11");
    set(&mut x, 2, "pct", "100");
    status(&mut x, "2026-11-02");
    let c = calc(&x);
    assert!(c.s.summary[0]);
    assert!(close(num(&c, 0, "pct"), 19.9));
    let planned = num(&c, 0, "plannedpct");
    assert!((planned - 20.0).abs() < 1e-6, "summary planned % {planned}");
    // The period report's WBS row (the same summary, same weighting) already says 20 %.
    let r = report::period_report(&c, "week", None);
    assert!((r.wbs[0].planned - 20.0).abs() < 1e-6, "period report wbs planned {}", r.wbs[0].planned);
}

// ---------------------------------------------------------------- earned schedule (Lipke)

/// T1 10d baselined, 25 % done at the end of Thu 5 Nov (AT = 4 days).
/// Lipke: PV(2) = 20 %, PV(3) = 30 % -> ES = 2 + (25-20)/(30-20) = 2.5 days;
/// SPI(t) = 2.5/4 = 0.625; SV(t) = -1.5 d; IEAC(t) = PD/SPI(t) = 10/0.625 = 16 days.
#[test]
fn earned_schedule_matches_lipke() {
    let mut x = doc(&["10d"]);
    baseline(&mut x);
    set(&mut x, 0, "pct", "25");
    status(&mut x, "2026-11-05");
    let c = calc(&x);
    let e = progress::earned_schedule(&c).unwrap();
    assert!(close(e.es, 2.5) && close(e.at, 4.0) && close(e.pd, 10.0), "{e:?}");
    assert!(close(e.spi_t, 0.625) && close(e.sv_t, -1.5) && close(e.ieac, 16.0), "{e:?}");
}

/// Two tasks: T1 4d then T2 6d (FS), duration-weighted, baselined. Status end of Fri 6 Nov
/// (AT = 5 d). T1 done, T2 0 %: EV = 4/10 = 40 %. PV(4) = 40 % -> ES = 4.0; SPI(t) = 0.8; IEAC(t) = 12.5 d.
#[test]
fn earned_schedule_two_tasks() {
    let mut x = doc(&["4d", "6d|1"]);
    baseline(&mut x);
    set(&mut x, 0, "pct", "100");
    status(&mut x, "2026-11-06");
    let c = calc(&x);
    let e = progress::earned_schedule(&c).unwrap();
    assert!(close(e.es, 4.0) && close(e.at, 5.0) && close(e.spi_t, 0.8) && close(e.ieac, 12.5), "{e:?}");
}

// ---------------------------------------------------------------- buckets

/// T2 5d after an 18d T1 starts Thu 26 Nov 2026 (R 100 %): Thu, Fri, Mon 30 Nov in November
/// (24 h), Tue 1, Wed 2 Dec in December (16 h). Monthly Task Usage: 24 | 16; weekly: week of
/// Mon 23 Nov 16 h, week of Mon 30 Nov 24 h. (Checks the happy path.)
#[test]
fn month_and_week_buckets_split_at_the_boundaries() {
    let mut x = doc(&["18d", "5d|1"]);
    add_res(&mut x, worker(1, "R", 10.0));
    set(&mut x, 1, "res", "R");
    let c = calc(&x);
    assert_eq!(cal::date_of(c.s.es[1]), d(2026, 11, 26));
    let uid = x.p.tasks[1].uid;
    let get = |unit: &str, day: NaiveDate| { let u = report::usage(&c, "task", "work", unit); let j = u.buckets.iter().position(|&b| b == dn(day)).unwrap(); u.rows.iter().find(|r| r.uid == uid && r.of.is_none()).unwrap().vals[j] };
    assert!(close(get("month", d(2026, 11, 1)), 24.0) && close(get("month", d(2026, 12, 1)), 16.0));
    assert!(close(get("week", d(2026, 11, 23)), 16.0) && close(get("week", d(2026, 11, 30)), 24.0));
}

// ---------------------------------------------------------------- summary costs

/// A fixed cost typed on a summary row (the editor accepts it: "fixedcost" is allowed on any row).
/// S (fixed cost 500) over T1 1d with no resources. Hand: S cost 500, project cost $500.
/// The engine only charges leaf tasks: the summary's own fixed cost is dropped (cost 0).
#[test]
fn summary_fixed_cost_is_counted() {
    let mut x = doc(&["1d", "1d"]);
    let uids = vec![x.p.tasks[1].uid];
    op(&mut x, Op::Indent { uids });
    set(&mut x, 0, "fixedcost", "500");
    assert!(close(x.p.tasks[0].fixed_cost, 500.0), "the editor stored it");
    let c = calc(&x);
    assert!(c.s.summary[0]);
    assert!(close(num(&c, 0, "cost"), 500.0), "summary cost {}", num(&c, 0, "cost"));
    assert_eq!(c.summary().cost, "$500.00");
}

/// Installed quantities: 1 of 3 installed on a $900 task. Hand: EV = 900 x 1/3 = 300.
/// The engine stores the share as a whole % (33) and earns 297.
#[test]
fn installed_quantity_earns_its_exact_share() {
    let mut x = doc(&["3d||fixedcost=900"]);
    baseline(&mut x);
    set(&mut x, 0, "qty", "3");
    set(&mut x, 0, "qtydone", "1");
    set(&mut x, 0, "pct", "33");
    status(&mut x, "2026-11-02");
    let c = calc(&x);
    assert!(close(num(&c, 0, "bcwp"), 300.0), "EV {}", num(&c, 0, "bcwp"));
}

/// Part days: T1 0.5d (Mon 8–12), then T2 1d (Mon 13:00 – Tue 12:00), R on both.
/// Hand: R Mon 240 + 240 = 480, Tue 240; T2's daily loads add up to exactly 8 h. (Happy path.)
#[test]
fn part_days_sum_exactly() {
    let mut x = doc(&["0.5d", "1d|1"]);
    add_res(&mut x, worker(1, "R", 10.0));
    set(&mut x, 0, "res", "R");
    set(&mut x, 1, "res", "R");
    let c = calc(&x);
    assert_eq!(daily(&c, 1), vec![(d(2026, 11, 2), 240.0), (d(2026, 11, 3), 240.0)]);
    assert!(!c.l.over[0]);
}

/// Workload view: R 1d on Mon and 2d Tue–Wed in one week: booked 24 h of 40 h, peak 60 %, free 16 h. (Happy path.)
#[test]
fn workload_booked_capacity_peak() {
    let mut x = doc(&["1d", "2d|1"]);
    add_res(&mut x, Resource { uid: 1, name: "R".into(), group: "Mech".into(), max: 1.0, ..Default::default() });
    set(&mut x, 0, "res", "R");
    set(&mut x, 1, "res", "R");
    let c = calc(&x);
    let w = workload::workload(&c, "week");
    let p = &w.trades[0].people[0];
    assert!(close(p.booked[0], 24.0) && close(p.cap[0], 40.0) && close(p.peak, 60.0) && close(w.free, 16.0), "{:?} {:?} {} {}", p.booked, p.cap, p.peak, w.free);
}
