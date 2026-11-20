//! Modules a plan can take on (Project › Modules), and the first of them: the Contract Register.
//! Contract dates are measured against the forecast of the task that achieves them; late ones run
//! up delay damages; payments fall due after their event, giving the cash-in curve. Variations add
//! to the contract value, notices count down their time bars, guarantees their expiry against the
//! task that releases them, invoices their payment terms and obligations their next due date.
use crate::{cal::{date_of, DAY}, fields::Calc, model::*};
use chrono::{Datelike, Days, Months, NaiveDate};
use serde::Serialize;

/// The modules: key, name, what it adds. A plan has only the ones it switched on.
pub const MODULES: [(&str, &str, &str); 3] = [
    ("contract", "Contract Register", "Contract dates against their forecast and delay damages at risk, payment milestones and the cash-in curve, variations, notices and their time bars, guarantees and insurances, invoices and obligations."),
    ("documents", "Document Register", "Vendor documents and every submission: review cycles in working days, return codes, status, client delay, weighted progress and S-curves."),
    ("procurement", "Procurement Register", "Sub-orders from enquiry to delivery: planned against actual and promised dates, the date each is needed, an expediting log, weighted progress and S-curves."),
];

/// What a contract date shows.
#[derive(Serialize, Debug, Default, Clone, PartialEq)]
pub struct MsOut {
    pub id: u32,
    pub name: String,
    pub task: u32,
    pub task_name: String,
    pub date: Option<NaiveDate>,
    /// The task's finish: actual when done, else forecast.
    pub forecast: Option<NaiveDate>,
    pub baseline: Option<NaiveDate>,
    pub done: bool,
    /// Days late (+) or early (−) against the contract date.
    pub var: Option<i64>,
    /// "Met", "Met late", "On track", "At risk", "Late", "No date", "No task".
    pub status: String,
    /// Delay damages at today's forecast, and their cap.
    pub ld: f64,
    pub ld_max: f64,
    /// The forecast at each period close (snapshots) and now: (date, forecast).
    pub trend: Vec<(NaiveDate, NaiveDate)>,
}

/// What a payment shows.
#[derive(Serialize, Debug, Default, Clone, PartialEq)]
pub struct PayOut {
    pub id: u32,
    pub name: String,
    pub amount: f64,
    pub task_name: String,
    /// Due by the baseline (or, without one, by the contract's planned dates) and by the forecast.
    pub planned: Option<NaiveDate>,
    pub due: Option<NaiveDate>,
    pub invoiced: Option<NaiveDate>,
    pub paid: Option<NaiveDate>,
    /// "Paid", "Invoiced", "Overdue", "Due", "Forecast", "No task".
    pub status: String,
}

/// What a variation, notice, guarantee, invoice or obligation shows: the date that matters (a
/// notice's due date, a guarantee's release, an invoice's payment, an obligation's next time),
/// the days left to it (to a guarantee's expiry), the status in words and its colour key
/// ("ok", "warn", "bad" or "").
#[derive(Serialize, Debug, Default, Clone, PartialEq)]
pub struct RowOut {
    pub id: u32,
    pub due: Option<NaiveDate>,
    pub left: Option<i64>,
    pub status: String,
    pub key: String,
}

#[derive(Serialize, Debug, Default, Clone, PartialEq)]
pub struct Out {
    pub value: f64,
    pub status_day: Option<NaiveDate>,
    pub milestones: Vec<MsOut>,
    pub payments: Vec<PayOut>,
    pub late: usize,
    pub at_risk: usize,
    pub ld: f64,
    pub ld_max: f64,
    /// Share of the contract value the payments cover (100 when complete).
    pub pay_pct: f64,
    pub received: f64,
    /// Month ends with cumulative cash in: planned, forecast, received (to the status date).
    pub cash: Vec<(NaiveDate, f64, f64, Option<f64>)>,
    pub variations: Vec<RowOut>,
    pub notices: Vec<RowOut>,
    pub guarantees: Vec<RowOut>,
    pub invoices: Vec<RowOut>,
    pub obligations: Vec<RowOut>,
    /// The contract value with the approved variations.
    pub revised_value: f64,
    pub vo_claimed: f64,
    pub vo_approved: f64,
    /// Claimed by variations still waiting for a decision.
    pub vo_pending: f64,
    pub vo_days: i64,
    /// Notices not sent with a week or less left (overdue included), and those overdue.
    pub notices_due: usize,
    pub notices_late: usize,
    /// Guarantees expiring soon, to extend, or expired.
    pub guar_warn: usize,
    pub invoiced: f64,
    pub certified: f64,
    pub inv_paid: f64,
    pub inv_overdue: usize,
    pub retention: f64,
    pub obl_late: usize,
    pub obl_due: usize,
}

fn row(id: u32, due: Option<NaiveDate>, left: Option<i64>, status: impl Into<String>, key: &str) -> RowOut { RowOut { id, due, left, status: status.into(), key: key.into() } }

/// The next time an obligation is due: its first date, then every week, month or quarter after the last time it was done.
pub fn next_due(o: &Obligation) -> Option<NaiveDate> {
    let mut d = o.due?;
    let step = |d: NaiveDate| match o.every.as_str() { "week" => d.checked_add_days(Days::new(7)), "month" => d.checked_add_months(Months::new(1)), "quarter" => d.checked_add_months(Months::new(3)), _ => None };
    if o.every.is_empty() || !["week", "month", "quarter"].contains(&o.every.as_str()) { return if o.done.is_some() { None } else { Some(d) } }
    let Some(done) = o.done else { return Some(d) };
    for _ in 0..5000 { if d > done { return Some(d) } d = step(d)? }
    None
}

/// The finish of a task as a date: actual when it has one.
fn finish(c: &Calc, i: usize) -> NaiveDate {
    c.p.tasks[i].act_finish.unwrap_or_else(|| date_of(c.s.ef[i] - 1))
}

/// Delay damages for `late` days: a rate in % of the base per week or part week after the grace
/// days, up to the cap (% of the base).
pub fn damages(m: &ContractDate, value: f64, late: i64) -> (f64, f64) {
    let base = if m.value > 0.0 { m.value } else { value };
    let max = base * m.ld_cap.max(0.0) / 100.0;
    let weeks = ((late - m.grace.max(0)).max(0) + 6).div_euclid(7) as f64;
    ((base * m.ld_rate.max(0.0) / 100.0 * weeks).min(if m.ld_cap > 0.0 { max } else { f64::INFINITY }), max)
}

pub fn status(c: &Calc) -> Out {
    let (p, k) = (c.p, &c.p.contract);
    let today = date_of(c.progress_at() - DAY);
    let idx = |uid: u32| if uid == 0 { None } else { p.idx(uid) };
    let mut o = Out { value: k.value, status_day: Some(today), ..Default::default() };
    for m in &k.dates {
        let i = idx(m.task);
        let mut r = MsOut { id: m.id, name: m.name.clone(), task: m.task, date: m.date, ..Default::default() };
        if let Some(i) = i {
            let t = &p.tasks[i];
            r.task_name = t.name.clone();
            r.done = c.pct[i] >= 100;
            r.forecast = Some(finish(c, i));
            r.baseline = t.bls.get(&0).map(|b| date_of(b.finish - 1));
            let mut snaps: Vec<&Snapshot> = p.snapshots.iter().filter(|s| s.date.is_some()).collect();
            snaps.sort_by_key(|s| s.date);
            r.trend = snaps.iter().filter_map(|s| Some((s.date?, date_of(s.rows.iter().find(|x| x.uid == t.uid)?.finish - 1)))).collect();
            if r.trend.last().is_none_or(|x| x.0 < today) { r.trend.push((today, finish(c, i))); }
        }
        r.var = r.forecast.zip(m.date).map(|(f, d)| (f - d).num_days());
        r.status = match (i, m.date, r.var) {
            (None, ..) => "No task",
            (_, None, _) | (_, _, None) => "No date",
            (_, _, Some(v)) if r.done => if v > 0 { "Met late" } else { "Met" },
            (_, _, Some(v)) if v > 0 => "Late",
            (_, _, Some(v)) if -v < k.warn_days.max(0) => "At risk",
            _ => "On track",
        }.into();
        if let Some(v) = r.var { (r.ld, r.ld_max) = damages(m, k.value, v); } else { r.ld_max = damages(m, k.value, 0).1; }
        o.late += usize::from(r.status == "Late");
        o.at_risk += usize::from(r.status == "At risk");
        o.ld += r.ld;
        o.ld_max += r.ld_max;
        o.milestones.push(r);
    }
    for pm in &k.payments {
        let amount = if pm.amount > 0.0 { pm.amount } else { k.value * pm.pct / 100.0 };
        let terms = Days::new(pm.terms.max(0) as u64);
        let billed: Vec<&Invoice> = k.invoices.iter().filter(|x| x.payment == pm.id && pm.id != 0).collect();
        let invoiced = pm.invoiced.or_else(|| billed.iter().filter_map(|x| x.invoiced).min());
        let paid = pm.paid.or_else(|| (!billed.is_empty() && billed.iter().all(|x| x.paid.is_some())).then(|| billed.iter().filter_map(|x| x.paid).max()).flatten());
        let mut r = PayOut { id: pm.id, name: pm.name.clone(), amount, invoiced, paid, ..Default::default() };
        let i = idx(pm.task);
        if let Some(i) = i {
            r.task_name = p.tasks[i].name.clone();
            r.due = finish(c, i).checked_add_days(terms);
            r.planned = p.tasks[i].bls.get(&0).map(|b| date_of(b.finish - 1)).unwrap_or_else(|| date_of(c.s.ef[i] - 1)).checked_add_days(terms);
        }
        let event_done = i.is_some_and(|i| c.pct[i] >= 100);
        r.status = if r.paid.is_some() { "Paid" } else if r.invoiced.is_some() { "Invoiced" } else if i.is_none() { "No task" } else if event_done && r.due.is_some_and(|d| d < today) { "Overdue" } else if event_done { "Due" } else { "Forecast" }.into();
        o.pay_pct += if pm.amount > 0.0 && k.value > 0.0 { pm.amount / k.value * 100.0 } else { pm.pct };
        if r.paid.is_some_and(|d| d <= today) { o.received += amount; }
        o.payments.push(r);
    }
    o.cash = cash(&o.payments, today);
    // Variations: approved ones change the contract value; submitted ones wait for a decision.
    let is = |v: &Variation, w: &str| v.status.trim().eq_ignore_ascii_case(w);
    for v in &k.variations {
        let waiting = is(v, "Submitted");
        let key = if is(v, "Approved") { "ok" } else if is(v, "Rejected") { "" } else if waiting { "warn" } else { "" };
        let age = waiting.then(|| v.submitted.map(|d| (today - d).num_days())).flatten();
        o.variations.push(row(v.id, v.decided, age, if v.status.is_empty() { "Draft" } else { &v.status }, key));
        o.vo_claimed += v.claimed;
        if is(v, "Approved") { o.vo_approved += v.approved; o.vo_days += v.days_granted; }
        if waiting { o.vo_pending += v.claimed; }
    }
    o.revised_value = k.value + o.vo_approved;
    // Notices: due the days allowed (the kind's when not given) after the event.
    for n in &k.notices {
        let days = if n.days > 0 { n.days } else { k.notice_kinds.iter().find(|x| x.name.eq_ignore_ascii_case(n.kind.trim())).map_or(0, |x| x.days) };
        let due = n.event_date.and_then(|d| d.checked_add_days(Days::new(days.max(0) as u64)));
        let left = due.map(|d| (d - today).num_days());
        let r = match (due, n.sent, left) {
            (Some(d), Some(s), _) if s <= d => row(n.id, due, None, "Sent in time", "ok"),
            (Some(d), Some(s), _) => row(n.id, due, None, format!("Sent late ({} d)", (s - d).num_days()), "warn"),
            (None, Some(_), _) => row(n.id, due, None, "Sent", "ok"),
            (_, None, Some(l)) if l < 0 => row(n.id, due, left, format!("Overdue {} d", -l), "bad"),
            (_, None, Some(l)) => row(n.id, due, left, format!("{l} day{} left", if l == 1 { "" } else { "s" }), if l <= 7 { "bad" } else if l <= 30 { "warn" } else { "" }),
            _ => row(n.id, due, None, "No event date", ""),
        };
        if n.sent.is_none() && left.is_some_and(|l| l <= 7) { o.notices_due += 1; }
        if n.sent.is_none() && left.is_some_and(|l| l < 0) { o.notices_late += 1; }
        o.notices.push(r);
    }
    // Guarantees: expiry against the task that releases them.
    let warn = if k.expiry_warn > 0 { k.expiry_warn } else { 60 };
    for g in &k.guarantees {
        let release = idx(g.task).and_then(|i| finish(c, i).checked_add_days(Days::new(g.after.max(0) as u64)));
        let left = g.expires.map(|d| (d - today).num_days());
        let r = if g.released.is_some() { row(g.id, release, None, "Released", "ok") }
            else if g.number.trim().is_empty() && g.from.is_none() { row(g.id, release, None, "Not issued", "") }
            else if left.is_some_and(|l| l < 0) { row(g.id, release, left, "Expired", "bad") }
            else if release.zip(g.expires).is_some_and(|(r, e)| e < r) { row(g.id, release, left, "Extend or reduce", if left.is_some_and(|l| l <= warn) { "bad" } else { "warn" }) }
            else if left.is_some_and(|l| l <= warn) { row(g.id, release, left, "Expiring", "warn") }
            else { row(g.id, release, left, "In force", "ok") };
        if matches!(r.key.as_str(), "warn" | "bad") { o.guar_warn += 1; }
        o.guarantees.push(r);
    }
    // Invoices: paid by their terms after they are sent.
    let terms = if k.invoice_terms > 0 { k.invoice_terms } else { 30 };
    for v in &k.invoices {
        let due = v.invoiced.and_then(|d| d.checked_add_days(Days::new((if v.terms > 0 { v.terms } else { terms }) as u64)));
        let left = due.map(|d| (d - today).num_days());
        let r = if v.paid.is_some() { row(v.id, due, None, "Paid", "ok") }
            else if v.invoiced.is_none() { row(v.id, due, None, "Draft", "") }
            else if left.is_some_and(|l| l < 0) { row(v.id, due, left, format!("Overdue {} d", -left.unwrap_or(0)), "bad") }
            else if v.certified > 0.0 { row(v.id, due, left, "Certified", "") }
            else { row(v.id, due, left, "Submitted", "") };
        if v.invoiced.is_some() { o.invoiced += v.amount; }
        o.certified += v.certified;
        o.retention += v.retention;
        if v.paid.is_some_and(|d| d <= today) { o.inv_paid += if v.certified > 0.0 { v.certified - v.retention } else { v.amount }; }
        o.inv_overdue += usize::from(r.key == "bad");
        o.invoices.push(r);
    }
    // Obligations: the next time each is due.
    for b in &k.obligations {
        let due = next_due(b);
        let left = due.map(|d| (d - today).num_days());
        let r = match left {
            _ if due.is_none() && b.done.is_some() => row(b.id, None, None, "Done", "ok"),
            None => row(b.id, None, None, "No date", ""),
            Some(l) if l < 0 => row(b.id, due, left, format!("Overdue {} d", -l), "bad"),
            Some(l) if l <= 7 => row(b.id, due, left, format!("Due in {l} d"), "warn"),
            Some(_) => row(b.id, due, left, "Open", ""),
        };
        o.obl_late += usize::from(r.key == "bad");
        o.obl_due += usize::from(r.key == "warn");
        o.obligations.push(r);
    }
    o
}

/// The month ends from the first payment date to the last, with cumulative cash in.
fn cash(ps: &[PayOut], today: NaiveDate) -> Vec<(NaiveDate, f64, f64, Option<f64>)> {
    let when = |r: &PayOut| r.paid.or(r.due);
    let all: Vec<NaiveDate> = ps.iter().flat_map(|r| [r.planned, when(r)]).flatten().collect();
    let (Some(a), Some(b)) = (all.iter().min(), all.iter().max()) else { return vec![] };
    let month_end = |d: NaiveDate| {
        let (y, m) = if d.month() == 12 { (d.year() + 1, 1) } else { (d.year(), d.month() + 1) };
        NaiveDate::from_ymd_opt(y, m, 1).and_then(|x| x.pred_opt()).unwrap_or(d)
    };
    let (mut d, end, mut out) = (month_end(*a), month_end(*b), vec![]);
    while d <= end && out.len() < 600 {
        let sum = |f: &dyn Fn(&PayOut) -> Option<NaiveDate>| ps.iter().filter(|r| f(r).is_some_and(|x| x <= d)).map(|r| r.amount).sum::<f64>();
        let got = (d <= month_end(today)).then(|| sum(&|r| r.paid.filter(|x| *x <= today)));
        out.push((d, sum(&|r| r.planned), sum(&when), got));
        d = month_end(d.checked_add_days(Days::new(1)).unwrap_or(d));
        if d == end && out.last().is_some_and(|x| x.0 == end) { break }
    }
    out
}

/// Tidies a contract before it is stored: finite, non-negative numbers, unique ids, short texts.
pub fn clean(k: &mut Contract) {
    let num = |v: &mut f64| { if !v.is_finite() || *v < 0.0 { *v = 0.0 } };
    let text = |s: &mut String, n: usize| { if s.chars().count() > n { *s = s.chars().take(n).collect() } };
    num(&mut k.value);
    k.warn_days = k.warn_days.clamp(0, 3650);
    text(&mut k.number, 120);
    let mut next = 1;
    let mut seen = std::collections::HashSet::new();
    for m in &mut k.dates {
        if m.id == 0 || !seen.insert(m.id) { while seen.contains(&next) { next += 1 } m.id = next; seen.insert(next); }
        for v in [&mut m.value, &mut m.ld_rate, &mut m.ld_cap] { num(v) }
        m.grace = m.grace.clamp(0, 3650);
        text(&mut m.name, 200);
        text(&mut m.note, 2000);
    }
    let mut seen = std::collections::HashSet::new();
    next = 1;
    for x in &mut k.payments {
        if x.id == 0 || !seen.insert(x.id) { while seen.contains(&next) { next += 1 } x.id = next; seen.insert(next); }
        for v in [&mut x.amount, &mut x.pct] { num(v) }
        x.terms = x.terms.clamp(0, 3650);
        text(&mut x.name, 200);
    }
    k.expiry_warn = k.expiry_warn.clamp(0, 3650);
    k.invoice_terms = k.invoice_terms.clamp(0, 3650);
    fn ids<T>(v: &mut Vec<T>, id: impl Fn(&mut T) -> &mut u32) {
        v.truncate(2000);
        let mut seen = std::collections::HashSet::new();
        let mut next = 1;
        for x in v.iter_mut() { let i = id(x); if *i == 0 || !seen.insert(*i) { while seen.contains(&next) { next += 1 } *i = next; seen.insert(next); } }
    }
    ids(&mut k.variations, |x| &mut x.id);
    ids(&mut k.notices, |x| &mut x.id);
    ids(&mut k.guarantees, |x| &mut x.id);
    ids(&mut k.invoices, |x| &mut x.id);
    ids(&mut k.obligations, |x| &mut x.id);
    for v in &mut k.variations {
        for x in [&mut v.claimed, &mut v.approved] { num(x) }
        v.days_claimed = v.days_claimed.clamp(0, 36500); v.days_granted = v.days_granted.clamp(0, 36500);
        for (t, n) in [(&mut v.no, 60), (&mut v.desc, 400), (&mut v.raised_by, 60), (&mut v.status, 60), (&mut v.delay_event, 200), (&mut v.remarks, 2000)] { text(t, n) }
    }
    for n in &mut k.notices {
        n.days = n.days.clamp(0, 3650);
        for (t, m) in [(&mut n.no, 60), (&mut n.event, 400), (&mut n.kind, 100), (&mut n.our_ref, 100), (&mut n.linked, 100)] { text(t, m) }
    }
    for g in &mut k.guarantees {
        num(&mut g.amount);
        g.after = g.after.clamp(0, 3650);
        for (t, m) in [(&mut g.name, 200), (&mut g.basis, 200), (&mut g.issuer, 100), (&mut g.number, 100)] { text(t, m) }
    }
    for v in &mut k.invoices {
        for x in [&mut v.amount, &mut v.certified, &mut v.retention] { num(x) }
        v.terms = v.terms.clamp(0, 3650);
        for (t, m) in [(&mut v.no, 60), (&mut v.remarks, 2000)] { text(t, m) }
    }
    for b in &mut k.obligations {
        if !["", "week", "month", "quarter"].contains(&b.every.as_str()) { b.every.clear() }
        for (t, m) in [(&mut b.clause, 60), (&mut b.what, 400), (&mut b.party, 60), (&mut b.owner, 100), (&mut b.remarks, 2000)] { text(t, m) }
    }
    k.notice_kinds.retain(|x| !x.name.trim().is_empty());
    k.notice_kinds.truncate(100);
    for x in &mut k.notice_kinds { x.days = x.days.clamp(0, 3650); text(&mut x.name, 100) }
    k.dates.truncate(2000);
    k.payments.truncate(2000);
}

#[cfg(test)]
mod t {
    use super::*;
    use crate::doc::{Doc, Op};

    const F: &str = "%d/%m/%Y";
    fn d(y: i32, m: u32, dd: u32) -> NaiveDate { NaiveDate::from_ymd_opt(y, m, dd).unwrap() }

    /// A unit delivered by a task, with its contract date, damages and two payments.
    fn plan() -> Doc {
        let mut doc = Doc::new(Project::new("Compressor package", d(2026, 11, 2)));
        doc.apply(Op::Modules { on: vec!["contract".into()] }, F).unwrap();
        let row = |n: &str, dur: &str| vec![("name".to_string(), n.to_string()), ("dur".to_string(), dur.to_string())];
        doc.apply(Op::InsertRows { at: 0, rows: vec![row("Manufacture", "40d"), row("Delivery FCA", "0d")] }, F).unwrap();
        let uids = doc.p.tasks.iter().map(|t| t.uid).collect();
        doc.apply(Op::Link { uids }, F).unwrap();
        doc
    }

    #[test]
    fn a_contract_date_against_its_forecast_with_damages_and_cash() {
        let mut doc = plan();
        let fca = doc.p.tasks[1].uid;
        let c = Calc::new(&doc.p, F, d(2026, 11, 2));
        let fc = finish(&c, 1);
        drop(c);
        // The contract date 10 days before the forecast: 2 weeks (or part) of 0.5 % on the unit's 1,000,000, capped at 10 %.
        let k = Contract {
            value: 4_000_000.0, warn_days: 14,
            dates: vec![
                ContractDate { id: 1, name: "Unit FCA".into(), task: fca, date: fc.checked_sub_days(Days::new(10)), value: 1_000_000.0, ld_rate: 0.5, ld_cap: 10.0, ..Default::default() },
                ContractDate { id: 1, name: "Unit FCA (second)".into(), task: fca, date: fc.checked_add_days(Days::new(5)), ..Default::default() },
                ContractDate { name: "No task".into(), date: Some(fc), ..Default::default() },
            ],
            payments: vec![
                Payment { id: 1, name: "Advance".into(), pct: 10.0, task: doc.p.tasks[0].uid, terms: 30, ..Default::default() },
                Payment { id: 2, name: "On delivery".into(), pct: 90.0, task: fca, terms: 60, paid: None, ..Default::default() },
            ],
            ..Default::default()
        };
        doc.apply(Op::Contract { c: Box::new(k) }, F).unwrap();
        assert_eq!(doc.p.contract.dates.iter().map(|m| m.id).collect::<Vec<_>>(), vec![1, 2, 3], "duplicate ids are renumbered");
        let o = status(&Calc::new(&doc.p, F, d(2026, 11, 2)));
        let m = &o.milestones;
        assert_eq!((m[0].var, m[0].status.as_str()), (Some(10), "Late"));
        assert_eq!((m[0].ld, m[0].ld_max), (10_000.0, 100_000.0), "2 weeks × 0.5 % of 1,000,000, cap 10 %");
        assert_eq!((m[1].var, m[1].status.as_str(), m[1].ld), (Some(-5), "At risk", 0.0), "5 days of float is under the 14-day warning");
        assert_eq!(m[2].status, "No task");
        assert_eq!((o.late, o.at_risk, o.ld), (1, 1, 10_000.0));
        assert_eq!(o.payments[1].amount, 3_600_000.0);
        assert_eq!(o.payments[1].due, fc.checked_add_days(Days::new(60)));
        assert_eq!(o.payments[0].status, "Forecast");
        assert_eq!(o.pay_pct, 100.0);
        let last = o.cash.last().unwrap();
        assert_eq!((last.1, last.2), (4_000_000.0, 4_000_000.0), "planned and forecast cash reach the full value");
        assert!(o.cash.windows(2).all(|w| w[0].0 < w[1].0 && w[0].2 <= w[1].2));
        assert_eq!(m[0].trend.len(), 1, "no period closes yet: the trend is today's forecast");
    }

    #[test]
    fn variations_notices_guarantees_invoices_and_obligations() {
        let mut doc = plan();
        let fca = doc.p.tasks[1].uid;
        let t = d(2026, 11, 20);
        let fc = finish(&Calc::new(&doc.p, F, t), 1);
        let k = Contract {
            value: 1_000_000.0,
            payments: vec![Payment { id: 1, name: "Delivery".into(), pct: 100.0, task: fca, ..Default::default() }],
            variations: vec![
                Variation { no: "VO-001".into(), status: "Approved".into(), claimed: 120.0, approved: 100.0, days_granted: 5, ..Default::default() },
                Variation { no: "VO-002".into(), status: "Submitted".into(), claimed: 50.0, submitted: Some(d(2026, 11, 10)), ..Default::default() },
                Variation { no: "VO-003".into(), status: "Rejected".into(), claimed: 40.0, ..Default::default() },
            ],
            notice_kinds: vec![NoticeKind { name: "Delay".into(), days: 28 }],
            notices: vec![
                Notice { no: "N1".into(), kind: "Delay".into(), event_date: Some(d(2026, 10, 26)), ..Default::default() },
                Notice { no: "N2".into(), kind: "delay".into(), event_date: Some(d(2026, 10, 1)), sent: Some(d(2026, 11, 2)), ..Default::default() },
                Notice { no: "N3".into(), days: 10, event_date: Some(d(2026, 11, 1)), ..Default::default() },
                Notice { no: "N4".into(), event_date: Some(d(2026, 11, 1)), days: 60, ..Default::default() },
            ],
            guarantees: vec![
                Guarantee { name: "Performance".into(), number: "PB-1".into(), expires: d(2026, 12, 1).into(), task: fca, after: 0, ..Default::default() },
                Guarantee { name: "Warranty".into(), ..Default::default() },
                Guarantee { name: "Advance".into(), number: "A-1".into(), expires: fc.checked_add_days(Days::new(400)), task: fca, ..Default::default() },
            ],
            invoices: vec![
                Invoice { no: "INV-1".into(), payment: 1, amount: 500.0, invoiced: Some(d(2026, 10, 1)), ..Default::default() },
                Invoice { no: "INV-2".into(), amount: 300.0, invoiced: Some(d(2026, 11, 1)), certified: 300.0, retention: 15.0, paid: Some(d(2026, 11, 15)), ..Default::default() },
            ],
            obligations: vec![
                Obligation { what: "Monthly progress report".into(), due: Some(d(2026, 8, 5)), every: "month".into(), done: Some(d(2026, 11, 6)), ..Default::default() },
                Obligation { what: "Quality plan".into(), due: Some(d(2026, 11, 1)), ..Default::default() },
                Obligation { what: "Kick-off".into(), due: Some(d(2026, 10, 1)), done: Some(d(2026, 10, 1)), ..Default::default() },
            ],
            ..Default::default()
        };
        doc.apply(Op::Contract { c: Box::new(k) }, F).unwrap();
        let o = status(&Calc::new(&doc.p, F, t));
        let today = o.status_day.unwrap();
        assert_eq!((o.revised_value, o.vo_approved, o.vo_pending, o.vo_days, o.vo_claimed), (1_000_100.0, 100.0, 50.0, 5, 210.0));
        assert_eq!(o.variations[1].left, Some((today - d(2026, 11, 10)).num_days()), "a submitted variation counts the days it waits");
        let n = &o.notices;
        assert_eq!(n[0].due, Some(d(2026, 11, 23)), "the kind's 28 days after the event");
        assert_eq!(n[1].status, "Sent late (4 d)", "the kind matches whatever its case");
        assert_eq!((n[2].due, n[2].key.as_str()), (Some(d(2026, 11, 11)), "bad"));
        assert!(n[2].status.starts_with("Overdue"));
        assert_eq!(n[3].key, "", "far off");
        assert_eq!((o.notices_late, o.notices_due), (1, 2));
        let g = &o.guarantees;
        assert_eq!(g[0].status, if d(2026, 12, 1) < fc { "Extend or reduce" } else { "Expiring" });
        assert_eq!(g[0].due, Some(fc), "released by the delivery task");
        assert_eq!((g[1].status.as_str(), g[2].status.as_str()), ("Not issued", "In force"));
        assert_eq!(o.guar_warn, 1);
        assert_eq!(o.invoices[0].status, format!("Overdue {} d", (today - d(2026, 10, 31)).num_days()), "30 days' terms by default");
        assert_eq!((o.invoices[1].status.as_str(), o.invoiced, o.certified, o.retention, o.inv_paid, o.inv_overdue), ("Paid", 800.0, 300.0, 15.0, 285.0, 1));
        assert_eq!(o.payments[0].invoiced, Some(d(2026, 10, 1)), "a payment is invoiced by the invoices that bill it");
        assert_eq!(o.payments[0].status, "Invoiced");
        assert_eq!(o.obligations[0].due, Some(d(2026, 12, 5)), "the month after the last time it was done");
        assert_eq!((o.obligations[1].key.as_str(), o.obligations[2].status.as_str()), ("bad", "Done"));
        assert_eq!(next_due(&Obligation { due: Some(d(2026, 1, 31)), every: "quarter".into(), done: Some(d(2026, 2, 1)), ..Default::default() }), Some(d(2026, 4, 30)));
    }

    #[test]
    fn damages_follow_grace_part_weeks_and_the_cap() {
        let m = ContractDate { ld_rate: 1.0, ld_cap: 5.0, grace: 7, ..Default::default() };
        assert_eq!(damages(&m, 100.0, 7), (0.0, 5.0), "within the grace days");
        assert_eq!(damages(&m, 100.0, 8), (1.0, 5.0), "one day into a week counts the week");
        assert_eq!(damages(&m, 100.0, 70), (5.0, 5.0), "capped");
        assert_eq!(damages(&ContractDate { ld_rate: 1.0, ..Default::default() }, 100.0, 70).0, 10.0, "no cap");
        assert_eq!(damages(&m, 100.0, -20).0, 0.0);
    }

    #[test]
    fn modules_switch_on_and_off_and_off_can_drop_the_data() {
        let mut doc = plan();
        assert!(doc.apply(Op::Modules { on: vec!["nonsense".into()] }, F).is_err());
        doc.apply(Op::Contract { c: Box::new(Contract { value: 5.0, ..Default::default() }) }, F).unwrap();
        doc.apply(Op::Modules { on: vec![] }, F).unwrap();
        assert!(doc.p.modules.is_empty() && doc.p.contract.value == 5.0, "switched off, the data stays");
        doc.apply(Op::Contract { c: Box::default() }, F).unwrap();
        let json = serde_json::to_string(&doc.p).unwrap();
        assert!(!json.contains("\"contract\"") && !json.contains("\"modules\""), "a plan without modules saves as before");
    }
}
