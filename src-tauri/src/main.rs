//! Datum Plan desktop shell: owns the open plan and talks to the window.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod update;

use datum_core::{doc::{Doc, Op}, sample, view, Project};
use serde::Serialize;
use std::{path::{Path, PathBuf}, sync::Mutex};
use tauri::{AppHandle, Emitter, Manager, State, WindowEvent};
use tauri_plugin_dialog::DialogExt;

struct St(Mutex<Doc>);
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
}

fn out(d: &Doc) -> Out {
    Out {
        view: view::build(&d.p),
        path: d.path.as_ref().map(|p| p.display().to_string()),
        dirty: d.dirty,
        undo: d.can_undo(),
        redo: d.can_redo(),
    }
}

fn today() -> chrono::NaiveDate {
    chrono::Local::now().date_naive()
}

#[tauri::command]
fn state(st: State<St>) -> Out {
    out(&st.0.lock().unwrap())
}

#[tauri::command]
fn apply(st: State<St>, op: Op) -> R<Out> {
    let mut d = st.0.lock().unwrap();
    d.apply(op)?;
    Ok(out(&d))
}

#[tauri::command]
fn history(st: State<St>, redo: bool) -> Out {
    let mut d = st.0.lock().unwrap();
    if redo { d.redo() } else { d.undo() }
    out(&d)
}

#[tauri::command]
fn new_plan(st: State<St>, example: bool) -> Out {
    let p = if example { sample(today()) } else { Project::new("Project1", today()) };
    let mut d = st.0.lock().unwrap();
    *d = Doc::new(p);
    out(&d)
}

#[tauri::command]
async fn open(app: AppHandle, st: State<'_, St>, path: Option<String>) -> R<Option<Out>> {
    let path = match path {
        Some(p) => PathBuf::from(p),
        None => match app.dialog().file().add_filter("Datum Plan", &["dplan"]).blocking_pick_file() {
            Some(f) => f.into_path().map_err(|e| e.to_string())?,
            None => return Ok(None),
        },
    };
    let d = Doc::open(&path)?;
    remember(&app, &path);
    let mut cur = st.0.lock().unwrap();
    *cur = d;
    Ok(Some(out(&cur)))
}

#[tauri::command]
async fn save(app: AppHandle, st: State<'_, St>, as_new: bool) -> R<Option<Out>> {
    let (known, name) = {
        let d = st.0.lock().unwrap();
        (d.path.clone().filter(|_| !as_new), d.p.name.clone())
    };
    let path = match known {
        Some(p) => p,
        None => match app.dialog().file().add_filter("Datum Plan", &["dplan"]).set_file_name(format!("{name}.dplan")).blocking_save_file() {
            Some(f) => f.into_path().map_err(|e| e.to_string())?,
            None => return Ok(None),
        },
    };
    let mut d = st.0.lock().unwrap();
    d.save(&path)?;
    remember(&app, &path);
    Ok(Some(out(&d)))
}

#[derive(Serialize)]
struct Recent { path: String, name: String, modified: String }

fn recent_file(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_config_dir().ok().map(|d| d.join("recent.json"))
}

fn recent_paths(app: &AppHandle) -> Vec<String> {
    recent_file(app).and_then(|f| std::fs::read_to_string(f).ok()).and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

fn remember(app: &AppHandle, path: &Path) {
    let p = path.display().to_string();
    let mut v = recent_paths(app);
    v.retain(|x| *x != p);
    v.insert(0, p);
    v.truncate(15);
    if let Some(f) = recent_file(app) {
        let _ = std::fs::create_dir_all(f.parent().unwrap());
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
        .manage(St(Mutex::new(Doc::new(Project::new("Project1", today())))))
        .on_window_event(|w, e| {
            if let WindowEvent::CloseRequested { api, .. } = e {
                if w.state::<St>().0.lock().unwrap().dirty {
                    api.prevent_close();
                    let _ = w.emit("close-requested", ());
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            state, apply, history, new_plan, open, save, recent, startup_file, version, quit, check_update, install_update
        ])
        .run(tauri::generate_context!())
        .expect("Datum Plan failed to start");
}
