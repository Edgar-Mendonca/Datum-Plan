//! What-if scenarios: a scenario is a copy of a plan, saved beside it, that can be changed freely,
//! compared with the plan (the change log and ghost bars) and merged back, kind by kind.
use crate::model::*;
use std::collections::{HashMap, HashSet};

/// The kinds of change a merge can take from a scenario.
pub const KINDS: [(&str, &str); 8] = [
    ("tasks", "Tasks added and deleted"), ("names", "Names"), ("dur", "Durations"), ("links", "Links"),
    ("constraints", "Constraints and deadlines"), ("calendars", "Task calendars"), ("resources", "Resource assignments"), ("progress", "Progress (actual dates and % complete)"),
];

/// A scenario numbers its new tasks this far above the plan's, so tasks added on both sides after
/// the copy never share a unique ID.
const GAP: u32 = 1_000_000;

/// A scenario of `base`: a copy with its own name that knows which plan it came from.
pub fn make(base: &Project, name: &str, base_file: &str) -> Project {
    let mut q = base.clone();
    q.name = name.to_string();
    q.scenario_of = base_file.to_string();
    // A scenario stands alone: it is not a master or a subplan, and keeps no snapshots of the plan.
    q.subplans.clear();
    q.master.clear();
    q.snapshots.clear();
    q.scenario_mark = Some([base.next_uid, base.resources.iter().map(|r| r.uid).max().unwrap_or(0) + 1]);
    q.next_uid = base.next_uid.saturating_add(GAP);
    q
}

/// The plan with the chosen kinds of change taken from the scenario, by unique ID. Everything the
/// merge does not cover (sheets, registers, settings) stays as it is in the plan.
pub fn merge(base: &Project, scen: &Project, kinds: &[String]) -> Project {
    let on = |k: &str| kinds.iter().any(|x| x == k);
    let mut q = base.clone();
    let from: HashMap<u32, &Task> = scen.tasks.iter().map(|t| (t.uid, t)).collect();
    // What the plan had when the scenario was made (an older scenario without the mark: everything).
    let [mark, rmark] = scen.scenario_mark.unwrap_or([u32::MAX; 2]);
    let added = |u: u32| scen.scenario_mark.is_none() || u >= mark;
    // The scenario's resources in the plan: the same one for those both had at the copy, else the
    // plan's resource of that name, else a new one.
    let key = |n: &str| n.trim().to_lowercase();
    let (mut rmap, mut fresh) = (HashMap::new(), vec![]);
    let mut top = q.resources.iter().map(|r| r.uid).max().unwrap_or(0);
    for r in &scen.resources {
        let u = if r.uid < rmark && q.res_idx(r.uid).is_some() { r.uid } else if let Some(x) = q.resources.iter().find(|x| key(&x.name) == key(&r.name)) { x.uid } else { top += 1; fresh.push(Resource { uid: top, ..r.clone() }); top };
        rmap.insert(r.uid, u);
    }
    let res = |s: &Task| -> Vec<Assign> { s.res.iter().filter_map(|a| Some(Assign { res: *rmap.get(&a.res)?, ..a.clone() })).collect() };
    if on("tasks") {
        // Deleted in the scenario: gone (tasks added to the plan since stay). Added in the scenario:
        // after the task that comes before it there (tasks deleted from the plan since stay deleted).
        q.tasks.retain(|t| t.uid >= mark || from.contains_key(&t.uid));
        let have: HashSet<u32> = q.tasks.iter().map(|t| t.uid).collect();
        let mut prev: Option<u32> = None;
        for t in &scen.tasks {
            if !have.contains(&t.uid) && added(t.uid) {
                let at = prev.and_then(|u| q.tasks.iter().position(|x| x.uid == u)).map_or(0, |k| k + 1);
                q.tasks.insert(at, Task { res: res(t), ..t.clone() });
            }
            prev = Some(t.uid);
        }
        q.next_uid = q.next_uid.max(scen.next_uid);
        for c in &scen.calendars { if !q.calendars.iter().any(|x| x.name == c.name) { q.calendars.push(c.clone()) } }
    }
    // New resources come along so new tasks and assignments keep them.
    if on("tasks") || on("resources") { q.resources.extend(fresh) }
    let uids: HashSet<u32> = q.tasks.iter().map(|t| t.uid).collect();
    for t in q.tasks.iter_mut() {
        // Tasks from after the copy are the plan's own (or came in whole above).
        let Some(s) = from.get(&t.uid).filter(|_| t.uid < mark) else { continue };
        if on("names") { t.name = s.name.clone() }
        if on("dur") { (t.dur, t.unit, t.elapsed) = (s.dur, s.unit, s.elapsed) }
        if on("links") { t.preds = s.preds.iter().filter(|l| uids.contains(&l.pred)).cloned().collect() }
        if on("constraints") { (t.constraint, t.cdate, t.deadline) = (s.constraint, s.cdate, s.deadline) }
        if on("calendars") { t.cal = s.cal.clone() }
        if on("resources") { t.res = res(s) }
        if on("progress") { (t.pct, t.phys, t.act_start, t.act_finish) = (s.pct, s.phys, s.act_start, s.act_finish) }
    }
    if on("calendars") { for c in &scen.calendars { if !q.calendars.iter().any(|x| x.name == c.name) { q.calendars.push(c.clone()) } } }
    q.normalize();
    q
}
