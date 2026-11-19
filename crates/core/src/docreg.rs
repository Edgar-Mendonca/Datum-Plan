//! The Document Register module: vendor documents with their class, a transmittal log of every
//! submission and the client's return code, planned dates from the P date and review cycles in
//! working days (each on the plan's calendar or one chosen for it), status, client delay, earned
//! progress by weighted steps and the weekly and monthly S-curves.
use crate::{cal::{date_of, WorkCal, DAY}, fields::Calc, model::*};
use chrono::{Datelike, Days, NaiveDate};
use serde::Serialize;
use std::collections::HashMap;

#[derive(Serialize, Debug, Default, Clone, PartialEq)]
pub struct DocRow {
    pub id: u32,
    /// Each step of the document's class: planned, actual and forecast dates.
    pub planned: Vec<Option<NaiveDate>>,
    pub actual: Vec<Option<NaiveDate>>,
    pub forecast: Vec<Option<NaiveDate>>,
    /// The class's step names (in order).
    pub steps: Vec<String>,
    /// The last step: done, and its date (actual, else forecast).
    pub done: bool,
    pub finish: Option<NaiveDate>,
    /// When the document is approved enough to build on (the approval step, else the first return):
    /// actual and forecast. Orders that wait for the document use these.
    pub approved: Option<NaiveDate>,
    pub approved_fc: Option<NaiveDate>,
    pub subs: usize,
    /// Submissions the client sent back to be revised (codes that ask for a resubmission).
    pub cycles: usize,
    pub rev: String,
    pub purpose: String,
    pub sent: Option<NaiveDate>,
    pub due: Option<NaiveDate>,
    pub returned: Option<NaiveDate>,
    pub code: String,
    pub status: String,
    /// The status's key (not_sub, not_sub_late, review, review_late, resubmit, accepted, final, off).
    pub skey: String,
    /// What the register's own columns show (formula columns worked out), in their order.
    pub own: Vec<String>,
    /// First issue: days after its planned date (to today while not sent).
    pub late1: i64,
    /// Days the client kept the document past the review period, over all submissions.
    pub client_late: i64,
    pub earned: f64,
    pub planned_pct: f64,
    pub task_name: String,
    /// The linked task's start: when the document is needed.
    pub need: Option<NaiveDate>,
    pub late_for_task: bool,
}

#[derive(Serialize, Debug, Default, Clone, PartialEq)]
pub struct SubRow {
    pub id: u32,
    pub due: Option<NaiveDate>,
    pub accepted: bool,
    /// Days with the client (to today while not returned).
    pub days: i64,
    /// Days past the review period.
    pub late: i64,
    /// "Returned", "In review", "Review overdue", "Not sent".
    pub state: String,
}

/// One point of an S-curve: period end, planned and actual cumulative % (actual stops at the data date),
/// first issues planned and done in the period.
#[derive(Serialize, Debug, Default, Clone, PartialEq)]
pub struct Point { pub end: NaiveDate, pub planned: f64, pub actual: Option<f64>, pub first_planned: usize, pub first_done: usize }

#[derive(Serialize, Debug, Default, Clone, PartialEq)]
pub struct RegOut {
    pub data_date: Option<NaiveDate>,
    pub docs: Vec<DocRow>,
    pub subs: Vec<SubRow>,
    pub earned: f64,
    pub planned: f64,
    pub client_late: i64,
    pub weekly: Vec<Point>,
    pub monthly: Vec<Point>,
    /// For each class: its weights add up to 100 %.
    pub weights_ok: Vec<bool>,
    pub classes: Vec<String>,
}
/// The purposes of a step ("ICR, IFC" → ["ICR", "IFC"]).
fn list(s: &str) -> Vec<String> { s.split([',', ';', '/']).map(|x| x.trim().to_uppercase()).filter(|x| !x.is_empty()).collect() }

fn add_wd(c: &WorkCal, d: NaiveDate, n: i64) -> NaiveDate {
    let (mut x, mut k) = (d, 0);
    for _ in 0..20_000 {
        if k >= n { break }
        x = x.succ_opt().unwrap_or(x);
        if c.is_work_day(x) { k += 1 }
    }
    x
}

pub fn status(c: &Calc) -> RegOut {
    let (p, r) = (c.p, &c.p.documents);
    let today = date_of(c.progress_at() - DAY);
    let review = WorkCal::new(p.cal((!r.review_cal.is_empty()).then_some(r.review_cal.as_str())));
    let work = WorkCal::new(p.cal((!r.work_cal.is_empty()).then_some(r.work_cal.as_str())));
    let (rv, tn) = (r.review_days.max(0), r.turnaround_days.max(0));
    let code = |s: &str| r.codes.iter().find(|x| x.code.eq_ignore_ascii_case(s.trim()));
    let accepted = |s: &str| code(s).is_some_and(|x| x.accepted);
    let resubmit = |s: &str| code(s).is_some_and(|x| x.resubmit);
    let p_date = r.p_date.unwrap_or(p.start);
    let unit = if r.p_unit == "days" { 1.0 } else { 7.0 };
    let mut by_doc: HashMap<u32, Vec<&Submission>> = HashMap::new();
    for s in &r.log { by_doc.entry(s.doc).or_default().push(s) }
    for v in by_doc.values_mut() { v.sort_by_key(|s| (s.sent, s.id)) }
    let classes = r.classes();
    let mut o = RegOut { data_date: Some(today), classes: classes.iter().map(|c| c.name.clone()).collect(), ..Default::default() };
    o.weights_ok = classes.iter().map(|c| (c.steps.iter().map(|s| s.weight).sum::<f64>() - 100.0).abs() < 0.01).collect();
    let (mut wsum, mut esum, mut psum) = (0.0, 0.0, 0.0);
    let mut eff: Vec<(f64, usize)> = vec![];
    let (appr, cons) = (r.flow.approval.to_uppercase(), r.flow.construction.to_uppercase());
    for d in &r.docs {
        let subs = by_doc.get(&d.id).cloned().unwrap_or_default();
        let cls = (d.class.max(1) as usize - 1).min(classes.len().saturating_sub(1));
        let steps: &[DocStep] = classes.get(cls).map(|c| c.steps.as_slice()).unwrap_or(&[]);
        let n = steps.len();
        let of = |st: &DocStep| { let ps = list(&st.purposes); subs.iter().filter(|s| ps.is_empty() || ps.contains(&s.purpose.trim().to_uppercase())).copied().collect::<Vec<_>>() };
        let counts = |st: &DocStep, code: &str| { let cs = list(&st.codes); if cs.is_empty() { accepted(code) } else { cs.contains(&code.trim().to_uppercase()) } };
        let mine: Vec<Vec<&Submission>> = steps.iter().map(of).collect();
        // Actual: the first issue sent for the step, or the first return with a code that counts.
        let a: Vec<Option<NaiveDate>> = steps.iter().zip(&mine).map(|(st, v)| if st.returned { v.iter().filter(|s| counts(st, &s.code)).filter_map(|s| s.returned).min() } else { v.iter().filter_map(|s| s.sent).min() }).collect();
        // The working days from the step before: its own, else the review before a return, the
        // turnaround after one, both when an issue follows an issue.
        let gap = |i: usize, x: NaiveDate| -> NaiveDate {
            let st = &steps[i];
            if let Some(k) = st.days { return add_wd(&work, x, k.max(0)) }
            if st.returned { add_wd(&review, x, rv) } else if i > 0 && steps[i - 1].returned { add_wd(&work, x, tn) } else { add_wd(&work, add_wd(&review, x, rv), tn) }
        };
        // Planned: the first issue from the P date (or a date of its own), then step by step.
        let mut pl: Vec<Option<NaiveDate>> = vec![None; n];
        if let Some(f) = d.first.or_else(|| d.offset.and_then(|k| p_date.checked_add_days(Days::new((k.max(0.0) * unit).round() as u64)))) {
            for i in 0..n { pl[i] = Some(if i == 0 { f } else { gap(i, pl[i - 1].unwrap_or(f)) }) }
        }
        // Forecast: actual, else the chain from the last thing done (a return waits on the issue with the client), never before today.
        let pending = |v: &[&Submission], f_sent: Option<NaiveDate>| -> Option<NaiveDate> {
            let last = v.last()?;
            if last.returned.is_some() && resubmit(&last.code) { return Some(add_wd(&review, add_wd(&work, last.returned?, tn).max(today), rv)) }
            if last.returned.is_none() { return Some(add_wd(&review, last.sent.or(f_sent)?, rv).max(today)) }
            None
        };
        let mut f: Vec<Option<NaiveDate>> = vec![None; n];
        for i in 0..n {
            f[i] = a[i].or_else(|| if i == 0 { pl[0].map(|x| x.max(today)) } else {
                (if steps[i].returned { pending(&mine[i], f[i - 1]) } else { None }).or(f[i - 1].map(|x| gap(i, x).max(today)))
            });
        }
        let has = |st: &DocStep, p: &str| list(&st.purposes).iter().any(|x| x == p);
        let ap = steps.iter().position(|st| st.returned && has(st, &appr)).or_else(|| steps.iter().position(|st| st.returned));
        let co = steps.iter().position(|st| !st.returned && has(st, &cons)).or(n.checked_sub(1));
        let last = subs.last();
        let due = last.and_then(|s| s.sent).map(|x| add_wd(&review, x, rv));
        let done = n > 0 && a[n - 1].is_some();
        let counted = r.counts(d);
        let skey = match last {
            _ if !counted => "off",
            None => if pl.first().copied().flatten().is_some_and(|x| x < today) { "not_sub_late" } else { "not_sub" },
            Some(_) if done => "final",
            Some(s) if s.returned.is_none() => if due.is_some_and(|x| today > x) { "review_late" } else { "review" },
            Some(s) if resubmit(&s.code) => "resubmit",
            _ => "accepted",
        };
        let status = r.words.get(skey).filter(|w| !w.trim().is_empty()).cloned()
            .unwrap_or_else(|| DOC_STATUS.iter().find(|x| x.0 == skey).map_or(String::new(), |x| x.1.to_string()));
        let wt = |i: usize| steps[i].weight / 100.0;
        let earned: f64 = (0..n).filter(|&i| a[i].is_some_and(|x| x <= today)).map(wt).sum();
        let planned: f64 = (0..n).filter(|&i| pl[i].is_some_and(|x| x <= today)).map(wt).sum();
        let (late1, finish, approved, approved_fc, need_fc) = (
            pl.first().copied().flatten().map(|x| (a.first().copied().flatten().unwrap_or(today) - x).num_days().max(0)).unwrap_or(0),
            n.checked_sub(1).and_then(|k| a[k].or(f[k])), ap.and_then(|k| a[k]), ap.and_then(|k| a[k].or(f[k])), co.and_then(|k| f[k]));
        let mut row = DocRow {
            id: d.id, steps: steps.iter().map(|s| s.name.clone()).collect(), done, finish, approved, approved_fc, planned: pl, actual: a, forecast: f, subs: subs.len(), cycles: subs.iter().filter(|s| s.returned.is_some() && resubmit(&s.code)).count(),
            rev: last.map(|s| s.rev.clone()).unwrap_or_default(), purpose: last.map(|s| s.purpose.clone()).unwrap_or_default(),
            sent: last.and_then(|s| s.sent), due, returned: last.and_then(|s| s.returned), code: last.map(|s| s.code.clone()).unwrap_or_default(),
            status, skey: skey.into(), own: r.columns.iter().map(|x| d.own.get(&x.name).cloned().unwrap_or_default()).collect(), late1, earned: earned * 100.0, planned_pct: planned * 100.0, ..Default::default()
        };
        for s in &subs {
            let due = s.sent.map(|x| add_wd(&review, x, rv));
            let end = s.returned.unwrap_or(today);
            let late = due.map(|x| (end - x).num_days().max(0)).unwrap_or(0);
            row.client_late += late;
            o.subs.push(SubRow { id: s.id, due, accepted: accepted(&s.code), days: s.sent.map(|x| (end - x).num_days().max(0)).unwrap_or(0), late,
                state: (if s.sent.is_none() { "Not sent" } else if s.returned.is_some() { "Returned" } else if due.is_some_and(|x| today > x) { "Review overdue" } else { "In review" }).into() });
        }
        if let Some(i) = (d.task > 0).then(|| p.idx(d.task)).flatten() {
            row.task_name = p.tasks[i].name.clone();
            row.need = Some(date_of(c.s.es[i]));
            row.late_for_task = need_fc.zip(row.need).is_some_and(|(x, n)| x > n);
        }
        o.client_late += row.client_late;
        let w = if d.progress && !d.recurring && counted { d.weight.max(0.0) } else { 0.0 };
        wsum += w; esum += w * earned; psum += w * planned;
        eff.push((w, cls));
        o.docs.push(row);
    }
    if wsum > 0.0 { o.earned = esum / wsum * 100.0; o.planned = psum / wsum * 100.0; }
    // The register's own formula columns, worked out per document.
    if r.columns.iter().any(|x| x.kind == "formula") {
        for (k, v) in crate::regtable::own_formulas(c, &o).into_iter().enumerate() {
            for (i, x) in r.columns.iter().enumerate() { if x.kind == "formula" { if let Some(s) = v.get(i) { o.docs[k].own[i] = s.clone() } } }
        }
    }
    o.weekly = curve(&classes, &o.docs, &eff, wsum, today, |d| { let back = (d.weekday().num_days_from_monday() as i64 - p.periods.week_end.min(6) as i64).rem_euclid(7); d.checked_add_days(Days::new(((7 - back) % 7) as u64)).unwrap_or(d) });
    o.monthly = curve(&classes, &o.docs, &eff, wsum, today, |d| { let (y, m) = if d.month() == 12 { (d.year() + 1, 1) } else { (d.year(), d.month() + 1) }; NaiveDate::from_ymd_opt(y, m, 1).and_then(|x| x.pred_opt()).unwrap_or(d) });
    o
}

/// Cumulative planned and actual % at each period end, from the first planned date to the last.
fn curve(classes: &[DocClass], docs: &[DocRow], eff: &[(f64, usize)], wsum: f64, today: NaiveDate, end_of: impl Fn(NaiveDate) -> NaiveDate) -> Vec<Point> {
    let all: Vec<NaiveDate> = docs.iter().flat_map(|d| d.planned.iter().chain(d.actual.iter()).flatten().copied()).collect();
    let (Some(a), Some(b)) = (all.iter().min(), all.iter().max()) else { return vec![] };
    let wt = |i: usize, cls: usize| classes.get(cls).and_then(|c| c.steps.get(i)).map(|s| s.weight).unwrap_or(0.0) / 100.0;
    let at = |e: NaiveDate, actual: bool| {
        if wsum <= 0.0 { return 0.0 }
        docs.iter().zip(eff).map(|(d, &(w, cls))| w * (0..d.planned.len()).filter(|&i| (if actual { d.actual[i] } else { d.planned[i] }).is_some_and(|x| x <= e)).map(|i| wt(i, cls)).sum::<f64>()).sum::<f64>() / wsum * 100.0
    };
    let (mut e, last, mut out, mut prev) = (end_of(*a), end_of(*b), vec![], a.pred_opt().unwrap_or(*a));
    while e <= last && out.len() < 1500 {
        let n = |v: &dyn Fn(&DocRow) -> Option<NaiveDate>| docs.iter().filter(|d| v(d).is_some_and(|x| x > prev && x <= e)).count();
        out.push(Point { end: e, planned: at(e, false), actual: (e <= end_of(today) || e <= today).then(|| at(e.min(today), true)), first_planned: n(&|d| d.planned.first().copied().flatten()), first_done: n(&|d| d.actual.first().copied().flatten()) });
        prev = e;
        e = end_of(e.checked_add_days(Days::new(1)).unwrap_or(e));
        if e == prev { break }
    }
    out
}

/// Tidies a register before it is stored: classes 1–3, finite weights, unique ids, short texts,
/// submissions only for documents in the register.
pub fn clean(r: &mut DocRegister) {
    let t = |s: &mut String, n: usize| { if s.chars().count() > n { *s = s.chars().take(n).collect() } };
    r.review_days = r.review_days.clamp(0, 365);
    r.turnaround_days = r.turnaround_days.clamp(0, 365);
    if r.weights.len() != 6 || r.weights.iter().any(|s| s.len() != 3) { r.weights = DocRegister::default().weights }
    for s in &mut r.weights { for v in s { if !v.is_finite() || *v < 0.0 { *v = 0.0 } } }
    // Classes and steps: the register's own (made from the flow and weights the first time), in bounds.
    if r.classes.is_empty() { r.classes = r.classes() }
    r.classes.truncate(20);
    for (k, c) in r.classes.iter_mut().enumerate() {
        t(&mut c.name, 40);
        if c.name.trim().is_empty() { c.name = (k + 1).to_string() }
        c.steps.truncate(20);
        for s in &mut c.steps {
            for x in [&mut s.name, &mut s.purposes, &mut s.codes] { t(x, 200) }
            if !s.weight.is_finite() || s.weight < 0.0 { s.weight = 0.0 }
            s.days = s.days.map(|d| d.clamp(0, 3650));
        }
    }
    // Lists: named, without repeats, and holding every value the documents use.
    r.lists.truncate(40);
    for l in &mut r.lists {
        t(&mut l.name, 60); t(&mut l.field, 60);
        let mut seen = std::collections::HashSet::new();
        l.items.retain(|i| !i.value.trim().is_empty() && seen.insert(i.value.trim().to_lowercase()));
        l.items.truncate(2000);
        for i in &mut l.items { t(&mut i.value, 120); t(&mut i.label, 200); }
    }
    r.columns.retain(|c| !c.name.trim().is_empty());
    r.columns.truncate(40);
    for c in &mut r.columns { t(&mut c.name, 60); t(&mut c.formula, 2000); if !["text", "number", "date", "list", "formula"].contains(&c.kind.as_str()) { c.kind = "text".into() } }
    r.words.retain(|k, v| DOC_STATUS.iter().any(|x| x.0 == k) && !v.trim().is_empty());
    for v in r.words.values_mut() { t(v, 80) }
    let fields: Vec<String> = r.lists.iter().map(|l| l.field.clone()).collect();
    for f in fields {
        let used: Vec<String> = r.docs.iter().map(|d| match f.as_str() {
            "vendor" => d.vendor.clone(), "package" => d.package.clone(), "disc" => d.disc.clone(), "kind" => d.kind.clone(), "state" => d.state.clone(),
            own => d.own.get(own).cloned().unwrap_or_default(),
        }).collect();
        let Some(l) = r.lists.iter_mut().find(|l| l.field == f) else { continue };
        for v in used {
            let v = v.trim();
            if !v.is_empty() && !l.items.iter().any(|i| i.value.trim().eq_ignore_ascii_case(v)) && l.items.len() < 2000 { l.items.push(ListItem { value: v.into(), ..Default::default() }) }
        }
    }
    let names: std::collections::HashSet<String> = r.columns.iter().map(|c| c.name.clone()).collect();
    let nc = r.classes.len().max(1) as u8;
    r.docs.truncate(10_000);
    r.log.truncate(50_000);
    let mut seen = std::collections::HashSet::new();
    let mut next = r.docs.iter().map(|d| d.id).max().unwrap_or(0) + 1;
    for d in &mut r.docs {
        if d.id == 0 || !seen.insert(d.id) { d.id = next; next += 1; seen.insert(d.id); }
        d.class = d.class.clamp(1, nc);
        if !d.weight.is_finite() || d.weight < 0.0 { d.weight = 0.0 }
        if d.offset.is_some_and(|v| !v.is_finite() || !(-1000.0..=10_000.0).contains(&v)) { d.offset = None }
        for s in [&mut d.number, &mut d.title, &mut d.vendor, &mut d.package, &mut d.disc, &mut d.kind, &mut d.remarks, &mut d.state] { t(s, 300) }
        d.own.retain(|k, v| names.contains(k) && !v.is_empty());
        for v in d.own.values_mut() { t(v, 500) }
        d.flags.retain(|f| !f.is_empty()); d.flags.truncate(20);
    }
    let docs = seen;
    r.log.retain(|s| docs.contains(&s.doc));
    let mut seen = std::collections::HashSet::new();
    let mut next = r.log.iter().map(|s| s.id).max().unwrap_or(0) + 1;
    for s in &mut r.log {
        if s.id == 0 || !seen.insert(s.id) { s.id = next; next += 1; seen.insert(s.id); }
        for x in [&mut s.rev, &mut s.purpose, &mut s.transmittal, &mut s.ret_transmittal, &mut s.code, &mut s.crs] { t(x, 300) }
    }
}

/// The register and the transmittal log as a workbook (.xlsx) or an OpenDocument spreadsheet (.ods), by the file's extension.
pub fn write_book(p: &Project, o: &RegOut, path: &std::path::Path) -> Result<(), String> {
    use crate::io::book::{Cell, Sheet};
    let r = &p.documents;
    let num = |v: f64| Cell::Num((v * 10.0).round() / 10.0);
    let yes = |b: bool| Cell::from(if b { "Yes" } else { "" });
    let mut head: Vec<String> = ["No.", "Document No.", "Title", "Vendor", "Package", "Discipline", "Type", "Class", "Key", "Flags", "Weight", "In progress"].map(String::from).to_vec();
    let cls = r.classes();
    let ns = cls.iter().map(|c| c.steps.len()).max().unwrap_or(0);
    let names: Vec<String> = (0..ns).map(|i| cls.iter().find_map(|c| c.steps.get(i)).map(|s| s.name.clone()).unwrap_or_else(|| format!("Step {}", i + 1))).collect();
    head.extend(names.iter().map(|s| format!("Planned: {s}")));
    head.extend(names.iter().map(|s| format!("Actual: {s}")));
    head.extend(["Submissions", "Latest rev", "Latest purpose", "Latest sent", "Review due", "Returned", "Code", "Status", "1st issue late (d)", "Client late (d)", "Earned %", "Planned %", "Forecast final issue", "Remarks"].map(String::from));
    let mut rows = vec![head.into_iter().map(Cell::from).collect::<Vec<_>>()];
    for (i, (d, x)) in r.docs.iter().zip(&o.docs).enumerate() {
        let cname = cls.get(d.class.max(1) as usize - 1).map(|c| c.name.clone()).unwrap_or_else(|| d.class.to_string());
        let mut v: Vec<Cell> = vec![Cell::Num((i + 1) as f64), d.number.clone().into(), d.title.clone().into(), d.vendor.clone().into(), d.package.clone().into(), d.disc.clone().into(), d.kind.clone().into(), cname.parse::<f64>().map(Cell::Num).unwrap_or_else(|_| cname.into()), yes(d.key), d.flags.join(", ").into(), Cell::Num(d.weight), Cell::from(if d.progress && !d.recurring { "Yes" } else { "No" })];
        v.extend((0..ns).map(|k| Cell::from(x.planned.get(k).copied().flatten())));
        v.extend((0..ns).map(|k| Cell::from(x.actual.get(k).copied().flatten())));
        v.extend([Cell::Num(x.subs as f64), x.rev.clone().into(), x.purpose.clone().into(), x.sent.into(), x.due.into(), x.returned.into(), x.code.clone().into(), x.status.clone().into(), Cell::Num(x.late1 as f64), Cell::Num(x.client_late as f64), num(x.earned), num(x.planned_pct), x.finish.into(), d.remarks.clone().into()]);
        rows.push(v);
    }
    let mut widths = vec![12.0; rows[0].len()];
    (widths[1], widths[2]) = (28.0, 40.0);
    let mut log = vec![["Document No.", "Title", "Revision", "Purpose", "Transmittal", "Sent", "Review due", "Return transmittal", "Returned", "Code", "Comments ref.", "Accepted", "Days with client", "Days late", "State"].map(Cell::from).to_vec()];
    let byid: HashMap<u32, &VDoc> = r.docs.iter().map(|d| (d.id, d)).collect();
    for (s, x) in r.log.iter().zip(&o.subs) {
        let d = byid.get(&s.doc);
        log.push(vec![d.map(|d| d.number.clone()).unwrap_or_default().into(), d.map(|d| d.title.clone()).unwrap_or_default().into(), s.rev.clone().into(), s.purpose.clone().into(), s.transmittal.clone().into(), s.sent.into(), x.due.into(), s.ret_transmittal.clone().into(), s.returned.into(), s.code.clone().into(), s.crs.clone().into(), yes(x.accepted), Cell::Num(x.days as f64), Cell::Num(x.late as f64), x.state.clone().into()]);
    }
    let mut lw = vec![12.0; 15];
    (lw[0], lw[1]) = (28.0, 40.0);
    crate::io::book::write(path, &[Sheet { name: "Register".into(), widths, rows }, Sheet { name: "Transmittal Log".into(), widths: lw, rows: log }])
}

#[cfg(test)]
mod t {
    use super::*;
    use crate::doc::{Doc, Op};
    const F: &str = "%d/%m/%Y";
    fn d(y: i32, m: u32, dd: u32) -> NaiveDate { NaiveDate::from_ymd_opt(y, m, dd).unwrap() }
    fn sub(id: u32, doc: u32, rev: &str, purpose: &str, sent: NaiveDate, returned: Option<NaiveDate>, code: &str) -> Submission {
        Submission { id, doc, rev: rev.into(), purpose: purpose.into(), sent: Some(sent), returned, code: code.into(), ..Default::default() }
    }

    /// A Monday start, Monday–Friday calendar; P date Monday 2 Nov 2026, review 10 and turnaround 10 working days.
    fn reg() -> Doc {
        let mut doc = Doc::new(Project::new("Package", d(2026, 11, 2)));
        doc.apply(Op::Modules { on: vec!["documents".into()] }, F).unwrap();
        let mut r = DocRegister { p_date: Some(d(2026, 11, 2)), ..Default::default() };
        r.docs = vec![
            VDoc { id: 1, number: "DOC-001".into(), title: "General arrangement".into(), class: 1, offset: Some(2.0), weight: 1.0, progress: true, ..Default::default() },
            VDoc { id: 2, number: "DOC-002".into(), title: "Spare parts list".into(), class: 3, offset: Some(4.0), weight: 1.0, progress: true, ..Default::default() },
            VDoc { id: 3, number: "DOC-003".into(), title: "Monthly report".into(), class: 2, offset: Some(1.0), weight: 1.0, progress: true, recurring: true, ..Default::default() },
        ];
        doc.apply(Op::Documents { r: Box::new(r) }, F).unwrap();
        doc
    }

    #[test]
    fn planned_dates_follow_the_p_date_and_working_day_cycles() {
        let doc = reg();
        let o = status(&Calc::new(&doc.p, F, d(2026, 11, 2)));
        let a = &o.docs[0].planned;
        assert_eq!(a[0], Some(d(2026, 11, 16)), "P + 2 weeks");
        assert_eq!(a[1], Some(d(2026, 11, 30)), "+10 working days of review");
        assert_eq!(a[2], Some(d(2026, 12, 14)), "+10 working days of turnaround");
        assert_eq!(a[5], Some(d(2027, 2, 8)), "final issue: review + turnaround after the construction issue (11 Jan)");
        let b = &o.docs[1].planned;
        assert_eq!((b.len(), o.docs[1].steps[0].as_str()), (3, "Construction issue submitted"), "class 3 goes straight to the construction issue");
        assert_eq!(b[2], Some(d(2026, 12, 28)), "its final issue: review then turnaround (30 Nov + 10 + 10 working days)");
        assert!(o.weights_ok.len() == 3 && o.weights_ok.iter().all(|x| *x), "default weights add up to 100 for every class");
        assert_eq!(doc.p.documents.classes.len(), 3, "the classes are stored with the register");
    }

    #[test]
    fn the_log_drives_status_progress_and_client_delay() {
        let mut doc = reg();
        let mut r = doc.p.documents.clone();
        // Sent on time, back with code 3 after 20 calendar days (10 working days are due on 30 Nov), resent, then accepted.
        r.log = vec![
            sub(1, 1, "A", "ICR", d(2026, 11, 16), Some(d(2026, 12, 4)), "Code 3"),
            sub(2, 1, "B", "ICR", d(2026, 12, 11), Some(d(2026, 12, 18)), "Code 2"),
            sub(3, 1, "C", "IFA", d(2027, 1, 4), None, ""),
            sub(4, 99, "A", "ICR", d(2026, 12, 1), None, ""),
        ];
        doc.apply(Op::Documents { r: Box::new(r) }, F).unwrap();
        assert_eq!(doc.p.documents.log.len(), 3, "a submission for a document not in the register is dropped");
        let o = status(&Calc::new(&doc.p, F, d(2027, 1, 25)));
        let x = &o.docs[0];
        assert_eq!(x.actual[0], Some(d(2026, 11, 16)));
        assert_eq!(x.actual[1], Some(d(2026, 12, 18)), "accepted at the first accepted return");
        assert_eq!((x.subs, x.cycles, x.rev.as_str()), (3, 1, "C"));
        assert_eq!(x.due, Some(d(2027, 1, 18)), "10 working days after 4 Jan");
        assert_eq!(x.status, "Review overdue");
        assert_eq!(o.subs[0].late, 4, "returned 4 Dec, due 30 Nov");
        assert_eq!(o.subs[2].late, 7, "still with the client a week after it was due (to 25 Jan)");
        assert_eq!(x.client_late, 11);
        // Class 1 weights: 35 + 15 + 10 (IFA sent) = 60 %.
        assert!((x.earned - 60.0).abs() < 1e-9, "{}", x.earned);
        assert_eq!(o.docs[1].status, "Not submitted – overdue");
        // The recurring report is left out of progress: (60 + 0) / 2.
        assert!((o.earned - 30.0).abs() < 1e-9, "{}", o.earned);
        assert!(!o.weekly.is_empty() && o.weekly.iter().all(|p| p.end.weekday() == chrono::Weekday::Fri), "weeks end on the plan's week-end day");
        assert!(o.monthly.windows(2).all(|w| w[0].planned <= w[1].planned));
        assert_eq!(o.monthly.last().map(|p| p.planned.round()), Some(100.0));
        assert!(o.weekly.iter().filter(|p| p.end > d(2027, 1, 29)).all(|p| p.actual.is_none()) && o.weekly.iter().any(|p| p.end == d(2027, 1, 29) && p.actual.is_some()), "the actual line stops in the week of the data date");
    }

    /// A client of its own: Code 3 approves and Code 1 sends back, two classes named A and B,
    /// class A without an approval issue, class B with one, and an approval step that counts only
    /// the codes it names.
    #[test]
    fn classes_steps_and_codes_are_the_clients_own() {
        let mut doc = reg();
        let mut r = doc.p.documents.clone();
        let code = |c: &str, accepted: bool, resubmit: bool| ReturnCode { code: c.into(), meaning: String::new(), accepted, resubmit };
        r.codes = vec![code("Code 1", false, true), code("Code 2", true, false), code("Code 3", true, false)];
        let st = |name: &str, p: &str, returned: bool, codes: &str, weight: f64| DocStep { name: name.into(), purposes: p.into(), returned, codes: codes.into(), weight, days: None };
        r.classes = vec![
            DocClass { name: "A".into(), steps: vec![st("Issued", "IFR", false, "", 50.0), st("Approved", "IFR", true, "Code 3", 50.0)] },
            DocClass { name: "B".into(), steps: vec![st("Issued", "IFR", false, "", 40.0), st("Reviewed", "IFR", true, "", 20.0), st("Approval issued", "IFA", false, "", 20.0), DocStep { days: Some(5), ..st("Approved", "IFA", true, "Code 3", 20.0) }] },
        ];
        r.docs[0].class = 1;
        r.docs[1].class = 2;
        r.docs[2].class = 7;
        r.log = vec![
            sub(1, 1, "A", "IFR", d(2026, 11, 16), Some(d(2026, 11, 20)), "Code 1"),
            sub(2, 1, "B", "IFR", d(2026, 11, 23), Some(d(2026, 11, 27)), "Code 2"),
            sub(3, 1, "C", "IFR", d(2026, 11, 30), Some(d(2026, 12, 4)), "Code 3"),
            sub(4, 2, "A", "IFR", d(2026, 11, 30), Some(d(2026, 12, 2)), "Code 2"),
        ];
        doc.apply(Op::Documents { r: Box::new(r) }, F).unwrap();
        assert_eq!(doc.p.documents.docs[2].class, 2, "a class beyond the register's comes back to the last one");
        let o = status(&Calc::new(&doc.p, F, d(2026, 12, 7)));
        assert_eq!(o.classes, ["A", "B"]);
        let a = &o.docs[0];
        assert_eq!(a.steps, ["Issued", "Approved"]);
        assert_eq!(a.actual, [Some(d(2026, 11, 16)), Some(d(2026, 12, 4))], "Code 1 sends it back and Code 2 does not count for this step: only Code 3 approves");
        assert_eq!((a.cycles, a.done, a.status.as_str()), (1, true, "Final issued"), "Code 1 is this client's resubmission");
        assert!((a.earned - 100.0).abs() < 1e-9);
        let b = &o.docs[1];
        assert_eq!(b.actual[1], Some(d(2026, 12, 2)), "a step without codes counts any code that accepts");
        assert_eq!(b.planned[3], b.planned[2].map(|x| add_wd(&WorkCal::new(doc.p.cal(None)), x, 5)), "a step's own days");
        assert!((b.earned - 60.0).abs() < 1e-9, "40 + 20: {}", b.earned);
        assert_eq!(b.approved, None, "approved when the approval step is done, not before");
        assert!(o.weights_ok.iter().all(|x| *x));
    }

    #[test]
    fn review_days_count_on_their_own_calendar() {
        let mut doc = reg();
        let mut cal = Calendar::standard("Client week");
        cal.week[0] = vec![];
        cal.week[4] = vec![];
        cal.week[6] = vec![(480, 1020)];
        doc.apply(Op::CalPut { old: None, cal }, F).unwrap();
        let mut r = doc.p.documents.clone();
        r.review_cal = "Client week".into();
        doc.apply(Op::Documents { r: Box::new(r) }, F).unwrap();
        let o = status(&Calc::new(&doc.p, F, d(2026, 11, 2)));
        // Sunday and Tuesday–Thursday: 10 working days after Mon 16 Nov is Wed 2 Dec (30 Nov on the plan's calendar).
        assert_eq!(o.docs[0].planned[1], Some(d(2026, 12, 2)));
    }
}
