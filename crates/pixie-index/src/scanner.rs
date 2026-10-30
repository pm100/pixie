use crate::db::Db;
use pixie_core::{media_kind, FolderId, MediaKind, PhotoId, PhotoMeta, PixieError, Result};
use pixie_image::media::read_media_meta;
use serde::Serialize;
use std::collections::HashSet;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, UNIX_EPOCH};
use walkdir::WalkDir;

#[derive(Debug, Default, Clone, PartialEq, Serialize)]
pub struct ScanStats {
    pub added: usize,
    pub updated: usize,
    pub unchanged: usize,
    pub unreadable: usize,
    pub marked_missing: usize,
    /// Directory entries the walk could not read (drive unplugged, permissions...).
    pub walk_errors: usize,
    pub cancelled: bool,
}

/// Allows one scan job at a time.
#[derive(Clone, Default)]
pub struct ScanGate(Arc<AtomicBool>);

pub struct ScanGuard(Arc<AtomicBool>);

impl ScanGate {
    /// A scan job is running right now.
    pub fn is_busy(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }

    pub fn try_start(&self) -> Option<ScanGuard> {
        self.0
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .ok()
            .map(|_| ScanGuard(self.0.clone()))
    }
}

impl Drop for ScanGuard {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ScanProgress {
    pub folder_id: FolderId,
    /// Media files (photos and videos) found so far.
    pub seen: usize,
    /// New or changed files whose metadata has been stored so far.
    pub indexed: usize,
    /// How many files will be read in all: 0 while the folder is still being walked, then fixed.
    pub to_index: usize,
    /// A photo or video stored a moment ago, for a "now reading" view; set on the calls made while reading, at most one
    /// every `LATEST_EVERY` (and always for the first one).
    pub latest: Option<LatestItem>,
}

/// How often at most a progress call names a stored photo.
const LATEST_EVERY: Duration = Duration::from_millis(100);

/// A stored file, with what a client needs to ask for its thumbnail.
#[derive(Debug, Clone, Serialize)]
pub struct LatestItem {
    pub id: PhotoId,
    pub mtime: i64,
    pub size: i64,
    pub name: String,
    /// A video has no thumbnail yet while it is being scanned, so a client shows a placeholder for it.
    pub kind: MediaKind,
}

fn mtime_ms(md: &std::fs::Metadata) -> i64 {
    md.modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// A file a walk found that is new or changed: it still has to be read.
#[derive(Debug, Clone)]
pub struct Candidate {
    pub folder_id: FolderId,
    pub path: String,
    pub size: i64,
    pub mtime: i64,
    /// The file is already in the library (and changed), not new.
    pub is_update: bool,
    pub kind: MediaKind,
}

/// What a walk of a folder found.
pub struct Walk {
    pub folder_id: FolderId,
    pub root: std::path::PathBuf,
    /// New or changed files, still to be read.
    pub candidates: Vec<Candidate>,
    /// Every media file the walk saw, as paths.
    pub seen: HashSet<String>,
    pub stats: ScanStats,
}

/// Threads used to read photo metadata. First-open latency (OneDrive, antivirus,
/// slow disks) dominates, not CPU, so use more threads than cores.
fn metadata_workers() -> usize {
    let cpus = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
    (cpus * 2).clamp(4, 32)
}

/// How much of a folder a scan reads again.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScanMode {
    /// Only new files and files whose size or modified time changed.
    Quick,
    /// Every file, whether or not it looks changed. Items keep their ids.
    Full,
}

/// A quick scan of a watched folder: see [`scan_folder_with`].
pub fn scan_folder(db: &Db, folder_id: FolderId, cancel: &AtomicBool, on_progress: impl FnMut(&ScanProgress)) -> Result<ScanStats> {
    scan_folder_with(db, folder_id, ScanMode::Quick, cancel, on_progress)
}

/// How many files a walk hands over at a time when it hands them over as it goes.
pub const HAND_OVER: usize = 100;
/// The files whose sizes and times are looked up together. Each lookup waits for the drive: a network share (over wifi
/// especially) answers one question at a time about 20 times slower than it answers many at once.
const STAT_CHUNK: usize = 256;
const STAT_AT_ONCE: usize = 32;

/// A file the walk has listed and not yet settled: its path, its kind, and its size and time if the listing gave them.
type Listed = (String, MediaKind, Option<(i64, i64)>);

/// The size and modified time of each path, asked for `STAT_AT_ONCE` at a time; None for one that cannot be read.
fn stat_all(paths: &[&str]) -> Vec<Option<(i64, i64)>> {
    let mut out: Vec<Option<(i64, i64)>> = vec![None; paths.len()];
    let per = paths.len().div_ceil(STAT_AT_ONCE).max(1);
    std::thread::scope(|scope| {
        for (slots, ps) in out.chunks_mut(per).zip(paths.chunks(per)) {
            scope.spawn(move || {
                for (slot, p) in slots.iter_mut().zip(ps) {
                    *slot = std::fs::symlink_metadata(p).ok().map(|md| (md.len() as i64, mtime_ms(&md)));
                }
            });
        }
    });
    out
}

/// Look up the details of the listed files that lack them (together, see `STAT_CHUNK`), then sort each into unchanged
/// (counted) or new or changed (added to `todo`).
fn settle_chunk(db: &Db, folder_id: FolderId, mode: ScanMode, chunk: Vec<Listed>, stats: &mut ScanStats, todo: &mut Vec<Candidate>) -> Result<()> {
    let mut details: Vec<Option<(i64, i64)>> = chunk.iter().map(|c| c.2).collect();
    {
        let lacking: Vec<usize> = (0..chunk.len()).filter(|&i| details[i].is_none()).collect();
        let paths: Vec<&str> = lacking.iter().map(|&i| chunk[i].0.as_str()).collect();
        for (&i, d) in lacking.iter().zip(stat_all(&paths)) {
            details[i] = d;
        }
    }
    for ((path, kind, _), d) in chunk.into_iter().zip(details) {
        let Some((size, mtime)) = d else {
            stats.unreadable += 1;
            continue;
        };
        match db.find_photo_stat(&path)? {
            Some(s) if mode == ScanMode::Quick && s.size == size && s.mtime == mtime && !s.missing && !s.reread => stats.unchanged += 1,
            existing => todo.push(Candidate { folder_id, path, size, mtime, is_update: existing.is_some(), kind }),
        }
    }
    Ok(())
}

/// Walk a watched folder and work out which files are new or changed (cheap): it reads no file and changes nothing. If the
/// root is unreachable, returns `NotFound`. A cancelled walk comes back with `stats.cancelled` set.
pub fn walk_folder(
    db: &Db,
    folder_id: FolderId,
    mode: ScanMode,
    cancel: &AtomicBool,
    on_progress: impl FnMut(&ScanProgress),
) -> Result<Walk> {
    walk_folder_streaming(db, folder_id, mode, cancel, on_progress, None)
}

/// `walk_folder`, but with `hand_over` the new and changed files are passed on `HAND_OVER` at a time while the walk is
/// still going (and the last few at its end), so whoever reads them can begin at once, not when the walk is over. The
/// returned `Walk` then holds none of them.
#[allow(unused_assignments)] // (the last progress count is set and not read)
pub fn walk_folder_streaming(
    db: &Db,
    folder_id: FolderId,
    mode: ScanMode,
    cancel: &AtomicBool,
    mut on_progress: impl FnMut(&ScanProgress),
    mut hand_over: Option<&mut dyn FnMut(Vec<Candidate>)>,
) -> Result<Walk> {
    // Say right away that this folder has begun: opening a slow or sleeping drive can take a while before anything else.
    on_progress(&ScanProgress { folder_id, seen: 0, indexed: 0, to_index: 0, latest: None });
    let folder = db
        .get_folder(folder_id)?
        .ok_or_else(|| PixieError::NotFound(format!("folder {folder_id}")))?;
    let root = Path::new(&folder.path);
    if !root.is_dir() {
        return Err(PixieError::NotFound(folder.path.clone()));
    }

    let mut stats = ScanStats::default();
    let mut seen: HashSet<String> = HashSet::new();
    let mut todo: Vec<Candidate> = Vec::new();
    let mut chunk: Vec<Listed> = Vec::new();
    let mut announced = 0usize;

    // The listed files are settled a chunk at a time; then progress is told and the new files are handed on.
    macro_rules! settle {
        () => {{
            settle_chunk(db, folder_id, mode, std::mem::take(&mut chunk), &mut stats, &mut todo)?;
            if let Some(h) = hand_over.as_mut() {
                if todo.len() >= HAND_OVER {
                    h(std::mem::take(&mut todo));
                }
            }
            if seen.len() / 100 > announced {
                announced = seen.len() / 100;
                on_progress(&ScanProgress { folder_id, seen: seen.len(), indexed: 0, to_index: 0, latest: None });
            }
        }};
    }

    // Phase 1: walk.
    for entry in WalkDir::new(root).follow_links(false).sort_by_file_name() {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => {
                stats.walk_errors += 1;
                continue;
            }
        };
        if cancel.load(Ordering::Relaxed) {
            stats.cancelled = true;
            return Ok(Walk { folder_id, root: root.to_path_buf(), candidates: Vec::new(), seen, stats });
        }
        if !entry.file_type().is_file() {
            continue;
        }
        let Some(kind) = media_kind(entry.path()) else { continue };
        let Some(path) = entry.path().to_str().map(str::to_owned) else { continue };
        seen.insert(path.clone());
        // On Windows the listing already gave the size and time (no further question to the drive); elsewhere they are
        // asked for, many at once, when the chunk is settled.
        #[cfg(windows)]
        let known = entry.metadata().ok().map(|md| (md.len() as i64, mtime_ms(&md)));
        #[cfg(not(windows))]
        let known = None;
        chunk.push((path, kind, known));
        if chunk.len() >= STAT_CHUNK {
            settle!();
        }
    }
    settle!();
    if let Some(h) = hand_over.as_mut() {
        if !todo.is_empty() {
            h(std::mem::take(&mut todo));
        }
    }
    Ok(Walk { folder_id, root: root.to_path_buf(), candidates: todo, seen, stats })
}

/// Close a walk that was not cancelled: files that are no longer there are marked missing, but only after a fully clean walk
/// (an unreadable subfolder or an unplugged drive must not make its photos disappear).
pub fn finish_walk(db: &Db, walk: &mut Walk) -> Result<()> {
    if !walk.root.is_dir() {
        walk.stats.walk_errors += 1; // root vanished during the scan
    }
    if walk.stats.walk_errors == 0 {
        walk.stats.marked_missing = db.mark_missing_except(walk.folder_id, &walk.seen)?;
    }
    Ok(())
}

/// Scan a watched folder. Never touches originals.
/// If the root is unreachable, returns `NotFound` and changes nothing.
///
/// Phase 1 ([`walk_folder`]) walks the folder and works out which files are new or changed (cheap, sequential). Phase 2
/// reads metadata for those files on many threads and writes the results to the database as they arrive. (The app does
/// not use this: it hands the walk's files to the analysis pipeline, which reads each file once for everything.)
pub fn scan_folder_with(
    db: &Db,
    folder_id: FolderId,
    mode: ScanMode,
    cancel: &AtomicBool,
    mut on_progress: impl FnMut(&ScanProgress),
) -> Result<ScanStats> {
    let mut walk = walk_folder(db, folder_id, mode, cancel, &mut on_progress)?;
    if walk.stats.cancelled {
        return Ok(walk.stats);
    }
    let todo = std::mem::take(&mut walk.candidates);
    let mut stats = std::mem::take(&mut walk.stats);
    let seen = &walk.seen;
    // Phase 2: read metadata in parallel, store results as they arrive.
    // Say how much there is to read, so the UI can show real progress instead of the finished walk's count.
    on_progress(&ScanProgress { folder_id, seen: seen.len(), indexed: 0, to_index: todo.len(), latest: None });
    let next = AtomicUsize::new(0);
    let (todo_ref, next_ref) = (&todo, &next);
    let (tx, rx) = std::sync::mpsc::channel::<(usize, PhotoMeta)>();
    let mut db_error: Option<PixieError> = None;
    let mut indexed = 0;
    let mut last_latest: Option<Instant> = None;
    std::thread::scope(|scope| {
        for _ in 0..metadata_workers().min(todo.len().max(1)) {
            let tx = tx.clone();
            scope.spawn(move || loop {
                if cancel.load(Ordering::Relaxed) {
                    break;
                }
                let i = next_ref.fetch_add(1, Ordering::Relaxed);
                let Some(c) = todo_ref.get(i) else { break };
                if tx.send((i, read_media_meta(Path::new(&c.path), c.kind))).is_err() {
                    break;
                }
            });
        }
        drop(tx);
        let rules = db.date_rules();
        for (i, mut meta) in rx {
            if cancel.load(Ordering::Relaxed) {
                break; // what was already read but not saved is found again by the next scan; dropping `rx` stops the workers
            }
            let c = &todo[i];
            if meta.width.is_none() {
                stats.unreadable += 1;
            }
            // The date inside the file is kept as read; the date to show is what the person's rules make of it and of the
            // file's own date.
            let exif_at = meta.taken_at;
            meta.taken_at = rules.effective(exif_at, c.mtime / 1000);
            let id = match db.upsert_photo_dated(folder_id, &c.path, c.size, c.mtime, &meta, exif_at) {
                Ok(id) => id,
                Err(e) => {
                    db_error = Some(e);
                    cancel_workers(next_ref, todo_ref.len());
                    break;
                }
            };
            if c.is_update { stats.updated += 1 } else { stats.added += 1 }
            indexed += 1;
            let due = last_latest.map_or(true, |t| t.elapsed() >= LATEST_EVERY);
            if due || indexed % 50 == 0 {
                let latest = due.then(|| {
                    last_latest = Some(Instant::now());
                    let name = Path::new(&c.path).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                    LatestItem { id, mtime: c.mtime, size: c.size, name, kind: c.kind }
                });
                on_progress(&ScanProgress { folder_id, seen: seen.len(), indexed, to_index: todo_ref.len(), latest });
            }
        }
    });
    if let Some(e) = db_error {
        return Err(e);
    }
    if cancel.load(Ordering::Relaxed) {
        stats.cancelled = true;
        return Ok(stats);
    }

    walk.stats = stats;
    finish_walk(db, &mut walk)?;
    on_progress(&ScanProgress { folder_id, seen: walk.seen.len(), indexed, to_index: todo.len(), latest: None });
    Ok(walk.stats)
}

/// Make the remaining work items unavailable so worker threads stop.
fn cancel_workers(next: &AtomicUsize, len: usize) {
    next.store(len, Ordering::Relaxed);
}

#[cfg(test)]
mod tests {
    use super::*;
    use pixie_testutil::{jpeg, jpeg_with_exif};
    use std::fs;

    fn setup() -> (tempfile::TempDir, Db, FolderId) {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open_in_memory().unwrap();
        let f = db.add_folder(dir.path().to_str().unwrap()).unwrap();
        (dir, db, f)
    }

    fn scan(db: &Db, f: FolderId) -> ScanStats {
        scan_folder(db, f, &AtomicBool::new(false), |_| {}).unwrap()
    }

    #[test]
    fn first_scan_adds_second_scan_changes_nothing() {
        let (dir, db, f) = setup();
        fs::write(dir.path().join("a.jpg"), jpeg(20, 10)).unwrap();
        fs::write(dir.path().join("notes.txt"), "hi").unwrap();
        let s1 = scan(&db, f);
        assert_eq!((s1.added, s1.updated, s1.unchanged), (1, 0, 0));
        let s2 = scan(&db, f);
        assert_eq!((s2.added, s2.updated, s2.unchanged), (0, 0, 1));
        assert_eq!(db.count_photos().unwrap(), 1);
    }

    #[test]
    fn videos_are_indexed_with_kind_duration_and_dimensions() {
        let (dir, db, f) = setup();
        let s = pixie_testutil::Mp4Spec { width: 1080, height: 1920, duration_ms: 4200, rotation: 0, ..Default::default() };
        fs::write(dir.path().join("clip.mp4"), pixie_testutil::mp4(&s)).unwrap();
        fs::write(dir.path().join("a.jpg"), jpeg(8, 8)).unwrap();
        let st = scan(&db, f);
        assert_eq!(st.added, 2);
        let rows = db.list_photos(0, 10).unwrap();
        let v = rows.iter().find(|p| p.path.ends_with("clip.mp4")).unwrap();
        assert_eq!(v.meta.kind, pixie_core::MediaKind::Video);
        assert_eq!(v.meta.duration_ms, Some(4200));
        assert_eq!((v.meta.width, v.meta.height), (Some(1080), Some(1920)));
        assert!(v.meta.taken_at.is_some()); // mtime fallback
        let p = rows.iter().find(|p| p.path.ends_with("a.jpg")).unwrap();
        assert_eq!(p.meta.kind, pixie_core::MediaKind::Photo);
    }

    #[test]
    fn corrupt_empty_videos_are_indexed_and_the_scan_continues() {
        let (dir, db, f) = setup();
        fs::write(dir.path().join("empty.mp4"), []).unwrap();
        fs::write(dir.path().join("junk.mov"), [0xAB; 300]).unwrap();
        fs::write(dir.path().join("._resource.mp4"), [1, 2, 3]).unwrap();
        fs::write(dir.path().join("old.avi"), [1, 2, 3]).unwrap();
        fs::write(dir.path().join("z.jpg"), jpeg(8, 8)).unwrap();
        let st = scan(&db, f);
        assert_eq!(st.added, 3); // empty.mp4, junk.mov, z.jpg
        let q = pixie_core::PhotoQuery { kind: Some(pixie_core::MediaKind::Video), ..Default::default() };
        assert_eq!(db.count_photos_in(&q).unwrap(), 2);
    }

    #[test]
    fn photo_without_exif_falls_back_to_mtime_for_date() {
        let (dir, db, f) = setup();
        fs::write(dir.path().join("shot.png.jpg"), jpeg(20, 10)).unwrap();
        scan(&db, f);
        let p = &db.list_photos(0, 10).unwrap()[0];
        assert_eq!(p.meta.taken_at, Some(p.mtime / 1000));
    }

    #[test]
    fn exif_date_wins_over_mtime() {
        let (dir, db, f) = setup();
        fs::write(dir.path().join("a.jpg"), jpeg_with_exif(20, 10, 1, Some("2020:01:02 03:04:05"))).unwrap();
        scan(&db, f);
        let p = &db.list_photos(0, 10).unwrap()[0];
        assert_eq!(p.meta.taken_at, Some(1_577_934_245));
    }

    fn set_mtime(path: &std::path::Path, secs: u64) {
        fs::File::options().write(true).open(path).unwrap().set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_secs(secs)).unwrap();
    }

    #[test]
    fn the_date_rules_decide_what_an_odd_or_missing_date_becomes() {
        let (dir, db, f) = setup();
        fs::write(dir.path().join("odd.jpg"), jpeg_with_exif(20, 10, 1, Some("1980:01:01 00:01:45"))).unwrap();
        fs::write(dir.path().join("real.jpg"), jpeg_with_exif(20, 10, 1, Some("2020:01:02 03:04:05"))).unwrap();
        fs::write(dir.path().join("none.jpg"), jpeg(20, 10)).unwrap();
        for n in ["odd.jpg", "real.jpg", "none.jpg"] {
            set_mtime(&dir.path().join(n), 1_313_805_178); // the file date: 2011
        }
        scan(&db, f);
        let by_name = |n: &str| db.list_photos(0, 10).unwrap().into_iter().find(|p| p.path.ends_with(n)).unwrap().meta.taken_at;
        assert_eq!((by_name("odd.jpg"), by_name("real.jpg"), by_name("none.jpg")), (Some(1_313_805_178), Some(1_577_934_245), Some(1_313_805_178)));
        // With the rules changed, a scan puts it the other way, and a change of rules needs no scan to apply.
        let rules = pixie_core::DateRules { file_when_odd: false, ..Default::default() };
        db.set_date_rules(&rules).unwrap();
        db.apply_date_rules(&rules).unwrap();
        assert_eq!((by_name("odd.jpg"), by_name("none.jpg")), (None, Some(1_313_805_178)));
    }

    #[test]
    fn a_file_date_that_is_the_same_dead_clock_gives_no_date() {
        let (dir, db, f) = setup();
        let p = dir.path().join("olympus.jpg");
        fs::write(&p, jpeg(20, 10)).unwrap(); // no date inside
        set_mtime(&p, 946_684_800 + 7 * 3600); // 2000-01-01 07:00, as an Olympus camera that was never set stamps its files
        scan(&db, f);
        assert_eq!(db.list_photos(0, 10).unwrap()[0].meta.taken_at, None);
    }

    #[test]
    fn a_streaming_walk_hands_files_over_while_it_goes_and_returns_none() {
        let (dir, db, f) = setup();
        for i in 0..(STAT_CHUNK * 2 + 30) {
            fs::write(dir.path().join(format!("p{i:04}.jpg")), jpeg(8, 8)).unwrap();
        }
        let mut batches: Vec<usize> = Vec::new();
        let mut hand = |b: Vec<Candidate>| batches.push(b.len());
        let walk = walk_folder_streaming(&db, f, ScanMode::Quick, &AtomicBool::new(false), |_| {}, Some(&mut hand)).unwrap();
        assert_eq!(batches.iter().sum::<usize>(), STAT_CHUNK * 2 + 30, "every file was handed over");
        assert!(batches.len() >= 3, "in several batches, while the walk went on: {batches:?}");
        assert!(batches.iter().all(|n| *n >= HAND_OVER || n == batches.last().unwrap()), "{batches:?}");
        assert!(walk.candidates.is_empty(), "nothing is left in the walk");
        assert_eq!(walk.seen.len(), STAT_CHUNK * 2 + 30);
    }

    #[test]
    fn many_files_are_looked_up_in_groups_and_come_out_in_order_with_their_sizes() {
        let (dir, db, f) = setup();
        let n = STAT_CHUNK * 2 + 17;
        for i in 0..n {
            fs::write(dir.path().join(format!("p{i:04}.jpg")), vec![0u8; i + 1]).unwrap();
        }
        let walk = walk_folder(&db, f, ScanMode::Quick, &AtomicBool::new(false), |_| {}).unwrap();
        assert_eq!(walk.candidates.len(), n);
        for (i, c) in walk.candidates.iter().enumerate() {
            assert!(c.path.ends_with(&format!("p{i:04}.jpg")), "in file-name order: {}", c.path);
            assert_eq!(c.size, i as i64 + 1);
        }
    }

    #[test]
    fn stat_all_gives_sizes_and_none_for_what_cannot_be_read() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.bin");
        fs::write(&a, b"12345").unwrap();
        let missing = dir.path().join("nope.bin");
        let paths: Vec<String> = (0..70).map(|i| if i == 33 { missing.to_string_lossy().into_owned() } else { a.to_string_lossy().into_owned() }).collect();
        let refs: Vec<&str> = paths.iter().map(String::as_str).collect();
        let out = stat_all(&refs);
        assert_eq!(out.len(), 70);
        for (i, o) in out.iter().enumerate() {
            if i == 33 {
                assert!(o.is_none());
            } else {
                assert_eq!(o.map(|d| d.0), Some(5));
            }
        }
        assert!(stat_all(&[]).is_empty());
    }

    #[test]
    fn a_walk_lists_new_and_changed_files_and_reads_and_stores_nothing() {
        let (dir, db, f) = setup();
        fs::write(dir.path().join("a.jpg"), jpeg(8, 8)).unwrap();
        fs::write(dir.path().join("b.jpg"), jpeg(8, 8)).unwrap();
        fs::write(dir.path().join("notes.txt"), b"x").unwrap();
        let walk = walk_folder(&db, f, ScanMode::Quick, &AtomicBool::new(false), |_| {}).unwrap();
        let names: Vec<String> = walk.candidates.iter().map(|c| Path::new(&c.path).file_name().unwrap().to_string_lossy().into_owned()).collect();
        assert_eq!(names, vec!["a.jpg", "b.jpg"]);
        assert!(walk.candidates.iter().all(|c| !c.is_update && c.folder_id == f));
        assert_eq!(walk.seen.len(), 2);
        assert_eq!(db.count_photos_in(&Default::default()).unwrap(), 0, "nothing was stored");

        // once a file is stored (as the pipeline will), the next walk lists only what changed
        let a = &walk.candidates[0];
        db.upsert_photo(f, &a.path, a.size, a.mtime, &Default::default()).unwrap();
        fs::write(dir.path().join("b.jpg"), jpeg(9, 9)).unwrap();
        let again = walk_folder(&db, f, ScanMode::Quick, &AtomicBool::new(false), |_| {}).unwrap();
        assert_eq!(again.candidates.len(), 1, "a.jpg is known and unchanged; b.jpg was never stored");
        assert_eq!(again.stats.unchanged, 1);
    }

    #[test]
    fn progress_reports_how_many_files_will_be_read_and_how_many_are_done() {
        let (dir, db, f) = setup();
        for i in 0..120 {
            fs::write(dir.path().join(format!("p{i:03}.jpg")), jpeg(8, 8)).unwrap();
        }
        let mut events = Vec::new();
        scan_folder(&db, f, &AtomicBool::new(false), |p| events.push(p.clone())).unwrap();
        // While walking nothing is known yet; once reading starts the total is announced before any file is done.
        let first_reading = events.iter().position(|p| p.to_index > 0).expect("a reading-phase event");
        assert_eq!(events[first_reading].to_index, 120);
        assert_eq!(events[first_reading].indexed, 0);
        assert!(events[..first_reading].iter().all(|p| p.to_index == 0));
        // Later events keep the total and count up.
        assert!(events[first_reading..].iter().all(|p| p.to_index == 120));
        assert!(events.iter().any(|p| p.indexed == 100)); // reported every 50 files
        assert_eq!(events.last().unwrap().indexed, 120); // and once more when the scan ends
        assert!(events.windows(2).all(|w| w[0].indexed <= w[1].indexed));
    }

    #[test]
    fn a_full_scan_rereads_files_whose_size_and_mtime_are_unchanged() {
        let (dir, db, f) = setup();
        fs::write(dir.path().join("a.jpg"), jpeg_with_exif(20, 10, 1, Some("2020:01:02 03:04:05"))).unwrap();
        scan(&db, f);
        // Make the stored date stale without touching size or mtime (what a changed rule or a bad first read leaves).
        let p = db.list_photos(0, 10).unwrap().remove(0);
        let stale = pixie_core::PhotoMeta { taken_at: Some(5), ..p.meta.clone() };
        db.upsert_photo(f, &p.path, p.size, p.mtime, &stale).unwrap();

        let quick = scan(&db, f);
        assert_eq!((quick.unchanged, quick.updated), (1, 0));
        assert_eq!(db.list_photos(0, 10).unwrap()[0].meta.taken_at, Some(5)); // the quick scan trusts size and mtime

        let full = scan_folder_with(&db, f, ScanMode::Full, &AtomicBool::new(false), |_| {}).unwrap();
        assert_eq!((full.unchanged, full.updated, full.added), (0, 1, 0));
        let p = &db.list_photos(0, 10).unwrap()[0];
        assert_eq!(p.meta.taken_at, Some(1_577_934_245));
        assert_eq!(db.count_photos().unwrap(), 1); // same row, not a second one
    }

    #[test]
    fn corrupt_and_empty_jpgs_are_indexed_and_scan_continues() {
        let (dir, db, f) = setup();
        fs::write(dir.path().join("bad.jpg"), b"junk").unwrap();
        fs::write(dir.path().join("empty.jpg"), b"").unwrap();
        fs::write(dir.path().join("good.jpg"), jpeg(20, 10)).unwrap();
        let s = scan(&db, f);
        assert_eq!(s.added, 3);
        assert_eq!(s.unreadable, 2);
        assert_eq!(db.count_photos().unwrap(), 3);
    }

    #[test]
    fn same_name_in_subfolders_unicode_paths_and_resource_forks() {
        let (dir, db, f) = setup();
        for sub in ["x", "y", "Ferien 2024/Café ☕"] {
            fs::create_dir_all(dir.path().join(sub)).unwrap();
            fs::write(dir.path().join(sub).join("IMG_1.jpg"), jpeg(8, 8)).unwrap();
        }
        fs::write(dir.path().join("x").join("._IMG_1.jpg"), b"appledouble").unwrap();
        let s = scan(&db, f);
        assert_eq!(s.added, 3);
        assert_eq!(db.count_photos().unwrap(), 3);
    }

    #[test]
    fn replaced_file_is_reindexed() {
        let (dir, db, f) = setup();
        let p = dir.path().join("a.jpg");
        fs::write(&p, jpeg(20, 10)).unwrap();
        scan(&db, f);
        fs::write(&p, jpeg(200, 100)).unwrap(); // different size
        let s = scan(&db, f);
        assert_eq!((s.added, s.updated), (0, 1));
        assert_eq!(db.list_photos(0, 1).unwrap()[0].meta.width, Some(200));
    }

    #[test]
    fn deleted_file_is_marked_missing_not_removed() {
        let (dir, db, f) = setup();
        let p = dir.path().join("a.jpg");
        fs::write(&p, jpeg(20, 10)).unwrap();
        fs::write(dir.path().join("b.jpg"), jpeg(20, 10)).unwrap();
        scan(&db, f);
        fs::remove_file(&p).unwrap();
        let s = scan(&db, f);
        assert_eq!(s.marked_missing, 1);
        assert_eq!(db.count_photos().unwrap(), 1);
        // reappears -> restored
        fs::write(&p, jpeg(20, 10)).unwrap();
        scan(&db, f);
        assert_eq!(db.count_photos().unwrap(), 2);
    }

    #[test]
    fn unreachable_root_errors_and_marks_nothing_missing() {
        let (dir, db, f) = setup();
        fs::write(dir.path().join("a.jpg"), jpeg(20, 10)).unwrap();
        scan(&db, f);
        let root = dir.path().to_path_buf();
        drop(dir); // "unplug the drive": directory disappears
        assert!(!root.exists());
        let r = scan_folder(&db, f, &AtomicBool::new(false), |_| {});
        assert!(matches!(r, Err(PixieError::NotFound(_))));
        assert_eq!(db.count_photos().unwrap(), 1); // still listed, not missing
    }

    #[test]
    fn cancel_stops_early_without_marking_missing() {
        let (dir, db, f) = setup();
        fs::write(dir.path().join("a.jpg"), jpeg(20, 10)).unwrap();
        scan(&db, f);
        let s = scan_folder(&db, f, &AtomicBool::new(true), |_| {}).unwrap();
        assert!(s.cancelled);
        assert_eq!(s.marked_missing, 0);
        assert_eq!(db.count_photos().unwrap(), 1);
    }

    #[test]
    fn progress_is_reported_and_final_count_matches() {
        let (dir, db, f) = setup();
        for i in 0..250 {
            fs::write(dir.path().join(format!("{i}.jpg")), jpeg(8, 8)).unwrap();
        }
        let mut last = (0, 0);
        scan_folder(&db, f, &AtomicBool::new(false), |p| last = (p.seen, p.indexed)).unwrap();
        assert_eq!(last, (250, 250), "found and indexed counts must both reach the total");
    }

    #[test]
    fn walk_error_mid_scan_marks_nothing_missing() {
        let (dir, db, f) = setup();
        for i in 0..600 {
            fs::write(dir.path().join(format!("f{i:03}.jpg")), jpeg(8, 8)).unwrap();
        }
        let z = dir.path().join("z");
        fs::create_dir_all(&z).unwrap();
        fs::write(z.join("deep.jpg"), jpeg(8, 8)).unwrap();
        scan(&db, f);
        assert_eq!(db.count_photos().unwrap(), 601);
        // "Unplug" the subfolder part-way through the rescan.
        let zc = z.clone();
        let s = scan_folder(&db, f, &AtomicBool::new(false), |p| {
            if p.seen >= 100 {
                let _ = fs::remove_dir_all(&zc);
            }
        })
        .unwrap();
        assert!(s.walk_errors > 0, "{s:?}");
        assert_eq!(s.marked_missing, 0);
        assert_eq!(db.count_photos().unwrap(), 601);
    }

    #[test]
    fn scan_gate_allows_one_scan_at_a_time() {
        let gate = ScanGate::default();
        let a = gate.try_start().expect("first start");
        assert!(gate.try_start().is_none());
        drop(a);
        assert!(gate.try_start().is_some());
    }

    #[test]
    fn scan_gate_reports_whether_a_scan_is_running() {
        let gate = ScanGate::default();
        assert!(!gate.is_busy());
        let g = gate.try_start().unwrap();
        assert!(gate.is_busy());
        drop(g);
        assert!(!gate.is_busy());
    }

    #[test]
    fn cancel_during_indexing_stops_early_and_marks_nothing_missing() {
        let (dir, db, f) = setup();
        for i in 0..400 {
            fs::write(dir.path().join(format!("{i}.jpg")), jpeg(8, 8)).unwrap();
        }
        let cancel = AtomicBool::new(false);
        let s = scan_folder(&db, f, &cancel, |p| {
            if p.indexed >= 50 {
                cancel.store(true, Ordering::Relaxed);
            }
        })
        .unwrap();
        assert!(s.cancelled);
        assert_eq!(s.marked_missing, 0);
        assert!(db.count_photos().unwrap() < 400);
    }

    #[test]
    fn progress_during_reading_names_a_stored_photo() {
        let (dir, db, f) = setup();
        for i in 0..400 {
            fs::write(dir.path().join(format!("{i}.jpg")), jpeg(8, 8)).unwrap();
        }
        let mut latest = Vec::new();
        scan_folder(&db, f, &AtomicBool::new(false), |p| latest.extend(p.latest.clone())).unwrap();
        assert!(!latest.is_empty(), "the first stored photo is always reported");
        for l in latest {
            let photo = db.get_photo(l.id).unwrap().expect("the named photo is stored");
            assert!(photo.path.ends_with(&l.name), "{} vs {}", photo.path, l.name);
            assert_eq!((photo.mtime, photo.size), (l.mtime, l.size));
        }
    }

    /// The kind of the first item a scan of a folder holding just this file names as the latest.
    fn first_latest_kind(file: &str, bytes: &[u8]) -> MediaKind {
        let (dir, db, f) = setup();
        fs::write(dir.path().join(file), bytes).unwrap();
        let mut kind = None;
        scan_folder(&db, f, &AtomicBool::new(false), |p| {
            if kind.is_none() {
                kind = p.latest.as_ref().map(|l| l.kind);
            }
        })
        .unwrap();
        kind.expect("the first stored file is always named")
    }

    #[test]
    fn progress_says_whether_the_latest_item_is_a_photo_or_a_video() {
        assert_eq!(first_latest_kind("a.jpg", &jpeg(8, 8)), MediaKind::Photo);
        assert_eq!(first_latest_kind("b.mp4", b"junk"), MediaKind::Video); // a video even when it cannot be read
    }

    #[test]
    fn the_first_progress_call_comes_before_any_file_is_read_so_a_slow_folder_shows_life() {
        let (dir, db, f) = setup();
        fs::write(dir.path().join("a.jpg"), jpeg(8, 8)).unwrap();
        let mut first = None;
        scan_folder(&db, f, &AtomicBool::new(false), |p| {
            first.get_or_insert((p.folder_id, p.seen, p.indexed, p.to_index, p.latest.is_some()));
        })
        .unwrap();
        assert_eq!(first, Some((f, 0, 0, 0, false)));
    }

    #[test]
    fn progress_is_throttled() {
        let (dir, db, f) = setup();
        for i in 0..400 {
            fs::write(dir.path().join(format!("{i}.jpg")), jpeg(8, 8)).unwrap();
        }
        let mut with_latest = 0;
        scan_folder(&db, f, &AtomicBool::new(false), |p| with_latest += usize::from(p.latest.is_some())).unwrap();
        assert!(with_latest < 100, "a call per photo would be 400, got {with_latest}");
    }

    /// By hand, read-only: times a scan of a real folder (into memory) to see where the quiet gaps are.
    /// `PIXIE_LIVE_DIR=P:/ cargo test -p pixie-index --lib live_scan_timing -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn live_scan_timing() {
        let dir = std::env::var("PIXIE_LIVE_DIR").expect("set PIXIE_LIVE_DIR");
        let db = Db::open_in_memory().unwrap();
        let f = db.add_folder(&dir).unwrap();
        let cancel = AtomicBool::new(false);
        let start = Instant::now();
        let mut last = Instant::now();
        let mut calls = 0;
        let stats = scan_folder(&db, f, &cancel, |p| {
            calls += 1;
            let gap = last.elapsed().as_secs_f32();
            last = Instant::now();
            if gap > 1.0 || calls <= 3 || calls % 20 == 0 {
                eprintln!("{:>6.1}s (+{:>4.1}s) call {calls}: seen {} indexed {} of {} latest {}", start.elapsed().as_secs_f32(), gap, p.seen, p.indexed, p.to_index, p.latest.as_ref().map_or("-", |l| l.name.as_str()));
            }
            if start.elapsed().as_secs() > 40 {
                cancel.store(true, Ordering::Relaxed);
            }
        })
        .unwrap();
        eprintln!("done after {:.1}s: {} progress calls, {stats:?}", start.elapsed().as_secs_f32(), calls);
    }
}
