//! Resource loading: work and cost per task and resource, daily loads, overallocation and workload.
use crate::cal::{at, date_of, DAY};
use crate::model::{Project, ResKind};
use crate::sched::Sched;
use serde::Serialize;
use std::collections::BTreeMap;

pub struct Loads {
    /// Work minutes and cost per task (summaries roll up).
    pub work: Vec<f64>,
    pub cost: Vec<f64>,
    /// Per resource.
    pub res_work: Vec<f64>,
    pub res_cost: Vec<f64>,
    pub over: Vec<bool>,
    /// (resource index, epoch day) → working minutes assigned.
    pub daily: BTreeMap<(usize, i64), f64>,
}

pub fn compute(p: &Project, s: &Sched) -> Loads {
    let (n, m) = (p.tasks.len(), p.resources.len());
    let mut l = Loads { work: vec![0.0; n], cost: vec![0.0; n], res_work: vec![0.0; m], res_cost: vec![0.0; m], over: vec![false; m], daily: BTreeMap::new() };
    for (i, t) in p.tasks.iter().enumerate().filter(|&(i, _)| !s.summary[i] && !s.off[i]) {
        let c = &s.cals[s.ci[i]];
        for a in &t.res {
            let Some(k) = p.res_idx(a.res) else { continue };
            let r = &p.resources[k];
            let (w, cost) = match r.kind {
                ResKind::Work => (t.dur as f64 * a.units, t.dur as f64 * a.units / 60.0 * r.rate + r.per_use),
                ResKind::Material => (0.0, a.units * r.rate + r.per_use),
                ResKind::Cost => (0.0, a.units + r.per_use),
            };
            l.work[i] += w;
            l.cost[i] += cost;
            l.res_work[k] += w;
            l.res_cost[k] += cost;
            if r.kind == ResKind::Work && a.units > 0.0 {
                let mut d = date_of(s.es[i]);
                while at(d) < s.ef[i] {
                    let mins = c.between(s.es[i].max(at(d)), s.ef[i].min(at(d) + DAY));
                    if mins > 0 {
                        *l.daily.entry((k, at(d) / DAY)).or_default() += mins as f64 * a.units;
                    }
                    d = d.succ_opt().unwrap();
                }
            }
        }
    }
    let pc = &s.cals[p.calendars.iter().position(|c| c.name == p.calendar).unwrap_or(0)];
    for (&(k, day), &mins) in &l.daily {
        let cap = p.resources[k].max * pc.day_minutes(date_of(day * DAY)).max(0) as f64;
        if mins > cap + 0.5 {
            l.over[k] = true;
        }
    }
    for i in (0..n).filter(|&i| s.summary[i]) {
        for j in p.subtree(i).skip(1).filter(|&j| !s.summary[j]) {
            l.work[i] += l.work[j];
            l.cost[i] += l.cost[j];
        }
    }
    l
}

#[derive(Serialize)]
pub struct Workload {
    /// First day of each bucket (days since 1970-01-01).
    pub buckets: Vec<i64>,
    pub rows: Vec<WorkRow>,
}

#[derive(Serialize)]
pub struct WorkRow {
    pub uid: u32,
    pub name: String,
    /// Hours assigned and hours available per bucket.
    pub load: Vec<f64>,
    pub cap: Vec<f64>,
}

/// Hours per resource per week (or day) across the project.
pub fn workload(p: &Project, s: &Sched, weekly: bool) -> Workload {
    let l = compute(p, s);
    let pc = &s.cals[p.calendars.iter().position(|c| c.name == p.calendar).unwrap_or(0)];
    let first = s.start.div_euclid(DAY);
    let first = if weekly { first - (first + 3).rem_euclid(7) } else { first };
    let last = s.finish.div_euclid(DAY) + 1;
    let step = if weekly { 7 } else { 1 };
    let buckets: Vec<i64> = (first..last).step_by(step).collect();
    let rows = p.resources.iter().enumerate().filter(|(_, r)| r.kind == ResKind::Work).map(|(k, r)| {
        let (mut load, mut cap) = (vec![0.0; buckets.len()], vec![0.0; buckets.len()]);
        for (b, &d0) in buckets.iter().enumerate() {
            for d in d0..d0 + step as i64 {
                load[b] += l.daily.get(&(k, d)).copied().unwrap_or(0.0) / 60.0;
                cap[b] += r.max * pc.day_minutes(date_of(d * DAY)) as f64 / 60.0;
            }
        }
        WorkRow { uid: r.uid, name: r.name.clone(), load, cap }
    }).collect();
    Workload { buckets, rows }
}
