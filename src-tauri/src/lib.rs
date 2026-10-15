mod state;

use pixie_core::{Folder, Photo, PixieError};
use pixie_image::thumbs::ThumbClass;
use pixie_index::{scan_folder, Db};
use serde::Serialize;
use state::AppState;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;
use tauri::http::{Response, StatusCode};
use tauri::{Emitter, Manager};

#[tauri::command]
fn add_folder(state: tauri::State<AppState>, path: String) -> Result<i64, PixieError> {
    state.db.add_folder(&path)
}

#[tauri::command]
fn list_folders(state: tauri::State<AppState>) -> Result<Vec<Folder>, PixieError> {
    state.db.list_folders()
}

#[tauri::command]
fn list_photos(state: tauri::State<AppState>, offset: i64, limit: i64) -> Result<Vec<Photo>, PixieError> {
    state.db.list_photos(offset.max(0), limit.clamp(1, 1000))
}

#[tauri::command]
fn count_photos(state: tauri::State<AppState>) -> Result<i64, PixieError> {
    state.db.count_photos()
}

#[derive(Serialize, Clone)]
struct ScanDone {
    folders: usize,
}

#[derive(Serialize, Clone)]
struct ScanErrorEvent {
    folder_id: i64,
    message: String,
}

#[tauri::command]
fn start_scan(app: tauri::AppHandle, state: tauri::State<AppState>, folder_id: Option<i64>) -> Result<(), PixieError> {
    // Only one scan job at a time; a second request is refused, not queued.
    let guard = state
        .gate
        .try_start()
        .ok_or_else(|| PixieError::Busy("a scan is already running".into()))?;
    let db = state.db.clone();
    let cancel = state.cancel_scan.clone();
    cancel.store(false, Ordering::Relaxed);
    let ids: Vec<i64> = match folder_id {
        Some(id) => vec![id],
        None => db.list_folders()?.into_iter().map(|f| f.id).collect(),
    };
    std::thread::spawn(move || {
        let _guard = guard; // released when the job ends
        let mut scanned = 0;
        for id in ids {
            if cancel.load(Ordering::Relaxed) {
                break;
            }
            let progress_app = app.clone();
            match scan_folder(&db, id, &cancel, |p| {
                let _ = progress_app.emit("scan-progress", p);
            }) {
                Ok(_) => scanned += 1,
                Err(e) => {
                    let _ = app.emit("scan-error", ScanErrorEvent { folder_id: id, message: e.user_message() });
                }
            }
        }
        let _ = app.emit("scan-done", ScanDone { folders: scanned });
    });
    Ok(())
}

#[tauri::command]
fn cancel_scan(state: tauri::State<AppState>) {
    state.cancel_scan.store(true, Ordering::Relaxed);
}

#[tauri::command]
fn print_page(window: tauri::WebviewWindow) -> Result<(), String> {
    window.print().map_err(|e| e.to_string())
}

fn status(code: StatusCode) -> Response<Vec<u8>> {
    Response::builder().status(code).body(Vec::new()).unwrap()
}

/// `/{grid|view}/{photo_id}` -> JPEG thumbnail bytes.
fn handle_thumb(state: &AppState, path: &str) -> Response<Vec<u8>> {
    let mut parts = path.trim_matches('/').split('/');
    let class = match parts.next() {
        Some("grid") => ThumbClass::Grid,
        Some("view") => ThumbClass::Viewer,
        _ => return status(StatusCode::NOT_FOUND),
    };
    let Some(id) = parts.next().and_then(|s| s.parse::<i64>().ok()) else {
        return status(StatusCode::NOT_FOUND);
    };
    let photo = match state.db.get_photo(id) {
        Ok(Some(p)) => p,
        Ok(None) => return status(StatusCode::NOT_FOUND),
        Err(_) => return status(StatusCode::INTERNAL_SERVER_ERROR),
    };
    let rx = state.thumbs.request(class, photo.id, photo.mtime, photo.size, PathBuf::from(&photo.path));
    match rx.recv_timeout(Duration::from_secs(120)) {
        Ok(Ok(file)) => match std::fs::read(file) {
            Ok(bytes) => Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", "image/jpeg")
                .header("Cache-Control", "public, max-age=31536000, immutable")
                .header("Access-Control-Allow-Origin", "*")
                .body(bytes)
                .unwrap(),
            Err(_) => status(StatusCode::INTERNAL_SERVER_ERROR),
        },
        _ => status(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .register_asynchronous_uri_scheme_protocol("pixie", |ctx, request, responder| {
            let state = ctx.app_handle().state::<AppState>().inner().clone();
            let path = request.uri().path().to_string();
            // Generation can take a while; never block the protocol thread.
            std::thread::spawn(move || responder.respond(handle_thumb(&state, &path)));
        })
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            let db = Db::open(&dir.join("pixie.db"))?;
            let workers = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).clamp(2, 8);
            let thumbs = pixie_image::service::ThumbService::new(dir.join("thumbs"), workers);
            app.manage(AppState { db, thumbs, cancel_scan: Arc::new(Default::default()), gate: Default::default() });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            add_folder, list_folders, list_photos, count_photos, start_scan, cancel_scan, print_page
        ])
        .run(tauri::generate_context!())
        .expect("error while running Pixie");
}
