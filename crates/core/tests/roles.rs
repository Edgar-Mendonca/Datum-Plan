//! Roles (0.9.40): a generic resource planned before the names are known, and handing its work to
//! people. Plans start Monday 2 Nov 2026 on the standard calendar (8 h a day).
use chrono::NaiveDate;
use datum_core::{doc::{Doc, Op}, fields::Calc, *};

fn d(y: i32, m: u32, day: u32) -> NaiveDate { NaiveDate::from_ymd_opt(y, m, day).unwrap() }
const F: &str = "%Y-%m-%d";

/// Two 5-day tasks in a row (T1 → T2); "Mech Engineer" is a role (× 2) at 100 % on both.
/// Ann (Mechanical, full time) is free; Bob (Mechanical) is busy on T3 all week 1; Cy (Electrical) is free.
fn plan() -> Doc {
    let mut p = Project::new("A", d(2026, 11, 2));
    for n in ["T1", "T2", "T3"] { let t = p.new_task(n, 0, 0); p.tasks.push(t) }
    let r = |uid, name: &str, group: &str| Resource { uid, name: name.into(), group: group.into(), max: 1.0, rate: 10.0, ..Default::default() };
    p.resources = vec![Resource { max: 2.0, role: true, ..r(1, "Mech Engineer", "Mechanical") }, r(2, "Ann", "Mechanical"), r(3, "Bob", "Mechanical"), r(4, "Cy", "Electrical")];
    let mut doc = Doc::new(p);
    for (k, (dur, preds, res)) in [("5d", "", "Mech Engineer"), ("5d", "1", "Mech Engineer"), ("5d", "", "Bob")].into_iter().enumerate() {
        let uid = doc.p.tasks[k].uid;
        for (f, v) in [("dur", dur), ("preds", preds), ("res", res)] { doc.apply(Op::Set { uid, field: f.into(), value: v.into() }, F).unwrap() }
    }
    doc
}
fn calc(doc: &Doc) -> Calc<'static> { Calc::new(Box::leak(Box::new(doc.p.clone())), F, d(2026, 11, 2)) }

/// The role needs 80 h (two weeks at 8 h). Ann can take all 80; Bob only week 2 (40); Cy all 80 but
/// is not in the trade. Order: trade first, then those who can take all, then by hours.
#[test]
fn people_who_could_take_the_role() {
    let x = plan();
    let o = roles::candidates(&calc(&x), 1, &[]).unwrap();
    assert_eq!(o.tasks.len(), 2);
    assert_eq!(o.needed, 80.0);
    let got: Vec<(&str, f64, bool, bool)> = o.people.iter().map(|c| (c.name.as_str(), c.cover, c.fits, c.same_trade)).collect();
    assert_eq!(got, vec![("Ann", 80.0, true, true), ("Bob", 40.0, false, true), ("Cy", 80.0, true, false)]);
    // Only T1: 40 h, which Bob cannot take (busy that week).
    let t1 = x.p.tasks[0].uid;
    let o = roles::candidates(&calc(&x), 1, &[t1]).unwrap();
    assert_eq!((o.needed, o.people[1].name.as_str(), o.people[1].cover), (40.0, "Bob", 0.0));
    assert!(roles::candidates(&calc(&x), 2, &[]).is_err(), "Ann is not a role");
}

/// Handing T2 to Bob keeps the units and the contour; the role keeps T1. Handing the rest to Ann.
#[test]
fn hand_the_work_to_people() {
    let mut x = plan();
    let (t1, t2) = (x.p.tasks[0].uid, x.p.tasks[1].uid);
    x.apply(Op::Set { uid: t2, field: "contour".into(), value: "Bell".into() }, F).unwrap();
    x.apply(Op::RoleAssign { role: 1, person: 3, uids: vec![t2] }, F).unwrap();
    let c = calc(&x);
    assert_eq!((c.text(0, "res"), c.text(1, "res"), c.text(1, "contour")), ("Mech Engineer".to_string(), "Bob".to_string(), "Bell".to_string()));
    assert_eq!(c.val(1, "work").num(), 40.0, "the same work");
    x.apply(Op::RoleAssign { role: 1, person: 2, uids: vec![] }, F).unwrap();
    assert_eq!(calc(&x).text(0, "res"), "Ann");
    // Nothing left for the role; people and roles in the wrong place are refused.
    assert!(x.apply(Op::RoleAssign { role: 1, person: 2, uids: vec![] }, F).is_err());
    assert!(x.apply(Op::RoleAssign { role: 2, person: 3, uids: vec![t1] }, F).is_err(), "Ann is not a role");
}

/// Handing a role's work to someone already on the task adds the units together.
#[test]
fn onto_someone_already_there() {
    let mut x = plan();
    let t1 = x.p.tasks[0].uid;
    x.apply(Op::Set { uid: t1, field: "res".into(), value: "Mech Engineer, Ann[50%]".into() }, F).unwrap();
    x.apply(Op::RoleAssign { role: 1, person: 2, uids: vec![t1] }, F).unwrap();
    assert_eq!(calc(&x).text(0, "res"), "Ann[150%]");
}

/// The Role column reads and sets the flag; a plan saved with a role opens with it.
#[test]
fn the_role_column_and_the_file() {
    let mut x = plan();
    x.apply(Op::ResSet { uid: 4, field: "role".into(), value: "Yes".into() }, F).unwrap();
    let c = calc(&x);
    assert_eq!(fields::res_text(&c, 3, "role"), "Yes");
    let back: Project = serde_json::from_str(&serde_json::to_string(&x.p).unwrap()).unwrap();
    assert!(back.resources[0].role && back.resources[3].role && !back.resources[1].role);
}
