//! The time-scaled logic diagram (0.11.0): lanes, bands and links, against hand layouts.
//! Plans start Monday 2 Nov 2026 on the standard calendar (8 h, Mon–Fri).
use chrono::NaiveDate;
use datum_core::{doc::{Doc, Op}, fields::Calc, *};

fn d(y: i32, m: u32, day: u32) -> NaiveDate { NaiveDate::from_ymd_opt(y, m, day).unwrap() }
const F: &str = "%Y-%m-%d";
/// Rows of (name, level, duration, predecessors by row number).
fn plan(rows: &[(&str, u8, &str, &str)]) -> Doc {
    let mut p = Project::new("L", d(2026, 11, 2));
    for &(name, level, _, _) in rows { let t = p.new_task(name, level, 0); p.tasks.push(t) }
    let mut doc = Doc::new(p);
    for (k, &(_, _, dur, preds)) in rows.iter().enumerate() {
        let uid = doc.p.tasks[k].uid;
        for (f, v) in [("dur", dur), ("preds", preds)].into_iter().filter(|x| !x.1.is_empty()) { doc.apply(Op::Set { uid, field: f.into(), value: v.into() }, F).unwrap() }
    }
    doc
}
fn calc(doc: &Doc) -> Calc<'static> { Calc::new(Box::leak(Box::new(doc.p.clone())), F, d(2026, 11, 2)) }
fn lanes(t: &tsld::Tsld) -> Vec<(&str, usize)> { let mut v: Vec<_> = t.bars.iter().map(|b| (b.name.as_str(), b.lane)).collect(); v.sort(); v }

/// T1 (5 d) → T2 (3 d), and T3 (2 d) → T4 (2 d) beside them. T3 overlaps T1, so it takes a second
/// lane; T4 follows T3 on its lane and T2 follows T1 on the first: two lanes.
#[test]
fn chains_keep_their_lane() {
    let x = plan(&[("T1", 0, "5d", ""), ("T2", 0, "3d", "1"), ("T3", 0, "2d", ""), ("T4", 0, "2d", "3")]);
    let t = tsld::diagram(&calc(&x), false, 0, 0);
    assert_eq!(t.lanes, 2);
    assert_eq!(lanes(&t), vec![("T1", 0), ("T2", 0), ("T3", 1), ("T4", 1)]);
    assert_eq!(t.links.len(), 2);
    assert!(t.links.iter().all(|l| l.driving && l.kind == "FS"));
    let crit: Vec<&str> = t.bars.iter().filter(|b| b.critical).map(|b| b.name.as_str()).collect();
    assert_eq!(crit, vec!["T1", "T2"], "the longest path");
    assert_eq!(t.bands.len(), 1);
}

/// A gap keeps names apart: with a day's gap T4 cannot follow T3 at once on its lane only if they
/// touch; here T3 ends Tue and T4 starts Wed (a night between), so a gap of 16 h still fits, one
/// of 20 h does not (T4 goes to a new lane).
#[test]
fn the_gap_between_bars() {
    let x = plan(&[("T1", 0, "5d", ""), ("T3", 0, "2d", ""), ("T4", 0, "2d", "2")]);
    assert_eq!(tsld::diagram(&calc(&x), false, 15 * 60, 0).lanes, 2);
    assert_eq!(tsld::diagram(&calc(&x), false, 20 * 60, 0).lanes, 3);
    // Names measured: "T3" (with its ID "A1020", 9 characters) at a day a character outruns T4's start.
    assert_eq!(tsld::diagram(&calc(&x), false, 0, 1440).lanes, 3);
}

/// By phase: each top-level summary gets its own band of lanes; summaries draw no bar.
#[test]
fn a_band_per_phase() {
    let x = plan(&[("Design", 0, "", ""), ("D1", 1, "3d", ""), ("D2", 1, "3d", ""), ("Build", 0, "", ""), ("B1", 1, "4d", "2"), ("M", 0, "0", "5")]);
    let t = tsld::diagram(&calc(&x), true, 0, 0);
    let bands: Vec<(&str, usize, usize)> = t.bands.iter().map(|b| (b.name.as_str(), b.lane, b.lanes)).collect();
    assert_eq!(bands, vec![("Design", 0, 2), ("Build", 2, 1), ("", 3, 1)], "D1 and D2 run together; the milestone sits outside any phase");
    assert_eq!(t.bars.len(), 4);
    assert!(t.bars.iter().find(|b| b.name == "M").unwrap().milestone);
    assert_eq!(lanes(&t).iter().find(|b| b.0 == "B1").unwrap().1, 2);
}
