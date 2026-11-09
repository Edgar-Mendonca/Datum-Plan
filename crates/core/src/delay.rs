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
    let events = p.delays.iter().filter(|e| e.on).count();
    let ev = |extra: &str| if events == 0 { format!("No delay events yet: add them on the Delay events tab.{extra}") } else { format!("{events} delay event{} to apply.{extra}", if events == 1 { "" } else { "s" }) };
    methods.push(Method { key: "tia", name: "Time impact", needs: "Today's plan, and the delay events added to a copy of it at the status date.", ok: events > 0, why: ev("") });
    methods.push(Method { key: "iap", name: "Impacted as-planned", needs: "A baseline (the plan as agreed), and the delay events added to it.", ok: events > 0 && baseline > 0,
        why: if baseline == 0 { "No baseline: save one on the plan as agreed.".into() } else { ev("") } });
    methods.push(Method { key: "cab", name: "Collapsed as-built", needs: "Actual dates for the work done, and the delay events taken out of it.", ok: events > 0 && actuals > 0,
        why: if actuals == 0 { "No actual dates yet: record progress first.".into() } else { ev(" Activities of a fragnet count only once added to the plan.") } });
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
    let mut sheets = vec![
        Sheet { name: "Summary".into(), widths: vec![30.0, 16.0], rows: sum },
        Sheet { name: "As-planned v as-built".into(), widths: vec![12.0, 40.0, 10.0, 12.0, 12.0, 12.0, 12.0, 9.0, 9.0, 10.0, 10.0, 12.0, 10.0], rows },
        Sheet { name: "Windows".into(), widths: vec![12.0, 12.0, 18.0, 12.0, 12.0, 9.0, 9.0, 10.0, 10.0, 60.0, 40.0], rows: win },
    ];
    // Each method that can run on the delay events: its basis, then each event's effect.
    let ok = |k: &str| available(c).methods.iter().any(|m| m.key == k && m.ok);
    for (key, title, run) in [("tia", "Time impact", tia as fn(&Calc) -> Result<Impact, String>), ("iap", "Impacted as-planned", iap), ("cab", "Collapsed as-built", cab)] {
        if !ok(key) { continue }
        let Ok(r) = run(c) else { continue };
        let mut rows = vec![vec![Cell::from(r.basis.clone())], vec![Cell::from(r.fidelity.clone().unwrap_or_default())], vec![],
            ["Event", "Arose", "Finish before", "Finish after", "Effect (d)", "Total (d)", "Notes"].map(Cell::from).to_vec()];
        for e in &r.events {
            rows.push(vec![e.name.clone().into(), e.date.as_deref().map_or(Cell::Empty, date), day(e.before), day(e.after), Cell::Num(e.impact), Cell::Num(e.total), e.notes.join(" ").into()]);
        }
        sheets.push(Sheet { name: title.into(), widths: vec![36.0, 12.0, 12.0, 12.0, 10.0, 10.0, 60.0], rows });
    }
    crate::io::book::write(path, &sheets)
}

// ---------- delay events: time impact, impacted as-planned, collapsed as-built ----------

/// One event's effect in a method: the finish before and after it, and its share of the delay.
#[derive(Serialize)]
pub struct EventImpact { pub id: u32, pub name: String, pub date: Option<String>, pub before: i64, pub after: i64, pub impact: f64, pub total: f64, pub notes: Vec<String> }

/// A method's result: the plan it ran on (and how close that rebuilt plan comes to the real one),
/// each event's impact in order, and the finish with all of them.
#[derive(Serialize)]
pub struct Impact { pub key: &'static str, pub basis: String, pub fidelity: Option<String>, pub start_finish: i64, pub end_finish: i64, pub total: f64, pub events: Vec<EventImpact> }

/// The name a fragnet activity has in the plan.
pub fn frag_name(e: &DelayEvent, f: &FragTask) -> String { format!("{}: {}", e.name, f.name) }

/// Adds an event's effect to a plan; returns what could not be applied.
fn add(q: &mut Project, e: &DelayEvent) -> Vec<String> {
    let mut notes = vec![];
    for &(uid, m) in &e.extend {
        match q.idx(uid) {
            Some(i) if q.tasks[i].act_finish.is_some() => notes.push(format!("\u{201c}{}\u{201d} is finished; its extension was not added.", q.tasks[i].name)),
            Some(i) => q.tasks[i].dur += m,
            None => notes.push(format!("Task #{uid} is not in this plan.")),
        }
    }
    for &(uid, d) in &e.hold {
        match q.idx(uid) {
            Some(i) if q.tasks[i].act_start.is_some() => notes.push(format!("\u{201c}{}\u{201d} has started; its hold was not added.", q.tasks[i].name)),
            Some(i) => {
                let t = &mut q.tasks[i];
                if matches!(t.constraint, Constraint::ASAP) || (t.constraint == Constraint::SNET && t.cdate.is_some_and(|c| c < d)) { (t.constraint, t.cdate) = (Constraint::SNET, Some(d)) }
                else if t.constraint != Constraint::SNET { notes.push(format!("\u{201c}{}\u{201d} has its own constraint; the hold was not added.", t.name)) }
            }
            None => notes.push(format!("Task #{uid} is not in this plan.")),
        }
    }
    for f in &e.frag {
        let mut t = q.new_task(&frag_name(e, f), 0, f.dur);
        t.manual = false;
        t.preds = f.after.iter().filter(|u| q.idx(**u).is_some()).map(|&u| Link { pred: u, kind: LinkType::FS, lag: 0, pct: false, elapsed: false }).collect();
        let uid = t.uid;
        q.tasks.push(t);
        for b in &f.before { if let Some(i) = q.idx(*b) { q.tasks[i].preds.push(Link { pred: uid, kind: LinkType::FS, lag: 0, pct: false, elapsed: false }) } }
    }
    q.normalize();
    notes
}

/// Puts an event's new activities into the plan itself (so collapsed as-built can take them out again).
pub fn add_fragnet(p: &mut Project, e: &DelayEvent) {
    let only = DelayEvent { frag: e.frag.iter().filter(|f| !p.tasks.iter().any(|t| t.name == frag_name(e, f))).cloned().collect(), ..e.clone() };
    let only = DelayEvent { extend: vec![], hold: vec![], ..only };
    add(p, &only);
}

/// Takes an event's effect out of a plan (collapsed as-built): extensions off, holds lifted, fragnet
/// activities already in the plan reduced to nothing.
fn remove(q: &mut Project, e: &DelayEvent) -> Vec<String> {
    let mut notes = vec![];
    for &(uid, m) in &e.extend { if let Some(i) = q.idx(uid) { q.tasks[i].dur = (q.tasks[i].dur - m).max(0) } }
    for &(uid, d) in &e.hold { if let Some(i) = q.idx(uid) { let t = &mut q.tasks[i]; if t.constraint == Constraint::SNET && t.cdate == Some(d) { (t.constraint, t.cdate) = (Constraint::ASAP, None) } } }
    for f in &e.frag {
        let n = frag_name(e, f);
        match q.tasks.iter().position(|t| t.name == n) { Some(i) => q.tasks[i].dur = 0, None => notes.push(format!("\u{201c}{n}\u{201d} is not in the plan: add the fragnet to the plan to collapse it.")) }
    }
    notes
}

fn finish(q: &Project) -> Result<i64, String> { crate::sched::schedule(q).map(|s| s.finish) }

/// Events in the order they arose (undated last), the ones switched on.
fn events(p: &Project) -> Vec<&DelayEvent> { let mut v: Vec<&DelayEvent> = p.delays.iter().filter(|e| e.on).collect(); v.sort_by_key(|e| (e.date.is_none(), e.date, e.id)); v }

/// Applies the events one after another to `base`, measuring each one's effect on the finish.
fn cumulate(c: &Calc, key: &'static str, basis: String, fidelity: Option<String>, mut q: Project, take_out: bool) -> Result<Impact, String> {
    let first = finish(&q)?;
    let mut last = first;
    let mut total = 0.0;
    let mut evs = events(c.p);
    if take_out { evs.reverse() }
    let mut out = vec![];
    for e in evs {
        let notes = if take_out { remove(&mut q, e) } else { add(&mut q, e) };
        let now = finish(&q)?;
        // Taking an event out shows its effect as the finish it saves (a positive delay).
        let impact = if take_out { wd(c, now, last) } else { wd(c, last, now) };
        total += impact;
        out.push(EventImpact { id: e.id, name: e.name.clone(), date: e.date.map(|d| d.to_string()), before: (last - 1).div_euclid(DAY), after: (now - 1).div_euclid(DAY), impact, total: (total * 10.0f64).round() / 10.0, notes });
        last = now;
    }
    Ok(Impact { key, basis, fidelity, start_finish: (first - 1).div_euclid(DAY), end_finish: (last - 1).div_euclid(DAY), total: (total * 10.0f64).round() / 10.0, events: out })
}

/// Time impact: the events added to a copy of today's plan (at its status date), one after another.
pub fn tia(c: &Calc) -> Result<Impact, String> {
    let at = c.p.status.map_or("today's plan".to_string(), |d| format!("today's plan at its status date ({d})"));
    cumulate(c, "tia", format!("A copy of {at}; work not yet done is moved past the status date as usual."), None, c.p.clone(), false)
}

/// The plan as agreed: baseline durations, no progress, only the tasks the baseline has.
fn as_planned(p: &Project) -> Project {
    let mut q = p.clone();
    let s = crate::sched::schedule(p).ok();
    let keep: std::collections::HashSet<u32> = q.tasks.iter().enumerate().filter(|(i, t)| s.as_ref().is_some_and(|s| s.summary[*i]) || t.bls.contains_key(&0)).map(|(_, t)| t.uid).collect();
    q.tasks.retain(|t| keep.contains(&t.uid));
    for t in &mut q.tasks {
        t.preds.retain(|l| keep.contains(&l.pred));
        if let Some(b) = t.bls.get(&0) { t.dur = b.dur }
        (t.pct, t.phys, t.act_start, t.act_finish) = (0, 0, None, None);
        t.steps.iter_mut().for_each(|x| x.done = false);
    }
    q.status = None;
    q.normalize();
    q
}

/// Impacted as-planned: the events added to the plan as agreed, one after another.
pub fn iap(c: &Calc) -> Result<Impact, String> {
    let q = as_planned(c.p);
    let rebuilt = finish(&q)?;
    let base = c.p.tasks.iter().filter_map(|t| t.bls.get(&0).map(|b| b.finish)).max();
    let fid = base.map(|b| format!("Rebuilt from today's logic with baseline durations: it finishes {}, the baseline {} ({} working days apart).", date_of(rebuilt - 1), date_of(b - 1), wd(c, b, rebuilt).abs()));
    cumulate(c, "iap", "The plan as agreed: baseline durations, no progress, today's logic (tasks added after the baseline left out).".into(), fid, q, false)
}

/// The plan as built: each task as long as it took (or is forecast to take), driven by its links,
/// with the gaps that actually happened kept as lags, and tasks without predecessors at their actual start.
fn as_built(c: &Calc) -> Project {
    let (p, s) = (c.p, &c.s);
    let mut q = p.clone();
    let es: std::collections::HashMap<u32, (i64, i64)> = p.tasks.iter().enumerate().map(|(i, t)| (t.uid, (s.es[i], s.ef[i]))).collect();
    for (i, t) in q.tasks.iter_mut().enumerate() {
        if s.summary[i] { continue }
        let cal = &s.cals[s.ci[i]];
        let (a, b) = (s.es[i], s.ef[i]);
        if t.act_start.is_some() { t.dur = cal.between(a, b) }
        // Only the driving predecessor (the one that finished last) keeps the gap that happened, so
        // taking time out of any path that drove the task moves it.
        let drv = t.preds.iter().filter(|l| l.kind == LinkType::FS && !l.pct && !l.elapsed).filter_map(|l| es.get(&l.pred).map(|x| (x.1, l.pred))).max();
        if let Some((pf, u)) = drv.filter(|(pf, _)| a > *pf) {
            if let Some(l) = t.preds.iter_mut().find(|l| l.pred == u && l.kind == LinkType::FS) { l.lag = l.lag.max(cal.between(pf, a)) }
        }
        if t.preds.is_empty() && t.act_start.is_some() { (t.constraint, t.cdate) = (Constraint::SNET, Some(date_of(a))) }
        (t.pct, t.phys, t.act_start, t.act_finish) = (0, 0, None, None);
    }
    q.status = None;
    q
}

/// Collapsed as-built: the events taken out of the plan as built, latest first; each one's effect is
/// the finish it saves.
pub fn cab(c: &Calc) -> Result<Impact, String> {
    let q = as_built(c);
    let rebuilt = finish(&q)?;
    let fid = Some(format!("Rebuilt from actual dates and today's logic: it finishes {}, today's plan {} ({} working days apart).", date_of(rebuilt - 1), date_of(c.s.finish - 1), wd(c, c.s.finish, rebuilt).abs()));
    cumulate(c, "cab", "The plan as built: actual durations, the gaps that happened kept as lags; each event taken out, latest first.".into(), fid, q, true)
}
