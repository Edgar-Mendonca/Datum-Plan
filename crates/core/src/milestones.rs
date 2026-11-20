//! The Milestones view: every milestone of the plan with its forecast against each baseline, its
//! deadline and contract date, the float left, how far it moved since the last period close, and a
//! status in words. The engine works it all out; the window only shows it.
use crate::{cal::{date_of, DAY}, fields::Calc, model::*};
use chrono::NaiveDate;
use serde::Serialize;

#[derive(Serialize, Debug, Default, Clone, PartialEq)]
pub struct MsRow {
    pub uid: u32,
    /// The task's row number and its Activity ID.
    pub id: usize,
    pub code: String,
    pub name: String,
    /// The top-level summary it sits under.
    pub phase: String,
    pub forecast: Option<NaiveDate>,
    pub actual: Option<NaiveDate>,
    pub done: bool,
    /// Each saved baseline's date: (number, date).
    pub baselines: Vec<(u8, NaiveDate)>,
    /// Days later (+) or earlier (−) than the baseline compared with, calendar days.
    pub var: Option<i64>,
    pub deadline: Option<NaiveDate>,
    /// The contract date it meets (Contract Register), and that date.
    pub contract: String,
    pub contract_date: Option<NaiveDate>,
    /// Total float in working days (not when done).
    pub float_days: Option<i64>,
    pub critical: bool,
    /// Days it moved since the last period close (+ later).
    pub moved: Option<i64>,
    /// The forecast at each period close, then now: (date, forecast).
    pub trend: Vec<(NaiveDate, NaiveDate)>,
    /// "Achieved", "Achieved late", "Late", "Slipping", "At risk", "On track", "No baseline".
    pub status: String,
    /// "ok", "warn", "bad" or "".
    pub key: String,
}

#[derive(Serialize, Debug, Default, Clone, PartialEq)]
pub struct MsOut {
    pub rows: Vec<MsRow>,
    /// The baselines the plan has.
    pub baselines: Vec<u8>,
    pub day: Option<NaiveDate>,
    pub achieved: usize,
    pub late: usize,
    /// Moved later since the last period close (late ones included).
    pub slipping: usize,
    pub at_risk: usize,
}

/// A milestone: a task of no duration or a start or finish milestone (summaries and inactive tasks left out).
pub fn is_milestone(c: &Calc, i: usize) -> bool {
    let t = &c.p.tasks[i];
    !c.s.summary[i] && !c.s.off[i] && (t.dur == 0 || matches!(t.act_type, ActType::StartMs | ActType::FinishMs))
}

/// The milestones, compared with baseline `base`; at risk with `warn` working days of float or less.
pub fn list(c: &Calc, base: u8, warn: i64) -> MsOut {
    let (p, s) = (c.p, &c.s);
    let day = date_of(c.progress_at() - DAY);
    let mut snaps: Vec<&Snapshot> = p.snapshots.iter().filter(|x| x.date.is_some()).collect();
    snaps.sort_by_key(|x| x.date);
    let date = |start: i64, finish: i64| date_of((finish - 1).max(start));
    let mut out = MsOut { day: Some(day), ..Default::default() };
    let mut all: Vec<u8> = vec![];
    for i in (0..p.tasks.len()).filter(|&i| is_milestone(c, i)) {
        let t = &p.tasks[i];
        let done = c.pct[i] >= 100;
        let actual = if done { t.act_finish.or(t.act_start) } else { None };
        let forecast = date(s.es[i], s.ef[i]);
        let baselines: Vec<(u8, NaiveDate)> = t.bls.iter().map(|(n, b)| (*n, date(b.start, b.finish))).collect();
        for (n, _) in &baselines { if !all.contains(n) { all.push(*n) } }
        let bdate = baselines.iter().find(|x| x.0 == base).map(|x| x.1);
        let when = actual.unwrap_or(forecast);
        let var = bdate.map(|b| (when - b).num_days());
        let mut trend: Vec<(NaiveDate, NaiveDate)> = snaps.iter().filter_map(|x| { let r = x.rows.iter().find(|r| r.uid == t.uid)?; Some((x.date?, date(r.start, r.finish))) }).collect();
        let moved = trend.last().map(|l| (forecast - l.1).num_days());
        // Now as the last point (a period closed today shows today's forecast, as the trend chart does).
        if trend.last().is_none_or(|l| l.0 < day) { trend.push((day, when)) } else if let Some(l) = trend.last_mut() { l.1 = when }
        let float_days = (!done).then(|| s.tf[i].div_euclid(p.day_min.max(1)));
        let (contract, contract_date) = p.contract.dates.iter().find(|d| d.task == t.uid).map_or((String::new(), None), |d| (d.name.clone(), d.date));
        let target = [bdate, t.deadline, contract_date].into_iter().flatten().min();
        let (status, key) = if done {
            if target.is_some_and(|d| when > d) { ("Achieved late", "warn") } else { ("Achieved", "ok") }
        } else if target.is_some_and(|d| forecast > d) || float_days.is_some_and(|f| f < 0) { ("Late", "bad") }
        else if moved.is_some_and(|m| m > 0) { ("Slipping", "warn") }
        else if float_days.is_some_and(|f| f <= warn) && s.critical(p, i) { ("At risk", "warn") }
        else if bdate.is_none() && t.deadline.is_none() && contract_date.is_none() { ("No baseline", "") }
        else { ("On track", "ok") };
        match status { "Achieved" | "Achieved late" => out.achieved += 1, "Late" => out.late += 1, "At risk" => out.at_risk += 1, _ => {} }
        if !done && moved.is_some_and(|m| m > 0) { out.slipping += 1 }
        let phase = (0..i).rev().find(|&k| p.tasks[k].level == 0 && p.tasks.get(k + 1).is_some_and(|n| n.level > 0)).filter(|_| t.level > 0).map_or(String::new(), |k| p.tasks[k].name.clone());
        out.rows.push(MsRow {
            uid: t.uid, id: i + 1, code: t.code.clone(), name: t.name.clone(), phase, forecast: Some(forecast), actual, done, baselines, var,
            deadline: t.deadline, contract, contract_date, float_days, critical: s.critical(p, i), moved, trend, status: status.into(), key: key.into(),
        });
    }
    all.sort();
    out.baselines = all;
    out
}

#[cfg(test)]
mod t {
    use super::*;
    use crate::doc::{Doc, Op};
    const F: &str = "%Y-%m-%d";
    fn d(m: u32, dd: u32) -> NaiveDate { NaiveDate::from_ymd_opt(2026, m, dd).unwrap() }

    #[test]
    fn milestones_against_their_baselines_deadlines_and_period_closes() {
        let mut doc = Doc::new(crate::templates::build(&crate::templates::TEMPLATES[0], d(11, 2)));
        let today = d(11, 2);
        let ms = |doc: &Doc| list(&Calc::new(&doc.p, F, today), 0, 5);
        let o = ms(&doc);
        assert!(o.rows.len() >= 2 && o.rows.iter().all(|r| r.var.is_none() && r.status != "Late") && o.baselines.is_empty(), "no baseline yet: nothing to be late against");
        doc.apply(Op::Baseline { uids: vec![], clear: false, n: 0 }, F).unwrap();
        doc.apply(Op::Snapshot { label: "Week 45".into(), comment: String::new() }, F).unwrap();
        let o = ms(&doc);
        assert_eq!(o.baselines, vec![0]);
        assert!(o.rows.iter().all(|r| r.var == Some(0) && r.moved == Some(0) && r.status != "Late"), "on the baseline");
        // The first task takes 10 more days: the milestones after it are late and slipping.
        let first = doc.p.tasks.iter().position(|t| t.dur > 0 && !doc.p.tasks.iter().any(|x| x.level > t.level)).unwrap();
        let uid = doc.p.tasks[first].uid;
        let dur = crate::fields::Calc::new(&doc.p, F, today).text(first, "dur");
        let n: f64 = dur.split_whitespace().next().unwrap().parse().unwrap();
        doc.apply(Op::Set { uid, field: "dur".into(), value: format!("{}d", n + 10.0) }, F).unwrap();
        doc.apply(Op::Baseline { uids: vec![], clear: false, n: 1 }, F).unwrap();
        let o = ms(&doc);
        let late = o.rows.iter().find(|r| r.var.is_some_and(|v| v > 0)).expect("a milestone after it moves");
        assert_eq!((late.status.as_str(), late.key.as_str()), ("Late", "bad"));
        assert!(late.moved.is_some_and(|m| m > 0) && late.trend.last().map(|x| x.1) == late.forecast, "moved since Week 45: {late:?}");
        assert_eq!(o.baselines, vec![0, 1]);
        assert_eq!(o.slipping, o.rows.iter().filter(|r| r.moved.is_some_and(|m| m > 0)).count(), "every milestone that moved counts, late or not");
        assert_eq!(list(&Calc::new(&doc.p, F, today), 1, 5).rows.iter().find(|r| r.uid == late.uid).unwrap().var, Some(0), "against baseline 1 it is on time");
        // A deadline before the forecast makes it late too; an achieved one says so.
        let last = o.rows.last().unwrap().uid;
        doc.apply(Op::Set { uid: last, field: "deadline".into(), value: "2026-11-03".into() }, F).unwrap();
        assert_eq!(list(&Calc::new(&doc.p, F, today), 1, 5).rows.last().unwrap().status, "Late");
    }
}
