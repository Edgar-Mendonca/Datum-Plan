//! Audit of file exchange and sheets (integration tests through the public API).
use chrono::NaiveDate;
use datum_core::io::{export, import, Imported};
use datum_core::*;
use std::path::PathBuf;

fn d(y: i32, m: u32, day: u32) -> NaiveDate { NaiveDate::from_ymd_opt(y, m, day).unwrap() }
const FMT: &str = "%Y-%m-%d";

fn dir() -> PathBuf {
    let p = PathBuf::from("/tmp/claude-0/-home-user/a3b85871-70f1-5aec-a1bf-670055f8001a/scratchpad/audit_io");
    std::fs::create_dir_all(&p).unwrap();
    p
}
fn file(name: &str) -> PathBuf {
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    dir().join(format!("{}-{}-{name}", std::process::id(), N.fetch_add(1, std::sync::atomic::Ordering::Relaxed)))
}
fn out(p: &Project, ext: &str) -> (PathBuf, String) {
    let f = file(&format!("plan.{ext}"));
    export(p, &f, FMT, d(2026, 11, 1), &[]).unwrap();
    let s = String::from_utf8_lossy(&std::fs::read(&f).unwrap()).to_string();
    (f, s)
}
fn rt(p: &Project, ext: &str) -> Project {
    let (f, _) = out(p, ext);
    match import(&f).unwrap() { Imported::Plan(q, _) => *q, _ => panic!("not a plan") }
}
fn rich() -> Project {
    let mut p = templates::build(&templates::TEMPLATES[0], d(2026, 11, 2));
    // A holiday on the calendar, a Chinese name with markup characters, notes with lines and tabs.
    p.calendars[0].exceptions.push(Exception { name: "Holiday & \"rest\"".into(), from: d(2026, 12, 24), to: d(2026, 12, 26), hours: vec![] });
    p.tasks[1].name = "启动会议 & <kick-off> \"A\"".into();
    p.tasks[1].notes = "Line 1\nLine 2\twith tab & <b>".into();
    // Constraints, a deadline, link lags of every kind.
    p.tasks[4].constraint = Constraint::SNET; p.tasks[4].cdate = Some(d(2026, 11, 23));
    p.tasks[12].constraint = Constraint::FNLT; p.tasks[12].cdate = Some(d(2027, 3, 31));
    p.tasks[11].deadline = Some(d(2027, 2, 26));
    let u = p.tasks[16].uid;
    p.tasks[17].preds = vec![Link { pred: u, kind: LinkType::FS, lag: 2 * 480, pct: false, elapsed: false }];
    let u = p.tasks[17].uid;
    p.tasks[19].preds.push(Link { pred: u, kind: LinkType::FF, lag: -480, pct: false, elapsed: false });
    // A baseline, then progress through 20 Nov with a status date.
    let mut doc = doc::Doc::new(p);
    doc.apply(doc::Op::Baseline { uids: vec![], clear: false, n: 0 }, FMT).unwrap();
    doc.apply(doc::Op::UpdateProject { through: "2026-11-20".into(), reschedule: false }, FMT).unwrap();
    doc.apply(doc::Op::ProjectSet { field: "status".into(), value: serde_json::json!("2026-11-20") }, FMT).unwrap();
    doc.p
}

// ------------------------------------------------------------------------------------------------
// XER
// ------------------------------------------------------------------------------------------------

/// The writer puts each resource's max units in RSRC.def_qty_per_hr (and RSRCRATE.max_qty_per_hr), but
/// the reader never reads them: a crew of 2 (Design Engineer, max 200 %) comes back as 1 person, and
/// leveling / over-allocation change after a round trip.
#[test]
fn xer_round_trip_keeps_resource_max_units() {
    let p = rich();
    let (_, text) = out(&p, "xer");
    assert!(text.lines().any(|l| l.starts_with("%R\t") && l.contains("Design Engineer") && l.contains("\t2\t")), "the file holds 2 for the crew");
    let q = rt(&p, "xer");
    let max = |p: &Project| p.resources.iter().map(|r| (r.name.clone(), r.max)).collect::<Vec<_>>();
    assert_eq!(max(&q), max(&p));
}

/// Written into RSRC (rsrc_title_name = group, email_addr, rsrc_short_name = initials) but not read back.
#[test]
fn xer_round_trip_keeps_resource_group_and_initials() {
    let mut p = rich();
    p.resources[0].email = "pm@example.com".into();
    let q = rt(&p, "xer");
    let f = |p: &Project| p.resources.iter().map(|r| (r.group.clone(), r.initials.clone(), r.email.clone())).collect::<Vec<_>>();
    assert_eq!(f(&q), f(&p));
}

/// A finish constraint's date-time in P6 is the moment the activity must finish by. The writer puts
/// "2027-03-31 00:00" for "finish no later than 31 Mar 2027" (and for MFO/FNET), which in P6 means
/// finish by the *start* of 31 March, i.e. one working day earlier than the plan says. A finish
/// constraint should carry the end of the working day (17:00 on the standard calendar), as the
/// deadline (cstr_date2, "23:59") already does.
#[test]
fn xer_finish_constraint_is_written_at_end_of_day() {
    let mut p = rich();
    let i = p.tasks.iter().position(|t| t.name == "Design FMEA").unwrap();
    p.tasks[i].constraint = Constraint::FNLT;
    p.tasks[i].cdate = Some(d(2027, 3, 31));
    let (_, text) = out(&p, "xer");
    let fields: Vec<&str> = text.lines().skip_while(|l| *l != "%T\tTASK").nth(1).unwrap().split('\t').skip(1).collect();
    let row: Vec<&str> = text.lines().find(|l| l.starts_with("%R\t") && l.contains("Design FMEA")).unwrap().split('\t').skip(1).collect();
    let get = |k: &str| row[fields.iter().position(|f| *f == k).unwrap()];
    assert_eq!(get("cstr_type"), "CS_MEOB");
    assert_eq!(get("cstr_date"), "2027-03-31 17:00", "a finish constraint at 00:00 is a day early in P6");
}

/// P6 writes XER files in the database code page, Windows-1252 by default (MPXJ also reads XER as
/// Windows-1252 unless told otherwise). The importer decodes the bytes as UTF-8 (lossy), so every
/// accented letter in a P6 file becomes U+FFFD: "Café" → "Caf\u{FFFD}".
#[test]
fn xer_in_windows_1252_keeps_accents() {
    let mut p = Project::new("Bau", d(2026, 11, 2));
    let t = p.new_task("Cafe", 0, 480);
    p.tasks.push(t);
    let (_, text) = out(&p, "xer");
    // The same file as P6 would save it: "Café" with é = 0xE9.
    let mut bytes = vec![];
    for part in text.split("Cafe").enumerate().flat_map(|(k, s)| if k == 0 { vec![s.as_bytes().to_vec()] } else { vec![b"Caf\xE9".to_vec(), s.as_bytes().to_vec()] }) { bytes.extend(part) }
    let f = file("p6.xer");
    std::fs::write(&f, bytes).unwrap();
    let Imported::Plan(q, _) = import(&f).unwrap() else { panic!() };
    assert_eq!(q.tasks.iter().find(|t| t.level == 0 && t.name.starts_with("Caf")).map(|t| t.name.clone()), Some("Café".to_string()));
}

// ------------------------------------------------------------------------------------------------
// Project XML (MSPDI)
// ------------------------------------------------------------------------------------------------

/// The reader takes <Initials>, <EmailAddress> and <OvertimeRate> from a resource, but the writer never
/// writes them: a Datum Plan → XML → Datum Plan round trip drops every resource's initials (all six in
/// the engineering template), e-mail and overtime rate.
#[test]
fn xml_round_trip_keeps_resource_initials_email_and_overtime() {
    let mut p = rich();
    p.resources[0].email = "pm@example.com".into();
    p.resources[0].ovt = 140.0;
    let q = rt(&p, "xml");
    let f = |p: &Project| p.resources.iter().map(|r| (r.initials.clone(), r.email.clone(), r.ovt)).collect::<Vec<_>>();
    assert_eq!(f(&q), f(&p));
}

/// MS Project reads a ConstraintDate as a date and time. The writer gives every constraint 08:00, so
/// "finish no later than 31 Mar 2027" becomes "finish by 31 Mar 08:00", which MS Project meets only by
/// finishing on 30 March: one day early. A finish constraint (FNET/FNLT/MFO) needs the end of the
/// working day, as <Deadline> (17:00) already has.
#[test]
fn xml_finish_constraint_is_written_at_end_of_day() {
    let mut p = rich();
    let i = p.tasks.iter().position(|t| t.name == "Design FMEA").unwrap();
    p.tasks[i].constraint = Constraint::FNLT;
    p.tasks[i].cdate = Some(d(2027, 3, 31));
    let (_, text) = out(&p, "xml");
    let task = text.split("<Task>").find(|t| t.contains("<Name>Design FMEA</Name>")).unwrap();
    assert!(task.contains("<ConstraintType>7</ConstraintType>"));
    assert!(task.contains("<ConstraintDate>2027-03-31T17:00:00</ConstraintDate>"), "{}", &task[task.find("<ConstraintDate>").unwrap()..][..50]);
}

// ------------------------------------------------------------------------------------------------
// MPX and PMXML (export only)
// ------------------------------------------------------------------------------------------------

/// MPX 4.0 records: 10 Currency Settings (symbol, position, digits, thousands separator, decimal
/// separator), 11 Default Settings (duration units, duration type, work units, hours per day, hours
/// per week, std rate, ovt rate, ...), 12 Date and Time Settings (date order, time format, default
/// time, date separator, ...), 30 Project Header (title, company, manager, calendar, start date,
/// finish date, ...). The writer puts the hours per day/week in record 12 (as "default time" and
/// "date separator"), leaves 0 hours per day in record 11, writes "0" as both number separators in
/// record 10, and puts the start date in the Calendar field of record 30 (the finish in Start Date).
#[test]
fn mpx_header_records_follow_the_format() {
    let mut p = rich();
    p.day_min = 600; // 10-hour days: the reader needs this to read "5d" right
    let (_, text) = out(&p, "mpx");
    // Fields split as MPX quotes them: a separator inside quotes (the "," thousands separator) is text.
    let rec = |n: &str| -> Vec<String> {
        let line = text.lines().find(|l| l.starts_with(&format!("{n},"))).unwrap().trim_end_matches('\r');
        let (mut out, mut cur, mut q) = (vec![], String::new(), false);
        for ch in line.chars() { match ch { '"' => q = !q, ',' if !q => out.push(std::mem::take(&mut cur)), c => cur.push(c) } }
        out.push(cur);
        out
    };
    let (r10, r11, r30) = (rec("10"), rec("11"), rec("30"));
    let s = sched::schedule(&p).unwrap();
    let start = datum_core::cal::date_of(s.start).format("%m/%d/%Y").to_string();
    let got = [(r10[4].clone(), r10[5].clone()), (r11[4].clone(), r11[5].clone()), (r30[4].clone(), r30[5].clone())];
    // record 10 separators; record 11 hours per day and week; record 30 calendar name and start date.
    let want = [(",".to_string(), ".".to_string()), ("10".into(), "50".into()), ("Standard".into(), start)];
    assert_eq!(got, want, "records: {r10:?} {r11:?} {:?} {r30:?}", rec("12"));
}

/// The P6 XML API (PMXML) holds percentages as fractions 0–1 (MPXJ multiplies them by 100 on reading
/// and divides on writing). A task 50 % complete is written <PhysicalPercentComplete>50</...>, i.e. 5000 %.
#[test]
fn pmxml_percent_complete_is_a_fraction() {
    let mut p = Project::new("P", d(2026, 11, 2));
    let t = p.new_task("Half", 0, 2400);
    p.tasks.push(t);
    p.tasks[0].pct = 50;
    p.tasks[0].act_start = Some(d(2026, 11, 2));
    let (_, text) = out(&p, "pmxml");
    assert!(text.contains("<PhysicalPercentComplete>0.5</PhysicalPercentComplete>"), "{}", text.lines().find(|l| l.contains("PhysicalPercentComplete")).unwrap_or(""));
}

// ------------------------------------------------------------------------------------------------
// Sheets
// ------------------------------------------------------------------------------------------------

fn with_sheet(cells: &[(&str, &str)]) -> Project {
    let mut p = Project::new("S", d(2026, 11, 2));
    let t = p.new_task("T1", 0, 960);
    p.tasks.push(t);
    p.sheets.push(SheetData { name: "Sheet1".into(), cells: cells.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect(), ..Default::default() });
    p
}
fn shown(p: &Project, a1: &str) -> (String, Option<f64>) {
    let c = fields::Calc::new(p, "%d/%m/%Y", d(2026, 11, 2));
    let v = sheet::sheet_view(&c, 0).unwrap();
    v.cells.iter().find(|x| x.0 == a1).map(|x| (x.2.clone(), x.4)).unwrap_or_default()
}

/// A date typed in a cell becomes a serial counted back from 1970-01-01 = 25569. Before 1 March 1900
/// that is one too many, because the 1900 date system has the fictitious 29 Feb 1900 at serial 60
/// (1900-01-01 = 1, 1900-02-28 = 59, 1900-03-01 = 61). So 1 Jan 1900 typed in a cell is serial 2 and
/// DAY() of it is 2; 28 Feb 1900 typed becomes serial 60 and shows as 29 Feb 1900.
#[test]
fn sheet_typed_dates_before_march_1900() {
    let p = with_sheet(&[("A1", "1900-01-01"), ("B1", "=DAY(A1)"), ("A2", "1900-02-28"), ("B2", "=MONTH(A2)*100+DAY(A2)"), ("A3", "1900-03-01"), ("B3", "=A3")]);
    // 1900-03-01 is right (61): the error is only before it.
    assert_eq!(shown(&p, "B3").1, Some(61.0));
    assert_eq!((shown(&p, "B1").1, shown(&p, "B2").1), (Some(1.0), Some(228.0)));
}

/// Tasks[Duration] is "days, as the task sheet shows them": working minutes / minutes per day. For an
/// elapsed task (3 edays, shown as "3 edays") the minutes are clock minutes, so the sheet gets
/// 4320 / 480 = 9 days. The workbook export writes the same 9 ("9 d") in its Duration column.
#[test]
fn sheet_duration_of_an_elapsed_task() {
    let mut p = with_sheet(&[("A1", "=INDEX(Tasks[Duration],1)")]);
    p.tasks[0].dur = 3 * 1440;
    p.tasks[0].elapsed = true;
    let c = fields::Calc::new(&p, "%d/%m/%Y", d(2026, 11, 2));
    assert_eq!(c.text(0, "dur"), "3 edays");
    assert_eq!(shown(&p, "A1").1, Some(3.0), "Tasks[Duration] of a 3-eday task");
}

/// Same in the workbook (.xlsx) export: the Duration column holds 9 for a task the plan shows as 3 edays.
#[test]
fn workbook_duration_of_an_elapsed_task() {
    let mut p = with_sheet(&[]);
    p.sheets.clear();
    p.tasks[0].dur = 3 * 1440;
    p.tasks[0].elapsed = true;
    let f = file("el.xlsx");
    export(&p, &f, FMT, d(2026, 11, 1), &["name".into(), "dur".into()]).unwrap();
    let (_, rows) = io::table::read_sheet(&f, None).unwrap();
    assert_eq!(rows[1][1], "3", "Duration cell of the 3-eday task");
}

// ------------------------------------------------------------------------------------------------
// Classic formulas (formula.rs)
// ------------------------------------------------------------------------------------------------

struct NoRow;
impl formula::Ctx for NoRow {
    fn field(&self, _: &str) -> Option<formula::F> { None }
    fn now(&self) -> i64 { 0 }
    fn work_between(&self, a: i64, b: i64) -> i64 { b - a }
    fn work_add(&self, a: i64, m: i64) -> i64 { a + m }
}
fn calc(src: &str) -> formula::F { formula::eval(&formula::parse(src).unwrap(), &NoRow).unwrap() }

/// Exponentiation in VBA-style expressions (and in Excel) is left-associative: 2^3^2 = (2^3)^2 = 64.
/// The parser makes it right-associative: 2^(3^2) = 512.
#[test]
fn formula_power_is_left_associative() {
    assert_eq!(calc("2^3^2"), formula::F::Num(64.0));
}

// ------------------------------------------------------------------------------------------------
// Workbook export with live formulas
// ------------------------------------------------------------------------------------------------

fn zip_part(path: &std::path::Path, name: &str) -> String {
    use std::io::Read;
    let mut z = zip::ZipArchive::new(std::fs::File::open(path).unwrap()).unwrap();
    let mut s = String::new();
    if let Ok(mut e) = z.by_name(name) { e.read_to_string(&mut s).unwrap(); }
    s
}

/// A formula column may read the plan's names (=[Cost]/TotalCost) and the Tasks table; the app works
/// them out. The .xlsx export writes the column as a live formula ("=M2/TotalCost") but defines the
/// names and the Tasks table only when the plan has sheet tabs, so in a plan without tabs the
/// exported cells are #NAME? in the spreadsheet while the app shows the share.
#[test]
fn workbook_formula_column_reading_plan_names_without_sheets() {
    let mut p = rich();
    p.fields.push(CustomDef { key: "number1".into(), alias: "Share".into(), formula: "=[Cost]/TotalCost".into(), ..Default::default() });
    let c = fields::Calc::new(&p, FMT, d(2026, 11, 1));
    let i = p.tasks.iter().position(|t| t.name == "Order prototype parts").unwrap();
    let share = c.val(i, "c:number1").num();
    assert!(share > 0.0 && share < 1.0, "the app works the share out: {share}");
    let f = file("names.xlsx");
    export(&p, &f, FMT, d(2026, 11, 1), &["name".into(), "cost".into(), "c:number1".into()]).unwrap();
    let sheet1 = zip_part(&f, "xl/worksheets/sheet1.xml");
    assert!(sheet1.contains("TotalCost"), "the cell is a live formula reading TotalCost");
    let wbx = zip_part(&f, "xl/workbook.xml");
    assert!(wbx.contains("name=\"TotalCost\""), "the workbook must define TotalCost for that formula; definedNames: {:?}", wbx.find("definedNames"));
}

// ------------------------------------------------------------------------------------------------
// Custom fields: roll-ups
// ------------------------------------------------------------------------------------------------

/// The Custom Fields dialog offers Max/Min/Sum/Average/Count roll-ups for every kind, flags included.
/// A flag counts as -1 when Yes (F::num, the classic True = -1), so "Rollup: Max" over a summary whose
/// tasks are Yes and No gives max(-1, 0) = 0 = No, and Min gives Yes: the opposite of "any task is
/// Yes" (Max) and "all tasks are Yes" (Min).
#[test]
fn flag_rollup_max_is_any_yes() {
    let mut p = Project::new("F", d(2026, 11, 2));
    for (n, l) in [("Phase", 0), ("A", 1), ("B", 1)] { let t = p.new_task(n, l, 480); p.tasks.push(t); }
    p.tasks[1].custom.insert("flag1".into(), serde_json::json!(true));
    p.tasks[2].custom.insert("flag1".into(), serde_json::json!(false));
    p.fields.push(CustomDef { key: "flag1".into(), alias: "Hold".into(), rollup: Rollup::Max, ..Default::default() });
    let c = fields::Calc::new(&p, FMT, d(2026, 11, 2));
    assert_eq!(c.text(0, "c:flag1"), "Yes", "Max over Yes and No");
    p.fields[0].rollup = Rollup::Min;
    let c = fields::Calc::new(&p, FMT, d(2026, 11, 2));
    assert_eq!(c.text(0, "c:flag1"), "No", "Min over Yes and No");
}

// ------------------------------------------------------------------------------------------------
// Workbook import
// ------------------------------------------------------------------------------------------------

/// A spreadsheet stores 50 % as 0.5 with a percent number format. The importer reads the raw number
/// (formats are not looked at for numbers), so a "% Complete" column of 50 % becomes 0.5 → 0 % or 1 %.
#[test]
fn xlsx_import_reads_percent_cells() {
    let f = file("pct.xlsx");
    let mut wb = rust_xlsxwriter::Workbook::new();
    let ws = wb.add_worksheet();
    let pct = rust_xlsxwriter::Format::new().set_num_format("0%");
    ws.write_string(0, 0, "Task Name").unwrap();
    ws.write_string(0, 1, "Duration").unwrap();
    ws.write_string(0, 2, "% Complete").unwrap();
    ws.write_string(1, 0, "Dig").unwrap();
    ws.write_number(1, 1, 4.0).unwrap();
    ws.write_number_with_format(1, 2, 0.5, &pct).unwrap();
    wb.save(&f).unwrap();
    let Imported::Table(t) = import(&f).unwrap() else { panic!() };
    let (q, _) = io::table::to_project(&t, &t.guess, "X", d(2026, 11, 2), FMT).unwrap();
    assert_eq!(q.tasks[0].pct, 50);
}

/// The task sheet's "WBS" column (key wbs) shows the outline number ("1.2"), and its "Activity ID"
/// column (key code) shows "A1020". In the sheets, the Tasks table's column called "WBS" holds the
/// Activity ID (PLAN_COLS maps key "code" to "WBS"), and since a column of that name exists,
/// Tasks[WBS] is never mapped to the task sheet's WBS field: a formula copying the grid's heading reads
/// a different value than the grid shows. (There is no Tasks column with the outline number at all.)
#[test]
fn sheet_tasks_wbs_is_the_task_sheet_wbs() {
    let p = rich();
    let c = fields::Calc::new(&p, FMT, d(2026, 11, 1));
    let got = sheet::eval(&c, &[("=INDEX(Tasks[WBS],3)".into(), String::new())]);
    assert_eq!(c.name("wbs"), "WBS");
    assert_eq!(got[0][0][0].0, c.text(2, "wbs"), "Tasks[WBS] vs the task sheet's WBS column (Activity ID is {})", c.text(2, "code"));
}

/// (Progress, outside this area but visible in every sheet date.) Updating progress to a status date
/// stores an integer % complete; for a 3-day task 2 days done that is 66 %, and the remaining
/// duration becomes 34 % of 1440 = 489.6 min instead of 480, so the task (and all its successors) now
/// finish at 08:10 the next working day: Tasks[Finish] and ProjectFinish move a day although the work
/// is exactly on plan.
#[test]
fn progress_update_on_plan_keeps_the_finish_day() {
    let mut p = Project::new("P", d(2026, 11, 2));
    let t = p.new_task("Three days", 0, 3 * 480);
    p.tasks.push(t);
    let before = sched::schedule(&p).unwrap().ef[0];
    let mut doc = doc::Doc::new(p);
    doc.apply(doc::Op::UpdateProject { through: "2026-11-03".into(), reschedule: false }, FMT).unwrap();
    let after = sched::schedule(&doc.p).unwrap().ef[0];
    assert_eq!((doc.p.tasks[0].pct, datum_core::cal::date_of(after)), (doc.p.tasks[0].pct, datum_core::cal::date_of(before)), "finish {} → {} (pct {})", before % 1440, after % 1440, doc.p.tasks[0].pct);
}

/// In a plan whose dates are day first (%d/%m/%Y), 02/11/2026 typed in a cell is 2 November. The same
/// date with a time of day, "02/11/2026 13:30", is not read by the plan's date reader and falls to the
/// generic parser, which reads it month first: 11 February.
#[test]
fn sheet_typed_date_with_time_follows_the_plan_order() {
    let p = with_sheet(&[("A1", "02/11/2026"), ("B1", "=MONTH(A1)*100+DAY(A1)"), ("A2", "02/11/2026 13:30"), ("B2", "=MONTH(A2)*100+DAY(A2)"), ("C2", "=ROUND(MOD(A2,1)*24,2)")]);
    assert_eq!(shown(&p, "B1").1, Some(1102.0));
    assert_eq!((shown(&p, "B2").1, shown(&p, "C2").1), (Some(1102.0), Some(13.5)), "A2 shows {:?}", shown(&p, "A2"));
}

/// Same decoding for CSV: a CSV saved by a spreadsheet on Windows ("CSV (Comma delimited)") is in the
/// ANSI code page, Windows-1252. "Béton" arrives as "B\u{FFFD}ton".
#[test]
fn csv_in_windows_1252_keeps_accents() {
    let f = file("ansi.csv");
    std::fs::write(&f, b"Task Name,Duration\r\nB\xE9ton coul\xE9,3\r\n").unwrap();
    let Imported::Table(t) = import(&f).unwrap() else { panic!() };
    assert_eq!(t.rows[0][0], "Béton coulé");
}

/// P6 has nine constraint types; CS_MANDSTART and CS_MANDFIN (mandatory start / finish) are not in the
/// reader's table, so such an activity silently becomes As Soon As Possible (no note in the report),
/// and the plan's dates move. They are the hard "must start / finish on" constraints (MSO / MFO).
#[test]
fn xer_mandatory_constraints_are_kept() {
    let mut p = rich();
    let i = p.tasks.iter().position(|t| t.name == "Design FMEA").unwrap();
    p.tasks[i].constraint = Constraint::MSO;
    p.tasks[i].cdate = Some(d(2027, 1, 4));
    let (_, text) = out(&p, "xer");
    let f = file("mand.xer");
    std::fs::write(&f, text.replace("\tCS_MSO\t", "\tCS_MANDSTART\t")).unwrap();
    let Imported::Plan(q, notes) = import(&f).unwrap() else { panic!() };
    let t = q.tasks.iter().find(|t| t.name == "Design FMEA").unwrap();
    assert_eq!((t.constraint, t.cdate), (Constraint::MSO, Some(d(2027, 1, 4))), "notes: {notes:?}");
}
