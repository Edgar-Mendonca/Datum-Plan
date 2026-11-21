//! The time-location (linear) view: for work along a route (pipelines, roads, piping runs), each
//! task drawn from its start chainage at its start to its end chainage at its finish, so a crew
//! moving along the line is a sloped line and work at one place is a vertical bar.
use crate::fields::Calc;
use serde::Serialize;

#[derive(Serialize, Debug, Default, Clone, PartialEq)]
pub struct LItem {
    pub uid: u32,
    pub id: usize,
    pub code: String,
    pub name: String,
    /// Chainage at the start and at the finish.
    pub from: f64,
    pub to: f64,
    /// Start and finish instants (minutes since 1970).
    pub start: i64,
    pub finish: i64,
    pub critical: bool,
    pub pct: u8,
}

#[derive(Serialize, Debug, Default, Clone, PartialEq)]
pub struct Linear {
    pub items: Vec<LItem>,
    /// The chainage range shown, and the time range.
    pub lo: f64,
    pub hi: f64,
    pub start: i64,
    pub finish: i64,
    pub status: Option<i64>,
    /// Tasks (not summaries) without a chainage, left out.
    pub missing: usize,
}

/// The tasks with a chainage (Chainage From, Chainage To, or both), as lines in time and place.
pub fn linear(c: &Calc) -> Linear {
    let (p, s) = (c.p, &c.s);
    let mut out = Linear { status: p.status.map(crate::cal::at), ..Default::default() };
    for (i, t) in p.tasks.iter().enumerate().filter(|&(i, _)| !s.summary[i] && !s.off[i]) {
        let (a, b) = match (t.loc_from, t.loc_to) { (Some(a), Some(b)) => (a, b), (Some(a), None) | (None, Some(a)) => (a, a), (None, None) => { out.missing += 1; continue } };
        out.items.push(LItem { uid: t.uid, id: i + 1, code: t.code.clone(), name: t.name.clone(), from: a, to: b, start: s.es[i], finish: s.ef[i].max(s.es[i]), critical: s.critical(p, i), pct: c.pct[i].min(100) });
    }
    if out.items.is_empty() { return out }
    out.lo = out.items.iter().map(|x| x.from.min(x.to)).fold(f64::INFINITY, f64::min);
    out.hi = out.items.iter().map(|x| x.from.max(x.to)).fold(f64::NEG_INFINITY, f64::max);
    out.start = out.items.iter().map(|x| x.start).min().unwrap_or(s.start);
    out.finish = out.items.iter().map(|x| x.finish).max().unwrap_or(s.finish);
    out
}
