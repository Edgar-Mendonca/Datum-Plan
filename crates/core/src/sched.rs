//! Critical path method on working time. Each task runs on its own calendar; lags count on the
//! successor's calendar. Summaries roll up; a link to or from a summary applies to every task in it.
use crate::cal::{at, WorkCal, DAY};
use crate::model::{Constraint as C, LinkType, Project};
use std::collections::HashMap;

/// Instants per task index (minutes); floats in working minutes of the task's calendar.
pub struct Sched {
    pub es: Vec<i64>,
    pub ef: Vec<i64>,
    pub ls: Vec<i64>,
    pub lf: Vec<i64>,
    pub tf: Vec<i64>,
    pub ff: Vec<i64>,
    pub summary: Vec<bool>,
    /// Inactive, or under an inactive summary.
    pub off: Vec<bool>,
    /// Index into `cals` for each task.
    pub ci: Vec<usize>,
    pub cals: Vec<WorkCal>,
    pub start: i64,
    pub finish: i64,
}

struct Edge { p: usize, s: usize, kind: LinkType, lag: i64 }

pub fn schedule(p: &Project) -> Result<Sched, String> {
    let n = p.tasks.len();
    let summary: Vec<bool> = (0..n).map(|i| p.is_summary(i)).collect();
    let mut off = vec![false; n];
    for i in 0..n {
        if p.tasks[i].inactive && !off[i] {
            p.subtree(i).for_each(|j| off[j] = true);
        }
    }
    let names: Vec<&str> = p.calendars.iter().map(|c| c.name.as_str()).collect();
    let cals: Vec<WorkCal> = p.calendars.iter().map(WorkCal::new).collect();
    let def = names.iter().position(|n| *n == p.calendar).unwrap_or(0);
    let ci: Vec<usize> = p.tasks.iter().map(|t| t.cal.as_deref().and_then(|c| names.iter().position(|n| *n == c)).unwrap_or(def)).collect();
    let ix: HashMap<u32, usize> = p.tasks.iter().enumerate().map(|(i, t)| (t.uid, i)).collect();
    let dur: Vec<i64> = p.tasks.iter().map(|t| t.dur).collect();
    let leaf = |i: usize| !summary[i] && !off[i];
    let leaves_of = |i: usize| -> Vec<usize> { if summary[i] { p.subtree(i).filter(|&j| leaf(j)).collect() } else { vec![i] } };

    let mut edges = vec![];
    let (mut inc, mut out) = (vec![vec![]; n], vec![vec![]; n]);
    for (s, t) in p.tasks.iter().enumerate().filter(|&(s, _)| !off[s]) {
        for l in &t.preds {
            let Some(&q) = ix.get(&l.pred) else { continue };
            if off[q] {
                continue;
            }
            if (summary[q] && p.subtree(q).contains(&s)) || (summary[s] && p.subtree(s).contains(&q)) {
                return Err(format!("Task {} is linked to its own summary task. Link tasks inside the summary instead.", s + 1));
            }
            for &a in &leaves_of(q) {
                for &b in &leaves_of(s) {
                    inc[b].push(edges.len());
                    out[a].push(edges.len());
                    edges.push(Edge { p: a, s: b, kind: l.kind, lag: l.lag });
                }
            }
        }
    }

    // Topological order (Kahn), stable by row.
    let mut indeg: Vec<usize> = inc.iter().map(Vec::len).collect();
    let mut order: Vec<usize> = (0..n).filter(|&i| leaf(i) && indeg[i] == 0).collect();
    let mut k = 0;
    while k < order.len() {
        for &e in &out[order[k]] {
            let s = edges[e].s;
            indeg[s] -= 1;
            if indeg[s] == 0 {
                order.push(s);
            }
        }
        k += 1;
    }
    if order.len() < (0..n).filter(|&i| leaf(i)).count() {
        let i = (0..n).find(|&i| leaf(i) && indeg[i] > 0).unwrap();
        return Err(format!("Circular link: task {} \"{}\" ends up depending on itself.", i + 1, p.tasks[i].name));
    }

    let pc = &cals[def];
    let start = pc.next_start(at(p.start));
    let data = p.status.map(|d| at(d) + DAY);
    let (mut es, mut ef) = (vec![start; n], vec![start; n]);
    let mut fixed = vec![false; n];
    for &i in &order {
        let (t, c, d) = (&p.tasks[i], &cals[ci[i]], dur[i]);
        if let Some(a) = t.act_start {
            es[i] = c.next_start(at(a));
            ef[i] = match (t.act_finish, data) {
                (Some(f), _) => {
                    fixed[i] = true;
                    c.end_of(f).max(es[i])
                }
                (None, Some(dd)) if t.pct < 100 => c.add(c.next_start(dd.max(es[i])), d * (100 - t.pct as i64) / 100),
                _ => c.add(es[i], d),
            };
            continue;
        }
        let mut v = match data { Some(dd) if t.pct == 0 => dd.max(start), _ => start };
        if !(t.manual && t.mstart.is_some()) {
            for &e in &inc[i] {
                let Edge { p: q, kind, lag, .. } = edges[e];
                v = v.max(match kind {
                    LinkType::FS => c.add(ef[q], lag),
                    LinkType::SS => c.add(es[q], lag),
                    LinkType::FF => c.sub(c.add(ef[q], lag), d),
                    LinkType::SF => c.sub(c.add(es[q], lag), d),
                });
            }
        }
        if let Some(cd) = t.cdate {
            match t.constraint {
                C::SNET => v = v.max(at(cd)),
                C::FNET => v = v.max(c.sub(c.end_of(cd), d)),
                C::MSO => v = at(cd),
                C::MFO => v = c.sub(c.end_of(cd), d),
                _ => {}
            }
        }
        if let (true, Some(m)) = (t.manual, t.mstart) {
            v = at(m);
        }
        es[i] = if d > 0 || !c.in_or_end(v) { c.next_start(v) } else { v };
        ef[i] = c.add(es[i], d);
    }
    let finish = order.iter().map(|&i| ef[i]).max().unwrap_or(start);

    let (mut ls, mut lf, mut ff) = (es.clone(), ef.clone(), vec![0i64; n]);
    for &i in order.iter().rev() {
        let (t, c, d) = (&p.tasks[i], &cals[ci[i]], dur[i]);
        let mut v = finish;
        let mut free = c.between(ef[i], finish);
        for &e in &out[i] {
            let Edge { s, kind, lag, .. } = edges[e];
            let cs = &cals[ci[s]];
            v = v.min(match kind {
                LinkType::FS => cs.sub(ls[s], lag),
                LinkType::SS => c.add(cs.sub(ls[s], lag), d),
                LinkType::FF => cs.sub(lf[s], lag),
                LinkType::SF => c.add(cs.sub(lf[s], lag), d),
            });
            free = free.min(match kind {
                LinkType::FS => c.between(cs.add(ef[i], lag), es[s]),
                LinkType::SS => c.between(cs.add(es[i], lag), es[s]),
                LinkType::FF => c.between(cs.add(ef[i], lag), ef[s]),
                LinkType::SF => c.between(cs.add(es[i], lag), ef[s]),
            });
        }
        if let Some(cd) = t.cdate {
            match t.constraint {
                C::SNLT => v = v.min(c.add(c.next_start(at(cd)), d)),
                C::FNLT => v = v.min(c.end_of(cd)),
                C::MSO => v = c.add(c.next_start(at(cd)), d),
                C::MFO => v = c.end_of(cd),
                _ => {}
            }
        }
        if let Some(dl) = t.deadline {
            v = v.min(c.end_of(dl));
        }
        if fixed[i] {
            continue;
        }
        lf[i] = if d > 0 { c.prev_end(v) } else { v };
        ls[i] = c.sub(lf[i], d);
        ff[i] = free.max(0);
        if t.constraint == C::ALAP && t.act_start.is_none() {
            (es[i], ef[i]) = (ls[i], lf[i]);
            ff[i] = 0;
        }
    }
    let mut tf: Vec<i64> = (0..n).map(|i| if leaf(i) && !fixed[i] { cals[ci[i]].between(ef[i], lf[i]) } else { 0 }).collect();

    for i in (0..n).filter(|&i| summary[i]) {
        let kids: Vec<usize> = p.subtree(i).filter(|&j| leaf(j)).collect();
        if kids.is_empty() {
            continue;
        }
        es[i] = kids.iter().map(|&j| es[j]).min().unwrap();
        ef[i] = kids.iter().map(|&j| ef[j]).max().unwrap();
        ls[i] = kids.iter().map(|&j| ls[j]).min().unwrap();
        lf[i] = kids.iter().map(|&j| lf[j]).max().unwrap();
        tf[i] = kids.iter().map(|&j| tf[j]).min().unwrap();
        ff[i] = kids.iter().map(|&j| ff[j]).min().unwrap();
    }
    Ok(Sched { es, ef, ls, lf, tf, ff, summary, off, ci, cals, start, finish })
}
