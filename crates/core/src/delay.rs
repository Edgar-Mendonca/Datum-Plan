//! Delay analysis on the data the plan holds. Each method says what it needs and runs only when the
//! plan has it: as-planned against as-built (a baseline, and actual or forecast dates), and windows
//! (two or more snapshots). The analysis measures; it does not say who is responsible.
use crate::cal::{date_of, DAY};
use crate::fields::Calc;
use crate::model::*;
use serde::Serialize;

/// A method of analysis, whether the plan has what it needs, and why not.
#[derive(Serialize)]
pub struct Method { pub key: &'static str, pub name: &'static str, pub needs: &'static str, pub ok: bool, pub why: String }

/// What the plan holds for an analysis, and the methods it allows.
#[derive(Serialize)]
pub struct Available { pub baseline: usize, pub actuals: usize, pub snapshots: usize, pub status: Option<String>, pub methods: Vec<Method> }

pub fn available(c: &Calc) -> Available {
    let (p, s) = (c.p, &c.s);
    let leaf = |i: usize| !s.summary[i] && !s.off[i];
    let baseline = (0..p.tasks.len()).filter(|&i| leaf(i) && p.tasks[i].bls.contains_key(&0)).count();
    let actuals = (0..p.tasks.len()).filter(|&i| leaf(i) && p.tasks[i].act_start.is_some()).count();
    let snapshots = p.snapshots.iter().filter(|x| x.date.is_some() && !x.rows.is_empty()).count();
    let mut methods = vec![
        Method { key: "apab", name: "As-planned against as-built", needs: "A baseline (the plan as agreed), and actual dates or the current forecast.", ok: baseline > 0,
            why: if baseline > 0 { format!("{baseline} tasks have a baseline; {actuals} have started.") } else { "No baseline: save one first (Progress › Set Baseline), on the plan as agreed.".into() } },
        Method { key: "windows", name: "Windows", needs: "Two or more snapshots, one at each update (Progress › Snapshot).", ok: snapshots >= 2,
            why: if snapshots >= 2 { format!("{snapshots} snapshots make {} windows.", snapshots - 1) } else { format!("{snapshots} snapshot{}: take one at each update.", if snapshots == 1 { "" } else { "s" }) } },
    ];
    // Methods that come in 0.8.3: shown so the choice is clear.
    for (key, name, needs) in [("tia", "Time impact", "The current plan and a delay event added to a copy of it (a fragnet)."),
        ("iap", "Impacted as-planned", "A baseline and the delay events added to it."), ("cab", "Collapsed as-built", "Actual dates for the work, and the delay events taken out.")] {
        methods.push(Method { key, name, needs, ok: false, why: "Comes in the next release.".into() });
    }
    Available { baseline, actuals, snapshots, status: p.status.map(|d| d.to_string()), methods }
}

/// One task in the as-planned against as-built comparison; dates in days since 1970, variances in working days.
#[derive(Serialize)]
pub struct ApabRow {
    pub uid: u32, pub id: String, pub name: String, pub critical: bool,
    pub bs: i64, pub bf: i64, pub s: i64, pub f: i64, pub started: bool, pub finished: bool,
    pub start_var: f64, pub finish_var: f64, pub planned_dur: f64, pub dur: f64,
}

#[derive(Serialize)]
pub struct Apab { pub planned_finish: Option<i64>, pub finish: i64, pub delay: f64, pub rows: Vec<ApabRow>, pub drivers: Vec<u32> }

/// Working days from `a` to `b` on the project calendar (negative when `b` is earlier).
fn wd(c: &Calc, a: i64, b: i64) -> f64 {
    let pc = &c.s.cals[c.p.calendars.iter().position(|x| x.name == c.p.calendar).unwrap_or(0)];
    let m = if b >= a { pc.between(a, b) } else { -pc.between(b, a) };
    (m as f64 / c.p.day_min.max(1) as f64 * 10.0).round() / 10.0
}

/// Each task with a baseline: planned against actual (or forecast) start, finish and duration; the
/// project's planned and current finish; and the drivers: tasks on today's critical path that
/// finish later than planned, latest first.
pub fn apab(c: &Calc) -> Apab {
    let (p, s) = (c.p, &c.s);
    let day = |m: i64| m.div_euclid(DAY);
    let mut rows = vec![];
    for (i, t) in p.tasks.iter().enumerate() {
        if s.summary[i] || s.off[i] { continue }
        let Some(b) = t.bls.get(&0) else { continue };
        let (es, ef) = (s.es[i], s.ef[i]);
        rows.push(ApabRow {
            uid: t.uid, id: if t.code.is_empty() { c.text(i, "id") } else { t.code.clone() }, name: t.name.clone(), critical: s.critical(p, i),
            bs: day(b.start), bf: day((b.finish - 1).max(b.start)), s: day(es), f: day((ef - 1).max(es)), started: t.act_start.is_some(), finished: t.act_finish.is_some(),
            start_var: wd(c, b.start, es), finish_var: wd(c, b.finish, ef), planned_dur: (b.dur as f64 / p.day_min.max(1) as f64 * 10.0).round() / 10.0,
            dur: (s.cals[s.ci[i]].between(es, ef) as f64 / p.day_min.max(1) as f64 * 10.0).round() / 10.0,
        });
    }
    let planned = p.tasks.iter().enumerate().filter(|(i, _)| !s.summary[*i] && !s.off[*i]).filter_map(|(_, t)| t.bls.get(&0).map(|b| b.finish)).max();
    let mut drivers: Vec<&ApabRow> = rows.iter().filter(|r| r.critical && r.finish_var > 0.0).collect();
    drivers.sort_by(|a, b| b.finish_var.total_cmp(&a.finish_var));
    Apab { planned_finish: planned.map(|f| day(f - 1)), finish: day(s.finish - 1), delay: planned.map_or(0.0, |f| wd(c, f, s.finish)), drivers: drivers.iter().take(20).map(|r| r.uid).collect(), rows }
}

/// A task that slipped in a window: its forecast finish at the start and at the end of it.
#[derive(Serialize)]
pub struct Slip { pub uid: u32, pub name: String, pub was: i64, pub now: i64, pub moved: f64, pub critical: bool }

/// One window between two snapshots: how far the project finish moved, and the tasks behind it.
#[derive(Serialize)]
pub struct Window {
    pub from: String, pub to: String, pub label: String, pub comment: String,
    pub finish_before: i64, pub finish_after: i64, pub moved: f64, pub total: f64, pub pct_before: f64, pub pct_after: f64, pub slips: Vec<Slip>,
}

/// The windows between consecutive snapshots, and a last one to today's plan when the status date is
/// later than the last snapshot. In each, the tasks that slipped most: first those critical at the
/// window's start (when the snapshot knows them), then the rest.
pub fn windows(c: &Calc) -> Vec<Window> {
    let (p, s) = (c.p, &c.s);
    let mut snaps: Vec<&Snapshot> = p.snapshots.iter().filter(|x| x.date.is_some() && !x.rows.is_empty()).collect();
    snaps.sort_by_key(|x| x.date);
    // Today's plan as the last point, when it is newer than the last snapshot.
    let now: Option<Snapshot> = p.status.filter(|d| snaps.last().is_some_and(|x| x.date < Some(*d))).map(|d| Snapshot {
        date: Some(d), label: "Now".into(), finish: s.finish,
        pct: snaps.last().map_or(0.0, |_| { let leaf: Vec<usize> = (0..p.tasks.len()).filter(|&i| !s.summary[i] && !s.off[i]).collect(); let w: i64 = leaf.iter().map(|&i| p.tasks[i].dur).sum(); if w > 0 { leaf.iter().map(|&i| p.tasks[i].dur * p.tasks[i].pct as i64).sum::<i64>() as f64 / w as f64 } else { 0.0 } }),
        rows: p.tasks.iter().enumerate().map(|(i, t)| SnapRow { uid: t.uid, start: s.es[i], finish: s.ef[i], pct: t.pct, cost: 0.0, crit: s.critical(p, i), tf: s.tf[i] }).collect(),
        ..Default::default()
    });
    if let Some(n) = &now { snaps.push(n) }
    let leafs: std::collections::HashSet<u32> = (0..p.tasks.len()).filter(|&i| !s.summary[i]).map(|i| p.tasks[i].uid).collect();
    let name = |u: u32| p.idx(u).map_or(format!("#{u}"), |i| p.tasks[i].name.clone());
    let day = |m: i64| (m - 1).div_euclid(DAY);
    let mut total = 0.0;
    snaps.windows(2).map(|w| {
        let (a, b) = (w[0], w[1]);
        let moved = wd(c, a.finish, b.finish);
        total += moved;
        let before: std::collections::HashMap<u32, &SnapRow> = a.rows.iter().map(|r| (r.uid, r)).collect();
        let mut slips: Vec<Slip> = b.rows.iter().filter(|r| leafs.contains(&r.uid)).filter_map(|r| {
            let o = before.get(&r.uid)?;
            (r.finish > o.finish).then(|| Slip { uid: r.uid, name: name(r.uid), was: day(o.finish.max(o.start + 1)), now: day(r.finish.max(r.start + 1)), moved: wd(c, o.finish, r.finish), critical: o.crit })
        }).collect();
        slips.sort_by(|x, y| y.critical.cmp(&x.critical).then(y.moved.total_cmp(&x.moved)));
        slips.truncate(10);
        Window {
            from: a.date.map(|d| d.to_string()).unwrap_or_default(), to: b.date.map(|d| d.to_string()).unwrap_or_default(), label: b.label.clone(), comment: b.comment.clone(),
            finish_before: day(a.finish), finish_after: day(b.finish), moved, total: (total * 10.0f64).round() / 10.0, pct_before: a.pct, pct_after: b.pct, slips,
        }
    }).collect()
}

/// The finish in days since 1970 as a date (for reports).
pub fn date(d: i64) -> String { date_of(d * DAY).to_string() }

/// The analysis as a workbook (.xlsx) or OpenDocument spreadsheet: a summary, the tasks, the windows.
pub fn write_book(c: &Calc, path: &std::path::Path) -> Result<(), String> {
    use crate::io::book::{Cell, Sheet};
    let day = |d: i64| Cell::Date(date_of(d * DAY));
    let a = apab(c);
    let w = windows(c);
    let mut sum = vec![vec![Cell::from(format!("Delay analysis: {}", c.p.name))], vec![]];
    sum.push(vec!["Status date".into(), c.p.status.map_or(Cell::Empty, Cell::Date)]);
    sum.push(vec!["Planned finish (baseline)".into(), a.planned_finish.map_or(Cell::Empty, day)]);
    sum.push(vec!["Finish now".into(), day(a.finish)]);
    sum.push(vec!["Delay (working days)".into(), Cell::Num(a.delay)]);
    sum.push(vec![]);
    sum.push(vec![Cell::from("The figures measure what moved; they do not say who is responsible.")]);
    let mut rows = vec![["Activity ID", "Name", "Critical now", "Baseline start", "Baseline finish", "Start", "Finish", "Started", "Finished", "Start var. (d)", "Finish var. (d)", "Baseline duration (d)", "Duration (d)"].map(Cell::from).to_vec()];
    for r in &a.rows {
        rows.push(vec![r.id.clone().into(), r.name.clone().into(), Cell::from(if r.critical { "Yes" } else { "" }), day(r.bs), day(r.bf), day(r.s), day(r.f),
            Cell::from(if r.started { "Actual" } else { "" }), Cell::from(if r.finished { "Actual" } else { "" }), Cell::Num(r.start_var), Cell::Num(r.finish_var), Cell::Num(r.planned_dur), Cell::Num(r.dur)]);
    }
    let mut win = vec![["From", "To", "Snapshot", "Finish before", "Finish after", "Moved (d)", "Total (d)", "% complete before", "% complete after", "Tasks that slipped most", "Notes"].map(Cell::from).to_vec()];
    let date = |s: &str| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").map_or(Cell::Empty, Cell::Date);
    for x in &w {
        let slips = x.slips.iter().take(3).map(|s| format!("{} (+{} d{})", s.name, s.moved, if s.critical { ", critical" } else { "" })).collect::<Vec<_>>().join("; ");
        win.push(vec![date(&x.from), date(&x.to), x.label.clone().into(), day(x.finish_before), day(x.finish_after), Cell::Num(x.moved), Cell::Num(x.total),
            Cell::Num((x.pct_before * 10.0).round() / 10.0), Cell::Num((x.pct_after * 10.0).round() / 10.0), slips.into(), x.comment.clone().into()]);
    }
    crate::io::book::write(path, &[
        Sheet { name: "Summary".into(), widths: vec![30.0, 16.0], rows: sum },
        Sheet { name: "As-planned v as-built".into(), widths: vec![12.0, 40.0, 10.0, 12.0, 12.0, 12.0, 12.0, 9.0, 9.0, 10.0, 10.0, 12.0, 10.0], rows },
        Sheet { name: "Windows".into(), widths: vec![12.0, 12.0, 18.0, 12.0, 12.0, 9.0, 9.0, 10.0, 10.0, 60.0, 40.0], rows: win },
    ])
}
