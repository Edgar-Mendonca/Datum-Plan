//! What-if scenarios: a scenario is a copy of a plan, saved beside it, that can be changed freely,
//! compared with the plan (the change log and ghost bars) and merged back, kind by kind.
use crate::model::*;
use std::collections::{HashMap, HashSet};

/// The kinds of change a merge can take from a scenario.
pub const KINDS: [(&str, &str); 8] = [
    ("tasks", "Tasks added and deleted"), ("names", "Names"), ("dur", "Durations"), ("links", "Links"),
    ("constraints", "Constraints and deadlines"), ("calendars", "Task calendars"), ("resources", "Resource assignments"), ("progress", "Progress (actual dates and % complete)"),
];

/// A scenario of `base`: a copy with its own name that knows which plan it came from.
pub fn make(base: &Project, name: &str, base_file: &str) -> Project {
    let mut q = base.clone();
    q.name = name.to_string();
    q.scenario_of = base_file.to_string();
    // A scenario stands alone: it is not a master or a subplan, and keeps no snapshots of the plan.
    q.subplans.clear();
    q.master.clear();
    q
}

/// The plan with the chosen kinds of change taken from the scenario, by unique ID. Everything the
/// merge does not cover (sheets, registers, settings) stays as it is in the plan.
pub fn merge(base: &Project, scen: &Project, kinds: &[String]) -> Project {
    let on = |k: &str| kinds.iter().any(|x| x == k);
    let mut q = base.clone();
    let from: HashMap<u32, &Task> = scen.tasks.iter().map(|t| (t.uid, t)).collect();
    if on("tasks") {
        // Deleted in the scenario: gone. Added in the scenario: after the task that comes before it there.
        q.tasks.retain(|t| from.contains_key(&t.uid));
        let have: HashSet<u32> = q.tasks.iter().map(|t| t.uid).collect();
        let mut prev: Option<u32> = None;
        for t in &scen.tasks {
            if !have.contains(&t.uid) {
                let at = prev.and_then(|u| q.tasks.iter().position(|x| x.uid == u)).map_or(0, |k| k + 1);
                q.tasks.insert(at, t.clone());
            }
            prev = Some(t.uid);
        }
        q.next_uid = q.next_uid.max(scen.next_uid);
        // New resources come along so new tasks keep their assignments.
        for r in &scen.resources { if !q.resources.iter().any(|x| x.uid == r.uid) { q.resources.push(r.clone()) } }
        for c in &scen.calendars { if !q.calendars.iter().any(|x| x.name == c.name) { q.calendars.push(c.clone()) } }
    }
    let uids: HashSet<u32> = q.tasks.iter().map(|t| t.uid).collect();
    for t in q.tasks.iter_mut() {
        let Some(s) = from.get(&t.uid) else { continue };
        if on("names") { t.name = s.name.clone() }
        if on("dur") { (t.dur, t.unit, t.elapsed) = (s.dur, s.unit, s.elapsed) }
        if on("links") { t.preds = s.preds.iter().filter(|l| uids.contains(&l.pred)).cloned().collect() }
        if on("constraints") { (t.constraint, t.cdate, t.deadline) = (s.constraint, s.cdate, s.deadline) }
        if on("calendars") { t.cal = s.cal.clone() }
        if on("resources") { t.res = s.res.clone() }
        if on("progress") { (t.pct, t.phys, t.act_start, t.act_finish) = (s.pct, s.phys, s.act_start, s.act_finish) }
    }
    if on("resources") { for r in &scen.resources { if !q.resources.iter().any(|x| x.uid == r.uid) { q.resources.push(r.clone()) } } }
    if on("calendars") { for c in &scen.calendars { if !q.calendars.iter().any(|x| x.name == c.name) { q.calendars.push(c.clone()) } } }
    q.normalize();
    q
}
