use crate::{cal::{at, date_of, WorkCal, DAY}, doc::{copy, Doc, Op}, fields::Calc, load, parse, sched::schedule, *};
use chrono::NaiveDate;

fn d(y: i32, m: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, day).unwrap()
}
const H: i64 = 60;
const D: i64 = 480;

/// Leaf tasks with (working days, preds) on the standard calendar starting Monday 2 Nov 2026.
fn plan(rows: &[(i64, &str)]) -> Project {
    let mut p = Project::new("T", d(2026, 11, 2));
    for (i, &(dur, _)) in rows.iter().enumerate() {
        let t = p.new_task(&format!("T{}", i + 1), 0, dur * D);
        p.tasks.push(t);
    }
    let ids: Vec<u32> = p.tasks.iter().map(|t| t.uid).collect();
    for (t, &(_, pr)) in p.tasks.iter_mut().zip(rows) {
        t.preds = parse::preds(pr, &ids, D).unwrap();
    }
    p
}

/// "dd hh:mm" of an instant, for readable assertions.
fn when(t: i64) -> String {
    format!("{} {:02}:{:02}", date_of(t).format("%d"), t.rem_euclid(DAY) / 60, t.rem_euclid(60))
}

fn op(doc: &mut Doc, o: Op) -> Result<(), String> {
    doc.apply(o, "%d-%b-%y")
}

#[test]
fn calendar_arithmetic() {
    let c = WorkCal::new(&Calendar::standard("S"));
    let mon = at(d(2026, 11, 2));
    assert_eq!(when(c.next_start(mon)), "02 08:00");
    assert_eq!(when(c.add(mon + 8 * H, 5 * H)), "02 14:00"); // lunch skipped
    assert_eq!(when(c.add(mon + 8 * H, 5 * D)), "06 17:00");
    assert_eq!(when(c.add(at(d(2026, 11, 6)) + 17 * H, 1)), "09 08:01"); // over the weekend
    assert_eq!(when(c.sub(at(d(2026, 11, 9)) + 8 * H, H)), "06 16:00");
    assert_eq!(c.between(mon, mon + 7 * DAY), 5 * D);
    assert_eq!(when(c.prev_end(at(d(2026, 11, 9)))), "06 17:00");
    let mut hol = Calendar::standard("H");
    hol.exceptions.push(Exception { name: "Holiday".into(), from: d(2026, 11, 3), to: d(2026, 11, 4), hours: vec![] });
    let c = WorkCal::new(&hol);
    assert_eq!(when(c.add(mon + 8 * H, 2 * D)), "05 17:00");
}

#[test]
fn forward_backward_and_float() {
    // 1 → 2 → 4 and 1 → 3 → 4; 3 is shorter so it has float.
    let p = plan(&[(2, ""), (5, "1"), (2, "1"), (1, "2,3")]);
    let s = schedule(&p).unwrap();
    assert_eq!(when(s.es[1]), "04 08:00");
    assert_eq!(when(s.ef[1]), "10 17:00");
    assert_eq!(when(s.es[3]), "11 08:00");
    assert_eq!(s.tf, vec![0, 0, 3 * D, 0]);
    assert_eq!(s.ff[2], 3 * D);
}

#[test]
fn link_types_lags_and_hours() {
    let mut p = plan(&[(4, ""), (3, "1SS+2d"), (3, "1FF"), (2, "1FS-1d"), (0, "1")]);
    p.tasks[1].preds[0].lag = 4 * H; // SS+4h
    let s = schedule(&p).unwrap();
    assert_eq!(when(s.es[1]), "02 13:00");
    assert_eq!(when(s.ef[2]), "05 17:00");
    assert_eq!(when(s.es[3]), "05 08:00");
    assert_eq!(when(s.es[4]), "05 17:00"); // a milestone sits at its predecessor's finish
}

#[test]
fn constraints_deadlines_and_alap() {
    let mut p = plan(&[(2, ""), (2, "1"), (1, "")]);
    p.tasks[0].constraint = Constraint::SNET;
    p.tasks[0].cdate = Some(d(2026, 11, 4));
    p.tasks[1].deadline = Some(d(2026, 11, 6));
    p.tasks[2].constraint = Constraint::ALAP;
    let s = schedule(&p).unwrap();
    assert_eq!(when(s.es[0]), "04 08:00");
    assert_eq!(when(s.ef[1]), "09 17:00");
    assert_eq!(s.tf[1], -D); // a missed deadline gives negative float
    assert_eq!(when(s.es[2]), "09 08:00"); // as late as possible
    p.tasks[0].constraint = Constraint::MFO;
    p.tasks[0].cdate = Some(d(2026, 11, 3));
    let s = schedule(&p).unwrap();
    assert_eq!(when(s.ef[0]), "03 17:00");
}

#[test]
fn task_calendars_summary_links_and_inactive() {
    let mut p = plan(&[(0, ""), (2, ""), (2, "2"), (1, "1"), (3, "")]);
    p.calendars.push(Calendar::full("24 Hours"));
    p.tasks[1].level = 1;
    p.tasks[2].level = 1;
    p.tasks[2].cal = Some("24 Hours".into());
    p.tasks[4].inactive = true;
    let s = schedule(&p).unwrap();
    assert_eq!(when(s.ef[2]), "04 09:00"); // 16 working hours on a 24-hour calendar
    assert_eq!(when(s.es[3]), "04 09:00"); // the link from the summary waits for all its tasks
    assert_eq!(s.finish, s.ef[3]); // the inactive task does not extend the project
    p.tasks[1].preds = vec![Link { pred: p.tasks[0].uid, kind: LinkType::FS, lag: 0, pct: false }];
    assert!(schedule(&p).err().unwrap().contains("own summary"));
}

#[test]
fn progress_against_the_status_date() {
    let mut p = plan(&[(4, ""), (4, "1"), (2, "")]);
    p.status = Some(d(2026, 11, 4));
    p.tasks[0].act_start = Some(d(2026, 11, 2));
    p.tasks[0].pct = 50;
    p.tasks[2].act_start = Some(d(2026, 11, 2));
    p.tasks[2].act_finish = Some(d(2026, 11, 3));
    p.tasks[2].pct = 100;
    let s = schedule(&p).unwrap();
    assert_eq!(when(s.ef[0]), "06 17:00"); // 2 days remain after the status date
    assert_eq!(when(s.ef[2]), "03 17:00");
    let mut doc = Doc::new(p);
    let uid = doc.p.tasks[1].uid;
    op(&mut doc, Op::Set { uid, field: "pct".into(), value: "100".into() }).unwrap();
    assert!(doc.p.tasks[1].act_start.is_some() && doc.p.tasks[1].act_finish.is_some());
    op(&mut doc, Op::Set { uid, field: "pct".into(), value: "0".into() }).unwrap();
    assert!(doc.p.tasks[1].act_start.is_none());
    let u0 = doc.p.tasks[0].uid;
    op(&mut doc, Op::MarkOnTrack { uids: vec![u0, uid] }).unwrap();
    assert_eq!(doc.p.tasks[1].pct, 0);
}

#[test]
fn edits_undo_and_cycles() {
    let mut doc = Doc::new(plan(&[(1, ""), (1, "1"), (1, ""), (1, "")]));
    let u: Vec<u32> = doc.p.tasks.iter().map(|t| t.uid).collect();
    assert!(op(&mut doc, Op::Set { uid: u[0], field: "preds".into(), value: "2".into() }).unwrap_err().contains("Circular"));
    op(&mut doc, Op::Set { uid: u[0], field: "dur".into(), value: "2w".into() }).unwrap();
    assert_eq!((doc.p.tasks[0].dur, doc.p.tasks[0].unit), (10 * D, Unit::W));
    doc.undo();
    assert_eq!(doc.p.tasks[0].dur, D);
    doc.redo();
    op(&mut doc, Op::Indent { uids: vec![u[1], u[2]] }).unwrap();
    assert!(doc.p.is_summary(0));
    assert!(doc.p.tasks[1].preds.is_empty()); // a link to its own new summary is dropped
    assert!(doc.p.tasks[0].preds.is_empty());
    op(&mut doc, Op::Outdent { uids: vec![u[1], u[2]] }).unwrap();
    op(&mut doc, Op::Move { uids: vec![u[3]], up: true }).unwrap();
    assert_eq!(doc.p.tasks[2].uid, u[3]);
    op(&mut doc, Op::Move { uids: vec![u[3]], up: false }).unwrap();
    assert_eq!(doc.p.tasks[3].uid, u[3]);
    op(&mut doc, Op::Set { uid: u[1], field: "preds".into(), value: "1".into() }).unwrap();
    let clip = copy(&doc.p, &[u[0], u[1]]);
    op(&mut doc, Op::Paste { at: 4, tasks: clip }).unwrap();
    assert_eq!(doc.p.tasks.len(), 6);
    assert_eq!(doc.p.tasks[5].preds[0].pred, doc.p.tasks[4].uid); // links inside the copy follow it
    op(&mut doc, Op::Set { uid: u[2], field: "finish".into(), value: "2026-11-06".into() }).unwrap();
    assert_eq!(doc.p.tasks[2].dur, 5 * D);
    op(&mut doc, Op::Delete { uids: vec![u[0]] }).unwrap();
    assert!(doc.p.tasks.iter().all(|t| t.preds.iter().all(|l| l.pred != u[0])));
}

#[test]
fn resources_costs_and_baseline() {
    let mut doc = Doc::new(plan(&[(2, ""), (3, "")]));
    let u: Vec<u32> = doc.p.tasks.iter().map(|t| t.uid).collect();
    op(&mut doc, Op::ResInsert { name: "Concrete".into() }).unwrap();
    let m = doc.p.resources[0].uid;
    op(&mut doc, Op::ResSet { uid: m, field: "kind".into(), value: "Material".into() }).unwrap();
    op(&mut doc, Op::ResSet { uid: m, field: "rate".into(), value: "50".into() }).unwrap();
    op(&mut doc, Op::Set { uid: u[0], field: "res".into(), value: "Engineer[50%], Concrete[12]".into() }).unwrap();
    op(&mut doc, Op::Set { uid: u[1], field: "res".into(), value: "Engineer".into() }).unwrap();
    let e = doc.p.resources[1].uid;
    op(&mut doc, Op::ResSet { uid: e, field: "rate".into(), value: "100".into() }).unwrap();
    let s = schedule(&doc.p).unwrap();
    let l = load::compute(&doc.p, &s);
    assert_eq!(l.work[0], 8.0 * H as f64); // 2 days at 50%
    assert_eq!(l.cost[0], 800.0 + 600.0);
    assert!(l.over[1]); // both tasks run on the same days
    assert_eq!(parse::fmt_res(&doc.p.tasks[0].res, &doc.p), "Engineer[50%], Concrete[12]");
    op(&mut doc, Op::Baseline { uids: vec![], clear: false, n: 0 }).unwrap();
    op(&mut doc, Op::Set { uid: u[0], field: "dur".into(), value: "4d".into() }).unwrap();
    let c = Calc::new(&doc.p, "%d-%b-%y", d(2026, 11, 1));
    assert_eq!(c.text(0, "vfinish"), "2 days");
    assert_eq!(c.text(0, "cost"), "$2,200.00");
    let w = load::workload(&doc.p, &schedule(&doc.p).unwrap(), true);
    assert_eq!(w.rows[0].load[0], 16.0 + 24.0);
}

#[test]
fn calendars_and_project_settings() {
    let mut doc = Doc::new(plan(&[(1, "")]));
    let mut c = Calendar::standard("Site");
    c.week[5] = vec![(480, 720)];
    op(&mut doc, Op::CalPut { old: None, cal: c.clone() }).unwrap();
    let u0 = doc.p.tasks[0].uid;
    op(&mut doc, Op::Set { uid: u0, field: "cal".into(), value: "Site".into() }).unwrap();
    c.name = "Site 6-day".into();
    op(&mut doc, Op::CalPut { old: Some("Site".into()), cal: c }).unwrap();
    assert_eq!(doc.p.tasks[0].cal.as_deref(), Some("Site 6-day"));
    assert!(op(&mut doc, Op::CalDelete { name: "Standard".into() }).is_err());
    op(&mut doc, Op::Project { name: "P".into(), start: "2026-11-02".into(), status: "".into(), calendar: "Site 6-day".into(), day_hours: 10.0, manual_default: true }).unwrap();
    assert_eq!(doc.p.day_min, 600);
    op(&mut doc, Op::Insert { at: 1, name: "M".into(), ms: false }).unwrap();
    assert!(doc.p.tasks[1].manual);
}

#[test]
fn quick_add_and_rows() {
    let mut doc = Doc::new(plan(&[(1, "")]));
    op(&mut doc, Op::QuickAdd { at: 1, text: "Pour slab 5d after 1 @Site Crew[50%] by 20-Nov-26".into() }).unwrap();
    let t = &doc.p.tasks[1];
    assert_eq!((t.name.as_str(), t.dur, t.preds.len(), t.deadline), ("Pour slab", 5 * D, 1, Some(d(2026, 11, 20))));
    assert_eq!(doc.p.resources[0].name, "Site Crew");
    let rows = vec![vec![("name".into(), "A".into()), ("dur".into(), "3h".into())], vec![("name".into(), "B".into()), ("preds".into(), "1, 2".into())]];
    op(&mut doc, Op::InsertRows { at: 1, rows }).unwrap();
    assert_eq!(doc.p.tasks[1].dur, 3 * H);
    // IDs typed with new rows are the ones shown before the insert: 2 was "Pour slab", now row 4.
    assert_eq!(doc.p.tasks[2].preds.iter().map(|l| l.pred).collect::<Vec<_>>(), vec![doc.p.tasks[0].uid, doc.p.tasks[3].uid]);
}

#[test]
fn parsing() {
    assert_eq!(parse::dur("3", D), Ok((3 * D, Unit::D)));
    assert_eq!(parse::dur("2 wks", D), Ok((10 * D, Unit::W)));
    assert_eq!(parse::dur("4h", D), Ok((4 * H, Unit::H)));
    assert!(parse::dur("x", D).is_err());
    assert_eq!(parse::fmt_dur(12 * H, Unit::D, D), "1.5 days");
    assert_eq!(parse::date("24-Oct-26"), Ok(d(2026, 10, 24)));
    assert_eq!(parse::date_in("10/24/2026", "%m/%d/%Y"), Ok(d(2026, 10, 24)));
    let ls = parse::preds("2SS+3d, 1; 3ff-4h", &[10, 20, 30], D).unwrap();
    assert_eq!((ls[0].pred, ls[0].kind, ls[0].lag), (20, LinkType::SS, 3 * D));
    assert_eq!((ls[2].kind, ls[2].lag), (LinkType::FF, -4 * H));
    assert!(parse::preds("9", &[1], D).is_err());
    let ids = [(10, 1), (20, 2), (30, 3)].into_iter().collect();
    assert_eq!(parse::fmt_preds(&ls, &ids, D), "2SS+3d, 1, 3FF-4h");
    assert_eq!(parse::constraint("start no earlier than"), Ok(Constraint::SNET));
    let eur = Currency { code: "EUR".into(), symbol: "€".into(), decimals: 2, after: true };
    assert_eq!(fields::money(1234567.5, &eur), "1,234,567.50 €");
    assert_eq!(fields::money(-12.0, &Currency { decimals: 0, ..Currency::default() }), "-$12");
}

#[test]
fn templates_and_old_files() {
    for t in crate::templates::TEMPLATES {
        let p = crate::templates::build(t, d(2026, 11, 2));
        assert!(Calc::new(&p, "%d-%b-%y", d(2026, 11, 1)).error.is_none(), "{}", t.id);
        let back = Project::from_json(&serde_json::to_string(&p).unwrap()).unwrap();
        assert_eq!(back, p);
    }
    let v1 = r#"{"format":1,"name":"Old","start":"2026-11-02","calendar":{"name":"Standard","week":[true,true,true,true,true,false,false],"holidays":["2026-11-03"]},
        "tasks":[{"uid":1,"name":"A","dur":2,"snet":"2026-11-02"},{"uid":2,"name":"B","dur":1,"preds":[{"pred":1,"kind":"FS","lag":1}]}],"next_uid":3}"#;
    let p = Project::from_json(v1).map_err(|e| e.to_string()).unwrap();
    assert_eq!((p.tasks[0].dur, p.tasks[0].constraint, p.tasks[1].preds[0].lag), (2 * D, Constraint::SNET, D));
    let s = schedule(&p).unwrap();
    assert_eq!(when(s.ef[0]), "04 17:00"); // the holiday carried over
}

fn rich() -> Project {
    let mut p = crate::templates::build(&crate::templates::TEMPLATES[0], d(2026, 11, 2));
    p.calendars[0].exceptions.push(Exception { name: "Holiday".into(), from: d(2026, 12, 24), to: d(2026, 12, 25), hours: vec![] });
    p.calendars.push(Calendar::full("24 Hours"));
    p.tasks[16].cal = Some("24 Hours".into());
    p.tasks[2].constraint = Constraint::SNET;
    p.tasks[2].cdate = Some(d(2026, 11, 5));
    p.tasks[1].pct = 100;
    p.tasks[1].act_start = Some(d(2026, 11, 2));
    p.tasks[1].act_finish = Some(d(2026, 11, 3));
    let mut doc = Doc::new(p);
    let u = doc.p.tasks[9].uid;
    op(&mut doc, Op::Set { uid: u, field: "res".into(), value: "CAD Designer, Design Engineer[50%]".into() }).unwrap();
    doc.p
}

fn same_schedule(a: &Project, b: &Project) {
    let (sa, sb) = (schedule(a).unwrap(), schedule(b).unwrap());
    assert_eq!(a.tasks.len(), b.tasks.len());
    assert_eq!(when(sa.finish), when(sb.finish));
    assert_eq!(a.tasks.iter().map(|t| t.preds.len()).sum::<usize>(), b.tasks.iter().map(|t| t.preds.len()).sum::<usize>());
    assert_eq!(a.resources.len(), b.resources.len());
}

#[test]
fn exchange_round_trips() {
    let p = rich();
    let dir = std::env::temp_dir().join(format!("datum-io-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    for ext in ["xml", "xer"] {
        let f = dir.join(format!("plan.{ext}"));
        crate::io::export(&p, &f, "%d-%b-%y", d(2026, 11, 1), &[]).unwrap();
        let crate::io::Imported::Plan(q) = crate::io::import(&f).unwrap() else { panic!("{ext} should import as a plan") };
        same_schedule(&p, &q);
        assert_eq!(q.tasks[1].act_finish, Some(d(2026, 11, 3)), "{ext}");
        assert!(q.calendars.iter().any(|c| c.exceptions.iter().any(|e| e.from == d(2026, 12, 24))), "{ext}");
    }
    for ext in ["csv", "xlsx"] {
        let f = dir.join(format!("plan.{ext}"));
        crate::io::export(&p, &f, "%d-%b-%y", d(2026, 11, 1), &[]).unwrap();
        let crate::io::Imported::Table(t) = crate::io::import(&f).unwrap() else { panic!("{ext} should import as a table") };
        assert!(t.guess.contains(&"name".to_string()) && t.guess.contains(&"preds".to_string()), "{ext}: {:?}", t.guess);
        let (q, bad) = crate::io::table::to_project(&t, &t.guess, "Q", d(2026, 11, 2), "%d-%b-%y").unwrap();
        assert_eq!(bad, 0, "{ext}");
        assert_eq!(q.tasks.len(), p.tasks.len());
        assert!(q.is_summary(0) && q.tasks[3].dur == 0);
        assert_eq!(q.tasks[10].preds[0].kind, LinkType::SS);
    }
    // Codes as IDs, and headers that look alike.
    let f = dir.join("codes.csv");
    std::fs::write(&f, "Activity,Description,Days,Depends on,Who\nA1,Survey,3,,Surveyor\nA2,Design,5,A1,Engineer\nA10,Build,8,\"A2SS+2d, A1\",Crew[200%]\n").unwrap();
    let crate::io::Imported::Table(t) = crate::io::import(&f).unwrap() else { panic!() };
    assert_eq!(t.guess, ["id", "name", "dur", "preds", "res"]);
    let (q, bad) = crate::io::table::to_project(&t, &t.guess, "Q", d(2026, 11, 2), "%d-%b-%y").unwrap();
    assert_eq!((bad, q.tasks[2].preds.len(), q.tasks[2].preds[0].kind), (0, 2, LinkType::SS));
    std::fs::remove_dir_all(dir).ok();
}

// ---------- 0.3 engine ----------

#[test]
fn elapsed_pct_lags_splits_and_from_finish() {
    let mut p = plan(&[(10, ""), (2, "1SS+50%"), (2, "")]);
    let (m, _, e) = parse::dur_any("2ed", D).unwrap();
    (p.tasks[2].dur, p.tasks[2].elapsed) = (m, e);
    let s = schedule(&p).unwrap();
    assert_eq!(when(s.es[1]), "09 08:00"); // half of task 1's ten days later
    assert_eq!(when(s.ef[2]), "04 08:00"); // two days around the clock
    p.tasks[1].splits = vec![(D, D)];
    let s = schedule(&p).unwrap();
    assert_eq!(when(s.ef[1]), "11 17:00"); // a one-day pause inside the work
    assert_eq!(parse::fmt_preds(&p.tasks[1].preds, &[(p.tasks[0].uid, 1)].into_iter().collect(), D), "1SS+50%");
    p.opts.from_finish = true;
    p.opts.finish_date = Some(d(2026, 11, 30));
    let s = schedule(&p).unwrap();
    assert_eq!(when(s.ef[0]), "30 17:00");
}

#[test]
fn level_of_effort_progress_logic_and_longest_path() {
    let mut p = plan(&[(5, ""), (3, "1"), (0, "1SS"), (2, "")]);
    p.tasks[3].preds = parse::preds("2FF", &p.tasks.iter().map(|t| t.uid).collect::<Vec<_>>(), D).unwrap();
    p.tasks[2].act_type = ActType::Loe;
    p.tasks[2].preds = vec![Link { pred: p.tasks[0].uid, kind: LinkType::SS, lag: 0, pct: false }];
    p.tasks[3].preds.clear();
    let ids: Vec<u32> = p.tasks.iter().map(|t| t.uid).collect();
    p.tasks[2].dur = 0;
    p.tasks.push(Task { uid: 99, name: "after LOE".into(), dur: D, preds: parse::preds("3", &ids, D).unwrap(), ..Default::default() });
    let s = schedule(&p).unwrap();
    assert_eq!(when(s.es[2]), "02 08:00");
    assert_eq!(when(s.ef[2]), "02 08:00"); // a level of effort only reaches its successors' needs…
    assert_eq!(when(s.es[4]), "02 08:00"); // …and never drives them
    assert!(s.lp[0] && s.lp[1] && !s.lp[3]);

    // Out-of-sequence progress: task 2 started before task 1 finished.
    let mut p = plan(&[(4, ""), (4, "1")]);
    p.status = Some(d(2026, 11, 3));
    p.tasks[1].act_start = Some(d(2026, 11, 2));
    p.tasks[1].pct = 50;
    assert_eq!(when(schedule(&p).unwrap().ef[1]), "11 17:00"); // retained logic waits for task 1 (which cannot start before the status date)
    p.opts.retained = false;
    assert_eq!(when(schedule(&p).unwrap().ef[1]), "05 17:00"); // progress override does not
}

#[test]
fn leveling_and_task_types() {
    let mut doc = Doc::new(plan(&[(2, ""), (3, ""), (1, "")]));
    let u: Vec<u32> = doc.p.tasks.iter().map(|t| t.uid).collect();
    for &uid in &u[..2] {
        op(&mut doc, Op::Set { uid, field: "res".into(), value: "Welder".into() }).unwrap();
    }
    op(&mut doc, Op::Level { uids: vec![], clear: false }).unwrap();
    let s = schedule(&doc.p).unwrap();
    assert!(!load::compute(&doc.p, &s).over.iter().any(|&o| o));
    assert!(s.es[1] >= s.ef[0] || s.es[0] >= s.ef[1]);
    op(&mut doc, Op::Level { uids: vec![], clear: true }).unwrap();
    assert!(doc.p.tasks.iter().all(|t| t.level_delay == 0));

    // Fixed work: two people halve the duration; fixed duration keeps it.
    op(&mut doc, Op::Set { uid: u[1], field: "type".into(), value: "Fixed Work".into() }).unwrap();
    op(&mut doc, Op::Set { uid: u[1], field: "res".into(), value: "Welder, Fitter".into() }).unwrap();
    assert_eq!(doc.p.tasks[1].dur, 3 * D / 2);
    op(&mut doc, Op::Set { uid: u[0], field: "res".into(), value: "Welder, Fitter".into() }).unwrap();
    assert_eq!(doc.p.tasks[0].dur, 2 * D);
    op(&mut doc, Op::Set { uid: u[0], field: "work".into(), value: "8".into() }).unwrap();
    assert_eq!(doc.p.tasks[0].res[0].units, 0.25); // 8h over 2 days with two people
    // Custom fields, codes, risks and steps.
    op(&mut doc, Op::Set { uid: u[2], field: "c:number1".into(), value: "12.5".into() }).unwrap();
    assert_eq!(doc.p.tasks[2].custom["number1"], serde_json::json!(12.5));
    op(&mut doc, Op::CodeDef { old: None, def: Some(CodeDef { name: "Discipline".into(), scope: String::new(), values: vec![CodeValue { value: "MECH".into(), ..Default::default() }] }) }).unwrap();
    assert!(op(&mut doc, Op::Set { uid: u[2], field: "code:Discipline".into(), value: "CIVIL".into() }).is_err());
    op(&mut doc, Op::Set { uid: u[2], field: "code:Discipline".into(), value: "MECH".into() }).unwrap();
    op(&mut doc, Op::TaskList { uid: u[2], field: "steps".into(), value: serde_json::json!([{"name": "a", "weight": 3, "done": true}, {"name": "b", "weight": 1}]) }).unwrap();
    assert_eq!(doc.p.tasks[2].phys, 75);
    op(&mut doc, Op::Risk { id: 0, risk: Some(Risk { name: "Late steel".into(), prob: 3, impact: 4, ..Default::default() }) }).unwrap();
    assert_eq!(doc.p.risks[0].id, 1);
    assert!(doc.p.tasks.iter().all(|t| t.code.starts_with('A') || t.code.is_empty()));
}

struct Row1(std::collections::HashMap<&'static str, crate::formula::F>);
impl crate::formula::Ctx for Row1 {
    fn field(&self, n: &str) -> Option<crate::formula::F> { self.0.get(n.to_lowercase().as_str()).cloned() }
    fn now(&self) -> i64 { at(d(2026, 11, 2)) }
    fn work_between(&self, a: i64, b: i64) -> i64 { WorkCal::new(&Calendar::standard("S")).between(a, b) }
    fn work_add(&self, a: i64, m: i64) -> i64 { WorkCal::new(&Calendar::standard("S")).add(a, m) }
}

#[test]
fn formulas() {
    use crate::formula::{eval, parse as fp, F};
    let row = Row1([("duration", F::Num(960.0)), ("finish", F::Date(at(d(2026, 11, 6)))), ("baseline finish", F::Date(at(d(2026, 11, 4)))), ("name", F::Str("Pump".into()))].into_iter().collect());
    let ev = |s: &str| eval(&fp(s).unwrap(), &row).unwrap();
    assert_eq!(ev("[Duration] / 480"), F::Num(2.0));
    assert_eq!(ev(r#"IIf([Finish] > [Baseline Finish], "Late", "On time")"#), F::Str("Late".into()));
    assert_eq!(ev("[Finish] - [Baseline Finish]"), F::Num(2.0));
    assert_eq!(ev("ProjDateDiff([Baseline Finish], [Finish]) / 60"), F::Num(16.0));
    assert_eq!(ev(r#"Switch([Duration] > 1000, "Long", [Duration] > 100, "Medium")"#), F::Str("Medium".into()));
    assert_eq!(ev(r#"UCase(Left([Name], 2)) & "-" & 7 Mod 4"#), F::Str("PU-3".into()));
    assert_eq!(ev("Not (1 > 2) And 3 >= 3"), F::Bool(true));
    assert!(fp("IIf(1, 2").is_err() && fp("Foo(1)").is_err());
    assert!(eval(&fp("1 / 0").unwrap(), &row).is_err());
}

#[test]
fn fields_earned_value_queries_and_checks() {
    use crate::query::{Filter, Query, Rule};
    let mut doc = Doc::new(plan(&[(0, ""), (2, ""), (4, "2"), (0, "3")]));
    let u: Vec<u32> = doc.p.tasks.iter().map(|t| t.uid).collect();
    op(&mut doc, Op::Indent { uids: vec![u[1], u[2]] }).unwrap();
    op(&mut doc, Op::Set { uid: u[0], field: "name".into(), value: "Phase".into() }).unwrap();
    op(&mut doc, Op::Set { uid: u[1], field: "fixedcost".into(), value: "1000".into() }).unwrap();
    op(&mut doc, Op::Set { uid: u[2], field: "fixedcost".into(), value: "2000".into() }).unwrap();
    op(&mut doc, Op::Baseline { uids: vec![], clear: false, n: 0 }).unwrap();
    op(&mut doc, Op::Set { uid: u[1], field: "pct".into(), value: "100".into() }).unwrap();
    op(&mut doc, Op::Set { uid: u[2], field: "pct".into(), value: "25".into() }).unwrap();
    // Status at the end of Wednesday: task 2 (Mon–Tue) done, task 3 one day in of four.
    op(&mut doc, Op::ProjectSet { field: "status".into(), value: serde_json::json!("2026-11-04") }).unwrap();
    let def = CustomDef { key: "number1".into(), alias: "Double cost".into(), formula: "[Cost] * 2".into(), ..CustomDef::default() };
    op(&mut doc, Op::FieldDef { def }).unwrap();
    let c = Calc::new(&doc.p, "%d-%b-%y", d(2026, 11, 1));
    assert_eq!((c.val(1, "bcws").num(), c.val(2, "bcws").num()), (1000.0, 500.0));
    assert_eq!((c.val(0, "bcwp").num(), c.val(0, "bac").num()), (1500.0, 3000.0));
    assert_eq!(c.text(0, "spi"), "1");
    assert_eq!(c.text(1, "c:number1"), "2000");
    assert_eq!(c.key_of("Double cost").as_deref(), Some("c:number1"));
    assert_eq!(c.text(0, "pct"), "50%"); // weighted by duration: (2×100 + 4×25) / 6
    // Filters keep the summary of matching tasks; sorting keeps the outline.
    let q = |f: Filter, sort: Vec<(String, bool)>| c.grid(&Query { cols: vec!["name".into(), "Duration".into()], filter: f, sort, ..Query::default() });
    let crit = Filter { rules: vec![Rule { field: "Milestone".into(), test: "equals".into(), value: "Yes".into(), ..Rule::default() }], summaries: true, ..Filter::default() };
    let g = q(crit, vec![]);
    assert_eq!(g.rows.iter().map(|r| r.cells[0].as_str()).collect::<Vec<_>>(), ["T4"]);
    let g = q(Filter::default(), vec![("dur".into(), true)]);
    assert_eq!(g.rows.iter().map(|r| r.cells[0].as_str()).collect::<Vec<_>>(), ["Phase", "T3", "T2", "T4"]);
    assert_eq!(g.cols[1].name, "Duration");
    let grouped = c.grid(&Query { cols: vec!["name".into()], group: vec!["milestone".into()], ..Query::default() });
    assert_eq!(grouped.rows.iter().map(|r| (r.kind, r.cells[0].as_str())).collect::<Vec<_>>(), [("group", "Milestone: No"), ("task", "T2"), ("task", "T3"), ("group", "Milestone: Yes"), ("task", "T4")]);
    let checks = crate::check::run(&c, &Default::default());
    let logic = checks.iter().find(|k| k.key == "logic").unwrap();
    assert_eq!((logic.count, logic.pass), (0, true));
    assert!(checks.iter().find(|k| k.key == "resources").is_some_and(|k| k.count == 0 && k.pass));
    let net = crate::network::layout(&c, &[1, 2, 3]);
    assert_eq!(net.nodes.iter().map(|n| n.col).collect::<Vec<_>>(), [0, 1, 2]);
    let curve = crate::report::curve(&c, "week");
    assert_eq!(curve.planned.iter().sum::<f64>().round(), 3000.0);
    assert_eq!(c.summary().tasks, 3);
    let html = crate::io::formats::html(&c, &["name".into(), "cost".into()]);
    assert!(html.contains("$3,000.00") && html.contains("T3"));
    assert!(crate::io::formats::pmxml(&c).contains("<Relationship>"));
}

#[test]
fn excel_formula_columns_sheets_and_periods() {
    let mut doc = Doc::new(plan(&[(5, ""), (5, "1"), (0, "2")]));
    let u: Vec<u32> = doc.p.tasks.iter().map(|t| t.uid).collect();
    op(&mut doc, Op::Baseline { uids: vec![], clear: false, n: 0 }).unwrap();
    op(&mut doc, Op::Cells { cells: vec![(u[0], "pct".into(), "100".into()), (u[1], "pct".into(), "40".into()), (u[1], "dur".into(), "oops".into())] }).unwrap();
    let def = |key: &str, alias: &str, f: &str| Op::FieldDef { def: CustomDef { key: key.into(), alias: alias.into(), formula: f.into(), ..CustomDef::default() } };
    op(&mut doc, def("number1", "Days left", "=[Duration]*(100-[% Complete])/100")).unwrap();
    op(&mut doc, def("text1", "Health", r#"=IF([Days left]>2,"Watch","OK")"#)).unwrap();
    op(&mut doc, def("date1", "Due", "=WORKDAY([Finish],2)")).unwrap();
    assert!(op(&mut doc, def("number2", "", "=[No such field]+1")).is_err());
    op(&mut doc, Op::Title { key: "dur".into(), title: "Days".into() }).unwrap();
    let c = Calc::new(&doc.p, "%d-%b-%y", d(2026, 11, 1));
    assert_eq!((c.text(0, "c:number1"), c.text(1, "c:number1")), ("0".to_string(), "3".to_string()));
    assert_eq!((c.text(0, "c:text1"), c.text(1, "c:text1")), ("OK".to_string(), "Watch".to_string()));
    assert_eq!(c.text(0, "c:date1"), "10-Nov-26"); // Fri 6 Nov + 2 working days
    assert_eq!(c.name("dur"), "Days");
    op(&mut doc, def("number3", "Progress variance", "=[% Complete]-[Baseline % Complete]")).unwrap();
    let c = Calc::new(&doc.p, "%d-%b-%y", d(2026, 11, 1));
    assert_eq!(c.text(0, "c:number3"), "100"); // done, but the baseline says not started on 1 Nov
    // A spreadsheet tab reading the Plan sheet.
    let sheet = SheetData { name: "Calc".into(), ..Default::default() };
    op(&mut doc, Op::Sheet { k: None, sheet: Some(sheet) }).unwrap();
    op(&mut doc, Op::SheetCells { k: 0, cells: vec![("A1".into(), "=SUM(Plan!F2:F4)".into()), ("b1".into(), "=A1*2".into()), ("C1".into(), "hello".into())], fmt: None }).unwrap();
    let c = Calc::new(&doc.p, "%d-%b-%y", d(2026, 11, 1));
    let v = crate::sheet::sheet_view(&c, 0).unwrap();
    assert_eq!(v.cells.iter().map(|x| x.2.as_str()).collect::<Vec<_>>(), ["10", "20", "hello"]);
    // Weekly periods end on Fridays; the S-curve reaches 100% planned.
    op(&mut doc, Op::ProjectSet { field: "status".into(), value: serde_json::json!("2026-11-10") }).unwrap();
    let c = Calc::new(&doc.p, "%d-%b-%y", d(2026, 11, 1));
    let ends = crate::report::period_ends(&c, "week");
    assert_eq!(date_of(ends[0] * DAY), d(2026, 11, 6));
    let sc = crate::report::scurve(&c, "week", "duration", None);
    assert!((sc.planned.last().unwrap() - 100.0).abs() < 1e-6);
    let r = crate::report::period_report(&c, "week", None);
    assert_eq!(r.to, ends[1]);
    assert!(r.actual > 50.0 && r.actual < 80.0, "{}", r.actual);
    assert_eq!(r.milestones.len(), 1);
}

#[test]
fn recurring_tasks_and_excel_export_with_formulas() {
    let mut doc = Doc::new(plan(&[(2, "")]));
    op(&mut doc, Op::Recurring { at: 1, name: "Site meeting".into(), dur: "2h".into(), every: "week".into(), interval: 1, count: 4, start: "2026-11-04".into() }).unwrap();
    let p = &doc.p;
    assert_eq!(p.tasks.len(), 6);
    assert_eq!((p.tasks[1].name.as_str(), p.tasks[5].name.as_str(), p.tasks[5].level), ("Site meeting", "Site meeting 4", 1));
    let s = schedule(p).unwrap();
    assert_eq!(date_of(s.es[5]), d(2026, 11, 25));
    let codes: std::collections::HashSet<&str> = p.tasks.iter().map(|t| t.code.as_str()).collect();
    assert_eq!(codes.len(), 6, "Activity IDs stay unique");
    let def = CustomDef { key: "number1".into(), alias: "Double".into(), formula: "=[Duration]*2".into(), ..CustomDef::default() };
    op(&mut doc, Op::FieldDef { def }).unwrap();
    op(&mut doc, Op::Sheet { k: None, sheet: Some(SheetData { name: "Calc".into(), ..Default::default() }) }).unwrap();
    op(&mut doc, Op::SheetCells { k: 0, cells: vec![("A1".into(), "=SUM(Plan!F2:F7)".into())], fmt: None }).unwrap();
    let f = std::env::temp_dir().join(format!("dp-fx-{}.xlsx", std::process::id()));
    crate::io::export(&doc.p, &f, "%d-%b-%y", d(2026, 11, 1), &["name".into(), "dur".into(), "c:number1".into()]).unwrap();
    let bytes = std::fs::read(&f).unwrap();
    let _ = std::fs::remove_file(&f);
    let mut formulas = Vec::new();
    // The sheet XML is deflated inside the zip; check through calamine that the formula is there.
    use calamine::{open_workbook_auto, Reader};
    let tmp = std::env::temp_dir().join(format!("dp-fx2-{}.xlsx", std::process::id()));
    std::fs::write(&tmp, &bytes).unwrap();
    let mut wb = open_workbook_auto(&tmp).unwrap();
    let fx = wb.worksheet_formula("Tasks").unwrap();
    formulas.extend(fx.used_cells().map(|(_, _, f)| f.clone()));
    assert!(formulas.iter().any(|f| f.contains("B2*2")), "{formulas:?}");
    assert!(wb.sheet_names().contains(&"Plan".to_string()) && wb.sheet_names().contains(&"Calc".to_string()));
    let _ = std::fs::remove_file(&tmp);
}

#[test]
fn text_formatting_on_tasks_and_sheets() {
    let mut doc = Doc::new(plan(&[(2, ""), (3, "1")]));
    let u = doc.p.tasks[0].uid;
    op(&mut doc, Op::Style { cells: vec![(u, "name".into(), "b;c:#C0392B".into()), (u, "*".into(), "f:#FFF2CC".into())] }).unwrap();
    let c = Calc::new(&doc.p, "%d-%b-%y", d(2026, 11, 2));
    let g = c.grid(&crate::query::Query { cols: vec!["name".into()], ..Default::default() });
    assert_eq!(g.rows[0].style.get("name").map(String::as_str), Some("b;c:#C0392B"));
    assert_eq!(g.rows[0].style.get("*").map(String::as_str), Some("f:#FFF2CC"));
    op(&mut doc, Op::Style { cells: vec![(u, "name".into(), String::new()), (u, "*".into(), String::new())] }).unwrap();
    assert!(doc.p.tasks[0].style.is_null(), "clearing the last style leaves nothing in the file");
    op(&mut doc, Op::Sheet { k: None, sheet: Some(SheetData { name: "Q".into(), ..Default::default() }) }).unwrap();
    op(&mut doc, Op::SheetStyle { k: 0, cells: vec![("b2".into(), "i;a:r".into())] }).unwrap();
    assert_eq!(doc.p.sheets[0].style.get("B2").map(String::as_str), Some("i;a:r"));
}

#[test]
fn delete_custom_field_removes_values() {
    let mut doc = Doc::new(plan(&[(2, "")]));
    let u = doc.p.tasks[0].uid;
    op(&mut doc, Op::FieldDef { def: CustomDef { key: "text1".into(), alias: "Area".into(), ..CustomDef::default() } }).unwrap();
    op(&mut doc, Op::Cells { cells: vec![(u, "c:text1".into(), "North".into())] }).unwrap();
    assert!(doc.p.tasks[0].custom.contains_key("text1"));
    op(&mut doc, Op::DeleteField { key: "text1".into() }).unwrap();
    assert!(doc.p.fields.is_empty() && doc.p.tasks[0].custom.is_empty());
}
