//! Import: the commands behind the Import tool. A drive or folder is opened as a temporary source (indexed where it is, see
//! `pixie_index::source`), looked at in Browse, and its picked or new photos are kept into a library folder (`pixie_index::keep`).

use crate::drives::{self, Removable};
use crate::state::AppState;
use pixie_core::{PhotoQuery, PixieError};
use pixie_image::thumbs::{thumb_path, ThumbClass};
use pixie_index::copier::CopyJob;
use pixie_index::import::SourceSummary;
use pixie_index::inbox;
use pixie_index::keep::{run_keep, KeepProgress, Layout, Mode};
use pixie_index::source::scan_source;
use pixie_index::zipsrc::{extract_zip, list_zip, ExtractProgress, ExtractReport};
use serde::Serialize;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, State};

/// What the Import tool needs to remember between commands: one Keep at a time, and which source is being scanned.
#[derive(Default)]
pub struct ImportState {
    /// A Keep is running (and how to stop it).
    pub job: CopyJob,
    /// Asks the scan of a source to stop.
    scan_stop: AtomicBool,
    /// The source being scanned now.
    scanning: Mutex<Option<i64>>,
    /// The source a zip is being extracted into now (stopped with `scan_stop`).
    extracting: Mutex<Option<i64>>,
}

/// A source that is open, with what is known about it.
#[derive(Serialize)]
pub struct OpenSource {
    pub id: i64,
    pub path: String,
    pub label: String,
    pub key: String,
    pub scanning: bool,
    /// A zip is being extracted into it.
    pub extracting: bool,
    /// It is in the Inbox (extracted from a zip): Keep moves its files out, and Discard deletes what is left.
    pub inbox: bool,
    pub summary: SourceSummary,
}

#[derive(Serialize, Clone)]
struct ScanEvent {
    source: i64,
    seen: usize,
    indexed: usize,
    to_index: usize,
}

#[derive(Serialize, Clone)]
struct ScanDone {
    source: i64,
    error: Option<String>,
    summary: Option<SourceSummary>,
}

#[derive(Serialize, Clone)]
struct KeepDone {
    source: i64,
    error: Option<String>,
    report: Option<pixie_index::keep::KeepReport>,
}

/// What a zip holds, for the Look step: its directory is read, nothing is extracted.
#[derive(Serialize)]
pub struct ZipLook {
    pub name: String,
    pub photos: usize,
    pub videos: usize,
    pub other: usize,
    pub archives: usize,
    pub encrypted: usize,
    pub unsafe_names: usize,
    pub too_big: usize,
    /// What extracting the photos and videos would take.
    pub bytes: u64,
    /// What is free where the Inbox is.
    pub free: u64,
    pub fits: bool,
}

#[derive(Serialize, Clone)]
struct ExtractEvent {
    source: i64,
    #[serde(flatten)]
    progress: ExtractProgress,
}

#[derive(Serialize, Clone)]
struct ExtractDone {
    source: i64,
    error: Option<String>,
    report: Option<ExtractReport>,
}

/// A size in words, for a sentence about space.
fn human(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["bytes", "KB", "MB", "GB", "TB"];
    let (mut n, mut u) = (bytes as f64, 0);
    while n >= 1024.0 && u < UNITS.len() - 1 {
        n /= 1024.0;
        u += 1;
    }
    if u == 0 { format!("{bytes} bytes") } else { format!("{n:.1} {}", UNITS[u]) }
}

/// The drives to offer: cards, USB sticks and drives with a `DCIM` folder. Asks the drives for their labels, so it runs off the UI thread.
#[tauri::command]
pub async fn import_available() -> Result<Vec<Removable>, String> {
    tauri::async_runtime::spawn_blocking(drives::removable_roots).await.map_err(|e| e.to_string())
}

/// The items of a source, read before they go, so their thumbnails can be deleted after.
fn items_of(state: &AppState, id: i64) -> Vec<pixie_core::Photo> {
    state.db.list_photos_in(&PhotoQuery { folder_id: Some(id), ..Default::default() }, 0, i64::MAX).unwrap_or_default()
}

/// Deletes the thumbnails and posters of items that were removed (they are in a cache that nothing else looks after).
fn forget_thumbnails(state: &AppState, photos: &[pixie_core::Photo], removed: &[i64]) {
    for p in photos.iter().filter(|p| removed.contains(&p.id)) {
        for class in [ThumbClass::Grid, ThumbClass::Viewer] {
            let _ = std::fs::remove_file(thumb_path(&state.thumbs_dir, class, p.id, p.mtime, p.size));
        }
        let _ = std::fs::remove_file(pixie_image::posters::poster_path(&state.posters_dir, p.id, p.mtime, p.size));
    }
}

/// Forgets a source with its rows, picks and thumbnails.
fn close_source(state: &AppState, id: i64) {
    let photos = items_of(state, id);
    if let Ok(ids) = state.db.remove_source(id) {
        forget_thumbnails(state, &photos, &ids);
    }
}

/// Whether the source is one of the Inbox's (extracted from a zip).
fn in_inbox(state: &AppState, id: i64) -> bool {
    state.db.list_sources().unwrap_or_default().iter().any(|s| s.id == id && inbox::is_in_inbox(&state.inbox_dir, &s.path))
}

/// Closes the sources whose drive or folder is no longer there (a card pulled out, a stick unplugged), also at start-up.
pub fn prune_sources(state: &AppState) {
    for s in state.db.list_sources().unwrap_or_default() {
        if !Path::new(&s.path).is_dir() {
            close_source(state, s.id);
        }
    }
}

/// The open sources, with what is known about each. Sources whose drive has gone are closed first.
#[tauri::command]
pub async fn import_sources(app: AppHandle) -> Result<Vec<OpenSource>, PixieError> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        prune_sources(&state);
        let scanning = *state.import.scanning.lock().unwrap_or_else(|p| p.into_inner());
        let extracting = *state.import.extracting.lock().unwrap_or_else(|p| p.into_inner());
        state
            .db
            .list_sources()?
            .into_iter()
            .map(|s| {
                Ok(OpenSource {
                    summary: state.db.source_summary(s.id)?,
                    scanning: scanning == Some(s.id),
                    extracting: extracting == Some(s.id),
                    inbox: inbox::is_in_inbox(&state.inbox_dir, &s.path),
                    id: s.id,
                    path: s.path,
                    label: s.label,
                    key: s.key,
                })
            })
            .collect()
    })
    .await
    .map_err(|e| PixieError::Db(e.to_string()))?
}

/// Opens a drive or folder as a source and starts reading its list of photos (dates, sizes and dimensions only; nothing is copied or analysed).
/// `import-scan` events say how far it has got, `import-scan-done` that it has finished. Returns the source's id at once.
#[tauri::command]
pub async fn import_open(app: AppHandle, path: String, label: Option<String>) -> Result<i64, PixieError> {
    let opened = {
        let app = app.clone();
        tauri::async_runtime::spawn_blocking(move || -> Result<i64, PixieError> {
            let state = app.state::<AppState>();
            if !Path::new(&path).is_dir() {
                return Err(PixieError::NotFound(path));
            }
            // A drive is known by its volume, so it is the same source whatever letter it gets; any other folder by its path.
            let drive = drives::removable_roots().into_iter().find(|d| Path::new(&d.path) == Path::new(&path));
            let (key, name) = match drive {
                Some(d) => (d.key, d.label),
                None => {
                    let name = label.filter(|l| !l.trim().is_empty()).unwrap_or_else(|| Path::new(&path).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| path.clone()));
                    (format!("folder:{path}"), name)
                }
            };
            state.db.add_source(&path, &key, &name)
        })
        .await
        .map_err(|e| PixieError::Db(e.to_string()))??
    };
    scan(app, opened);
    Ok(opened)
}

/// Reads a source's list in the background.
fn scan(app: AppHandle, id: i64) {
    std::thread::spawn(move || {
        let state = app.state::<AppState>();
        {
            let mut busy = state.import.scanning.lock().unwrap_or_else(|p| p.into_inner());
            if busy.is_some() {
                let _ = app.emit("import-scan-done", ScanDone { source: id, error: Some("Another source is still being read. Stop it first.".into()), summary: None });
                return;
            }
            *busy = Some(id);
        }
        state.import.scan_stop.store(false, Ordering::Release);
        let events = app.clone();
        let result = scan_source(&state.db, id, &state.import.scan_stop, |p| {
            let _ = events.emit("import-scan", ScanEvent { source: id, seen: p.seen, indexed: p.indexed, to_index: p.to_index });
        });
        *state.import.scanning.lock().unwrap_or_else(|p| p.into_inner()) = None;
        let (error, summary) = match result {
            Ok(_) => (None, state.db.source_summary(id).ok()),
            Err(e) => (Some(e.user_message()), None),
        };
        let _ = app.emit("import-scan-done", ScanDone { source: id, error, summary });
    });
}

/// Reads a source's list again (after a card was changed, or a scan was stopped).
#[tauri::command]
pub fn import_rescan(app: AppHandle, id: i64) {
    scan(app, id);
}

/// Closes a source: its rows, picks and thumbnails go; the card is not touched.
#[tauri::command]
pub async fn import_close(app: AppHandle, id: i64) -> Result<(), PixieError> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        if in_inbox(&state, id) {
            return Err(PixieError::Invalid("That is in the Inbox: use Discard to remove what is left of it.".into()));
        }
        if *state.import.scanning.lock().unwrap_or_else(|p| p.into_inner()) == Some(id) {
            state.import.scan_stop.store(true, Ordering::Release);
        }
        close_source(&state, id);
        Ok(())
    })
    .await
    .map_err(|e| PixieError::Db(e.to_string()))?
}

/// What a source holds and how much of it is new.
#[tauri::command]
pub fn import_summary(state: State<AppState>, id: i64) -> Result<SourceSummary, PixieError> {
    state.db.source_summary(id)
}

/// How many photos of a source are picked (what "Keep picked" would keep).
#[tauri::command]
pub fn import_picked_count(state: State<AppState>, source: i64) -> Result<usize, PixieError> {
    Ok(state.db.picked_in(source)?.len())
}

/// Keeps photos of a source into the library folder `dest`, filed by `layout` (`year-date`, `year-month` or `flat`). `which` is `new` (everything
/// the library and earlier imports do not already have) or `picked` (the photos of this source that are picked). Copies safely, one file at a time, and sends `import-progress` while it goes and `import-done` at the end. The
/// folder is then scanned so the copies join the library.
#[tauri::command]
pub fn import_keep(app: AppHandle, state: State<AppState>, source: i64, which: String, dest: i64, layout: String) -> Result<(), PixieError> {
    let layout = Layout::parse(&layout).ok_or_else(|| PixieError::Invalid("That way of filing photos is not known.".into()))?;
    if which != "new" && which != "picked" {
        return Err(PixieError::Invalid("Choose to keep what is new or what is picked.".into()));
    }
    if *state.import.extracting.lock().unwrap_or_else(|p| p.into_inner()) == Some(source) {
        return Err(PixieError::Busy("that zip is still being extracted".into()));
    }
    // What is in the Inbox is moved into the folder (the Inbox is only a holding place); a card is only read.
    let mode = if in_inbox(state.inner(), source) { Mode::Move } else { Mode::Copy };
    let job = state.import.clone();
    if !job.job.try_start() {
        return Err(PixieError::Busy("photos are already being kept".into()));
    }
    std::thread::spawn(move || {
        let state = app.state::<AppState>();
        let ids = if which == "picked" { state.db.picked_in(source) } else { state.db.source_new_ids(source) };
        let result = ids.and_then(|ids| {
            let events = app.clone();
            run_keep(&state.db, source, &ids, dest, layout, mode, job.job.stop_flag(), |p: &KeepProgress| {
                let _ = events.emit("import-progress", *p);
            })
        });
        job.job.finish();
        let done = match result {
            Ok(report) => {
                if report.copied + report.already > 0 {
                    crate::request_scan(&app, &state, &[dest]);
                }
                KeepDone { source, error: None, report: Some(report) }
            }
            Err(e) => KeepDone { source, error: Some(e.user_message()), report: None },
        };
        let _ = app.emit("import-done", done);
    });
    Ok(())
}

/// Stops a Keep after the file it is on, or a scan of a source.
#[tauri::command]
pub fn import_stop(state: State<AppState>) {
    state.import.job.stop();
    state.import.scan_stop.store(true, Ordering::Release);
}

/// Reads what a zip holds (its directory only: nothing is extracted) and whether it would fit where the Inbox is.
#[tauri::command]
pub async fn import_zip_look(app: AppHandle, path: String) -> Result<ZipLook, PixieError> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let listing = list_zip(Path::new(&path))?;
        let free = inbox::free_space(&state.inbox_dir).unwrap_or(0);
        Ok(ZipLook {
            name: Path::new(&path).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or(path.clone()),
            photos: listing.photos,
            videos: listing.videos,
            other: listing.other,
            archives: listing.archives,
            encrypted: listing.encrypted,
            unsafe_names: listing.unsafe_names,
            too_big: listing.too_big,
            bytes: listing.bytes,
            fits: listing.bytes <= free,
            free,
        })
    })
    .await
    .map_err(|e| PixieError::Db(e.to_string()))?
}

/// What `import_zip_extract` works out before it starts: the source, what to extract and where, or that the zip was open already.
struct Prepared {
    id: i64,
    listing: pixie_index::zipsrc::ZipListing,
    dir: std::path::PathBuf,
    already_open: bool,
}

fn prepare_zip(state: &AppState, path: &str) -> Result<Prepared, PixieError> {
    let zip = Path::new(path);
    let listing = list_zip(zip)?;
    let key = inbox::zip_key(zip).map_err(|e| PixieError::Invalid(format!("That zip cannot be read ({e}).")))?;
    if let Some(open) = state.db.list_sources()?.into_iter().find(|s| s.key == key) {
        return Ok(Prepared { id: open.id, listing, dir: std::path::PathBuf::new(), already_open: true });
    }
    if listing.bytes == 0 {
        return Err(PixieError::Invalid("There are no photos or videos in this zip.".into()));
    }
    let free = inbox::free_space(&state.inbox_dir).unwrap_or(0);
    if listing.bytes > free {
        return Err(PixieError::Invalid(format!("This zip needs {} but only {} is free where the Inbox is.", human(listing.bytes), human(free))));
    }
    let mut busy = state.import.extracting.lock().unwrap_or_else(|p| p.into_inner());
    if busy.is_some() {
        return Err(PixieError::Busy("another zip is being extracted".into()));
    }
    let label = zip.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "zip".into());
    let dir = inbox::new_inbox_dir(&state.inbox_dir, &label).map_err(|e| PixieError::Invalid(format!("The Inbox folder cannot be made ({e}).")))?;
    let id = match state.db.add_source(&dir.to_string_lossy(), &key, &label) {
        Ok(id) => id,
        Err(e) => {
            let _ = std::fs::remove_dir_all(&dir);
            return Err(e);
        }
    };
    *busy = Some(id);
    Ok(Prepared { id, listing, dir, already_open: false })
}

/// Extracts the photos and videos of a zip into a new folder of the Inbox, which becomes a source to look at, keep from and discard. Returns the
/// source's id at once; `import-extract` events say how far it has got and `import-extract-done` that it has finished (the source is then read like
/// any other). A zip that is already open gives its source again.
#[tauri::command]
pub async fn import_zip_extract(app: AppHandle, path: String) -> Result<i64, PixieError> {
    let prepared = {
        let (app, path) = (app.clone(), path.clone());
        tauri::async_runtime::spawn_blocking(move || prepare_zip(&app.state::<AppState>(), &path)).await.map_err(|e| PixieError::Db(e.to_string()))??
    };
    if prepared.already_open {
        return Ok(prepared.id);
    }
    let Prepared { id, listing, dir, .. } = prepared;
    let zip = std::path::PathBuf::from(path);
    std::thread::spawn(move || {
        let state = app.state::<AppState>();
        state.import.scan_stop.store(false, Ordering::Release);
        let events = app.clone();
        let result = extract_zip(&zip, &listing, &dir, &state.import.scan_stop, |p| {
            let _ = events.emit("import-extract", ExtractEvent { source: id, progress: *p });
        });
        *state.import.extracting.lock().unwrap_or_else(|p| p.into_inner()) = None;
        let done = match result {
            Ok(report) => ExtractDone { source: id, error: None, report: Some(report) },
            Err(e) => ExtractDone { source: id, error: Some(e.user_message()), report: None },
        };
        let _ = app.emit("import-extract-done", done);
        scan(app, id); // reads what was extracted, like any source
    });
    Ok(id)
}

/// Deletes what is left of an Inbox source: its files, its folder, its rows, picks and thumbnails. Never a card.
#[tauri::command]
pub async fn import_discard(app: AppHandle, id: i64) -> Result<(), PixieError> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let reading = *state.import.extracting.lock().unwrap_or_else(|p| p.into_inner()) == Some(id) || *state.import.scanning.lock().unwrap_or_else(|p| p.into_inner()) == Some(id);
        if reading {
            state.import.scan_stop.store(true, Ordering::Release);
            return Err(PixieError::Busy("that is still being read; try again in a moment".into()));
        }
        let photos = items_of(&state, id);
        let removed = inbox::discard_source(&state.db, &state.inbox_dir, id)?;
        forget_thumbnails(&state, &photos, &removed);
        Ok(())
    })
    .await
    .map_err(|e| PixieError::Db(e.to_string()))?
}
