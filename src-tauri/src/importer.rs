//! Import: the commands behind the Import tool. A drive or folder is opened as a temporary source (indexed where it is, see
//! `pixie_index::source`), looked at in Browse, and its picked or new photos are kept into a library folder (`pixie_index::keep`).

use crate::drives::{self, Removable};
use crate::state::AppState;
use pixie_core::{PhotoQuery, PixieError};
use pixie_image::thumbs::{thumb_path, ThumbClass};
use pixie_index::copier::CopyJob;
use pixie_index::import::SourceSummary;
use pixie_index::keep::{run_keep, KeepProgress, Layout};
use pixie_index::source::scan_source;
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
}

/// A source that is open, with what is known about it.
#[derive(Serialize)]
pub struct OpenSource {
    pub id: i64,
    pub path: String,
    pub label: String,
    pub key: String,
    pub scanning: bool,
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

/// The drives to offer: cards, USB sticks and drives with a `DCIM` folder. Asks the drives for their labels, so it runs off the UI thread.
#[tauri::command]
pub async fn import_available() -> Result<Vec<Removable>, String> {
    tauri::async_runtime::spawn_blocking(drives::removable_roots).await.map_err(|e| e.to_string())
}

/// Forgets a source with its rows, picks and thumbnails.
fn close_source(state: &AppState, id: i64) {
    let photos = state.db.list_photos_in(&PhotoQuery { folder_id: Some(id), ..Default::default() }, 0, i64::MAX).unwrap_or_default();
    if let Ok(ids) = state.db.remove_source(id) {
        // (their thumbnails are in a cache that is not looked after by anything else)
        let thumbs = state.thumbs_dir.clone();
        for p in photos.iter().filter(|p| ids.contains(&p.id)) {
            for class in [ThumbClass::Grid, ThumbClass::Viewer] {
                let _ = std::fs::remove_file(thumb_path(&thumbs, class, p.id, p.mtime, p.size));
            }
            let _ = std::fs::remove_file(pixie_image::posters::poster_path(&state.posters_dir, p.id, p.mtime, p.size));
        }
    }
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
        state
            .db
            .list_sources()?
            .into_iter()
            .map(|s| {
                Ok(OpenSource { summary: state.db.source_summary(s.id)?, scanning: scanning == Some(s.id), id: s.id, path: s.path, label: s.label, key: s.key })
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
        if *state.import.scanning.lock().unwrap_or_else(|p| p.into_inner()) == Some(id) {
            state.import.scan_stop.store(true, Ordering::Release);
        }
        close_source(&state, id);
    })
    .await
    .map_err(|e| PixieError::Db(e.to_string()))
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
    let job = state.import.clone();
    if !job.job.try_start() {
        return Err(PixieError::Busy("photos are already being kept".into()));
    }
    std::thread::spawn(move || {
        let state = app.state::<AppState>();
        let ids = if which == "picked" { state.db.picked_in(source) } else { state.db.source_new_ids(source) };
        let result = ids.and_then(|ids| {
            let events = app.clone();
            run_keep(&state.db, source, &ids, dest, layout, job.job.stop_flag(), |p: &KeepProgress| {
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
