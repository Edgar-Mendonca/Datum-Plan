//! The plan model (file format 2). Instants are minutes since 1970-01-01 00:00 local time;
//! durations and lags are working minutes.
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::ops::Range;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LinkType { FS, SS, FF, SF }

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Constraint { #[default] ASAP, ALAP, SNET, SNLT, FNET, FNLT, MSO, MFO }

pub const CONSTRAINTS: [(Constraint, &str); 8] = [
    (Constraint::ASAP, "As Soon As Possible"), (Constraint::ALAP, "As Late As Possible"),
    (Constraint::SNET, "Start No Earlier Than"), (Constraint::SNLT, "Start No Later Than"),
    (Constraint::FNET, "Finish No Earlier Than"), (Constraint::FNLT, "Finish No Later Than"),
    (Constraint::MSO, "Must Start On"), (Constraint::MFO, "Must Finish On"),
];

/// How a duration is shown: minutes, hours, days, weeks or months.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Unit { M, H, #[default] D, W, Mo }

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResKind { #[default] Work, Material, Cost }

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Link {
    pub pred: u32,
    pub kind: LinkType,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub lag: i64,
}

/// Work resources: units 1.0 = 100%. Material: quantity. Cost: amount.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Assign { pub res: u32, pub units: f64 }

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Baseline { pub start: i64, pub finish: i64, pub dur: i64, #[serde(default)] pub cost: f64 }

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Task {
    pub uid: u32,
    pub name: String,
    #[serde(skip_serializing_if = "is_zero_u8")]
    pub level: u8,
    pub dur: i64,
    #[serde(skip_serializing_if = "is_day")]
    pub unit: Unit,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub preds: Vec<Link>,
    #[serde(skip_serializing_if = "is_zero_u8")]
    pub pct: u8,
    #[serde(skip_serializing_if = "is_asap")]
    pub constraint: Constraint,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cdate: Option<NaiveDate>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deadline: Option<NaiveDate>,
    #[serde(skip_serializing_if = "is_false")]
    pub manual: bool,
    /// Start of a manually scheduled task.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mstart: Option<NaiveDate>,
    #[serde(skip_serializing_if = "is_false")]
    pub inactive: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cal: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub act_start: Option<NaiveDate>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub act_finish: Option<NaiveDate>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub res: Vec<Assign>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bl: Option<Baseline>,
    #[serde(skip_serializing_if = "is_false")]
    pub timeline: bool,
    /// Activity ID (P6) or any short code.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub code: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub notes: String,
}

/// Working intervals of one day, in minutes from midnight.
pub type Hours = Vec<(u16, u16)>;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Exception {
    pub name: String,
    pub from: NaiveDate,
    pub to: NaiveDate,
    /// Empty means not working.
    #[serde(default)]
    pub hours: Hours,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Calendar {
    pub name: String,
    /// Monday first.
    pub week: [Hours; 7],
    #[serde(default)]
    pub exceptions: Vec<Exception>,
}

pub const STD_HOURS: [(u16, u16); 2] = [(480, 720), (780, 1020)];

impl Calendar {
    pub fn standard(name: &str) -> Self {
        let d = STD_HOURS.to_vec();
        Self { name: name.into(), week: [d.clone(), d.clone(), d.clone(), d.clone(), d, vec![], vec![]], exceptions: vec![] }
    }
    pub fn full(name: &str) -> Self {
        let d = vec![(0, 1440)];
        Self { name: name.into(), week: std::array::from_fn(|_| d.clone()), exceptions: vec![] }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Resource {
    pub uid: u32,
    pub name: String,
    pub kind: ResKind,
    pub group: String,
    /// 1.0 = one full-time person.
    pub max: f64,
    /// Per hour for work, per unit for material.
    pub rate: f64,
    pub per_use: f64,
    /// Material unit, such as m3 or t.
    pub label: String,
}

impl Default for Resource {
    fn default() -> Self {
        Self { uid: 0, name: String::new(), kind: ResKind::Work, group: String::new(), max: 1.0, rate: 0.0, per_use: 0.0, label: String::new() }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Project {
    pub format: u32,
    pub name: String,
    pub start: NaiveDate,
    /// Status (data) date: progress is reported up to the end of this day.
    pub status: Option<NaiveDate>,
    /// Working minutes in a "day" when durations are typed or shown in days.
    pub day_min: i64,
    /// New tasks are manually scheduled.
    pub manual_default: bool,
    /// Name of the project calendar.
    pub calendar: String,
    pub calendars: Vec<Calendar>,
    pub tasks: Vec<Task>,
    pub resources: Vec<Resource>,
    pub next_uid: u32,
    /// Grid columns: field and width.
    pub columns: Vec<(String, u16)>,
}

impl Default for Project {
    fn default() -> Self {
        Self::new("Project1", NaiveDate::from_ymd_opt(2026, 1, 5).unwrap())
    }
}

pub const DEFAULT_COLUMNS: [(&str, u16); 7] =
    [("name", 250), ("dur", 78), ("start", 86), ("finish", 86), ("preds", 100), ("res", 140), ("pct", 78)];

fn is_zero(v: &i64) -> bool { *v == 0 }
fn is_zero_u8(v: &u8) -> bool { *v == 0 }
fn is_false(v: &bool) -> bool { !*v }
fn is_day(u: &Unit) -> bool { *u == Unit::D }
fn is_asap(c: &Constraint) -> bool { *c == Constraint::ASAP }

impl Project {
    pub fn new(name: &str, start: NaiveDate) -> Self {
        Self {
            format: 2, name: name.into(), start, status: None, day_min: 480, manual_default: false,
            calendar: "Standard".into(), calendars: vec![Calendar::standard("Standard")], tasks: vec![], resources: vec![],
            next_uid: 1, columns: DEFAULT_COLUMNS.iter().map(|(f, w)| (f.to_string(), *w)).collect(),
        }
    }

    /// Reads a .dplan file, upgrading 0.1 files (whole working days, one Mon–Fri calendar).
    pub fn from_json(text: &str) -> Result<Self, String> {
        let mut v: Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
        if v["format"].as_u64().unwrap_or(1) < 2 {
            let c = v["calendar"].take();
            let day = |on: bool| if on { json!(STD_HOURS) } else { json!([]) };
            let week: Vec<Value> = (0..7).map(|i| day(c["week"][i].as_bool().unwrap_or(i < 5))).collect();
            let exc: Vec<Value> = c["holidays"].as_array().into_iter().flatten().map(|d| json!({"name": "Holiday", "from": d, "to": d})).collect();
            v["calendars"] = json!([{ "name": "Standard", "week": week, "exceptions": exc }]);
            v["calendar"] = json!("Standard");
            for t in v.get_mut("tasks").and_then(Value::as_array_mut).into_iter().flatten() {
                t["dur"] = json!(t["dur"].as_i64().unwrap_or(0) * 480);
                for l in t.get_mut("preds").and_then(Value::as_array_mut).into_iter().flatten() {
                    l["lag"] = json!(l["lag"].as_i64().unwrap_or(0) * 480);
                }
                if let Some(d) = t["snet"].as_str().map(String::from) {
                    t["constraint"] = json!("SNET");
                    t["cdate"] = json!(d);
                }
            }
            v["format"] = json!(2);
        }
        let mut p: Project = serde_json::from_value(v).map_err(|e| e.to_string())?;
        p.normalize();
        Ok(p)
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

    pub fn res_idx(&self, uid: u32) -> Option<usize> {
        self.resources.iter().position(|r| r.uid == uid)
    }

    pub fn new_task(&mut self, name: &str, level: u8, dur: i64) -> Task {
        let uid = self.next_uid;
        self.next_uid += 1;
        Task { uid, name: name.into(), level, dur, manual: self.manual_default, ..Default::default() }
    }

    /// Finds a resource by name, adding a work resource if there is none.
    pub fn resource(&mut self, name: &str) -> u32 {
        if let Some(r) = self.resources.iter().find(|r| r.name.eq_ignore_ascii_case(name)) {
            return r.uid;
        }
        let uid = self.resources.iter().map(|r| r.uid).max().unwrap_or(0) + 1;
        self.resources.push(Resource { uid, name: name.into(), ..Default::default() });
        uid
    }

    pub fn cal(&self, name: Option<&str>) -> &Calendar {
        let n = name.unwrap_or(&self.calendar);
        self.calendars.iter().find(|c| c.name == n).or_else(|| self.calendars.iter().find(|c| c.name == self.calendar)).unwrap_or(&self.calendars[0])
    }

    /// Repairs anything an edit or a hand-made file can leave inconsistent.
    pub fn normalize(&mut self) {
        if self.calendars.is_empty() {
            self.calendars.push(Calendar::standard("Standard"));
        }
        if !self.calendars.iter().any(|c| c.name == self.calendar) {
            self.calendar = self.calendars[0].name.clone();
        }
        if self.day_min <= 0 {
            self.day_min = 480;
        }
        let mut prev: i32 = -1;
        for t in &mut self.tasks {
            t.level = (t.level as i32).min(prev + 1).max(0) as u8;
            prev = t.level as i32;
            t.dur = t.dur.max(0);
            t.pct = t.pct.min(100);
        }
        self.next_uid = self.next_uid.max(self.tasks.iter().map(|t| t.uid + 1).max().unwrap_or(1));
        let uids: std::collections::HashSet<u32> = self.tasks.iter().map(|t| t.uid).collect();
        let res: std::collections::HashSet<u32> = self.resources.iter().map(|r| r.uid).collect();
        let cals: Vec<String> = self.calendars.iter().map(|c| c.name.clone()).collect();
        // A task cannot be linked to its own summary tasks.
        let mut stack: Vec<(u8, u32)> = vec![];
        let anc: std::collections::HashMap<u32, Vec<u32>> = self.tasks.iter().map(|t| {
            stack.retain(|&(l, _)| l < t.level);
            let a = stack.iter().map(|&(_, u)| u).collect();
            stack.push((t.level, t.uid));
            (t.uid, a)
        }).collect();
        for t in &mut self.tasks {
            let me = t.uid;
            let mut seen = std::collections::HashSet::new();
            t.preds.retain(|l| l.pred != me && uids.contains(&l.pred) && !anc[&me].contains(&l.pred) && !anc[&l.pred].contains(&me) && seen.insert(l.pred));
            t.res.retain(|a| res.contains(&a.res));
            if t.cal.as_ref().is_some_and(|c| !cals.contains(c)) {
                t.cal = None;
            }
        }
        if self.columns.is_empty() {
            self.columns = Project::new("", self.start).columns;
        }
    }
}
