mod state;

use pixie_core::{DayGroup, Folder, Photo, PhotoQuery, PixieError, SortOrder};
use pixie_geo::{GeoError, HttpFetcher, Place, PlaceCache, PlaceService, TileService};
use pixie_image::service::ThumbService;
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

fn query_of(order: Option<SortOrder>, folder_id: Option<i64>) -> PhotoQuery {
    PhotoQuery { order: order.unwrap_or_default(), folder_id }
}

#[tauri::command]
fn list_photos(
    state: tauri::State<AppState>,
    offset: i64,
    limit: i64,
    order: Option<SortOrder>,
    folder_id: Option<i64>,
) -> Result<Vec<Photo>, PixieError> {
    state.db.list_photos_in(&query_of(order, folder_id), offset.max(0), limit.clamp(1, 1000))
}

#[tauri::command]
fn count_photos(state: tauri::State<AppState>, folder_id: Option<i64>) -> Result<i64, PixieError> {
    state.db.count_photos_in(&query_of(None, folder_id))
}

#[tauri::command]
fn list_day_groups(
    state: tauri::State<AppState>,
    order: Option<SortOrder>,
    folder_id: Option<i64>,
) -> Result<Vec<DayGroup>, PixieError> {
    state.db.day_groups(&query_of(order, folder_id))
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
    let thumbs = state.thumbs.clone();
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
        prefetch_all(&db, &thumbs); // build thumbnails for anything new, in the background
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

/// Blocks on the rate limit and the network, so it runs on the blocking pool, not an
/// async worker. A lookup overtaken by a newer one returns `Err` quickly (the UI drops it).
#[tauri::command]
async fn place_for(state: tauri::State<'_, AppState>, lat: f64, lon: f64) -> Result<Option<Place>, String> {
    let places = state.places.clone();
    tauri::async_runtime::spawn_blocking(move || places.place_for(lat, lon))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())
}

fn status(code: StatusCode) -> Response<Vec<u8>> {
    Response::builder().status(code).body(Vec::new()).unwrap()
}

fn jpeg_response(bytes: Vec<u8>) -> Response<Vec<u8>> {
    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "image/jpeg")
        .header("Cache-Control", "public, max-age=31536000, immutable")
        .header("Access-Control-Allow-Origin", "*")
        .body(bytes)
        .unwrap()
}

fn png_response(bytes: Vec<u8>) -> Response<Vec<u8>> {
    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "image/png")
        .header("Cache-Control", "public, max-age=86400")
        .header("Access-Control-Allow-Origin", "*")
        .body(bytes)
        .unwrap()
}

/// `/tile/{z}/{x}/{y}`: an OpenStreetMap tile, fetched and cached by the backend.
fn handle_tile(state: &AppState, path: &str, responder: tauri::UriSchemeResponder) {
    let mut parts = path.trim_matches('/').split('/').skip(1); // skip "tile"
    let z = parts.next().and_then(|s| s.parse::<u8>().ok());
    let x = parts.next().and_then(|s| s.parse::<u32>().ok());
    let y = parts.next().and_then(|s| s.parse::<u32>().ok());
    let (Some(z), Some(x), Some(y)) = (z, x, y) else {
        return responder.respond(status(StatusCode::NOT_FOUND));
    };
    state.tiles.request(
        z,
        x,
        y,
        Box::new(move |result| {
            responder.respond(match result {
                Ok(bytes) => png_response(bytes),
                Err(GeoError::BadTile(..)) => status(StatusCode::NOT_FOUND),
                Err(e) => {
                    eprintln!("[tile] {z}/{x}/{y}: {e}");
                    status(StatusCode::BAD_GATEWAY)
                }
            });
        }),
    );
}

/// `/{grid|view}/{photo_id}`: answers through `responder` once the thumbnail is
/// ready. No thread is parked per request; the worker that finishes it replies.
fn handle_thumb(state: &AppState, path: &str, responder: tauri::UriSchemeResponder) {
    let mut parts = path.trim_matches('/').split('/');
    let class = match parts.next() {
        Some("grid") => ThumbClass::Grid,
        Some("view") => ThumbClass::Viewer,
        _ => return responder.respond(status(StatusCode::NOT_FOUND)),
    };
    let Some(id) = parts.next().and_then(|s| s.parse::<i64>().ok()) else {
        return responder.respond(status(StatusCode::NOT_FOUND));
    };
    let photo = match state.db.get_photo(id) {
        Ok(Some(p)) => p,
        Ok(None) => return responder.respond(status(StatusCode::NOT_FOUND)),
        Err(_) => return responder.respond(status(StatusCode::INTERNAL_SERVER_ERROR)),
    };
    let shown = photo.path.clone();
    state.thumbs.request_with(
        class,
        photo.id,
        photo.mtime,
        photo.size,
        PathBuf::from(&photo.path),
        Box::new(move |result| {
            let resp = match result {
                Ok(file) => match std::fs::read(&file) {
                    Ok(bytes) => jpeg_response(bytes),
                    Err(e) => {
                        eprintln!("[thumb] cannot read {}: {e}", file.display());
                        status(StatusCode::INTERNAL_SERVER_ERROR)
                    }
                },
                Err(e) => {
                    eprintln!("[thumb] FAILED {shown}: {e}");
                    status(StatusCode::INTERNAL_SERVER_ERROR)
                }
            };
            responder.respond(resp);
        }),
    );
}

/// Queue grid thumbnails for the whole library (newest first, the order the grid
/// shows them) at the lowest priority, so scrolling finds them already built.
fn prefetch_all(db: &Db, thumbs: &ThumbService) {
    let mut offset = 0;
    loop {
        let Ok(page) = db.list_photos(offset, 1000) else { return };
        if page.is_empty() {
            return;
        }
        offset += page.len() as i64;
        thumbs.prefetch(
            page.into_iter()
                .map(|p| (ThumbClass::Grid, p.id, p.mtime, p.size, PathBuf::from(p.path)))
                .collect(),
        );
    }
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .register_asynchronous_uri_scheme_protocol("pixie", |ctx, request, responder| {
            let state = ctx.app_handle().state::<AppState>().inner().clone();
            let path = request.uri().path();
            if path.trim_matches('/').starts_with("tile/") {
                handle_tile(&state, path, responder);
            } else {
                handle_thumb(&state, path, responder);
            }
        })
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            let db = Db::open(&dir.join("pixie.db"))?;
            // Leave one core for the UI; decoding is the heavy part.
            let workers = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).saturating_sub(1).clamp(2, 6);
            let thumbs = ThumbService::new(dir.join("thumbs"), workers);
            let (pdb, pthumbs) = (db.clone(), thumbs.clone());
            std::thread::spawn(move || prefetch_all(&pdb, &pthumbs));
            let fetcher: Arc<dyn pixie_geo::Fetcher> = Arc::new(HttpFetcher::new());
            let places = Arc::new(PlaceService::new(
                PlaceCache::open(&dir.join("places.db"))?,
                fetcher.clone(),
                Duration::from_millis(1100),
            ));
            let tiles = TileService::new(dir.join("tiles"), fetcher, 2);
            app.manage(AppState {
                db,
                thumbs,
                tiles,
                places,
                cancel_scan: Arc::new(Default::default()),
                gate: Default::default(),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            add_folder, list_folders, list_photos, count_photos, list_day_groups, start_scan, cancel_scan, print_page, place_for
        ])
        .run(tauri::generate_context!())
        .expect("error while running Pixie");
}
