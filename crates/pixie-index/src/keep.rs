use crate::copier::{copy_into, dest_dir, Phase, Step, Trouble};
use crate::db::Db;
use crate::source::relative_to;
use pixie_core::{FolderId, PhotoId, PixieError, Result};
use crate::trips::{day_of, day_string};
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

/// How kept photos are filed inside the folder they go to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layout {
    /// `2024/2024-03-05` (the same as Sync).
    YearDate,
    /// `2024/2024-03`.
    YearMonth,
    /// All in the folder itself.
    Flat,
}

impl Layout {
    pub fn parse(name: &str) -> Option<Layout> {
        match name {
            "year-date" => Some(Layout::YearDate),
            "year-month" => Some(Layout::YearMonth),
            "flat" => Some(Layout::Flat),
            _ => None,
        }
    }

    /// The folder under `root` for a photo taken at `taken_at` (`Undated` when it has no date, except in the flat layout).
    pub fn dir(self, root: &Path, taken_at: Option<i64>) -> PathBuf {
        match (self, taken_at) {
            (Layout::Flat, _) => root.to_path_buf(),
            (Layout::YearDate, at) => dest_dir(root, at),
            (Layout::YearMonth, Some(t)) => {
                let day = day_string(day_of(t));
                root.join(&day[..4]).join(&day[..7])
            }
            (Layout::YearMonth, None) => root.join("Undated"),
        }
    }
}

/// How a file was put in the folder.
#[derive(Debug, PartialEq, Eq)]
pub enum Kept {
    /// Copied and checked, to this path; the size in bytes.
    Copied(PathBuf, u64),
    /// An identical file was already there.
    AlreadyThere(PathBuf),
}

/// Copies one file into `dir` the way Sync does: to a temporary name in `dir`, read back and checked (size and hash), then put in place.
/// Never overwrites: an identical file counts as done, a different one with the same name gives the copy a number. The original is only read.
/// `on_bytes` is told how far the copy and then its check have got; setting `stop` ends it with nothing left behind. The error is a sentence.
pub fn copy_file_into(src: &Path, dir: &Path, name: &OsStr, stop: &AtomicBool, mut on_bytes: impl FnMut(u64)) -> std::result::Result<Kept, String> {
    // (the temporary file is in the folder the copy goes to, so only that folder has to be writable)
    let temp = dir.join(format!(".pixie-copy-{}.part", std::process::id()));
    let result = {
        let mut on = |_: Phase, n: u64| -> bool {
            on_bytes(n);
            !stop.load(Ordering::Acquire)
        };
        copy_into(src, dir, name, &temp, &mut on)
    };
    if !matches!(result, Ok(Step::Copied(..))) {
        let _ = fs::remove_file(&temp); // (after a good copy the temporary name is gone: it was renamed)
    }
    match result {
        Ok(Step::Copied(path, size)) => Ok(Kept::Copied(path, size)),
        Ok(Step::AlreadyThere(path)) => Ok(Kept::AlreadyThere(path)),
        Err(Trouble::Source(e)) => Err(format!("cannot read the original ({e})")),
        Err(Trouble::Master(e, at)) => Err(format!("cannot write {} ({e})", at.display())),
        Err(Trouble::Mismatch) => Err("the copy did not match the original and was removed".into()),
        Err(Trouble::Stopped) => Err("stopped".into()),
    }
}

/// How far a Keep has got.
#[derive(Debug, Clone, Copy, Default, serde::Serialize)]
pub struct KeepProgress {
    pub done: usize,
    pub total: usize,
    pub bytes_done: u64,
    pub bytes_total: u64,
}

/// What a Keep did.
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct KeepReport {
    pub batch: i64,
    pub copied: usize,
    pub already: usize,
    pub failed: usize,
    /// The first few problems, as sentences naming the file.
    pub problems: Vec<String>,
    pub bytes: u64,
    pub stopped: bool,
}

/// How many problems a report keeps to show.
const PROBLEMS_SHOWN: usize = 5;

/// Keeps these photos of a source: each is copied safely (see `copy_file_into`) into the library folder `dest_folder`, filed by `layout`, and logged
/// as kept (a batch of the import log), so it is known next time. A photo that cannot be read is a problem and the rest go on. Stopping, or the
/// destination failing, ends the run; what was not reached is still new. The card is only read.
pub fn run_keep(db: &Db, source: FolderId, ids: &[PhotoId], dest_folder: FolderId, layout: Layout, stop: &AtomicBool, mut progress: impl FnMut(&KeepProgress)) -> Result<KeepReport> {
    let src = db.list_sources()?.into_iter().find(|s| s.id == source).ok_or_else(|| PixieError::Invalid("That is not an open source.".into()))?;
    let dest = db.list_folders()?.into_iter().find(|f| f.id == dest_folder).ok_or_else(|| PixieError::Invalid("Choose one of your library folders to keep into.".into()))?;
    let dest_root = PathBuf::from(&dest.path);
    let mut photos = Vec::new();
    for id in ids {
        match db.get_photo(*id)? {
            Some(p) if p.folder_id == source && !p.missing => photos.push(p),
            _ => {}
        }
    }
    let batch = db.start_batch(&src.key, &src.label, dest_folder)?;
    let mut report = KeepReport { batch, ..Default::default() };
    let bytes_total: u64 = photos.iter().map(|p| p.size.max(0) as u64).sum();
    let total = photos.len();
    let mut bytes_before = 0u64;
    let mut last = std::time::Instant::now() - std::time::Duration::from_secs(1);
    for (i, p) in photos.iter().enumerate() {
        if stop.load(Ordering::Acquire) {
            report.stopped = true;
            break;
        }
        progress(&KeepProgress { done: i, total, bytes_done: bytes_before, bytes_total });
        let path = Path::new(&p.path);
        let name = path.file_name().unwrap_or(OsStr::new("unnamed"));
        let dir = layout.dir(&dest_root, p.meta.taken_at);
        let size = p.size.max(0) as u64;
        // (a file's copy and its check count as half each, so the bytes move steadily)
        let result = copy_file_into(path, &dir, name, stop, |n| {
            if last.elapsed() >= std::time::Duration::from_millis(150) {
                last = std::time::Instant::now();
                progress(&KeepProgress { done: i, total, bytes_done: (bytes_before + n / 2).min(bytes_total), bytes_total });
            }
        });
        let rel = relative_to(&src.path, &p.path);
        match result {
            Ok(Kept::Copied(to, copied)) => {
                report.copied += 1;
                report.bytes += copied;
                db.log_kept(batch, &src.key, &rel, p.size, p.mtime, None, &to.to_string_lossy())?;
            }
            Ok(Kept::AlreadyThere(to)) => {
                report.already += 1;
                db.log_kept(batch, &src.key, &rel, p.size, p.mtime, None, &to.to_string_lossy())?;
            }
            Err(e) if e == "stopped" => {
                report.stopped = true;
                break;
            }
            Err(e) => {
                report.failed += 1;
                if report.problems.len() < PROBLEMS_SHOWN {
                    report.problems.push(format!("{}: {e}", name.to_string_lossy()));
                }
            }
        }
        bytes_before += size;
    }
    progress(&KeepProgress { done: total, total, bytes_done: bytes_before, bytes_total });
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};

    // 2024-03-05 12:00:00 UTC
    const MARCH_5: i64 = 1_709_640_000;

    #[test]
    fn a_layout_is_read_from_its_name() {
        assert_eq!(Layout::parse("year-date"), Some(Layout::YearDate));
        assert_eq!(Layout::parse("year-month"), Some(Layout::YearMonth));
        assert_eq!(Layout::parse("flat"), Some(Layout::Flat));
        assert_eq!(Layout::parse("by-camera"), None);
    }

    #[test]
    fn each_layout_puts_a_photo_in_its_folder() {
        let root = Path::new("/lib");
        assert_eq!(Layout::YearDate.dir(root, Some(MARCH_5)), Path::new("/lib/2024/2024-03-05"));
        assert_eq!(Layout::YearMonth.dir(root, Some(MARCH_5)), Path::new("/lib/2024/2024-03"));
        assert_eq!(Layout::Flat.dir(root, Some(MARCH_5)), Path::new("/lib"));
        assert_eq!(Layout::YearDate.dir(root, None), Path::new("/lib/Undated"));
        assert_eq!(Layout::YearMonth.dir(root, None), Path::new("/lib/Undated"));
        assert_eq!(Layout::Flat.dir(root, None), Path::new("/lib"));
    }

    fn file(dir: &Path, name: &str, bytes: &[u8]) -> PathBuf {
        let p = dir.join(name);
        std::fs::write(&p, bytes).unwrap();
        p
    }
    fn names(dir: &Path) -> Vec<String> {
        let mut v: Vec<String> = std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
        v.sort();
        v
    }

    #[test]
    fn a_file_is_copied_checked_and_the_original_is_left_alone() {
        let (from, to) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let src = file(from.path(), "a.jpg", b"picture bytes");
        let dir = to.path().join("2024");
        let mut seen = 0u64;
        let kept = copy_file_into(&src, &dir, OsStr::new("a.jpg"), &AtomicBool::new(false), |n| seen = seen.max(n)).unwrap();
        assert_eq!(kept, Kept::Copied(dir.join("a.jpg"), 13));
        assert_eq!(std::fs::read(dir.join("a.jpg")).unwrap(), b"picture bytes");
        assert_eq!(names(&dir), vec!["a.jpg"], "no temporary file is left");
        assert_eq!(std::fs::read(&src).unwrap(), b"picture bytes");
        assert!(seen > 0, "progress was reported");
    }

    #[test]
    fn the_same_file_again_counts_as_already_there_and_a_different_one_gets_a_number() {
        let (from, to) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let a = file(from.path(), "a.jpg", b"one");
        let stop = AtomicBool::new(false);
        copy_file_into(&a, to.path(), OsStr::new("a.jpg"), &stop, |_| {}).unwrap();
        assert_eq!(copy_file_into(&a, to.path(), OsStr::new("a.jpg"), &stop, |_| {}).unwrap(), Kept::AlreadyThere(to.path().join("a.jpg")));
        let other = file(from.path(), "other.jpg", b"two");
        assert_eq!(copy_file_into(&other, to.path(), OsStr::new("a.jpg"), &stop, |_| {}).unwrap(), Kept::Copied(to.path().join("a-1.jpg"), 3));
        assert_eq!(names(to.path()), vec!["a-1.jpg", "a.jpg"]);
        assert_eq!(std::fs::read(to.path().join("a.jpg")).unwrap(), b"one", "nothing is overwritten");
    }

    #[test]
    fn a_stopped_copy_leaves_nothing_behind() {
        let (from, to) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let src = file(from.path(), "big.jpg", &vec![7u8; 3 << 20]);
        let stop = AtomicBool::new(false);
        let err = copy_file_into(&src, to.path(), OsStr::new("big.jpg"), &stop, |_| stop.store(true, Ordering::Relaxed)).unwrap_err();
        assert!(err.contains("stopped"), "{err}");
        assert!(names(to.path()).is_empty(), "{:?}", names(to.path()));
    }

    #[test]
    fn an_unreadable_original_is_a_plain_message_not_a_panic() {
        let to = tempfile::tempdir().unwrap();
        let err = copy_file_into(Path::new("/no/such/file.jpg"), to.path(), OsStr::new("file.jpg"), &AtomicBool::new(false), |_| {}).unwrap_err();
        assert!(err.contains("cannot read"), "{err}");
        assert!(names(to.path()).is_empty());
    }

    // ---- keeping a source's photos into a library folder ----

    use crate::source::scan_source;

    /// A source (a temp dir with `n` photos, scanned) and an empty library folder to keep them into.
    struct Kit {
        db: Db,
        card: tempfile::TempDir,
        lib: tempfile::TempDir,
        src: i64,
        dest: i64,
    }

    fn kit(n: usize) -> Kit {
        let (card, lib) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        for i in 0..n {
            std::fs::create_dir_all(card.path().join("DCIM")).unwrap();
            std::fs::write(card.path().join("DCIM").join(format!("IMG_{i}.jpg")), pixie_testutil::jpeg(8 + i as u32, 6)).unwrap();
        }
        let db = Db::open_in_memory().unwrap();
        let src = db.add_source(card.path().to_str().unwrap(), "K", "Card").unwrap();
        scan_source(&db, src, &AtomicBool::new(false), |_| {}).unwrap();
        let dest = db.add_folder(lib.path().to_str().unwrap()).unwrap();
        Kit { db, card, lib, src, dest }
    }

    fn keep_all(k: &Kit, stop: &AtomicBool) -> KeepReport {
        let ids = k.db.source_new_ids(k.src).unwrap();
        run_keep(&k.db, k.src, &ids, k.dest, Layout::Flat, stop, |_| {}).unwrap()
    }

    #[test]
    fn keeping_copies_what_was_asked_logs_it_and_leaves_the_card_alone() {
        let k = kit(3);
        assert_eq!(k.db.source_new_ids(k.src).unwrap().len(), 3);
        let report = keep_all(&k, &AtomicBool::new(false));
        assert_eq!((report.copied, report.already, report.failed, report.stopped), (3, 0, 0, false));
        assert!(report.bytes > 0);
        assert_eq!(names(k.lib.path()), vec!["IMG_0.jpg", "IMG_1.jpg", "IMG_2.jpg"]);
        assert_eq!(names(&k.card.path().join("DCIM")).len(), 3, "the card is only read");
        let n: i64 = k.db.conn().query_row("SELECT items FROM import_batch WHERE id = ?1", [report.batch], |r| r.get(0)).unwrap();
        assert_eq!(n, 3);
        assert!(k.db.source_new_ids(k.src).unwrap().is_empty(), "everything kept is known now");
        let again = keep_all(&k, &AtomicBool::new(false));
        assert_eq!((again.copied, again.already), (0, 0), "nothing new, nothing done");
    }

    #[test]
    fn a_file_already_in_the_folder_counts_as_already_there() {
        let k = kit(2);
        std::fs::copy(k.card.path().join("DCIM/IMG_0.jpg"), k.lib.path().join("IMG_0.jpg")).unwrap();
        let report = keep_all(&k, &AtomicBool::new(false));
        assert_eq!((report.copied, report.already), (1, 1));
        assert_eq!(names(k.lib.path()), vec!["IMG_0.jpg", "IMG_1.jpg"]);
    }

    #[test]
    fn only_the_chosen_photos_are_kept_and_files_go_to_the_layouts_folder() {
        let k = kit(3);
        let ids = k.db.source_new_ids(k.src).unwrap();
        let report = run_keep(&k.db, k.src, &ids[..1], k.dest, Layout::YearDate, &AtomicBool::new(false), |_| {}).unwrap();
        assert_eq!(report.copied, 1);
        let p = k.db.get_photo(ids[0]).unwrap().unwrap();
        let expected = Layout::YearDate.dir(k.lib.path(), p.meta.taken_at).join(Path::new(&p.path).file_name().unwrap());
        assert!(expected.is_file(), "{expected:?}");
        assert_eq!(k.db.source_new_ids(k.src).unwrap().len(), 2, "the others are still new");
    }

    #[test]
    fn a_photo_that_went_from_the_card_is_a_problem_and_the_rest_go_on() {
        let k = kit(3);
        let ids = k.db.source_new_ids(k.src).unwrap();
        let gone = k.db.get_photo(ids[1]).unwrap().unwrap().path;
        std::fs::remove_file(&gone).unwrap();
        let report = run_keep(&k.db, k.src, &ids, k.dest, Layout::Flat, &AtomicBool::new(false), |_| {}).unwrap();
        assert_eq!((report.copied, report.failed), (2, 1));
        assert!(report.problems[0].contains("IMG_"), "{:?}", report.problems);
        assert!(!k.db.source_new_ids(k.src).unwrap().is_empty(), "the one that failed is still new, to try again");
    }

    #[test]
    fn stopping_ends_the_run_and_the_rest_stay_new() {
        let k = kit(3);
        let stop = AtomicBool::new(true);
        let report = keep_all(&k, &stop);
        assert!(report.stopped);
        assert_eq!(report.copied, 0);
        assert_eq!(k.db.source_new_ids(k.src).unwrap().len(), 3);
        assert!(names(k.lib.path()).is_empty());
    }

    #[test]
    fn something_that_is_not_a_source_or_a_library_folder_is_refused() {
        let k = kit(1);
        let ids = k.db.source_new_ids(k.src).unwrap();
        assert!(run_keep(&k.db, k.dest, &ids, k.dest, Layout::Flat, &AtomicBool::new(false), |_| {}).is_err(), "the library folder is not a source");
        assert!(run_keep(&k.db, k.src, &ids, k.src, Layout::Flat, &AtomicBool::new(false), |_| {}).is_err(), "nor is a source a place to keep into");
    }
}
