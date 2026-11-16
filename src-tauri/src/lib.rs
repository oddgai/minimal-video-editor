pub mod detection;
pub mod export;
pub mod media;
pub mod preview;

use export::{ExportControl, ExportRequest, Progress};
use media::{MediaInfo, ScanControl, Tools};
use serde::Serialize;
use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::{Emitter, Manager};
use tauri_plugin_dialog::DialogExt;

#[derive(Clone)]
struct Session {
    info: MediaInfo,
    keys: Option<Vec<f64>>,
}

#[derive(Default)]
struct AppState {
    session: Arc<Mutex<Option<Session>>>,
    export: Arc<Mutex<Option<ExportControl>>>,
    destination: Arc<Mutex<Option<PathBuf>>>,
    scan: Arc<Mutex<Option<ScanControl>>>,
    preview: Arc<Mutex<Option<Arc<preview::PreviewServer>>>>,
    detection: Arc<Mutex<Option<detection::DetectionControl>>>,
    detection_cache: Arc<Mutex<detection::DetectionCache>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct OpenedMedia {
    #[serde(flatten)]
    info: MediaInfo,
    preview_url: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ExportProgress {
    job_id: String,
    fraction: f64,
    stage: String,
}

#[tauri::command]
async fn open_media(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    path: Option<String>,
) -> Result<Option<OpenedMedia>, String> {
    if state.export.lock().map_err(|e| e.to_string())?.is_some()
        || state.detection.lock().map_err(|e| e.to_string())?.is_some()
    {
        return Err("Wait for the current operation to finish before opening a file.".into());
    }
    let session = state.session.clone();
    let scan = state.scan.clone();
    let preview = state.preview.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let selected = if let Some(path) = path {
            Some(PathBuf::from(path))
        } else {
            app.dialog()
                .file()
                .set_title("Open file")
                .add_filter(
                    "Video",
                    &[
                        "mp4", "mov", "mkv", "webm", "m4v", "avi", "ts", "mts", "m2ts",
                    ],
                )
                .blocking_pick_file()
                .map(|file| file.into_path().map_err(|e| e.to_string()))
                .transpose()?
        };
        let Some(path) = selected else {
            return Ok(None);
        };
        let path = path
            .canonicalize()
            .map_err(|e| format!("Cannot open file: {e}"))?;
        let info = media::probe_media(&Tools::discover()?, &path)?;
        let preview_url = {
            let mut slot = preview.lock().map_err(|e| e.to_string())?;
            if slot.is_none() {
                *slot = Some(Arc::new(
                    preview::PreviewServer::start()
                        .map_err(|e| format!("Cannot start preview: {e}"))?,
                ));
            }
            slot.as_ref()
                .unwrap()
                .select(&path)
                .map_err(|e| e.to_string())?
        };
        if let Some(previous) = scan.lock().map_err(|e| e.to_string())?.take() {
            previous.cancel();
        }
        *session.lock().map_err(|e| e.to_string())? = Some(Session {
            info: info.clone(),
            keys: None,
        });
        Ok(Some(OpenedMedia { info, preview_url }))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn analyze_keyframes(state: tauri::State<'_, AppState>) -> Result<Vec<f64>, String> {
    let session = state.session.clone();
    let source = session
        .lock()
        .map_err(|e| e.to_string())?
        .as_ref()
        .ok_or("Open a video file first.")?
        .clone();
    if let Some(keys) = source.keys {
        return Ok(keys);
    }
    let scan_slot = state.scan.clone();
    let control = ScanControl::default();
    if let Some(previous) = scan_slot
        .lock()
        .map_err(|e| e.to_string())?
        .replace(control.clone())
    {
        previous.cancel();
    }
    let worker_control = control.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let keys = media::scan_keyframes_with_control(
            &Tools::discover()?,
            Path::new(&source.info.path),
            &worker_control,
        )?;
        if let Some(current) = session.lock().map_err(|e| e.to_string())?.as_mut() {
            if current.info.path == source.info.path {
                current.keys = Some(keys.clone());
            }
        }
        Ok(keys)
    })
    .await;
    let mut slot = scan_slot.lock().map_err(|e| e.to_string())?;
    if slot
        .as_ref()
        .is_some_and(|current| current.same_job(&control))
    {
        *slot = None;
    }
    result.map_err(|e| e.to_string())?
}

#[tauri::command]
async fn choose_output(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<Option<String>, String> {
    let info = state
        .session
        .lock()
        .map_err(|e| e.to_string())?
        .as_ref()
        .ok_or("Open a video file first.")?
        .info
        .clone();
    let destination = state.destination.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let input = Path::new(&info.path);
        let extension = input
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("mkv")
            .to_ascii_lowercase();
        let extension = if ["mp4", "mov", "mkv", "webm", "m4v"].contains(&extension.as_str()) {
            extension.as_str()
        } else {
            "mkv"
        };
        let stem = input.file_stem().unwrap_or_default().to_string_lossy();
        let selected = app
            .dialog()
            .file()
            .set_title("Export video")
            .set_directory(input.parent().unwrap_or(Path::new(".")))
            .set_file_name(format!("{stem}-edited.{extension}"))
            .add_filter("Video", &["mp4", "mov", "mkv", "webm", "m4v"])
            .blocking_save_file();
        let path = selected
            .map(|file| file.into_path().map_err(|e| e.to_string()))
            .transpose()?;
        *destination.lock().map_err(|e| e.to_string())? = path.clone();
        Ok(path.map(|p| p.to_string_lossy().into()))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn start_export(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    request: ExportRequest,
    job_id: String,
) -> Result<(), String> {
    let session = state
        .session
        .lock()
        .map_err(|e| e.to_string())?
        .clone()
        .ok_or("Open a video file first.")?;
    if request.input != session.info.path {
        return Err("The export source does not match the open file.".into());
    }
    if state
        .destination
        .lock()
        .map_err(|e| e.to_string())?
        .as_deref()
        != Some(Path::new(&request.output))
    {
        return Err("Choose an output file again.".into());
    }
    let keys = session
        .keys
        .ok_or("Wait for keyframe analysis to finish.")?;
    let control = ExportControl::default();
    let active = state.export.clone();
    {
        let mut slot = active.lock().map_err(|e| e.to_string())?;
        if slot.is_some() || state.detection.lock().map_err(|e| e.to_string())?.is_some() {
            return Err("Another export is already running.".into());
        }
        *slot = Some(control.clone());
    }
    let result = tauri::async_runtime::spawn_blocking(move || {
        export::export_with_keyframes(
            &Tools::discover()?,
            &request,
            &control,
            Some(&keys),
            |p: Progress| {
                let _ = app.emit(
                    "export-progress",
                    ExportProgress {
                        job_id: job_id.clone(),
                        fraction: p.fraction,
                        stage: p.stage,
                    },
                );
            },
        )
    })
    .await;
    *active.lock().map_err(|e| e.to_string())? = None;
    result.map_err(|e| e.to_string())?
}

#[tauri::command]
fn cancel_export(state: tauri::State<'_, AppState>) -> Result<(), String> {
    if let Some(control) = state.export.lock().map_err(|e| e.to_string())?.as_ref() {
        control.cancel();
    }
    Ok(())
}

#[tauri::command]
async fn detect_chapters(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    job_id: String,
    precise: bool,
) -> Result<Vec<f64>, String> {
    let session = state
        .session
        .lock()
        .map_err(|e| e.to_string())?
        .clone()
        .ok_or("Open a video file first.")?;
    let keys = session
        .keys
        .ok_or("Wait for keyframe analysis to finish.")?;
    let control = detection::DetectionControl::default();
    let active = state.detection.clone();
    {
        let export = state.export.lock().map_err(|e| e.to_string())?;
        let mut slot = active.lock().map_err(|e| e.to_string())?;
        if export.is_some() || slot.is_some() {
            return Err("Another operation is already running.".into());
        }
        *slot = Some(control.clone());
    }
    let worker_control = control.clone();
    let cache = state.detection_cache.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let reported = Mutex::new(-1.0_f64);
        let mut cache = cache.lock().map_err(|e| e.to_string())?;
        cache.detect_with_mode(
            &Tools::discover()?,
            Path::new(&session.info.path),
            &keys,
            precise,
            &worker_control,
            |p| {
                if let Ok(mut previous) = reported.lock() {
                    if p.fraction - *previous >= 0.005 || p.fraction >= 1.0 {
                        *previous = p.fraction;
                        let _ = app.emit(
                            "detection-progress",
                            ExportProgress {
                                job_id: job_id.clone(),
                                fraction: p.fraction,
                                stage: p.stage,
                            },
                        );
                    }
                }
            },
        )
    })
    .await;
    let mut slot = active.lock().map_err(|e| e.to_string())?;
    if slot
        .as_ref()
        .is_some_and(|current| current.same_job(&control))
    {
        *slot = None;
    }
    result.map_err(|e| e.to_string())?
}

#[tauri::command]
fn cancel_detection(state: tauri::State<'_, AppState>) -> Result<(), String> {
    if let Some(control) = state.detection.lock().map_err(|e| e.to_string())?.as_ref() {
        control.cancel();
    }
    Ok(())
}

#[tauri::command]
async fn close_media(state: tauri::State<'_, AppState>) -> Result<(), String> {
    if state.export.lock().map_err(|e| e.to_string())?.is_some()
        || state.detection.lock().map_err(|e| e.to_string())?.is_some()
    {
        return Err("Wait for the current operation to finish before closing the file.".into());
    }
    *state.session.lock().map_err(|e| e.to_string())? = None;
    state
        .detection_cache
        .lock()
        .map_err(|e| e.to_string())?
        .clear();
    *state.destination.lock().map_err(|e| e.to_string())? = None;
    if let Some(control) = state.scan.lock().map_err(|e| e.to_string())?.take() {
        control.cancel();
    }
    let server = state.preview.lock().map_err(|e| e.to_string())?.take();
    tauri::async_runtime::spawn_blocking(move || {
        if let Some(server) = server {
            server.shutdown();
        }
    })
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn quit_app(app: tauri::AppHandle, state: tauri::State<'_, AppState>) -> Result<(), String> {
    if state.export.lock().map_err(|e| e.to_string())?.is_none()
        && state.detection.lock().map_err(|e| e.to_string())?.is_none()
    {
        app.exit(0);
    }
    Ok(())
}

fn editor_menu(app: &tauri::AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    let quit = MenuItem::with_id(app, "quit", "Quit", true, Some("CmdOrCtrl+Q"))?;
    let file = Submenu::with_items(
        app,
        "File",
        true,
        &[
            &MenuItem::with_id(app, "open", "Open file…", true, Some("CmdOrCtrl+O"))?,
            &MenuItem::with_id(app, "export", "Export…", true, Some("CmdOrCtrl+S"))?,
            &MenuItem::with_id(
                app,
                "detect",
                "Detect chapters…",
                true,
                Some("CmdOrCtrl+Shift+D"),
            )?,
            &MenuItem::with_id(app, "close", "Close file", true, Some("CmdOrCtrl+W"))?,
            #[cfg(not(target_os = "macos"))]
            &quit,
        ],
    )?;
    let edit = Submenu::with_items(
        app,
        "Edit",
        true,
        &[
            &PredefinedMenuItem::undo(app, None)?,
            &PredefinedMenuItem::redo(app, None)?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::cut(app, None)?,
            &PredefinedMenuItem::copy(app, None)?,
            &PredefinedMenuItem::paste(app, None)?,
            &PredefinedMenuItem::select_all(app, None)?,
        ],
    )?;
    Menu::with_items(
        app,
        &[
            #[cfg(target_os = "macos")]
            &Submenu::with_items(
                app,
                "Minimal Video Editor",
                true,
                &[
                    &PredefinedMenuItem::about(app, None, None)?,
                    &PredefinedMenuItem::separator(app)?,
                    &PredefinedMenuItem::services(app, None)?,
                    &PredefinedMenuItem::separator(app)?,
                    &PredefinedMenuItem::hide(app, None)?,
                    &PredefinedMenuItem::hide_others(app, None)?,
                    &PredefinedMenuItem::show_all(app, None)?,
                    &PredefinedMenuItem::separator(app)?,
                    &quit,
                ],
            )?,
            &file,
            &edit,
            &Submenu::with_items(
                app,
                "Window",
                true,
                &[
                    &PredefinedMenuItem::minimize(app, None)?,
                    &PredefinedMenuItem::maximize(app, None)?,
                ],
            )?,
        ],
    )
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::default())
        .menu(editor_menu)
        .on_menu_event(|app, event| {
            let id = event.id().as_ref();
            if matches!(id, "open" | "export" | "close" | "quit" | "detect") {
                let _ = app.emit("editor-command", id);
            }
        })
        .invoke_handler(tauri::generate_handler![
            open_media,
            analyze_keyframes,
            choose_output,
            start_export,
            cancel_export,
            detect_chapters,
            cancel_detection,
            close_media,
            quit_app
        ])
        .on_window_event(|window, event| {
            if matches!(event, tauri::WindowEvent::Destroyed) {
                if let Ok(slot) = window.state::<AppState>().detection.lock() {
                    if let Some(control) = slot.as_ref() {
                        control.cancel();
                    }
                }
                if let Ok(slot) = window.state::<AppState>().scan.lock() {
                    if let Some(control) = slot.as_ref() {
                        control.cancel();
                    }
                }
                if let Ok(slot) = window.state::<AppState>().export.lock() {
                    if let Some(control) = slot.as_ref() {
                        control.cancel();
                    }
                }
            }
        })
        .build(tauri::generate_context!())
        .expect("Could not start the desktop app")
        .run(|app, event| {
            if matches!(event, tauri::RunEvent::Exit) {
                let state = app.state::<AppState>();
                if let Ok(slot) = state.detection.lock() {
                    if let Some(control) = slot.as_ref() {
                        control.cancel();
                    }
                }
                if let Ok(server) = state.preview.lock() {
                    if let Some(server) = server.as_ref() {
                        server.shutdown();
                    }
                }
                if let Ok(slot) = state.scan.lock() {
                    if let Some(control) = slot.as_ref() {
                        control.cancel();
                    }
                }
                if let Ok(slot) = state.export.lock() {
                    if let Some(control) = slot.as_ref() {
                        control.cancel();
                    }
                };
            }
        });
}
