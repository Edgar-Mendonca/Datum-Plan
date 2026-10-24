//! Network Diagram layout (MS Project style): one box per task, left to right by logic,
//! each task one column after its latest predecessor, rows ordered to keep links short.
use crate::fields::Calc;
use serde::Serialize;

#[derive(Serialize)]
pub struct Node { pub uid: u32, pub col: usize, pub row: usize }

#[derive(Serialize)]
pub struct NetLink { pub from: u32, pub to: u32, pub kind: crate::model::LinkType, pub crit: bool }

#[derive(Serialize)]
pub struct Net { pub nodes: Vec<Node>, pub links: Vec<NetLink>, pub cols: usize, pub rows: usize }

/// Lays out the given rows (task indexes, already filtered) as a network.
pub fn layout(c: &Calc, rows: &[usize]) -> Net {
    let (p, s) = (c.p, &c.s);
    let n = p.tasks.len();
    let mut shown = vec![false; n];
    rows.iter().for_each(|&i| shown[i] = true);
    let edges: Vec<&crate::sched::Edge> = s.edges.iter().filter(|e| shown[e.p] && shown[e.s]).collect();
    let mut preds: Vec<Vec<usize>> = vec![vec![]; n];
    edges.iter().filter(|e| e.p != e.s).for_each(|e| preds[e.s].push(e.p));
    // Columns: longest chain of predecessors, in schedule order so predecessors come first.
    let mut order: Vec<usize> = rows.to_vec();
    order.sort_by_key(|&i| (s.es[i], s.ef[i], i));
    let mut col = vec![0usize; n];
    for _ in 0..2 {
        for &i in &order {
            col[i] = preds[i].iter().map(|&q| col[q] + 1).max().unwrap_or(0);
        }
    }
    let cols = rows.iter().map(|&i| col[i] + 1).max().unwrap_or(0);
    // Rows: each column in order of its predecessors' average row, then start.
    let mut row = vec![0usize; n];
    let mut height = 0;
    let mut by_col: Vec<Vec<usize>> = vec![vec![]; cols];
    order.iter().for_each(|&i| by_col[col[i]].push(i));
    for here in by_col {
        let mut here: Vec<(f64, i64, usize)> = here.into_iter().map(|i| {
            let pr: Vec<f64> = preds[i].iter().map(|&q| row[q] as f64).collect();
            (if pr.is_empty() { f64::MAX } else { pr.iter().sum::<f64>() / pr.len() as f64 }, s.es[i], i)
        }).collect();
        here.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
        let mut used: Vec<bool> = vec![];
        for (want, _, i) in here {
            let mut r = if want == f64::MAX { 0 } else { want.round() as usize };
            while used.get(r).copied().unwrap_or(false) {
                r += 1;
            }
            if used.len() <= r {
                used.resize(r + 1, false);
            }
            used[r] = true;
            row[i] = r;
            height = height.max(r + 1);
        }
    }
    Net {
        nodes: rows.iter().map(|&i| Node { uid: p.tasks[i].uid, col: col[i], row: row[i] }).collect(),
        links: edges.iter().map(|e| NetLink { from: p.tasks[e.p].uid, to: p.tasks[e.s].uid, kind: e.kind, crit: s.critical(p, e.p) && s.critical(p, e.s) }).collect(),
        cols, rows: height,
    }
}
