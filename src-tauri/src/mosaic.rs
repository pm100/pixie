//! The Photomosaic tool's commands: preparing the photos, the live draft, building the full-size picture, looking at it and saving it.

use crate::collage::say;
use crate::state::AppState;
use base64::Engine;
use pixie_core::{Bounds, MediaKind, SortOrder};
use pixie_image::collage::{page_px, MAX_SIDE_PX};
use pixie_image::decode::decode_scaled;
use pixie_image::mosaic::{self, Layout};
use pixie_image::thumbs::{ensure_thumbnail, ThumbClass};
use pixie_index::mosaic::{TileCandidate, TileSig, TileSource};
use pixie_index::Db;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicUsize, Ordering};
use std::sync::Mutex;
use tauri::http::{Response, StatusCode};
use tauri::Emitter;

/// The window's settings for the tool are kept as one piece of text in the settings, written by the window; the app only stores it.
const KEY: &str = "mosaic";
/// What was built last: its key and size.
const BUILT_KEY: &str = "mosaic_built";
const MAX_DOC_BYTES: usize = 1_000_000;
/// More tiles than this would take longer to choose than anyone wants to wait, and cannot be told apart.
const MAX_TILES: u64 = 250_000;
/// The working copy of the target has this many pixels a side per tile.
const TARGET_PX_PER_TILE: u32 = 8;

/// One run at a time (preparing photos or building), and a way to ask it to stop.
pub struct MosaicJob {
    running: AtomicBool,
    stop: AtomicBool,
    /// What the run is doing and how far it has got, for a window that was closed and opened again while it ran.
    phase: AtomicU8,
    done: AtomicUsize,
    total: AtomicUsize,
    built_dir: PathBuf,
    thumbs_dir: PathBuf,
}

impl MosaicJob {
    pub fn new(built_dir: PathBuf, thumbs_dir: PathBuf) -> MosaicJob {
        MosaicJob { running: AtomicBool::new(false), stop: AtomicBool::new(false), phase: AtomicU8::new(0), done: AtomicUsize::new(0), total: AtomicUsize::new(0), built_dir, thumbs_dir }
    }

    /// Take the job for a run: None while another run has it. The run ends when the guard is dropped.
    fn try_start(&self, phase: Phase) -> Option<Running<'_>> {
        if self.running.swap(true, Ordering::AcqRel) {
            return None;
        }
        self.stop.store(false, Ordering::Release);
        self.done.store(0, Ordering::Release);
        self.total.store(0, Ordering::Release);
        self.phase.store(phase as u8, Ordering::Release);
        Some(Running(self))
    }

    /// How far the run has got.
    fn note(&self, done: usize, total: usize) {
        self.done.store(done, Ordering::Release);
        self.total.store(total, Ordering::Release);
    }

    fn status(&self) -> Status {
        let phase = match self.phase.load(Ordering::Acquire) {
            1 => "index",
            2 => "build",
            _ => "idle",
        };
        Status { phase, done: self.done.load(Ordering::Acquire), total: self.total.load(Ordering::Acquire) }
    }

    fn stopped(&self) -> bool {
        self.stop.load(Ordering::Acquire)
    }

    pub fn built_dir(&self) -> &Path {
        &self.built_dir
    }
}

struct Running<'a>(&'a MosaicJob);

impl Drop for Running<'_> {
    fn drop(&mut self) {
        self.0.phase.store(0, Ordering::Release);
        self.0.running.store(false, Ordering::Release);
    }
}

/// What a run is doing.
#[derive(Clone, Copy)]
enum Phase {
    Index = 1,
    Build = 2,
}

/// What the tool is doing now: `idle`, `index` (preparing photos) or `build`, and how far it has got.
#[derive(Serialize, Debug, PartialEq)]
pub struct Status {
    phase: &'static str,
    done: usize,
    total: usize,
}

/// Where the tiles come from: the whole library, what Browse shows (with its filters, as for Pick all), or the picked photos.
#[derive(Deserialize, Clone)]
pub struct SourceArg {
    /// "library", "browse" (what Browse shows, with the filters below), "picked" or "tag" (the photos with the user's tag `tag`).
    which: String,
    tag: Option<String>,
    order: Option<SortOrder>,
    folder_id: Option<i64>,
    kind: Option<MediaKind>,
    bounds: Option<Bounds>,
    max_id: Option<i64>,
    since: Option<i64>,
    until: Option<i64>,
    search: Option<bool>,
}

#[derive(Deserialize, Clone)]
pub struct DraftArgs {
    source: SourceArg,
    /// The photo to rebuild.
    target: i64,
    focus_x: f32,
    focus_y: f32,
    zoom: f32,
    paper_w_mm: f32,
    paper_h_mm: f32,
    dpi: u32,
    tile_mm: f32,
    /// Photos repeat no closer than this many tiles (the most that is wanted).
    gap: u32,
    /// 0 to 1.
    blend: f32,
}

#[derive(Serialize)]
pub struct IndexSummary {
    /// Photos that can be tiles.
    candidates: usize,
    /// Of those, the ones that could not be read.
    skipped: usize,
}

#[derive(Serialize)]
pub struct DraftResult {
    /// The draft picture, a JPEG.
    jpeg_base64: String,
    cols: u32,
    rows: u32,
    tiles: u64,
    /// Photos used as the pool.
    photos: usize,
    /// Photos that could not be read and are left out.
    skipped: usize,
    /// How far apart repeats can be kept (it may be less than asked when there are few photos).
    gap_used: u32,
    /// Everything the picture depends on: the same key on a built picture means it matches these settings.
    key: String,
}

/// What was built, and the settings it was built from.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct BuiltInfo {
    pub key: String,
    pub width: u32,
    pub height: u32,
    pub dpi: u32,
    pub levels: u32,
    /// When it was built (unix milliseconds): the window puts it in the address of the pictures it asks for, so a new build is never shown
    /// from what the window kept of an older one built from the same settings.
    #[serde(default)]
    pub built_at: i64,
}

/// The mosaic so far, while it is being built.
#[derive(Serialize, Clone)]
struct Preview {
    jpeg_base64: String,
}

#[derive(Serialize, Clone)]
struct Progress {
    done: usize,
    total: usize,
}

fn resolve_source(db: &Db, s: &SourceArg) -> Result<TileSource, String> {
    match s.which.as_str() {
        "library" => Ok(TileSource::Library),
        "picked" => Ok(TileSource::Ids(db.list_picked_ids().map_err(|e| e.user_message())?)),
        "tag" => {
            let name = s.tag.as_deref().ok_or("No tag was chosen.")?;
            Ok(TileSource::Ids(db.list_tagged_ids(name).map_err(|e| e.user_message())?))
        }
        "browse" => {
            let q = crate::query_of(s.order, s.folder_id, s.kind, s.bounds.clone(), s.max_id, s.since, s.until, s.search);
            Ok(TileSource::Ids(db.photo_ids_in(&q).map_err(|e| e.user_message())?))
        }
        other => Err(format!("\"{other}\" is not a place to take tiles from.")),
    }
}

const NO_TILES: &str = "No photos are available to use as tiles.";

/// Everything a draft and a build share: the grid, what the target looks like in each cell, and the pool of tiles.
struct Plan {
    page: (u32, u32),
    layout: Layout,
    /// The pool, in the order of `sigs`.
    usable: Vec<TileCandidate>,
    sigs: Vec<TileSig>,
    /// The average colour of each of the pool, in the same order.
    avgs: Vec<[u8; 3]>,
    /// Photos that can be tiles but have no signature (they could not be read).
    skipped: usize,
    key: String,
}

fn fnv(hash: &mut u64, v: i64) {
    for b in v.to_le_bytes() {
        *hash ^= b as u64;
        *hash = hash.wrapping_mul(0x100_0000_01b3);
    }
}

/// A short text of everything the finished picture depends on.
fn key_of(a: &DraftArgs, target: (i64, i64, i64), page: (u32, u32), usable: &[TileCandidate]) -> String {
    let mut pool = 0xcbf2_9ce4_8422_2325u64;
    for t in usable {
        fnv(&mut pool, t.id);
        fnv(&mut pool, t.mtime);
        fnv(&mut pool, t.size);
    }
    format!(
        "t{}-{}-{}|f{:.4},{:.4}|z{:.3}|p{}x{}@{}|m{:.3}|g{}|b{:.3}|n{}:{:x}",
        target.0, target.1, target.2, a.focus_x, a.focus_y, a.zoom, page.0, page.1, a.dpi, a.tile_mm, a.gap, a.blend, usable.len(), pool
    )
}

fn plan(db: &Db, a: &DraftArgs) -> Result<Plan, String> {
    let page = page_px(a.paper_w_mm, a.paper_h_mm, a.dpi).map_err(say)?;
    let tile = mosaic::tile_px(a.tile_mm, a.dpi);
    let (cols, rows) = mosaic::grid_size(page.0, page.1, tile);
    if cols as u64 * rows as u64 > MAX_TILES {
        return Err(format!("That would be {} tiles; the most is {}. Choose a bigger tile size or a smaller page.", cols as u64 * rows as u64, MAX_TILES));
    }
    let cands = db.tile_candidates(&resolve_source(db, &a.source)?).map_err(|e| e.user_message())?;
    let ids: Vec<i64> = cands.iter().map(|c| c.id).collect();
    let sigs = db.tile_sigs(&ids).map_err(|e| e.user_message())?;
    let have: HashSet<i64> = sigs.iter().map(|s| s.id).collect();
    let total = cands.len();
    let usable: Vec<TileCandidate> = cands.into_iter().filter(|c| have.contains(&c.id)).collect();
    if usable.is_empty() {
        return Err(NO_TILES.into());
    }
    let target = db
        .get_photo(a.target)
        .map_err(|e| e.user_message())?
        .filter(|p| !p.missing && p.meta.kind == MediaKind::Photo)
        .ok_or("The photo to rebuild is no longer in the library.")?;
    let (out_w, out_h) = (cols * TARGET_PX_PER_TILE, rows * TARGET_PX_PER_TILE);
    let zoom = if a.zoom.is_finite() { a.zoom.clamp(1.0, 4.0) } else { 1.0 };
    let edge = ((out_w.max(out_h) as f32 * zoom).ceil() as u32).clamp(64, MAX_SIDE_PX);
    let img = decode_scaled(Path::new(&target.path), edge).map_err(say)?;
    let crop = mosaic::crop_target(&img, out_w, out_h, a.focus_x, a.focus_y, a.zoom);
    let layout = Layout { cols, rows, cell_colours: mosaic::cell_colours(&crop, cols, rows) };
    let key = key_of(a, (target.id, target.mtime, target.size), page, &usable);
    let avgs = sigs.iter().map(|s| s.avg).collect();
    Ok(Plan { page, layout, skipped: total - usable.len(), usable, sigs, avgs, key })
}

/// Make the signatures that are missing for the photos of the source. `progress(done, total)` is called as it goes.
fn index_core(db: &Db, job: &MosaicJob, src: &TileSource, progress: &(dyn Fn(usize, usize) + Sync)) -> Result<IndexSummary, String> {
    let cands = db.tile_candidates(src).map_err(|e| e.user_message())?;
    let todo = db.tiles_without_sig(&cands).map_err(|e| e.user_message())?;
    let total = todo.len();
    let next = AtomicUsize::new(0);
    let done = AtomicUsize::new(0);
    let skipped = AtomicUsize::new(0);
    let made: Mutex<Vec<TileSig>> = Mutex::new(Vec::new());
    let workers = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).saturating_sub(1).clamp(1, 8);
    progress(0, total);
    std::thread::scope(|s| {
        for _ in 0..workers {
            s.spawn(|| loop {
                if job.stopped() {
                    break;
                }
                let i = next.fetch_add(1, Ordering::Relaxed);
                let Some(t) = todo.get(i) else { break };
                let sig = ensure_thumbnail(&job.thumbs_dir, ThumbClass::Grid, t.id, t.mtime, t.size, Path::new(&t.path)).and_then(|thumb| mosaic::signature_of_thumbnail(&thumb));
                match sig {
                    Ok(sig) => {
                        let mut batch = made.lock().unwrap_or_else(|e| e.into_inner());
                        batch.push(TileSig { id: t.id, mtime: t.mtime, size: t.size, avg: sig.avg, tiny: sig.tiny });
                        if batch.len() >= 200 {
                            let _ = db.set_tile_sigs(&batch);
                            batch.clear();
                        }
                    }
                    Err(_) => {
                        skipped.fetch_add(1, Ordering::Relaxed);
                    }
                }
                let d = done.fetch_add(1, Ordering::Relaxed) + 1;
                if d % 25 == 0 || d == total {
                    progress(d, total);
                }
            });
        }
    });
    let rest = made.into_inner().unwrap_or_else(|e| e.into_inner());
    db.set_tile_sigs(&rest).map_err(|e| e.user_message())?;
    if job.stopped() {
        return Err("Cancelled.".into());
    }
    Ok(IndexSummary { candidates: cands.len(), skipped: skipped.load(Ordering::Relaxed) })
}

fn draft_core(db: &Db, a: &DraftArgs) -> Result<DraftResult, String> {
    let p = plan(db, a)?;
    let asg = mosaic::assign(&p.layout.cell_colours, p.layout.cols, &p.avgs, a.gap);
    let tinies: Vec<&[u8]> = p.sigs.iter().map(|s| s.tiny.as_slice()).collect();
    let tile = mosaic::preview_tile_px(p.layout.cols, p.layout.rows);
    let img = mosaic::draft(&p.layout, &asg.picks, &tinies, a.blend, tile);
    let jpeg = mosaic::jpeg_bytes(&img, 85).map_err(say)?;
    Ok(DraftResult {
        jpeg_base64: base64::engine::general_purpose::STANDARD.encode(jpeg),
        cols: p.layout.cols,
        rows: p.layout.rows,
        tiles: p.layout.cols as u64 * p.layout.rows as u64,
        photos: p.usable.len(),
        skipped: p.skipped,
        gap_used: asg.gap_used,
        key: p.key,
    })
}

fn stored_built(db: &Db) -> Option<BuiltInfo> {
    db.get_setting(BUILT_KEY).ok().flatten().and_then(|t| serde_json::from_str(&t).ok())
}

/// The built picture, if there is one still on disk.
fn built_now(db: &Db, job: &MosaicJob) -> Option<BuiltInfo> {
    stored_built(db).filter(|_| job.built_dir.join("full.png").exists())
}

fn build_core(db: &Db, job: &MosaicJob, a: &DraftArgs, progress: &mut dyn FnMut(usize, usize), preview: &mut dyn FnMut(Vec<u8>)) -> Result<BuiltInfo, String> {
    let p = plan(db, a)?;
    let asg = mosaic::assign(&p.layout.cell_colours, p.layout.cols, &p.avgs, a.gap);
    let paths: Vec<PathBuf> = p.usable.iter().map(|t| PathBuf::from(&t.path)).collect();
    // A photo that cannot be read gives its cells to the photo nearest in colour that can.
    let mut failed: HashSet<usize> = HashSet::new();
    let mut spare = |bad: usize| -> Option<usize> {
        failed.insert(bad);
        let avgs = &p.avgs;
        let dist = |i: usize| (0..3).map(|k| (avgs[i][k] as i32 - avgs[bad][k] as i32).pow(2)).sum::<i32>();
        (0..avgs.len()).filter(|i| !failed.contains(i)).min_by_key(|i| dist(*i))
    };
    let img = mosaic::build(
        &p.layout,
        &asg.picks,
        &paths,
        p.page.0,
        p.page.1,
        a.blend,
        &mut mosaic::Hooks {
            progress: &mut |done, total| {
                progress(done as usize, total as usize);
                !job.stopped()
            },
            spare: &mut spare,
            preview: &mut |img| {
                if let Ok(jpeg) = mosaic::jpeg_bytes(img, 80) {
                    preview(jpeg);
                }
            },
        },
    )
    .map_err(say)?;
    let built = mosaic::write_built(&img, &job.built_dir, a.dpi).map_err(say)?;
    let built_at = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0);
    let info = BuiltInfo { key: p.key, width: built.width, height: built.height, dpi: built.dpi, levels: built.levels, built_at };
    db.set_setting(BUILT_KEY, &serde_json::to_string(&info).map_err(|e| e.to_string())?).map_err(|e| e.user_message())?;
    Ok(info)
}

/// The window's settings for the tool, as the text it stored, or null when there is none.
#[tauri::command]
pub fn get_mosaic(state: tauri::State<AppState>) -> Result<Option<String>, String> {
    state.db.get_setting(KEY).map_err(|e| e.user_message())
}

/// Keep the window's settings for the tool (as the text it made of them).
#[tauri::command]
pub fn set_mosaic(state: tauri::State<AppState>, doc: String) -> Result<(), String> {
    store(&state.db, &doc)
}

fn store(db: &Db, doc: &str) -> Result<(), String> {
    if doc.len() > MAX_DOC_BYTES {
        return Err("The mosaic settings are too big to keep.".into());
    }
    db.set_setting(KEY, doc).map_err(|e| e.user_message())
}

/// Make the small picture kept for every photo that can be a tile and has none yet. Sends `mosaic-index` with how far it is.
#[tauri::command]
pub async fn mosaic_index(app: tauri::AppHandle, state: tauri::State<'_, AppState>, source: SourceArg) -> Result<IndexSummary, String> {
    let (db, job) = (state.db.clone(), state.mosaic.clone());
    let src = resolve_source(&db, &source)?;
    tauri::async_runtime::spawn_blocking(move || {
        let _run = job.try_start(Phase::Index).ok_or("A mosaic is already being made.")?;
        index_core(&db, &job, &src, &|done, total| {
            job.note(done, total);
            let _ = app.emit("mosaic-index", Progress { done, total });
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

/// The draft: a quick picture of the mosaic from the small pictures kept for the photos.
#[tauri::command]
pub async fn mosaic_draft(state: tauri::State<'_, AppState>, args: DraftArgs) -> Result<DraftResult, String> {
    let db = state.db.clone();
    tauri::async_runtime::spawn_blocking(move || draft_core(&db, &args)).await.map_err(|e| e.to_string())?
}

/// Build the full-size picture from the original files and keep it, ready to look at and save. Sends `mosaic-build` as it goes.
#[tauri::command]
pub async fn mosaic_build(app: tauri::AppHandle, state: tauri::State<'_, AppState>, args: DraftArgs) -> Result<BuiltInfo, String> {
    let (db, job) = (state.db.clone(), state.mosaic.clone());
    tauri::async_runtime::spawn_blocking(move || {
        let _run = job.try_start(Phase::Build).ok_or("A mosaic is already being made.")?;
        build_core(&db, &job, &args, &mut |done, total| {
            job.note(done, total);
            let _ = app.emit("mosaic-build", Progress { done, total });
        }, &mut |jpeg| {
            let _ = app.emit("mosaic-preview", Preview { jpeg_base64: base64::engine::general_purpose::STANDARD.encode(jpeg) });
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Stop preparing photos or building after the photo in hand. The picture built before stays.
#[tauri::command]
pub fn mosaic_cancel(state: tauri::State<AppState>) {
    state.mosaic.stop.store(true, Ordering::Release);
}

/// What the tool is doing now, for a window that was away while it ran.
#[tauri::command]
pub fn mosaic_status(state: tauri::State<AppState>) -> Status {
    state.mosaic.status()
}

/// Forget the picture built last (a new mosaic starts from nothing). Refused while one is being made.
fn clear_core(db: &Db, job: &MosaicJob) -> Result<(), String> {
    let _run = job.try_start(Phase::Build).ok_or("A mosaic is being made: stop it first.")?;
    if job.built_dir.exists() {
        std::fs::remove_dir_all(&job.built_dir).map_err(|e| e.to_string())?;
    }
    db.set_setting(BUILT_KEY, "").map_err(|e| e.user_message())
}

#[tauri::command]
pub fn mosaic_clear(state: tauri::State<AppState>) -> Result<(), String> {
    clear_core(&state.db, &state.mosaic)
}

/// The picture built last, when it is still there.
#[tauri::command]
pub fn get_mosaic_built(state: tauri::State<AppState>) -> Option<BuiltInfo> {
    built_now(&state.db, &state.mosaic)
}

/// Save the built picture as a PNG or JPEG (by the extension of `dst`).
#[tauri::command]
pub async fn mosaic_save(state: tauri::State<'_, AppState>, dst: String) -> Result<(), String> {
    let (db, job) = (state.db.clone(), state.mosaic.clone());
    tauri::async_runtime::spawn_blocking(move || {
        let built = built_now(&db, &job).ok_or("Build the mosaic first.")?;
        mosaic::save_built(job.built_dir(), Path::new(&dst), built.dpi).map_err(say)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// `/mosaic/{z}/{x}/{y}`: a tile of the built picture for the viewer.
pub fn handle(state: &AppState, path: &str, responder: tauri::UriSchemeResponder) {
    let n: Vec<u32> = path.trim_matches('/').split('/').skip(1).filter_map(|s| s.parse().ok()).collect();
    let bytes = match n.as_slice() {
        [z, x, y] => mosaic::view_tile(state.mosaic.built_dir(), *z, *x, *y),
        _ => None,
    };
    responder.respond(match bytes {
        // Rebuilt in place, so never kept: the address carries the key of the picture too, which is what makes the window ask again.
        Some(bytes) => Response::builder().status(StatusCode::OK).header("Content-Type", "image/jpeg").header("Cache-Control", "no-store").header("Access-Control-Allow-Origin", "*").body(bytes).unwrap(),
        None => Response::builder().status(StatusCode::NOT_FOUND).body(Vec::new()).unwrap(),
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use pixie_core::PhotoMeta;

    fn source() -> SourceArg {
        SourceArg { which: "library".into(), tag: None, order: None, folder_id: None, kind: None, bounds: None, max_id: None, since: None, until: None, search: None }
    }

    fn args(target: i64) -> DraftArgs {
        DraftArgs { source: source(), target, focus_x: 0.5, focus_y: 0.5, zoom: 1.0, paper_w_mm: 100.0, paper_h_mm: 100.0, dpi: 100, tile_mm: 20.0, gap: 2, blend: 0.0 }
    }

    fn colour_files(dir: &Path, n: u8) -> (Db, Vec<i64>) {
        let db = Db::open_in_memory().unwrap();
        let f = db.add_folder(&dir.to_string_lossy()).unwrap();
        let ids = (0..n)
            .map(|i| {
                let p = dir.join(format!("{i}.png"));
                image::RgbImage::from_pixel(60, 40, image::Rgb([i * 40, 255 - i * 40, 90])).save(&p).unwrap();
                let size = std::fs::metadata(&p).unwrap().len() as i64;
                db.upsert_photo(f, &p.to_string_lossy(), size, 1, &PhotoMeta::default()).unwrap()
            })
            .collect();
        (db, ids)
    }

    fn job(dir: &Path) -> MosaicJob {
        MosaicJob::new(dir.join("mosaic"), dir.join("thumbs"))
    }

    #[test]
    fn the_key_changes_when_any_setting_changes() {
        let base = args(1);
        let pool = vec![TileCandidate { id: 1, path: "a".into(), mtime: 1, size: 1 }];
        let key = |a: &DraftArgs, t: (i64, i64, i64), p: (u32, u32), u: &[TileCandidate]| key_of(a, t, p, u);
        let k0 = key(&base, (1, 1, 1), (100, 100), &pool);
        assert_eq!(k0, key(&base, (1, 1, 1), (100, 100), &pool));
        let mut seen = vec![k0.clone()];
        let variants: Vec<DraftArgs> = vec![
            DraftArgs { focus_x: 0.4, ..base.clone() },
            DraftArgs { focus_y: 0.4, ..base.clone() },
            DraftArgs { zoom: 2.0, ..base.clone() },
            DraftArgs { dpi: 200, ..base.clone() },
            DraftArgs { tile_mm: 10.0, ..base.clone() },
            DraftArgs { gap: 3, ..base.clone() },
            DraftArgs { blend: 0.5, ..base.clone() },
        ];
        for v in &variants {
            seen.push(key(v, (1, 1, 1), (100, 100), &pool));
        }
        seen.push(key(&base, (2, 1, 1), (100, 100), &pool));
        seen.push(key(&base, (1, 2, 1), (100, 100), &pool));
        seen.push(key(&base, (1, 1, 2), (100, 100), &pool));
        seen.push(key(&base, (1, 1, 1), (200, 100), &pool));
        seen.push(key(&base, (1, 1, 1), (100, 100), &[]));
        seen.push(key(&base, (1, 1, 1), (100, 100), &[TileCandidate { mtime: 2, ..pool[0].clone() }]));
        let n = seen.len();
        seen.sort();
        seen.dedup();
        assert_eq!(seen.len(), n, "every change gives a different key");
    }

    #[test]
    fn a_stored_document_comes_back_and_a_huge_one_is_refused() {
        let db = Db::open_in_memory().unwrap();
        store(&db, "{\"v\":1}").unwrap();
        assert_eq!(db.get_setting(KEY).unwrap().as_deref(), Some("{\"v\":1}"));
        assert!(store(&db, &"x".repeat(MAX_DOC_BYTES + 1)).is_err());
        assert_eq!(db.get_setting(KEY).unwrap().as_deref(), Some("{\"v\":1}"), "the old one stays");
    }

    #[test]
    fn an_empty_source_says_so_in_words() {
        let db = Db::open_in_memory().unwrap();
        assert_eq!(draft_core(&db, &args(1)).err().as_deref(), Some(NO_TILES));
    }

    #[test]
    fn a_page_over_the_limit_is_refused_with_the_collage_message() {
        let db = Db::open_in_memory().unwrap();
        let big = DraftArgs { paper_w_mm: 5000.0, dpi: 300, ..args(1) };
        let e = draft_core(&db, &big).err().unwrap();
        assert!(e.contains("longest side can be at most 16384"), "{e}");
    }

    #[test]
    fn too_many_tiles_are_refused_in_words() {
        let db = Db::open_in_memory().unwrap();
        let e = draft_core(&db, &DraftArgs { tile_mm: 1.0, paper_w_mm: 1000.0, paper_h_mm: 1000.0, dpi: 300, ..args(1) }).err().unwrap();
        assert!(e.contains("Choose a bigger tile size"), "{e}");
    }

    #[test]
    fn a_second_run_is_refused_while_one_is_going() {
        let dir = tempfile::tempdir().unwrap();
        let j = job(dir.path());
        let first = j.try_start(Phase::Build);
        assert!(first.is_some());
        assert!(j.try_start(Phase::Index).is_none());
        drop(first);
        assert!(j.try_start(Phase::Index).is_some(), "free again once the run ends");
    }

    #[test]
    fn the_status_says_what_is_running_and_how_far() {
        let dir = tempfile::tempdir().unwrap();
        let j = job(dir.path());
        assert_eq!(j.status(), Status { phase: "idle", done: 0, total: 0 });
        let run = j.try_start(Phase::Build).unwrap();
        j.note(3, 40);
        assert_eq!(j.status(), Status { phase: "build", done: 3, total: 40 });
        drop(run);
        assert_eq!(j.status().phase, "idle");
        let _again = j.try_start(Phase::Index).unwrap();
        assert_eq!(j.status(), Status { phase: "index", done: 0, total: 0 }, "a new run starts from nothing");
    }

    #[test]
    fn photos_are_prepared_drafted_built_and_saved() {
        let dir = tempfile::tempdir().unwrap();
        let (db, ids) = colour_files(dir.path(), 6);
        let j = job(dir.path());
        // Nothing is prepared yet: there is nothing to draw tiles from.
        assert_eq!(draft_core(&db, &args(ids[0])).err().as_deref(), Some(NO_TILES));
        let seen = Mutex::new(Vec::new());
        let sum = index_core(&db, &j, &TileSource::Library, &|d, t| seen.lock().unwrap().push((d, t))).unwrap();
        assert_eq!((sum.candidates, sum.skipped), (6, 0));
        assert_eq!(*seen.lock().unwrap().last().unwrap(), (6, 6));
        // Preparing again finds nothing to do.
        let again = Mutex::new(Vec::new());
        index_core(&db, &j, &TileSource::Library, &|d, t| again.lock().unwrap().push((d, t))).unwrap();
        assert_eq!(*again.lock().unwrap(), vec![(0, 0)]);

        let draft = draft_core(&db, &args(ids[2])).unwrap();
        assert_eq!((draft.cols, draft.rows, draft.tiles, draft.photos), (4, 4, 16, 6));
        assert!(!draft.jpeg_base64.is_empty());

        let mut steps = Vec::new();
        let built = build_core(&db, &j, &args(ids[2]), &mut |d, t| steps.push((d, t)), &mut |_| {}).unwrap();
        assert_eq!(built.key, draft.key, "the draft and the build agree on what the picture is");
        assert_eq!((built.width, built.height), (394, 394));
        assert!(steps.last().is_some());
        assert!(built.built_at > 0, "a build says when it was made");
        assert_eq!(built_now(&db, &j), Some(built.clone()));
        let out = dir.path().join("out.png");
        mosaic::save_built(j.built_dir(), &out, 100).unwrap();
        assert!(out.exists());

        // A different setting is a different picture.
        assert_ne!(draft_core(&db, &DraftArgs { blend: 0.5, ..args(ids[2]) }).unwrap().key, built.key);
    }

    #[test]
    fn the_photos_with_a_tag_can_be_the_tiles() {
        let dir = tempfile::tempdir().unwrap();
        let (db, ids) = colour_files(dir.path(), 5);
        db.tag_photos(&ids[1..4], "Holiday").unwrap();
        let tag = SourceArg { which: "tag".into(), tag: Some("Holiday".into()), ..source() };
        let TileSource::Ids(got) = resolve_source(&db, &tag).unwrap() else { panic!("a list of ids") };
        assert_eq!(got.len(), 3);
        assert!(resolve_source(&db, &SourceArg { which: "tag".into(), tag: None, ..source() }).is_err());
    }

    #[test]
    fn a_new_mosaic_forgets_the_built_picture_but_not_while_one_is_being_made() {
        let dir = tempfile::tempdir().unwrap();
        let (db, ids) = colour_files(dir.path(), 6);
        let j = job(dir.path());
        index_core(&db, &j, &TileSource::Library, &|_, _| {}).unwrap();
        build_core(&db, &j, &args(ids[1]), &mut |_, _| {}, &mut |_| {}).unwrap();
        assert!(built_now(&db, &j).is_some());
        let run = j.try_start(Phase::Build).unwrap();
        assert!(clear_core(&db, &j).is_err(), "refused while one is going");
        assert!(built_now(&db, &j).is_some());
        drop(run);
        clear_core(&db, &j).unwrap();
        assert_eq!(built_now(&db, &j), None);
        assert!(!j.built_dir().exists());
        clear_core(&db, &j).unwrap(); // nothing there: fine
    }

    #[test]
    fn a_cancelled_build_leaves_the_earlier_picture() {
        let dir = tempfile::tempdir().unwrap();
        let (db, ids) = colour_files(dir.path(), 6);
        let j = job(dir.path());
        index_core(&db, &j, &TileSource::Library, &|_, _| {}).unwrap();
        let first = build_core(&db, &j, &args(ids[1]), &mut |_, _| {}, &mut |_| {}).unwrap();
        j.stop.store(true, Ordering::Release);
        let e = build_core(&db, &j, &args(ids[3]), &mut |_, _| {}, &mut |_| {}).unwrap_err();
        assert_eq!(e, "Cancelled.");
        assert_eq!(built_now(&db, &j), Some(first));
    }

    #[test]
    fn an_unreadable_photo_is_left_out_and_counted() {
        let dir = tempfile::tempdir().unwrap();
        let (db, ids) = colour_files(dir.path(), 4);
        std::fs::write(dir.path().join("1.png"), b"not a picture").unwrap();
        let j = job(dir.path());
        let sum = index_core(&db, &j, &TileSource::Library, &|_, _| {}).unwrap();
        assert_eq!((sum.candidates, sum.skipped), (4, 1));
        let d = draft_core(&db, &args(ids[0])).unwrap();
        assert_eq!((d.photos, d.skipped), (3, 1));
    }

    #[test]
    fn a_video_cannot_be_the_target() {
        let dir = tempfile::tempdir().unwrap();
        let (db, _) = colour_files(dir.path(), 3);
        let j = job(dir.path());
        index_core(&db, &j, &TileSource::Library, &|_, _| {}).unwrap();
        let f = db.add_folder("/v").unwrap();
        let video = db.upsert_photo(f, "/v/a.mp4", 1, 1, &PhotoMeta { kind: MediaKind::Video, ..Default::default() }).unwrap();
        assert_eq!(draft_core(&db, &args(video)).err().as_deref(), Some("The photo to rebuild is no longer in the library."));
    }
}
