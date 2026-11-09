//! Schedule risk analysis (Monte Carlo): each run draws every task's duration from its range and
//! lets each open risk happen with its probability, then reschedules the plan with the full engine.
use crate::{cal::DAY, load, model::*, sched::schedule};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct SimOpts {
    pub iterations: u32,
    /// Range for tasks without their own: −low % to +high % of the duration.
    pub low: f64,
    pub high: f64,
    pub seed: u64,
    /// "tri" (triangular, the planned duration most likely) or "uniform" (any value in the range as likely).
    pub dist: String,
    /// Tasks whose durations move together: "" none, "all", or the name of an activity code (tasks
    /// with the same value of it move together), and how strongly (0–0.99).
    pub corr_by: String,
    pub corr: f64,
    /// Targets for the joint confidence: a finish date (YYYY-MM-DD) and a cost; empty or 0 = the plan's.
    pub target_date: String,
    pub target_cost: f64,
}
impl Default for SimOpts {
    fn default() -> Self { Self { iterations: 1000, low: 10.0, high: 25.0, seed: 1, dist: String::new(), corr_by: String::new(), corr: 0.0, target_date: String::new(), target_cost: 0.0 } }
}

#[derive(Serialize, Default, Debug)]
pub struct TaskRisk { pub uid: u32, pub name: String, pub value: f64 }

#[derive(Serialize, Default, Debug)]
pub struct SimResult {
    pub runs: usize,
    /// The plan's own finish (instant) and the chance of finishing by then.
    pub planned: i64,
    pub planned_chance: f64,
    /// Finish at P10, P50, P80, P90 (instants).
    pub p10: i64,
    pub p50: i64,
    pub p80: i64,
    pub p90: i64,
    /// Finishing day (days since 1970) and how many runs finished that day.
    pub days: Vec<(i64, usize)>,
    pub cost: f64,
    pub cost_p50: f64,
    pub cost_p80: f64,
    /// Share of runs (0–1) in which each task was critical, most critical first.
    pub criticality: Vec<TaskRisk>,
    /// Correlation of each task's duration with the finish, strongest first (the tornado).
    pub drivers: Vec<TaskRisk>,
    /// Risk register entries that were simulated: how often each happened.
    pub risks: Vec<TaskRisk>,
    pub notes: Vec<String>,
    /// Joint confidence: the targets, the chance of meeting each, and of meeting both in the same run.
    pub target_finish: i64,
    pub target_cost: f64,
    pub finish_chance: f64,
    pub cost_chance: f64,
    pub joint: f64,
    /// Finish day and cost of up to 500 runs, for the scatter chart.
    pub scatter: Vec<(i64, f64)>,
}

struct Rng(u64);
impl Rng {
    fn f(&mut self) -> f64 { self.0 ^= self.0 << 13; self.0 ^= self.0 >> 7; self.0 ^= self.0 << 17; (self.0 >> 11) as f64 / (1u64 << 53) as f64 }
}

/// A triangular draw between `a` and `b` with its peak at `m`.
fn tri(r: f64, a: f64, m: f64, b: f64) -> f64 {
    if b <= a { return m }
    let c = (m - a) / (b - a);
    if r < c { a + ((b - a) * (m - a) * r).sqrt() } else { b - ((b - a) * (b - m) * (1.0 - r)).sqrt() }
}

/// Probability of a risk scored 1–5.
pub fn chance(prob: u8) -> f64 { [0.0, 0.1, 0.3, 0.5, 0.7, 0.9][prob.min(5) as usize] }

/// A risk's probability: its own percent when it has one, else its 1–5 score.
fn risk_chance(r: &Risk) -> f64 { if r.pct > 0.0 { (r.pct / 100.0).clamp(0.0, 1.0) } else { chance(r.prob) } }

/// The standard normal distribution function (Abramowitz and Stegun 7.1.26, error under 1.5e-7).
fn phi(x: f64) -> f64 {
    let t = 1.0 / (1.0 + 0.327_591_1 * x.abs() / std::f64::consts::SQRT_2);
    let y = 1.0 - (((((1.061_405_429 * t - 1.453_152_027) * t) + 1.421_413_741) * t - 0.284_496_736) * t + 0.254_829_592) * t * (-(x * x) / 2.0).exp();
    if x >= 0.0 { 0.5 * (1.0 + y) } else { 0.5 * (1.0 - y) }
}

/// A standard normal draw (Box–Muller).
fn normal(rng: &mut Rng) -> f64 { let (u, v) = (rng.f().max(1e-12), rng.f()); (-2.0 * u.ln()).sqrt() * (std::f64::consts::TAU * v).cos() }

fn pct(sorted: &[i64], q: f64) -> i64 { sorted[((sorted.len() - 1) as f64 * q).round() as usize] }

pub fn simulate(p: &Project, o: &SimOpts) -> Result<SimResult, String> {
    let base = schedule(p)?;
    let n = p.tasks.len();
    let cost0 = load::compute(p, &base).cost;
    let leaves: Vec<usize> = (0..n).filter(|&i| !base.summary[i] && !base.off[i] && p.tasks[i].act_finish.is_none()).collect();
    let range = |i: usize| {
        let t = &p.tasks[i];
        let d = t.dur as f64;
        let lo = if t.dmin > 0 { (t.dmin as f64).min(d) } else { d * (1.0 - o.low.clamp(0.0, 100.0) / 100.0) };
        let hi = if t.dmax > 0 { (t.dmax as f64).max(d) } else { d * (1.0 + o.high.max(0.0) / 100.0) };
        (lo, d, hi)
    };
    let risks: Vec<&Risk> = p.risks.iter().filter(|r| r.status != "Closed" && (r.days != 0.0 || r.days_max != 0.0) && !r.tasks.is_empty()).collect();
    // Correlation groups: tasks in one group share part of their draw.
    let rho = o.corr.clamp(0.0, 0.99);
    let mut names: Vec<String> = vec![];
    let group: Vec<Option<usize>> = leaves.iter().map(|&i| {
        if rho == 0.0 || o.corr_by.is_empty() { return None }
        let key = if o.corr_by == "all" { String::new() } else { p.tasks[i].codes.get(&o.corr_by).cloned().filter(|v| !v.is_empty())? };
        Some(names.iter().position(|n| *n == key).unwrap_or_else(|| { names.push(key); names.len() - 1 }))
    }).collect();
    let uniform = o.dist == "uniform";
    let runs = o.iterations.clamp(10, 10_000) as usize;
    let mut q = p.clone();
    let mut rng = Rng(o.seed.max(1).wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
    let (mut fin, mut costs) = (Vec::with_capacity(runs), Vec::with_capacity(runs));
    let mut durs = vec![Vec::with_capacity(runs); leaves.len()];
    let (mut crit, mut hits) = (vec![0usize; n], vec![0usize; risks.len()]);
    let mut failed = 0;
    for _ in 0..runs {
        let (mut add, mut hit) = (vec![0i64; n], vec![]);
        for (k, r) in risks.iter().enumerate() {
            if rng.f() < risk_chance(r) {
                hit.push(k);
                // A risk with a range of days draws its delay from it (most likely its Delay).
                let days = if r.days_max > 0.0 { let lo = r.days_min.min(r.days.abs()); tri(rng.f(), lo, r.days.abs().max(lo), r.days_max.max(r.days.abs())) } else { r.days.abs() };
                let m = (days * p.day_min as f64).round() as i64 * if r.kind == "Opportunity" { -1 } else { 1 };
                // A risk on a phase delays its last task.
                r.tasks.iter().filter_map(|u| p.idx(*u)).map(|i| if base.summary[i] { p.subtree(i).filter(|&j| !base.summary[j]).max_by_key(|&j| base.ef[j]).unwrap_or(i) } else { i }).for_each(|i| add[i] += m);
            }
        }
        let mut cost = 0.0;
        let mut drawn = Vec::with_capacity(leaves.len());
        // One shared draw per correlation group for this run.
        let shared: Vec<f64> = names.iter().map(|_| normal(&mut rng)).collect();
        for (k, &i) in leaves.iter().enumerate() {
            let (a, m, b) = range(i);
            let u = match group[k] { Some(g) => phi(rho.sqrt() * shared[g] + (1.0 - rho).sqrt() * normal(&mut rng)), None => rng.f() };
            let x = if uniform { a + (b - a) * u } else { tri(u, a, m, b) };
            let d = ((x.round() as i64) + add[i]).max(0);
            q.tasks[i].dur = d;
            drawn.push(d as f64);
            let fixed = p.tasks[i].fixed_cost;
            cost += fixed + (cost0[i] - fixed) * if m > 0.0 { d as f64 / m } else { 1.0 };
        }
        let Ok(s) = schedule(&q) else { failed += 1; continue };
        // Only runs kept count, so a risk's rate shares the runs the other results use.
        hit.into_iter().for_each(|k| hits[k] += 1);
        durs.iter_mut().zip(drawn).for_each(|(v, d)| v.push(d));
        fin.push(s.finish);
        costs.push(cost + (0..n).filter(|i| !leaves.contains(i) && !base.summary[*i]).map(|i| cost0[i]).sum::<f64>());
        (0..n).filter(|&i| s.critical(&q, i) && !s.summary[i]).for_each(|i| crit[i] += 1);
    }
    if fin.is_empty() {
        return Err("No run could be scheduled. Check the plan for errors first.".into());
    }
    let mut res = SimResult { runs: fin.len(), planned: base.finish, notes: vec![], ..Default::default() };
    if uniform { res.notes.push("Durations are drawn evenly across their ranges (uniform).".into()) }
    if rho > 0.0 {
        let n_in = group.iter().filter(|g| g.is_some()).count();
        res.notes.push(if o.corr_by == "all" { format!("All tasks move together with correlation {rho:.2}.") } else { format!("{n_in} tasks in {} groups of the code {} move together with correlation {rho:.2}.", names.len(), o.corr_by) });
    }
    // Joint confidence: the chance of meeting the finish and the cost targets in the same run.
    let total: f64 = (0..n).filter(|&i| !base.summary[i] && !base.off[i]).map(|i| cost0[i]).sum();
    res.target_finish = chrono::NaiveDate::parse_from_str(&o.target_date, "%Y-%m-%d").map_or(base.finish, |d| crate::cal::at(d) + DAY);
    res.target_cost = if o.target_cost > 0.0 { o.target_cost } else { total };
    let kept = fin.len() as f64;
    res.finish_chance = fin.iter().filter(|&&f| f <= res.target_finish).count() as f64 / kept;
    res.cost_chance = costs.iter().filter(|&&c| c <= res.target_cost + 1e-6).count() as f64 / kept;
    res.joint = fin.iter().zip(&costs).filter(|(&f, &c)| f <= res.target_finish && c <= res.target_cost + 1e-6).count() as f64 / kept;
    let step = (fin.len() / 500).max(1);
    res.scatter = fin.iter().zip(&costs).step_by(step).map(|(&f, &c)| ((f - 1).div_euclid(DAY), (c * 100.0).round() / 100.0)).collect();
    if failed > 0 { res.notes.push(format!("{failed} runs could not be scheduled and were left out.")) }
    if leaves.iter().all(|&i| p.tasks[i].dur == 0) { res.notes.push("No unfinished tasks with a duration: nothing to vary.".into()) }
    let ranged = leaves.iter().filter(|&&i| p.tasks[i].dmin > 0 || p.tasks[i].dmax > 0).count();
    res.notes.push(format!("{ranged} of {} tasks have their own optimistic and pessimistic durations; the rest use −{}% to +{}%.", leaves.len(), o.low, o.high));
    // Drivers: correlation of each task's duration with the finish.
    let fx: Vec<f64> = fin.iter().map(|&f| f as f64).collect();
    let corr = |v: &[f64]| {
        let (mx, my) = (v.iter().sum::<f64>() / v.len() as f64, fx.iter().sum::<f64>() / fx.len() as f64);
        let (mut sxy, mut sxx, mut syy) = (0.0, 0.0, 0.0);
        for (x, y) in v.iter().zip(&fx) { sxy += (x - mx) * (y - my); sxx += (x - mx).powi(2); syy += (y - my).powi(2) }
        if sxx > 0.0 && syy > 0.0 { sxy / (sxx * syy).sqrt() } else { 0.0 }
    };
    res.drivers = leaves.iter().zip(&durs).map(|(&i, v)| TaskRisk { uid: p.tasks[i].uid, name: p.tasks[i].name.clone(), value: corr(v) }).filter(|t| t.value.abs() > 0.05).collect();
    res.drivers.sort_by(|a, b| b.value.abs().total_cmp(&a.value.abs()));
    res.drivers.truncate(10);
    let runs_f = res.runs as f64;
    res.criticality = (0..n).filter(|&i| crit[i] > 0).map(|i| TaskRisk { uid: p.tasks[i].uid, name: p.tasks[i].name.clone(), value: crit[i] as f64 / runs_f }).collect();
    res.criticality.sort_by(|a, b| b.value.total_cmp(&a.value));
    res.criticality.truncate(15);
    res.risks = risks.iter().zip(&hits).map(|(r, &h)| TaskRisk { uid: r.id, name: r.name.clone(), value: h as f64 / runs_f }).collect();
    res.planned_chance = fin.iter().filter(|&&f| f <= base.finish).count() as f64 / runs_f;
    fin.sort_unstable();
    (res.p10, res.p50, res.p80, res.p90) = (pct(&fin, 0.1), pct(&fin, 0.5), pct(&fin, 0.8), pct(&fin, 0.9));
    for &f in &fin {
        let d = (f - 1).div_euclid(DAY);
        match res.days.last_mut() { Some((x, c)) if *x == d => *c += 1, _ => res.days.push((d, 1)) }
    }
    costs.sort_by(f64::total_cmp);
    res.cost = (0..n).filter(|&i| !base.summary[i] && !base.off[i]).map(|i| cost0[i]).sum();
    let cp = |q: f64| costs[((costs.len() - 1) as f64 * q).round() as usize];
    (res.cost_p50, res.cost_p80) = (cp(0.5), cp(0.8));
    Ok(res)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{cal::at, parse};
    use chrono::NaiveDate;

    fn plan(rows: &[(i64, &str)]) -> Project {
        let mut p = Project::new("R", NaiveDate::from_ymd_opt(2026, 11, 2).unwrap_or_default());
        for (i, &(d, _)) in rows.iter().enumerate() {
            let t = p.new_task(&format!("T{}", i + 1), 0, d * 480);
            p.tasks.push(t);
        }
        let ids: Vec<u32> = p.tasks.iter().map(|t| t.uid).collect();
        for (t, &(_, pr)) in p.tasks.iter_mut().zip(rows) { t.preds = parse::preds(pr, &ids, 480).unwrap_or_default() }
        p
    }
    fn day(y: u32, m: u32, d: u32) -> i64 { at(NaiveDate::from_ymd_opt(y as i32, m, d).unwrap_or_default()) }
    const NO_RANGE: SimOpts = SimOpts { iterations: 200, low: 0.0, high: 0.0, seed: 1, dist: String::new(), corr_by: String::new(), corr: 0.0, target_date: String::new(), target_cost: 0.0 };

    #[test]
    fn without_uncertainty_every_run_is_the_plan() {
        let p = plan(&[(5, ""), (3, "1")]);
        let r = simulate(&p, &NO_RANGE).unwrap_or_default();
        assert_eq!((r.p10, r.p50, r.p90, r.planned_chance), (r.planned, r.planned, r.planned, 1.0));
        assert_eq!(r.planned, day(2026, 11, 11) + 17 * 60);
        assert!(r.drivers.is_empty());
        assert_eq!(r.criticality.iter().map(|t| t.value).collect::<Vec<_>>(), [1.0, 1.0]);
    }

    #[test]
    fn ranges_skew_the_finish_and_the_long_task_drives_it() {
        let p = plan(&[(10, ""), (2, ""), (3, "1")]);
        let r = simulate(&p, &SimOpts::default()).unwrap_or_default();
        assert!(r.p10 <= r.p50 && r.p50 <= r.p80 && r.p80 <= r.p90);
        assert!(r.p50 > r.planned && r.planned_chance < 0.5, "a range skewed late (−10 %, +25 %) makes the plan's own date unlikely: {}", r.planned_chance);
        assert_eq!(r.drivers[0].name, "T1");
        assert!(r.drivers[0].value > 0.8);
        let c = |name: &str| r.criticality.iter().find(|t| t.name == name).map_or(0.0, |t| t.value);
        assert_eq!((c("T1"), c("T2")), (1.0, 0.0));
        assert_eq!(r.days.iter().map(|d| d.1).sum::<usize>(), r.runs);
        // The same seed gives the same answer.
        assert_eq!(simulate(&p, &SimOpts::default()).map(|x| x.p80).unwrap_or_default(), r.p80);
    }

    #[test]
    fn own_ranges_and_register_risks() {
        let mut p = plan(&[(10, "")]);
        (p.tasks[0].dmin, p.tasks[0].dmax) = (8 * 480, 20 * 480);
        let r = simulate(&p, &NO_RANGE).unwrap_or_default();
        assert!(r.p10 >= day(2026, 11, 11) && r.p90 <= day(2026, 11, 27) + DAY, "{} {}", r.p10, r.p90);
        // A risk scored 4 (70 %) adding 5 days: most runs finish a week late.
        let mut p = plan(&[(5, "")]);
        p.risks.push(Risk { id: 1, name: "Late permit".into(), prob: 4, impact: 3, status: "Open".into(), tasks: vec![p.tasks[0].uid], kind: "Threat".into(), days: 5.0, ..Default::default() });
        let r = simulate(&p, &NO_RANGE).unwrap_or_default();
        assert_eq!((r.p10, r.p50), (r.planned, day(2026, 11, 13) + 17 * 60));
        assert!((r.risks[0].value - 0.7).abs() < 0.07, "{}", r.risks[0].value);
        // An opportunity takes days off; a closed risk is left out.
        (p.risks[0].kind, p.risks[0].days) = ("Opportunity".into(), 2.0);
        assert_eq!(simulate(&p, &NO_RANGE).map(|x| x.p50).unwrap_or_default(), day(2026, 11, 4) + 17 * 60);
        p.risks[0].status = "Closed".into();
        assert!(simulate(&p, &NO_RANGE).map(|x| x.risks.is_empty()).unwrap_or(false));
    }

    #[test]
    fn cost_moves_with_duration() {
        let mut p = plan(&[(5, "")]);
        p.resources.push(Resource { uid: 1, name: "Fitter".into(), rate: 50.0, max: 1.0, ..Default::default() });
        p.tasks[0].res = vec![Assign { res: 1, units: 1.0, table: 0 }];
        p.tasks[0].fixed_cost = 1000.0;
        let r = simulate(&p, &SimOpts { iterations: 300, ..SimOpts::default() }).unwrap_or_default();
        assert_eq!(r.cost, 1000.0 + 40.0 * 50.0);
        assert!(r.cost_p50 > r.cost && r.cost_p80 >= r.cost_p50 && r.cost_p80 < 1000.0 + 50.0 * 40.0 * 1.25);
    }

    // 0.8.5: tasks that move together widen the spread of the finish (ten tasks in a row, each
    // −10 % to +25 %): independent, their deviations partly cancel; correlated, they add up.
    #[test]
    fn correlation_widens_the_spread() {
        let rows: Vec<(i64, String)> = (0..10).map(|k| (5, if k == 0 { String::new() } else { k.to_string() })).collect();
        let p = plan(&rows.iter().map(|(d, s)| (*d, s.as_str())).collect::<Vec<_>>());
        let spread = |o: SimOpts| simulate(&p, &o).map(|r| r.p90 - r.p10).unwrap_or_default();
        let base = SimOpts { iterations: 2000, ..SimOpts::default() };
        let (free, tied) = (spread(base.clone()), spread(SimOpts { corr_by: "all".into(), corr: 0.9, ..base.clone() }));
        assert!(tied as f64 > free as f64 * 1.6, "correlated {tied} against independent {free}");
        // By an activity code: only tasks with the same value move together; the note says how many.
        let mut q = p.clone();
        for (k, t) in q.tasks.iter_mut().enumerate() { t.codes.insert("Crew".into(), if k < 5 { "A".into() } else { "B".into() }); }
        let r = simulate(&q, &SimOpts { corr_by: "Crew".into(), corr: 0.9, ..base.clone() }).unwrap_or_default();
        assert!(r.notes.iter().any(|n| n.contains("10 tasks in 2 groups of the code Crew")), "{:?}", r.notes);
        // Uniform draws: an even range (−20 % to +20 %) is centred on the plan.
        let u = simulate(&p, &SimOpts { dist: "uniform".into(), low: 20.0, high: 20.0, ..base }).unwrap_or_default();
        assert!((u.planned_chance - 0.5).abs() < 0.1, "{}", u.planned_chance);
    }

    // A risk's own chance in percent and a range of delay; joint confidence of finish and cost.
    #[test]
    fn risk_percent_range_and_joint_confidence() {
        let mut p = plan(&[(5, "")]);
        p.risks.push(Risk { id: 1, name: "Late permit".into(), prob: 1, status: "Open".into(), tasks: vec![p.tasks[0].uid], kind: "Threat".into(), days: 4.0, pct: 60.0, days_min: 2.0, days_max: 10.0, ..Default::default() });
        let r = simulate(&p, &SimOpts { iterations: 2000, ..NO_RANGE }).unwrap_or_default();
        assert!((r.risks[0].value - 0.6).abs() < 0.05, "its own 60 %, not the score's 10 %: {}", r.risks[0].value);
        assert!(r.p90 > day(2026, 11, 13) + 17 * 60, "the range reaches past the 4 days: {}", r.p90);
        assert!((r.finish_chance - 0.4).abs() < 0.05);
        // Joint: never more than either chance; with no cost the cost target is always met.
        assert!(r.joint <= r.finish_chance.min(r.cost_chance) + 1e-9 && r.cost_chance == 1.0);
        assert!(!r.scatter.is_empty() && r.scatter.len() <= 500);
        // A later target date is met more often.
        let later = simulate(&p, &SimOpts { iterations: 2000, target_date: "2026-11-20".into(), ..NO_RANGE }).unwrap_or_default();
        assert!(later.finish_chance > r.finish_chance && later.target_finish > r.target_finish);
    }
}
