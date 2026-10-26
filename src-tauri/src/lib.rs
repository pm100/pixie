mod autoscan;
mod drives;
mod outlook;
mod placement;
mod sniff;
mod state;
mod subject;

use pixie_core::{
    Bounds, Contact, DayGroup, Folder, FolderInfo, GeoPoint, MailAccount, MailSecurity, MediaKind, NewContact, Photo, PhotoQuery,
    PixieError, SortOrder,
};
use pixie_geo::{GeoError, HttpFetcher, Place, PlaceCache, PlaceService, TileService};
use pixie_image::posters::{poster_path, save_poster as write_poster, validate_poster};
use pixie_image::range::{plan_range, read_range, served_end, RangePlan};
use pixie_image::service::ThumbService;
use pixie_image::thumbs::ThumbClass;
use pixie_index::suggest::{known_places, Places, Suggestion};
use pixie_index::{finish_walk, walk_folder_streaming, Db, ScanGuard, ScanMode, ScanQueue};
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

/// An approximate item count for one suggested folder (`suggestion-count` event).
#[derive(Serialize, Clone)]
struct SuggestionCount {
    path: String,
    n: usize,
    capped: bool,
}

/// First-run folder suggestions. Returns the well-known places that exist at once, then in the background emits
/// `suggestion-count` for each (a time-boxed count, capped at 10,000), then `suggestion-found` for each further
/// candidate (unticked) as it is found, then `suggestions-done`; `suggestion-looking` names what is being sniffed now.
/// Subscribe to the events before calling. The look-around covers the home folder, then every other built-in drive and
/// mapped network drive: each is checked for pictures on a thread of its own, at the same time, with a deadline, so
/// an unreachable network share costs only its own row ("not reachable right now") and never freezes anything.
/// With `deep` false (adding a folder later) only the usual places are counted: the wider search is left to
/// `suggest_more`, when asked for.
#[tauri::command]
async fn suggest_folders(app: tauri::AppHandle, deep: Option<bool>) -> Result<Vec<Suggestion>, PixieError> {
    let (places, list) = known_places_of(&app);
    let deep = deep.unwrap_or(true);
    let (events, known, home) = (app.clone(), list.clone(), places.home.clone());
    std::thread::spawn(move || {
        let (home, others) = if deep { (home, other_drives()) } else { (None, Vec::new()) };
        sniff_and_emit(&events, true, known, home, others);
    });
    Ok(list)
}

/// The wider search, on request: the home folder and every other drive, for folders with pictures in them that are not
/// among the usual places. Emits the same events as `suggest_folders` (without counting the usual places again).
#[tauri::command]
async fn suggest_more(app: tauri::AppHandle) -> Result<(), PixieError> {
    let (places, list) = known_places_of(&app);
    std::thread::spawn(move || sniff_and_emit(&app, false, list, places.home.clone(), other_drives()));
    Ok(())
}

/// The well-known places on this computer, and those that exist as suggestions.
fn known_places_of(app: &tauri::AppHandle) -> (Places, Vec<Suggestion>) {
    let path = app.path();
    let mut onedrive: Vec<PathBuf> = Vec::new();
    for key in ["OneDrive", "OneDriveConsumer", "OneDriveCommercial"] {
        if let Some(p) = std::env::var_os(key).map(PathBuf::from) {
            if !onedrive.contains(&p) {
                onedrive.push(p);
            }
        }
    }
    let places = Places {
        pictures: path.picture_dir().ok(),
        videos: path.video_dir().ok(),
        documents: path.document_dir().ok(),
        downloads: path.download_dir().ok(),
        desktop: path.desktop_dir().ok(),
        home: path.home_dir().ok(),
        onedrive,
    };
    let list = known_places(&places);
    (places, list)
}

/// Every built-in and network drive except the system drive (the home folder covers that one), each sniffed as a whole.
fn other_drives() -> Vec<drives::DriveRoot> {
    let system = std::env::var("SystemDrive").unwrap_or_default().to_uppercase();
    drives::drive_roots()
        .into_iter()
        .filter(|d| system.is_empty() || !d.root.to_string_lossy().to_uppercase().starts_with(&system))
        .collect()
}

/// Run the look-around and tell the window as it goes (see `suggest_folders` for the events).
fn sniff_and_emit(events: &tauri::AppHandle, count_known: bool, known: Vec<Suggestion>, home: Option<PathBuf>, others: Vec<drives::DriveRoot>) {
    sniff::run_sniff_with(count_known, known, home, others, drives::volume_label, &sniff::Budgets::app(), &mut |event| {
        let _ = match event {
            sniff::SniffEvent::Looking(what) => events.emit("suggestion-looking", what),
            sniff::SniffEvent::Count { path, n, capped } => events.emit("suggestion-count", SuggestionCount { path, n, capped }),
            sniff::SniffEvent::Found(s) => events.emit("suggestion-found", s),
            sniff::SniffEvent::Done => events.emit("suggestions-done", ()),
        };
    });
}

#[tauri::command]
fn list_folder_info(state: tauri::State<AppState>) -> Result<Vec<FolderInfo>, PixieError> {
    state.db.folder_infos()
}

#[tauri::command]
fn remove_folder(state: tauri::State<AppState>, id: i64) -> Result<(), PixieError> {
    state.pipeline.forget_folder(id);
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
    search: Option<bool>,
) -> PhotoQuery {
    PhotoQuery { order: order.unwrap_or_default(), folder_id, kind, bounds, max_id, since, until, search: search.unwrap_or(false) }
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
    search: Option<bool>,
) -> Result<Vec<Photo>, PixieError> {
    state.db.list_photos_in(&query_of(order, folder_id, kind, bounds, max_id, since, until, search), offset.max(0), limit.clamp(1, 1000))
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
    search: Option<bool>,
) -> Result<i64, PixieError> {
    state.db.count_photos_in(&query_of(None, folder_id, kind, bounds, max_id, since, until, search))
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
    search: Option<bool>,
) -> Result<Vec<DayGroup>, PixieError> {
    state.db.day_groups(&query_of(order, folder_id, kind, bounds, max_id, since, until, search))
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
    search: Option<bool>,
) -> Result<Vec<GeoPoint>, PixieError> {
    state.db.list_geo_points_in(&PhotoQuery { folder_id, kind, since, until, search: search.unwrap_or(false), ..Default::default() })
}

#[derive(Serialize, Clone)]
struct ScanDone {
    folders: usize,
    /// Media files the walks found in all (new or already known).
    found: usize,
}

#[derive(Serialize, Clone)]
struct ScanErrorEvent {
    folder_id: i64,
    message: String,
}

/// Tells the scan queue its job is over when the job thread ends however it ends.
struct JobEnd(ScanQueue);

impl Drop for JobEnd {
    fn drop(&mut self) {
        self.0.finish();
    }
}

/// The drive or share a folder path lives on, if it is on one that can be unplugged or disconnected: `P:\`, a `\\server\share`,
/// or `/Volumes/name` (the Mac), with the name to show for it.
fn removable_root(path: &str) -> Option<(String, String)> {
    let b = path.as_bytes();
    if b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':' {
        let drive = format!("{}:", (b[0] as char).to_ascii_uppercase());
        return Some((format!("{drive}\\"), format!("drive {drive}")));
    }
    if let Some(rest) = path.strip_prefix("\\\\") {
        let mut parts = rest.split('\\').filter(|p| !p.is_empty());
        let (server, share) = (parts.next()?, parts.next()?);
        return Some((format!("\\\\{server}\\{share}"), format!("\\\\{server}\\{share}")));
    }
    let name = path.strip_prefix("/Volumes/")?.split('/').next().filter(|n| !n.is_empty())?;
    Some((format!("/Volumes/{name}"), format!("\"{name}\"")))
}

/// What to tell the person when a folder could not be scanned. A folder that cannot be reached is named, with the reason
/// as far as it can be told: its drive is not connected, or the folder itself is gone. `exists` says whether a path exists.
fn scan_error_message_with(e: &PixieError, folder: Option<&str>, exists: impl Fn(&str) -> bool) -> String {
    let (PixieError::NotFound(_), Some(path)) = (e, folder) else { return e.user_message() };
    match removable_root(path) {
        Some((root, shown)) if !exists(&root) => format!("Pixie can't reach {path}: {shown} is not connected."),
        _ => format!("Pixie can't find {path}. It may have been moved, renamed or deleted."),
    }
}

fn scan_error_message(e: &PixieError, folder: Option<&str>) -> String {
    scan_error_message_with(e, folder, |p| std::path::Path::new(p).exists())
}

/// Run one scan job on its own thread: `ids` first, then any folders added to the queue while it runs. The caller
/// holds the gate; it is released when the job ends.
fn spawn_scan_job(app: tauri::AppHandle, state: &AppState, guard: ScanGuard, ids: Vec<i64>, mode: ScanMode) {
    let db = state.db.clone();
    let thumbs = state.thumbs.clone();
    let cancel = state.cancel_scan.clone();
    let analysis = state.analysis.clone();
    let people = state.people.clone();
    let pipeline = state.pipeline.clone();
    let queue = state.queue.clone();
    cancel.store(false, Ordering::Relaxed);
    queue.start_job();
    std::thread::spawn(move || {
        let _guard = guard; // released when the job ends
        let _end = JobEnd(queue.clone()); // dropped first: a job that dies stops accepting folders
        let mut todo = ids;
        let mut scanned = 0;
        let mut found = 0;
        loop {
            for id in todo {
                if cancel.load(Ordering::Relaxed) {
                    break;
                }
                let progress_app = app.clone();
                // The walk only lists what is new or changed; the analysis pipeline then reads each file once and does
                // everything with it, starting with its row in the library.
                // (the files are handed to the pipeline as the walk finds them, so analysis begins at once)
                let mut hand = |batch: Vec<pixie_index::Candidate>| pipeline.enqueue(batch);
                match walk_folder_streaming(&db, id, mode, &cancel, |p| {
                    let _ = progress_app.emit("scan-progress", p);
                }, Some(&mut hand)) {
                    Ok(mut walk) => {
                        if walk.stats.cancelled {
                            break;
                        }
                        if let Err(e) = finish_walk(&db, &mut walk) {
                            let _ = app.emit("scan-error", ScanErrorEvent { folder_id: id, message: e.user_message() });
                        }
                        found += walk.seen.len();
                        scanned += 1;
                    }
                    Err(e) => {
                        let path = db.get_folder(id).ok().flatten().map(|f| f.path);
                        let _ = app.emit("scan-error", ScanErrorEvent { folder_id: id, message: scan_error_message(&e, path.as_deref()) });
                    }
                }
            }
            if cancel.load(Ordering::Relaxed) {
                break;
            }
            todo = queue.next_batch(); // folders added while this job ran; empty means the job is over
            if todo.is_empty() {
                break;
            }
        }
        if cancel.load(Ordering::Relaxed) {
            pipeline.clear_intake(); // stopped: the files already handed over but not yet read are let go too
        }
        let _ = app.emit("scan-done", ScanDone { folders: scanned, found });
        analysis.kick(); // new photos to analyse; the worker waits while this scan job runs
        people.kick();
        prefetch_newest(&db, &thumbs); // queue thumbnails for the newest photos, in the background
    });
}

#[tauri::command]
/// `full` re-reads every file of the chosen folder(s); otherwise only new and changed files.
fn start_scan(app: tauri::AppHandle, state: tauri::State<AppState>, folder_id: Option<i64>, full: Option<bool>) -> Result<(), PixieError> {
    // Only one scan job at a time; a second request is refused, not queued.
    let guard = state
        .gate
        .try_start()
        .ok_or_else(|| PixieError::Busy("a scan is already running".into()))?;
    let mode = if full.unwrap_or(false) { ScanMode::Full } else { ScanMode::Quick };
    let ids: Vec<i64> = match folder_id {
        Some(id) => vec![id],
        None => state.db.list_folders()?.into_iter().map(|f| f.id).collect(),
    };
    spawn_scan_job(app, state.inner(), guard, ids, mode);
    Ok(())
}

/// How far the analysis has got for the photos in these folders: the load screen's counters. Counted from those
/// photos alone, so what else is in the library, and duplicates hiding older photos, cannot throw it off. Runs off
/// the main thread.
#[tauri::command]
async fn load_progress(state: tauri::State<'_, AppState>, folder_ids: Vec<i64>) -> Result<pixie_core::LoadProgress, String> {
    let db = state.db.clone();
    let pipeline = state.pipeline.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let mut p = db.load_progress(&folder_ids, pixie_analysis::clip::MODEL_VERSION, pixie_analysis::faces::FACES_VERSION).map_err(|e| e.to_string())?;
        (p.pending, p.pending_videos) = pipeline.pending_in(&folder_ids); // found by the walk, not stored yet
        Ok(p)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// The counts beside the entries of the left-hand tree under the filters in force: each section is counted with every
/// filter except its own. `tag` and `person` are the ones chosen in the tree; `trip_dates` says the date window is a
/// chosen trip; `trip_windows` are the `[since, until)` windows of the trips listed; `recent_since` is the start of the
/// Last 30 days row. Runs off the main thread.
#[tauri::command]
async fn facet_counts(
    state: tauri::State<'_, AppState>,
    folder_id: Option<i64>,
    kind: Option<MediaKind>,
    bounds: Option<Bounds>,
    max_id: Option<i64>,
    since: Option<i64>,
    until: Option<i64>,
    tag: Option<String>,
    person: Option<i64>,
    trip_dates: bool,
    trip_windows: Vec<(i64, i64)>,
    recent_since: Option<i64>,
) -> Result<pixie_core::FacetCounts, String> {
    let db = state.db.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let groups: Vec<Vec<String>> = pixie_analysis::tags::CATEGORIES.iter().map(|(_, labels)| labels.iter().map(|l| l.to_string()).collect()).collect();
        let ask = pixie_index::FacetAsk { tag: tag.as_deref(), person, trip_dates, tag_groups: &groups, trip_windows: &trip_windows, recent_since };
        let mut counts = db.facet_counts(&query_of(None, folder_id, kind, bounds, max_id, since, until, None), &ask).map_err(|e| e.to_string())?;
        counts.tag_group_names = pixie_analysis::tags::CATEGORIES.iter().map(|(name, _)| name.to_string()).collect();
        Ok(counts)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// What `scan_folders` did: the folders' ids, and whether they joined a scan that was already running.
#[derive(Serialize)]
struct ScanStart {
    folder_ids: Vec<i64>,
    queued: bool,
}

/// Ask for a scan of these folders. When a scan job is already running (a launch rescan, say) they join it, so a folder
/// added at the wrong moment is never left unscanned; otherwise a new job starts over them all. Returns whether they were
/// queued behind a running job.
fn request_scan(app: &tauri::AppHandle, state: &AppState, ids: &[i64]) -> bool {
    let app_job = app.clone();
    let mut placed = placement::place_folders(ids, &state.gate, &state.queue, 200, Duration::from_millis(10), &mut |rest, guard| {
        spawn_scan_job(app_job.clone(), state, guard, rest, ScanMode::Quick);
    });
    if let placement::Outcome::Waiting(rest) = std::mem::replace(&mut placed.outcome, placement::Outcome::Queued) {
        // A job is finishing (closing steps after its last folder) and neither takes folders nor has freed the gate yet.
        // The folders are already saved: start their scan the moment the gate frees, however long that takes.
        let shared = state.clone();
        let app = app.clone();
        std::thread::spawn(move || loop {
            if let Some(guard) = shared.gate.try_start() {
                spawn_scan_job(app, &shared, guard, rest, ScanMode::Quick);
                return;
            }
            std::thread::sleep(Duration::from_millis(50));
        });
        placed.queued = true; // it will be scanned after the running job, which is what "queued" tells the caller
    }
    placed.queued
}

/// Add folders and scan them (see `request_scan`).
#[tauri::command]
fn scan_folders(app: tauri::AppHandle, state: tauri::State<AppState>, paths: Vec<String>) -> Result<ScanStart, PixieError> {
    let mut ids: Vec<i64> = Vec::new();
    for p in &paths {
        let id = state.db.add_folder(p)?;
        if !ids.contains(&id) {
            ids.push(id);
        }
    }
    let queued = request_scan(&app, state.inner(), &ids);
    Ok(ScanStart { folder_ids: ids, queued })
}

/// Scan the folders on the network every `minutes` (none: never). Folders on this computer need no setting: they are
/// watched, and scanned shortly after something in them changes.
#[tauri::command]
fn set_network_rescan(auto: tauri::State<autoscan::AutoScan>, minutes: Option<u32>) {
    auto.set_network_rescan(minutes.filter(|m| *m > 0).map(|m| Duration::from_secs(m as u64 * 60)));
}

#[tauri::command]
fn analysis_status(state: tauri::State<AppState>) -> pixie_analysis::service::AnalysisStatus {
    state.analysis.status()
}

#[tauri::command]
fn analysis_set_paused(state: tauri::State<AppState>, paused: bool) {
    state.analysis.set_paused(paused);
    state.people.set_paused(paused);
    state.dupes.set_paused(paused); // the one Pause covers the duplicate check too
}

#[tauri::command]
fn people_status(state: tauri::State<AppState>) -> pixie_analysis::people_service::PeopleStatus {
    state.people.status()
}

/// The people to list in the nav (see `Db::list_people`).
#[tauri::command]
fn list_people(state: tauri::State<AppState>) -> Result<Vec<pixie_core::PersonInfo>, PixieError> {
    state.db.list_people()
}

/// Groups of identical files (same contents), most wasted space first; the copy to keep is first in each.
#[tauri::command]
fn duplicate_groups(state: tauri::State<AppState>) -> Result<Vec<pixie_core::DupGroup>, PixieError> {
    state.db.duplicate_groups()
}

/// How far the background hashing has got and what the copies found add up to.
#[tauri::command]
fn duplicate_status(state: tauri::State<AppState>) -> Result<pixie_core::DupStatus, PixieError> {
    let mut s = state.db.dup_status()?;
    s.paused = state.dupes.is_paused();
    Ok(s)
}

/// Move extra copies to the Recycle Bin (never a permanent delete). Only copies that still have another live copy
/// outside the request are touched, and only if the file on disk is still the size that was hashed. Returns the ids
/// that were removed.
#[tauri::command]
fn remove_duplicates(state: tauri::State<AppState>, ids: Vec<i64>) -> Result<Vec<i64>, PixieError> {
    let mut removed = Vec::new();
    for p in state.db.removable_copies(&ids)? {
        let path = std::path::Path::new(&p.path);
        let unchanged = std::fs::metadata(path).map(|m| m.len() as i64 == p.size).unwrap_or(false);
        if !unchanged {
            continue; // gone or edited since it was hashed: leave it for the next look
        }
        if trash::delete(path).is_err() {
            continue;
        }
        state.db.mark_removed(p.id)?;
        removed.push(p.id);
    }
    state.db.refresh_dup_flags()?; // a survivor that was hidden is shown again
    Ok(removed)
}

/// Show this copy in Browse for its set of identical files, and keep it if the others are removed.
#[tauri::command]
fn set_duplicate_keeper(state: tauri::State<AppState>, id: i64) -> Result<(), PixieError> {
    state.db.set_keep_pick(id)
}

/// The faces in one photo.
#[tauri::command]
fn faces_for(state: tauri::State<AppState>, photo_id: i64) -> Result<Vec<pixie_core::FaceInfo>, PixieError> {
    state.db.faces_of(photo_id)
}

/// Something was confirmed (a name, a "this is…"): faces that clearly match it can join now. This waits for the regroup
/// (a few seconds) so the screen can redraw with the result, and tells the window.
fn tidy_now(db: &Db, people: &pixie_analysis::people_service::PeopleService) {
    if let Err(e) = db.tidy() {
        eprintln!("[people] cannot regroup faces: {e}");
    }
    people.announce();
}

/// Run `f` off the main thread (the regroup is heavy), with the database and the people service.
async fn blocking<T: Send + 'static>(
    state: &tauri::State<'_, AppState>,
    f: impl FnOnce(Db, Arc<pixie_analysis::people_service::PeopleService>) -> Result<T, PixieError> + Send + 'static,
) -> Result<T, String> {
    let (db, people) = (state.db.clone(), state.people.clone());
    tauri::async_runtime::spawn_blocking(move || f(db, people).map_err(|e| e.to_string())).await.map_err(|e| e.to_string())?
}

/// Name a person (which confirms them). A name somebody already has means the same person: the two become one. Returns
/// the id of the person kept.
#[tauri::command]
async fn rename_person(state: tauri::State<'_, AppState>, id: i64, name: Option<String>) -> Result<i64, String> {
    blocking(&state, move |db, people| {
        let kept = db.name_person(id, name.as_deref())?;
        if name.as_deref().is_some_and(|n| !n.trim().is_empty()) {
            tidy_now(&db, &people);
        }
        Ok(kept)
    })
    .await
}

#[tauri::command]
fn hide_person(state: tauri::State<AppState>, id: i64) -> Result<(), PixieError> {
    state.db.hide_person(id)
}

#[tauri::command]
fn unhide_person(state: tauri::State<AppState>, id: i64) -> Result<(), PixieError> {
    state.db.unhide_person(id)
}

/// Say a group is one person without naming them yet.
#[tauri::command]
async fn confirm_person(state: tauri::State<'_, AppState>, id: i64) -> Result<(), String> {
    blocking(&state, move |db, people| {
        db.confirm_person(id)?;
        tidy_now(&db, &people);
        Ok(())
    })
    .await
}

/// "This is the same person as ...": `from` becomes part of `into`.
#[tauri::command]
async fn assign_group(state: tauri::State<'_, AppState>, from: i64, into: i64) -> Result<(), String> {
    blocking(&state, move |db, people| {
        db.assign_group(from, into)?;
        tidy_now(&db, &people);
        Ok(())
    })
    .await
}

/// What each part of the People tab holds.
#[tauri::command]
fn face_overview(state: tauri::State<AppState>) -> Result<pixie_core::FaceOverview, PixieError> {
    state.db.face_overview()
}

/// The cards of one section of the People tab (see `Db::list_face_people`).
#[tauri::command]
fn list_face_people(state: tauri::State<AppState>, view: pixie_core::FaceView) -> Result<Vec<pixie_core::FacePerson>, PixieError> {
    state.db.list_face_people(view)
}

/// One person's faces, `limit` (default 60) from `offset`, in the order asked for.
#[tauri::command]
fn faces_of_person(state: tauri::State<AppState>, person_id: i64, offset: Option<i64>, limit: Option<i64>, order: pixie_core::FaceOrder) -> Result<Vec<pixie_core::FaceTile>, PixieError> {
    state.db.faces_of_person(person_id, offset.unwrap_or(0).max(0), limit.unwrap_or(60).clamp(1, 5000), order)
}

/// Faces in no group, each with who it looks like.
#[tauri::command]
fn unsorted_faces(state: tauri::State<AppState>, offset: Option<i64>, limit: Option<i64>) -> Result<Vec<pixie_core::UnsortedFace>, PixieError> {
    state.db.unsorted_faces(offset.unwrap_or(0).max(0), limit.unwrap_or(60).clamp(1, 500))
}

/// Say these faces are who they are filed under.
#[tauri::command]
fn confirm_faces(state: tauri::State<AppState>, face_ids: Vec<i64>) -> Result<(), PixieError> {
    state.db.confirm_faces(&face_ids)
}

#[tauri::command]
async fn move_faces(state: tauri::State<'_, AppState>, face_ids: Vec<i64>, person_id: i64) -> Result<(), String> {
    blocking(&state, move |db, people| {
        db.move_faces(&face_ids, person_id)?;
        tidy_now(&db, &people);
        Ok(())
    })
    .await
}

/// "Not this person": the faces wait in no group and are not guessed back.
#[tauri::command]
fn reject_faces(state: tauri::State<AppState>, face_ids: Vec<i64>) -> Result<(), PixieError> {
    state.db.reject_faces(&face_ids)
}

/// Start a new person from these faces (optionally named); returns their id.
#[tauri::command]
async fn new_person_from_faces(state: tauri::State<'_, AppState>, face_ids: Vec<i64>, name: Option<String>) -> Result<i64, String> {
    blocking(&state, move |db, people| {
        let id = db.new_person_from_faces(&face_ids, name.as_deref())?;
        tidy_now(&db, &people);
        Ok(id)
    })
    .await
}

/// Cut a group that looks like several people into pieces; returns how many (0: it does not come apart).
#[tauri::command]
async fn split_group(state: tauri::State<'_, AppState>, id: i64) -> Result<usize, String> {
    let db = state.db.clone();
    tauri::async_runtime::spawn_blocking(move || db.split_group(id).map_err(|e| e.to_string())).await.map_err(|e| e.to_string())?
}

/// Ignore these faces, each on its own.
#[tauri::command]
fn ignore_faces(state: tauri::State<AppState>, face_ids: Vec<i64>) -> Result<(), PixieError> {
    state.db.ignore_faces(&face_ids)
}

#[tauri::command]
fn unignore_faces(state: tauri::State<AppState>, face_ids: Vec<i64>) -> Result<(), PixieError> {
    state.db.unignore_faces(&face_ids)
}

/// The faces the user ignored.
#[tauri::command]
fn ignored_faces(state: tauri::State<AppState>, offset: Option<i64>, limit: Option<i64>) -> Result<Vec<pixie_core::FaceTile>, PixieError> {
    state.db.ignored_faces(offset.unwrap_or(0).max(0), limit.unwrap_or(60).clamp(1, 500))
}

/// Who these faces look like among the named people (for "This is…"), best first.
#[tauri::command]
fn suggest_for_faces(state: tauri::State<AppState>, face_ids: Vec<i64>) -> Result<Vec<pixie_core::Suggestion>, PixieError> {
    state.db.suggest_for_faces(&face_ids)
}

/// Who a group looks like among the named people, best first.
#[tauri::command]
fn suggest_for_group(state: tauri::State<AppState>, id: i64) -> Result<Vec<pixie_core::Suggestion>, PixieError> {
    state.db.suggest_for_group(id)
}

/// Regroup the faces nobody has settled now. Returns false when the faces kept changing meanwhile.
#[tauri::command]
async fn tidy_faces(state: tauri::State<'_, AppState>) -> Result<bool, String> {
    let db = state.db.clone();
    tauri::async_runtime::spawn_blocking(move || db.tidy().map(|r| r.is_some()).map_err(|e| e.to_string())).await.map_err(|e| e.to_string())?
}

/// The tag and the person the listings are really restricted to, so the chips can never disagree with them.
#[derive(Serialize)]
struct ActiveFilters {
    tag: Option<String>,
    person: Option<i64>,
}

#[tauri::command]
fn active_filters(state: tauri::State<AppState>) -> Result<ActiveFilters, PixieError> {
    let (tag, person) = state.db.active_filters()?;
    Ok(ActiveFilters { tag, person })
}

/// Show only photos with this person (null turns it off). List with the `search` flag, like a text search.
#[tauri::command]
fn set_person_filter(state: tauri::State<AppState>, person: Option<i64>) -> Result<(), PixieError> {
    state.db.set_person_filter(person)
}

#[derive(Serialize)]
struct TagCount {
    tag: String,
    count: i64,
    /// The Auto tags group it is listed under (`tags::CATEGORIES`).
    category: String,
    /// How many different photos carry a tag of that category (a photo with two of them counts once).
    category_photos: i64,
}

/// Every tag in use with its photo count and category. Categories come biggest first (by photos), and the tags inside
/// one most common first. A tag the vocabulary no longer has is not listed.
#[tauri::command]
fn list_tags(state: tauri::State<AppState>) -> Result<Vec<TagCount>, PixieError> {
    let mut rows: Vec<(usize, TagCount)> = Vec::new();
    for (tag, count) in state.db.tag_counts()? {
        let Some((category, order)) = pixie_analysis::tags::category_of(&tag) else { continue };
        rows.push((order, TagCount { tag, count, category: category.to_string(), category_photos: 0 }));
    }
    let mut photos: std::collections::HashMap<usize, i64> = std::collections::HashMap::new();
    for (order, _) in &rows {
        if photos.contains_key(order) {
            continue;
        }
        let members: Vec<&str> = rows.iter().filter(|(o, _)| o == order).map(|(_, t)| t.tag.as_str()).collect();
        photos.insert(*order, state.db.photos_with_any_tag(&members)?);
    }
    for (order, t) in &mut rows {
        t.category_photos = photos[order];
    }
    rows.sort_by(|a, b| {
        b.1.category_photos
            .cmp(&a.1.category_photos)
            .then(a.0.cmp(&b.0))
            .then(b.1.count.cmp(&a.1.count))
            .then(a.1.tag.cmp(&b.1.tag))
    });
    Ok(rows.into_iter().map(|(_, t)| t).collect())
}

#[derive(Serialize)]
struct PhotoTag {
    tag: String,
    /// 0..1: the tag's share of the probability against every label and decoy.
    score: f32,
}

/// One photo's tags, best first.
#[tauri::command]
fn tags_for(state: tauri::State<AppState>, photo_id: i64) -> Result<Vec<PhotoTag>, PixieError> {
    Ok(state.db.tags_for(photo_id)?.into_iter().map(|(tag, score)| PhotoTag { tag, score }).collect())
}

/// Show only photos with this tag (null turns it off). List with the `search` flag, like a text search.
#[tauri::command]
fn set_tag_filter(state: tauri::State<AppState>, tag: Option<String>) -> Result<(), PixieError> {
    state.db.set_tag_filter(tag.as_deref())
}

#[tauri::command]
fn analysis_retry(state: tauri::State<AppState>) {
    state.analysis.retry();
    state.people.retry();
}

/// Make `text` the current search (see the `search` flag of the listing commands); returns the match count.
/// Runs on the blocking pool: it embeds the text and scans every stored vector.
#[tauri::command]
async fn search_photos(state: tauri::State<'_, AppState>, text: String) -> Result<usize, String> {
    let analysis = state.analysis.clone();
    tauri::async_runtime::spawn_blocking(move || analysis.search(&text)).await.map_err(|e| e.to_string())?
}

#[tauri::command]
fn cancel_scan(state: tauri::State<AppState>) {
    state.cancel_scan.store(true, Ordering::Relaxed);
    state.pipeline.clear_intake(); // at once, not when the walk notices
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

/// Whether this is a debug build: the Developer page in Settings exists only there.
#[tauri::command]
fn is_debug_build() -> bool {
    cfg!(debug_assertions)
}

/// Where the library, caches and models live. Debug builds obey `PIXIE_DATA_DIR`, so a test run can use a folder of its own
/// (Windows ignores a changed `LOCALAPPDATA` for this, so that cannot be used to keep a test run off the real library).
fn data_dir(path: &tauri::path::PathResolver<tauri::Wry>) -> tauri::Result<PathBuf> {
    #[cfg(debug_assertions)]
    if let Some(d) = std::env::var_os("PIXIE_DATA_DIR") {
        return Ok(PathBuf::from(d));
    }
    path.app_local_data_dir()
}

/// Debug builds only: show Pixie's data folder (database, caches, models) in the file manager.
#[tauri::command]
fn open_data_folder(app: tauri::AppHandle) -> Result<(), PixieError> {
    if !cfg!(debug_assertions) {
        return Err(PixieError::Unsupported("only available in debug builds".into()));
    }
    let dir = data_dir(app.path()).map_err(|e| PixieError::Io(e.to_string()))?;
    open::that_detached(&dir).map_err(|e| PixieError::Io(e.to_string()))
}

fn now_secs() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

/// Runs of days spent away from home, newest first. Home and the thresholds come from the caller (they are settings
/// of the interface), so changing them needs no stored state here.
#[tauri::command]
fn list_trips(state: tauri::State<AppState>, home_lat: f64, home_lon: f64, radius_km: f64, min_nights: u32) -> Result<Vec<pixie_index::trips::Trip>, PixieError> {
    state.db.trips(&pixie_index::trips::TripSettings { home_lat, home_lon, radius_km, min_nights }, now_secs())
}

/// The places a trip went and the order it visited them, for naming its destination and drawing it on the map.
#[tauri::command]
fn trip_route(
    state: tauri::State<AppState>,
    since: i64,
    until: i64,
    home_lat: f64,
    home_lon: f64,
    radius_km: f64,
    stop_km: f64,
) -> Result<pixie_index::trips::Route, PixieError> {
    let home = pixie_index::trips::TripSettings { home_lat, home_lon, radius_km, min_nights: 0 };
    state.db.trip_route(since, until, &home, stop_km)
}

/// A guess at where home is, as [latitude, longitude]; None when no photo has a position.
#[tauri::command]
fn suggest_home(state: tauri::State<AppState>) -> Result<Option<(f64, f64)>, PixieError> {
    state.db.suggest_home(now_secs())
}

/// Names the trip covering `since` to `until` (as given by `list_trips`); an empty name removes it.
#[tauri::command]
fn rename_trip(state: tauri::State<AppState>, since: i64, until: i64, name: String) -> Result<(), PixieError> {
    state.db.rename_trip(since, until, &name)
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
    search: Option<bool>,
) -> Result<Vec<Photo>, PixieError> {
    state.db.photos_at(&query_of(order, folder_id, kind, bounds, max_id, since, until, search), &indexes)
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
    search: Option<bool>,
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
        let query = query_of(request.order, request.folder_id, request.kind, request.bounds, request.max_id, request.since, request.until, request.search);
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

/// Opens a new email with the photo attached (Outlook). Returns which way it was done, for the message to the person.
#[tauri::command]
async fn email_photo(app: tauri::AppHandle, state: tauri::State<'_, AppState>, id: i64, lang: String) -> Result<String, String> {
    let photo = match state.db.get_photo(id) {
        Ok(Some(p)) => p,
        _ => return Err("That photo is not in the library any more.".into()),
    };
    let db = state.db.clone();
    let places = state.places.clone();
    tauri::async_runtime::spawn_blocking(move || {
        // The subject: who is in the photo, where it was taken and when, as far as is known (a place needs the network;
        // without it the subject simply has no place).
        let people: Vec<String> = db.faces_of(id).unwrap_or_default().into_iter().filter_map(|f| f.person_name).collect();
        let place = match (photo.meta.gps_lat, photo.meta.gps_lon) {
            (Some(lat), Some(lon)) => places.place_for_background(lat, lon, &lang).ok().flatten().and_then(|p| p.landmark.or(p.locality)),
            _ => None,
        };
        let file_name = std::path::Path::new(&photo.path).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let subject = subject::email_subject(&people, place.as_deref(), photo.meta.taken_at, &file_name);
        outlook::email_file(&app, std::path::Path::new(&photo.path), &subject).map(|r| match r {
            outlook::Route::ClassicOutlook => "outlook".to_string(),
            outlook::Route::NewOutlook => "newoutlook".to_string(),
            outlook::Route::MacOutlook => "outlook".to_string(),
            outlook::Route::AppleMail => "mail".to_string(),
            outlook::Route::ShareSheet => "share".to_string(),
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Blocks on the rate limit and the network, so it runs on the blocking pool, not an
/// async worker. A lookup overtaken by a newer one returns `Err` quickly (the UI drops it).
#[tauri::command]
async fn place_for(
    state: tauri::State<'_, AppState>,
    lat: f64,
    lon: f64,
    lang: String,
    background: Option<bool>,
) -> Result<Option<Place>, String> {
    let places = state.places.clone();
    // Background lookups (naming trips) must not cancel, or be cancelled by, the one for the photo on screen.
    tauri::async_runtime::spawn_blocking(move || if background == Some(true) { places.place_for_background(lat, lon, &lang) } else { places.place_for(lat, lon, &lang) })
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

/// `/face/{face_id}?s=96|160|320`: a square JPEG of that face, cut from the photo's (upright) viewer thumbnail.
fn handle_face(state: &AppState, path: &str, query: Option<String>, responder: tauri::UriSchemeResponder) {
    let size = pixie_analysis::people::face_size(query.as_deref());
    let Some(id) = path.trim_matches('/').split('/').nth(1).and_then(|s| s.parse::<i64>().ok()) else {
        return responder.respond(status(StatusCode::NOT_FOUND));
    };
    let (photo, face) = match state.db.face_location(id) {
        Ok(Some(found)) => found,
        Ok(None) => return responder.respond(status(StatusCode::NOT_FOUND)),
        Err(_) => return responder.respond(status(StatusCode::INTERNAL_SERVER_ERROR)),
    };
    // Everything the picture depends on is in the key, so a kept picture is always the right one.
    let key = format!("{}-{}-{}-{:.4}-{:.4}-{:.4}-{:.4}-{size}", photo.id, photo.mtime, photo.size, face[0], face[1], face[2], face[3]);
    let dir = state.faces_dir.clone();
    if let Ok(bytes) = pixie_analysis::people::cached_face(&dir, &key, || Err(String::new())) {
        return responder.respond(jpeg_response(bytes)); // made before: no thumbnail needed
    }
    state.thumbs.request_with(
        ThumbClass::Viewer,
        photo.id,
        photo.mtime,
        photo.size,
        PathBuf::from(&photo.path),
        Box::new(move |result| {
            let jpeg = result.map_err(|e| e.to_string()).and_then(|file| {
                pixie_analysis::people::cached_face(&dir, &key, || {
                    let bytes = std::fs::read(&file).map_err(|e| e.to_string())?;
                    pixie_analysis::people::face_jpeg(&bytes, face, size)
                })
            });
            responder.respond(match jpeg {
                Ok(bytes) => jpeg_response(bytes),
                Err(e) => {
                    eprintln!("[face] {id}: {e}");
                    status(StatusCode::INTERNAL_SERVER_ERROR)
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

/// How many of the newest photos get their grid thumbnail made up front.
const PREFETCH_NEWEST: i64 = 500;

/// Queue grid thumbnails for the newest photos (the first screens of Browse, newest first) at the lowest priority, so
/// they are ready at once. The rest are made by the analysis pipeline as it reads each photo (it works newest first
/// too), or when a tile is first shown.
fn prefetch_newest(db: &Db, thumbs: &ThumbService) {
    let Ok(page) = db.list_photos(0, PREFETCH_NEWEST) else { return };
    thumbs.prefetch(
        page.into_iter()
            .filter(|p| p.meta.kind == MediaKind::Photo)
            .map(|p| (ThumbClass::Grid, p.id, p.mtime, p.size, PathBuf::from(p.path)))
            .collect(),
    );
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
                Some("face") => handle_face(&state, path, request.uri().query().map(str::to_owned), responder),
                _ => handle_thumb(&state, path, responder),
            }
        })
        .setup(|app| {
            // The local (not roaming) data folder: a big database and caches do not belong in a roaming profile, and
            // it is where the app window keeps its own storage too.
            let dir = data_dir(app.path())?;
            std::fs::create_dir_all(&dir)?;
            let db = Db::open(&dir.join("pixie.db"))?;
            // Leave one core for the UI; decoding is the heavy part.
            let workers = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).saturating_sub(1).clamp(2, 6);
            let thumbs = ThumbService::new(dir.join("thumbs"), workers);
            let (pdb, pthumbs) = (db.clone(), thumbs.clone());
            std::thread::spawn(move || prefetch_newest(&pdb, &pthumbs));
            let fetcher: Arc<dyn pixie_geo::Fetcher> = Arc::new(HttpFetcher::new());
            let places = Arc::new(PlaceService::new(
                PlaceCache::open(&dir.join("places.db"))?,
                fetcher.clone(),
                Duration::from_millis(1100),
            ));
            let tiles = TileService::new(dir.join("tiles"), fetcher, 2);
            let gate = pixie_index::ScanGate::default();
            // One pipeline reads, hashes and decodes each photo once for every step; the two services below give it their
            // models and show its progress.
            let pipeline_gate = gate.clone();
            let pipeline = Arc::new(pixie_analysis::pipeline::PhotoPipeline::start(pixie_analysis::pipeline::PipelineConfig::new(
                db.clone(),
                dir.join("thumbs"),
                Arc::new(move || pipeline_gate.is_busy()),
            )));
            let latest_app = app.handle().clone();
            // The photo the pipeline just finished, for the loading screen's strip of pictures going by.
            pipeline.set_latest_listener(Arc::new(move |item| {
                let _ = latest_app.emit("analysis-latest", item);
            }));
            let analysis_app = app.handle().clone();
            let analysis = Arc::new(pixie_analysis::service::AnalysisService::start(pixie_analysis::service::Config {
                db: db.clone(),
                version: pixie_analysis::clip::MODEL_VERSION.to_string(),
                dir: dir.join("models").join(pixie_analysis::clip::MODEL_VERSION),
                files: pixie_analysis::models::CLIP_FILES,
                downloader: Arc::new(pixie_analysis::models::HttpDownloader),
                loader: Arc::new(|d| {
                    pixie_analysis::clip::ClipEmbedder::load(d).map(|e| Arc::new(e) as Arc<dyn pixie_analysis::embedder::Embedder>)
                }),
                pipeline: pipeline.clone(),
                on_status: Arc::new(move |s| {
                    let _ = analysis_app.emit("analysis-status", s);
                }),
            }));
            let people_app = app.handle().clone();
            let people = Arc::new(pixie_analysis::people_service::PeopleService::start(pixie_analysis::people_service::PeopleConfig {
                db: db.clone(),
                version: pixie_analysis::faces::FACES_VERSION.to_string(),
                dir: dir.join("models").join(pixie_analysis::faces::FACES_VERSION),
                files: pixie_analysis::models::FACE_FILES,
                downloader: Arc::new(pixie_analysis::models::HttpDownloader),
                loader: Arc::new(|d| {
                    pixie_analysis::faces::FaceModels::load(d).map(|e| Arc::new(e) as Arc<dyn pixie_analysis::faces::FaceEngine>)
                }),
                pipeline: pipeline.clone(),
                on_status: Arc::new(move |s| {
                    let _ = people_app.emit("people-status", s);
                }),
            }));
            let dupes_app = app.handle().clone();
            let dupes_gate = gate.clone();
            let dupes = Arc::new(pixie_index::DupeService::start(
                db.clone(),
                Arc::new(move || dupes_gate.is_busy()),
                Arc::new(move |s| {
                    let _ = dupes_app.emit("duplicate-status", s);
                }),
            ));
            let watch_db = db.clone();
            app.manage(AppState {
                db,
                thumbs,
                tiles,
                places,
                posters_dir: dir.join("posters"),
                faces_dir: dir.join("faces"),
                secrets: Arc::new(OsSecrets),
                mailer: Arc::new(SmtpMailer),
                cancel_scan: Arc::new(Default::default()),
                gate,
                queue: ScanQueue::default(),
                analysis,
                people,
                dupes,
                pipeline,
            });
            // Keeps the library up to date: folders on this computer are watched, network ones are scanned on a timer (the
            // window turns that on and says how often).
            let auto_app = app.handle().clone();
            app.manage(autoscan::AutoScan::start(
                watch_db,
                Arc::new(|p| drives::is_network_path(p)),
                Arc::new(move |ids| {
                    let state = auto_app.state::<AppState>();
                    request_scan(&auto_app, state.inner(), &ids);
                }),
                autoscan::Timing::APP,
            ));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            add_folder, scan_folders, set_network_rescan, suggest_more, load_progress, facet_counts, list_folders, list_folder_info, remove_folder, open_viewer_window, list_photos, count_photos, list_day_groups, list_geo_points, max_photo_id, get_photo, start_scan, cancel_scan, analysis_status, analysis_set_paused, analysis_retry, search_photos, list_tags, tags_for, set_tag_filter, people_status, list_people, faces_for, rename_person, hide_person, confirm_person, assign_group, face_overview, unsorted_faces, confirm_faces, new_person_from_faces, split_group, tidy_faces, ignore_faces, unignore_faces, ignored_faces, suggest_for_faces, suggest_for_group, set_person_filter, active_filters, suggest_folders, is_debug_build, open_data_folder, list_trips, trip_route, suggest_home, rename_trip, list_face_people, faces_of_person, move_faces, reject_faces, unhide_person, print_page, place_for, save_poster,
            open_in_default_player, email_photo, list_providers, get_mail_account, save_mail_account, delete_mail_account, list_contacts,
            save_contact, delete_contact, duplicate_groups, duplicate_status, remove_duplicates, set_duplicate_keeper, photos_at, send_test_email, send_email
        ])
        .run(tauri::generate_context!())
        .expect("error while running Pixie");
}

#[cfg(test)]
mod scan_message_tests {
    use super::*;

    fn nf() -> PixieError {
        PixieError::NotFound("x".into())
    }

    #[test]
    fn a_drive_that_is_not_connected_is_named() {
        let none = |_: &str| false;
        assert_eq!(scan_error_message_with(&nf(), Some("/Volumes/photo/2019"), none), "Pixie can't reach /Volumes/photo/2019: \"photo\" is not connected.");
        assert_eq!(scan_error_message_with(&nf(), Some("p:\\joan"), none), "Pixie can't reach p:\\joan: drive P: is not connected.");
        assert_eq!(scan_error_message_with(&nf(), Some("\\\\nas\\photo\\joan"), none), "Pixie can't reach \\\\nas\\photo\\joan: \\\\nas\\photo is not connected.");
    }

    #[test]
    fn a_folder_that_is_gone_from_a_connected_drive_is_said_to_be_gone() {
        let all = |_: &str| true;
        let m = scan_error_message_with(&nf(), Some("/Volumes/photo/2019"), all);
        assert_eq!(m, "Pixie can't find /Volumes/photo/2019. It may have been moved, renamed or deleted.");
        assert_eq!(scan_error_message_with(&nf(), Some("/Users/me/Pictures"), |_| false), "Pixie can't find /Users/me/Pictures. It may have been moved, renamed or deleted.");
    }

    #[test]
    fn other_problems_keep_their_usual_message() {
        assert_eq!(scan_error_message_with(&nf(), None, |_| false), nf().user_message());
    }
}
