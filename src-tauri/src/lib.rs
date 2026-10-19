mod state;

use pixie_core::{
    Bounds, Contact, DayGroup, Folder, FolderInfo, GeoPoint, MailAccount, MailSecurity, MediaKind, NewContact, Photo, PhotoQuery,
    PixieError, SortOrder,
};
use pixie_geo::{GeoError, HttpFetcher, Place, PlaceCache, PlaceService, TileService};
use pixie_image::posters::{poster_path, save_poster as write_poster, validate_poster};
use pixie_image::range::{plan_range, read_range, served_end, RangePlan};
use pixie_image::service::ThumbService;
use pixie_image::thumbs::ThumbClass;
use pixie_index::{scan_folder_with, Db, ScanMode};
use pixie_mail::{resolve_recipients, send_photos, send_test, OsSecrets, RecipientRef, SendRequest, SizeChoice, SmtpMailer};
use serde::{Deserialize, Serialize};
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
fn list_folder_info(state: tauri::State<AppState>) -> Result<Vec<FolderInfo>, PixieError> {
    state.db.folder_infos()
}

#[tauri::command]
fn remove_folder(state: tauri::State<AppState>, id: i64) -> Result<(), PixieError> {
    state.db.remove_folder(id)
}

/// The separate viewer window: created on first use, shown and focused afterwards. Async on purpose: on Windows,
/// building a window from a synchronous command (which runs on the main thread) deadlocks.
#[tauri::command]
async fn open_viewer_window(app: tauri::AppHandle) -> Result<(), PixieError> {
    if let Some(w) = app.get_webview_window("viewer") {
        let _ = w.show();
        let _ = w.set_focus();
        return Ok(());
    }
    tauri::WebviewWindowBuilder::new(&app, "viewer", tauri::WebviewUrl::App("index.html?viewer=1".into()))
        .title("Pixie viewer")
        .inner_size(900.0, 700.0)
        .build()
        .map_err(|e| PixieError::Io(e.to_string()))?;
    Ok(())
}

#[tauri::command]
fn list_folders(state: tauri::State<AppState>) -> Result<Vec<Folder>, PixieError> {
    state.db.list_folders()
}

fn query_of(
    order: Option<SortOrder>,
    folder_id: Option<i64>,
    kind: Option<MediaKind>,
    bounds: Option<Bounds>,
    max_id: Option<i64>,
    since: Option<i64>,
    until: Option<i64>,
) -> PhotoQuery {
    PhotoQuery { order: order.unwrap_or_default(), folder_id, kind, bounds, max_id, since, until }
}

/// The current highest photo id: the UI passes it back as `max_id` to freeze a listing while a scan adds photos.
#[tauri::command]
fn max_photo_id(state: tauri::State<AppState>) -> Result<i64, PixieError> {
    state.db.max_photo_id()
}

#[tauri::command]
fn list_photos(
    state: tauri::State<AppState>,
    offset: i64,
    limit: i64,
    order: Option<SortOrder>,
    folder_id: Option<i64>,
    kind: Option<MediaKind>,
    bounds: Option<Bounds>,
    max_id: Option<i64>,
    since: Option<i64>,
    until: Option<i64>,
) -> Result<Vec<Photo>, PixieError> {
    state.db.list_photos_in(&query_of(order, folder_id, kind, bounds, max_id, since, until), offset.max(0), limit.clamp(1, 1000))
}

#[tauri::command]
fn count_photos(
    state: tauri::State<AppState>,
    folder_id: Option<i64>,
    kind: Option<MediaKind>,
    bounds: Option<Bounds>,
    max_id: Option<i64>,
    since: Option<i64>,
    until: Option<i64>,
) -> Result<i64, PixieError> {
    state.db.count_photos_in(&query_of(None, folder_id, kind, bounds, max_id, since, until))
}

#[tauri::command]
fn list_day_groups(
    state: tauri::State<AppState>,
    order: Option<SortOrder>,
    folder_id: Option<i64>,
    kind: Option<MediaKind>,
    bounds: Option<Bounds>,
    max_id: Option<i64>,
    since: Option<i64>,
    until: Option<i64>,
) -> Result<Vec<DayGroup>, PixieError> {
    state.db.day_groups(&query_of(order, folder_id, kind, bounds, max_id, since, until))
}

/// One item by id (the photo behind a pin on the map).
#[tauri::command]
fn get_photo(state: tauri::State<AppState>, id: i64) -> Result<Option<Photo>, PixieError> {
    state.db.get_photo(id)
}

/// Id and position of every located item, for the map.
#[tauri::command]
fn list_geo_points(
    state: tauri::State<AppState>,
    folder_id: Option<i64>,
    kind: Option<MediaKind>,
    since: Option<i64>,
    until: Option<i64>,
) -> Result<Vec<GeoPoint>, PixieError> {
    state.db.list_geo_points(folder_id, kind, since, until)
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
/// `full` re-reads every file of the chosen folder(s); otherwise only new and changed files.
fn start_scan(app: tauri::AppHandle, state: tauri::State<AppState>, folder_id: Option<i64>, full: Option<bool>) -> Result<(), PixieError> {
    // Only one scan job at a time; a second request is refused, not queued.
    let guard = state
        .gate
        .try_start()
        .ok_or_else(|| PixieError::Busy("a scan is already running".into()))?;
    let db = state.db.clone();
    let thumbs = state.thumbs.clone();
    let cancel = state.cancel_scan.clone();
    let mode = if full.unwrap_or(false) { ScanMode::Full } else { ScanMode::Quick };
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
            match scan_folder_with(&db, id, mode, &cancel, |p| {
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

// ---- email ----

#[derive(Serialize)]
struct MailInfo {
    account: Option<MailAccount>,
    has_password: bool,
}

#[derive(Serialize)]
struct ProviderInfo {
    id: &'static str,
    label: &'static str,
    host: &'static str,
    port: u16,
    security: MailSecurity,
    limit_mb: u32,
    hint: &'static str,
}

#[tauri::command]
fn list_providers() -> Vec<ProviderInfo> {
    pixie_mail::providers()
        .iter()
        .map(|p| ProviderInfo {
            id: p.id, label: p.label, host: p.host, port: p.port, security: p.security, limit_mb: p.limit_mb, hint: p.hint,
        })
        .collect()
}

#[tauri::command]
fn get_mail_account(state: tauri::State<AppState>) -> Result<MailInfo, PixieError> {
    let account = state.db.get_mail_account()?;
    let has_password = match &account {
        Some(a) => matches!(state.secrets.get(&a.email), Ok(Some(p)) if !p.is_empty()),
        None => false,
    };
    Ok(MailInfo { account, has_password })
}

/// Saves the account settings. A non-empty `password` replaces the stored one; an empty or missing one
/// keeps it. The password goes to the credential store only and is never logged.
#[tauri::command]
fn save_mail_account(state: tauri::State<AppState>, account: MailAccount, password: Option<String>) -> Result<(), PixieError> {
    let old = state.db.get_mail_account()?;
    state.db.save_mail_account(&account)?;
    let saved = state.db.get_mail_account()?.ok_or_else(|| PixieError::NotFound("account".into()))?;
    if let Some(pw) = password.filter(|p| !p.is_empty()) {
        state.secrets.set(&saved.email, &pw)?;
    }
    if let Some(old) = old {
        if !old.email.eq_ignore_ascii_case(&saved.email) {
            state.secrets.delete(&old.email)?; // the old address's password must not linger
        }
    }
    Ok(())
}

#[tauri::command]
fn delete_mail_account(state: tauri::State<AppState>) -> Result<(), PixieError> {
    if let Some(a) = state.db.get_mail_account()? {
        state.secrets.delete(&a.email)?;
    }
    state.db.delete_mail_account()
}

#[tauri::command]
fn list_contacts(state: tauri::State<AppState>) -> Result<Vec<Contact>, PixieError> {
    state.db.list_contacts()
}

#[tauri::command]
fn save_contact(state: tauri::State<AppState>, contact: NewContact) -> Result<Contact, PixieError> {
    state.db.save_contact(&contact)
}

#[tauri::command]
fn delete_contact(state: tauri::State<AppState>, id: i64) -> Result<(), PixieError> {
    state.db.delete_contact(id)
}

/// The items at these positions of the current listing (the selection is by position).
#[tauri::command]
fn photos_at(
    state: tauri::State<AppState>,
    indexes: Vec<i64>,
    order: Option<SortOrder>,
    folder_id: Option<i64>,
    kind: Option<MediaKind>,
    bounds: Option<Bounds>,
    max_id: Option<i64>,
    since: Option<i64>,
    until: Option<i64>,
) -> Result<Vec<Photo>, PixieError> {
    state.db.photos_at(&query_of(order, folder_id, kind, bounds, max_id, since, until), &indexes)
}

#[tauri::command]
async fn send_test_email(state: tauri::State<'_, AppState>) -> Result<(), String> {
    let (db, secrets, mailer) = (state.db.clone(), state.secrets.clone(), state.mailer.clone());
    tauri::async_runtime::spawn_blocking(move || {
        let account = db
            .get_mail_account()
            .map_err(|e| e.user_message())?
            .ok_or_else(|| "Set up your email account first.".to_string())?;
        send_test(&*mailer, &*secrets, &account).map_err(|e| e.0)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[derive(Deserialize)]
struct SendEmailRequest {
    indexes: Vec<i64>,
    order: Option<SortOrder>,
    folder_id: Option<i64>,
    kind: Option<MediaKind>,
    bounds: Option<Bounds>,
    max_id: Option<i64>,
    since: Option<i64>,
    until: Option<i64>,
    to: Vec<RecipientRef>,
    subject: String,
    message: String,
    size: SizeChoice,
}

#[derive(Serialize, Clone)]
struct MailProgressEvent {
    done: usize,
    total: usize,
}

/// Sends the selected items. Runs on the blocking pool and emits `mail-progress` while preparing.
#[tauri::command]
async fn send_email(app: tauri::AppHandle, state: tauri::State<'_, AppState>, request: SendEmailRequest) -> Result<Vec<String>, String> {
    let (db, secrets, mailer) = (state.db.clone(), state.secrets.clone(), state.mailer.clone());
    tauri::async_runtime::spawn_blocking(move || {
        let account = db
            .get_mail_account()
            .map_err(|e| e.user_message())?
            .ok_or_else(|| "Set up your email account first.".to_string())?;
        let mut indexes = request.indexes.clone();
        indexes.sort_unstable();
        indexes.dedup();
        let query = query_of(request.order, request.folder_id, request.kind, request.bounds, request.max_id, request.since, request.until);
        let photos = db.photos_at(&query, &indexes).map_err(|e| e.user_message())?;
        if photos.len() != indexes.len() {
            // Selection is by position, and the library changed since it was made.
            return Err("Your selection changed. Please select the photos again.".to_string());
        }
        let contacts = db.list_contacts().map_err(|e| e.user_message())?;
        let to = resolve_recipients(&request.to, &contacts).map_err(|e| e.user_message())?;
        let req = SendRequest { photos, to, subject: request.subject, message: request.message, size: request.size };
        send_photos(&*mailer, &*secrets, &account, req, &|p| {
            let _ = app.emit("mail-progress", MailProgressEvent { done: p.done, total: p.total });
        })
        .map_err(|e| e.0)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Stores a poster frame the webview made for a video. Looked up by id; the bytes are
/// validated (JPEG, small) because they come from the page.
#[tauri::command]
fn save_poster(state: tauri::State<AppState>, id: i64, bytes: Vec<u8>) -> Result<(), String> {
    let photo = match state.db.get_photo(id) {
        Ok(Some(p)) if p.meta.kind == MediaKind::Video => p,
        _ => return Err("not a video".into()),
    };
    validate_poster(&bytes).map_err(|e| e.to_string())?;
    write_poster(&poster_path(&state.posters_dir, photo.id, photo.mtime, photo.size), &bytes).map_err(|e| e.to_string())
}

/// Opens a video in the default player of the system. The path comes from the database by id.
#[tauri::command]
fn open_in_default_player(state: tauri::State<AppState>, id: i64) -> Result<(), String> {
    let photo = match state.db.get_photo(id) {
        Ok(Some(p)) if p.meta.kind == MediaKind::Video => p,
        _ => return Err("not a video".into()),
    };
    open::that_detached(&photo.path).map_err(|e| e.to_string())
}

/// Blocks on the rate limit and the network, so it runs on the blocking pool, not an
/// async worker. A lookup overtaken by a newer one returns `Err` quickly (the UI drops it).
#[tauri::command]
async fn place_for(
    state: tauri::State<'_, AppState>,
    lat: f64,
    lon: f64,
    lang: String,
) -> Result<Option<Place>, String> {
    let places = state.places.clone();
    tauri::async_runtime::spawn_blocking(move || places.place_for(lat, lon, &lang))
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

fn video_response(bytes: Vec<u8>, start: u64, end: u64, size: u64) -> Response<Vec<u8>> {
    Response::builder()
        .status(StatusCode::PARTIAL_CONTENT)
        .header("Content-Type", "video/mp4")
        .header("Accept-Ranges", "bytes")
        .header("Content-Range", format!("bytes {start}-{end}/{size}"))
        .header("Access-Control-Allow-Origin", "*")
        .body(bytes)
        .unwrap()
}

/// `/video/{id}`: a range of the file, looked up by id only. Runs on its own thread so
/// a slow disk or OneDrive placeholder never blocks the protocol handler.
fn handle_video(state: &AppState, path: &str, range: Option<String>, responder: tauri::UriSchemeResponder) {
    let Some(id) = path.trim_matches('/').split('/').nth(1).and_then(|s| s.parse::<i64>().ok()) else {
        return responder.respond(status(StatusCode::NOT_FOUND));
    };
    let photo = match state.db.get_photo(id) {
        Ok(Some(p)) if p.meta.kind == MediaKind::Video => p,
        Ok(_) => return responder.respond(status(StatusCode::NOT_FOUND)),
        Err(_) => return responder.respond(status(StatusCode::INTERNAL_SERVER_ERROR)),
    };
    std::thread::spawn(move || {
        let size = match std::fs::metadata(&photo.path) {
            Ok(m) => m.len(),
            Err(_) => return responder.respond(status(StatusCode::NOT_FOUND)),
        };
        match plan_range(range.as_deref(), size) {
            RangePlan::Unsatisfiable => responder.respond(
                Response::builder()
                    .status(StatusCode::RANGE_NOT_SATISFIABLE)
                    .header("Content-Range", format!("bytes */{size}"))
                    .body(Vec::new())
                    .unwrap(),
            ),
            RangePlan::Partial { start, end } => match read_range(std::path::Path::new(&photo.path), start, end) {
                Ok(bytes) => match served_end(start, bytes.len()) {
                    // Content-Range must describe the bytes actually sent (the file may have shrunk).
                    Some(end) => responder.respond(video_response(bytes, start, end, size)),
                    None => responder.respond(status(StatusCode::NOT_FOUND)),
                },
                Err(e) => {
                    eprintln!("[video] {}: {e}", photo.path);
                    responder.respond(status(StatusCode::INTERNAL_SERVER_ERROR));
                }
            },
        }
    });
}

/// `/poster/{id}`: the stored poster frame, or 404 until the webview has made one.
fn handle_poster(state: &AppState, path: &str, responder: tauri::UriSchemeResponder) {
    let Some(id) = path.trim_matches('/').split('/').nth(1).and_then(|s| s.parse::<i64>().ok()) else {
        return responder.respond(status(StatusCode::NOT_FOUND));
    };
    let Ok(Some(p)) = state.db.get_photo(id) else { return responder.respond(status(StatusCode::NOT_FOUND)) };
    match std::fs::read(poster_path(&state.posters_dir, p.id, p.mtime, p.size)) {
        Ok(bytes) => responder.respond(jpeg_response(bytes)),
        Err(_) => responder.respond(status(StatusCode::NOT_FOUND)),
    }
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
    if photo.meta.kind == MediaKind::Video {
        return responder.respond(status(StatusCode::NOT_FOUND)); // videos use /poster, not image thumbnails
    }
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
                .filter(|p| p.meta.kind == MediaKind::Photo)
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
            match path.trim_matches('/').split('/').next() {
                Some("tile") => handle_tile(&state, path, responder),
                Some("video") => {
                    let range = request.headers().get("range").and_then(|v| v.to_str().ok()).map(str::to_owned);
                    handle_video(&state, path, range, responder);
                }
                Some("poster") => handle_poster(&state, path, responder),
                _ => handle_thumb(&state, path, responder),
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
                posters_dir: dir.join("posters"),
                secrets: Arc::new(OsSecrets),
                mailer: Arc::new(SmtpMailer),
                cancel_scan: Arc::new(Default::default()),
                gate: Default::default(),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            add_folder, list_folders, list_folder_info, remove_folder, open_viewer_window, list_photos, count_photos, list_day_groups, list_geo_points, max_photo_id, get_photo, start_scan, cancel_scan, print_page, place_for, save_poster,
            open_in_default_player, list_providers, get_mail_account, save_mail_account, delete_mail_account, list_contacts,
            save_contact, delete_contact, photos_at, send_test_email, send_email
        ])
        .run(tauri::generate_context!())
        .expect("error while running Pixie");
}
