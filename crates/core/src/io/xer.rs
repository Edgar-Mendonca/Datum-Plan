//! XER: tab-separated tables (%T table, %F fields, %R rows). Summary tasks become WBS
//! nodes, other tasks become activities.
use crate::cal::{at, date_of, DAY};
use crate::model::*;
use crate::sched::Sched;
use chrono::NaiveDate;
use std::collections::{HashMap, HashSet};
use std::fmt::Write;

fn dt(t: i64) -> String {
    format!("{} {:02}:{:02}", date_of(t), t.rem_euclid(DAY) / 60, t.rem_euclid(60))
}

fn clean(s: &str) -> String {
    s.replace(['\t', '\n', '\r'], " ")
}

/// Elapsed durations run on this round-the-clock calendar in the file (XER has no elapsed durations).
const ELAPSED: &str = "Elapsed (24 hours)";

const CS: [(Constraint, &str); 7] = [
    (Constraint::MSO, "CS_MSO"), (Constraint::MFO, "CS_MEO"), (Constraint::SNET, "CS_MSOA"), (Constraint::SNLT, "CS_MSOB"),
    (Constraint::FNET, "CS_MEOA"), (Constraint::FNLT, "CS_MEOB"), (Constraint::ALAP, "CS_ALAP"),
];

fn excel_day(d: NaiveDate) -> i64 {
    (d - NaiveDate::from_ymd_opt(1899, 12, 30).unwrap()).num_days()
}

/// XER calendar data: nested "(0||name(args)(children))" nodes.
fn clndr_data(c: &Calendar) -> String {
    let hours = |h: &Hours| h.iter().enumerate().map(|(k, (s, e))| format!("(0||{k}(s|{:02}:{:02}|f|{:02}:{:02})())", s / 60, s % 60, (e / 60) % 24, e % 60)).collect::<String>();
    let days: String = (0..7).map(|k| format!("(0||{}()({}))", (k + 1) % 7 + 1, hours(&c.week[k]))).collect();
    let mut exc = String::new();
    let mut n = 0;
    for e in &c.exceptions {
        let mut d = e.from;
        while d <= e.to && n < 5000 {
            let _ = write!(exc, "(0||{n}(d|{})({}))", excel_day(d), hours(&e.hours));
            d = d.succ_opt().unwrap();
            n += 1;
        }
    }
    format!("(0||CalendarData()((0||DaysOfWeek()({days}))(0||VIEW(ShowTotal|Y)())(0||Exceptions()({exc}))))")
}

type Row = HashMap<String, String>;

struct PNode { name: String, args: String, kids: Vec<PNode> }

/// One node of the XER calendar text; `depth` stops damaged text from nesting without end.
fn pnode(s: &[u8], i: &mut usize, depth: u8) -> Option<PNode> {
    let ws = |i: &mut usize| while *i < s.len() && (s[*i] as char).is_whitespace() || (*i < s.len() && s[*i] == 0x7f) { *i += 1 };
    ws(i);
    if s.get(*i) != Some(&b'(') || depth > 32 {
        return None;
    }
    *i += 1;
    while *i < s.len() && s[*i] != b'|' { *i += 1 }
    *i = (*i + 2).min(s.len()); // "||"
    let st = *i;
    while *i < s.len() && s[*i] != b'(' { *i += 1 }
    let name = String::from_utf8_lossy(&s[st..*i]).trim().to_string();
    *i = (*i + 1).min(s.len());
    let st = *i;
    while *i < s.len() && s[*i] != b')' { *i += 1 }
    let args = String::from_utf8_lossy(&s[st..*i]).to_string();
    *i += 1;
    ws(i);
    let mut kids = vec![];
    if s.get(*i) == Some(&b'(') {
        *i += 1;
        while let Some(k) = pnode(s, i, depth + 1) {
            kids.push(k);
        }
        ws(i);
        *i += 1; // ")" closing children
    }
    ws(i);
    *i += 1; // ")" closing node
    Some(PNode { name, args, kids })
}

fn read_cal(name: &str, data: &str, day_hours: f64) -> Calendar {
    let mut c = Calendar::standard(name);
    let s = data.as_bytes();
    let Some(root) = pnode(s, &mut 0, 0) else { return c };
    let hours = |n: &PNode| -> Hours {
        n.kids.iter().filter_map(|h| {
            let p: Vec<&str> = h.args.split('|').collect();
            let m = |x: &str| -> Option<u16> { Some(x.get(..2)?.parse::<u16>().ok()? * 60 + x.get(3..5)?.parse::<u16>().ok()?) };
            let v = |k: &str| p.iter().position(|x| *x == k).and_then(|i| p.get(i + 1)).and_then(|x| m(x));
            let (a, b) = (v("s")?, v("f")?);
            Some((a, if b == 0 { 1440 } else { b }))
        }).collect()
    };
    for sec in &root.kids {
        if sec.name == "DaysOfWeek" {
            for d in &sec.kids {
                if let Ok(k) = d.name.parse::<usize>() {
                    if (1..=7).contains(&k) {
                        c.week[(k + 5) % 7] = hours(d);
                    }
                }
            }
        } else if sec.name == "Exceptions" {
            for e in &sec.kids {
                if let Some(d) = e.args.split('|').nth(1).and_then(|x| x.parse::<u64>().ok()).and_then(|n| NaiveDate::from_ymd_opt(1899, 12, 30)?.checked_add_days(chrono::Days::new(n))) {
                    c.exceptions.push(Exception { name: "Exception".into(), from: d, to: d, hours: hours(e) });
                }
            }
        }
    }
    if c.week.iter().all(Vec::is_empty) {
        let h = vec![(480u16, 480 + (day_hours * 60.0) as u16)];
        c.week = std::array::from_fn(|k| if k < 5 { h.clone() } else { vec![] });
    }
    c
}

/// P6 versions an XER can be written for (the header line); older tools refuse a newer file.
pub const VERSIONS: [&str; 8] = ["8.4", "15.2", "16.2", "18.8", "19.12", "20.12", "21.12", "22.12"];
/// The version written when none is chosen: read by every version since 19.12.
pub const DEFAULT_VERSION: &str = "19.12";

/// Longest text the planning tool keeps in each field (longer text is cut, and the report says so).
const MAX_ID: usize = 40;
const MAX_NAME: usize = 120;
const MAX_WBS_NAME: usize = 100;
const MAX_CODE: usize = 60;

pub fn write(p: &Project, s: &Sched, today: NaiveDate) -> String { write_report(p, s, today, DEFAULT_VERSION).0 }

/// The file and the report: what was written, and anything the format could not hold.
pub fn write_report(p: &Project, s: &Sched, today: NaiveDate, version: &str) -> (String, Vec<String>) {
    let version = if VERSIONS.contains(&version) { version } else { DEFAULT_VERSION };
    let mut notes: Vec<String> = vec![];
    let mut cut = 0usize;
    let mut fit = |x: &str, n: usize| -> String { let x = clean(x); if x.chars().count() > n { cut += 1; x.chars().take(n).collect() } else { x } };
    let cur = &p.currency;
    let mut o = format!("ERMHDR\t{version}\t{today}\tProject\tadmin\tDatum Plan\tdbxDatabaseNoName\tProject Management\t{}\n", clean(&cur.code));
    let mut table = |name: &str, fields: &str, rows: Vec<String>| {
        if rows.is_empty() { return }
        let _ = writeln!(o, "%T\t{name}\n%F\t{}", fields.replace(' ', "\t"));
        rows.iter().for_each(|r| { let _ = writeln!(o, "%R\t{r}"); });
    };
    // The project ID: the plan's code when it has one, else letters and digits of its name.
    let from: &str = if p.code.trim().is_empty() { &p.name } else { &p.code };
    let short: String = from.chars().filter(|c| c.is_alphanumeric() || "-_.".contains(*c)).take(20).collect::<String>().to_uppercase();
    let short = if short.is_empty() { "DATUM".into() } else { short };
    let cal_id = |n: Option<&str>| p.calendars.iter().position(|c| c.name == n.unwrap_or(&p.calendar)).unwrap_or(0) + 1;
    let hd = p.day_min as f64 / 60.0;
    let hr = |m: i64| m as f64 / 60.0;
    let acts: Vec<usize> = (0..p.tasks.len()).filter(|&i| !s.summary[i]).collect();

    // ---- global tables: currency, notebook topics, custom fields ----
    table("CURRTYPE", "curr_id decimal_digit_cnt curr_symbol decimal_symbol digit_group_symbol pos_curr_fmt_type neg_curr_fmt_type curr_type curr_short_name group_digit_cnt base_exch_rate",
        vec![format!("1\t{}\t{}\t.\t,\t{}\t{}\t{}\t{}\t3\t1", cur.decimals, clean(&cur.symbol), if cur.after { "1.1#" } else { "#1.1" }, if cur.after { "(1.1#)" } else { "(#1.1)" }, clean(&cur.code), clean(&cur.code))]);
    // Notebook topics: "Notes" for the task's notes, then each topic the tasks use.
    let mut topics: Vec<String> = vec![];
    if p.tasks.iter().any(|t| !t.notes.trim().is_empty()) { topics.push("Notes".into()) }
    for t in &p.tasks { for n in &t.notebook { if !n.text.trim().is_empty() && !topics.iter().any(|x| x.eq_ignore_ascii_case(n.topic.trim())) { topics.push(clean(n.topic.trim())) } } }
    table("MEMOTYPE", "memo_type_id seq_num eps_flag proj_flag wbs_flag task_flag memo_type",
        topics.iter().enumerate().map(|(k, n)| format!("{}\t{}\tN\tN\tY\tY\t{n}", k + 1, k + 1)).collect());
    // Custom fields used by the activities, by the name the plan gives them.
    let calc = crate::fields::Calc::new(p, "%Y-%m-%d", today);
    let mut used: Vec<String> = p.fields.iter().filter(|d| !d.key.starts_with('r')).map(|d| d.key.clone()).collect();
    for &i in &acts { for k in p.tasks[i].custom.keys() { if !k.starts_with('r') && !used.contains(k) { used.push(k.clone()) } } }
    use crate::fields::Kind;
    let udf: Vec<(String, String, Kind)> = used.into_iter().filter_map(|k| {
        let key = format!("c:{k}");
        let kind = calc.kind(&key);
        acts.iter().any(|&i| !matches!(calc.val(i, &key), crate::formula::F::Null)).then(|| (key.clone(), calc.name(&key), kind))
    }).collect();
    let udf_type = |k: Kind| match k { Kind::Date => "FT_START_DATE", Kind::Fin => "FT_END_DATE", Kind::Cost => "FT_MONEY", Kind::Num | Kind::Dur | Kind::Pct | Kind::Work => "FT_FLOAT_2_DECIMALS", Kind::Int => "FT_INT", _ => "FT_TEXT" };
    table("UDFTYPE", "udf_type_id table_name udf_type_name udf_type_label logical_data_type super_flag indicator_expression summary_indicator_expression export_flag",
        udf.iter().enumerate().map(|(k, (_, name, kind))| {
            let label = if matches!(kind, Kind::Dur) { format!("{name} (days)") } else { name.clone() };
            format!("{}\tTASK\tuser_field_{}\t{}\t{}\tN\t\t\tY", 900 + k, 900 + k, fit(&label, 40), udf_type(*kind))
        }).collect());

    // ---- the project and its calendars ----
    let data = p.status.map(|d| dt(at(d) + DAY)).unwrap_or_else(|| dt(s.start));
    table("PROJECT", "proj_id fy_start_month_num allow_complete_flag project_flag step_complete_flag name_sep_char def_complete_pct_type proj_short_name clndr_id task_code_base task_code_step critical_drtn_hr_cnt last_recalc_date plan_start_date plan_end_date scd_end_date def_duration_type def_qty_type def_rate_type def_task_type critical_path_type task_code_prefix",
        vec![format!("1\t1\tY\tY\tY\t.\tCP_Drtn\t{short}\t{}\t{}\t{}\t{}\t{data}\t{}\t{}\t{}\tDT_FixedDUR2\tQT_Hour\tCOST_PER_QTY\tTT_Task\t{}\t{}",
            cal_id(None), p.id_start, p.id_step.max(1), hr(p.opts.critical_float), dt(s.start), dt(s.finish), dt(s.finish),
            if p.opts.longest_path { "CT_DrivPath" } else { "CT_TotFloat" }, fit(&p.id_prefix, 20))]);
    // A person with leave gets a resource calendar: their base calendar plus the leave.
    let own = |r: &crate::model::Resource| (!r.exceptions.is_empty()).then(|| {
        let mut c = p.calendars.iter().find(|c| Some(&c.name) == r.cal.as_ref()).unwrap_or(&p.calendars[cal_id(None) - 1]).clone();
        c.exceptions.extend(r.exceptions.iter().cloned());
        c.name = r.name.clone();
        c
    });
    let rcal: Vec<usize> = p.resources.iter().enumerate().map(|(k, r)| if own(r).is_some() { 1000 + k } else { cal_id(r.cal.as_deref()) }).collect();
    let mut cal_rows: Vec<String> = p.calendars.iter().enumerate().map(|(k, c)| format!("{}\t{}\t{}\t1\t\t{today} 00:00\tCA_Project\t{hd}\t{}\t{}\t{}\tN\t{}",
        k + 1, if c.name == p.calendar { "Y" } else { "N" }, fit(&c.name, 60), hd * 5.0, hd * 20.0, hd * 250.0, clndr_data(c))).collect();
    for (k, r) in p.resources.iter().enumerate() {
        if let Some(c) = own(r) {
            cal_rows.push(format!("{}\tN\t{}\t\t{}\t{today} 00:00\tCA_Rsrc\t{hd}\t{}\t{}\t{}\tY\t{}", 1000 + k, fit(&c.name, 60), cal_id(r.cal.as_deref()), hd * 5.0, hd * 20.0, hd * 250.0, clndr_data(&c)));
        }
    }
    let el = p.calendars.len() + 1;
    if p.tasks.iter().any(|t| t.elapsed) {
        cal_rows.push(format!("{el}\tN\t{ELAPSED}\t1\t\t{today} 00:00\tCA_Project\t24\t168\t720\t8760\tN\t{}", clndr_data(&Calendar::full(ELAPSED))));
    }
    table("CALENDAR", "clndr_id default_flag clndr_name proj_id base_clndr_id last_chng_date clndr_type day_hr_cnt week_hr_cnt month_hr_cnt year_hr_cnt rsrc_private clndr_data", cal_rows);

    // ---- WBS: the project node, then one node per summary task; its code relative to its parent ----
    let mut wbs_rows = vec![format!("1\t1\t0\tY\tWS_Open\t{short}\t{}\t", fit(&p.name, MAX_WBS_NAME))];
    let mut wbs_of = vec![1usize; p.tasks.len()];
    let mut stack: Vec<(u8, usize, String)> = vec![];
    let mut siblings: HashMap<(usize, String), u32> = HashMap::new();
    let mut wbs_ids = 0;
    for (i, t) in p.tasks.iter().enumerate() {
        stack.retain(|(l, _, _)| *l < t.level);
        let (parent, pcode) = stack.last().map_or((1, String::new()), |(_, w, c)| (*w, c.clone()));
        wbs_of[i] = parent;
        if s.summary[i] {
            let id = 100 + i;
            let full = if t.code.trim().is_empty() { format!("W{}", i + 1) } else { clean(t.code.trim()) };
            let own = full.strip_prefix(&format!("{pcode}.")).filter(|x| !pcode.is_empty() && !x.is_empty()).unwrap_or(&full).to_string();
            let mut code = fit(&own, MAX_ID);
            let n = siblings.entry((parent, code.to_lowercase())).or_insert(0);
            *n += 1;
            if *n > 1 { code = format!("{code}-{n}"); wbs_ids += 1 }
            wbs_rows.push(format!("{id}\t1\t{}\tN\tWS_Open\t{code}\t{}\t{parent}", i + 1, fit(&t.name, MAX_WBS_NAME)));
            stack.push((t.level, id, full));
        }
    }
    table("PROJWBS", "wbs_id proj_id seq_num proj_node_flag status_code wbs_short_name wbs_name parent_wbs_id", wbs_rows);
    if wbs_ids > 0 { notes.push(format!("{wbs_ids} WBS codes appear twice under the same parent; the second gets \"-2\" (and so on), since each must be unique there.")) }

    // ---- activity codes (project codes, so the client's own dictionary is not touched) ----
    let mut code_rows = vec![];
    let mut type_rows = vec![];
    let mut code_id: HashMap<(usize, String), usize> = HashMap::new();
    let mut next = 1;
    for (k, d) in p.codes.iter().enumerate() {
        let mut vals: Vec<(String, String, String)> = d.values.iter().map(|v| (v.value.clone(), v.desc.clone(), v.color.clone())).collect();
        for &i in &acts { if let Some(v) = p.tasks[i].codes.get(&d.name).filter(|v| !v.is_empty()) { if !vals.iter().any(|x| x.0 == *v) { vals.push((v.clone(), String::new(), String::new())) } } }
        if vals.is_empty() { continue }
        let len = vals.iter().map(|v| v.0.chars().count()).max().unwrap_or(1).clamp(1, MAX_CODE);
        type_rows.push(format!("{}\t{len}\t{}\t{}\t1\t\tAS_Project\tN", 800 + k, k + 1, fit(&d.name, 40)));
        for (j, (v, desc, color)) in vals.iter().enumerate() {
            code_id.insert((k, v.clone()), next);
            let c = color.trim_start_matches('#');
            code_rows.push(format!("{next}\t\t{}\t{}\t{}\t{}\t{}", 800 + k, fit(if desc.is_empty() { v } else { desc }, 100), fit(v, MAX_CODE), j + 1, if c.len() == 6 { c.to_uppercase() } else { String::new() }));
            next += 1;
        }
    }
    let ncodes = type_rows.len();
    table("ACTVTYPE", "actv_code_type_id actv_short_len seq_num actv_code_type proj_id wbs_id actv_code_type_scope super_flag", type_rows);
    table("ACTVCODE", "actv_code_id parent_actv_code_id actv_code_type_id actv_code_name short_name seq_num color", code_rows);

    // ---- resources ----
    let mut rshort: HashSet<String> = HashSet::new();
    table("RSRC", "rsrc_id parent_rsrc_id clndr_id rsrc_seq_num email_addr employee_code rsrc_name rsrc_short_name rsrc_title_name def_qty_per_hr cost_qty_type active_flag auto_compute_act_flag def_cost_qty_link_flag ot_flag curr_id rsrc_type",
        p.resources.iter().enumerate().map(|(k, r)| {
            let ty = match r.kind { ResKind::Work => "RT_Labor", ResKind::Material => "RT_Mat", ResKind::Cost => "RT_Equip" };
            let base: String = if r.initials.trim().is_empty() { r.name.chars().filter(|c| c.is_alphanumeric()).take(20).collect::<String>().to_uppercase() } else { clean(r.initials.trim()).to_uppercase() };
            let base = if base.is_empty() { format!("R{}", r.uid) } else { base };
            let mut sh = base.clone();
            let mut n = 1;
            while !rshort.insert(sh.to_lowercase()) { n += 1; sh = format!("{base}{n}") }
            format!("{}\t\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\tQT_Hour\tY\tY\tY\tN\t1\t{ty}", r.uid, rcal[k], k + 1, clean(&r.email), clean(&r.code), fit(&r.name, 100), fit(&sh, 20), clean(&r.group), if r.max > 0.0 { r.max } else { 1.0 })
        }).collect());
    table("RSRCRATE", "rsrc_rate_id rsrc_id max_qty_per_hr cost_per_qty start_date", p.resources.iter().enumerate().map(|(k, r)| format!("{}\t{}\t{}\t{}\t{} 00:00", k + 1, r.uid, if r.max > 0.0 { r.max } else { 1.0 }, r.rate, p.start)).collect());

    // ---- activities: unique activity IDs, cut to the format's length ----
    let held = super::ext_held(p);
    let mut ids: HashSet<String> = HashSet::new();
    let mut twice = 0;
    let act_rows: Vec<String> = acts.iter().map(|&i| {
        let t = &p.tasks[i];
        let rsrc = t.act_type == ActType::ResourceDep || (p.opts.res_cals && !t.ignore_rc && t.res.iter().any(|a| p.res_idx(a.res).is_some_and(|k| p.resources[k].kind == ResKind::Work)));
        let (dur, cons, cdate) = super::held(p, s, i, held[i]);
        // A milestone at the end of a working period is a finish milestone (a start milestone would move to the next morning).
        let kind = match t.act_type { ActType::Loe => "TT_LOE", ActType::WbsSummary => "TT_WBS", ActType::StartMs => "TT_Mile", ActType::FinishMs => "TT_FinMile",
            _ if dur == 0 => if s.cals[s.ci[i]].next_start(s.es[i]) == s.es[i] { "TT_Mile" } else { "TT_FinMile" }, _ if rsrc => "TT_Rsrc", _ => "TT_Task" };
        let status = if t.act_finish.is_some() { "TK_Complete" } else if t.act_start.is_some() { "TK_Active" } else { "TK_NotStart" };
        let (cd, ct) = match (cdate, CS.iter().find(|(c, _)| *c == cons)) {
            (_, Some((Constraint::ALAP, n))) => (String::new(), n.to_string()),
            (Some(d), Some((_, n))) => (format!("{d} 00:00"), n.to_string()),
            _ => (String::new(), String::new()),
        };
        let base = if t.code.trim().is_empty() { format!("A{}", (i + 1) * 10) } else { fit(t.code.trim(), MAX_ID) };
        let mut code = base.clone();
        let mut n = 1;
        while !ids.insert(code.to_lowercase()) { n += 1; code = format!("{}-{n}", base.chars().take(MAX_ID - 3).collect::<String>()); }
        if n > 1 { twice += 1 }
        // A physical-% task carries its physical %; the duration % stays in the remaining duration.
        let (pc, pt) = match t.pct_type { PctType::Physical => (t.phys, "CP_Phys"), PctType::Units => (t.pct, "CP_Units"), PctType::Duration => (t.pct, "CP_Drtn") };
        // A deadline goes as a secondary "finish on or before" constraint.
        let (cd2, ct2) = t.deadline.map_or((String::new(), ""), |d| (format!("{d} 23:59"), "CS_MEOB"));
        let work: f64 = t.res.iter().filter(|a| p.res_idx(a.res).is_some_and(|k| p.resources[k].kind == ResKind::Work)).map(|a| dur as f64 / 60.0 * a.units).sum();
        let left = (100 - t.pct.min(100) as i64) as f64 / 100.0;
        let done = t.act_finish.is_some();
        let (rs, re) = if done { (String::new(), String::new()) } else { (dt(s.rs[i].max(s.es[i])), dt(s.ef[i])) };
        [t.uid.to_string(), "1".into(), wbs_of[i].to_string(), (if t.elapsed { el } else { cal_id(t.cal.as_deref()) }).to_string(), pc.to_string(), pt.into(), kind.into(), "DT_FixedDUR2".into(), status.into(), code,
            fit(&t.name, MAX_NAME), hr(dur).to_string(), (if done { 0.0 } else { hr(dur) * left }).to_string(), hr(s.tf[i]).to_string(), hr(s.ff[i]).to_string(),
            t.act_start.map(|d| format!("{d} 08:00")).unwrap_or_default(), t.act_finish.map(|d| format!("{d} 17:00")).unwrap_or_default(),
            dt(s.es[i]), dt(s.ef[i]), dt(s.ls[i]), dt(s.lf[i]), dt(s.es[i]), dt(s.ef[i]), rs, re,
            work.to_string(), (work * left).to_string(), (work * (1.0 - left)).to_string(), cd, ct, cd2, ct2.into(), "PT_Normal".into(),
            (if s.lp[i] { "Y" } else { "N" }).into(), if s.fpath[i] > 0 { s.fpath[i].to_string() } else { String::new() }, "N".into(), "N".into(), "N".into()].join("\t")
    }).collect();
    if twice > 0 { notes.push(format!("{twice} activity IDs appear more than once; the repeats get \"-2\" (and so on), since each must be unique in the project.")) }
    table("TASK", "task_id proj_id wbs_id clndr_id phys_complete_pct complete_pct_type task_type duration_type status_code task_code task_name target_drtn_hr_cnt remain_drtn_hr_cnt total_float_hr_cnt free_float_hr_cnt act_start_date act_end_date early_start_date early_end_date late_start_date late_end_date target_start_date target_end_date restart_date reend_date target_work_qty remain_work_qty act_work_qty cstr_date cstr_type cstr_date2 cstr_type2 priority_type driving_path_flag float_path rev_fdbk_flag lock_plan_flag auto_compute_act_flag",
        act_rows);
    // A link to or from a summary applies to every activity under it; a percent lag becomes hours.
    let under = |i: usize| -> Vec<usize> { if s.summary[i] { p.subtree(i).filter(|&j| !s.summary[j]).collect() } else { vec![i] } };
    let mut links = vec![];
    let mut spread = 0;
    let idx: HashMap<u32, usize> = p.tasks.iter().enumerate().rev().map(|(i, t)| (t.uid, i)).collect();
    for (i, t) in p.tasks.iter().enumerate() {
        for l in &t.preds {
            let Some(&k) = idx.get(&l.pred) else { continue };
            if s.summary[i] || s.summary[k] { spread += 1 }
            let lag = super::work_lag(p, l, p.tasks[k].dur);
            for b in under(i) { for a in under(k) { links.push(format!("{}\t{}\t{}\t1\t1\tPR_{:?}\t{}", links.len() + 1, p.tasks[b].uid, p.tasks[a].uid, l.kind, lag as f64 / 60.0)) } }
        }
    }
    let nlinks = links.len();
    table("TASKPRED", "task_pred_id task_id pred_task_id proj_id pred_proj_id pred_type lag_hr_cnt", links);
    if spread > 0 { notes.push(format!("{spread} links to or from summary tasks were written to each activity under the summary (the format links activities only).")) }
    let pct_lags = p.tasks.iter().flat_map(|t| &t.preds).filter(|l| l.pct && l.lag != 0).count();
    if pct_lags > 0 { notes.push(format!("{pct_lags} percent lags were written in hours (that share of the predecessor's duration).")) }
    let mut n = 0;
    table("TASKRSRC", "taskrsrc_id task_id proj_id rsrc_id rsrc_type target_qty remain_qty act_reg_qty target_qty_per_hr remain_qty_per_hr target_cost target_start_date target_end_date", acts.iter().flat_map(|&i| p.tasks[i].res.iter().map(move |a| (i, a))).map(|(i, a)| {
        n += 1;
        let t = &p.tasks[i];
        let dates = format!("{}\t{}", dt(s.es[i]), dt(s.ef[i]));
        // A cost resource has no quantity, only an amount.
        let kind = p.res_idx(a.res).map(|k| p.resources[k].kind);
        let ty = match kind { Some(ResKind::Material) => "RT_Mat", Some(ResKind::Cost) => "RT_Equip", _ => "RT_Labor" };
        if kind == Some(ResKind::Cost) { return format!("{n}\t{}\t1\t{}\t{ty}\t0\t0\t0\t0\t0\t{}\t{dates}", t.uid, a.res, a.units) }
        let q = if kind == Some(ResKind::Work) { t.dur as f64 / 60.0 * a.units } else { a.units };
        let left = (100 - t.pct.min(100) as i64) as f64 / 100.0;
        format!("{n}\t{}\t1\t{}\t{ty}\t{q}\t{}\t{}\t{}\t{}\t\t{dates}", t.uid, a.res, q * left, q * (1.0 - left), a.units, a.units)
    }).collect());
    let mut tact = vec![];
    for (k, d) in p.codes.iter().enumerate() {
        for &i in &acts {
            if let Some(&cid) = p.tasks[i].codes.get(&d.name).and_then(|v| code_id.get(&(k, v.clone()))) { tact.push(format!("{}\t{}\t{cid}\t1", p.tasks[i].uid, 800 + k)) }
        }
    }
    let ntact = tact.len();
    table("TASKACTV", "task_id actv_code_type_id actv_code_id proj_id", tact);
    let on_sum = (0..p.tasks.len()).filter(|&i| s.summary[i] && !p.tasks[i].codes.is_empty()).count();
    if on_sum > 0 { notes.push(format!("{on_sum} summary tasks have activity codes; WBS nodes cannot hold them, so they were left out.")) }
    // Custom field values: dates as dates, numbers as numbers, the rest as text (Yes/No for flags).
    let mut vals = vec![];
    for (k, (key, _, kind)) in udf.iter().enumerate() {
        for &i in &acts {
            use crate::formula::F;
            let v = calc.val(i, key);
            if matches!(v, F::Null) { continue }
            let (d, num, txt) = match (kind, &v) {
                (Kind::Date | Kind::Fin, F::Date(m)) => (dt(*m), String::new(), String::new()),
                (Kind::Dur, F::Num(m)) => (String::new(), format!("{}", (m / p.day_min.max(1) as f64 * 100.0).round() / 100.0), String::new()),
                (Kind::Num | Kind::Cost | Kind::Pct | Kind::Work | Kind::Int, F::Num(x)) => (String::new(), x.to_string(), String::new()),
                _ => (String::new(), String::new(), fit(&v.to_string(), 255)),
            };
            vals.push(format!("{}\t{}\t1\t{d}\t{num}\t{txt}\t", 900 + k, p.tasks[i].uid));
        }
    }
    let nvals = vals.len();
    table("UDFVALUE", "udf_type_id fk_id proj_id udf_date udf_number udf_text udf_code_id", vals);
    // Notes: the task's notes and each notebook topic, as the simple web text the format holds.
    let memo = |x: &str| format!("<HTML><BODY>{}</BODY></HTML>", clean(&x.trim().replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace("\r\n", "\n").replace('\n', "<BR>")));
    let topic = |n: &str| topics.iter().position(|x| x.eq_ignore_ascii_case(n.trim())).map_or(1, |k| k + 1);
    let (mut tm, mut wm) = (vec![], vec![]);
    for (i, t) in p.tasks.iter().enumerate() {
        let mut items: Vec<(usize, String)> = vec![];
        if !t.notes.trim().is_empty() { items.push((topic("Notes"), memo(&t.notes))) }
        for nb in t.notebook.iter().filter(|x| !x.text.trim().is_empty()) { items.push((topic(&nb.topic), memo(&nb.text))) }
        for (ty, text) in items {
            if s.summary[i] { wm.push(format!("{}\t1\t{}\t{ty}\t{text}", wm.len() + 1, 100 + i)) } else { tm.push(format!("{}\t{}\t{ty}\t1\t{text}", tm.len() + 1, t.uid)) }
        }
    }
    let nmemo = tm.len() + wm.len();
    table("WBSMEMO", "wbs_memo_id proj_id wbs_id memo_type_id wbs_memo", wm);
    table("TASKMEMO", "memo_id task_id memo_type_id proj_id task_memo", tm);
    // Steps, with their weights and whether each is done.
    let mut steps = vec![];
    for &i in &acts {
        for (k, st) in p.tasks[i].steps.iter().enumerate() {
            steps.push(format!("{}\t{}\t{}\t1\t{}\t{}\t{}\t{}\t", steps.len() + 1, p.tasks[i].uid, k + 1, if st.done { "Y" } else { "N" }, fit(&st.name, 120), st.weight, if st.done { 100 } else { 0 }));
        }
    }
    let nsteps = steps.len();
    table("TASKPROC", "proc_id task_id seq_num proj_id complete_flag proc_name proc_wt complete_pct proc_descr", steps);
    o.push_str("%E\n");

    // ---- the report ----
    let nwbs = (0..p.tasks.len()).filter(|&i| s.summary[i]).count();
    let mut head = vec![format!("Written for version {version}: {} activities, {nwbs} WBS nodes, {nlinks} relationships, {} calendars, {} resources and {n} assignments.",
        acts.len(), p.calendars.len() + usize::from(p.tasks.iter().any(|t| t.elapsed)), p.resources.len())];
    let mut more = vec![];
    if ncodes > 0 { more.push(format!("{ncodes} activity codes ({ntact} assignments)")) }
    if !udf.is_empty() { more.push(format!("{} custom fields ({nvals} values)", udf.len())) }
    if nsteps > 0 { more.push(format!("{nsteps} steps")) }
    if nmemo > 0 { more.push(format!("{nmemo} notes")) }
    if !more.is_empty() { head.push(format!("Also: {}.", more.join(", "))) }
    if cut > 0 { notes.push(format!("{cut} texts were longer than the format allows (activity IDs {MAX_ID}, names {MAX_NAME} characters) and were cut.")) }
    let base = p.tasks.iter().any(|t| !t.bls.is_empty());
    if base { notes.push("Baselines are not in the file: the receiving tool keeps baselines as separate projects. Export the plan with its baseline as the current dates to send one.".into()) }
    let el = p.tasks.iter().flat_map(|t| &t.preds).filter(|l| l.elapsed && l.lag != 0).count();
    if el > 0 { notes.push(format!("{el} elapsed lags were written as working time of the same share of a week (7 elapsed days are 5 working days on a five-day week): the format has no elapsed lags.")) }
    let ext = p.tasks.iter().filter(|t| !t.ext.is_empty()).count();
    if ext > 0 { notes.push(format!("{ext} activities wait for tasks in other plans; those links became constraints at their current dates.")) }
    head.extend(notes);
    (o, head)
}

pub fn read(text: &str) -> Result<Project, String> { read_notes(text).map(|x| x.0) }

/// Reads an XER file and notes what was changed or left out on the way in.
pub fn read_notes(text: &str) -> Result<(Project, Vec<String>), String> {
    let mut notes = vec![];
    let mut tables: HashMap<String, Vec<HashMap<String, String>>> = HashMap::new();
    let (mut cur, mut fields): (String, Vec<String>) = (String::new(), vec![]);
    for line in text.lines() {
        let mut parts = line.split('\t');
        match parts.next() {
            Some("%T") => cur = parts.next().unwrap_or("").trim().to_string(),
            Some("%F") => fields = parts.map(|f| f.trim().to_string()).collect(),
            Some("%R") => {
                let row = fields.iter().cloned().zip(parts.map(String::from)).collect();
                tables.entry(cur.clone()).or_default().push(row);
            }
            _ => {}
        }
    }
    let rows = |t: &str| tables.get(t).map(Vec::as_slice).unwrap_or(&[]);
    let proj = rows("PROJECT").first().ok_or("This XER file has no project.")?;
    let pid = proj.get("proj_id").cloned().unwrap_or_default();
    let mine = |r: &&HashMap<String, String>| r.get("proj_id").is_none_or(|x| *x == pid);
    let g = |r: &HashMap<String, String>, k: &str| r.get(k).map(|s| s.trim().to_string()).unwrap_or_default();
    let date = |s: String| NaiveDate::parse_from_str(s.get(..10).unwrap_or(""), "%Y-%m-%d").ok();
    let start = date(g(proj, "plan_start_date")).unwrap_or_else(|| chrono::Local::now().date_naive());
    // The project's name is its top WBS node's; its ID (short name) is the fallback.
    let top = rows("PROJWBS").iter().find(|w| g(w, "proj_node_flag") == "Y" && g(w, "proj_id") == pid).map(|w| g(w, "wbs_name"));
    let name = [top.clone().unwrap_or_default(), g(proj, "proj_short_name")].into_iter().find(|s| !s.is_empty()).unwrap_or_else(|| "Imported project".into());
    let mut p = Project::new(&name, start);
    if top.is_some_and(|t| !t.is_empty()) { p.code = g(proj, "proj_short_name") }
    p.status = date(g(proj, "last_recalc_date")).and_then(|d| d.pred_opt()).filter(|d| *d >= start);
    let mut cal_names = HashMap::new();
    let cals: Vec<&HashMap<String, String>> = rows("CALENDAR").iter().collect();
    let pcal = g(proj, "clndr_id");
    let hd = cals.iter().find(|c| g(c, "clndr_id") == pcal).and_then(|c| g(c, "day_hr_cnt").parse::<f64>().ok()).unwrap_or(8.0);
    p.day_min = (hd * 60.0).round() as i64;
    if !cals.is_empty() {
        p.calendars = vec![];
        for c in &cals {
            let mut name = g(c, "clndr_name");
            if name.is_empty() || p.calendars.iter().any(|x| x.name == name) {
                name = format!("{name} {}", g(c, "clndr_id")).trim().to_string();
            }
            cal_names.insert(g(c, "clndr_id"), name.clone());
            p.calendars.push(read_cal(&name, &g(c, "clndr_data"), g(c, "day_hr_cnt").parse().unwrap_or(8.0)));
        }
        p.calendar = cal_names.get(&pcal).cloned().or_else(|| cals.iter().find(|c| g(c, "default_flag") == "Y").and_then(|c| cal_names.get(&g(c, "clndr_id")).cloned())).unwrap_or_else(|| p.calendars[0].name.clone());
    }
    // WBS tree, ordered by seq_num.
    let wbs: Vec<&HashMap<String, String>> = rows("PROJWBS").iter().filter(mine).collect();
    let root = wbs.iter().find(|w| g(w, "proj_node_flag") == "Y").map(|w| g(w, "wbs_id"));
    let mut kids: HashMap<String, Vec<&HashMap<String, String>>> = HashMap::new();
    for w in &wbs {
        if Some(g(w, "wbs_id")) != root {
            kids.entry(g(w, "parent_wbs_id")).or_default().push(w);
        }
    }
    kids.values_mut().for_each(|v| v.sort_by_key(|w| g(w, "seq_num").parse::<i64>().unwrap_or(0)));
    let mut acts: HashMap<String, Vec<&HashMap<String, String>>> = HashMap::new();
    for t in rows("TASK").iter().filter(mine) {
        acts.entry(g(t, "wbs_id")).or_default().push(t);
    }
    acts.values_mut().for_each(|v| v.sort_by_key(|a| (g(a, "early_start_date"), g(a, "task_code"))));
    // Under each WBS node: its activities, then its child nodes. A file Datum Plan wrote keeps the
    // plan's rows: nodes sit at their row (seq_num) and activities fill the other rows in file order.
    let mut place: HashMap<*const Row, usize> = HashMap::new();
    if text.lines().next().is_some_and(|l| l.split('\t').nth(5) == Some("Datum Plan")) {
        let taken: HashSet<usize> = wbs.iter().filter_map(|w| g(w, "seq_num").parse::<usize>().ok()).collect();
        let mut free = (1..).filter(|k| !taken.contains(k));
        for t in rows("TASK").iter().filter(mine) { place.insert(t as *const _, free.next().unwrap_or(0)); }
        for w in &wbs { place.insert(*w as *const _, g(w, "seq_num").parse().unwrap_or(0)); }
    }
    let cons = |s: &str| CS.iter().find(|(_, n)| *n == s).map(|(c, _)| *c).unwrap_or_default();
    let mut uid = 1_000_000u32;
    // `seen` walks each WBS node once, so a damaged file whose WBS loops back on itself still opens.
    #[allow(clippy::too_many_arguments)]
    fn walk<'a>(id: &'a str, level: u8, seen: &mut HashSet<&'a str>, p: &mut Project, kids: &'a HashMap<String, Vec<&'a Row>>, acts: &'a HashMap<String, Vec<&'a Row>>,
        place: &HashMap<*const Row, usize>, mk: &mut dyn FnMut(&Row, u8) -> Task, sum: &mut dyn FnMut(&Row, u8) -> Task) {
        if !seen.insert(id) {
            return;
        }
        let key = |r: &Row| place.get(&(r as *const _)).copied().unwrap_or(0);
        let mut items: Vec<(usize, bool, &Row)> = acts.get(id).into_iter().flatten().map(|t| (key(t), false, *t)).collect();
        items.extend(kids.get(id).into_iter().flatten().map(|w| (key(w), true, *w)));
        items.sort_by_key(|x| (x.0, x.1));
        for (_, node, r) in items {
            if !node {
                let task = mk(r, level);
                p.tasks.push(task);
                continue;
            }
            let at = p.tasks.len();
            let s = sum(r, level);
            p.tasks.push(s);
            walk(r.get("wbs_id").map_or("", |s| s.trim()), level.saturating_add(1), seen, p, kids, acts, place, mk, sum);
            if p.tasks.len() == at + 1 {
                p.tasks.pop(); // an empty WBS node
            }
        }
    }
    let mut mk = |t: &HashMap<String, String>, level: u8| {
        let ty = g(t, "task_type");
        let dur = if ty.contains("Mile") { 0 } else { (g(t, "target_drtn_hr_cnt").parse::<f64>().unwrap_or(0.0) * 60.0).round() as i64 };
        let ct = cons(&g(t, "cstr_type"));
        let phys = g(t, "complete_pct_type") == "CP_Phys";
        let pc = g(t, "phys_complete_pct").parse::<f64>().unwrap_or(0.0).clamp(0.0, 100.0).round() as u8;
        // The duration % comes from the remaining duration when the file has one (the % column holds
        // the physical %, which an activity measured by duration leaves at 0).
        let (target, remain) = (g(t, "target_drtn_hr_cnt").parse::<f64>().unwrap_or(0.0), g(t, "remain_drtn_hr_cnt").parse::<f64>().ok());
        let pct = match remain { Some(r) if target > 0.0 && r.is_finite() => ((1.0 - r / target) * 100.0).clamp(0.0, 100.0).round() as u8, _ => pc };
        let act_type = match ty.as_str() { "TT_LOE" => ActType::Loe, "TT_WBS" => ActType::WbsSummary, "TT_Rsrc" => ActType::ResourceDep, "TT_Mile" => ActType::StartMs, "TT_FinMile" => ActType::FinishMs, _ => ActType::Task };
        Task {
            uid: g(t, "task_id").parse().unwrap_or(0), name: g(t, "task_name"), level, dur, code: g(t, "task_code"), act_type, ignore_rc: ty == "TT_Task",
            pct, phys: if phys { pc } else { 0 }, pct_type: if phys { PctType::Physical } else { PctType::Duration },
            constraint: ct, cdate: if matches!(ct, Constraint::ASAP | Constraint::ALAP) { None } else { date(g(t, "cstr_date")) },
            cal: cal_names.get(&g(t, "clndr_id")).cloned(),
            act_start: date(g(t, "act_start_date")), act_finish: date(g(t, "act_end_date")),
            deadline: if g(t, "cstr_type2") == "CS_MEOB" { date(g(t, "cstr_date2")) } else { None },
            ..Default::default()
        }
    };
    let mut wbs_uid: HashMap<String, u32> = HashMap::new();
    let mut sum = |w: &HashMap<String, String>, level: u8| {
        uid += 1;
        wbs_uid.insert(g(w, "wbs_id"), uid);
        Task { uid, name: g(w, "wbs_name"), code: g(w, "wbs_short_name"), level, ..Default::default() }
    };
    walk(root.as_deref().unwrap_or(""), 0, &mut HashSet::new(), &mut p, &kids, &acts, &place, &mut mk, &mut sum);
    // Activities on the project calendar follow it (a later change of project calendar moves them too).
    let pc = p.calendar.clone();
    p.tasks.iter_mut().filter(|t| t.cal.as_ref() == Some(&pc)).for_each(|t| t.cal = None);
    // A task on the round-the-clock calendar Datum Plan writes for them is elapsed again.
    if p.calendars.iter().any(|c| c.name == ELAPSED && *c == Calendar::full(ELAPSED)) {
        p.tasks.iter_mut().filter(|t| t.cal.as_deref() == Some(ELAPSED)).for_each(|t| (t.elapsed, t.cal, t.unit) = (true, None, Unit::D));
        p.calendars.retain(|c| c.name != ELAPSED || p.calendar == ELAPSED);
    }
    // Activities outside the WBS tree go at the end.
    let have: std::collections::HashSet<String> = p.tasks.iter().map(|x| x.uid.to_string()).collect();
    let rest: Vec<Task> = rows("TASK").iter().filter(mine).filter(|t| !have.contains(&g(t, "task_id"))).map(|t| mk(t, 0)).collect();
    if !rest.is_empty() { notes.push(format!("{} activities are outside the WBS; they are at the end of the plan.", rest.len())) }
    p.tasks.extend(rest);
    let second = rows("TASK").iter().filter(mine).filter(|t| !matches!(g(t, "cstr_type2").as_str(), "" | "CS_MEOB")).count();
    if second > 0 { notes.push(format!("{second} activities have a secondary constraint other than finish on or before (kept as a deadline), which Datum Plan does not use yet; only the primary constraint was kept.")) }
    let mut dropped = 0;
    let idx: HashMap<u32, usize> = p.tasks.iter().enumerate().rev().map(|(i, t)| (t.uid, i)).collect();
    for l in rows("TASKPRED") {
        let (s, q): (u32, u32) = (g(l, "task_id").parse().unwrap_or(0), g(l, "pred_task_id").parse().unwrap_or(0));
        let kind = match g(l, "pred_type").as_str() { "PR_SS" => LinkType::SS, "PR_FF" => LinkType::FF, "PR_SF" => LinkType::SF, _ => LinkType::FS };
        let lag = (g(l, "lag_hr_cnt").parse::<f64>().unwrap_or(0.0) * 60.0).round() as i64;
        match idx.get(&q).and(idx.get(&s)) {
            Some(&i) => p.tasks[i].preds.push(Link { pred: q, kind, lag, pct: false, elapsed: false }),
            None => dropped += 1,
        }
    }
    if dropped > 0 { notes.push(format!("{dropped} relationships point to activities that are not in this project (another project or a deleted activity); they were left out.")) }
    for r in rows("RSRC") {
        let kind = if g(r, "rsrc_type") == "RT_Mat" { ResKind::Material } else { ResKind::Work };
        let cal = cal_names.get(&g(r, "clndr_id")).cloned().filter(|c| *c != p.calendar);
        let uid = g(r, "rsrc_id");
        let rate = rows("RSRCRATE").iter().find(|x| g(x, "rsrc_id") == uid).and_then(|x| g(x, "cost_per_qty").parse().ok()).unwrap_or(0.0);
        p.resources.push(Resource { uid: uid.parse().unwrap_or(0), name: g(r, "rsrc_name"), kind, cal, rate, ..Default::default() });
    }
    for a in rows("TASKRSRC").iter().filter(mine) {
        let (tu, ru): (u32, u32) = (g(a, "task_id").parse().unwrap_or(0), g(a, "rsrc_id").parse().unwrap_or(0));
        let qty = g(a, "target_qty").parse::<f64>().unwrap_or(0.0);
        let per_hr = g(a, "target_qty_per_hr").parse::<f64>().ok();
        let cost = g(a, "target_cost").parse::<f64>().unwrap_or(0.0);
        // An assignment with an amount and no quantity is a cost resource.
        let money = qty == 0.0 && cost > 0.0;
        if let Some(r) = p.resources.iter_mut().find(|r| r.uid == ru && money) { r.kind = ResKind::Cost }
        let work = p.resources.iter().any(|r| r.uid == ru && r.kind == ResKind::Work);
        if let Some(t) = idx.get(&tu).map(|&i| &mut p.tasks[i]) {
            let units = if money { cost } else if work { per_hr.unwrap_or(if t.dur > 0 { qty * 60.0 / t.dur as f64 } else { 1.0 }) } else { qty };
            t.res.push(Assign { res: ru, units, table: 0 });
        }
    }
    extras(&rows, &g, &date, &wbs_uid, &mut p, &mut notes);
    p.normalize();
    Ok((p, notes))
}

/// Activity codes, custom fields, notes and steps of the activities in the file.
fn extras<'a>(rows: &dyn Fn(&str) -> &'a [Row], g: &dyn Fn(&Row, &str) -> String, date: &dyn Fn(String) -> Option<NaiveDate>, wbs_uid: &HashMap<String, u32>, p: &mut Project, notes: &mut Vec<String>) {
    let idx: HashMap<u32, usize> = p.tasks.iter().enumerate().map(|(i, t)| (t.uid, i)).collect();
    let task = |r: &Row| g(r, if r.contains_key("fk_id") { "fk_id" } else { "task_id" }).parse::<u32>().ok().and_then(|u| idx.get(&u).copied());
    // Activity codes that the activities use, with their values (all of them for a short list).
    let mut by_code: HashMap<String, Vec<&Row>> = HashMap::new();
    for c in rows("ACTVCODE") { by_code.entry(g(c, "actv_code_type_id")).or_default().push(c) }
    let mut assigned: HashMap<String, Vec<(usize, String)>> = HashMap::new();
    for a in rows("TASKACTV") { if let Some(i) = task(a) { assigned.entry(g(a, "actv_code_type_id")).or_default().push((i, g(a, "actv_code_id"))) } }
    let mut types: Vec<&Row> = rows("ACTVTYPE").iter().filter(|t| assigned.contains_key(&g(t, "actv_code_type_id"))).collect();
    types.sort_by_key(|t| g(t, "seq_num").parse::<i64>().unwrap_or(0));
    for t in types {
        let id = g(t, "actv_code_type_id");
        let mut name = g(t, "actv_code_type");
        if name.is_empty() { name = format!("Code {id}") }
        while p.codes.iter().any(|c| c.name.eq_ignore_ascii_case(&name)) { name.push('*') }
        let mut vals: Vec<&Row> = by_code.get(&id).cloned().unwrap_or_default();
        vals.sort_by_key(|v| g(v, "seq_num").parse::<i64>().unwrap_or(0));
        let used: HashSet<String> = assigned[&id].iter().map(|x| x.1.clone()).collect();
        let short = |v: &Row| { let x = g(v, "short_name"); if x.is_empty() { g(v, "actv_code_name") } else { x } };
        let values = vals.iter().filter(|v| vals.len() <= 500 || used.contains(&g(v, "actv_code_id"))).map(|v| {
            let c = g(v, "color");
            CodeValue { value: short(v), desc: g(v, "actv_code_name"), color: if c.len() == 6 { format!("#{c}") } else { String::new() } }
        }).collect();
        let val_of: HashMap<String, String> = vals.iter().map(|v| (g(v, "actv_code_id"), short(v))).collect();
        for (i, cid) in &assigned[&id] { if let Some(v) = val_of.get(cid) { p.tasks[*i].codes.insert(name.clone(), v.clone()); } }
        p.codes.push(CodeDef { name, scope: if g(t, "actv_code_type_scope") == "AS_Global" { "Global".into() } else { "Project".into() }, values });
    }
    // Custom fields of activities: each to a free field of its kind.
    let mut by_type: HashMap<String, Vec<&Row>> = HashMap::new();
    for v in rows("UDFVALUE") { by_type.entry(g(v, "udf_type_id")).or_default().push(v) }
    let mut full = 0;
    for u in rows("UDFTYPE").iter().filter(|u| g(u, "table_name") == "TASK") {
        let Some(vals) = by_type.get(&g(u, "udf_type_id")).filter(|v| v.iter().any(|x| task(x).is_some())) else { continue };
        let mut label = g(u, "udf_type_label");
        let days = label.ends_with(" (days)");
        if days { label.truncate(label.len() - 7) }
        let (pre, max) = match g(u, "logical_data_type").as_str() {
            _ if days => ("duration", 10), "FT_START_DATE" => ("start", 10), "FT_END_DATE" => ("finish", 10), "FT_MONEY" => ("cost", 10),
            "FT_FLOAT_2_DECIMALS" | "FT_INT" => ("number", 20), _ => ("text", 30),
        };
        let Some(key) = (1..=max).map(|k| format!("{pre}{k}")).find(|k| !p.fields.iter().any(|d| d.key == *k)) else { full += 1; continue };
        p.fields.push(CustomDef { key: key.clone(), alias: label, ..Default::default() });
        for v in vals {
            let Some(i) = task(v) else { continue };
            let val = match pre {
                "start" | "finish" => date(g(v, "udf_date")).map(|d| serde_json::json!(d.to_string())),
                "duration" => g(v, "udf_number").parse::<f64>().ok().map(|d| serde_json::json!((d * p.day_min as f64).round() as i64)),
                "number" | "cost" => g(v, "udf_number").parse::<f64>().ok().map(|n| serde_json::json!(n)),
                _ => Some(g(v, "udf_text")).filter(|x| !x.is_empty()).map(|x| serde_json::json!(x)),
            };
            if let Some(val) = val { p.tasks[i].custom.insert(key.clone(), val); }
        }
    }
    if full > 0 { notes.push(format!("{full} custom fields did not fit (every custom field of that kind is in use) and were left out.")) }
    // Notes: the "Notes" topic goes to the task's notes, the other topics to its notebook.
    let topic: HashMap<String, String> = rows("MEMOTYPE").iter().map(|m| (g(m, "memo_type_id"), g(m, "memo_type"))).collect();
    let text = |html: &str| {
        let mut out = String::new();
        let mut tag = false;
        let x = html.replace("\u{7f}\u{7f}", "\n").replace("<br>", "\n").replace("<BR>", "\n").replace("<br/>", "\n").replace("</p>", "\n").replace("</P>", "\n");
        for ch in x.chars() { match ch { '<' => tag = true, '>' => tag = false, c if !tag => out.push(c), _ => {} } }
        out.replace("&nbsp;", " ").replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&amp;", "&").trim().to_string()
    };
    let put = |i: usize, ty: &str, body: &str, p: &mut Project| {
        let (name, body) = (topic.get(ty).cloned().unwrap_or_else(|| "Notes".into()), text(body));
        if body.is_empty() { return }
        let t = &mut p.tasks[i];
        if name.eq_ignore_ascii_case("Notes") && t.notes.is_empty() { t.notes = body } else { t.notebook.push(NoteTopic { topic: name, text: body }) }
    };
    for m in rows("TASKMEMO") { if let Some(i) = task(m) { put(i, &g(m, "memo_type_id"), &g(m, "task_memo"), p) } }
    for m in rows("WBSMEMO") { if let Some(&i) = wbs_uid.get(&g(m, "wbs_id")).and_then(|u| idx.get(u)) { put(i, &g(m, "memo_type_id"), &g(m, "wbs_memo"), p) } }
    // Steps, in their order.
    let mut steps: Vec<&Row> = rows("TASKPROC").iter().collect();
    steps.sort_by_key(|s| (g(s, "task_id"), g(s, "seq_num").parse::<i64>().unwrap_or(0)));
    for s in steps {
        let Some(i) = task(s) else { continue };
        let done = g(s, "complete_flag") == "Y" || g(s, "complete_pct").parse::<f64>().unwrap_or(0.0) >= 100.0;
        p.tasks[i].steps.push(Step { name: g(s, "proc_name"), weight: g(s, "proc_wt").parse().unwrap_or(1.0), done });
    }
}
