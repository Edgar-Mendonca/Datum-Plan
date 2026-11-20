#![allow(clippy::field_reassign_with_default)] // test setups read best one field at a time
//! Audit of the register modules (docreg, procure, contract, regtable, board, assist register tools).
//! Each test hand-works the expected value; a failing test is a confirmed bug.
use chrono::NaiveDate;
use datum_core::doc::{Doc, Op};
use datum_core::fields::Calc;
use datum_core::model::*;
use datum_core::{assist, contract, docreg, procure};
use serde_json::json;

const F: &str = "%Y-%m-%d";
fn d(y: i32, m: u32, dd: u32) -> NaiveDate { NaiveDate::from_ymd_opt(y, m, dd).unwrap() }

/// An empty plan starting Monday 2 Nov 2026, with the given document register.
fn docs_plan(r: DocRegister) -> Project {
    let mut p = Project::new("Audit", d(2026, 11, 2));
    p.modules = vec!["documents".into()];
    p.documents = r;
    p
}
fn vdoc(id: u32) -> VDoc { VDoc { id, number: format!("D-{id}"), class: 1, offset: Some(0.0), weight: 1.0, progress: true, ..Default::default() } }

// ---------------------------------------------------------------- regtable::own_eval

/// The documented example of an own formula column, `=[Review due]-TODAY()`, is a number of days.
/// own_eval shifts every date column by 1000 days and calls a result that moves by 1000 a date:
/// `[Review due] - <constant>` moves by exactly 1000, so the day count is shown as a date.
/// Sent Tue 10 Nov 2026, review 10 working days → due Tue 24 Nov 2026; minus 13 Nov = 11 days.
#[test]
fn own_formula_days_left_is_a_number_not_a_date() {
    let mut r = DocRegister::default();
    r.docs = vec![vdoc(1)];
    r.log = vec![Submission { id: 1, doc: 1, rev: "A".into(), purpose: "ICR".into(), sent: Some(d(2026, 11, 10)), ..Default::default() }];
    r.columns = vec![OwnCol { name: "Days left".into(), kind: "formula".into(), formula: "=[Review due]-DATE(2026,11,13)".into() }];
    let p = docs_plan(r);
    let o = docreg::status(&Calc::new(&p, F, d(2026, 11, 13)));
    assert_eq!(o.docs[0].due, Some(d(2026, 11, 24)));
    assert_eq!(o.docs[0].own[0], "11", "a day count, not a date");
}

/// Same with TODAY(), the exact example in the tool description: the result must be a plain number.
#[test]
fn own_formula_review_due_minus_today_is_a_number() {
    let mut r = DocRegister::default();
    r.docs = vec![vdoc(1)];
    r.log = vec![Submission { id: 1, doc: 1, rev: "A".into(), purpose: "ICR".into(), sent: Some(d(2026, 11, 10)), ..Default::default() }];
    r.columns = vec![OwnCol { name: "Left".into(), kind: "formula".into(), formula: "=[Review due]-TODAY()".into() }];
    let p = docs_plan(r);
    let o = docreg::status(&Calc::new(&p, F, d(2026, 11, 13)));
    let v = &o.docs[0].own[0];
    assert!(v.parse::<f64>().is_ok(), "days left should be a number, got {v:?}");
}

// ---------------------------------------------------------------- contract::next_due

/// A monthly obligation due on the 31st: 31 Jan → 28 Feb → 31 Mar. Stepping from the clamped
/// 28 Feb gives 28 Mar (the month end is lost for good).
#[test]
fn monthly_obligation_keeps_the_month_end() {
    let o = Obligation { due: Some(d(2026, 1, 31)), every: "month".into(), done: Some(d(2026, 2, 28)), ..Default::default() };
    assert_eq!(contract::next_due(&o), Some(d(2026, 3, 31)));
}

/// Quarterly from 31 Jan: 30 Apr, then 31 Jul (not 30 Jul).
#[test]
fn quarterly_obligation_keeps_the_month_end() {
    let o = Obligation { due: Some(d(2026, 1, 31)), every: "quarter".into(), done: Some(d(2026, 5, 1)), ..Default::default() };
    assert_eq!(contract::next_due(&o), Some(d(2026, 7, 31)));
}

/// Leap year: monthly from 29 Feb 2028 done 31 Mar → 29 Apr (both chained and anchored agree), sanity.
#[test]
fn monthly_obligation_from_leap_day() {
    let o = Obligation { due: Some(d(2028, 2, 29)), every: "month".into(), done: Some(d(2028, 3, 30)), ..Default::default() };
    assert_eq!(contract::next_due(&o), Some(d(2028, 4, 29)));
}

// ---------------------------------------------------------------- contract::clean ids

/// Removing payment 1 (its invoice keeps `payment: 1`) and adding a new payment: clean() hands the
/// new payment the smallest free id, 1, so it silently inherits the old paid invoice and shows
/// "Paid" though nothing billed it. Expected: a fresh id (max + 1, as docreg/procure do) → "No task".
#[test]
fn a_new_payment_does_not_inherit_a_removed_payments_invoices() {
    let mut doc = Doc::new(Project::new("Audit", d(2026, 11, 2)));
    doc.apply(Op::Modules { on: vec!["contract".into()] }, F).unwrap();
    let k = Contract {
        value: 1000.0,
        payments: vec![Payment { id: 1, name: "Advance".into(), pct: 10.0, ..Default::default() }, Payment { id: 2, name: "Final".into(), pct: 90.0, ..Default::default() }],
        invoices: vec![Invoice { id: 1, no: "INV-1".into(), payment: 1, amount: 100.0, invoiced: Some(d(2026, 11, 3)), paid: Some(d(2026, 11, 5)), ..Default::default() }],
        ..Default::default()
    };
    doc.apply(Op::Contract { c: Box::new(k) }, F).unwrap();
    let t = d(2026, 11, 13);
    assist::call(&mut doc, F, t, "set_payment", &json!({ "id": 1, "remove": true })).unwrap();
    let id = assist::call(&mut doc, F, t, "set_payment", &json!({ "fields": { "name": "Retention release", "pct": 5 } })).unwrap()["id"].as_u64().unwrap();
    let o = contract::status(&Calc::new(&doc.p, F, t));
    let pay = o.payments.iter().find(|x| u64::from(x.id) == id).unwrap();
    assert_ne!(id, 1, "a removed payment's id is reused");
    assert_eq!(pay.status, "No task", "the new payment is not paid");
}

// ---------------------------------------------------------------- procure earned

/// An order delivered (RFQ, PO, IRN, delivery recorded) but with no manufacturing-start date:
/// status() sets mfg_now = 1.0 for IRN/delivered, then only uses it when a[2] is set, so the 60 %
/// manufacturing weight is lost. Balanced preset: expected 100 %, actual 5+15+10+10 = 40 %.
#[test]
fn a_delivered_order_has_earned_everything() {
    let mut p = Project::new("Audit", d(2026, 11, 2));
    p.modules = vec!["procurement".into()];
    p.procurement.items = vec![PItem { id: 1, tag: "P-1".into(), value: 10.0,
        actual: [Some(d(2026, 11, 2)), Some(d(2026, 11, 3)), None, Some(d(2026, 11, 4)), Some(d(2026, 11, 5))], ..Default::default() }];
    let o = procure::status(&Calc::new(&p, F, d(2026, 11, 13)));
    assert_eq!(o.items[0].skey, "delivered");
    assert!((o.items[0].earned - 100.0).abs() < 1e-9, "earned {}", o.items[0].earned);
}

// ---------------------------------------------------------------- docreg::next_sub

fn reg_with_last(rev: &str, code: &str, returned: bool) -> (DocRegister, VDoc) {
    let mut r = DocRegister::default();
    r.docs = vec![vdoc(1)];
    r.log = vec![Submission { id: 1, doc: 1, rev: rev.into(), purpose: "ICR".into(), sent: Some(d(2026, 11, 3)),
        returned: returned.then(|| d(2026, 11, 10)), code: code.into(), ..Default::default() }];
    let v = r.docs[0].clone();
    (r, v)
}

/// Revisions "01" → "02": the number keeps its width (as next_transmittal does: TR-0009 → TR-0010).
#[test]
fn next_revision_keeps_leading_zeros() {
    let (r, v) = reg_with_last("01", "", false);
    assert_eq!(docreg::next_sub(&r, &v, d(2026, 11, 13)).rev, "02");
}

/// After rev "Z" the next revision must differ from "Z" (else two submissions share rev Z).
#[test]
fn next_revision_after_z_is_new() {
    let (r, v) = reg_with_last("Z", "", false);
    assert_ne!(docreg::next_sub(&r, &v, d(2026, 11, 13)).rev, "Z");
}

/// A return coded "code 3" (typed in lower case): status() matches codes case-insensitively and
/// calls it "resubmit", so the next submission must be the same purpose again (ICR). next_sub
/// compares codes exactly, does not see a resubmit and proposes the next issue step (IFA).
#[test]
fn next_sub_matches_return_codes_like_status_does() {
    let (r, v) = reg_with_last("A", "code 3", true);
    let p = docs_plan(r.clone());
    let o = docreg::status(&Calc::new(&p, F, d(2026, 11, 13)));
    assert_eq!(o.docs[0].skey, "resubmit");
    assert_eq!(docreg::next_sub(&r, &v, d(2026, 11, 13)).purpose, "ICR");
}

// ---------------------------------------------------------------- docreg::status

/// A Deleted document is "not counted" (skey off, weight 0), yet its client delay goes into the
/// register's total client_late. Sent Tue 3 Nov, due +10 wd = Tue 17 Nov, returned Fri 27 Nov
/// → 10 days late. Expected total 0 for the only (deleted) document.
#[test]
fn a_deleted_document_does_not_count_in_client_delay() {
    let mut r = DocRegister::default();
    r.docs = vec![VDoc { state: "Deleted".into(), ..vdoc(1) }];
    r.log = vec![Submission { id: 1, doc: 1, rev: "A".into(), purpose: "ICR".into(), sent: Some(d(2026, 11, 3)), returned: Some(d(2026, 11, 27)), code: "Code 1".into(), ..Default::default() }];
    let p = docs_plan(r);
    let o = docreg::status(&Calc::new(&p, F, d(2026, 12, 1)));
    assert_eq!(o.docs[0].skey, "off");
    assert_eq!(o.docs[0].client_late, 10);
    assert_eq!(o.client_late, 0, "documents not counted stay out of the totals");
}

/// clean() keeps P offsets down to -1000 (P - 2 weeks is a valid first issue), but status() clamps
/// them to 0: the planned first issue lands on the P date. P = Mon 16 Nov 2026, offset -2 weeks →
/// Mon 2 Nov 2026.
#[test]
fn a_negative_p_offset_plans_before_the_p_date() {
    let mut r = DocRegister { p_date: Some(d(2026, 11, 16)), ..Default::default() };
    r.docs = vec![VDoc { offset: Some(-2.0), ..vdoc(1) }];
    let mut doc = Doc::new(docs_plan(DocRegister::default()));
    doc.apply(Op::Documents { r: Box::new(r) }, F).unwrap();
    assert_eq!(doc.p.documents.docs[0].offset, Some(-2.0), "clean keeps the offset");
    let o = docreg::status(&Calc::new(&doc.p, F, d(2026, 11, 2)));
    assert_eq!(o.docs[0].planned[0], Some(d(2026, 11, 2)));
}

// ---------------------------------------------------------------- checks that pass (verified correct)

/// Planned chain of class 1 from P + 2 weeks, review 10 wd, turnaround 10 wd (Mon–Fri):
/// ICR 16 Nov → return 30 Nov → IFA 14 Dec → return 28 Dec → IFC1 11 Jan → IFC2 (rv+tn) 8 Feb 2027.
#[test]
fn ok_planned_chain_class_1() {
    let mut r = DocRegister::default();
    r.docs = vec![VDoc { offset: Some(2.0), ..vdoc(1) }];
    let p = docs_plan(r);
    let o = docreg::status(&Calc::new(&p, F, d(2026, 11, 2)));
    assert_eq!(o.docs[0].planned, vec![Some(d(2026, 11, 16)), Some(d(2026, 11, 30)), Some(d(2026, 12, 14)), Some(d(2026, 12, 28)), Some(d(2027, 1, 11)), Some(d(2027, 2, 8))]);
    assert!(o.weights_ok.iter().all(|x| *x));
}

#[test]
fn ok_next_transmittal_and_revisions() {
    let mut r = DocRegister::default();
    r.log = vec![Submission { transmittal: "TR-0009".into(), ..Default::default() }, Submission { transmittal: "TR-0012".into(), ..Default::default() }];
    assert_eq!(docreg::next_transmittal(&r), "TR-0013");
    for (a, b) in [("A", "B"), ("0", "1"), ("B1", "B2"), ("C9", "C10")] {
        let (r, v) = reg_with_last(a, "", false);
        assert_eq!(docreg::next_sub(&r, &v, d(2026, 11, 13)).rev, b);
    }
}

#[test]
fn ok_weekly_buckets_end_on_the_week_end() {
    let mut r = DocRegister::default();
    r.docs = vec![VDoc { offset: Some(2.0), ..vdoc(1) }];
    let p = docs_plan(r); // week_end 4 = Friday
    let o = docreg::status(&Calc::new(&p, F, d(2026, 11, 2)));
    assert_eq!(o.weekly[0].end, d(2026, 11, 20));
    assert!(o.weekly.iter().all(|x| x.end.format("%a").to_string() == "Fri"));
    assert_eq!(o.monthly.last().unwrap().end, d(2027, 2, 28));
    assert!((o.weekly.last().unwrap().planned - 100.0).abs() < 1e-9);
}
