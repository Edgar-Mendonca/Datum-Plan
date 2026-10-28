//! Datum Plan desktop shell: owns the open plan, files, options and recovery, and talks to the window.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod update;

use datum_core::{check, doc::{self, Doc, Op}, fields::{self, Calc}, io, network, query::{self, Query}, report, templates, Project, Task};
use serde::{Deserialize, Serialize};
use std::{path::{Path, PathBuf}, sync::Mutex};
use tauri::{AppHandle, Emitter, Manager, State, WindowEvent};
use tauri_plugin_dialog::DialogExt;

#[derive(Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct Options {
    /// chrono format for dates, such as %d-%b-%y.
    date_fmt: String,
    /// Minutes between recovery saves; 0 turns them off.
    autosave: u32,
    /// Offer beta (pre-release) versions in Check for updates.
    beta: bool,
    /// Defaults for new plans: currency, company and logo for reports.
    currency: datum_core::Currency,
    company: String,
    logo: String,
    /// The folder whose plans make up the workspace (portfolio).
    workspace: String,
    /// Ask for project details when a plan is created.
    setup_panel: bool,
    /// auto, light or dark.
    theme: String,
    /// Reopen the last plan at start, and the view a plan opens in.
    open_last: bool,
    start_view: String,
    /// Defaults for new plans: manual scheduling, week end day (0 = Mon … 6 = Sun), month close day (0 = last).
    manual_default: bool,
    week_end: u8,
    month_end: u8,
    /// Default paper and orientation for printing.
    paper: String,
    landscape: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self { date_fmt: "%d-%b-%y".into(), autosave: 5, beta: false, currency: Default::default(), company: String::new(), logo: String::new(), workspace: String::new(), setup_panel: true, theme: "auto".into(), open_last: false, start_view: "gantt".into(), manual_default: false, week_end: 4, month_end: 0, paper: "A4".into(), landscape: true }
    }
}

struct App {
    doc: Doc,
    opts: Options,
    clip: Vec<Task>,
    table: Option<(io::table::Table, String)>,
}

struct St(Mutex<App>);

impl St {
    /// The open plan. A command that panicked must not lock every later one out, so a poisoned lock is reused.
    fn lock(&self) -> std::sync::MutexGuard<'_, App> { self.0.lock().unwrap_or_else(|e| e.into_inner()) }
}
type R<T> = Result<T, String>;

/// What the window needs after every change: the project settings (everything except tasks),
/// its overall figures and the file state. Rows come separately through `query`.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Out {
    project: serde_json::Value,
    summary: query::Summary,
    path: Option<String>,
    dirty: bool,
    undo: bool,
    redo: bool,
    clip: usize,
    rev: u64,
}

fn out(a: &App) -> Out {
    let d = &a.doc;
    let mut project = serde_json::to_value(&d.p).unwrap_or_default();
    if let Some(m) = project.as_object_mut() {
        m.remove("tasks");
        if let Some(snaps) = m.get_mut("snapshots").and_then(|v| v.as_array_mut()) {
            snaps.iter_mut().filter_map(|x| x.as_object_mut()).for_each(|x| { x.remove("rows"); });
        }
    }
    Out {
        project,
        summary: calc(a).summary(),
        path: d.path.as_ref().map(|p| p.display().to_string()),
        dirty: d.dirty,
        undo: d.can_undo(),
        redo: d.can_redo(),
        clip: a.clip.len(),
        rev: d.undo_len() as u64 * 1_000_003 + d.p.next_uid as u64,
    }
}

fn calc(a: &App) -> Calc<'_> {
    Calc::new(&a.doc.p, &a.opts.date_fmt, today())
}

/// A new plan gets the user's defaults for currency and branding.
fn fresh(a: &App, mut p: Project) -> Project {
    p.currency = a.opts.currency.clone();
    if p.company.is_empty() {
        p.company = a.opts.company.clone();
    }
    if p.logo.is_empty() {
        p.logo = a.opts.logo.clone();
    }
    p.setup_done = !a.opts.setup_panel;
    p.manual_default = a.opts.manual_default;
    p.periods.week_end = a.opts.week_end;
    p.periods.month_end = a.opts.month_end;
    p
}

fn today() -> chrono::NaiveDate {
    chrono::Local::now().date_naive()
}

fn dir(app: &AppHandle, data: bool) -> Option<PathBuf> {
    let d = if data { app.path().app_data_dir() } else { app.path().app_config_dir() }.ok()?;
    std::fs::create_dir_all(&d).ok()?;
    Some(d)
}

#[tauri::command]
fn state(st: State<St>) -> Out {
    out(&st.lock())
}

#[tauri::command]
fn apply(st: State<St>, op: Op) -> R<Out> {
    let mut a = st.lock();
    let fmt = a.opts.date_fmt.clone();
    a.doc.apply(op, &fmt)?;
    Ok(out(&a))
}

#[tauri::command]
fn history(st: State<St>, redo: bool) -> Out {
    let mut a = st.lock();
    if redo { a.doc.redo() } else { a.doc.undo() }
    out(&a)
}

/// Templates start next Monday so the first week is a full one.
fn next_monday() -> chrono::NaiveDate {
    use chrono::Datelike;
    let t = today();
    t + chrono::Days::new(7 - t.weekday().num_days_from_monday() as u64)
}

#[derive(Serialize)]
struct Tpl { id: &'static str, name: &'static str, desc: &'static str, color: &'static str, tasks: usize, days: i64, bars: Vec<(u8, f64, f64)> }

/// Template cards with a miniature Gantt: (0 task / 1 summary / 2 milestone, start, end) as 0–1 fractions.
#[tauri::command]
fn templates() -> Vec<Tpl> {
    templates::TEMPLATES
        .iter()
        .map(|t| {
            let p = templates::build(t, next_monday());
            let c = Calc::new(&p, "%d-%b-%y", today());
            let (s0, f0) = (c.s.start as f64 / 1440.0, c.s.finish as f64 / 1440.0);
            let span = (f0 - s0).max(1.0);
            let bars = (0..p.tasks.len()).map(|i| {
                let kind = if c.s.summary[i] { 1 } else if p.tasks[i].dur == 0 { 2 } else { 0 };
                (kind, (c.s.es[i] as f64 / 1440.0 - s0) / span, (c.s.ef[i] as f64 / 1440.0 - s0) / span)
            }).collect();
            Tpl { id: t.id, name: t.name, desc: t.desc, color: t.color, tasks: (0..p.tasks.len()).filter(|&i| !c.s.summary[i]).count(), days: (f0 - s0).ceil() as i64, bars }
        })
        .collect()
}

#[tauri::command]
fn new_plan(st: State<St>, template: Option<String>) -> Out {
    let p = match templates::TEMPLATES.iter().find(|t| Some(t.id) == template.as_deref()) {
        Some(t) => templates::build(t, next_monday()),
        None => Project::new("Project1", today()),
    };
    let mut a = st.lock();
    let p = fresh(&a, p);
    a.doc = Doc::new(p);
    out(&a)
}

fn pick(app: &AppHandle, name: &str, exts: &[&str]) -> Option<PathBuf> {
    app.dialog().file().add_filter(name, exts).blocking_pick_file().and_then(|f| f.into_path().ok())
}

#[tauri::command]
async fn open(app: AppHandle, st: State<'_, St>, path: Option<String>) -> R<Option<Out>> {
    let Some(path) = path.map(PathBuf::from).or_else(|| pick(&app, "Datum Plan", &["dplan"])) else { return Ok(None) };
    backup_old(&path);
    let d = Doc::open(&path)?;
    remember(&app, &path);
    let mut a = st.lock();
    a.doc = d;
    Ok(Some(out(&a)))
}

/// Plans saved by an older version are copied to name.backup.dplan before they are opened,
/// because saving them again writes the newer format.
fn backup_old(path: &Path) {
    let old = std::fs::read_to_string(path).ok().and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok()).and_then(|v| v["format"].as_u64()).is_some_and(|f| f < 3);
    let backup = path.with_extension("backup.dplan");
    if old && !backup.exists() {
        let _ = std::fs::copy(path, backup);
    }
}

#[tauri::command]
async fn save(app: AppHandle, st: State<'_, St>, as_new: bool) -> R<Option<Out>> {
    let (known, name) = {
        let a = st.lock();
        (a.doc.path.clone().filter(|_| !as_new), a.doc.p.name.clone())
    };
    let path = match known {
        Some(p) => p,
        None => match app.dialog().file().add_filter("Datum Plan", &["dplan"]).set_file_name(format!("{name}.dplan")).blocking_save_file() {
            Some(f) => f.into_path().map_err(|e| e.to_string())?,
            None => return Ok(None),
        },
    };
    let mut a = st.lock();
    a.doc.save(&path)?;
    remember(&app, &path);
    clear_recovery(&app);
    Ok(Some(out(&a)))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Imported {
    plan: Option<Out>,
    /// The import report: what was changed or left out, and how the dates compare with the file.
    notes: Vec<String>,
    table: Option<io::table::Table>,
    fields: Vec<(&'static str, &'static str)>,
}

/// Opens an exchange file. Plans load straight away; spreadsheets come back for column mapping.
#[tauri::command]
async fn import(app: AppHandle, st: State<'_, St>) -> R<Option<Imported>> {
    let Some(path) = pick(&app, "Project files", &["xml", "xer", "xlsx", "xls", "csv", "json", "mpp"]) else { return Ok(None) };
    let name = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let fields = io::table::FIELDS.iter().map(|f| (f.0, f.1)).collect();
    let mut a = st.lock();
    match io::import(&path)? {
        io::Imported::Plan(p, notes) => {
            let p = fresh(&a, *p);
            a.doc = Doc::new(p);
            a.doc.dirty = true;
            Ok(Some(Imported { plan: Some(out(&a)), notes, table: None, fields }))
        }
        io::Imported::Table(t) => {
            let shown = io::table::Table { headers: t.headers.clone(), rows: t.rows.iter().take(8).cloned().collect(), guess: t.guess.clone() };
            a.table = Some((t, name));
            Ok(Some(Imported { plan: None, notes: vec![], table: Some(shown), fields }))
        }
    }
}

#[derive(Serialize)]
struct TableResult { plan: Out, skipped: usize }

#[tauri::command]
fn import_table(st: State<St>, map: Vec<String>) -> R<TableResult> {
    let mut a = st.lock();
    let (t, name) = a.table.take().ok_or("Choose a file to import first.")?;
    let (p, skipped) = io::table::to_project(&t, &map, &name, today(), &a.opts.date_fmt).inspect_err(|_| a.table = Some((io::table::Table { headers: t.headers.clone(), rows: t.rows.clone(), guess: t.guess.clone() }, name.clone())))?;
    let p = fresh(&a, p);
    a.doc = Doc::new(p);
    a.doc.dirty = true;
    Ok(TableResult { plan: out(&a), skipped })
}

#[tauri::command]
async fn export(app: AppHandle, st: State<'_, St>, kind: String, cols: Vec<String>) -> R<Option<String>> {
    let (label, ext) = match kind.as_str() {
        "xlsx" => ("Excel workbook", "xlsx"),
        "csv" => ("CSV", "csv"),
        "xml" => ("MS Project XML", "xml"),
        "xer" => ("Primavera P6 XER", "xer"),
        "pmxml" => ("Primavera P6 XML", "pmxml"),
        "mpx" => ("MPX", "mpx"),
        "json" => ("JSON", "json"),
        "html" => ("Web page", "html"),
        "ics" => ("iCalendar", "ics"),
        k => return Err(format!("Unknown export type {k}.")),
    };
    let name = st.lock().doc.p.name.clone();
    let Some(path) = app.dialog().file().add_filter(label, &[ext]).set_file_name(format!("{name}.{ext}")).blocking_save_file().and_then(|f| f.into_path().ok()) else { return Ok(None) };
    let a = st.lock();
    io::export(&a.doc.p, &path, &a.opts.date_fmt, today(), &cols)?;
    Ok(Some(path.display().to_string()))
}

/// Saves a picture or a report the window made (SVG, PNG data URL or HTML text).
#[tauri::command]
async fn save_file(app: AppHandle, name: String, data: String) -> R<Option<String>> {
    let ext = name.rsplit('.').next().unwrap_or("txt").to_string();
    let Some(path) = app.dialog().file().add_filter(ext.to_uppercase(), &[&ext]).set_file_name(&name).blocking_save_file().and_then(|f| f.into_path().ok()) else { return Ok(None) };
    let bytes = match data.strip_prefix("data:").and_then(|d| d.split_once(";base64,")) {
        Some((_, b64)) => base64_decode(b64)?,
        None => data.into_bytes(),
    };
    std::fs::write(&path, bytes).map_err(|e| format!("Could not save {}: {e}", path.display()))?;
    Ok(Some(path.display().to_string()))
}

fn base64_decode(s: &str) -> R<Vec<u8>> {
    let val = |c: u8| match c { b'A'..=b'Z' => Some(c - b'A'), b'a'..=b'z' => Some(c - b'a' + 26), b'0'..=b'9' => Some(c - b'0' + 52), b'+' => Some(62), b'/' => Some(63), _ => None };
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let (mut acc, mut bits) = (0u32, 0);
    for c in s.bytes().filter(|c| !c.is_ascii_whitespace() && *c != b'=') {
        acc = acc << 6 | val(c).ok_or("The picture data is damaged.")? as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }
    Ok(out)
}

/// Reads a picture for the report logo and returns it as a data URL.
#[tauri::command]
async fn pick_logo(app: AppHandle) -> R<Option<String>> {
    let Some(path) = pick(&app, "Pictures", &["png", "jpg", "jpeg", "svg", "gif"]) else { return Ok(None) };
    let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
    if bytes.len() > 1_500_000 {
        return Err("Choose a picture smaller than 1.5 MB.".into());
    }
    let mime = match path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase().as_str() { "svg" => "image/svg+xml", "jpg" | "jpeg" => "image/jpeg", "gif" => "image/gif", _ => "image/png" };
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut b64 = String::with_capacity(bytes.len() * 4 / 3 + 4);
    for ch in bytes.chunks(3) {
        let n = (ch[0] as u32) << 16 | (*ch.get(1).unwrap_or(&0) as u32) << 8 | *ch.get(2).unwrap_or(&0) as u32;
        for k in 0..4 {
            b64.push(if k <= ch.len() { T[(n >> (18 - 6 * k) & 63) as usize] as char } else { '=' });
        }
    }
    Ok(Some(format!("data:{mime};base64,{b64}")))
}

#[tauri::command]
fn copy(st: State<St>, uids: Vec<u32>) -> usize {
    let mut a = st.lock();
    a.clip = doc::copy(&a.doc.p, &uids);
    a.clip.len()
}

#[tauri::command]
fn paste(st: State<St>, at: usize) -> R<Out> {
    let mut a = st.lock();
    let (tasks, fmt) = (a.clip.clone(), a.opts.date_fmt.clone());
    a.doc.apply(Op::Paste { at, tasks }, &fmt)?;
    Ok(out(&a))
}

/// The rows a view shows: filtered, grouped, sorted, with cells and bars.
#[tauri::command]
fn query(st: State<St>, q: Query) -> query::Grid {
    calc(&st.lock()).grid(&q)
}

#[tauri::command]
fn resources(st: State<St>, cols: Vec<String>) -> Vec<(u32, bool, Vec<String>)> {
    calc(&st.lock()).res_grid(&cols)
}

#[derive(Serialize)]
struct FieldList { task: &'static [fields::FieldDef], resource: &'static [fields::FieldDef], filters: Vec<query::Filter> }

#[tauri::command]
fn field_list() -> FieldList {
    FieldList { task: fields::task_fields(), resource: fields::resource_fields(), filters: query::builtin_filters() }
}

#[derive(Serialize)]
struct TaskInfo { task: Task, fields: Vec<(String, String, &'static str, String)>, preds: Vec<Rel>, succs: Vec<Rel>, assigns: Vec<AssignRow> }

#[derive(Serialize)]
struct Rel { uid: u32, id: usize, code: String, name: String, kind: datum_core::LinkType, lag: String, driving: bool, tf: String }

#[derive(Serialize)]
struct AssignRow { res: u32, name: String, kind: String, units: String, work: String, cost: String }

/// Everything about one task, for Task Information, Inspect and the details tabs.
#[tauri::command]
fn task_info(st: State<St>, uid: u32) -> Option<TaskInfo> {
    let a = st.lock();
    let c = calc(&a);
    let p = c.p;
    let i = p.idx(uid)?;
    let t = &p.tasks[i];
    let rel = |k: usize, l: &datum_core::Link, driving: bool| Rel {
        uid: p.tasks[k].uid, id: k + 1, code: p.tasks[k].code.clone(), name: p.tasks[k].name.clone(), kind: l.kind,
        lag: if l.pct { format!("{}%", l.lag) } else { datum_core::parse::fmt_lag(l.lag, p.day_min) }, driving, tf: c.text(k, "tf"),
    };
    let drives = |a: usize, b: usize| c.s.edges.iter().enumerate().any(|(e, x)| x.p == a && x.s == b && c.s.drive[e]);
    let preds = t.preds.iter().filter_map(|l| p.idx(l.pred).map(|k| rel(k, l, drives(k, i)))).collect();
    let succs = p.tasks.iter().enumerate().flat_map(|(k, s)| s.preds.iter().filter(|l| l.pred == uid).map(move |l| (k, l))).map(|(k, l)| rel(k, l, drives(i, k))).collect();
    let assigns = t.res.iter().filter_map(|x| {
        let k = p.res_idx(x.res)?;
        let r = &p.resources[k];
        let w: f64 = c.l.assign_daily.range((k, i, i64::MIN)..=(k, i, i64::MAX)).map(|(_, m)| m).sum();
        let cost = match r.kind {
            datum_core::ResKind::Work => w / 60.0 * r.rate_on(x.table, p.start).0 + r.per_use,
            datum_core::ResKind::Material => x.units * r.rate + r.per_use,
            datum_core::ResKind::Cost => x.units + r.per_use,
        };
        let units = if r.kind == datum_core::ResKind::Work { format!("{}%", (x.units * 100.0).round()) } else { format!("{}", x.units) };
        Some(AssignRow { res: r.uid, name: r.name.clone(), kind: format!("{:?}", r.kind), units, work: format!("{} hrs", (w / 6.0).round() / 10.0), cost: fields::money(cost, &p.currency) })
    }).collect();
    Some(TaskInfo { task: t.clone(), fields: c.all_fields(i), preds, succs, assigns })
}

#[tauri::command]
fn usage(st: State<St>, by: String, what: String, unit: String) -> report::Usage {
    report::usage(&calc(&st.lock()), &by, &what, &unit)
}

#[tauri::command]
fn curve(st: State<St>, unit: String) -> report::Curve {
    report::curve(&calc(&st.lock()), &unit)
}

#[tauri::command]
/// Runs off the window's thread: a big plan takes a few seconds.
async fn risk_run(st: State<'_, St>, opts: datum_core::risk::SimOpts) -> R<datum_core::risk::SimResult> {
    let p = st.lock().doc.p.clone();
    datum_core::risk::simulate(&p, &opts)
}

#[tauri::command]
fn manpower(st: State<St>, unit: String) -> datum_core::progress::Manpower {
    datum_core::progress::manpower(&calc(&st.lock()), &unit)
}

#[tauri::command]
fn earned_schedule(st: State<St>) -> Option<datum_core::progress::EarnedSchedule> {
    datum_core::progress::earned_schedule(&calc(&st.lock()))
}

#[tauri::command]
fn lookahead(st: State<St>, weeks: u32) -> datum_core::progress::Lookahead {
    datum_core::progress::lookahead(&calc(&st.lock()), weeks)
}

#[tauri::command]
fn credit_library() -> Vec<datum_core::CreditRule> {
    datum_core::progress::credit_library()
}

#[tauri::command]
fn level_preview(st: State<St>, uids: Vec<u32>) -> R<datum_core::level::Preview> {
    datum_core::level::preview(&st.lock().doc.p, &uids)
}

#[tauri::command]
fn checks(st: State<St>, limits: check::Limits) -> Vec<check::Check> {
    check::run(&calc(&st.lock()), &limits)
}

/// Network Diagram of the tasks the query shows (summaries left out).
#[tauri::command]
fn network(st: State<St>, q: Query, summaries: Option<bool>, wbs: Option<bool>) -> network::Net {
    let a = st.lock();
    let c = calc(&a);
    let g = c.grid(&q);
    let pick = |k: &str| g.rows.iter().filter(|r| r.kind == k).filter_map(|r| c.p.idx(r.uid)).collect::<Vec<usize>>();
    let sums = if summaries.unwrap_or(false) { pick("summary") } else { vec![] };
    network::layout_with(&c, &pick("task"), &sums, wbs.unwrap_or(false))
}

/// Compares the open plan with another plan file.
#[tauri::command]
async fn compare(app: AppHandle, st: State<'_, St>) -> R<Option<(String, Vec<report::Diff>)>> {
    let Some(path) = pick(&app, "Datum Plan", &["dplan", "xml", "xer"]) else { return Ok(None) };
    let old = match io::import(&path)? {
        io::Imported::Plan(p, _) => *p,
        io::Imported::Table(_) => return Err("Compare needs a plan file (.dplan, .xml or .xer).".into()),
    };
    let a = st.lock();
    let fmt = a.opts.date_fmt.clone();
    let diffs = report::compare(&Calc::new(&old, &fmt, today()), &calc(&a));
    Ok(Some((path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(), diffs)))
}

#[derive(Serialize)]
struct PlanCard { path: String, folder: String, name: String, start: String, finish: String, pct: f64, cost: String, spi: Option<f64>, cpi: Option<f64>, late: usize, tasks: usize, manager: String, error: Option<String> }

/// Every plan in the workspace folder with its headline figures (portfolio view).
#[tauri::command]
async fn workspace(app: AppHandle, st: State<'_, St>, choose: bool) -> R<(String, Vec<PlanCard>)> {
    let mut folder = st.lock().opts.workspace.clone();
    if choose || folder.is_empty() {
        let Some(d) = app.dialog().file().blocking_pick_folder().and_then(|f| f.into_path().ok()) else { return Ok((folder, vec![])) };
        folder = d.display().to_string();
        let mut a = st.lock();
        a.opts.workspace = folder.clone();
        if let Some(f) = options_file(&app) {
            let _ = std::fs::write(f, serde_json::to_string(&a.opts).unwrap_or_default());
        }
    }
    let fmt = st.lock().opts.date_fmt.clone();
    // Subfolders are the EPS: each level of folders is a level of the portfolio tree.
    fn walk(dir: &Path, depth: u8, out: &mut Vec<PathBuf>) {
        let Ok(rd) = std::fs::read_dir(dir) else { return };
        for p in rd.filter_map(|e| e.ok().map(|e| e.path())) {
            if p.is_dir() && depth < 4 { walk(&p, depth + 1, out) }
            else if p.extension().is_some_and(|x| x == "dplan") && !p.to_string_lossy().ends_with(".backup.dplan") { out.push(p) }
        }
    }
    std::fs::read_dir(&folder).map_err(|e| format!("Could not read {folder}: {e}"))?;
    let mut files = vec![];
    walk(Path::new(&folder), 0, &mut files);
    files.sort();
    let rel = |f: &Path| f.parent().and_then(|d| d.strip_prefix(&folder).ok()).map(|d| d.display().to_string().replace(['\\', '/'], " › ")).unwrap_or_default();
    let cards = files.iter().take(200).map(|f| {
        let path = f.display().to_string();
        match Doc::open(f) {
            Ok(d) => {
                let c = Calc::new(&d.p, &fmt, today());
                let s = c.summary();
                PlanCard { path, folder: rel(f), name: d.p.name.clone(), start: s.start, finish: s.finish, pct: s.pct, cost: s.cost, spi: s.spi, cpi: s.cpi, late: s.late, tasks: s.tasks, manager: d.p.manager.clone(), error: s.error }
            }
            Err(e) => PlanCard { path, folder: rel(f), name: f.file_stem().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(), start: String::new(), finish: String::new(), pct: 0.0, cost: String::new(), spi: None, cpi: None, late: 0, tasks: 0, manager: String::new(), error: Some(e) },
        }
    }).collect();
    Ok((folder, cards))
}

#[tauri::command]
fn sheet_view(st: State<St>, k: usize) -> Option<datum_core::sheet::SheetOut> {
    datum_core::sheet::sheet_view(&calc(&st.lock()), k)
}

#[tauri::command]
fn scurve(st: State<St>, unit: String, how: String, scope: Option<u32>) -> report::SCurve {
    report::scurve(&calc(&st.lock()), &unit, &how, scope)
}

#[tauri::command]
fn periods(st: State<St>, kind: String) -> Vec<i64> {
    report::period_ends(&calc(&st.lock()), &kind)
}

#[tauri::command]
fn period_report(st: State<St>, kind: String, end: Option<i64>) -> report::PeriodReport {
    report::period_report(&calc(&st.lock()), &kind, end)
}

/// The full plan of a snapshot, for trend charts (rows are left out of `state`).
#[tauri::command]
fn snapshot_rows(st: State<St>, k: usize) -> Option<datum_core::Snapshot> {
    st.lock().doc.p.snapshots.get(k).cloned()
}

fn options_file(app: &AppHandle) -> Option<PathBuf> {
    dir(app, false).map(|d| d.join("options.json"))
}

#[tauri::command]
fn options(st: State<St>) -> Options {
    st.lock().opts.clone()
}

#[tauri::command]
fn set_options(app: AppHandle, st: State<St>, opts: Options) -> Out {
    if let Some(f) = options_file(&app) {
        let _ = std::fs::write(f, serde_json::to_string(&opts).unwrap_or_default());
    }
    let mut a = st.lock();
    a.opts = opts;
    out(&a)
}

// ---------- recovery: a copy of unsaved work, kept until it is saved or discarded ----------
#[derive(Serialize, Deserialize)]
struct Recovery { name: String, path: Option<String>, saved: String }

fn recovery_files(app: &AppHandle) -> Option<(PathBuf, PathBuf)> {
    dir(app, true).map(|d| (d.join("recovery.dplan"), d.join("recovery.json")))
}

fn clear_recovery(app: &AppHandle) {
    if let Some((a, b)) = recovery_files(app) {
        let _ = std::fs::remove_file(a);
        let _ = std::fs::remove_file(b);
    }
}

#[tauri::command]
fn autosave(app: AppHandle, st: State<St>) {
    let a = st.lock();
    if let (true, Some((plan, meta))) = (a.doc.dirty, recovery_files(&app)) {
        let info = Recovery { name: a.doc.p.name.clone(), path: a.doc.path.as_ref().map(|p| p.display().to_string()), saved: chrono::Local::now().format("%d-%b-%y %H:%M").to_string() };
        let _ = std::fs::write(plan, a.doc.json());
        let _ = std::fs::write(meta, serde_json::to_string(&info).unwrap_or_default());
    }
}

#[tauri::command]
fn recovery(app: AppHandle) -> Option<Recovery> {
    let (plan, meta) = recovery_files(&app)?;
    plan.exists().then_some(())?;
    serde_json::from_str(&std::fs::read_to_string(meta).ok()?).ok()
}

#[tauri::command]
fn recover(app: AppHandle, st: State<St>, keep: bool) -> R<Option<Out>> {
    let info = recovery(app.clone());
    let files = recovery_files(&app);
    if !keep {
        clear_recovery(&app);
        return Ok(None);
    }
    let (plan, _) = files.ok_or("No recovered plan was found.")?;
    let mut d = Doc::open(&plan)?;
    d.path = info.and_then(|i| i.path).map(PathBuf::from);
    d.dirty = true;
    let mut a = st.lock();
    a.doc = d;
    Ok(Some(out(&a)))
}

#[derive(Serialize)]
struct Recent { path: String, name: String, modified: String }

fn recent_paths(app: &AppHandle) -> Vec<String> {
    options_file(app).map(|f| f.with_file_name("recent.json")).and_then(|f| std::fs::read_to_string(f).ok()).and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

fn remember(app: &AppHandle, path: &Path) {
    let p = path.display().to_string();
    let mut v = recent_paths(app);
    v.retain(|x| *x != p);
    v.insert(0, p);
    v.truncate(15);
    if let Some(f) = options_file(app).map(|f| f.with_file_name("recent.json")) {
        let _ = std::fs::write(f, serde_json::to_string(&v).unwrap_or_default());
    }
}

#[tauri::command]
fn recent(app: AppHandle) -> Vec<Recent> {
    recent_paths(&app)
        .into_iter()
        .filter_map(|p| {
            let m = std::fs::metadata(&p).ok()?.modified().ok()?;
            let modified = chrono::DateTime::<chrono::Local>::from(m).format("%d-%b-%y %H:%M").to_string();
            let name = Path::new(&p).file_stem()?.to_string_lossy().into_owned();
            Some(Recent { path: p, name, modified })
        })
        .collect()
}

/// A .dplan path passed on the command line (double-clicked in Explorer).
#[tauri::command]
fn startup_file(app: AppHandle, st: State<St>) -> Option<String> {
    let last = || if st.lock().opts.open_last { recent_paths(&app).into_iter().find(|p| Path::new(p).exists()) } else { None };
    std::env::args().skip(1).find(|a| a.to_lowercase().ends_with(".dplan")).or_else(last)
}

/// Opens one of the project's GitHub pages in the browser (only those, so the window cannot open anything else).
#[tauri::command]
fn open_url(url: String) -> R<()> {
    if !url.starts_with("https://github.com/Edgar-Mendonca/Datum-Plan") {
        return Err("Only the Datum Plan GitHub pages can be opened.".into());
    }
    #[cfg(windows)]
    let r = std::process::Command::new("explorer").arg(&url).spawn();
    #[cfg(not(windows))]
    let r = std::process::Command::new("xdg-open").arg(&url).spawn();
    r.map(|_| ()).map_err(|e| e.to_string())
}

#[tauri::command]
fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[tauri::command]
fn quit(app: AppHandle) {
    clear_recovery(&app);
    app.exit(0);
}

#[tauri::command]
async fn check_update(st: State<'_, St>) -> R<update::Info> {
    let beta = st.lock().opts.beta;
    tauri::async_runtime::spawn_blocking(move || update::check(beta)).await.map_err(|e| e.to_string())?
}

#[tauri::command]
async fn install_update(app: AppHandle, url: String, sums: Option<String>) -> R<()> {
    tauri::async_runtime::spawn_blocking(move || update::install(&app, &url, sums.as_deref())).await.map_err(|e| e.to_string())?
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let opts = options_file(app.handle()).and_then(|f| std::fs::read_to_string(f).ok()).and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
            app.manage(St(Mutex::new(App { doc: Doc::new(Project::new("Project1", today())), opts, clip: vec![], table: None })));
            Ok(())
        })
        .on_window_event(|w, e| {
            if let WindowEvent::CloseRequested { api, .. } = e {
                if w.state::<St>().lock().doc.dirty {
                    api.prevent_close();
                    let _ = w.emit("close-requested", ());
                } else {
                    clear_recovery(w.app_handle());
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            state, apply, history, new_plan, templates, open, save, import, import_table, export, save_file, pick_logo, copy, paste,
            query, resources, field_list, task_info, usage, curve, checks, risk_run, level_preview, manpower, earned_schedule, lookahead, credit_library, network, compare, workspace, snapshot_rows, sheet_view, scurve, periods, period_report,
            options, set_options, autosave, recovery, recover, recent, startup_file, version, open_url, quit, check_update, install_update
        ])
        .run(tauri::generate_context!())
        .expect("Datum Plan failed to start");
}
