//! Resource loading and costs, day by day: work and cost per task, assignment and resource,
//! overallocation, and the timephased data behind usage views, cash flow and earned value.
use crate::cal::{date_of, DAY};
use crate::model::{Accrual, Project, ResKind};
use crate::sched::Sched;
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Default)]
pub struct Loads {
    /// Work minutes, cost, actual work and actual cost per task (summaries roll up).
    pub work: Vec<f64>,
    pub cost: Vec<f64>,
    pub awork: Vec<f64>,
    pub acost: Vec<f64>,
    /// Per resource.
    pub res_work: Vec<f64>,
    pub res_cost: Vec<f64>,
    pub over: Vec<bool>,
    /// (resource, day) → working minutes assigned.
    pub daily: BTreeMap<(usize, i64), f64>,
    /// (resource, task, day) → working minutes of that assignment.
    pub assign_daily: BTreeMap<(usize, usize, i64), f64>,
    /// (resource, task, day) → cost of that assignment (work, per use, material and cost resources).
    pub assign_cost: BTreeMap<(usize, usize, i64), f64>,
    /// (task, day) → (work minutes, cost) for leaf tasks.
    pub task_daily: BTreeMap<(usize, i64), (f64, f64)>,
}

/// The pauses of a split task as times, for work from `from` with `done` of its `dur` working minutes already done.
pub fn pauses(c: &crate::cal::WorkCal, from: i64, splits: &[(i64, i64)], dur: i64, done: i64) -> Vec<(i64, i64)> {
    let (mut pos, mut prev) = (from, done);
    splits.iter().filter(|&&(a, g)| a > done && a < dur && g > 0).map(|&(a, g)| {
        let p = c.add(pos, a - prev);
        (pos, prev) = (c.add(p, g), a);
        (p, pos)
    }).collect()
}

/// Working minutes in [a, b) outside the pauses `gaps`.
pub fn worked(c: &crate::cal::WorkCal, a: i64, b: i64, gaps: &[(i64, i64)]) -> i64 {
    (c.between(a, b) - gaps.iter().map(|&(x, y)| c.between(a.max(x), b.min(y)).max(0)).sum::<i64>()).max(0)
}

/// Spreads `amount` over the task's working days in proportion to working minutes, leaving out its pauses.
fn spread(c: &crate::cal::WorkCal, es: i64, ef: i64, gaps: &[(i64, i64)], mut f: impl FnMut(i64, f64)) {
    let total = worked(c, es, ef, gaps).max(1) as f64;
    if ef <= es {
        f(es.div_euclid(DAY), 1.0);
        return;
    }
    let mut d = es.div_euclid(DAY);
    while d * DAY < ef {
        let m = worked(c, es.max(d * DAY), ef.min((d + 1) * DAY), gaps);
        if m > 0 {
            f(d, m as f64 / total);
        }
        d += 1;
    }
}

/// Spreads a task's amounts like `spread`, split at the status date when work is under way (`cut`
/// = the status date and where the remaining work starts): the share `pct` over the actual start
/// up to the status date, the rest from the resume point to the finish. `sp` = (splits, duration).
fn phased(c: &crate::cal::WorkCal, es: i64, ef: i64, cut: Option<(i64, i64)>, pct: u8, sp: (&[(i64, i64)], i64), mut f: impl FnMut(i64, f64)) {
    let (done, dw) = (pct as f64 / 100.0, sp.1 * pct.min(100) as i64 / 100);
    let Some((dd, rs)) = cut else { return spread(c, es, ef, &pauses(c, es, sp.0, sp.1, 0), f) };
    if done > 0.0 {
        spread(c, es, dd.clamp(es, ef), &pauses(c, es, sp.0, sp.1, 0), |d, x| f(d, x * done));
    }
    spread(c, rs, ef, &pauses(c, rs, sp.0, sp.1, dw), |d, x| f(d, x * (1.0 - done)));
}

/// The days with work on them between `es` and `ef`, leaving out the pauses `gaps`.
pub fn working_days(c: &crate::cal::WorkCal, es: i64, ef: i64, gaps: &[(i64, i64)]) -> Vec<i64> {
    if ef <= es { return vec![] }
    (es.div_euclid(DAY)..=(ef - 1).div_euclid(DAY)).filter(|&d| worked(c, es.max(d * DAY), ef.min((d + 1) * DAY), gaps) > 0).collect()
}

/// The share of a contour's effort done by position `x` of the task (0 at its start, 1 at its end).
pub fn cum(w: &[f64], x: f64) -> f64 {
    let (n, tot) = (w.len(), w.iter().sum::<f64>());
    if n == 0 || tot <= 0.0 { return x.clamp(0.0, 1.0) }
    let x = x.clamp(0.0, 1.0) * n as f64;
    let k = (x.floor() as usize).min(n - 1);
    (w[..k].iter().sum::<f64>() + w[k] * (x - k as f64)) / tot
}

/// The position of the task by which `share` of a contour's effort is done (the inverse of `cum`).
pub fn at_share(w: &[f64], share: f64) -> f64 {
    let (n, tot) = (w.len(), w.iter().sum::<f64>());
    if n == 0 || tot <= 0.0 { return share.clamp(0.0, 1.0) }
    let mut left = share.clamp(0.0, 1.0) * tot;
    for (k, &x) in w.iter().enumerate() {
        if left <= x && x > 0.0 { return (k as f64 + left / x) / n as f64 }
        left -= x;
    }
    1.0
}

/// Spreads like `spread`, shaped by the contour `w`: the span covers positions `x0`..`x1` of the
/// contour, and each day takes its part of the effort there (the shares add up to 1).
fn spread_curve(c: &crate::cal::WorkCal, es: i64, ef: i64, gaps: &[(i64, i64)], w: &[f64], (x0, x1): (f64, f64), mut f: impl FnMut(i64, f64)) {
    let total = worked(c, es, ef, gaps).max(1) as f64;
    if ef <= es {
        f(es.div_euclid(DAY), 1.0);
        return;
    }
    let whole = cum(w, x1) - cum(w, x0);
    let (mut d, mut done) = (es.div_euclid(DAY), 0.0);
    while d * DAY < ef {
        let m = worked(c, es.max(d * DAY), ef.min((d + 1) * DAY), gaps) as f64;
        if m > 0.0 {
            let at = |u: f64| x0 + (x1 - x0) * (u / total).min(1.0);
            f(d, if whole > 1e-12 { (cum(w, at(done + m)) - cum(w, at(done))) / whole } else { m / total });
            done += m;
        }
        d += 1;
    }
}

/// `phased` for an assignment with a work contour `w` (None: flat). The work done (`pct`) is the
/// contour's effort up to the matching position; the rest follows the contour from there.
fn phased_w(c: &crate::cal::WorkCal, es: i64, ef: i64, (cut, pct): (Option<(i64, i64)>, u8), sp: (&[(i64, i64)], i64), w: Option<&[f64]>, mut f: impl FnMut(i64, f64)) {
    let Some(w) = w else { return phased(c, es, ef, cut, pct, sp, f) };
    let (done, dw) = (pct.min(100) as f64 / 100.0, sp.1 * pct.min(100) as i64 / 100);
    let Some((dd, rs)) = cut else { return spread_curve(c, es, ef, &pauses(c, es, sp.0, sp.1, 0), w, (0.0, 1.0), f) };
    let t = at_share(w, done);
    if done > 0.0 {
        spread_curve(c, es, dd.clamp(es, ef), &pauses(c, es, sp.0, sp.1, 0), w, (0.0, t), |d, x| f(d, x * done));
    }
    spread_curve(c, rs, ef, &pauses(c, rs, sp.0, sp.1, dw), w, (t, 1.0), |d, x| f(d, x * (1.0 - done)));
}

pub fn compute(p: &Project, s: &Sched) -> Loads {
    let (n, m) = (p.tasks.len(), p.resources.len());
    let mut l = Loads { work: vec![0.0; n], cost: vec![0.0; n], awork: vec![0.0; n], acost: vec![0.0; n], res_work: vec![0.0; m], res_cost: vec![0.0; m], over: vec![false; m], ..Default::default() };
    // A summary's % complete: its rows' weighted by duration (as the Calc shows it).
    let rolled = |i: usize| {
        let kids: Vec<_> = p.subtree(i).skip(1).filter(|&j| !s.summary[j] && !s.off[j]).map(|j| &p.tasks[j]).collect();
        let w: i64 = kids.iter().map(|k| k.dur).sum();
        (if w > 0 { kids.iter().map(|k| k.pct as f64 * k.dur as f64).sum::<f64>() / w as f64 } else { kids.iter().map(|k| k.pct as f64).sum::<f64>() / kids.len().max(1) as f64 }).round() as u8
    };
    // Leaves, and summaries for their own fixed cost and expenses.
    for (i, t) in p.tasks.iter().enumerate().filter(|&(i, _)| !s.off[i]) {
        let (c, es, ef) = (&s.cals[s.ci[i]], s.es[i], s.ef[i]);
        let pct = if s.summary[i] { rolled(i) } else { t.pct };
        let done = pct as f64 / 100.0;
        let first = date_of(es);
        // Under way at the status date: work done before it, the rest after it.
        let dd = p.status.map(|d| crate::cal::at(d) + DAY);
        let cut = dd.filter(|_| t.act_start.is_some() && t.act_finish.is_none() && t.pct < 100).map(|dd| (dd, s.rs[i]));
        // Level of effort and mirrored tasks last as long as their span; their work follows it.
        let span = s.loe[i] || s.mirror[i];
        let (dur, sp) = if span { (c.between(es, ef), (&[][..], 0)) } else { (t.dur, (&t.splits[..], t.dur)) };
        for a in t.res.iter().filter(|_| !s.summary[i]) {
            let Some(k) = p.res_idx(a.res) else { continue };
            let r = &p.resources[k];
            let (rate, per_use) = r.rate_on(a.table, first);
            let typed = r.kind == ResKind::Work && !a.daily.is_empty();
            let (w, cost) = match r.kind {
                ResKind::Work if typed => (a.daily.iter().sum::<f64>(), 0.0),
                ResKind::Work => (dur as f64 * a.units, 0.0),
                ResKind::Material => (0.0, a.units * rate),
                ResKind::Cost => (0.0, a.units),
            };
            let mut wcost = 0.0;
            // A work contour shapes the work and material over the task (cost resources stay even).
            let curve = if typed { Some(a.daily.clone()) } else { (!a.contour.is_flat() && r.kind != ResKind::Cost).then(|| a.contour.weights(&a.curve)) };
            let mut book = |d: i64, share: f64| {
                let mins = w * share;
                let day_cost = if r.kind == ResKind::Work { mins / 60.0 * r.rate_on(a.table, date_of(d * DAY)).0 } else { cost * share };
                wcost += day_cost;
                let e = l.task_daily.entry((i, d)).or_default();
                e.0 += mins;
                e.1 += day_cost;
                if mins > 0.0 {
                    *l.daily.entry((k, d)).or_default() += mins;
                    *l.assign_daily.entry((k, i, d)).or_default() += mins;
                }
                if day_cost != 0.0 {
                    *l.assign_cost.entry((k, i, d)).or_default() += day_cost;
                }
            };
            // Hours typed per day land on their days, as typed, while the task has as many working
            // days as were typed and nothing is under way; otherwise they shape the work like a curve.
            let days = if typed && cut.is_none() { working_days(c, es, ef, &pauses(c, es, sp.0, sp.1, 0)) } else { vec![] };
            if typed && days.len() == a.daily.len() {
                days.iter().zip(&a.daily).filter(|x| *x.1 > 0.0).for_each(|(&d, &m)| book(d, m / w));
            } else {
                phased_w(c, es, ef, (cut, pct), sp, curve.as_deref(), book);
            }
            l.task_daily.entry((i, es.div_euclid(DAY))).or_default().1 += per_use;
            if per_use != 0.0 {
                *l.assign_cost.entry((k, i, es.div_euclid(DAY))).or_default() += per_use;
            }
            let total = wcost + per_use;
            l.work[i] += w;
            l.cost[i] += total;
            l.awork[i] += w * done;
            // Cost per use is spent once the task starts.
            l.acost[i] += wcost * done + if pct > 0 { per_use } else { 0.0 };
            l.res_work[k] += w;
            l.res_cost[k] += total;
        }
        // Fixed cost and expenses, charged by their accrual.
        let mut charge = |amount: f64, accrual: Accrual, actual: f64| {
            if amount == 0.0 && actual == 0.0 {
                return;
            }
            match accrual {
                Accrual::Start => l.task_daily.entry((i, es.div_euclid(DAY))).or_default().1 += amount,
                Accrual::End => l.task_daily.entry((i, (ef - 1).max(es).div_euclid(DAY))).or_default().1 += amount,
                Accrual::Prorated => phased(c, es, ef, cut, pct, sp, |d, share| l.task_daily.entry((i, d)).or_default().1 += amount * share),
            }
            l.cost[i] += amount;
            l.acost[i] += actual;
        };
        let fixed_actual = match t.accrual { Accrual::Start if pct > 0 => t.fixed_cost, Accrual::End if pct == 100 => t.fixed_cost, Accrual::Prorated => t.fixed_cost * done, _ => 0.0 };
        charge(t.fixed_cost, t.accrual, fixed_actual);
        for e in &t.expenses {
            charge(e.planned.max(e.actual), e.accrual, e.actual);
        }
    }
    for (&(k, day), &mins) in &l.daily {
        let d = date_of(day * DAY);
        let cap = p.resources[k].max_on(d) * s.cals[s.rcal[k]].day_minutes(d).max(0) as f64;
        if mins > cap + 0.5 {
            l.over[k] = true;
        }
    }
    // Money in whole minor units (cents): each task's cost is rounded once, so every total is
    // the exact sum of the amounts shown.
    let unit = 10f64.powi(p.currency.decimals.min(4) as i32);
    let minor = |v: f64| (v * unit).round() / unit;
    for i in (0..n).filter(|&i| !s.summary[i]) {
        (l.cost[i], l.acost[i]) = (minor(l.cost[i]), minor(l.acost[i]));
    }
    l.res_cost.iter_mut().for_each(|v| *v = minor(*v));
    // Summaries add up their rows (a summary's own amounts are only its fixed cost and expenses).
    let own = (l.work.clone(), l.cost.clone(), l.awork.clone(), l.acost.clone());
    for i in (0..n).filter(|&i| s.summary[i]) {
        for j in p.subtree(i).skip(1).filter(|&j| !s.off[j]) {
            l.work[i] += own.0[j];
            l.cost[i] += own.1[j];
            l.awork[i] += own.2[j];
            l.acost[i] += own.3[j];
        }
        (l.cost[i], l.acost[i]) = (minor(l.cost[i]), minor(l.acost[i]));
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

/// Bucket starts from `first` to `last` (days) for day, week (Monday), month or quarter.
pub fn buckets(first: i64, last: i64, unit: &str) -> Vec<i64> {
    use chrono::Datelike;
    let mut out = vec![];
    let mut d = match unit {
        "week" => first - (first + 3).rem_euclid(7),
        "month" | "quarter" => crate::cal::at(date_of(first * DAY).with_day(1).unwrap()) / DAY,
        _ => first,
    };
    if unit == "quarter" {
        let x = date_of(d * DAY);
        d = crate::cal::at(chrono::NaiveDate::from_ymd_opt(x.year(), (x.month0() / 3) * 3 + 1, 1).unwrap()) / DAY;
    }
    while d <= last && out.len() < 5000 {
        out.push(d);
        d = match unit {
            "week" => d + 7,
            "month" | "quarter" => {
                let x = date_of(d * DAY);
                let months = if unit == "month" { 1 } else { 3 };
                crate::cal::at(x.checked_add_months(chrono::Months::new(months)).unwrap()) / DAY
            }
            _ => d + 1,
        };
    }
    out
}

/// Hours per resource per week (or day) across the project.
pub fn workload(p: &Project, s: &Sched, weekly: bool) -> Workload {
    let l = compute(p, s);
    let b = buckets(s.start.div_euclid(DAY), s.finish.div_euclid(DAY), if weekly { "week" } else { "day" });
    let rows = p.resources.iter().enumerate().filter(|(_, r)| r.kind == ResKind::Work).map(|(k, r)| {
        let (mut load, mut cap) = (vec![0.0; b.len()], vec![0.0; b.len()]);
        for (j, &d0) in b.iter().enumerate() {
            let end = b.get(j + 1).copied().unwrap_or(d0 + if weekly { 7 } else { 1 });
            for d in d0..end {
                load[j] += l.daily.get(&(k, d)).copied().unwrap_or(0.0) / 60.0;
                cap[j] += r.max_on(date_of(d * DAY)) * s.cals[s.rcal[k]].day_minutes(date_of(d * DAY)) as f64 / 60.0;
            }
        }
        WorkRow { uid: r.uid, name: r.name.clone(), load, cap }
    }).collect();
    Workload { buckets: b, rows }
}
