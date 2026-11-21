//! Roles: generic resources (a Mechanical Engineer × 2) planned before the names are known, and
//! the people who could take a role's work, with the hours they have free on the role's days.
use crate::{cal::{date_of, DAY}, fields::Calc, model::*};
use chrono::NaiveDate;
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Serialize, Debug, Default, Clone, PartialEq)]
pub struct RoleTask { pub uid: u32, pub id: usize, pub name: String, pub hours: f64, pub start: Option<NaiveDate>, pub finish: Option<NaiveDate> }

#[derive(Serialize, Debug, Default, Clone, PartialEq)]
pub struct Candidate {
    pub uid: u32,
    pub name: String,
    pub group: String,
    /// In the role's trade (same group, or a group named as the role).
    pub same_trade: bool,
    /// Hours of the role's work they could take on its days (free time up to the role's hours each day).
    pub cover: f64,
    /// They can take all of it.
    pub fits: bool,
}

#[derive(Serialize, Debug, Default, Clone, PartialEq)]
pub struct RoleOut { pub uid: u32, pub name: String, pub group: String, pub tasks: Vec<RoleTask>, pub needed: f64, pub people: Vec<Candidate> }

/// The tasks role `role` is on (those in `uids`, or all), the hours they need, and the people who
/// could take them: work resources that are not roles, the role's trade first, then those who can
/// take all of it, then by the hours they can take.
pub fn candidates(c: &Calc, role: u32, uids: &[u32]) -> Result<RoleOut, String> {
    let (p, s, l) = (c.p, &c.s, &c.l);
    let k = p.res_idx(role).ok_or("That resource no longer exists.")?;
    let r = &p.resources[k];
    if !r.role { return Err(format!("{} is not a role (Resource Sheet › Role).", r.name)) }
    let picked = |i: usize| uids.is_empty() || uids.contains(&p.tasks[i].uid);
    let mut tasks = vec![];
    // The role's minutes on each day, over the chosen tasks.
    let mut need: BTreeMap<i64, f64> = BTreeMap::new();
    for (i, t) in p.tasks.iter().enumerate().filter(|&(i, t)| !s.summary[i] && !s.off[i] && picked(i) && t.res.iter().any(|a| a.res == role)) {
        let mut mins = 0.0;
        for (&(_, _, d), &m) in l.assign_daily.range((k, i, i64::MIN)..=(k, i, i64::MAX)) { *need.entry(d).or_default() += m; mins += m }
        tasks.push(RoleTask { uid: t.uid, id: i + 1, name: t.name.clone(), hours: mins / 60.0, start: Some(date_of(s.es[i])), finish: Some(date_of((s.ef[i] - 1).max(s.es[i]))) });
    }
    let needed: f64 = need.values().sum::<f64>() / 60.0;
    let key = |x: &str| x.trim().to_lowercase();
    let mut people: Vec<Candidate> = p.resources.iter().enumerate().filter(|(_, x)| x.kind == ResKind::Work && !x.role).map(|(j, x)| {
        let cover: f64 = need.iter().map(|(&d, &m)| {
            let day = date_of(d * DAY);
            let cap = x.max_on(day) * s.cals[s.rcal[j]].day_minutes(day).max(0) as f64;
            m.min((cap - l.daily.get(&(j, d)).copied().unwrap_or(0.0)).max(0.0))
        }).sum::<f64>() / 60.0;
        let same_trade = !x.group.trim().is_empty() && (key(&x.group) == key(&r.group) || key(&x.group) == key(&r.name));
        Candidate { uid: x.uid, name: x.name.clone(), group: x.group.clone(), same_trade, cover, fits: cover + 0.01 >= needed }
    }).collect();
    people.sort_by(|a, b| b.same_trade.cmp(&a.same_trade).then(b.fits.cmp(&a.fits)).then(b.cover.total_cmp(&a.cover)).then(a.name.cmp(&b.name)));
    Ok(RoleOut { uid: role, name: r.name.clone(), group: r.group.clone(), tasks, needed, people })
}

/// Gives the role's work on the tasks `uids` (all its tasks when empty) to `person`: their
/// assignment takes the role's units, rate table, contour and typed hours (added to the person's
/// units when they are on the task already).
pub fn assign(p: &mut Project, role: u32, person: u32, uids: &[u32]) -> Result<usize, String> {
    let rk = p.res_idx(role).ok_or("That role no longer exists.")?;
    if !p.resources[rk].role { return Err(format!("{} is not a role.", p.resources[rk].name)) }
    let pk = p.res_idx(person).ok_or("That person no longer exists.")?;
    let x = &p.resources[pk];
    if x.kind != ResKind::Work || x.role { return Err(format!("{} cannot take a role's work: choose a person or crew (a work resource that is not a role).", x.name)) }
    let mut n = 0;
    for t in p.tasks.iter_mut().filter(|t| (uids.is_empty() || uids.contains(&t.uid)) && t.res.iter().any(|a| a.res == role)) {
        let at = t.res.iter().position(|a| a.res == role).unwrap_or(0);
        let a = t.res.remove(at);
        match t.res.iter_mut().find(|b| b.res == person) {
            Some(b) => { b.units += a.units; if !a.daily.is_empty() || !b.daily.is_empty() { b.daily.clear() } }
            None => t.res.insert(at, Assign { res: person, ..a }),
        }
        n += 1;
    }
    if n == 0 { return Err("The role is on none of those tasks.".into()) }
    Ok(n)
}
