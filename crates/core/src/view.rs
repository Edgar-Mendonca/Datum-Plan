//! What the screen shows: formatted rows for every field, numbers for the Gantt, the resource sheet.
use crate::{cal::{at, date_of, DAY}, load, model::*, parse, sched};
use chrono::NaiveDate;
use serde::Serialize;
use std::collections::HashMap;

#[derive(Serialize)]
pub struct Row {
    pub uid: u32,
    pub id: usize,
    pub wbs: String,
    pub code: String,
    pub name: String,
    pub lvl: u8,
    pub sum: bool,
    pub ms: bool,
    pub crit: bool,
    pub late: bool,
    pub manual: bool,
    pub off: bool,
    pub tl: bool,
    pub dur: String,
    pub start: String,
    pub finish: String,
    pub preds: String,
    pub succs: String,
    pub res: String,
    pub pct: u8,
    pub tf: String,
    pub ff: String,
    pub work: String,
    pub cost: String,
    pub ctype: String,
    pub cdate: String,
    pub deadline: String,
    pub cal: String,
    pub mode: String,
    pub active: String,
    pub astart: String,
    pub afinish: String,
    pub rem: String,
    pub bstart: String,
    pub bfinish: String,
    pub bdur: String,
    pub vstart: String,
    pub vfinish: String,
    pub notes: String,
    /// Gantt numbers in days since 1970-01-01 (fractions are times of day).
    pub s: f64,
    pub f: f64,
    pub bs: Option<f64>,
    pub bf: Option<f64>,
    pub dl: Option<f64>,
    pub links: Vec<(u32, LinkType)>,
}

#[derive(Serialize)]
pub struct ResRow {
    pub uid: u32,
    pub id: usize,
    pub name: String,
    pub kind: String,
    pub group: String,
    pub max: String,
    pub rate: String,
    pub per_use: String,
    pub label: String,
    pub work: String,
    pub cost: String,
    pub over: bool,
}

#[derive(Serialize)]
pub struct View {
    pub name: String,
    pub start: String,
    pub start_iso: String,
    pub finish: String,
    pub status: String,
    pub status_iso: String,
    pub status_day: Option<f64>,
    pub s: f64,
    pub f: f64,
    pub calendar: String,
    pub day_hours: f64,
    pub manual_default: bool,
    /// Working weekdays of the project calendar (Monday first) and its non-working exception days.
    pub week: [bool; 7],
    pub off_days: Vec<i64>,
    pub columns: Vec<(String, u16)>,
    pub calendars: Vec<Calendar>,
    pub resources: Vec<ResRow>,
    pub work: String,
    pub cost: String,
    pub bl_finish: String,
    pub var_finish: String,
    pub rows: Vec<Row>,
    pub error: Option<String>,
}

pub fn money(v: f64) -> String {
    let s = format!("{:.2}", v.abs());
    let (int, dec) = s.split_at(s.len() - 3);
    let mut out = String::new();
    for (k, ch) in int.chars().enumerate() {
        if k > 0 && (int.len() - k) % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    format!("{}{out}{dec}", if v < 0.0 { "-" } else { "" })
}

fn hours(min: f64) -> String {
    let h = min / 60.0;
    if h == 0.0 { "0 hrs".into() } else { format!("{} hrs", format!("{h:.1}").trim_end_matches(".0")) }
}

fn days(min: i64, day: i64) -> String {
    parse::fmt_dur(min, Unit::D, day)
}

pub fn build(p: &Project, fmt: &str, today: NaiveDate) -> View {
    let (s, error) = match sched::schedule(p) {
        Ok(s) => (s, None),
        Err(e) => {
            let mut q = p.clone();
            q.tasks.iter_mut().for_each(|t| t.preds.clear());
            (sched::schedule(&q).expect("a plan without links always schedules"), Some(e))
        }
    };
    let l = load::compute(p, &s);
    let df = |d: NaiveDate| d.format(fmt).to_string();
    let od = |d: Option<NaiveDate>| d.map(df).unwrap_or_default();
    let day = p.day_min;
    let id_of: HashMap<u32, usize> = p.tasks.iter().enumerate().map(|(i, t)| (t.uid, i + 1)).collect();
    let mut succs: Vec<Vec<String>> = vec![vec![]; p.tasks.len()];
    for (i, t) in p.tasks.iter().enumerate() {
        for lk in &t.preds {
            if let Some(&k) = id_of.get(&lk.pred) {
                succs[k - 1].push(parse::fmt_link(i + 1, lk, day));
            }
        }
    }
    // Unfinished work due by the status date (or before today) is late.
    let due = p.status.unwrap_or(today.pred_opt().unwrap());
    let mut wbs: Vec<u32> = vec![];
    let pc = p.calendars.iter().position(|c| c.name == p.calendar).unwrap_or(0);
    let rows = p.tasks.iter().enumerate().map(|(i, t)| {
        let lv = t.level as usize;
        wbs.truncate(lv + 1);
        wbs.resize(lv + 1, 0);
        wbs[lv] += 1;
        let (sum, c) = (s.summary[i], &s.cals[s.ci[i]]);
        let ms = !sum && t.dur == 0;
        let (es, ef) = (s.es[i], s.ef[i]);
        let fin = if ef > es { date_of(ef - 1) } else { date_of(ef) };
        let pct = if sum {
            let kids: Vec<&Task> = p.subtree(i).skip(1).filter(|&j| !s.summary[j] && !s.off[j]).map(|j| &p.tasks[j]).collect();
            let w: i64 = kids.iter().map(|k| k.dur).sum();
            if w > 0 { (kids.iter().map(|k| k.pct as i64 * k.dur).sum::<i64>() / w) as u8 } else { (kids.iter().map(|k| k.pct as i64).sum::<i64>() / kids.len().max(1) as i64) as u8 }
        } else { t.pct };
        let dur = if sum { c.between(es, ef) } else { t.dur };
        let unit = if sum { Unit::D } else { t.unit };
        let bl = t.bl;
        let var = |a: i64, b: i64| days(if b >= a { c.between(a, b) } else { -c.between(b, a) }, day);
        Row {
            uid: t.uid, id: i + 1,
            wbs: wbs.iter().map(u32::to_string).collect::<Vec<_>>().join("."),
            code: t.code.clone(), name: t.name.clone(), lvl: t.level, sum, ms,
            crit: !s.off[i] && s.tf[i] <= 0 && pct < 100,
            late: !sum && !s.off[i] && pct < 100 && fin <= due,
            manual: t.manual, off: s.off[i], tl: t.timeline,
            dur: parse::fmt_dur(dur, unit, day), start: df(date_of(es)), finish: df(fin),
            preds: parse::fmt_preds(&t.preds, &id_of, day), succs: succs[i].join(", "), res: parse::fmt_res(&t.res, p), pct,
            tf: days(s.tf[i], day), ff: days(s.ff[i], day),
            work: hours(l.work[i]), cost: money(l.cost[i]),
            ctype: CONSTRAINTS.iter().find(|(k, _)| *k == t.constraint).map(|(_, n)| n.to_string()).unwrap_or_default(),
            cdate: od(t.cdate), deadline: od(t.deadline), cal: t.cal.clone().unwrap_or_default(),
            mode: if t.manual { "Manual".into() } else { "Auto".into() }, active: if t.inactive { "No".into() } else { "Yes".into() },
            astart: od(t.act_start), afinish: od(t.act_finish),
            rem: parse::fmt_dur(dur * (100 - pct as i64) / 100, unit, day),
            bstart: bl.map(|b| df(date_of(b.start))).unwrap_or_default(),
            bfinish: bl.map(|b| df(if b.finish > b.start { date_of(b.finish - 1) } else { date_of(b.finish) })).unwrap_or_default(),
            bdur: bl.map(|b| parse::fmt_dur(b.dur, unit, day)).unwrap_or_default(),
            vstart: bl.map(|b| var(b.start, es)).unwrap_or_default(),
            vfinish: bl.map(|b| var(b.finish, ef)).unwrap_or_default(),
            notes: t.notes.clone(),
            s: es as f64 / DAY as f64, f: ef as f64 / DAY as f64,
            bs: bl.map(|b| b.start as f64 / DAY as f64), bf: bl.map(|b| b.finish as f64 / DAY as f64),
            dl: t.deadline.map(|d| (at(d) + DAY) as f64 / DAY as f64),
            links: t.preds.iter().map(|l| (l.pred, l.kind)).collect(),
        }
    }).collect::<Vec<Row>>();
    let start = date_of(s.start);
    let fin = if s.finish > s.start { date_of(s.finish - 1) } else { start };
    let cal = &p.calendars[pc];
    let mut off_days: Vec<i64> = cal.exceptions.iter().filter(|e| e.hours.is_empty()).flat_map(|e| {
        let (a, b) = (at(e.from) / DAY, at(e.to) / DAY);
        a..=b.min(a + 3700)
    }).collect();
    off_days.sort_unstable();
    let top: Vec<usize> = (0..p.tasks.len()).filter(|&i| p.tasks[i].level == 0 && !s.off[i]).collect();
    let bl_end = p.tasks.iter().filter_map(|t| t.bl.map(|b| b.finish)).max();
    View {
        name: p.name.clone(), start: df(start), start_iso: p.start.to_string(), finish: df(fin),
        status: od(p.status), status_iso: p.status.map(|d| d.to_string()).unwrap_or_default(),
        status_day: p.status.map(|d| (at(d) + DAY) as f64 / DAY as f64),
        s: s.start as f64 / DAY as f64, f: s.finish as f64 / DAY as f64,
        calendar: p.calendar.clone(), day_hours: day as f64 / 60.0, manual_default: p.manual_default,
        week: std::array::from_fn(|k| !cal.week[k].is_empty()), off_days,
        columns: p.columns.clone(), calendars: p.calendars.clone(),
        resources: p.resources.iter().enumerate().map(|(k, r)| ResRow {
            uid: r.uid, id: k + 1, name: r.name.clone(), kind: format!("{:?}", r.kind), group: r.group.clone(),
            max: if r.kind == ResKind::Work { format!("{}%", (r.max * 100.0).round()) } else { String::new() },
            rate: money(r.rate), per_use: money(r.per_use), label: r.label.clone(),
            work: hours(l.res_work[k]), cost: money(l.res_cost[k]), over: l.over[k],
        }).collect(),
        work: hours(top.iter().map(|&i| l.work[i]).sum()), cost: money(top.iter().map(|&i| l.cost[i]).sum()),
        bl_finish: bl_end.map(|b| df(date_of(b - 1))).unwrap_or_default(),
        var_finish: bl_end.map(|b| days(if s.finish >= b { s.cals[pc].between(b, s.finish) } else { -s.cals[pc].between(s.finish, b) }, day)).unwrap_or_default(),
        rows, error,
    }
}

#[derive(Serialize)]
pub struct Rel { pub uid: u32, pub id: usize, pub name: String, pub kind: LinkType, pub lag: String }

#[derive(Serialize)]
pub struct AssignRow { pub res: u32, pub name: String, pub kind: String, pub units: String, pub work: String, pub cost: String }

#[derive(Serialize)]
pub struct Detail { pub preds: Vec<Rel>, pub succs: Vec<Rel>, pub assigns: Vec<AssignRow> }

/// Relationships and assignments of one task, for the details pane.
pub fn detail(p: &Project, uid: u32) -> Option<Detail> {
    let i = p.idx(uid)?;
    let t = &p.tasks[i];
    let rel = |k: usize, l: &Link| Rel { uid: p.tasks[k].uid, id: k + 1, name: p.tasks[k].name.clone(), kind: l.kind, lag: parse::fmt_lag(l.lag, p.day_min) };
    let preds = t.preds.iter().filter_map(|l| p.idx(l.pred).map(|k| rel(k, l))).collect();
    let succs = p.tasks.iter().enumerate().flat_map(|(k, s)| s.preds.iter().filter(|l| l.pred == uid).map(move |l| (k, l))).map(|(k, l)| rel(k, l)).collect();
    let assigns = t.res.iter().filter_map(|a| {
        let r = &p.resources[p.res_idx(a.res)?];
        let (w, c) = match r.kind {
            ResKind::Work => (t.dur as f64 * a.units, t.dur as f64 * a.units / 60.0 * r.rate + r.per_use),
            ResKind::Material => (0.0, a.units * r.rate + r.per_use),
            ResKind::Cost => (0.0, a.units + r.per_use),
        };
        let units = if r.kind == ResKind::Work { format!("{}%", (a.units * 100.0).round()) } else { format!("{}", a.units) };
        Some(AssignRow { res: r.uid, name: r.name.clone(), kind: format!("{:?}", r.kind), units, work: hours(w), cost: money(c) })
    }).collect();
    Some(Detail { preds, succs, assigns })
}
