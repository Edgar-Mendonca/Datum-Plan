//! Scheduling cases worked out by hand from the critical-path rules, one per rule.
//! Calendar: Mon–Fri 08:00–12:00 and 13:00–17:00 (8 h days); the project starts Mon 2 Nov 2026.
//! Each row is "duration | predecessors | field=value; …" (the fields and text the window uses);
//! each result is "early start, early finish, total float in days".
use crate::{cal::date_of, doc::{set, Doc, Op}, sched::schedule, *};
use chrono::NaiveDate;

fn d(y: i32, m: u32, day: u32) -> NaiveDate { NaiveDate::from_ymd_opt(y, m, day).unwrap_or_default() }

fn build(rows: &[&str], tweak: impl FnOnce(&mut Project)) -> Project {
    let mut p = Project::new("Case", d(2026, 11, 2));
    for i in 0..rows.len() {
        let t = p.new_task(&format!("T{}", i + 1), 0, 0);
        p.tasks.push(t);
    }
    tweak(&mut p);
    for (i, r) in rows.iter().enumerate() {
        let mut parts = r.split('|').map(str::trim);
        let (dur, preds, extra) = (parts.next().unwrap_or(""), parts.next().unwrap_or(""), parts.next().unwrap_or(""));
        set(&mut p, i, "dur", dur, "%d-%b-%y").unwrap_or_else(|e| panic!("row {}: {e}", i + 1));
        set(&mut p, i, "preds", preds, "%d-%b-%y").unwrap_or_else(|e| panic!("row {}: {e}", i + 1));
        for kv in extra.split(';').map(str::trim).filter(|s| !s.is_empty()) {
            let (k, v) = kv.split_once('=').unwrap_or((kv, ""));
            set(&mut p, i, k.trim(), v.trim(), "%d-%b-%y").unwrap_or_else(|e| panic!("row {} {k}: {e}", i + 1));
        }
    }
    p
}

fn at(t: i64) -> String { format!("{} {:02}:{:02}", date_of(t).format("%b%d"), t.rem_euclid(1440) / 60, t.rem_euclid(60)) }

fn dates(p: &Project) -> Vec<String> {
    let s = schedule(p).unwrap_or_else(|e| panic!("{e}"));
    (0..p.tasks.len()).map(|i| {
        let f = s.tf[i] as f64 / 480.0;
        format!("{} {} {}", at(s.es[i]), at(s.ef[i]), if f.fract() == 0.0 { format!("{f}") } else { format!("{f:.2}") })
    }).collect()
}

fn check(name: &str, rows: &[&str], tweak: impl FnOnce(&mut Project), want: &[&str]) {
    let got = dates(&build(rows, tweak));
    for (i, (g, w)) in got.iter().zip(want).enumerate() {
        assert_eq!(g, w, "{name}: task {}", i + 1);
    }
    assert_eq!(got.len(), want.len(), "{name}");
}
fn plain(_: &mut Project) {}

#[test]
fn a_textbook_network() {
    // A 5d; B 3d and C 4d after A; D 2d after B and C; E 6d after A; F 1d after D and E.
    check("network", &["5d||", "3d|1|", "4d|1|", "2d|2,3|", "6d|1|", "1d|4,5|"], plain, &[
        "Nov02 08:00 Nov06 17:00 0", "Nov09 08:00 Nov11 17:00 1", "Nov09 08:00 Nov12 17:00 0",
        "Nov13 08:00 Nov16 17:00 0", "Nov09 08:00 Nov16 17:00 0", "Nov17 08:00 Nov17 17:00 0"]);
    // Free float: B can slip a day before it delays D.
    let p = build(&["5d||", "3d|1|", "4d|1|", "2d|2,3|"], plain);
    let s = schedule(&p).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!((s.ff[1], s.ff[2]), (480, 0));
}

#[test]
fn link_types_and_lags() {
    let two = |pred: &str, b: &str| ["5d||".to_string(), format!("{b}|{pred}|")];
    let run = |name: &str, pred: &str, b: &str, want: &str| {
        let r = two(pred, b);
        check(name, &[r[0].as_str(), r[1].as_str()], plain, &["Nov02 08:00 Nov06 17:00 0", want]);
    };
    run("FS", "1", "3d", "Nov09 08:00 Nov11 17:00 0");
    run("FS+2d", "1FS+2d", "3d", "Nov11 08:00 Nov13 17:00 0");
    run("FS lead", "1FS-1d", "3d", "Nov06 08:00 Nov10 17:00 0");
    run("FS+4h", "1FS+4h", "1d", "Nov09 13:00 Nov10 12:00 0");
    run("FS+50%", "1FS+50%", "1d", "Nov11 13:00 Nov12 12:00 0");
    run("SS+2d", "1SS+2d", "3d", "Nov04 08:00 Nov06 17:00 0");
    run("FF+1d", "1FF+1d", "3d", "Nov05 08:00 Nov09 17:00 0");
    run("SF+3d", "1SF+3d", "2d", "Nov03 08:00 Nov04 17:00 2");
}

#[test]
fn durations_and_units() {
    check("units", &["2w||", "12h||", "1.5d||", "90m||", "0||"], plain, &[
        "Nov02 08:00 Nov13 17:00 0", "Nov02 08:00 Nov03 12:00 8.50", "Nov02 08:00 Nov03 12:00 8.50", "Nov02 08:00 Nov02 09:30 9.81", "Nov02 08:00 Nov02 08:00 10"]);
    // A milestone sits at the end of its predecessor; the next task starts the next morning.
    check("milestone", &["5d||", "0|1|", "1d|2|"], plain, &["Nov02 08:00 Nov06 17:00 0", "Nov06 17:00 Nov06 17:00 0", "Nov09 08:00 Nov09 17:00 0"]);
}

#[test]
fn constraints_and_deadlines() {
    let ab = |extra: &str| ["5d||".to_string(), format!("3d|1|{extra}")];
    let run = |name: &str, extra: &str, want: [&str; 2]| {
        let r = ab(extra);
        check(name, &[r[0].as_str(), r[1].as_str()], plain, &want);
    };
    run("SNET", "ctype=Start No Earlier Than; cdate=16-Nov-26", ["Nov02 08:00 Nov06 17:00 5", "Nov16 08:00 Nov18 17:00 0"]);
    run("FNET", "ctype=Finish No Earlier Than; cdate=20-Nov-26", ["Nov02 08:00 Nov06 17:00 7", "Nov18 08:00 Nov20 17:00 0"]);
    run("MSO early", "ctype=Must Start On; cdate=04-Nov-26", ["Nov02 08:00 Nov06 17:00 -3", "Nov04 08:00 Nov06 17:00 0"]);
    run("MFO", "ctype=Must Finish On; cdate=13-Nov-26", ["Nov02 08:00 Nov06 17:00 2", "Nov11 08:00 Nov13 17:00 0"]);
    run("FNLT", "ctype=Finish No Later Than; cdate=10-Nov-26", ["Nov02 08:00 Nov06 17:00 -1", "Nov09 08:00 Nov11 17:00 -1"]);
    run("SNLT", "ctype=Start No Later Than; cdate=06-Nov-26", ["Nov02 08:00 Nov06 17:00 -1", "Nov09 08:00 Nov11 17:00 -1"]);
    check("deadline", &["5d||deadline=05-Nov-26", "3d|1|"], plain, &["Nov02 08:00 Nov06 17:00 -1", "Nov09 08:00 Nov11 17:00 0"]);
    // As late as possible: B has no successors, so it ends with the project.
    check("ALAP", &["5d||", "2d||ctype=As Late As Possible", "1d|1|"], plain, &[
        "Nov02 08:00 Nov06 17:00 0", "Nov06 08:00 Nov09 17:00 0", "Nov09 08:00 Nov09 17:00 0"]);
    // With a successor, it uses only its free float: it moves up to C's start and C is not delayed.
    check("ALAP with successor", &["5d||", "2d||ctype=As Late As Possible", "1d|1,2|"], plain, &[
        "Nov02 08:00 Nov06 17:00 0", "Nov05 08:00 Nov06 17:00 0", "Nov09 08:00 Nov09 17:00 0"]);
}

#[test]
fn calendars() {
    // A public holiday on Wed 4 Nov pushes the work a day.
    check("holiday", &["5d||", "1d|1|"], |p| p.calendars[0].exceptions.push(Exception { name: "Holiday".into(), from: d(2026, 11, 4), to: d(2026, 11, 4), hours: vec![] }), &[
        "Nov02 08:00 Nov09 17:00 0", "Nov10 08:00 Nov10 17:00 0"]);
    // A task on a Mon–Thu 10-hour calendar: 2 days of work (960 min) take 1.6 of its days.
    let four = |p: &mut Project| {
        let ten = vec![(420, 720), (750, 1050)];
        p.calendars.push(Calendar { name: "Four tens".into(), week: [ten.clone(), ten.clone(), ten.clone(), ten, vec![], vec![], vec![]], exceptions: vec![] });
    };
    check("task calendar", &["5d||", "2d|1|cal=Four tens"], four, &["Nov02 08:00 Nov06 17:00 0", "Nov09 07:00 Nov10 13:30 0"]);
    // Elapsed durations run round the clock, weekends included.
    check("elapsed", &["5d||", "2ed|1|"], plain, &["Nov02 08:00 Nov06 17:00 0", "Nov06 17:00 Nov08 17:00 0"]);
    // Lags on a 24-hour calendar: 2d of lag is 16 clock hours, so B starts Monday.
    check("lag on 24 h", &["5d||", "1d|1FS+2d|"], |p| p.opts.lag_cal = LagCal::H24, &["Nov02 08:00 Nov06 17:00 0", "Nov09 08:00 Nov09 17:00 0"]);
}

#[test]
fn summaries_inactive_and_splits() {
    // Summary 1 holds tasks 2 and 3; task 4 follows the summary.
    let mut p = build(&["0||", "5d||", "3d|2|", "2d|1|"], plain);
    for t in &mut p.tasks[1..3] { t.level = 1 }
    assert_eq!(dates(&p), ["Nov02 08:00 Nov11 17:00 0", "Nov02 08:00 Nov06 17:00 0", "Nov09 08:00 Nov11 17:00 0", "Nov12 08:00 Nov13 17:00 0"]);
    // An inactive task and its links are left out (it is not scheduled at all).
    check("inactive", &["5d||", "10d|1|active=No", "2d|2|"], plain, &["Nov02 08:00 Nov06 17:00 0", "Nov02 08:00 Nov02 08:00 0", "Nov02 08:00 Nov03 17:00 3"]);
    // A split: one day of work, a two-day pause, then the remaining four days.
    let mut doc = Doc::new(build(&["5d||", "1d|1|"], plain));
    let uid = doc.p.tasks[0].uid;
    doc.apply(Op::Split { uid, after: 480, gap: 960 }, "%d-%b-%y").unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(dates(&doc.p), ["Nov02 08:00 Nov10 17:00 0", "Nov11 08:00 Nov11 17:00 0"]);
}

#[test]
fn progress_retained_logic_and_override() {
    let status = |p: &mut Project| p.status = Some(d(2026, 11, 4));
    // A is 40 % done at the status date (Wed 4 Nov): its 3 remaining days start Thursday.
    check("remaining", &["5d||astart=02-Nov-26; pct=40", "3d|1|", "2d||"], status, &[
        "Nov02 08:00 Nov09 17:00 0", "Nov10 08:00 Nov12 17:00 0", "Nov05 08:00 Nov06 17:00 4"]);
    // B started early, out of sequence. Retained logic keeps its rest after A; override lets it go on.
    let rows = ["5d||astart=02-Nov-26; pct=40", "4d|1|astart=03-Nov-26; pct=50"];
    check("retained", &rows, status, &["Nov02 08:00 Nov09 17:00 0", "Nov03 08:00 Nov11 17:00 0"]);
    check("override", &rows, |p| { status(p); p.opts.retained = false }, &["Nov02 08:00 Nov09 17:00 0", "Nov03 08:00 Nov06 17:00 1"]);
    // A finished task keeps its actual dates and its successor follows them.
    check("actuals", &["5d||astart=02-Nov-26; afinish=04-Nov-26", "3d|1|"], plain, &["Nov02 08:00 Nov04 17:00 0", "Nov05 08:00 Nov09 17:00 0"]);
}

#[test]
fn scheduling_from_the_finish() {
    check("from finish", &["5d||", "3d|1|"], |p| { p.opts.from_finish = true; p.opts.finish_date = Some(d(2026, 11, 20)) }, &[
        "Nov11 08:00 Nov17 17:00 0", "Nov18 08:00 Nov20 17:00 0"]);
}

#[test]
fn levelling_moves_the_second_task() {
    // Float stays a logic measure: after levelling, A could slip 5 days before it delays the finish.
    let mut doc = Doc::new(build(&["5d||res=Alice", "5d||res=Alice"], plain));
    doc.apply(Op::Level { uids: vec![], clear: false }, "%d-%b-%y").unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(dates(&doc.p), ["Nov02 08:00 Nov06 17:00 5", "Nov09 08:00 Nov13 17:00 0"]);
}

#[test]
fn more_links_and_leads() {
    // A lead that would start B before the project is held at the project start.
    check("SS lead", &["5d||", "3d|1SS-1d|"], plain, &["Nov02 08:00 Nov06 17:00 0", "Nov02 08:00 Nov04 17:00 2"]);
    // Two kinds of link at once: the later one wins.
    check("mixed", &["3d||", "5d||", "2d|1,2SS+1d|"], plain, &["Nov02 08:00 Nov04 17:00 0", "Nov02 08:00 Nov06 17:00 0", "Nov05 08:00 Nov06 17:00 0"]);
    check("FF+4h", &["5d||", "1d|1FF+4h|"], plain, &["Nov02 08:00 Nov06 17:00 0", "Nov06 13:00 Nov09 12:00 0"]);
    check("SF", &["5d||", "2d|1SF|"], plain, &["Nov02 08:00 Nov06 17:00 0", "Nov02 08:00 Nov03 17:00 3"]);
    // A chain: each link type in turn.
    check("chain", &["2d||", "2d|1SS+1d|", "2d|2FF+1d|", "1d|3FS|"], plain, &[
        "Nov02 08:00 Nov03 17:00 0", "Nov03 08:00 Nov04 17:00 0", "Nov04 08:00 Nov05 17:00 0", "Nov06 08:00 Nov06 17:00 0"]);
}

#[test]
fn more_constraints_and_calendars() {
    let weekend = |p: &mut Project| p.start = d(2026, 10, 31);
    check("weekend start", &["1d||"], weekend, &["Nov02 08:00 Nov02 17:00 0"]);
    check("MSO late", &["5d||", "3d|1|ctype=Must Start On; cdate=16-Nov-26"], plain, &["Nov02 08:00 Nov06 17:00 5", "Nov16 08:00 Nov18 17:00 0"]);
    check("deadline met", &["5d||deadline=20-Nov-26"], plain, &["Nov02 08:00 Nov06 17:00 0"]);
    let half_day = |p: &mut Project| p.calendars[0].exceptions.push(Exception { name: "Half day".into(), from: d(2026, 11, 2), to: d(2026, 11, 2), hours: vec![(480, 720)] });
    check("half day", &["1d||"], half_day, &["Nov02 08:00 Nov03 12:00 0"]);
    let week_off = |p: &mut Project| p.calendars[0].exceptions.push(Exception { name: "Shutdown".into(), from: d(2026, 11, 9), to: d(2026, 11, 13), hours: vec![] });
    check("shutdown", &["5d||", "3d|1|"], week_off, &["Nov02 08:00 Nov06 17:00 0", "Nov16 08:00 Nov18 17:00 0"]);
    // Work on a not-started task waits for the status date.
    check("after status", &["5d||astart=02-Nov-26; pct=40", "1d|1|", "2d||"], |p| p.status = Some(d(2026, 11, 4)), &[
        "Nov02 08:00 Nov09 17:00 0", "Nov10 08:00 Nov10 17:00 0", "Nov05 08:00 Nov06 17:00 2"]);
}

#[test]
fn critical_tasks_and_man_hours() {
    let p = build(&["5d||", "3d|1|", "4d|1|", "2d|2,3|", "6d|1|", "1d|4,5|"], plain);
    let s = schedule(&p).unwrap_or_else(|e| panic!("{e}"));
    let crit: Vec<bool> = (0..p.tasks.len()).map(|i| s.critical(&p, i)).collect();
    assert_eq!(crit, [true, false, true, true, true, true]);
    // Two people full time on a 5-day task: 80 man-hours.
    let p = build(&["5d||res=Alice, Bob"], plain);
    let s = schedule(&p).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(crate::load::compute(&p, &s).work[0], 2.0 * 5.0 * 480.0);
}
