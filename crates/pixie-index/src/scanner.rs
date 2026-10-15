use crate::db::Db;
use pixie_core::{is_supported_image, FolderId, PixieError, Result};
use pixie_image::meta::read_meta;
use serde::Serialize;
use std::collections::HashSet;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
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
    pub seen: usize,
}

fn mtime_ms(md: &std::fs::Metadata) -> i64 {
    md.modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Incrementally scan a watched folder. Never touches originals.
/// If the root is unreachable, returns `NotFound` and changes nothing.
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
        if !entry.file_type().is_file() || !is_supported_image(entry.path()) {
            continue;
        }
        let Some(path) = entry.path().to_str().map(str::to_owned) else { continue };
        seen.insert(path.clone());

        let Ok(md) = entry.metadata() else {
            stats.unreadable += 1;
            continue;
        };
        let (size, mtime) = (md.len() as i64, mtime_ms(&md));

        match db.find_photo_stat(&path)? {
            Some(s) if s.size == size && s.mtime == mtime && !s.missing => stats.unchanged += 1,
            existing => {
                let mut meta = read_meta(entry.path());
                if meta.width.is_none() {
                    stats.unreadable += 1;
                }
                if meta.taken_at.is_none() {
                    meta.taken_at = Some(mtime / 1000);
                }
                db.upsert_photo(folder_id, &path, size, mtime, &meta)?;
                if existing.is_some() {
                    stats.updated += 1
                } else {
                    stats.added += 1
                }
            }
        }
        if seen.len() % 100 == 0 {
            on_progress(&ScanProgress { folder_id, seen: seen.len() });
        }
    }

    if !root.is_dir() {
        stats.walk_errors += 1; // root vanished during the walk
    }
    // Only mark photos missing after a fully clean walk: an unreadable subfolder
    // or unplugged drive must not make its photos disappear.
    if stats.walk_errors == 0 {
        stats.marked_missing = db.mark_missing_except(folder_id, &seen)?;
    }
    on_progress(&ScanProgress { folder_id, seen: seen.len() });
    Ok(stats)
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
        let mut last = 0;
        scan_folder(&db, f, &AtomicBool::new(false), |p| last = p.seen).unwrap();
        assert_eq!(last, 250);
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
}
