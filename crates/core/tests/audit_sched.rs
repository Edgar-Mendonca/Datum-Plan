//! Audit of the scheduling engine (forward/backward pass, constraints, calendars, progress, leveling).
//! Calendar: Standard, Mon–Fri 08:00–12:00 and 13:00–17:00 (8 h days); the project starts Mon 2 Nov 2026.
//! Every expected value is worked by hand in the comment above it.
//!
//! Part 1 (`bug_*`): tests that FAIL today; each shows a calculation bug.
//! Part 2 (`ok_*`): tests that pass; they document what was verified.
use chrono::NaiveDate;
use datum_core::{
    cal::{at, date_of, WorkCal, DAY},
    doc::{Doc, Op},
    fields::Calc,
    sched::schedule,
    *,
};

const F: &str = "%d-%b-%y";
const D: i64 = 480;

fn d(y: i32, m: u32, day: u32) -> NaiveDate { NaiveDate::from_ymd_opt(y, m, day).unwrap() }
fn t(y: i32, m: u32, day: u32, h: i64, min: i64) -> i64 { at(d(y, m, day)) + h * 60 + min }
fn hm(x: i64) -> String { format!("{} {:02}:{:02}", date_of(x).format("%b%d"), x.rem_euclid(DAY) / 60, x.rem_euclid(60)) }

/// Rows "duration | predecessors | field=value; …", entered through Doc::apply(Op::Set) like the window does.
/// `levels` are applied before the fields (so summaries exist), `tweak` before anything.
fn build_lv(rows: &[&str], levels: &[u8], tweak: impl FnOnce(&mut Project)) -> Doc {
    let mut p = Project::new("Audit", d(2026, 11, 2));
    for i in 0..rows.len() {
        let mut x = p.new_task(&format!("T{}", i + 1), 0, 0);
        x.level = levels.get(i).copied().unwrap_or(0);
        p.tasks.push(x);
    }
    tweak(&mut p);
    let mut doc = Doc::new(p);
    for (i, r) in rows.iter().enumerate() {
        let uid = doc.p.tasks[i].uid;
        let mut parts = r.split('|').map(str::trim);
        let (dur, preds, extra) = (parts.next().unwrap_or(""), parts.next().unwrap_or(""), parts.next().unwrap_or(""));
        let summary = doc.p.is_summary(i);
        if !summary {
            doc.apply(Op::Set { uid, field: "dur".into(), value: dur.into() }, F).unwrap_or_else(|e| panic!("row {} dur: {e}", i + 1));
        }
        if !preds.is_empty() {
            doc.apply(Op::Set { uid, field: "preds".into(), value: preds.into() }, F).unwrap_or_else(|e| panic!("row {} preds: {e}", i + 1));
        }
        for kv in extra.split(';').map(str::trim).filter(|s| !s.is_empty()) {
            let (k, v) = kv.split_once('=').unwrap_or((kv, ""));
            doc.apply(Op::Set { uid, field: k.trim().into(), value: v.trim().into() }, F).unwrap_or_else(|e| panic!("row {} {k}: {e}", i + 1));
        }
    }
    doc
}
fn build(rows: &[&str], tweak: impl FnOnce(&mut Project)) -> Project { build_lv(rows, &[], tweak).p }
fn plain(_: &mut Project) {}

/// "es ef tf(days)" per row.
fn dates(p: &Project) -> Vec<String> {
    let s = schedule(p).unwrap_or_else(|e| panic!("{e}"));
    (0..p.tasks.len()).map(|i| format!("{} {} {}", hm(s.es[i]), hm(s.ef[i]), s.tf[i] as f64 / D as f64)).collect()
}

// =====================================================================================
// PART 1: FAILING TESTS (bugs)
// =====================================================================================

/// Free float can never exceed total float (standard CPM: FF <= TF; with negative TF, FF = 0).
/// A 2d, deadline Mon 2 Nov; B 10d unrelated.
///   A: ES Mon02 08:00, EF Tue03 17:00, LF = end of Mon02 = Mon02 17:00 -> TF = -1d.
///   Project finish = B EF = Fri13 17:00. sched.rs:303 seeds A's free float with the gap to the project
///   finish (8d) and the deadline (sched.rs:338) never limits it -> FF = 8d although TF = -1d.
///   Expected FF = 0.
#[test]
fn bug_free_float_exceeds_negative_total_float_with_deadline() {
    let p = build(&["2d||deadline=02-Nov-26", "10d||"], plain);
    let s = schedule(&p).unwrap();
    assert_eq!(s.tf[0], -D, "TF of A");
    assert_eq!(s.ff[0], 0, "free float of A must not exceed its total float (-1d) and is never below 0");
}

/// Same with a Finish No Later Than constraint and a positive total float smaller than the free float.
/// A 2d FNLT Wed 4 Nov; B 10d. A EF Tue03 17:00, LF Wed04 17:00 -> TF = 1d. Gap to project finish = 8d.
/// Expected FF = min(8d, TF 1d) = 1d.
#[test]
fn bug_free_float_exceeds_total_float_with_fnlt() {
    let p = build(&["2d||ctype=Finish No Later Than; cdate=04-Nov-26", "10d||"], plain);
    let s = schedule(&p).unwrap();
    assert_eq!(s.tf[0], D, "TF of A");
    assert_eq!(s.ff[0], D, "free float of A is capped by its total float");
}

/// A deadline on a summary task is accepted (doc.rs:384 lets it be set on summary rows) but the
/// backward pass only visits leaf tasks (`order` holds leaves), so sched.rs:338 never applies it.
/// Summary S { K1 5d; K2 3d after K1 }, deadline on S = Fri 6 Nov.
///   S finishes Wed11 17:00, deadline Fri06 17:00 -> 3 days late: K2 LF = Fri06 17:00, TF(K2) = -3d,
///   K1 LF = Tue03 17:00 (Fri06 - 3d), TF(K1) = -3d, TF(S) = -3d.
/// Actual: every TF is 0 (the deadline is ignored).
#[test]
fn bug_summary_deadline_is_ignored() {
    let doc = build_lv(&["0||deadline=06-Nov-26", "5d||", "3d|2|"], &[0, 1, 1], plain);
    let p = &doc.p;
    assert!(p.is_summary(0));
    assert_eq!(p.tasks[0].deadline, Some(d(2026, 11, 6)));
    let s = schedule(p).unwrap();
    assert_eq!(hm(s.ef[0]), "Nov11 17:00");
    assert_eq!((s.tf[1], s.tf[2], s.tf[0]), (-3 * D, -3 * D, -3 * D), "a summary deadline must limit its tasks' late finish");
}

/// Scheduling from the finish: all tasks are placed at their late dates, but float must still be
/// late minus early (early dates forward from the calculated project start). sched.rs:354-356 copies
/// the late dates into es/ef and sched.rs:381 then measures TF as LF - EF = 0 for every task, so
/// every task is critical.
/// A 5d, B 1d, C 1d after A and B; finish date Fri 20 Nov.
///   Backward: C Fri20; A Fri13..Thu19; B Thu19. Project start = Fri13 08:00.
///   Early (forward from Fri13): B could start Fri13 -> B's float = Fri13..Wed18 = 4d. A and C are critical.
/// Expected TF(B) = 4d and B not critical; actual TF(B) = 0 and B critical.
#[test]
fn bug_from_finish_makes_every_task_critical() {
    let p = build(&["5d||", "1d||", "1d|1,2|"], |p| { p.opts.from_finish = true; p.opts.finish_date = Some(d(2026, 11, 20)) });
    let s = schedule(&p).unwrap();
    assert_eq!(hm(s.es[1]), "Nov19 08:00", "B sits at its late dates");
    assert_eq!(hm(s.es[0]), "Nov13 08:00");
    assert_eq!(s.tf[0], 0);
    assert_eq!(s.tf[1], 4 * D, "B is not on the critical path: 4 days of float");
    assert!(!s.critical(&p, 1));
}

/// A percentage lag on a link from a summary is a share of the summary's duration
/// (sched.rs:145 takes `dur[a]` of each leaf instead).
/// S { K1 2d; K2 4d after K1 } = Mon02 08:00 .. Mon09 17:00, 6 working days. X FS+50% from S:
///   lag = 3d -> X starts after Mon09 17:00 + 3d = Thu12 17:00 -> Fri13 08:00.
/// Actual: K2's own 50% (2d) wins -> X starts Thu12 08:00.
#[test]
fn bug_percent_lag_from_summary_uses_leaf_durations() {
    let doc = build_lv(&["0||", "2d||", "4d|2|", "1d|1FS+50%|"], &[0, 1, 1, 0], plain);
    let s = schedule(&doc.p).unwrap();
    assert_eq!((hm(s.es[0]), hm(s.ef[0])), ("Nov02 08:00".to_string(), "Nov09 17:00".to_string()));
    assert_eq!(hm(s.es[3]), "Nov13 08:00");
}

/// Finish No Earlier Than on a day off. The constraint says the task may not finish before Sat 7 Nov;
/// sched.rs:246 uses `c.end_of(cd)`, which for a day off is the end of the working day BEFORE it
/// (Fri 6 Nov 17:00), so the task finishes on Friday, earlier than the constraint allows.
/// (SNET on a day off rolls forward to Monday, sched.rs:245, so the two are inconsistent.)
/// 1d task FNET Sat 7 Nov -> expected Mon09 08:00 .. Mon09 17:00.
/// NOTE: in working-time terms Fri 17:00 and Sat 00:00 are the same instant (no work between them),
/// so this is a convention/display question (Finish shows 06-Nov under a 07-Nov FNET), not a CPM error.
#[test]
#[ignore = "divergence, not a confirmed bug: FNET on a day off rolls back"]
fn div_fnet_on_a_day_off_finishes_before_the_constraint() {
    let p = build(&["1d||ctype=Finish No Earlier Than; cdate=07-Nov-26"], plain);
    assert_eq!(p.tasks[0].cdate, Some(d(2026, 11, 7)));
    let s = schedule(&p).unwrap();
    assert!(s.ef[0] >= at(d(2026, 11, 7)), "finish {} is before the FNET date Sat 7 Nov", hm(s.ef[0]));
    assert_eq!((hm(s.es[0]), hm(s.ef[0])), ("Nov09 08:00".to_string(), "Nov09 17:00".to_string()));
}

/// A critical milestone (TF 0) shows a Late Start/Late Finish on the NEXT working day.
/// A 5d -> M (0d) -> B 1d. M: ES = EF = Fri06 17:00, TF 0. Backward: B LS = Mon09 08:00, and for a
/// milestone sched.rs:350 keeps LF = v = Mon09 08:00 (no prev_end), LS = LF.
/// Expected late dates shown as 06-Nov-26 (same as its start/finish, as for any task with TF 0);
/// actual 09-Nov-26. Fix: for sp == 0, lf = c.prev_end(v).max(ef) when v is the start of a working
/// period (or show late dates of milestones with the same rule as the finish).
#[test]
fn bug_critical_milestone_late_dates_on_next_day() {
    let p = build(&["5d||", "0|1|", "1d|2|"], plain);
    let c = Calc::new(&p, F, d(2026, 11, 2));
    assert_eq!(c.s.tf[1], 0);
    assert_eq!(c.text(1, "start"), "06-Nov-26");
    assert_eq!(c.text(1, "ls"), "06-Nov-26", "late start of a critical milestone");
    assert_eq!(c.text(1, "lf"), "06-Nov-26", "late finish of a critical milestone");
}

/// Fixed Work: the work stays when the duration changes (units follow). doc.rs "dur" only sets the
/// duration, and load.rs computes work as dur * units, so the work doubles.
/// 4d Fixed Work, Alice 100% -> 32 h. Duration 8d -> work still 32 h (Alice 50%).
#[test]
fn bug_fixed_work_duration_change_changes_work() {
    let mut doc = build_lv(&["4d||type=Fixed Work; res=Alice"], &[], plain);
    let uid = doc.p.tasks[0].uid;
    let w0 = { let s = schedule(&doc.p).unwrap(); load::compute(&doc.p, &s).work[0] };
    assert_eq!(w0, 32.0 * 60.0);
    doc.apply(Op::Set { uid, field: "dur".into(), value: "8d".into() }, F).unwrap();
    let s = schedule(&doc.p).unwrap();
    assert_eq!(load::compute(&doc.p, &s).work[0], 32.0 * 60.0, "Fixed Work keeps its work");
}

/// Fixed Duration, effort-driven: adding a second person keeps duration AND total work (units drop).
/// doc.rs "res" handles only Fixed Work and effort-driven Fixed Units, so the work doubles.
/// 4d, Alice -> 32 h; then "Alice, Bob" -> still 4d and 32 h (16 h each).
#[test]
fn bug_fixed_duration_effort_driven_adding_people_doubles_work() {
    let mut doc = build_lv(&["4d||type=Fixed Duration; effort=yes; res=Alice"], &[], plain);
    let uid = doc.p.tasks[0].uid;
    doc.apply(Op::Set { uid, field: "res".into(), value: "Alice, Bob".into() }, F).unwrap();
    let s = schedule(&doc.p).unwrap();
    assert_eq!(doc.p.tasks[0].dur, 4 * D);
    assert_eq!(load::compute(&doc.p, &s).work[0], 32.0 * 60.0, "effort-driven: the work stays");
}

/// Actual start AFTER the status date with some progress: the work already done is lost.
/// Status Wed 4 Nov; A 2d, actual start Mon 9 Nov, 50 %. Done = 1d (Mon09), remaining 1d.
/// sched.rs:217 starts the remaining work at max(status+1 day, actual start) = Mon09 08:00, so the
/// remaining day overlaps the day already worked: A shows Mon09 08:00 .. Mon09 17:00, one day for a
/// 2-day task at 50 %. Expected: remaining work after the work done -> finish Tue10 17:00.
/// Fix: `let mut from = dd.max(c.add(es[i], dur[i] - rem_without_gaps))` (i.e. not before
/// actual start + work done) whenever the actual start is after the status date.
#[test]
fn bug_actual_start_after_status_date_loses_work_done() {
    let p = build(&["2d||astart=09-Nov-26; pct=50"], |p| p.status = Some(d(2026, 11, 4)));
    let s = schedule(&p).unwrap();
    assert_eq!(hm(s.es[0]), "Nov09 08:00");
    assert_eq!(hm(s.ef[0]), "Nov10 17:00", "a 2-day task half done from Mon 9 cannot finish on Mon 9");
}

/// Completing a milestone gives it the whole working day: sched.rs:213 sets EF = end_of(actual finish)
/// even for a 0-duration task. A milestone at the project start (Mon02 08:00) with a successor:
/// before: A 5d = Mon02 08:00 .. Fri06 17:00. Mark the milestone 100 % (actual start = finish = Mon02):
/// its EF jumps to Mon02 17:00 and A slips a whole day to Tue03 .. Mon09, moving the project finish.
/// Expected: a finished milestone keeps EF = ES (Mon02 08:00); A unchanged.
/// Fix: in the actual-finish branch, when dur == 0 (or act_start == act_finish and dur == 0) use ef = es.
#[test]
fn bug_completing_a_milestone_pushes_its_successors_a_day() {
    let mut doc = build_lv(&["0||", "5d|1|"], &[], plain);
    assert_eq!(dates(&doc.p)[1], "Nov02 08:00 Nov06 17:00 0");
    let uid = doc.p.tasks[0].uid;
    doc.apply(Op::Set { uid, field: "pct".into(), value: "100".into() }, F).unwrap();
    assert_eq!((doc.p.tasks[0].act_start, doc.p.tasks[0].act_finish), (Some(d(2026, 11, 2)), Some(d(2026, 11, 2))));
    let s = schedule(&doc.p).unwrap();
    assert_eq!(hm(s.ef[0]), "Nov02 08:00", "a finished milestone has no duration");
    assert_eq!((hm(s.es[1]), hm(s.ef[1])), ("Nov02 08:00".to_string(), "Nov06 17:00".to_string()), "completing the milestone must not move A");
}

/// Same with a Start Milestone after a task: A 5d -> SM (Start Milestone) sits at Mon09 08:00 -> B 1d Mon09.
/// Marking SM 100 % moves B to Tue10.
#[test]
fn bug_completing_a_start_milestone_pushes_its_successor() {
    let mut doc = build_lv(&["5d||astart=02-Nov-26; afinish=06-Nov-26", "0|1|acttype=Start Milestone", "1d|2|"], &[], plain);
    assert_eq!(hm(schedule(&doc.p).unwrap().es[2]), "Nov09 08:00");
    let uid = doc.p.tasks[1].uid;
    doc.apply(Op::Set { uid, field: "pct".into(), value: "100".into() }, F).unwrap();
    let s = schedule(&doc.p).unwrap();
    assert_eq!(hm(s.es[2]), "Nov09 08:00", "B must not move when its start milestone is achieved on time");
}

// =====================================================================================
// PART 2: PASSING TESTS (verified behaviour)
// =====================================================================================

#[test]
fn ok_link_types_with_leads_and_percent() {
    // A 4d Mon02..Thu05. B 2d FS-50% (lead 2d): Thu05 17:00 - 2d = Wed04 08:00 -> Wed04..Thu05.
    // C 1d SF-1d: finish >= A start - 1d = before project start -> held at project start, Mon02.
    // D 2d FF-1d: finish >= Thu05 17:00 - 1d = Wed04 17:00 -> Tue03..Wed04.
    // E 1d SS+25% (1d): Mon02 08:00 + 1d -> Mon02 17:00 -> Tue03 08:00.
    let p = build(&["4d||", "2d|1FS-50%|", "1d|1SF-1d|", "2d|1FF-1d|", "1d|1SS+25%|"], plain);
    let s = schedule(&p).unwrap();
    assert_eq!(hm(s.es[1]), "Nov04 08:00");
    assert_eq!(hm(s.es[2]), "Nov02 08:00");
    assert_eq!((hm(s.es[3]), hm(s.ef[3])), ("Nov03 08:00".into(), "Nov04 17:00".into()));
    assert_eq!(hm(s.es[4]), "Nov03 08:00");
}

#[test]
fn ok_free_float_several_successors_and_lags() {
    // A 2d (Mon02..Tue03). X 5d (Mon02..Fri06), Y 3d (Mon02..Wed04).
    // B 1d after A FS+1d and after X: B ES = Mon09 08:00.  C 1d after A SS+2d and after Y: C ES = Thu05 08:00.
    // FF(A) via B: from Tue03 17:00 + 1d = Wed04 17:00 to Mon09 08:00 = 2d.
    //       via C: from Mon02 08:00 + 2d = Tue03 17:00 to Thu05 08:00 = 1d.  -> FF(A) = 1d.
    let p = build(&["2d||", "5d||", "3d||", "1d|1FS+1d,2|", "1d|1SS+2d,3|"], plain);
    let s = schedule(&p).unwrap();
    assert_eq!((hm(s.es[3]), hm(s.es[4])), ("Nov09 08:00".into(), "Nov05 08:00".into()));
    assert_eq!(s.ff[0], D);
    // TF(A): C LF = Mon09 17:00 (project finish), LS = Mon09 08:00 -> A LF via SS+2d = Mon09 08:00 - 2d + 2d = ...
    // via B: B LS Mon09 08:00 - 1d = Fri06 08:00 -> prev_end... A LF <= Fri06 08:00 -> Thu05 17:00 -> TF = 2d.
    // via C: C LS Mon09 08:00, SS: A LS <= Mon09 08:00 - 2d = Thu05 08:00 -> A LF = Fri06 17:00 -> 3d. min = 2d.
    assert_eq!(s.tf[0], 2 * D);
}

#[test]
fn ok_constraints_hard_and_soft() {
    // SNLT/FNLT/MSO/MFO negative float, SNET on a day off rolls to Monday, MSO milestone.
    let p = build(&[
        "5d||",
        "1d|1|ctype=Start No Earlier Than; cdate=07-Nov-26",   // Sat -> Mon09
        "2d|1|ctype=Must Finish On; cdate=05-Nov-26",           // hard: Wed04..Thu05 (ignores link)
        "0||ctype=Must Start On; cdate=11-Nov-26",              // Wed11 08:00
    ], plain);
    let s = schedule(&p).unwrap();
    assert_eq!(hm(s.es[1]), "Nov09 08:00");
    assert_eq!((hm(s.es[2]), hm(s.ef[2])), ("Nov04 08:00".into(), "Nov05 17:00".into()));
    // A must finish by Tue03 17:00 for C to start Wed04 -> A TF = Tue03 - Fri06 = -3d.
    assert_eq!(s.tf[0], -3 * D);
    assert_eq!(hm(s.es[3]), "Nov11 08:00");
}

#[test]
fn ok_alap_chain_moves_both() {
    // A ALAP 1d -> B ALAP 1d -> C 1d; D 5d -> C. C starts Mon09.
    // B moves to Fri06, then A to Thu05 (successor processed first in the backward pass).
    let p = build(&["1d||ctype=As Late As Possible", "1d|1|ctype=As Late As Possible", "1d|2,4|", "5d||"], plain);
    let s = schedule(&p).unwrap();
    assert_eq!(hm(s.es[0]), "Nov05 08:00");
    assert_eq!(hm(s.es[1]), "Nov06 08:00");
    assert_eq!(hm(s.es[2]), "Nov09 08:00");
}

#[test]
fn ok_year_end_holidays_and_leap_day() {
    // 5d from Wed 23 Dec 2026 with holidays 25 Dec and 1 Jan: Wed23, Thu24, Mon28, Tue29, Wed30.
    let hol = |p: &mut Project| {
        p.start = d(2026, 12, 23);
        for x in [d(2026, 12, 25), d(2027, 1, 1)] { p.calendars[0].exceptions.push(Exception { name: "H".into(), from: x, to: x, hours: vec![] }) }
    };
    let p = build(&["5d||", "3d|1|"], hol);
    let s = schedule(&p).unwrap();
    assert_eq!(hm(s.ef[0]), "Dec30 17:00");
    // Wed30 -> Thu31, (Fri01 holiday), Mon04, Tue05.
    assert_eq!((hm(s.es[1]), hm(s.ef[1])), ("Dec31 08:00".into(), "Jan05 17:00".into()));
    // Leap year: Mon 28 Feb 2028 + 3d = Mon28, Tue29 Feb, Wed01 Mar. Elapsed 2ed SNET Tue 29 Feb: on the
    // 24 h clock the constraint instant (00:00) is working time, so 29 Feb 00:00 .. 2 Mar 00:00 (shown 29-Feb..01-Mar).
    let p = build(&["3d||", "2ed||ctype=Start No Earlier Than; cdate=29-Feb-28"], |p| p.start = d(2028, 2, 28));
    let s = schedule(&p).unwrap();
    assert_eq!(hm(s.ef[0]), "Mar01 17:00");
    assert_eq!(hm(s.ef[1]), "Mar02 00:00");
    assert_eq!(Calc::new(&p, F, d(2028, 2, 28)).text(1, "finish"), "01-Mar-28");
}

#[test]
fn ok_between_matches_day_by_day_over_long_spans() {
    // `between` counts whole weeks for spans > 28 days; compare with a day-by-day sum, including
    // holidays, a working Saturday and a short day, for many start/end pairs.
    let mut c = Calendar::standard("S");
    c.exceptions.push(Exception { name: "H".into(), from: d(2026, 12, 24), to: d(2027, 1, 1), hours: vec![] });
    c.exceptions.push(Exception { name: "Sat".into(), from: d(2027, 2, 6), to: d(2027, 2, 6), hours: vec![(480, 720)] });
    c.exceptions.push(Exception { name: "Short".into(), from: d(2027, 3, 3), to: d(2027, 3, 3), hours: vec![(480, 600)] });
    c.exceptions.push(Exception { name: "Leap".into(), from: d(2028, 2, 29), to: d(2028, 2, 29), hours: vec![] });
    let w = WorkCal::new(&c);
    let brute = |a: i64, b: i64| -> i64 {
        let mut n = 0;
        let mut day = a.div_euclid(DAY);
        while day * DAY < b {
            for &(s, e) in w.day(date_of(day * DAY)) { n += ((day * DAY + e).min(b) - (day * DAY + s).max(a)).max(0) }
            day += 1;
        }
        n
    };
    let base = at(d(2026, 11, 1));
    let mut x: i64 = 12345;
    for _ in 0..400 {
        x = (x * 1103515245 + 12345) % 2147483648;
        let a = base + x % (600 * DAY);
        x = (x * 1103515245 + 12345) % 2147483648;
        let b = a + x % (400 * DAY);
        assert_eq!(w.between(a, b), brute(a, b), "between {} and {}", hm(a), hm(b));
        assert_eq!(w.between(b, a), -brute(a, b));
    }
}

#[test]
fn ok_add_sub_roundtrip_and_shift_ends() {
    let w = WorkCal::new(&Calendar::standard("S"));
    // Fri 17:00 + 1 min = Mon 08:01; Mon 08:00 - 1 min = Fri 16:59; 4h from Mon 08:00 ends at 12:00 (shift end).
    assert_eq!(hm(w.add(t(2026, 11, 6, 17, 0), 1)), "Nov09 08:01");
    assert_eq!(hm(w.sub(t(2026, 11, 9, 8, 0), 1)), "Nov06 16:59");
    assert_eq!(hm(w.add(t(2026, 11, 2, 8, 0), 240)), "Nov02 12:00");
    assert_eq!(hm(w.next_start(t(2026, 11, 2, 12, 0))), "Nov02 13:00");
    assert_eq!(hm(w.prev_end(t(2026, 11, 9, 8, 0))), "Nov06 17:00");
    assert_eq!(hm(w.end_of(d(2026, 11, 7))), "Nov06 17:00");
}

#[test]
fn ok_progress_status_date_and_actuals() {
    // Status Wed04. A 5d started Mon02 40% -> 3d remaining from Thu05 -> Mon09 17:00.
    // B 2d not started, no preds -> waits for the status date: Thu05..Fri06.
    // C 2d finished (Mon02..Tue03 actual) -> fixed.
    let p = build(&["5d||astart=02-Nov-26; pct=40", "2d||", "2d||astart=02-Nov-26; afinish=03-Nov-26"], |p| p.status = Some(d(2026, 11, 4)));
    let s = schedule(&p).unwrap();
    assert_eq!(hm(s.ef[0]), "Nov09 17:00");
    assert_eq!(hm(s.rs[0]), "Nov05 08:00");
    assert_eq!(hm(s.es[1]), "Nov05 08:00");
    assert_eq!(hm(s.ef[2]), "Nov03 17:00");
}

#[test]
fn ok_longest_path_several_end_tasks() {
    // A 5d -> B 5d (ends Fri13); C 10d (ends Fri13); D 3d. X 2d -> Y 1d SNET Fri13 (ends Fri13, X not driving).
    let p = build(&["5d||", "5d|1|", "10d||", "3d||", "2d||", "1d|5|ctype=Start No Earlier Than; cdate=13-Nov-26"], plain);
    let s = schedule(&p).unwrap();
    assert_eq!(s.lp, vec![true, true, true, false, false, true]);
    assert_eq!(hm(s.finish), "Nov13 17:00");
}

#[test]
fn ok_cycles_empty_plans_and_huge_lags() {
    // Cycle refused by Doc.
    let mut doc = build_lv(&["1d||", "1d|1|"], &[], plain);
    let uid = doc.p.tasks[0].uid;
    assert!(doc.apply(Op::Set { uid, field: "preds".into(), value: "2".into() }, F).is_err());
    // Empty plan schedules and Calc works.
    let p = Project::new("Empty", d(2026, 11, 2));
    let s = schedule(&p).unwrap();
    assert_eq!(hm(s.finish), "Nov02 08:00");
    let _ = Calc::new(&p, F, d(2026, 11, 2));
    // A huge lag saturates without panicking.
    let mut p = build(&["1d||", "1d||"], plain);
    let u = p.tasks[0].uid;
    p.tasks[1].preds = vec![Link { pred: u, kind: LinkType::FS, lag: model::MAX_SPAN, pct: false, elapsed: false }];
    let s = schedule(&p).unwrap();
    assert!(s.es[1] > s.ef[0]);
    p.tasks[1].preds[0].lag = -model::MAX_SPAN;
    let s = schedule(&p).unwrap();
    assert_eq!(hm(s.es[1]), "Nov02 08:00");
}

#[test]
fn ok_leveling_keeps_logic_and_priority_order() {
    // A 2d (prio 100) and B 2d (prio 900) both on Alice; C 1d after A. Priority order -> B keeps its
    // place, A moves to Wed04, C follows A (logic kept). (In Standard order float ranks first: A has
    // TF 0 via C and B TF 1d, so B would move; that is the documented order.)
    let mut doc = build_lv(&["2d||res=Alice; priority=100", "2d||res=Alice; priority=900", "1d|1|"], &[], |p| p.opts.level_order = LevelOrder::Priority);
    doc.apply(Op::Level { uids: vec![], clear: false }, F).unwrap();
    let s = schedule(&doc.p).unwrap();
    assert_eq!(hm(s.es[1]), "Nov02 08:00");
    assert!(s.es[0] >= s.ef[1], "A after B: {}", hm(s.es[0]));
    assert!(s.es[2] >= s.ef[0], "C still after A");
}

#[test]
fn ok_task_calendar_resource_calendar_and_fixed_units() {
    // Fixed Units effort-driven: 4d with Alice, then Alice and Bob -> 2d.
    let mut doc = build_lv(&["4d||type=Fixed Units; effort=yes; res=Alice"], &[], plain);
    let uid = doc.p.tasks[0].uid;
    doc.apply(Op::Set { uid, field: "res".into(), value: "Alice, Bob".into() }, F).unwrap();
    assert_eq!(doc.p.tasks[0].dur, 2 * D);
    // Resource calendars on: Alice off Wed04 -> a 3d task runs Mon02, Tue03, Thu05.
    let mut doc = build_lv(&["3d||res=Alice"], &[], |p| p.opts.res_cals = true);
    let k = doc.p.res_idx(doc.p.tasks[0].res[0].res).unwrap();
    doc.p.resources[k].exceptions.push(Exception { name: "Leave".into(), from: d(2026, 11, 4), to: d(2026, 11, 4), hours: vec![] });
    let s = schedule(&doc.p).unwrap();
    assert_eq!(hm(s.ef[0]), "Nov05 17:00");
}

#[test]
fn ok_elapsed_and_inactive_and_manual() {
    // Elapsed 3ed SNET Fri06 -> Fri06 00:00 .. Mon09 00:00 (24 h clock). Successor on standard starts Mon09 08:00.
    let p = build(&["3ed||ctype=Start No Earlier Than; cdate=06-Nov-26", "1d|1|"], plain);
    let s = schedule(&p).unwrap();
    assert_eq!(hm(s.ef[0]), "Nov09 00:00");
    assert_eq!(hm(s.es[1]), "Nov09 08:00");
    // Inactive predecessor: its link is dropped.
    let p = build(&["5d||", "5d|1|active=No", "1d|2|"], plain);
    assert_eq!(dates(&p)[2], "Nov02 08:00 Nov02 17:00 4");
}
