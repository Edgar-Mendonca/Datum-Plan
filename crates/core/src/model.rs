//! The plan model. Durations and lags are whole working days in 0.1.
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use std::ops::Range;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LinkType { FS, SS, FF, SF }

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Link {
    pub pred: u32,
    pub kind: LinkType,
    #[serde(default)]
    pub lag: i32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Task {
    pub uid: u32,
    pub name: String,
    #[serde(default)]
    pub level: u8,
    /// Working days; 0 makes a milestone.
    pub dur: i32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub preds: Vec<Link>,
    #[serde(default)]
    pub pct: u8,
    /// Start no earlier than.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snet: Option<NaiveDate>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub notes: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Calendar {
    pub name: String,
    /// Working weekdays, Monday first.
    pub week: [bool; 7],
    #[serde(default)]
    pub holidays: Vec<NaiveDate>,
}

impl Default for Calendar {
    fn default() -> Self {
        Self { name: "Standard".into(), week: [true, true, true, true, true, false, false], holidays: vec![] }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Project {
    pub format: u32,
    pub name: String,
    pub start: NaiveDate,
    #[serde(default)]
    pub calendar: Calendar,
    #[serde(default)]
    pub tasks: Vec<Task>,
    #[serde(default)]
    pub next_uid: u32,
}

impl Project {
    pub fn new(name: &str, start: NaiveDate) -> Self {
        Self { format: 1, name: name.into(), start, calendar: Calendar::default(), tasks: vec![], next_uid: 1 }
    }

    pub fn is_summary(&self, i: usize) -> bool {
        self.tasks.get(i + 1).is_some_and(|n| n.level > self.tasks[i].level)
    }

    /// Task `i` and all its descendants.
    pub fn subtree(&self, i: usize) -> Range<usize> {
        let l = self.tasks[i].level;
        let mut j = i + 1;
        while j < self.tasks.len() && self.tasks[j].level > l {
            j += 1;
        }
        i..j
    }

    pub fn idx(&self, uid: u32) -> Option<usize> {
        self.tasks.iter().position(|t| t.uid == uid)
    }

    pub fn new_task(&mut self, name: &str, level: u8, dur: i32) -> Task {
        let uid = self.next_uid;
        self.next_uid += 1;
        Task { uid, name: name.into(), level, dur, preds: vec![], pct: 0, snet: None, notes: String::new() }
    }

    /// Repairs anything an edit or a hand-made file can leave inconsistent.
    pub fn normalize(&mut self) {
        let mut prev: i32 = -1;
        for t in &mut self.tasks {
            t.level = (t.level as i32).min(prev + 1).max(0) as u8;
            prev = t.level as i32;
            t.dur = t.dur.max(0);
            t.pct = t.pct.min(100);
        }
        self.next_uid = self.next_uid.max(self.tasks.iter().map(|t| t.uid + 1).max().unwrap_or(1));
        // Links join leaf tasks only (summary links come in a later version).
        let leaf: std::collections::HashSet<u32> =
            (0..self.tasks.len()).filter(|&i| !self.is_summary(i)).map(|i| self.tasks[i].uid).collect();
        for t in &mut self.tasks {
            let me = t.uid;
            if leaf.contains(&me) {
                t.preds.retain(|l| l.pred != me && leaf.contains(&l.pred));
                let mut seen = std::collections::HashSet::new();
                t.preds.retain(|l| seen.insert(l.pred));
            } else {
                t.preds.clear();
            }
        }
    }
}
