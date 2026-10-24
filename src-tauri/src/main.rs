//! Datum Plan desktop shell: owns the open plan, files, options and recovery, and talks to the window.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod update;

use datum_core::{doc::{self, Doc, Op}, io, load, sched, templates, view, Project, Task};
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
}

impl Default for Options {
    fn default() -> Self {
        Self { date_fmt: "%d-%b-%y".into(), autosave: 5 }
    }
}

struct App {
    doc: Doc,
    opts: Options,
    clip: Vec<Task>,
    table: Option<(io::table::Table, String)>,
}

struct St(Mutex<App>);
type R<T> = Result<T, String>;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Out {
    #[serde(flatten)]
    view: view::View,
    path: Option<String>,
    dirty: bool,
    undo: bool,
    redo: bool,
    clip: usize,
}

fn out(a: &App) -> Out {
    let d = &a.doc;
    Out {
        view: view::build(&d.p, &a.opts.date_fmt, today()),
        path: d.path.as_ref().map(|p| p.display().to_string()),
        dirty: d.dirty,
        undo: d.can_undo(),
        redo: d.can_redo(),
        clip: a.clip.len(),
    }
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
    out(&st.0.lock().unwrap())
}

#[tauri::command]
fn apply(st: State<St>, op: Op) -> R<Out> {
    let mut a = st.0.lock().unwrap();
    let fmt = a.opts.date_fmt.clone();
    a.doc.apply(op, &fmt)?;
    Ok(out(&a))
}

#[tauri::command]
fn history(st: State<St>, redo: bool) -> Out {
    let mut a = st.0.lock().unwrap();
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
            let v = view::build(&templates::build(t, next_monday()), "%d-%b-%y", today());
            let span = (v.f - v.s).max(1.0);
            let bars = v.rows.iter().map(|r| (if r.ms { 2 } else { r.sum as u8 }, (r.s - v.s) / span, (r.f - v.s) / span)).collect();
            Tpl { id: t.id, name: t.name, desc: t.desc, color: t.color, tasks: v.rows.iter().filter(|r| !r.sum).count(), days: (v.f - v.s).ceil() as i64, bars }
        })
        .collect()
}

#[tauri::command]
fn new_plan(st: State<St>, template: Option<String>) -> Out {
    let p = match templates::TEMPLATES.iter().find(|t| Some(t.id) == template.as_deref()) {
        Some(t) => templates::build(t, next_monday()),
        None => Project::new("Project1", today()),
    };
    let mut a = st.0.lock().unwrap();
    a.doc = Doc::new(p);
    out(&a)
}

fn pick(app: &AppHandle, name: &str, exts: &[&str]) -> Option<PathBuf> {
    app.dialog().file().add_filter(name, exts).blocking_pick_file().and_then(|f| f.into_path().ok())
}

#[tauri::command]
async fn open(app: AppHandle, st: State<'_, St>, path: Option<String>) -> R<Option<Out>> {
    let Some(path) = path.map(PathBuf::from).or_else(|| pick(&app, "Datum Plan", &["dplan"])) else { return Ok(None) };
    let d = Doc::open(&path)?;
    remember(&app, &path);
    let mut a = st.0.lock().unwrap();
    a.doc = d;
    Ok(Some(out(&a)))
}

#[tauri::command]
async fn save(app: AppHandle, st: State<'_, St>, as_new: bool) -> R<Option<Out>> {
    let (known, name) = {
        let a = st.0.lock().unwrap();
        (a.doc.path.clone().filter(|_| !as_new), a.doc.p.name.clone())
    };
    let path = match known {
        Some(p) => p,
        None => match app.dialog().file().add_filter("Datum Plan", &["dplan"]).set_file_name(format!("{name}.dplan")).blocking_save_file() {
            Some(f) => f.into_path().map_err(|e| e.to_string())?,
            None => return Ok(None),
        },
    };
    let mut a = st.0.lock().unwrap();
    a.doc.save(&path)?;
    remember(&app, &path);
    clear_recovery(&app);
    Ok(Some(out(&a)))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Imported {
    plan: Option<Out>,
    table: Option<io::table::Table>,
    fields: Vec<(&'static str, &'static str)>,
}

/// Opens an exchange file. Plans load straight away; spreadsheets come back for column mapping.
#[tauri::command]
async fn import(app: AppHandle, st: State<'_, St>) -> R<Option<Imported>> {
    let Some(path) = pick(&app, "Project files", &["xml", "xer", "xlsx", "xls", "csv", "mpp"]) else { return Ok(None) };
    let name = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let fields = io::table::FIELDS.iter().map(|f| (f.0, f.1)).collect();
    let mut a = st.0.lock().unwrap();
    match io::import(&path)? {
        io::Imported::Plan(p) => {
            a.doc = Doc::new(p);
            a.doc.dirty = true;
            Ok(Some(Imported { plan: Some(out(&a)), table: None, fields }))
        }
        io::Imported::Table(t) => {
            let shown = io::table::Table { headers: t.headers.clone(), rows: t.rows.iter().take(8).cloned().collect(), guess: t.guess.clone() };
            a.table = Some((t, name));
            Ok(Some(Imported { plan: None, table: Some(shown), fields }))
        }
    }
}

#[derive(Serialize)]
struct TableResult { plan: Out, skipped: usize }

#[tauri::command]
fn import_table(st: State<St>, map: Vec<String>) -> R<TableResult> {
    let mut a = st.0.lock().unwrap();
    let (t, name) = a.table.take().ok_or("Choose a file to import first.")?;
    let (p, skipped) = io::table::to_project(&t, &map, &name, today(), &a.opts.date_fmt).inspect_err(|_| a.table = Some((io::table::Table { headers: t.headers.clone(), rows: t.rows.clone(), guess: t.guess.clone() }, name.clone())))?;
    a.doc = Doc::new(p);
    a.doc.dirty = true;
    Ok(TableResult { plan: out(&a), skipped })
}

#[tauri::command]
async fn export(app: AppHandle, st: State<'_, St>, kind: String) -> R<Option<String>> {
    let (label, ext) = match kind.as_str() {
        "xlsx" => ("Excel workbook", "xlsx"),
        "csv" => ("CSV", "csv"),
        "xml" => ("MS Project XML", "xml"),
        "xer" => ("Primavera P6 XER", "xer"),
        k => return Err(format!("Unknown export type {k}.")),
    };
    let name = st.0.lock().unwrap().doc.p.name.clone();
    let Some(path) = app.dialog().file().add_filter(label, &[ext]).set_file_name(format!("{name}.{ext}")).blocking_save_file().and_then(|f| f.into_path().ok()) else { return Ok(None) };
    let a = st.0.lock().unwrap();
    io::export(&a.doc.p, &path, &a.opts.date_fmt, today())?;
    Ok(Some(path.display().to_string()))
}

#[tauri::command]
fn copy(st: State<St>, uids: Vec<u32>) -> usize {
    let mut a = st.0.lock().unwrap();
    a.clip = doc::copy(&a.doc.p, &uids);
    a.clip.len()
}

#[tauri::command]
fn paste(st: State<St>, at: usize) -> R<Out> {
    let mut a = st.0.lock().unwrap();
    let (tasks, fmt) = (a.clip.clone(), a.opts.date_fmt.clone());
    a.doc.apply(Op::Paste { at, tasks }, &fmt)?;
    Ok(out(&a))
}

#[tauri::command]
fn detail(st: State<St>, uid: u32) -> Option<view::Detail> {
    view::detail(&st.0.lock().unwrap().doc.p, uid)
}

#[tauri::command]
fn workload(st: State<St>, weekly: bool) -> R<load::Workload> {
    let a = st.0.lock().unwrap();
    Ok(load::workload(&a.doc.p, &sched::schedule(&a.doc.p)?, weekly))
}

fn options_file(app: &AppHandle) -> Option<PathBuf> {
    dir(app, false).map(|d| d.join("options.json"))
}

#[tauri::command]
fn options(st: State<St>) -> Options {
    st.0.lock().unwrap().opts.clone()
}

#[tauri::command]
fn set_options(app: AppHandle, st: State<St>, opts: Options) -> Out {
    if let Some(f) = options_file(&app) {
        let _ = std::fs::write(f, serde_json::to_string(&opts).unwrap_or_default());
    }
    let mut a = st.0.lock().unwrap();
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
    let a = st.0.lock().unwrap();
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
    let mut a = st.0.lock().unwrap();
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
        let _ = std::fs::write(f, serde_json::to_string(&v).unwrap());
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
fn startup_file() -> Option<String> {
    std::env::args().skip(1).find(|a| a.to_lowercase().ends_with(".dplan"))
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
async fn check_update() -> R<update::Info> {
    tauri::async_runtime::spawn_blocking(update::check).await.map_err(|e| e.to_string())?
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
                if w.state::<St>().0.lock().unwrap().doc.dirty {
                    api.prevent_close();
                    let _ = w.emit("close-requested", ());
                } else {
                    clear_recovery(w.app_handle());
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            state, apply, history, new_plan, templates, open, save, import, import_table, export, copy, paste, detail, workload,
            options, set_options, autosave, recovery, recover, recent, startup_file, version, quit, check_update, install_update
        ])
        .run(tauri::generate_context!())
        .expect("Datum Plan failed to start");
}
