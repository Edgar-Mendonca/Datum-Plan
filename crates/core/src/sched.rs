//! Critical path method: forward and backward pass over leaf tasks, summaries roll up.
use crate::cal::WorkCal;
use crate::model::{LinkType, Project};
use std::collections::HashMap;

/// Working-day ordinals per task index; `ef`/`lf` are exclusive ends.
pub struct Sched {
    pub es: Vec<i64>,
    pub ef: Vec<i64>,
    pub ls: Vec<i64>,
    pub lf: Vec<i64>,
    pub tf: Vec<i64>,
    pub ff: Vec<i64>,
    pub summary: Vec<bool>,
    pub finish: i64,
}

struct Edge { p: usize, s: usize, kind: LinkType, lag: i64 }

pub fn schedule(p: &Project, cal: &mut WorkCal) -> Result<Sched, String> {
    let n = p.tasks.len();
    let summary: Vec<bool> = (0..n).map(|i| p.is_summary(i)).collect();
    let ix: HashMap<u32, usize> = p.tasks.iter().enumerate().map(|(i, t)| (t.uid, i)).collect();
    let dur: Vec<i64> = p.tasks.iter().map(|t| t.dur as i64).collect();

    let mut edges = vec![];
    let (mut inc, mut out) = (vec![vec![]; n], vec![vec![]; n]);
    for (s, t) in p.tasks.iter().enumerate().filter(|(s, _)| !summary[*s]) {
        for l in &t.preds {
            if let Some(&pi) = ix.get(&l.pred).filter(|&&pi| pi != s && !summary[pi]) {
                inc[s].push(edges.len());
                out[pi].push(edges.len());
                edges.push(Edge { p: pi, s, kind: l.kind, lag: l.lag as i64 });
            }
        }
    }

    // Topological order (Kahn), stable by row.
    let mut indeg: Vec<usize> = inc.iter().map(Vec::len).collect();
    let mut order: Vec<usize> = (0..n).filter(|&i| !summary[i] && indeg[i] == 0).collect();
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
    let leaves = summary.iter().filter(|s| !**s).count();
    if order.len() < leaves {
        let i = (0..n).find(|&i| !summary[i] && indeg[i] > 0).unwrap();
        return Err(format!("Circular link: task {} \"{}\" ends up depending on itself.", i + 1, p.tasks[i].name));
    }

    let (mut es, mut ef) = (vec![0i64; n], vec![0i64; n]);
    for &i in &order {
        let mut v = p.tasks[i].snet.map(|d| cal.ord(d)).unwrap_or(0).max(0);
        for &e in &inc[i] {
            let Edge { p: q, kind, lag, .. } = edges[e];
            v = v.max(match kind {
                LinkType::FS => ef[q] + lag,
                LinkType::SS => es[q] + lag,
                LinkType::FF => ef[q] + lag - dur[i],
                LinkType::SF => es[q] + lag - dur[i],
            });
        }
        es[i] = v;
        ef[i] = v + dur[i];
    }
    let finish = order.iter().map(|&i| ef[i]).max().unwrap_or(0);

    let (mut ls, mut lf, mut ff) = (vec![0i64; n], vec![finish; n], vec![0i64; n]);
    for &i in order.iter().rev() {
        let mut v = finish;
        let mut free = finish - ef[i];
        for &e in &out[i] {
            let Edge { s, kind, lag, .. } = edges[e];
            v = v.min(match kind {
                LinkType::FS => ls[s] - lag,
                LinkType::SS => ls[s] - lag + dur[i],
                LinkType::FF => lf[s] - lag,
                LinkType::SF => lf[s] - lag + dur[i],
            });
            free = free.min(match kind {
                LinkType::FS => es[s] - ef[i] - lag,
                LinkType::SS => es[s] - es[i] - lag,
                LinkType::FF => ef[s] - ef[i] - lag,
                LinkType::SF => ef[s] - es[i] - lag,
            });
        }
        lf[i] = v;
        ls[i] = v - dur[i];
        ff[i] = free.max(0);
    }
    let mut tf: Vec<i64> = (0..n).map(|i| ls[i] - es[i]).collect();

    // Summaries span their leaves and carry the smallest float below them.
    for i in (0..n).filter(|&i| summary[i]) {
        let kids = || p.subtree(i).skip(1).filter(|&j| !summary[j]);
        es[i] = kids().map(|j| es[j]).min().unwrap_or(0);
        ef[i] = kids().map(|j| ef[j]).max().unwrap_or(0);
        ls[i] = kids().map(|j| ls[j]).min().unwrap_or(0);
        lf[i] = kids().map(|j| lf[j]).max().unwrap_or(0);
        tf[i] = kids().map(|j| tf[j]).min().unwrap_or(0);
        ff[i] = kids().map(|j| ff[j]).min().unwrap_or(0);
    }
    Ok(Sched { es, ef, ls, lf, tf, ff, summary, finish })
}
