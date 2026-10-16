use crate::db::Db;
use pixie_core::{media_kind, FolderId, MediaKind, PhotoMeta, PixieError, Result};
use pixie_image::media::read_media_meta;
use serde::Serialize;
use std::collections::HashSet;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::UNIX_EPOCH;
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
}

fn mtime_ms(md: &std::fs::Metadata) -> i64 {
    md.modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

struct Candidate {
    path: String,
    size: i64,
    mtime: i64,
    is_update: bool,
    kind: MediaKind,
}

/// Threads used to read photo metadata. First-open latency (OneDrive, antivirus,
/// slow disks) dominates, not CPU, so use more threads than cores.
fn metadata_workers() -> usize {
    let cpus = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
    (cpus * 2).clamp(4, 32)
}

/// Incrementally scan a watched folder. Never touches originals.
/// If the root is unreachable, returns `NotFound` and changes nothing.
///
/// Phase 1 walks the folder and works out which files are new or changed (cheap,
/// sequential). Phase 2 reads metadata for those files on many threads and writes
/// the results to the database as they arrive.
pub fn scan_folder(
    db: &Db,
    folder_id: FolderId,
    cancel: &AtomicBool,
    mut on_progress: impl FnMut(&ScanProgress),
) -> Result<ScanStats> {
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
            return Ok(stats);
        }
        if !entry.file_type().is_file() {
            continue;
        }
        let Some(kind) = media_kind(entry.path()) else { continue };
        let Some(path) = entry.path().to_str().map(str::to_owned) else { continue };
        seen.insert(path.clone());

        let Ok(md) = entry.metadata() else {
            stats.unreadable += 1;
            continue;
        };
        let (size, mtime) = (md.len() as i64, mtime_ms(&md));

        match db.find_photo_stat(&path)? {
            Some(s) if s.size == size && s.mtime == mtime && !s.missing => stats.unchanged += 1,
            existing => todo.push(Candidate { path, size, mtime, is_update: existing.is_some(), kind }),
        }
        if seen.len() % 100 == 0 {
            on_progress(&ScanProgress { folder_id, seen: seen.len(), indexed: 0 });
        }
    }

    // Phase 2: read metadata in parallel, store results as they arrive.
    let next = AtomicUsize::new(0);
    let (todo_ref, next_ref) = (&todo, &next);
    let (tx, rx) = std::sync::mpsc::channel::<(usize, PhotoMeta)>();
    let mut db_error: Option<PixieError> = None;
    let mut indexed = 0;
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
        for (i, mut meta) in rx {
            let c = &todo[i];
            if meta.width.is_none() {
                stats.unreadable += 1;
            }
            if meta.taken_at.is_none() {
                meta.taken_at = Some(c.mtime / 1000);
            }
            if let Err(e) = db.upsert_photo(folder_id, &c.path, c.size, c.mtime, &meta) {
                db_error = Some(e);
                cancel_workers(next_ref, todo_ref.len());
                break;
            }
            if c.is_update { stats.updated += 1 } else { stats.added += 1 }
            indexed += 1;
            if indexed % 50 == 0 {
                on_progress(&ScanProgress { folder_id, seen: seen.len(), indexed });
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

    if !root.is_dir() {
        stats.walk_errors += 1; // root vanished during the scan
    }
    // Only mark photos missing after a fully clean walk: an unreadable subfolder
    // or unplugged drive must not make its photos disappear.
    if stats.walk_errors == 0 {
        stats.marked_missing = db.mark_missing_except(folder_id, &seen)?;
    }
    on_progress(&ScanProgress { folder_id, seen: seen.len(), indexed });
    Ok(stats)
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
        for i in 0..150 {
            fs::write(dir.path().join(format!("f{i:03}.jpg")), jpeg(8, 8)).unwrap();
        }
        let z = dir.path().join("z");
        fs::create_dir_all(&z).unwrap();
        fs::write(z.join("deep.jpg"), jpeg(8, 8)).unwrap();
        scan(&db, f);
        assert_eq!(db.count_photos().unwrap(), 151);
        // "Unplug" the subfolder part-way through the rescan.
        let zc = z.clone();
        let s = scan_folder(&db, f, &AtomicBool::new(false), |p| {
            if p.seen == 100 {
                let _ = fs::remove_dir_all(&zc);
            }
        })
        .unwrap();
        assert!(s.walk_errors > 0, "{s:?}");
        assert_eq!(s.marked_missing, 0);
        assert_eq!(db.count_photos().unwrap(), 151);
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
}
