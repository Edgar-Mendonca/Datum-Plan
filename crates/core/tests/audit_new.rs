//! Audit of the calculations added in 0.9.38 – 0.11.x (contours, typed hours, roles, expected
//! finish, branching and weather, time-scaled logic, time-location), against hand figures.
//! Plans start Monday 2 Nov 2026 on the standard calendar (8–12, 13–17, Mon–Fri: 480 min a day).
use chrono::NaiveDate;
use datum_core::{cal::{at, DAY}, doc::{Doc, Op}, fields::Calc, risk::{simulate, SimOpts}, *};

fn d(y: i32, m: u32, day: u32) -> NaiveDate { NaiveDate::from_ymd_opt(y, m, day).unwrap() }
const F: &str = "%Y-%m-%d";

/// One task of `dur` with R (100 %, 10 an hour) on it.
fn plan(dur: &str) -> Doc {
    let mut p = Project::new("A", d(2026, 11, 2));
    let t = p.new_task("T1", 0, 0);
    p.tasks.push(t);
    p.resources.push(Resource { uid: 1, name: "R".into(), max: 1.0, rate: 10.0, ..Default::default() });
    let mut doc = Doc::new(p);
    set(&mut doc, 0, "dur", dur);
    set(&mut doc, 0, "res", "R");
    doc
}
fn set(doc: &mut Doc, k: usize, f: &str, v: &str) { let uid = doc.p.tasks[k].uid; doc.apply(Op::Set { uid, field: f.into(), value: v.into() }, F).unwrap_or_else(|e| panic!("{f}={v}: {e}")) }
fn status(doc: &mut Doc, day: &str) { doc.apply(Op::ProjectSet { field: "status".into(), value: serde_json::json!(day) }, F).unwrap() }
fn hours(doc: &mut Doc, k: usize, res: u32, day: &str, h: &str) { let uid = doc.p.tasks[k].uid; doc.apply(Op::DayWork { uid, res, day: day.into(), hours: h.into() }, F).unwrap() }
fn calc(doc: &Doc) -> Calc<'static> { Calc::new(Box::leak(Box::new(doc.p.clone())), F, d(2026, 11, 2)) }
/// Work minutes of task 0 on day `day`.
fn on(c: &Calc, day: NaiveDate) -> f64 { c.l.task_daily.get(&(0, at(day) / DAY)).map_or(0.0, |x| x.0) }
fn no_range(runs: u32) -> SimOpts { SimOpts { iterations: runs, low: 0.0, high: 0.0, seed: 3, ..Default::default() } }

// ---------------------------------------------------------------------------------------------
// Faults
// ---------------------------------------------------------------------------------------------

/// FAULT (load.rs `phased` / `phased_w`): a task whose actual start is after the status date.
/// 5 d task, status date Wed 4 Nov (data date Thu 5 Nov 00:00), actual start Mon 9 Nov, 40 % done.
/// The scheduler (sched.rs, "after the work done when it started later than that") puts the work
/// done at its start: 40 % of 2400 = 960 min = Mon 9 + Tue 10; the rest resumes Wed 11 to Fri 13.
/// So the usage must be 480 on each of Mon 9 … Fri 13.
/// The loading spreads the done share over `es .. dd.clamp(es, ef)` = an empty span, so all 960 min
/// land on Mon 9 (twice a day's capacity; R is flagged overallocated) and Tue 10 carries nothing.
#[test]
fn started_after_the_status_date_books_the_work_done_on_its_days() {
    let mut x = plan("5d");
    status(&mut x, "2026-11-04");
    set(&mut x, 0, "astart", "2026-11-09");
    set(&mut x, 0, "pct", "40");
    let c = calc(&x);
    assert_eq!((c.text(0, "start"), c.text(0, "finish")), ("2026-11-09".to_string(), "2026-11-13".to_string()), "the dates are right");
    let days: Vec<f64> = (9..=13).map(|k| (on(&c, d(2026, 11, k)) * 1e6).round() / 1e6).collect();
    assert_eq!(days, vec![480.0; 5], "a day's work on each day, the work done on Mon 9 and Tue 10");
    assert!(!c.l.over[0], "R works 8 h a day: not overallocated");
}

/// FAULT (doc.rs `"work"` setter): typing the Work of a task whose hours are typed per day.
/// 5 d task, R's Monday typed as 0 h: 0 + 8 + 8 + 8 + 8 = 32 h. The user types Work = 20 h.
/// Expected: Work 20 h (the value just typed). The setter only changes the units (Fixed Duration)
/// or the duration and leaves `Assign.daily` alone, and the loading takes the work from the typed
/// hours, so the task still shows 32 h: the typed value is silently ignored.
#[test]
fn typing_work_on_a_task_with_typed_hours() {
    let mut x = plan("5d");
    hours(&mut x, 0, 1, "2026-11-02", "0");
    assert_eq!(calc(&x).val(0, "work").num(), 32.0);
    set(&mut x, 0, "work", "20");
    assert_eq!(calc(&x).val(0, "work").num(), 20.0, "the work typed");
}

/// FAULT (roles.rs `assign`, the "person already on the task" branch): the work is not kept.
/// T1 (5 d) has the role Mech Engineer and Ann, both 100 %. The role's Monday is typed as 0 h,
/// so the role has 32 h and Ann 40 h: 72 h on T1. Handing the role's work to Ann must leave 72 h
/// (the same work, now all Ann's). `assign` adds the units (Ann 200 %) and clears the typed hours,
/// so T1 becomes 5 d × 2 × 8 h = 80 h: 8 h of work appear from nowhere (and 80 of cost at 10 an hour).
#[test]
fn handing_a_role_to_someone_already_on_the_task_keeps_the_work() {
    let mut p = Project::new("A", d(2026, 11, 2));
    let t = p.new_task("T1", 0, 0);
    p.tasks.push(t);
    let r = |uid, name: &str| Resource { uid, name: name.into(), group: "Mechanical".into(), max: 1.0, rate: 10.0, ..Default::default() };
    p.resources = vec![Resource { max: 2.0, role: true, ..r(1, "Mech Engineer") }, r(2, "Ann")];
    let mut x = Doc::new(p);
    set(&mut x, 0, "dur", "5d");
    set(&mut x, 0, "res", "Mech Engineer, Ann");
    hours(&mut x, 0, 1, "2026-11-02", "0");
    let before = calc(&x).val(0, "work").num();
    assert_eq!(before, 72.0);
    let t1 = x.p.tasks[0].uid;
    x.apply(Op::RoleAssign { role: 1, person: 2, uids: vec![t1] }, F).unwrap();
    let c = calc(&x);
    assert_eq!(x.p.tasks[0].res.len(), 1, "only Ann is left");
    assert_eq!(c.val(0, "work").num(), 72.0, "the same 72 h, now Ann's");
}

/// FAULT (io/mspdi.rs `write`, Assignment <Work>): typed hours are not exported.
/// 5 d task with R's Monday typed as 0 h: the assignment's work is 32 h (the engine shows 32 h).
/// Project XML must carry <Work>PT32H0M0S</Work>; the writer computes dur × units = 40 h.
#[test]
fn project_xml_writes_the_typed_work() {
    let mut x = plan("5d");
    hours(&mut x, 0, 1, "2026-11-02", "0");
    assert_eq!(calc(&x).val(0, "work").num(), 32.0);
    let s = sched::schedule(&x.p).unwrap();
    let xml = io::mspdi::write(&x.p, &s, &[320.0]);
    assert!(xml.contains("<Work>PT32H0M0S</Work>"), "32 h of work in the assignment: {}", &xml[xml.find("<Assignment>").unwrap()..][..170]);
}

/// FAULT (risk.rs `simulate`, weather): a Saturday made a working day by a calendar exception is
/// never lost to weather. Weather 100 % in every month; exception: Sat 7 Nov works 8–12, 13–17.
/// A 6 d task runs Mon 2 – Sat 7 Nov (6 working days to the planned finish), and at 100 % every
/// one of them is lost: weather_days = 6. The weather loop decides "works" from the weekly pattern
/// only (`!c.week[weekday].is_empty()`) and skips any day with an exception, so Sat 7 is never
/// drawn: 5.
#[test]
fn weather_loses_a_saturday_worked_by_exception() {
    let mut p = Project::new("W", d(2026, 11, 2));
    let t = p.new_task("Earthworks", 0, 6 * 480);
    p.tasks.push(t);
    let k = p.calendars.iter().position(|c| c.name == p.calendar).unwrap();
    p.calendars[k].weather = vec![100.0; 12];
    p.calendars[k].exceptions.push(Exception { name: "Sat shift".into(), from: d(2026, 11, 7), to: d(2026, 11, 7), hours: vec![(480, 720), (780, 1020)]});
    let r = simulate(&p, &no_range(10)).unwrap();
    assert_eq!(r.planned, at(d(2026, 11, 7)) + 17 * 60);
    assert_eq!(r.weather_days, 6.0, "six working days, all lost");
}

/// FAULT (risk.rs `simulate`, weather): same cause, a weekday with changed hours by exception
/// (Wed 4 Nov works only 8–12) is still a working day, but is never lost to weather.
/// 5 d task (36 h of working time Mon–Fri with the short Wednesday, it runs into Mon 9 Nov);
/// at 100 % weather every working day up to the planned finish is lost: Mon 2 … Mon 9 = 6 days.
/// The engine leaves Wed 4 out: 5.
#[test]
fn weather_loses_a_short_day() {
    let mut p = Project::new("W", d(2026, 11, 2));
    let t = p.new_task("Earthworks", 0, 5 * 480);
    p.tasks.push(t);
    let k = p.calendars.iter().position(|c| c.name == p.calendar).unwrap();
    p.calendars[k].weather = vec![100.0; 12];
    p.calendars[k].exceptions.push(Exception { name: "Half day".into(), from: d(2026, 11, 4), to: d(2026, 11, 4), hours: vec![(480, 720)]});
    let r = simulate(&p, &no_range(10)).unwrap();
    assert_eq!(r.planned, at(d(2026, 11, 9)) + 12 * 60, "4 h short on Wednesday: Mon 9 at noon");
    assert_eq!(r.weather_days, 6.0);
}

/// FAULT (risk.rs `simulate`, weather from `base.start`): days before the status date are
/// history; weather cannot take them and they move nothing (actual dates stand). 10 d task started
/// Mon 2 Nov, 50 % done at the status date Fri 6 Nov: the rest runs Mon 9 – Fri 13 Nov. With 100 %
/// weather the days that can be lost up to the planned finish are Mon 9 … Fri 13 = 5. The engine
/// draws weather from the plan's first start (Mon 2) and reports 10.
#[test]
fn weather_days_lost_start_at_the_status_date() {
    let mut p = Project::new("W", d(2026, 11, 2));
    let mut t = p.new_task("Earthworks", 0, 10 * 480);
    (t.act_start, t.pct) = (Some(d(2026, 11, 2)), 50);
    p.tasks.push(t);
    p.status = Some(d(2026, 11, 6));
    let k = p.calendars.iter().position(|c| c.name == p.calendar).unwrap();
    p.calendars[k].weather = vec![100.0; 12];
    let r = simulate(&p, &no_range(10)).unwrap();
    assert_eq!(r.planned, at(d(2026, 11, 13)) + 17 * 60);
    assert_eq!(r.weather_days, 5.0, "only Mon 9 – Fri 13 can still be lost");
}

/// FAULT (doc.rs `"locfrom"`/`"locto"`): a chainage "km+m" is read by pasting the digits after
/// the "+" as decimals, which is right only for exactly three digits. 12+500 = 12.5 km (right), but
/// 12+50 is 12 km + 50 m = 12.050 and 12+5 is 12.005; the engine reads both as 12.5.
#[test]
fn chainage_with_fewer_than_three_digits_of_metres() {
    let mut x = plan("1d");
    set(&mut x, 0, "locfrom", "12+500");
    assert_eq!(x.p.tasks[0].loc_from, Some(12.5));
    set(&mut x, 0, "locfrom", "12+050");
    assert_eq!(x.p.tasks[0].loc_from, Some(12.05));
    set(&mut x, 0, "locfrom", "12+50");
    assert_eq!(x.p.tasks[0].loc_from, Some(12.05), "12 km + 50 m");
}

/// FAULT (sched.rs forward pass, `t.exp_finish` under way): a milestone has no work to finish, so
/// an Expected Finish cannot stretch it. A 0 d milestone with an actual start Mon 2 Nov and no
/// actual finish, status Wed 4 Nov: it sits at its resume point and finishes Thu 5 Nov (as without
/// an expected finish). With Expected Finish Fri 20 Nov the engine gives it a finish of Fri 20 Nov
/// (`ef = end_on_or_after(x)`), a three-week span on a task whose duration still reads "0 days".
#[test]
fn expected_finish_does_not_stretch_a_milestone() {
    let mut x = plan("0d");
    status(&mut x, "2026-11-04");
    set(&mut x, 0, "astart", "2026-11-02");
    let before = calc(&x).text(0, "finish");
    assert_eq!(before, "2026-11-05");
    set(&mut x, 0, "expfinish", "2026-11-20");
    let c = calc(&x);
    assert_eq!(c.text(0, "dur"), "0 days");
    assert_eq!(c.text(0, "finish"), before, "a milestone keeps its point");
}

/// CHECKED (passes): typed hours on a split task. 5 d split after 2 d for 1 d: the work days are
/// Mon, Tue, Thu, Fri, Mon (Wed is the pause). Thursday typed as 3 h: 8+8+3+8+8 = 35 h.
#[test]
fn typed_hours_on_a_split_task() {
    let mut y = plan("5d");
    let uid = y.p.tasks[0].uid;
    y.apply(Op::Split { uid, after: 480 * 2, gap: 480 }, F).unwrap();
    hours(&mut y, 0, 1, "2026-11-05", "3");
    let c = calc(&y);
    assert_eq!(c.val(0, "work").num(), 35.0);
    assert_eq!(on(&c, d(2026, 11, 4)), 0.0, "nothing in the pause");
    assert_eq!([2, 3, 5, 6, 9].map(|k| on(&c, d(2026, 11, k))), [480.0, 480.0, 180.0, 480.0, 480.0]);
}

// ---------------------------------------------------------------------------------------------
// Checked and correct (these pass; they document cases audited)
// ---------------------------------------------------------------------------------------------

/// CORRECT: a back-loaded contour on a split task under way keeps all the work once.
/// 10 d (4800 min), split after 3 d for 2 d, started Mon 2 Nov, 30 % done at status Wed 4 Nov.
/// Sum of the daily minutes = 4800, cost 800, and the work done by the status date is 30 % = 1440.
#[test]
fn contour_split_and_progress_keep_the_work() {
    let mut x = plan("10d");
    set(&mut x, 0, "contour", "Back Loaded");
    let uid = x.p.tasks[0].uid;
    x.apply(Op::Split { uid, after: 3 * 480, gap: 2 * 480 }, F).unwrap();
    status(&mut x, "2026-11-04");
    set(&mut x, 0, "astart", "2026-11-02");
    set(&mut x, 0, "pct", "30");
    let c = calc(&x);
    let total: f64 = c.l.task_daily.values().map(|v| v.0).sum();
    assert!((total - 4800.0).abs() < 1e-6, "{total}");
    let done: f64 = (2..=4).map(|k| on(&c, d(2026, 11, k))).sum();
    assert!((done - 1440.0).abs() < 1e-6, "{done}");
    assert_eq!((c.val(0, "work").num(), c.val(0, "cost").num()), (80.0, 800.0));
}

/// CORRECT: typed hours on a task that starts at noon (after a 0.5 d predecessor): 6 working days,
/// Mon (4 h), Tue … Fri, Mon (4 h). Typing 2 h on the first Monday: 2+8+8+8+8+4 = 38 h, booked as typed.
#[test]
fn typed_hours_on_a_partial_first_day() {
    let mut x = plan("5d");
    let mut q = x.p.clone();
    let t = q.new_task("T0", 0, 240);
    q.tasks.insert(0, t);
    x = Doc::new(q);
    set(&mut x, 1, "preds", "1");
    hours(&mut x, 1, 1, "2026-11-02", "2");
    let c = calc(&x);
    assert_eq!(c.val(1, "work").num(), 38.0);
    let got: Vec<f64> = c.l.task_daily.iter().filter(|(k, v)| k.0 == 1 && v.0 > 0.0).map(|(_, v)| v.0).collect();
    assert_eq!(got, vec![120.0, 480.0, 480.0, 480.0, 480.0, 240.0]);
}

/// CORRECT: Expected Finish with retained logic. T1 (2 d, not started) → T2 (4 d, started Mon 2 Nov,
/// 50 %), status Fri 6 Nov, expected finish Wed 11 Nov. T1 runs Mon 9 – Tue 10, so T2 resumes Wed 11
/// and finishes at the end of Wed 11 (its expected finish). An expected finish before the resume
/// (Mon 9) finishes on the resume day (Wed 11), never earlier.
#[test]
fn expected_finish_with_retained_logic() {
    let mut x = plan("2d");
    let mut q = x.p.clone();
    let t = q.new_task("T2", 0, 0);
    q.tasks.push(t);
    x = Doc::new(q);
    set(&mut x, 1, "dur", "4d");
    set(&mut x, 1, "preds", "1");
    status(&mut x, "2026-11-06");
    set(&mut x, 1, "astart", "2026-11-02");
    set(&mut x, 1, "pct", "50");
    set(&mut x, 1, "expfinish", "2026-11-11");
    let c = calc(&x);
    assert_eq!((c.text(0, "start"), c.text(0, "finish"), c.text(1, "finish")), ("2026-11-09".into(), "2026-11-10".into(), "2026-11-11".into()));
    set(&mut x, 1, "expfinish", "2026-11-09");
    assert_eq!(calc(&x).text(1, "finish"), "2026-11-11");
}

/// CORRECT: Finish No Earlier Than a Sunday on a milestone sits at the end of Monday 9 Nov.
#[test]
fn fnet_milestone_on_a_day_off() {
    let mut x = plan("0d");
    set(&mut x, 0, "ctype", "Finish No Earlier Than");
    set(&mut x, 0, "cdate", "2026-11-08");
    let s = sched::schedule(&x.p).unwrap();
    assert_eq!(s.es[0], at(d(2026, 11, 9)) + 17 * 60);
}

/// CORRECT: in the time-scaled logic diagram no two bars of one lane overlap in time, with
/// milestones, a split, a gap and measured names, over a tangle of links.
#[test]
fn tsld_lanes_never_overlap() {
    let rows: [(&str, &str); 10] = [("3d", ""), ("0d", "1"), ("2d", "1SS+1d"), ("4d", ""), ("1d", "4"), ("0d", "3,5"), ("2d", "2"), ("5d", "1FF"), ("1d", ""), ("2d", "9")];
    let mut p = Project::new("L", d(2026, 11, 2));
    for k in 0..rows.len() { let t = p.new_task(&format!("Task {k}"), 0, 0); p.tasks.push(t) }
    let mut x = Doc::new(p);
    for (k, (dur, pr)) in rows.iter().enumerate() {
        set(&mut x, k, "dur", dur);
        if !pr.is_empty() { set(&mut x, k, "preds", pr) }
    }
    let uid = x.p.tasks[3].uid;
    x.apply(Op::Split { uid, after: 480, gap: 960 }, F).unwrap();
    for (gap, ch) in [(0, 0), (240, 0), (0, 30), (960, 60)] {
        let t = tsld::diagram(&calc(&x), false, gap, ch);
        for a in &t.bars { for b in &t.bars {
            if a.uid != b.uid && a.lane == b.lane { assert!(a.finish <= b.start || b.finish <= a.start, "{} and {} overlap on lane {} (gap {gap})", a.name, b.name, a.lane) }
        } }
    }
}

/// CORRECT: the time-location view keeps milestones (a point) and leaves summaries out.
#[test]
fn linear_milestone_and_summary() {
    let mut p = Project::new("L", d(2026, 11, 2));
    for (n, l) in [("Phase", 0), ("Lay pipe", 1), ("Tie-in", 1)] { let t = p.new_task(n, l, 0); p.tasks.push(t) }
    let mut x = Doc::new(p);
    set(&mut x, 1, "dur", "5d");
    set(&mut x, 2, "dur", "0d");
    set(&mut x, 2, "preds", "2");
    assert!(x.apply(Op::Set { uid: x.p.tasks[0].uid, field: "locfrom".into(), value: "9".into() }, F).is_err(), "a summary takes its chainage from its tasks");
    for (k, a, b) in [(1, "0+000", "2+500"), (2, "2+500", "")] { set(&mut x, k, "locfrom", a); if !b.is_empty() { set(&mut x, k, "locto", b) } }
    let l = linear::linear(&calc(&x));
    assert_eq!(l.items.iter().map(|i| (i.name.as_str(), i.from, i.to)).collect::<Vec<_>>(), vec![("Lay pipe", 0.0, 2.5), ("Tie-in", 2.5, 2.5)]);
    let m = &l.items[1];
    assert_eq!(m.start, m.finish);
    assert_eq!((l.lo, l.hi, l.missing), (0.0, 2.5, 0));
}
