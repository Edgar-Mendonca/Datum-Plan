//! An open plan: edit operations, undo/redo and .dplan files.
use crate::{cal::WorkCal, model::*, parse, sched};
use serde::Deserialize;
use std::{collections::HashSet, path::{Path, PathBuf}};

#[derive(Debug, Clone, Deserialize)]
pub enum Op {
    Insert { at: usize, name: String, ms: bool },
    Delete { uids: Vec<u32> },
    Set { uid: u32, field: String, value: String },
    Indent { uids: Vec<u32> },
    Outdent { uids: Vec<u32> },
    Link { uids: Vec<u32> },
    Unlink { uids: Vec<u32> },
    Project { name: String, start: String, week: [bool; 7] },
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
    pub fn apply(&mut self, op: Op) -> Result<(), String> {
        let mut q = self.p.clone();
        exec(&mut q, op)?;
        q.normalize();
        sched::schedule(&q, &mut WorkCal::new(&q.calendar, q.start))?;
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
        let mut p: Project = serde_json::from_str(&text).map_err(|e| format!("This is not a Datum Plan file ({e})."))?;
        p.normalize();
        let mut d = Self::new(p);
        d.path = Some(path.into());
        Ok(d)
    }

    pub fn save(&mut self, path: &Path) -> Result<(), String> {
        let text = serde_json::to_string_pretty(&self.p).map_err(|e| e.to_string())?;
        std::fs::write(path, text).map_err(|e| format!("Could not save {}: {e}", path.display()))?;
        self.path = Some(path.into());
        self.dirty = false;
        Ok(())
    }
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

fn exec(p: &mut Project, op: Op) -> Result<(), String> {
    match op {
        Op::Insert { at, name, ms } => {
            let at = at.min(p.tasks.len());
            let level = p.tasks.get(at).or_else(|| p.tasks.last()).map_or(0, |t| t.level);
            let t = p.new_task(name.trim(), level, if ms { 0 } else { 1 });
            p.tasks.insert(at, t);
        }
        Op::Delete { uids } => {
            let gone: HashSet<usize> = idxs(p, &uids).into_iter().flat_map(|i| p.subtree(i)).collect();
            let mut i = 0;
            p.tasks.retain(|_| (!gone.contains(&i), i += 1).0);
        }
        Op::Set { uid, field, value } => {
            let i = p.idx(uid).ok_or("That task no longer exists.")?;
            let leaf_only = |what: &str| -> Result<(), String> {
                if p.is_summary(i) { Err(format!("A summary task's {what} is calculated from the tasks under it.")) } else { Ok(()) }
            };
            match field.as_str() {
                "name" => p.tasks[i].name = value.trim().into(),
                "notes" => p.tasks[i].notes = value,
                "dur" => {
                    leaf_only("duration")?;
                    p.tasks[i].dur = parse::dur(&value)?;
                }
                "pct" => {
                    leaf_only("% complete")?;
                    let v: u8 = value.trim().trim_end_matches('%').parse().map_err(|_| "Enter a number from 0 to 100.")?;
                    p.tasks[i].pct = v.min(100);
                }
                "finish" => {
                    leaf_only("finish")?;
                    let d = parse::date(&value)?;
                    let mut cal = WorkCal::new(&p.calendar, p.start);
                    let es = sched::schedule(p, &mut cal)?.es[i];
                    let end = cal.ord(d) + i64::from(cal.is_work(d));
                    p.tasks[i].dur = (end - es).max(0) as i32;
                }
                "start" => {
                    leaf_only("start")?;
                    p.tasks[i].snet = if value.trim().is_empty() { None } else { Some(parse::date(&value)?) };
                }
                "preds" => {
                    let ids: Vec<u32> = p.tasks.iter().map(|t| t.uid).collect();
                    let links = parse::preds(&value, &ids)?;
                    let summary_hit = links.iter().any(|l| p.idx(l.pred).is_some_and(|j| p.is_summary(j)));
                    if p.is_summary(i) || summary_hit {
                        return Err("Links to and from summary tasks come in a later version. Link the tasks inside it.".into());
                    }
                    if links.iter().any(|l| l.pred == uid) {
                        return Err("A task cannot be its own predecessor.".into());
                    }
                    p.tasks[i].preds = links;
                }
                f => return Err(format!("{f} is calculated and cannot be edited.")),
            }
        }
        Op::Indent { uids } => shift(p, &uids, 1),
        Op::Outdent { uids } => shift(p, &uids, -1),
        Op::Link { uids } => {
            let v: Vec<usize> = idxs(p, &uids).into_iter().filter(|&i| !p.is_summary(i)).collect();
            if v.len() < 2 {
                return Err("Select two or more tasks (not summaries) to link.".into());
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
        Op::Project { name, start, week } => {
            if !week.iter().any(|&w| w) {
                return Err("Pick at least one working day.".into());
            }
            p.name = name.trim().into();
            p.start = parse::date(&start)?;
            p.calendar.week = week;
        }
        Op::Batch(ops) => ops.into_iter().try_for_each(|o| exec(p, o))?,
    }
    Ok(())
}
