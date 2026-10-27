//! Hardening tests: a failed edit changes nothing, random plans and damaged files never panic,
//! and rescheduling stays fast.
use crate::{doc::{Doc, Op}, io::{mspdi, xer}, load, sched::schedule, templates, *};
use chrono::NaiveDate;
use std::{panic, time::Instant};

/// A small deterministic random generator (no extra crate).
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 { self.0 ^= self.0 << 13; self.0 ^= self.0 >> 7; self.0 ^= self.0 << 17; self.0 }
    fn below(&mut self, n: u64) -> u64 { self.next() % n.max(1) }
}

fn start() -> NaiveDate { NaiveDate::from_ymd_opt(2026, 11, 2).unwrap_or_default() }
fn sample() -> Project { templates::build(&templates::TEMPLATES[0], start()) }

/// A plan of `n` leaf tasks with random durations, outline levels and links (loops allowed).
fn random_plan(r: &mut Rng, n: usize, loops: bool) -> Project {
    let mut p = Project::new("R", start());
    for i in 0..n {
        let t = p.new_task(&format!("T{i}"), 0, r.below(10) as i64 * 480);
        p.tasks.push(t);
    }
    let uids: Vec<u32> = p.tasks.iter().map(|t| t.uid).collect();
    for i in 1..n {
        for _ in 0..r.below(3) {
            let j = if loops { r.below(n as u64) as usize } else { r.below(i as u64) as usize };
            let kind = ["FS", "SS", "FF", "SF"][r.below(4) as usize];
            let lag = r.below(5) as i64 - 2;
            if let Ok(mut l) = parse::preds(&format!("{}{kind}{lag:+}d", j + 1), &uids, 480) { p.tasks[i].preds.append(&mut l) }
        }
    }
    p
}

#[test]
fn a_failed_edit_changes_nothing() {
    let mut d = Doc::new(sample());
    let before = d.json();
    let (a, b) = (d.p.tasks[1].uid, d.p.tasks[2].uid);
    let bad = vec![
        Op::Delete { uids: vec![99_999] },
        Op::Set { uid: 99_999, field: "name".into(), value: "x".into() },
        Op::Set { uid: a, field: "duration".into(), value: "not a duration".into() },
        Op::Set { uid: a, field: "start".into(), value: "31-Feb-99999".into() },
        Op::Set { uid: a, field: "predecessors".into(), value: "99999".into() },
        Op::LinkSet { pred: a, succ: b, kind: Some(LinkType::FS), lag: "zz".into() },
        Op::Split { uid: 99_999, after: 1, gap: 1 },
        Op::CalDelete { name: "No such calendar".into() },
        Op::ResSet { uid: 99_999, field: "name".into(), value: "x".into() },
        Op::Sheet { k: Some(999), sheet: None },
        Op::SheetCells { k: 999, cells: vec![("A1".into(), "1".into())], fmt: None },
        Op::Style { cells: vec![(99_999, "name".into(), "b".into())] },
        Op::UpdateProject { through: "not a date".into(), reschedule: true },
        Op::Recurring { at: 0, name: "R".into(), dur: "1d".into(), every: "fortnight".into(), interval: 0, count: 0, start: "?".into() },
    ];
    for o in bad {
        let name = format!("{o:?}");
        let r = panic::catch_unwind(panic::AssertUnwindSafe(|| d.apply(o, "%d-%b-%y")));
        assert!(r.is_ok(), "panicked: {name}");
        if r.is_ok_and(|x| x.is_err()) { assert_eq!(d.json(), before, "a failed {name} changed the plan") }
        d = Doc::new(serde_json::from_str(&before).unwrap_or_default());
    }
    // A link that closes a loop is refused and leaves the plan as it was.
    let mut d = Doc::new(sample());
    let last = d.p.tasks.iter().rev().find(|t| !t.preds.is_empty()).map(|t| t.uid).unwrap_or(a);
    let first = d.p.tasks.iter().find(|t| t.preds.is_empty() && t.uid != last).map(|t| t.uid).unwrap_or(b);
    let json = d.json();
    let r = d.apply(Op::LinkSet { pred: last, succ: first, kind: Some(LinkType::FS), lag: String::new() }, "%d-%b-%y");
    if r.is_err() { assert_eq!(d.json(), json) }
}

#[test]
fn random_plans_schedule_without_panics() {
    let mut r = Rng(0x2545_F491_4F6C_DD1D);
    for round in 0..1000 {
        let loops = round % 3 == 0;
        let n = 2 + r.below(40) as usize;
        let p = random_plan(&mut r, n, loops);
        let s = panic::catch_unwind(|| schedule(&p)).unwrap_or_else(|_| panic!("schedule panicked on round {round}"));
        let Ok(s) = s else { assert!(loops, "a plan without loops failed to schedule (round {round})"); continue };
        assert_eq!(Some(&s.es), schedule(&p).ok().as_ref().map(|x| &x.es), "same plan, same dates");
        for i in (0..p.tasks.len()).filter(|&i| !s.summary[i]) {
            assert!(s.es[i] <= s.ef[i] && s.ls[i] <= s.lf[i], "start after finish (round {round}, task {i})");
            assert!(s.tf[i] >= 0, "negative float with no constraints (round {round}, task {i})");
            // Compared in working time: 17:00 one day and 08:00 the next are the same moment.
            let c = &s.cals[s.ci[i]];
            assert!(c.between(s.es[i], s.ls[i]) >= 0 && c.between(s.ef[i], s.lf[i]) >= 0, "late dates before early dates (round {round}, task {i})");
        }
        let _ = load::compute(&p, &s);
    }
}

/// Cuts and garbles a good file many ways; every reader must answer with Ok or an error, never panic.
fn survives(name: &str, good: &str, read: fn(&str) -> Result<Project, String>) {
    let mut r = Rng(0x9E37_79B9_7F4A_7C15);
    let b = good.as_bytes();
    let mut cases: Vec<String> = (0..60).map(|k| String::from_utf8_lossy(&b[..b.len() * k / 60]).into_owned()).collect();
    for _ in 0..3000 {
        let mut v = b.to_vec();
        for _ in 0..1 + r.below(8) {
            let i = r.below(v.len() as u64) as usize;
            v[i] = [b'<', b'>', b'/', b'\t', b'\n', b'"', b'-', b'9', 0, 0xFF][r.below(10) as usize];
        }
        cases.push(String::from_utf8_lossy(&v).into_owned());
    }
    cases.extend(["", "<", "</a>", "<a><b></a>", "%T\tTASK\n%F\ttask_id\n%R\t1", "\u{FEFF}", "<?xml?><Project><Tasks><Task><Duration>PT99999999999H</Duration></Task></Tasks></Project>"].map(String::from));
    for (k, c) in cases.iter().enumerate() {
        let res = panic::catch_unwind(|| read(c).map(|p| { let _ = schedule(&p); }));
        assert!(res.is_ok(), "{name} reader panicked on case {k}: {:?}", &c[..c.len().min(120)]);
    }
}

#[test]
fn damaged_files_never_panic() {
    let p = sample();
    let s = schedule(&p).unwrap_or_else(|e| panic!("{e}"));
    let cost = load::compute(&p, &s).cost;
    survives("XML", &mspdi::write(&p, &s, &cost), mspdi::read);
    survives("XER", &xer::write(&p, &s, start()), xer::read);
    survives("JSON", &serde_json::to_string(&p).unwrap_or_default(), |t| Project::from_json(t).map_err(|e| e.to_string()));
}

#[test]
fn rescheduling_stays_fast() {
    let mut r = Rng(7);
    let p = random_plan(&mut r, 2_000, false);
    let _ = schedule(&p);
    let t = Instant::now();
    for _ in 0..5 { let _ = schedule(&p); }
    let ms = t.elapsed().as_secs_f64() * 1000.0 / 5.0;
    // Debug builds (cargo test) are about 10–20× slower than the app.
    let limit = if cfg!(debug_assertions) { 400.0 } else { 20.0 };
    assert!(ms < limit, "2,000 tasks took {ms:.1} ms to schedule (limit {limit} ms)");
    eprintln!("2,000 tasks schedule in {ms:.2} ms");
}

