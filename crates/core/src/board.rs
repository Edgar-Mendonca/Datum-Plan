//! The Task Board: the plan's tasks (or the Document Register's documents, or the Procurement
//! Register's orders) as cards in columns. Tasks go by status (Not started, In progress, Blocked,
//! Done) or by any field, in swimlanes by another; each card shows the critical edge, float,
//! progress, steps, people and the documents and orders the task waits for. Moving a card is one
//! change: a task's actual start or finish (or the field its column stands for), a document's next
//! submission or return, an order's milestones, or a block with its reason.
use crate::{cal::{date_of, DAY}, doc::Op, fields::Calc, model::*};
use chrono::{Days, NaiveDate};
use serde::Serialize;

/// What the task status columns are: key and name.
pub const TASK_COLS: [(&str, &str); 4] = [("todo", "Not started"), ("doing", "In progress"), ("blocked", "Blocked"), ("done", "Done")];
pub const DOC_COLS: [(&str, &str); 5] = [("todo", "To issue"), ("review", "With the client"), ("resubmit", "To resubmit"), ("blocked", "Blocked"), ("done", "Final")];
pub const ORDER_COLS: [(&str, &str); 7] = [("m0", "Enquiry"), ("m1", "To order"), ("m2", "Ordered"), ("m3", "In manufacture"), ("m4", "To deliver"), ("blocked", "Blocked"), ("m5", "Delivered")];

#[derive(Serialize, Debug, Default, Clone, PartialEq)]
pub struct Wait {
    /// "document" or "order".
    pub kind: String,
    pub label: String,
    pub state: String,
    /// Late for the task that needs it.
    pub late: bool,
}

#[derive(Serialize, Debug, Default, Clone, PartialEq)]
pub struct Card {
    /// "t:uid", "d:id" or "o:id".
    pub key: String,
    pub id: u32,
    /// The task's ID, the document number or the item number.
    pub code: String,
    pub name: String,
    pub col: String,
    pub lane: String,
    pub crit: bool,
    pub float_days: Option<i64>,
    pub pct: f64,
    /// Steps done and in all.
    pub steps: (usize, usize),
    pub people: String,
    pub start: Option<NaiveDate>,
    pub finish: Option<NaiveDate>,
    /// Past its finish (a task), its due date (a document) or its next milestone (an order).
    pub late: bool,
    /// The reason it is set aside, or what it waits for when that is late.
    pub blocked: String,
    pub waits: Vec<Wait>,
    /// A line of status: a document's state, an order's next milestone.
    pub note: String,
}

#[derive(Serialize, Debug, Default, Clone, PartialEq)]
pub struct Column { pub key: String, pub name: String, pub count: usize, pub wip: u32, pub over: bool }

#[derive(Serialize, Debug, Default, Clone, PartialEq)]
pub struct BoardOut {
    pub kind: String,
    pub by: String,
    pub cols: Vec<Column>,
    pub lanes: Vec<String>,
    pub cards: Vec<Card>,
    /// The day the board is drawn for (the status date, or today).
    pub day: Option<NaiveDate>,
    /// Whether a card can be dropped in another column (a field the user can change).
    pub movable: bool,
}

/// The board as the plan's settings say (`p.board`), worked out for the day of the calculation.
pub fn board(c: &Calc) -> BoardOut {
    let (p, b) = (c.p, &c.p.board);
    let day = date_of(c.progress_at() - DAY);
    let mut o = match b.kind.as_str() { "documents" => docs(c, b), "orders" => orders(c, b), _ => tasks(c, b, day) };
    o.day = Some(day);
    // Lanes in the order they first appear, "(none)" last.
    let mut lanes: Vec<String> = vec![];
    for k in &o.cards { if !lanes.contains(&k.lane) { lanes.push(k.lane.clone()) } }
    lanes.sort_by_key(|l| l == "(none)");
    o.lanes = if b.lanes.is_empty() { vec![String::new()] } else { lanes };
    for col in &mut o.cols {
        col.count = o.cards.iter().filter(|k| k.col == col.key).count();
        col.wip = b.wip.get(&col.key).copied().unwrap_or(0);
        col.over = col.wip > 0 && col.count as u32 > col.wip;
    }
    let _ = p;
    o
}

/// A task's top-level summary (its phase).
fn phase(p: &Project, i: usize) -> String {
    (0..=i).rev().find(|&k| p.tasks[k].level == 0).filter(|&k| k != i || p.tasks.get(i + 1).is_some_and(|n| n.level > 0)).map_or(String::new(), |k| p.tasks[k].name.clone())
}

fn tasks(c: &Calc, b: &Board, day: NaiveDate) -> BoardOut {
    let p = c.p;
    let by = if b.by.is_empty() { "status" } else { b.by.as_str() };
    let field = |i: usize, key: &str| if key == "phase" { phase(p, i) } else { c.text(i, key) };
    let horizon = day.checked_add_days(Days::new(u64::from(b.weeks) * 7));
    // What each task waits for: vendor documents and orders the registers link to it.
    let dr = (!p.documents.docs.is_empty()).then(|| crate::docreg::status(c));
    let pr = (!p.procurement.items.is_empty()).then(|| crate::procure::status(c));
    let mut out = BoardOut { kind: "tasks".into(), by: by.into(), movable: by != "phase", ..Default::default() };
    let mut cols: Vec<String> = vec![];
    for i in 0..p.tasks.len() {
        if c.s.summary[i] || c.s.off[i] { continue }
        let t = &p.tasks[i];
        let pct = c.pctf.get(i).copied().unwrap_or(c.pct[i] as f64);
        let done = c.pct[i] >= 100;
        let start = t.act_start.unwrap_or_else(|| date_of(c.s.es[i]));
        let finish = t.act_finish.unwrap_or_else(|| date_of(c.s.ef[i] - 1));
        if let Some(h) = horizon.filter(|_| b.weeks > 0) {
            let keep = if done { t.act_finish.is_some_and(|f| f + Days::new(14) >= day) } else { start <= h || pct > 0.0 };
            if !keep { continue }
        }
        let mut waits = vec![];
        if let Some(o) = &dr {
            for (k, d) in p.documents.docs.iter().enumerate().filter(|(_, d)| d.task == t.uid) {
                let Some(x) = o.docs.get(k) else { continue };
                if x.skey == "off" || x.done { continue }
                waits.push(Wait { kind: "document".into(), label: if d.number.is_empty() { d.title.clone() } else { d.number.clone() }, state: x.status.clone(), late: x.late_for_task });
            }
        }
        if let Some(o) = &pr {
            for (k, it) in p.procurement.items.iter().enumerate().filter(|(_, it)| it.need == t.uid) {
                let Some(x) = o.items.get(k) else { continue };
                if x.skey == "off" || it.actual[4].is_some() { continue }
                waits.push(Wait { kind: "order".into(), label: if it.tag.is_empty() { it.name.clone() } else { it.tag.clone() }, state: x.next.clone(), late: x.rag == "red" });
            }
        }
        let key = format!("t:{}", t.uid);
        let mut blocked = b.blocked.get(&key).cloned().unwrap_or_default();
        if blocked.is_empty() && !done {
            if let Some(w) = waits.iter().find(|w| w.late) { blocked = format!("Waits for {} ({})", w.label, w.state) }
        }
        let col = if by == "status" {
            (if done { "done" } else if !blocked.is_empty() { "blocked" } else if pct > 0.0 || t.act_start.is_some() { "doing" } else { "todo" }).to_string()
        } else {
            let v = field(i, by);
            let v = if v.trim().is_empty() { "(none)".to_string() } else { v };
            if !cols.contains(&v) { cols.push(v.clone()) }
            v
        };
        let lane = if b.lanes.is_empty() { String::new() } else { let v = field(i, &b.lanes); if v.trim().is_empty() { "(none)".into() } else { v } };
        out.cards.push(Card {
            key, id: t.uid, code: if t.code.is_empty() { (i + 1).to_string() } else { t.code.clone() }, name: t.name.clone(), col, lane,
            crit: c.s.critical(p, i), float_days: (!done).then(|| c.s.tf[i].div_euclid(p.day_min.max(1))), pct,
            steps: (t.steps.iter().filter(|s| s.done).count(), t.steps.len()), people: c.text(i, "res"),
            start: Some(start), finish: Some(finish), late: !done && finish < day, blocked, waits, note: String::new(),
        });
    }
    out.cols = if by == "status" { TASK_COLS.iter().map(|(k, n)| Column { key: (*k).into(), name: (*n).into(), ..Default::default() }).collect() } else {
        cols.sort_by_key(|v| (v == "(none)", v.to_lowercase()));
        cols.into_iter().map(|v| Column { key: v.clone(), name: v, ..Default::default() }).collect()
    };
    out
}

fn docs(c: &Calc, b: &Board) -> BoardOut {
    let (p, r) = (c.p, &c.p.documents);
    let o = crate::docreg::status(c);
    let mut out = BoardOut { kind: "documents".into(), by: "status".into(), movable: true, cols: DOC_COLS.iter().map(|(k, n)| Column { key: (*k).into(), name: (*n).into(), ..Default::default() }).collect(), ..Default::default() };
    for (k, d) in r.docs.iter().enumerate() {
        let Some(x) = o.docs.get(k) else { continue };
        if x.skey == "off" { continue }
        let key = format!("d:{}", d.id);
        let blocked = b.blocked.get(&key).cloned().unwrap_or_default();
        let col = if !blocked.is_empty() { "blocked" } else { match x.skey.as_str() { "review" | "review_late" => "review", "resubmit" => "resubmit", "final" => "done", _ => "todo" } };
        let lane = match b.lanes.as_str() { "vendor" => d.vendor.clone(), "package" => d.package.clone(), "disc" => d.disc.clone(), "kind" => d.kind.clone(), "" => String::new(), _ => String::new() };
        out.cards.push(Card {
            key, id: d.id, code: d.number.clone(), name: d.title.clone(), col: col.into(), lane: if !b.lanes.is_empty() && lane.trim().is_empty() { "(none)".into() } else { lane },
            pct: x.earned, people: d.vendor.clone(), start: x.sent, finish: x.due.or(x.planned.first().copied().flatten()),
            late: matches!(x.skey.as_str(), "not_sub_late" | "review_late") || x.late_for_task, blocked, note: format!("{}{}", x.status, if x.rev.is_empty() { String::new() } else { format!(" · rev {}", x.rev) }),
            ..Default::default()
        });
        if let Some(t) = p.idx(d.task) { out.cards.last_mut().unwrap().waits.push(Wait { kind: "task".into(), label: p.tasks[t].name.clone(), state: x.need.map(|n| n.to_string()).unwrap_or_default(), late: x.late_for_task }) }
    }
    out
}

fn orders(c: &Calc, b: &Board) -> BoardOut {
    let (p, r) = (c.p, &c.p.procurement);
    let o = crate::procure::status(c);
    let mut out = BoardOut { kind: "orders".into(), by: "status".into(), movable: true, cols: ORDER_COLS.iter().map(|(k, n)| Column { key: (*k).into(), name: (*n).into(), ..Default::default() }).collect(), ..Default::default() };
    for (k, it) in r.items.iter().enumerate() {
        let Some(x) = o.items.get(k) else { continue };
        if x.skey == "off" { continue }
        let key = format!("o:{}", it.id);
        let blocked = b.blocked.get(&key).cloned().unwrap_or_default();
        let next = it.actual.iter().position(|d| d.is_none()).unwrap_or(5);
        let col = if !blocked.is_empty() { "blocked".to_string() } else { format!("m{next}") };
        let lane = match b.lanes.as_str() { "vendor" => it.vendor.clone(), "origin" => it.origin.clone(), _ => String::new() };
        out.cards.push(Card {
            key, id: it.id, code: it.tag.clone(), name: it.name.clone(), col, lane: if !b.lanes.is_empty() && lane.trim().is_empty() { "(none)".into() } else { lane },
            crit: x.rag == "red", float_days: x.float, pct: x.earned, people: it.vendor.clone(), finish: x.forecast, late: x.days_late > 0, blocked,
            note: if x.next.is_empty() { x.status.clone() } else { format!("{}{}", x.next, if x.days_late > 0 { format!(" · {} d late", x.days_late) } else { String::new() }) },
            ..Default::default()
        });
        if let Some(t) = p.idx(it.need) { out.cards.last_mut().unwrap().waits.push(Wait { kind: "task".into(), label: p.tasks[t].name.clone(), state: x.required.map(|d| d.to_string()).unwrap_or_default(), late: x.rag == "red" }) }
    }
    out
}

/// Tidies the board's settings: known kinds, at most a year of look-ahead, short texts.
pub fn clean(b: &mut Board) {
    if !["", "tasks", "documents", "orders"].contains(&b.kind.as_str()) { b.kind.clear() }
    b.weeks = b.weeks.min(52);
    b.wip.retain(|k, v| *v > 0 && k.len() < 200);
    b.blocked.retain(|k, v| (k.starts_with("t:") || k.starts_with("d:") || k.starts_with("o:")) && !v.trim().is_empty());
    for v in b.blocked.values_mut() { if v.chars().count() > 300 { *v = v.chars().take(300).collect() } }
    for s in [&mut b.by, &mut b.lanes] { if s.len() > 100 { s.clear() } }
}

/// The changes a card's move makes, as operations (applied together, one Undo step), and the
/// blocked cards after it.
pub fn move_ops(p: &Project, kind: &str, id: u32, to: &str, date: &str, reason: &str, fmt: &str) -> Result<(Vec<Op>, std::collections::BTreeMap<String, String>), String> {
    let day = NaiveDate::parse_from_str(date.trim(), "%Y-%m-%d").or_else(|_| crate::parse::date_in(date, fmt)).map_err(|_| format!("Not a date: {date}."))?;
    let pre = match kind { "documents" => "d", "orders" => "o", _ => "t" };
    let key = format!("{pre}:{id}");
    let mut blocked = p.board.blocked.clone();
    let was_blocked = blocked.remove(&key).is_some();
    if to == "blocked" {
        if reason.trim().is_empty() { return Err("Say why it is blocked.".into()) }
        blocked.insert(key, reason.trim().chars().take(300).collect());
        return Ok((vec![], blocked));
    }
    let mut ops = vec![];
    match kind {
        "documents" => {
            let mut r = p.documents.clone();
            let d = r.docs.iter().find(|d| d.id == id).cloned().ok_or("No document with that id.")?;
            let open = r.log.iter().rposition(|s| s.doc == id && s.sent.is_some() && s.returned.is_none());
            match to {
                "review" => {
                    if open.is_some() { if !was_blocked { return Err(format!("{} is with the client already.", d.number)) } } else {
                        let mut s = crate::docreg::next_sub(&r, &d, day);
                        s.transmittal = crate::docreg::next_transmittal(&r);
                        r.log.push(s);
                    }
                }
                "done" | "resubmit" => {
                    let k = open.ok_or(format!("{} is not with the client: send it first (move it to With the client).", d.number))?;
                    let want = to == "resubmit";
                    let code = r.codes.iter().find(|c| if want { c.resubmit } else { c.accepted && !c.resubmit }).map(|c| c.code.clone()).ok_or("The register has no return code for that: Settings › Codes.")?;
                    r.log[k].returned = Some(day);
                    r.log[k].code = code;
                }
                "todo" if was_blocked => {}
                "todo" => return Err("A submission is taken back in the Transmittal log.".into()),
                _ => return Err(format!("No column {to}.")),
            }
            if r != p.documents { ops.push(Op::Documents { r: Box::new(r) }) }
        }
        "orders" => {
            let mut r = p.procurement.clone();
            let it = r.items.iter_mut().find(|x| x.id == id).ok_or("No order with that id.")?;
            let j: usize = to.strip_prefix('m').and_then(|x| x.parse().ok()).filter(|j| *j <= 5).ok_or(format!("No column {to}."))?;
            let next = it.actual.iter().position(|d| d.is_none()).unwrap_or(5);
            if j < next && !was_blocked { return Err("Recorded milestones are cleared in the register.".into()) }
            for d in it.actual.iter_mut().take(j).skip(next) { *d = Some(day) }
            if j >= 4 { it.mfg_pct = 100.0 }
            if r != p.procurement { ops.push(Op::Procurement { r: Box::new(r) }) }
        }
        _ => {
            let i = p.idx(id).ok_or("No task with that id.")?;
            let t = &p.tasks[i];
            let d = day.format(fmt).to_string();
            let by = if p.board.by.is_empty() { "status" } else { p.board.by.as_str() };
            let set = |field: &str, value: &str| Op::Set { uid: id, field: field.into(), value: value.into() };
            if by != "status" {
                if by == "phase" { return Err("A task's phase is its place in the outline: indent or move the task instead.".into()) }
                ops.push(set(by, if to == "(none)" { "" } else { to }));
            } else {
                match to {
                    "todo" => {
                        if t.act_finish.is_some() { ops.push(set("afinish", "")) }
                        if t.act_start.is_some() || t.pct > 0 { ops.push(set("pct", "0")); ops.push(set("astart", "")) } }
                    "doing" => {
                        if t.act_finish.is_some() { ops.push(set("afinish", "")); ops.push(set("pct", "50")) }
                        if t.act_start.is_none() { ops.push(set("astart", &d)) }
                    }
                    "done" => { if t.act_start.is_none() { ops.push(set("astart", &d)) } ops.push(set("afinish", &d)); ops.push(set("pct", "100")) }
                    _ => return Err(format!("No column {to}.")),
                }
            }
        }
    }
    if ops.is_empty() && !was_blocked { return Err("Nothing to change.".into()) }
    Ok((ops, blocked))
}

#[cfg(test)]
mod t {
    use super::*;
    use crate::doc::Doc;
    const F: &str = "%Y-%m-%d";
    fn d(m: u32, dd: u32) -> NaiveDate { NaiveDate::from_ymd_opt(2026, m, dd).unwrap() }

    #[test]
    fn tasks_move_across_the_board_and_wait_for_documents_and_orders() {
        let mut doc = Doc::new(crate::templates::build(&crate::templates::TEMPLATES[0], d(11, 2)));
        let today = d(11, 20);
        let b = |doc: &Doc| board(&Calc::new(&doc.p, F, today));
        let o = b(&doc);
        assert_eq!(o.cols.iter().map(|c| c.key.as_str()).collect::<Vec<_>>(), ["todo", "doing", "blocked", "done"]);
        assert!(o.cards.iter().all(|k| k.col == "todo") && !o.cards.is_empty(), "nothing started yet");
        let uid = o.cards[0].id;
        let mv = |doc: &mut Doc, to: &str, reason: &str| doc.apply(Op::BoardMove { kind: "tasks".into(), id: uid, to: to.into(), date: "2026-11-03".into(), reason: reason.into() }, F);
        mv(&mut doc, "doing", "").unwrap();
        let k = |doc: &Doc| b(doc).cards.into_iter().find(|k| k.id == uid).unwrap();
        assert_eq!((k(&doc).col.as_str(), doc.p.tasks[doc.p.idx(uid).unwrap()].act_start), ("doing", Some(d(11, 3))), "dragged to In progress: started on the day");
        assert!(mv(&mut doc, "blocked", " ").is_err(), "a block needs its reason");
        mv(&mut doc, "blocked", "Waiting for scaffolding").unwrap();
        assert_eq!((k(&doc).col.as_str(), k(&doc).blocked.as_str()), ("blocked", "Waiting for scaffolding"));
        mv(&mut doc, "done", "").unwrap();
        let t = &doc.p.tasks[doc.p.idx(uid).unwrap()];
        assert_eq!((k(&doc).col.as_str(), t.act_finish, doc.p.board.blocked.len()), ("done", Some(d(11, 3)), 0), "done: finished on the day and no longer blocked");
        doc.undo();
        assert_eq!(k(&doc).col, "blocked", "one move is one Undo step");
        mv(&mut doc, "todo", "").unwrap();
        assert_eq!(k(&doc).col, "todo", "back to Not started: the actuals are cleared");
        // A task that waits for a late vendor document is blocked by it.
        let need = doc.p.tasks.iter().rev().find(|t| !doc.p.tasks.iter().any(|x| x.level > t.level)).map(|t| t.uid).unwrap();
        let mut r = doc.p.documents.clone();
        r.docs.push(VDoc { id: 1, number: "V-001".into(), title: "GA drawing".into(), class: 1, task: need, offset: Some(-30.0), weight: 1.0, progress: true, ..Default::default() });
        doc.apply(Op::Documents { r: Box::new(r) }, F).unwrap();
        let w = b(&doc).cards.into_iter().find(|k| k.id == need).unwrap();
        assert_eq!(w.waits.len(), 1);
        assert_eq!(w.waits[0].label, "V-001");
        // Columns by a field, lanes by phase, WIP limits.
        let mut s = doc.p.board.clone();
        s.lanes = "phase".into();
        s.wip.insert("todo".into(), 1);
        doc.apply(Op::Board { b: s }, F).unwrap();
        let o = b(&doc);
        assert!(o.lanes.len() > 1 && o.cols[0].over, "lanes by phase; the Not started column is over its limit of 1");
        let mut s = doc.p.board.clone();
        s.by = "responsible".into();
        doc.apply(Op::Board { b: s }, F).unwrap();
        doc.apply(Op::BoardMove { kind: "tasks".into(), id: uid, to: "Site team".into(), date: "2026-11-03".into(), reason: String::new() }, F).unwrap();
        let o = b(&doc);
        assert!(o.cols.iter().any(|c| c.key == "Site team") && o.cards.iter().find(|k| k.id == uid).unwrap().col == "Site team", "dropped in a column, the field takes its value");
    }

    #[test]
    fn documents_and_orders_move_on_the_same_board() {
        let mut doc = Doc::new(crate::templates::build(&crate::templates::TEMPLATES[0], d(11, 2)));
        let mut r = doc.p.documents.clone();
        r.docs.push(VDoc { id: 1, number: "V-001".into(), title: "GA".into(), class: 1, weight: 1.0, progress: true, ..Default::default() });
        doc.apply(Op::Documents { r: Box::new(r) }, F).unwrap();
        let mut p = doc.p.procurement.clone();
        p.items.push(PItem { id: 1, tag: "1.1".into(), name: "Pump".into(), manual_w: 1.0, ..Default::default() });
        doc.apply(Op::Procurement { r: Box::new(p) }, F).unwrap();
        let mv = |doc: &mut Doc, kind: &str, to: &str| doc.apply(Op::BoardMove { kind: kind.into(), id: 1, to: to.into(), date: "2026-11-10".into(), reason: String::new() }, F);
        let mut s = doc.p.board.clone(); s.kind = "documents".into(); doc.apply(Op::Board { b: s }, F).unwrap();
        let col = |doc: &Doc| board(&Calc::new(&doc.p, F, d(11, 20))).cards[0].col.clone();
        assert_eq!(col(&doc), "todo");
        assert!(mv(&mut doc, "documents", "done").unwrap_err().contains("send it first"));
        mv(&mut doc, "documents", "review").unwrap();
        assert_eq!((col(&doc).as_str(), doc.p.documents.log[0].transmittal.as_str(), doc.p.documents.log[0].rev.as_str()), ("review", "TR-0001", "A"), "to the client: the next submission is logged");
        mv(&mut doc, "documents", "resubmit").unwrap();
        assert_eq!(col(&doc), "resubmit", "returned with a code that asks for it again");
        let mut s = doc.p.board.clone(); s.kind = "orders".into(); doc.apply(Op::Board { b: s }, F).unwrap();
        assert_eq!(col(&doc), "m0");
        mv(&mut doc, "orders", "m3").unwrap();
        assert_eq!((col(&doc).as_str(), doc.p.procurement.items[0].actual[2]), ("m3", Some(d(11, 10))), "RFQ, PO and manufacturing start recorded on the day");
        assert!(mv(&mut doc, "orders", "m1").is_err(), "milestones are not taken back on the board");
        mv(&mut doc, "orders", "m5").unwrap();
        assert_eq!((col(&doc).as_str(), doc.p.procurement.items[0].mfg_pct), ("m5", 100.0));
    }
}
