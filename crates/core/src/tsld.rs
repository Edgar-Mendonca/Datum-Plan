//! The time-scaled logic diagram: every task as a bar at its dates, packed into as few lanes as
//! the dates allow (a task keeps the lane of the predecessor that drives it when that lane is free,
//! so chains read as lines), optionally one band per top-level phase, and the links between them.
use crate::fields::Calc;
use serde::Serialize;

#[derive(Serialize, Debug, Default, Clone, PartialEq)]
pub struct TBar {
    pub uid: u32,
    /// Row number in the plan, and the Activity ID.
    pub id: usize,
    pub code: String,
    pub name: String,
    /// The lane (0 at the top, counted over all bands).
    pub lane: usize,
    /// Start and finish instants (minutes since 1970).
    pub start: i64,
    pub finish: i64,
    pub critical: bool,
    pub milestone: bool,
    pub pct: u8,
}

#[derive(Serialize, Debug, Default, Clone, PartialEq)]
pub struct TLink { pub from: u32, pub to: u32, pub kind: String, pub critical: bool, pub driving: bool }

#[derive(Serialize, Debug, Default, Clone, PartialEq)]
pub struct TBand { pub name: String, pub lane: usize, pub lanes: usize }

#[derive(Serialize, Debug, Default, Clone, PartialEq)]
pub struct Tsld { pub start: i64, pub finish: i64, pub lanes: usize, pub bars: Vec<TBar>, pub links: Vec<TLink>, pub bands: Vec<TBand>, pub status: Option<i64> }

/// The diagram, with a band per top-level phase when `by_phase`. On a lane, the next bar starts no
/// sooner than `gap` minutes after the previous one ends, nor before the previous one's name ends:
/// a name of k characters (with its ID) takes `char_min` minutes of the timescale each (0: names
/// are not measured).
pub fn diagram(c: &Calc, by_phase: bool, gap: i64, char_min: i64) -> Tsld {
    let (p, s) = (c.p, &c.s);
    let n = p.tasks.len();
    let leaf = |i: usize| !s.summary[i] && !s.off[i];
    // The phase of each task: its top-level summary (or "" for top-level tasks).
    let mut phase = vec![usize::MAX; n];
    let mut cur = usize::MAX;
    for i in 0..n {
        if p.tasks[i].level == 0 { cur = if s.summary[i] { i } else { usize::MAX } }
        phase[i] = if p.tasks[i].level == 0 && !s.summary[i] { usize::MAX } else { cur };
    }
    let mut groups: Vec<(String, Vec<usize>)> = vec![];
    for i in (0..n).filter(|&i| leaf(i)) {
        let key = if by_phase && phase[i] != usize::MAX { p.tasks[phase[i]].name.clone() } else { String::new() };
        match groups.iter_mut().find(|g| g.0 == key) { Some(g) => g.1.push(i), None => groups.push((key, vec![i])) }
    }
    // The predecessor that drives each task (the one whose link is driving and finishes last).
    let mut driver: Vec<Option<usize>> = vec![None; n];
    for (k, e) in s.edges.iter().enumerate() {
        if s.drive.get(k).copied().unwrap_or(false) && driver[e.s].is_none_or(|d| s.ef[d] < s.ef[e.p]) { driver[e.s] = Some(e.p) }
    }
    let mut out = Tsld { start: s.start, finish: s.finish, status: p.status.map(crate::cal::at), ..Default::default() };
    let mut lane_of = vec![usize::MAX; n];
    let mut base = 0;
    for (name, mut tasks) in groups {
        tasks.sort_by_key(|&i| (s.es[i], i));
        // Each lane's end (the finish of its last bar plus the gap).
        let mut ends: Vec<i64> = vec![];
        for i in tasks {
            let (a, b) = (s.es[i], s.ef[i].max(s.es[i]));
            let free = |l: usize, ends: &Vec<i64>| ends.get(l).is_some_and(|&e| e <= a);
            let own = driver[i].map(|d| lane_of[d]).filter(|&l| l != usize::MAX && l >= base && free(l - base, &ends)).map(|l| l - base);
            let l = own.or_else(|| (0..ends.len()).find(|&l| free(l, &ends))).unwrap_or_else(|| { ends.push(i64::MIN); ends.len() - 1 });
            let label = (p.tasks[i].code.chars().count() + p.tasks[i].name.chars().count() + 2) as i64 * char_min.max(0);
            ends[l] = b.max(a + label) + gap;
            lane_of[i] = base + l;
        }
        let lanes = ends.len().max(1);
        out.bands.push(TBand { name, lane: base, lanes });
        base += lanes;
    }
    out.lanes = base;
    for i in (0..n).filter(|&i| lane_of[i] != usize::MAX) {
        let t = &p.tasks[i];
        out.bars.push(TBar { uid: t.uid, id: i + 1, code: t.code.clone(), name: t.name.clone(), lane: lane_of[i], start: s.es[i], finish: s.ef[i].max(s.es[i]),
            critical: s.critical(p, i), milestone: s.ef[i] <= s.es[i], pct: c.pct[i].min(100) });
    }
    for (k, e) in s.edges.iter().enumerate().filter(|(_, e)| lane_of[e.p] != usize::MAX && lane_of[e.s] != usize::MAX) {
        out.links.push(TLink { from: p.tasks[e.p].uid, to: p.tasks[e.s].uid, kind: format!("{:?}", e.kind), critical: s.critical(p, e.p) && s.critical(p, e.s), driving: s.drive.get(k).copied().unwrap_or(false) });
    }
    out
}
