//! What the screen shows: rows with formatted cells plus numbers for drawing the Gantt.
use crate::{cal::WorkCal, model::*, parse, sched};
use chrono::NaiveDate;
use serde::Serialize;
use std::collections::HashMap;

#[derive(Serialize)]
pub struct Row {
    pub uid: u32,
    pub id: usize,
    pub name: String,
    pub lvl: u8,
    pub sum: bool,
    pub ms: bool,
    pub dur: String,
    pub start: String,
    pub finish: String,
    /// Calendar days since 1970-01-01: start, and the day after the finish.
    pub s: i64,
    pub f: i64,
    pub preds: String,
    pub links: Vec<(u32, LinkType)>,
    pub pct: u8,
    pub tf: String,
    pub ff: String,
    pub crit: bool,
    /// Start-no-earlier-than as YYYY-MM-DD, or empty.
    pub snet: String,
    pub notes: String,
}

#[derive(Serialize)]
pub struct View {
    pub name: String,
    pub start: String,
    pub start_iso: String,
    pub finish: String,
    pub s: i64,
    pub f: i64,
    pub week: [bool; 7],
    pub rows: Vec<Row>,
    pub error: Option<String>,
}

pub fn epoch(d: NaiveDate) -> i64 {
    (d - NaiveDate::from_ymd_opt(1970, 1, 1).unwrap()).num_days()
}

pub fn build(p: &Project) -> View {
    let mut cal = WorkCal::new(&p.calendar, p.start);
    let (sc, error) = match sched::schedule(p, &mut cal) {
        Ok(s) => (s, None),
        Err(e) => {
            let mut q = p.clone();
            q.tasks.iter_mut().for_each(|t| t.preds.clear());
            (sched::schedule(&q, &mut cal).expect("a plan without links always schedules"), Some(e))
        }
    };
    let id_of: HashMap<u32, usize> = p.tasks.iter().enumerate().map(|(i, t)| (t.uid, i + 1)).collect();
    let rows = p
        .tasks
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let sum = sc.summary[i];
            let ms = !sum && t.dur == 0;
            let start = cal.date(sc.es[i]);
            let finish = if sc.ef[i] > sc.es[i] { cal.date(sc.ef[i] - 1) } else { start };
            let pct = if sum {
                let kids: Vec<&Task> = p.subtree(i).skip(1).filter(|&j| !sc.summary[j]).map(|j| &p.tasks[j]).collect();
                let w: i64 = kids.iter().map(|k| k.dur as i64).sum();
                if w > 0 {
                    (kids.iter().map(|k| k.pct as i64 * k.dur as i64).sum::<i64>() / w) as u8
                } else {
                    (kids.iter().map(|k| k.pct as i64).sum::<i64>() / kids.len().max(1) as i64) as u8
                }
            } else {
                t.pct
            };
            Row {
                uid: t.uid,
                id: i + 1,
                name: t.name.clone(),
                lvl: t.level,
                sum,
                ms,
                dur: parse::fmt_dur(sc.ef[i] - sc.es[i]),
                start: parse::fmt_date(start),
                finish: parse::fmt_date(finish),
                s: epoch(start),
                f: if ms { epoch(start) } else { epoch(finish) + 1 },
                preds: parse::fmt_preds(&t.preds, &id_of),
                links: t.preds.iter().map(|l| (l.pred, l.kind)).collect(),
                pct,
                tf: parse::fmt_dur(sc.tf[i]),
                ff: parse::fmt_dur(sc.ff[i]),
                crit: sc.tf[i] <= 0,
                snet: t.snet.map(|d| d.to_string()).unwrap_or_default(),
                notes: t.notes.clone(),
            }
        })
        .collect::<Vec<_>>();
    let s = cal.date(0);
    let f = (0..p.tasks.len()).map(|i| {
        if sc.ef[i] > sc.es[i] { cal.date(sc.ef[i] - 1) } else { cal.date(sc.es[i]) }
    }).max().unwrap_or(s);
    View {
        name: p.name.clone(),
        start: parse::fmt_date(s),
        start_iso: p.start.to_string(),
        finish: parse::fmt_date(f),
        s: epoch(s),
        f: epoch(f) + 1,
        week: p.calendar.week,
        rows,
        error,
    }
}
