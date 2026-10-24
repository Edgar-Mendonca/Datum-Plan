//! An open plan: edit operations, undo/redo and .dplan files.
use crate::{cal::{at, date_of, DAY}, load, model::*, parse, sched};
use chrono::NaiveDate;
use serde::Deserialize;
use std::{collections::{HashMap, HashSet}, path::{Path, PathBuf}};

#[derive(Debug, Clone, Deserialize)]
pub enum Op {
    Insert { at: usize, name: String, ms: bool },
    /// New tasks from (field, value) pairs, such as pasted spreadsheet rows.
    InsertRows { at: usize, rows: Vec<Vec<(String, String)>> },
    QuickAdd { at: usize, text: String },
    Paste { at: usize, tasks: Vec<Task> },
    Delete { uids: Vec<u32> },
    Set { uid: u32, field: String, value: String },
    Indent { uids: Vec<u32> },
    Outdent { uids: Vec<u32> },
    Move { uids: Vec<u32>, up: bool },
    Link { uids: Vec<u32> },
    Unlink { uids: Vec<u32> },
    MarkOnTrack { uids: Vec<u32> },
    /// Saves (or clears) the baseline of the given tasks, or of all tasks when empty.
    Baseline { uids: Vec<u32>, clear: bool },
    Project { name: String, start: String, status: String, calendar: String, day_hours: f64, manual_default: bool },
    CalPut { old: Option<String>, cal: Calendar },
    CalDelete { name: String },
    ResInsert { name: String },
    ResSet { uid: u32, field: String, value: String },
    ResDelete { uids: Vec<u32> },
    Columns { cols: Vec<(String, u16)> },
    Batch(Vec<Op>),
}

pub struct Doc {
    pub p: Project,
    pub path: Option<PathBuf>,
    pub dirty: bool,
    undo: Vec<Project>,
    redo: Vec<Project>,
}

const UNDO_LIMIT: usize = 200;

impl Doc {
    pub fn new(p: Project) -> Self {
        Self { p, path: None, dirty: false, undo: vec![], redo: vec![] }
    }

    pub fn can_undo(&self) -> bool { !self.undo.is_empty() }
    pub fn can_redo(&self) -> bool { !self.redo.is_empty() }

    /// Applies `op` to a copy and keeps it only if the plan still schedules.
    /// `fmt` is the date format the user types in.
    pub fn apply(&mut self, op: Op, fmt: &str) -> Result<(), String> {
        let mut q = self.p.clone();
        exec(&mut q, op, fmt)?;
        q.normalize();
        sched::schedule(&q)?;
        if q == self.p {
            return Ok(());
        }
        self.undo.push(std::mem::replace(&mut self.p, q));
        if self.undo.len() > UNDO_LIMIT {
            self.undo.remove(0);
        }
        self.redo.clear();
        self.dirty = true;
        Ok(())
    }

    pub fn undo(&mut self) {
        if let Some(p) = self.undo.pop() {
            self.redo.push(std::mem::replace(&mut self.p, p));
            self.dirty = true;
        }
    }

    pub fn redo(&mut self) {
        if let Some(p) = self.redo.pop() {
            self.undo.push(std::mem::replace(&mut self.p, p));
            self.dirty = true;
        }
    }

    pub fn open(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("Could not open {}: {e}", path.display()))?;
        let p = Project::from_json(&text).map_err(|e| format!("This is not a Datum Plan file ({e})."))?;
        let mut d = Self::new(p);
        d.path = Some(path.into());
        Ok(d)
    }

    pub fn save(&mut self, path: &Path) -> Result<(), String> {
        std::fs::write(path, self.json()).map_err(|e| format!("Could not save {}: {e}", path.display()))?;
        self.path = Some(path.into());
        self.dirty = false;
        Ok(())
    }

    pub fn json(&self) -> String {
        serde_json::to_string_pretty(&self.p).unwrap_or_default()
    }
}

/// Tasks (with their subtasks) to put on the clipboard; levels start at 0.
pub fn copy(p: &Project, uids: &[u32]) -> Vec<Task> {
    let mut keep = vec![false; p.tasks.len()];
    for i in idxs(p, uids) {
        p.subtree(i).for_each(|j| keep[j] = true);
    }
    let mut v: Vec<Task> = p.tasks.iter().zip(&keep).filter(|(_, k)| **k).map(|(t, _)| t.clone()).collect();
    let min = v.iter().map(|t| t.level).min().unwrap_or(0);
    v.iter_mut().for_each(|t| t.level -= min);
    v
}

fn idxs(p: &Project, uids: &[u32]) -> Vec<usize> {
    let mut v: Vec<usize> = uids.iter().filter_map(|&u| p.idx(u)).collect();
    v.sort_unstable();
    v.dedup();
    v
}

fn shift(p: &mut Project, uids: &[u32], by: i8) {
    let mut done = HashSet::new();
    for i in idxs(p, uids) {
        let t = &p.tasks[i];
        let ok = if by > 0 { i > 0 && t.level <= p.tasks[i - 1].level } else { t.level > 0 };
        if done.contains(&i) || !ok {
            continue;
        }
        for j in p.subtree(i) {
            done.insert(j);
            p.tasks[j].level = (p.tasks[j].level as i8 + by) as u8;
        }
    }
}

fn move_rows(p: &mut Project, uids: &[u32], up: bool) {
    let mut v = idxs(p, uids);
    if !up {
        v.reverse();
    }
    for uid in v.iter().map(|&i| p.tasks[i].uid).collect::<Vec<_>>() {
        let i = p.idx(uid).unwrap();
        let (r, lvl) = (p.subtree(i), p.tasks[i].level);
        if up {
            let mut k = i;
            while k > 0 && p.tasks[k - 1].level > lvl {
                k -= 1;
            }
            if k > 0 && p.tasks[k - 1].level == lvl {
                let k = p.subtree(k - 1).start;
                p.tasks[k..r.end].rotate_right(r.len());
            }
        } else if r.end < p.tasks.len() && p.tasks[r.end].level == lvl {
            let s = p.subtree(r.end);
            p.tasks[i..s.end].rotate_left(r.len());
        }
    }
}

fn level_at(p: &Project, at: usize) -> u8 {
    p.tasks.get(at).or_else(|| p.tasks.last()).map_or(0, |t| t.level)
}

fn yes(v: &str) -> bool {
    matches!(v.trim().to_lowercase().as_str(), "yes" | "y" | "true" | "1" | "on")
}

fn set_pct(p: &mut Project, i: usize, n: u8) -> Result<(), String> {
    let needs = (n > 0 && p.tasks[i].act_start.is_none()) || (n == 100 && p.tasks[i].act_finish.is_none());
    let (es, ef) = if needs { let s = sched::schedule(p)?; (s.es[i], s.ef[i]) } else { (0, 0) };
    let t = &mut p.tasks[i];
    t.pct = n.min(100);
    if n == 0 {
        (t.act_start, t.act_finish) = (None, None);
    } else {
        t.act_start.get_or_insert(date_of(es));
        t.act_finish = if n == 100 { Some(t.act_finish.unwrap_or(date_of((ef - 1).max(es)))) } else { None };
    }
    Ok(())
}

fn set(p: &mut Project, i: usize, field: &str, value: &str, fmt: &str) -> Result<(), String> {
    let v = value.trim();
    if p.is_summary(i) && !matches!(field, "name" | "notes" | "code" | "preds" | "active" | "timeline" | "deadline") {
        return Err(format!("A summary task's {field} comes from the tasks under it."));
    }
    let date = |s: &str| -> Result<Option<NaiveDate>, String> { if s.is_empty() { Ok(None) } else { parse::date_in(s, fmt).map(Some) } };
    let day = p.day_min;
    let uid = p.tasks[i].uid;
    match field {
        "name" => p.tasks[i].name = v.into(),
        "notes" => p.tasks[i].notes = value.into(),
        "code" => p.tasks[i].code = v.into(),
        "dur" => (p.tasks[i].dur, p.tasks[i].unit) = parse::dur(v, day)?,
        "pct" => set_pct(p, i, v.trim_end_matches('%').trim().parse().map_err(|_| "Enter a number from 0 to 100.")?)?,
        "rem" => {
            let (m, _) = parse::dur(v, day)?;
            let d = p.tasks[i].dur.max(1);
            set_pct(p, i, (100 - (m.min(d) * 100 / d)) as u8)?;
        }
        "start" => {
            let d = date(v)?;
            let t = &mut p.tasks[i];
            if t.manual {
                t.mstart = d;
            } else if d.is_some() {
                (t.constraint, t.cdate) = (Constraint::SNET, d);
            } else if t.constraint == Constraint::SNET {
                (t.constraint, t.cdate) = (Constraint::ASAP, None);
            }
        }
        "finish" => {
            let d = date(v)?.ok_or("Type a finish date.")?;
            let s = sched::schedule(p)?;
            let c = &s.cals[s.ci[i]];
            p.tasks[i].dur = c.between(s.es[i], c.end_of(d)).max(0);
        }
        "preds" => {
            let ids: Vec<u32> = p.tasks.iter().map(|t| t.uid).collect();
            let links = parse::preds(v, &ids, day)?;
            if links.iter().any(|l| l.pred == uid) {
                return Err("A task cannot be its own predecessor.".into());
            }
            p.tasks[i].preds = links;
        }
        "res" => p.tasks[i].res = parse::res(v, p)?,
        "ctype" => {
            let c = parse::constraint(v)?;
            if matches!(c, Constraint::ASAP | Constraint::ALAP) {
                p.tasks[i].cdate = None;
            } else if p.tasks[i].cdate.is_none() {
                let s = sched::schedule(p)?;
                let ends = matches!(c, Constraint::FNET | Constraint::FNLT | Constraint::MFO);
                p.tasks[i].cdate = Some(date_of(if ends { (s.ef[i] - 1).max(s.es[i]) } else { s.es[i] }));
            }
            p.tasks[i].constraint = c;
        }
        "cdate" => {
            let t = &mut p.tasks[i];
            t.cdate = date(v)?;
            if t.cdate.is_some() && matches!(t.constraint, Constraint::ASAP | Constraint::ALAP) {
                t.constraint = Constraint::SNET;
            } else if t.cdate.is_none() {
                t.constraint = Constraint::ASAP;
            }
        }
        "deadline" => p.tasks[i].deadline = date(v)?,
        "cal" => {
            if !v.is_empty() && !p.calendars.iter().any(|c| c.name == v) {
                return Err(format!("There is no calendar called \"{v}\". Add it in Project › Change Working Time."));
            }
            p.tasks[i].cal = Some(v.to_string()).filter(|c| !c.is_empty());
        }
        "mode" => {
            let manual = v.to_lowercase().starts_with('m');
            if manual && p.tasks[i].mstart.is_none() {
                let s = sched::schedule(p)?;
                p.tasks[i].mstart = Some(date_of(s.es[i]));
            }
            p.tasks[i].manual = manual;
        }
        "active" => p.tasks[i].inactive = !yes(v),
        "timeline" => p.tasks[i].timeline = yes(v),
        "astart" => {
            let d = date(v)?;
            let t = &mut p.tasks[i];
            t.act_start = d;
            if d.is_none() {
                (t.act_finish, t.pct) = (None, 0);
            }
        }
        "afinish" => {
            let d = date(v)?;
            if d.is_some() {
                if p.tasks[i].act_start.is_none() {
                    set_pct(p, i, 1)?;
                }
                p.tasks[i].pct = 100;
            } else if p.tasks[i].pct == 100 {
                p.tasks[i].pct = 99;
            }
            p.tasks[i].act_finish = d;
        }
        f => return Err(format!("{f} is calculated and cannot be edited.")),
    }
    Ok(())
}

fn insert_rows(p: &mut Project, at: usize, rows: Vec<Vec<(String, String)>>, fmt: &str) -> Result<(), String> {
    let at = at.min(p.tasks.len());
    let level = level_at(p, at);
    let mut later = vec![];
    for (k, row) in rows.into_iter().enumerate() {
        let t = p.new_task("", level, p.day_min);
        p.tasks.insert(at + k, t);
        for (f, v) in row {
            if f == "preds" { later.push((at + k, v)) } else if !v.trim().is_empty() || f == "name" { set(p, at + k, &f, &v, fmt)? }
        }
    }
    later.into_iter().try_for_each(|(i, v)| set(p, i, "preds", &v, fmt))
}

fn res_set(p: &mut Project, uid: u32, field: &str, v: &str) -> Result<(), String> {
    let i = p.res_idx(uid).ok_or("That resource no longer exists.")?;
    let v = v.trim();
    let num = |s: &str| -> Result<f64, String> { s.trim_end_matches('%').replace(',', "").trim().parse::<f64>().map_err(|_| format!("\"{s}\" is not a number.")) };
    let r = &mut p.resources[i];
    match field {
        "name" if v.is_empty() => return Err("A resource needs a name.".into()),
        "name" => r.name = v.into(),
        "kind" => r.kind = match v.to_lowercase().chars().next() { Some('m') => ResKind::Material, Some('c') => ResKind::Cost, _ => ResKind::Work },
        "group" => r.group = v.into(),
        "max" => r.max = num(v)? / if v.ends_with('%') || num(v)? > 10.0 { 100.0 } else { 1.0 },
        "rate" => r.rate = num(v)?,
        "per_use" => r.per_use = num(v)?,
        "label" => r.label = v.into(),
        f => return Err(format!("{f} cannot be edited.")),
    }
    let name = p.resources[i].name.clone();
    if p.resources.iter().filter(|r| r.name.eq_ignore_ascii_case(&name)).count() > 1 {
        return Err(format!("There is already a resource called \"{name}\"."));
    }
    Ok(())
}

fn exec(p: &mut Project, op: Op, fmt: &str) -> Result<(), String> {
    match op {
        Op::Insert { at, name, ms } => {
            let at = at.min(p.tasks.len());
            let t = p.new_task(name.trim(), level_at(p, at), if ms { 0 } else { p.day_min });
            p.tasks.insert(at, t);
        }
        Op::InsertRows { at, rows } => insert_rows(p, at, rows, fmt)?,
        Op::QuickAdd { at, text } => {
            let row = parse::quick(&text);
            if row[0].1.is_empty() {
                return Err("Start with the task name, like: Pour slab 5d after 4 @Crew.".into());
            }
            insert_rows(p, at, vec![row], fmt)?;
        }
        Op::Paste { at, mut tasks } => {
            let at = at.min(p.tasks.len());
            let base = level_at(p, at);
            let map: HashMap<u32, u32> = tasks.iter().enumerate().map(|(k, t)| (t.uid, p.next_uid + k as u32)).collect();
            p.next_uid += tasks.len() as u32;
            for t in &mut tasks {
                t.uid = map[&t.uid];
                t.level += base;
                t.preds.retain_mut(|l| map.get(&l.pred).map(|&u| l.pred = u).is_some());
            }
            p.tasks.splice(at..at, tasks);
        }
        Op::Delete { uids } => {
            let gone: HashSet<usize> = idxs(p, &uids).into_iter().flat_map(|i| p.subtree(i)).collect();
            let mut i = 0;
            p.tasks.retain(|_| (!gone.contains(&i), i += 1).0);
        }
        Op::Set { uid, field, value } => {
            let i = p.idx(uid).ok_or("That task no longer exists.")?;
            set(p, i, &field, &value, fmt)?;
        }
        Op::Indent { uids } => shift(p, &uids, 1),
        Op::Outdent { uids } => shift(p, &uids, -1),
        Op::Move { uids, up } => move_rows(p, &uids, up),
        Op::Link { uids } => {
            let v = idxs(p, &uids);
            if v.len() < 2 {
                return Err("Select two or more tasks to link.".into());
            }
            for w in v.windows(2) {
                let pred = p.tasks[w[0]].uid;
                let t = &mut p.tasks[w[1]];
                if !t.preds.iter().any(|l| l.pred == pred) {
                    t.preds.push(Link { pred, kind: LinkType::FS, lag: 0 });
                }
            }
        }
        Op::Unlink { uids } => {
            let set: HashSet<u32> = uids.iter().copied().collect();
            for t in &mut p.tasks {
                let me = set.contains(&t.uid);
                t.preds.retain(|l| if set.len() == 1 { !me && !set.contains(&l.pred) } else { !(me && set.contains(&l.pred)) });
            }
        }
        Op::MarkOnTrack { uids } => {
            let dd = at(p.status.ok_or("Set a status date in Project › Project Information first.")?) + DAY;
            let s = sched::schedule(p)?;
            for i in idxs(p, &uids).into_iter().filter(|&i| !s.summary[i]) {
                let c = &s.cals[s.ci[i]];
                let n = if dd <= s.es[i] { 0 } else if dd >= s.ef[i] { 100 } else { c.between(s.es[i], dd) * 100 / c.between(s.es[i], s.ef[i]).max(1) };
                set_pct(p, i, n as u8)?;
            }
        }
        Op::Baseline { uids, clear } => {
            let s = sched::schedule(p)?;
            let cost = load::compute(p, &s).cost;
            let all = uids.is_empty();
            let pick: HashSet<u32> = uids.into_iter().collect();
            for (i, t) in p.tasks.iter_mut().enumerate().filter(|(_, t)| all || pick.contains(&t.uid)) {
                t.bl = if clear { None } else { Some(Baseline { start: s.es[i], finish: s.ef[i], dur: if s.summary[i] { s.cals[s.ci[i]].between(s.es[i], s.ef[i]) } else { t.dur }, cost: cost[i] }) };
            }
        }
        Op::Project { name, start, status, calendar, day_hours, manual_default } => {
            if !(0.5..=24.0).contains(&day_hours) {
                return Err("Hours per day must be between 0.5 and 24.".into());
            }
            p.name = name.trim().into();
            p.start = parse::date_in(&start, fmt)?;
            p.status = if status.trim().is_empty() { None } else { Some(parse::date_in(&status, fmt)?) };
            if !p.calendars.iter().any(|c| c.name == calendar) {
                return Err(format!("There is no calendar called \"{calendar}\"."));
            }
            p.calendar = calendar;
            p.day_min = (day_hours * 60.0).round() as i64;
            p.manual_default = manual_default;
        }
        Op::CalPut { old, mut cal } => {
            cal.name = cal.name.trim().into();
            if cal.name.is_empty() {
                return Err("A calendar needs a name.".into());
            }
            for h in cal.week.iter_mut().chain(cal.exceptions.iter_mut().map(|e| &mut e.hours)) {
                h.retain(|(s, e)| s < e && *e <= 1440);
            }
            if cal.week.iter().all(Vec::is_empty) {
                return Err("A calendar needs at least one working day.".into());
            }
            if cal.exceptions.iter().any(|e| e.to < e.from) {
                return Err("An exception ends before it starts.".into());
            }
            let old = old.unwrap_or_else(|| cal.name.clone());
            if cal.name != old && p.calendars.iter().any(|c| c.name == cal.name) {
                return Err(format!("There is already a calendar called \"{}\".", cal.name));
            }
            if p.calendar == old {
                p.calendar = cal.name.clone();
            }
            for t in p.tasks.iter_mut().filter(|t| t.cal.as_deref() == Some(old.as_str())) {
                t.cal = Some(cal.name.clone());
            }
            match p.calendars.iter().position(|c| c.name == old) {
                Some(k) => p.calendars[k] = cal,
                None => p.calendars.push(cal),
            }
        }
        Op::CalDelete { name } => {
            if name == p.calendar {
                return Err("The project calendar cannot be deleted. Choose another project calendar first.".into());
            }
            p.calendars.retain(|c| c.name != name);
        }
        Op::ResInsert { name } => {
            let name = name.trim();
            if p.resources.iter().any(|r| r.name.eq_ignore_ascii_case(name)) {
                return Err(format!("There is already a resource called \"{name}\"."));
            }
            p.resource(if name.is_empty() { "New resource" } else { name });
        }
        Op::ResSet { uid, field, value } => res_set(p, uid, &field, &value)?,
        Op::ResDelete { uids } => p.resources.retain(|r| !uids.contains(&r.uid)),
        Op::Columns { cols } => p.columns = cols,
        Op::Batch(ops) => ops.into_iter().try_for_each(|o| exec(p, o, fmt))?,
    }
    Ok(())
}
