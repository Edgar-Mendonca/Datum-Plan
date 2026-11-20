//! The Workload view: people against weeks (or months), by trade. Each cell is the hours booked
//! against the hours the person can work there; a person over capacity in a period shows it, and
//! the tasks that make up each person's hours are listed under them. Moving a task to another
//! person, or later, is one change (`move_ops`), applied by `Op::WorkMove`.
use crate::{cal::date_of, doc::Op, fields::Calc, model::*};
use chrono::NaiveDate;
use serde::Serialize;

#[derive(Serialize, Debug, Default, Clone, PartialEq)]
pub struct WTask {
    pub uid: u32,
    pub name: String,
    pub hours: Vec<f64>,
    pub total: f64,
    pub crit: bool,
    pub start: Option<NaiveDate>,
    pub finish: Option<NaiveDate>,
}

#[derive(Serialize, Debug, Default, Clone, PartialEq)]
pub struct WPerson {
    pub uid: u32,
    pub name: String,
    pub trade: String,
    pub booked: Vec<f64>,
    pub cap: Vec<f64>,
    pub over: Vec<bool>,
    /// The highest share of capacity booked in a period (%).
    pub peak: f64,
    pub tasks: Vec<WTask>,
}

#[derive(Serialize, Debug, Default, Clone, PartialEq)]
pub struct WTrade {
    pub name: String,
    pub booked: Vec<f64>,
    pub cap: Vec<f64>,
    pub people: Vec<WPerson>,
}

#[derive(Serialize, Debug, Default, Clone, PartialEq)]
pub struct WOut {
    /// The first day of each period.
    pub periods: Vec<NaiveDate>,
    pub unit: String,
    pub trades: Vec<WTrade>,
    /// People over capacity in some period, and person-periods over.
    pub over_people: usize,
    pub over_cells: usize,
    /// Booked and free hours over the whole plan (free: capacity not booked).
    pub booked: f64,
    pub free: f64,
}

/// The workload by week ("week") or month ("month"), work resources only, grouped by trade.
pub fn workload(c: &Calc, unit: &str) -> WOut {
    let p = c.p;
    let u = crate::report::usage(c, "resource", "work", unit);
    let mut out = WOut { periods: u.buckets.iter().map(|d| date_of(d * crate::cal::DAY)).collect(), unit: u.unit.clone(), ..Default::default() };
    let n = u.buckets.len();
    let mut people: Vec<WPerson> = vec![];
    for r in &u.rows {
        match r.of {
            None => {
                let Some(res) = p.resources.iter().find(|x| x.uid == r.uid).filter(|x| x.kind == ResKind::Work) else { continue };
                let cap = if r.cap.len() == n { r.cap.clone() } else { vec![0.0; n] };
                let peak = (0..n).filter(|&j| cap[j] > 0.0).map(|j| r.vals[j] / cap[j] * 100.0).fold(0.0, f64::max);
                people.push(WPerson { uid: r.uid, name: r.name.clone(), trade: if res.group.trim().is_empty() { "No trade".into() } else { res.group.trim().into() },
                    booked: r.vals.clone(), over: r.over.clone(), cap, peak: (peak * 10.0).round() / 10.0, tasks: vec![] });
            }
            Some(of) => {
                let Some(pp) = people.last_mut().filter(|x| x.uid == of) else { continue };
                let i = p.idx(r.uid);
                pp.tasks.push(WTask { uid: r.uid, name: r.name.clone(), hours: r.vals.clone(), total: r.total, crit: i.is_some_and(|i| c.s.critical(p, i)),
                    start: i.map(|i| p.tasks[i].act_start.unwrap_or_else(|| date_of(c.s.es[i]))), finish: i.map(|i| p.tasks[i].act_finish.unwrap_or_else(|| date_of(c.s.ef[i] - 1))) });
            }
        }
    }
    for pp in &people {
        out.booked += pp.booked.iter().sum::<f64>();
        out.free += (0..n).map(|j| (pp.cap[j] - pp.booked[j]).max(0.0)).sum::<f64>();
        let o = pp.over.iter().filter(|x| **x).count();
        out.over_cells += o;
        out.over_people += usize::from(o > 0);
    }
    let mut trades: Vec<WTrade> = vec![];
    for pp in people {
        let k = match trades.iter().position(|t| t.name == pp.trade) { Some(k) => k, None => { trades.push(WTrade { name: pp.trade.clone(), booked: vec![0.0; n], cap: vec![0.0; n], people: vec![] }); trades.len() - 1 } };
        let t = &mut trades[k];
        for j in 0..n { t.booked[j] += pp.booked[j]; t.cap[j] += pp.cap[j]; }
        t.people.push(pp);
    }
    trades.sort_by(|a, b| (a.name == "No trade").cmp(&(b.name == "No trade")).then(a.name.cmp(&b.name)));
    out.trades = trades;
    out
}

/// The changes that move a task's work: from one person to another (`to`, keeping the units;
/// added to theirs when they are on it already), and/or later, to start no earlier than `start`.
pub fn move_ops(p: &Project, uid: u32, from: u32, to: u32, start: &str, fmt: &str) -> Result<Vec<Op>, String> {
    let i = p.idx(uid).ok_or("No task with that id.")?;
    let t = &p.tasks[i];
    let mut ops = vec![];
    if to != 0 && to != from {
        if !p.resources.iter().any(|r| r.uid == to) { return Err("No resource with that id.".into()) }
        let k = t.res.iter().position(|a| a.res == from).ok_or("That person is not on this task.")?;
        let mut res = t.res.clone();
        let moved = res.remove(k);
        match res.iter_mut().find(|a| a.res == to) { Some(a) => a.units += moved.units, None => res.insert(k, Assign { res: to, ..moved }) }
        ops.push(Op::Set { uid, field: "res".into(), value: crate::parse::fmt_res(&res, p) });
    }
    if !start.trim().is_empty() {
        if t.act_start.is_some() { return Err("This task has started: move what is left by splitting it instead.".into()) }
        let d = NaiveDate::parse_from_str(start.trim(), "%Y-%m-%d").or_else(|_| crate::parse::date_in(start, fmt)).map_err(|_| format!("Not a date: {start}."))?;
        ops.push(Op::Set { uid, field: "start".into(), value: d.format(fmt).to_string() });
    }
    if ops.is_empty() { return Err("Nothing to move.".into()) }
    Ok(ops)
}

#[cfg(test)]
mod t {
    use super::*;
    use crate::doc::Doc;
    const F: &str = "%Y-%m-%d";
    fn d(m: u32, dd: u32) -> NaiveDate { NaiveDate::from_ymd_opt(2026, m, dd).unwrap() }

    #[test]
    fn people_against_weeks_by_trade_and_moving_work() {
        let mut doc = Doc::new(crate::templates::build(&crate::templates::TEMPLATES[0], d(11, 2)));
        let w = |doc: &Doc| workload(&Calc::new(&doc.p, F, d(11, 2)), "week");
        let o = w(&doc);
        assert!(!o.periods.is_empty() && !o.trades.is_empty());
        let people: Vec<&WPerson> = o.trades.iter().flat_map(|t| &t.people).collect();
        assert!(people.iter().all(|x| x.booked.len() == o.periods.len() && x.cap.len() == o.periods.len()));
        for t in &o.trades { for j in 0..o.periods.len() { assert!((t.booked[j] - t.people.iter().map(|x| x.booked[j]).sum::<f64>()).abs() < 1e-6, "a trade adds up its people") } }
        let total: f64 = people.iter().map(|x| x.booked.iter().sum::<f64>()).sum();
        assert!((total - o.booked).abs() < 1e-6 && total > 0.0);
        // A person over capacity (a 400 % assignment): the cells and the people over are counted.
        assert!(o.over_people >= 1 && o.over_cells >= 1 && people.iter().any(|x| x.peak > 100.0), "the template books someone over capacity");
        // Moving a task from the busiest person to another, then later.
        let busy = people.iter().max_by(|a, b| a.peak.total_cmp(&b.peak)).unwrap();
        let task = busy.tasks.iter().max_by(|a, b| a.total.total_cmp(&b.total)).unwrap();
        let other = people.iter().find(|x| x.uid != busy.uid && !x.tasks.iter().any(|t| t.uid == task.uid)).unwrap();
        let (bu, ou, tu, bt) = (busy.uid, other.uid, task.uid, busy.tasks.iter().map(|t| t.total).sum::<f64>());
        doc.apply(Op::WorkMove { uid: tu, from: bu, to: ou, start: String::new() }, F).unwrap();
        let o2 = w(&doc);
        let find = |o: &WOut, uid: u32| o.trades.iter().flat_map(|t| &t.people).find(|x| x.uid == uid).cloned().unwrap();
        assert!(find(&o2, bu).tasks.iter().all(|t| t.uid != tu) && find(&o2, ou).tasks.iter().any(|t| t.uid == tu), "the task moved to the other person");
        assert!(find(&o2, bu).tasks.iter().map(|t| t.total).sum::<f64>() < bt);
        assert!(doc.apply(Op::WorkMove { uid: tu, from: bu, to: ou, start: String::new() }, F).is_err(), "they are no longer on it");
        doc.undo();
        assert!(find(&w(&doc), bu).tasks.iter().any(|t| t.uid == tu), "one Undo step");
        doc.apply(Op::WorkMove { uid: tu, from: bu, to: 0, start: "2026-12-07".into() }, F).unwrap();
        let t = find(&w(&doc), bu).tasks.into_iter().find(|t| t.uid == tu).unwrap();
        assert!(t.start.is_some_and(|s| s >= d(12, 7)), "moved later: it starts no earlier than the week chosen: {:?}", t.start);
    }
}
