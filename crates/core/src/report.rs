//! Timephased data: Task Usage and Resource Usage grids, cash flow and S-curves, and the
//! period figures behind progress reports.
use crate::cal::{date_of, DAY};
use crate::fields::Calc;
use crate::load::buckets;
use crate::model::ResKind;
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Serialize)]
pub struct UseRow {
    /// Task or resource uid; `of` is the parent row for assignment rows.
    pub uid: u32,
    pub of: Option<u32>,
    pub name: String,
    pub lvl: u8,
    pub total: f64,
    pub vals: Vec<f64>,
    pub over: Vec<bool>,
}

#[derive(Serialize)]
pub struct Usage { pub buckets: Vec<i64>, pub unit: String, pub rows: Vec<UseRow> }

fn bucket_of(b: &[i64], d: i64) -> Option<usize> {
    if b.is_empty() || d < b[0] {
        return None;
    }
    Some(b.partition_point(|&x| x <= d) - 1)
}

/// Usage grid: `by` is "task" or "resource", `what` is "work" (hours) or "cost".
pub fn usage(c: &Calc, by: &str, what: &str, unit: &str) -> Usage {
    let (p, s, l) = (c.p, &c.s, &c.l);
    let b = buckets(s.start.div_euclid(DAY), (s.finish - 1).max(s.start).div_euclid(DAY), unit);
    let cost = what == "cost";
    let empty = || vec![0.0; b.len()];
    let mut rows = vec![];
    let rate = |k: usize, i: usize, d: i64| {
        let a = p.tasks[i].res.iter().find(|a| p.res_idx(a.res) == Some(k)).map_or(0, |a| a.table);
        p.resources[k].rate_on(a, date_of(d * DAY)).0
    };
    if by == "resource" {
        let mut per: BTreeMap<(usize, usize), Vec<f64>> = BTreeMap::new();
        for (&(k, i, d), &m) in &l.assign_daily {
            if let Some(j) = bucket_of(&b, d) {
                per.entry((k, i)).or_insert_with(empty)[j] += if cost { m / 60.0 * rate(k, i, d) } else { m / 60.0 };
            }
        }
        let pc = &s.cals[p.calendars.iter().position(|c| c.name == p.calendar).unwrap_or(0)];
        for (k, r) in p.resources.iter().enumerate() {
            let kids: Vec<(&(usize, usize), &Vec<f64>)> = per.range((k, 0)..(k + 1, 0)).collect();
            let mut vals = empty();
            kids.iter().for_each(|(_, v)| v.iter().enumerate().for_each(|(j, x)| vals[j] += x));
            let over = b.iter().enumerate().map(|(j, &d0)| {
                let end = b.get(j + 1).copied().unwrap_or(d0 + 1);
                r.kind == ResKind::Work && (d0..end).any(|d| l.daily.get(&(k, d)).is_some_and(|&m| m > r.max_on(date_of(d * DAY)) * pc.day_minutes(date_of(d * DAY)) as f64 + 0.5))
            }).collect();
            rows.push(UseRow { uid: r.uid, of: None, name: r.name.clone(), lvl: 0, total: vals.iter().sum(), vals, over });
            for ((_, i), v) in kids {
                rows.push(UseRow { uid: p.tasks[*i].uid, of: Some(r.uid), name: p.tasks[*i].name.clone(), lvl: 1, total: v.iter().sum(), vals: v.clone(), over: vec![] });
            }
        }
    } else {
        let mut per: Vec<Vec<f64>> = vec![empty(); p.tasks.len()];
        for (&(i, d), &(m, cst)) in &l.task_daily {
            if let Some(j) = bucket_of(&b, d) {
                per[i][j] += if cost { cst } else { m / 60.0 };
            }
        }
        for i in (0..p.tasks.len()).rev().filter(|&i| s.summary[i]) {
            for j in p.subtree(i).skip(1).filter(|&j| !s.summary[j]) {
                let leaf = per[j].clone();
                per[i].iter_mut().zip(leaf).for_each(|(a, x)| *a += x);
            }
        }
        for (i, t) in p.tasks.iter().enumerate().filter(|&(i, _)| !s.off[i]) {
            rows.push(UseRow { uid: t.uid, of: None, name: t.name.clone(), lvl: t.level, total: per[i].iter().sum(), vals: per[i].clone(), over: vec![] });
            if s.summary[i] {
                continue;
            }
            for a in &t.res {
                let Some(k) = p.res_idx(a.res) else { continue };
                let mut v = empty();
                for (&(_, _, d), &m) in l.assign_daily.range((k, i, i64::MIN)..=(k, i, i64::MAX)) {
                    if let Some(j) = bucket_of(&b, d) {
                        v[j] += if cost { m / 60.0 * rate(k, i, d) } else { m / 60.0 };
                    }
                }
                rows.push(UseRow { uid: p.resources[k].uid, of: Some(t.uid), name: p.resources[k].name.clone(), lvl: t.level + 1, total: v.iter().sum(), vals: v, over: vec![] });
            }
        }
    }
    Usage { buckets: b, unit: unit.into(), rows }
}

/// Cash flow and S-curve data per bucket: baseline (planned value), current forecast and actual cost.
#[derive(Serialize)]
pub struct Curve { pub buckets: Vec<i64>, pub planned: Vec<f64>, pub forecast: Vec<f64>, pub actual: Vec<f64>, pub earned: Vec<f64> }

pub fn curve(c: &Calc, unit: &str) -> Curve {
    let (p, s, l) = (c.p, &c.s, &c.l);
    let leaves: Vec<usize> = (0..p.tasks.len()).filter(|&i| !s.summary[i] && !s.off[i]).collect();
    let lo = leaves.iter().filter_map(|&i| p.tasks[i].bls.get(&0).map(|b| b.start)).chain([s.start]).min().unwrap_or(s.start);
    let hi = leaves.iter().filter_map(|&i| p.tasks[i].bls.get(&0).map(|b| b.finish)).chain([s.finish]).max().unwrap_or(s.finish);
    let b = buckets(lo.div_euclid(DAY), (hi - 1).max(lo).div_euclid(DAY), unit);
    let z = vec![0.0; b.len()];
    let (mut planned, mut forecast, mut actual, mut earned) = (z.clone(), z.clone(), z.clone(), z);
    let dd = c.status_at().div_euclid(DAY);
    for &i in &leaves {
        let t = &p.tasks[i];
        let cal = &s.cals[s.ci[i]];
        if let Some(bl) = t.bls.get(&0) {
            let tot = cal.between(bl.start, bl.finish).max(1) as f64;
            let mut d = bl.start.div_euclid(DAY);
            let done = t.pct as f64 / 100.0;
            if bl.finish <= bl.start {
                if let Some(j) = bucket_of(&b, d) {
                    planned[j] += bl.cost;
                }
            }
            while d * DAY < bl.finish {
                let share = cal.between(bl.start.max(d * DAY), bl.finish.min((d + 1) * DAY)) as f64 / tot;
                if let Some(j) = bucket_of(&b, d) {
                    planned[j] += bl.cost * share;
                    if d < dd {
                        earned[j] += bl.cost * share * done;
                    }
                }
                d += 1;
            }
        }
        let mut before = 0.0;
        for (&(_, d), &(_, cost)) in l.task_daily.range((i, i64::MIN)..=(i, i64::MAX)) {
            if let Some(j) = bucket_of(&b, d) {
                forecast[j] += cost;
                if d < dd {
                    before += cost;
                }
            }
        }
        // Actual cost is spread like the forecast up to the status date.
        let scale = if before > 0.0 { l.acost[i] / before } else { 0.0 };
        for (&(_, d), &(_, cost)) in l.task_daily.range((i, i64::MIN)..=(i, i64::MAX)) {
            if let Some(j) = bucket_of(&b, d).filter(|_| d < dd) {
                actual[j] += cost * scale;
            }
        }
    }
    Curve { buckets: b, planned, forecast, actual, earned }
}

/// One task's difference between two versions of a plan (Compare Projects).
#[derive(Serialize)]
pub struct Diff { pub uid: u32, pub name: String, pub change: &'static str, pub was: [String; 4], pub now: [String; 4], pub finish_var: String }

/// Compares `old` with the current plan by unique ID: start, finish, duration and cost.
pub fn compare(old: &Calc, new: &Calc) -> Vec<Diff> {
    let keys = ["start", "finish", "dur", "cost"];
    let pick = |c: &Calc, i: usize| keys.map(|k| c.text(i, k));
    let mut out = vec![];
    for (i, t) in new.p.tasks.iter().enumerate() {
        match old.p.idx(t.uid) {
            None => out.push(Diff { uid: t.uid, name: t.name.clone(), change: "Added", was: Default::default(), now: pick(new, i), finish_var: String::new() }),
            Some(j) => {
                let (a, b) = (pick(old, j), pick(new, i));
                if a != b || old.p.tasks[j].name != t.name {
                    let c = &new.s.cals[new.s.ci[i]];
                    let (x, y) = (old.s.ef[j], new.s.ef[i]);
                    let var = if y >= x { c.between(x, y) } else { -c.between(y, x) };
                    out.push(Diff { uid: t.uid, name: t.name.clone(), change: "Changed", was: a, now: b, finish_var: crate::parse::fmt_dur(var, crate::model::Unit::D, new.p.day_min) });
                }
            }
        }
    }
    for (j, t) in old.p.tasks.iter().enumerate().filter(|(_, t)| new.p.idx(t.uid).is_none()) {
        out.push(Diff { uid: t.uid, name: t.name.clone(), change: "Removed", was: pick(old, j), now: Default::default(), finish_var: String::new() });
    }
    out
}
