//! Work contours (0.9.38): the shape of an assignment's work over the task, against hand figures.
//! Every plan starts Monday 2 Nov 2026 on the standard calendar (480 working minutes a day).
use chrono::NaiveDate;
use datum_core::{doc::{Doc, Op}, fields::Calc, load::{at_share, cum}, *};

fn d(y: i32, m: u32, day: u32) -> NaiveDate { NaiveDate::from_ymd_opt(y, m, day).unwrap() }
const F: &str = "%Y-%m-%d";
const DAYM: i64 = 1440;

/// One task of `dur` with R (100 %, 10 an hour) on it.
fn plan(dur: &str) -> Doc {
    let mut p = Project::new("A", d(2026, 11, 2));
    let t = p.new_task("T1", 0, 0);
    p.tasks.push(t);
    p.resources.push(Resource { uid: 1, name: "R".into(), max: 1.0, rate: 10.0, ..Default::default() });
    let mut doc = Doc::new(p);
    set(&mut doc, "dur", dur);
    set(&mut doc, "res", "R");
    doc
}
fn set(doc: &mut Doc, f: &str, v: &str) { let uid = doc.p.tasks[0].uid; doc.apply(Op::Set { uid, field: f.into(), value: v.into() }, F).unwrap_or_else(|e| panic!("{f}={v}: {e}")) }
fn try_set(doc: &mut Doc, f: &str, v: &str) -> Result<(), String> { let uid = doc.p.tasks[0].uid; doc.apply(Op::Set { uid, field: f.into(), value: v.into() }, F).map(|_| ()) }
fn calc(doc: &Doc) -> Calc<'static> { Calc::new(Box::leak(Box::new(doc.p.clone())), F, d(2026, 11, 2)) }
/// Work minutes per working day of the task.
fn daily(c: &Calc) -> Vec<f64> { c.l.task_daily.iter().filter(|x| x.1 .0 > 0.0).map(|(_, &(m, _))| (m * 1e6).round() / 1e6).collect() }
fn day_of(c: &Calc, k: usize) -> NaiveDate { cal::date_of(c.l.task_daily.keys().filter(|x| c.l.task_daily[x].0 > 0.0).nth(k).unwrap().1 * DAYM) }

/// Back loaded over 10 days: parts 10,15,25,50,50,75,75,100,100,100 (600); 80 h = 4800 min,
/// so a day takes its part × 8 minutes. The total work and cost do not change.
#[test]
fn back_loaded_spreads_the_work_by_its_parts() {
    let mut x = plan("10d");
    let flat = calc(&x);
    assert_eq!(daily(&flat), vec![480.0; 10]);
    set(&mut x, "contour", "back loaded");
    let c = calc(&x);
    assert_eq!(daily(&c), vec![80.0, 120.0, 200.0, 400.0, 400.0, 600.0, 600.0, 800.0, 800.0, 800.0]);
    assert_eq!(c.text(0, "contour"), "Back Loaded");
    assert_eq!((c.val(0, "work").num(), c.val(0, "cost").num()), (flat.val(0, "work").num(), flat.val(0, "cost").num()), "same work and cost");
    // Mon 2 Nov to Fri 13 Nov: the weekend takes none.
    assert_eq!((day_of(&c, 0), day_of(&c, 5)), (d(2026, 11, 2), d(2026, 11, 9)));
}

/// Bell over 5 days: each day takes two parts, (10+20, 40+80, 100+100, 80+40, 20+10) of 500,
/// of 40 h = 2400 min: 144, 576, 960, 576, 144.
#[test]
fn bell_over_five_days() {
    let mut x = plan("5d");
    set(&mut x, "contour", "Bell");
    assert_eq!(daily(&calc(&x)), vec![144.0, 576.0, 960.0, 576.0, 144.0]);
}

/// A custom curve of two parts, 1 then 3, over 4 days of 1920 min: 240, 240, 720, 720.
#[test]
fn custom_curve() {
    let mut x = plan("4d");
    set(&mut x, "contour", "Custom: 1, 3");
    let c = calc(&x);
    assert_eq!(daily(&c), vec![240.0, 240.0, 720.0, 720.0]);
    assert_eq!(c.text(0, "contour"), "Custom: 1, 3");
    for bad in ["Custom: 5", "Custom: 1, -1", "Custom: 0, 0", "Custom: a, b", "Wavy"] {
        assert!(try_set(&mut x, "contour", bad).is_err(), "{bad} is refused");
    }
}

/// Typing the names again keeps each person's contour; a new person comes in flat (Mixed).
#[test]
fn names_typed_again_keep_the_contour() {
    let mut x = plan("5d");
    set(&mut x, "contour", "Turtle");
    set(&mut x, "res", "R[50%]");
    assert_eq!(calc(&x).text(0, "contour"), "Turtle");
    set(&mut x, "res", "R[50%], S");
    assert_eq!(calc(&x).text(0, "contour"), "Mixed");
    set(&mut x, "contour", "");
    assert_eq!(calc(&x).text(0, "contour"), "Flat");
}

/// Under way: 30 % of a back-loaded task's work is done by the status date, the rest follows the
/// curve from where 30 % of its effort falls (parts 10…50 = 150 of 600, then 30 of the 75 part: 54 %).
#[test]
fn progress_on_a_contoured_task() {
    let w = Contour::BackLoaded.weights(&[]);
    assert!((at_share(&w, 0.3) - 0.54).abs() < 1e-12);
    for k in 0..=20 { let s = k as f64 / 20.0; assert!((cum(&w, at_share(&w, s)) - s).abs() < 1e-12, "cum inverts at_share at {s}") }
    let mut x = plan("10d");
    set(&mut x, "contour", "Back Loaded");
    x.apply(Op::ProjectSet { field: "status".into(), value: serde_json::json!("2026-11-06") }, F).unwrap();
    set(&mut x, "astart", "2026-11-02");
    set(&mut x, "pct", "30");
    let c = calc(&x);
    let total: f64 = c.l.task_daily.values().map(|x| x.0).sum();
    assert!((total - 4800.0).abs() < 1e-6, "all the work, once: {total}");
    let by_status: f64 = c.l.task_daily.range((0, cal::at(d(2026, 11, 2)) / DAYM)..=(0, cal::at(d(2026, 11, 6)) / DAYM)).map(|x| x.1 .0).sum();
    assert!((by_status - 1440.0).abs() < 1e-6, "30 % of 4800 done by the status date: {by_status}");
}

/// Project XML keeps the contour both ways; a custom curve has no code there and goes out flat.
#[test]
fn project_xml_reads_and_writes_the_contour() {
    let mut x = plan("5d");
    set(&mut x, "contour", "Early Peak");
    let s = sched::schedule(&x.p).unwrap();
    let xml = io::mspdi::write(&x.p, &s, &[0.0]);
    assert!(xml.contains("<WorkContour>4</WorkContour>"), "early peak is 4");
    let back = io::mspdi::read(&xml).unwrap();
    assert_eq!(back.tasks.iter().flat_map(|t| &t.res).map(|a| a.contour).collect::<Vec<_>>(), vec![Contour::EarlyPeak]);
    set(&mut x, "contour", "Custom: 1, 2, 1");
    assert!(io::mspdi::write(&x.p, &sched::schedule(&x.p).unwrap(), &[0.0]).contains("<WorkContour>0</WorkContour>"));
}

/// A plan saved with a contour opens with it; an older plan (no contour) opens flat.
#[test]
fn the_contour_is_saved_with_the_plan() {
    let mut x = plan("5d");
    set(&mut x, "contour", "Double Peak");
    let json = serde_json::to_string(&x.p).unwrap();
    assert!(json.contains("\"contour\":\"double_peak\""));
    let back: Project = serde_json::from_str(&json).unwrap();
    assert_eq!(back.tasks[0].res[0].contour, Contour::DoublePeak);
    let old: Project = serde_json::from_str(&json.replace(",\"contour\":\"double_peak\"", "")).unwrap();
    assert_eq!(old.tasks[0].res[0].contour, Contour::Flat);
}
