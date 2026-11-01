//! Copying the approved items into the master folder (see docs/superpowers/specs/2026-10-31-master-repository-design.md).
//!
//! Safe by construction: an original is only ever read; a copy is written under a temporary name, read back and checked
//! (size and hash) against the original, and only then put in place; a file already there is never overwritten (an
//! identical one counts as done, a different one with the same name makes the new copy take a numbered name). A master
//! that cannot be written (offline, disk full) pauses the run with the reason; the items stay approved and the next run
//! carries on.

use crate::db::Db;
use crate::dupes::full_hash;
use crate::trips::{day_of, day_string};
use pixie_core::Result;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::ffi::OsStr;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

/// How many problems a run keeps to show.
const PROBLEMS_SHOWN: usize = 5;
const LAST_RUN: &str = "master_last_run";

/// Who may copy: one run at a time, and a way to ask it to stop.
#[derive(Default)]
pub struct CopyJob {
    running: AtomicBool,
    stop: AtomicBool,
}

impl CopyJob {
    /// Take the job for a run: false while another run has it. Give it back with `finish`.
    pub fn try_start(&self) -> bool {
        if self.running.swap(true, Ordering::AcqRel) {
            return false;
        }
        self.stop.store(false, Ordering::Release);
        true
    }

    /// Set when the running copy is asked to stop; `run` looks at it between items.
    pub fn stop_flag(&self) -> &AtomicBool {
        &self.stop
    }

    pub fn finish(&self) {
        self.running.store(false, Ordering::Release);
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Acquire)
    }

    /// Ask the running copy to stop after the file it is on.
    pub fn stop(&self) {
        self.stop.store(true, Ordering::Release);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Progress {
    pub done: usize,
    pub total: usize,
}

/// What a run did.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Report {
    /// Unix seconds when the run ended.
    pub at: i64,
    pub copied: usize,
    /// Already in the master (the same file under the same name): done without copying.
    pub already: usize,
    pub failed: usize,
    /// Online-only cloud files, left until they are on this computer.
    pub waiting: usize,
    /// Why the run stopped early (master offline, disk full, stopped by the person); None when it went through.
    pub paused: Option<String>,
    /// The first few things that went wrong, one line each.
    pub problems: Vec<String>,
}

/// Where a copy of an item taken at `taken_at` goes: `YYYY/YYYY-MM-DD`, or `Undated`.
pub fn dest_dir(master: &Path, taken_at: Option<i64>) -> PathBuf {
    match taken_at {
        Some(t) => {
            let day = day_string(day_of(t));
            master.join(&day[..4]).join(day)
        }
        None => master.join("Undated"),
    }
}

fn numbered(dir: &Path, name: &OsStr, n: u32) -> PathBuf {
    if n == 0 {
        return dir.join(name);
    }
    let p = Path::new(name);
    let stem = p.file_stem().unwrap_or(name).to_string_lossy();
    match p.extension() {
        Some(ext) => dir.join(format!("{stem}-{n}.{}", ext.to_string_lossy())),
        None => dir.join(format!("{stem}-{n}")),
    }
}

/// Why a file was not copied: the original is the trouble (skip it, try again next run), or the master is (stop).
enum Trouble {
    Source(io::Error),
    /// The master could not be written to or read at this path.
    Master(io::Error, PathBuf),
    Mismatch,
}

enum Step {
    Copied(PathBuf),
    AlreadyThere(PathBuf),
}

enum Place {
    Free(PathBuf),
    Same(PathBuf),
}

/// The first name in `dir` (from the `from`-th) that is free, or that holds this very file.
fn find_place(src: &Path, size: u64, dir: &Path, name: &OsStr, from: u32) -> std::result::Result<(Place, u32), Trouble> {
    let mut ours: Option<String> = None;
    for n in from.. {
        let cand = numbered(dir, name, n);
        match fs::metadata(&cand) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok((Place::Free(cand), n)),
            Err(e) => return Err(Trouble::Master(e, cand)),
            Ok(m) if m.len() == size => {
                let theirs = full_hash(&cand).map_err(|e| Trouble::Master(e, cand.clone()))?;
                if ours.is_none() {
                    ours = Some(full_hash(src).map_err(Trouble::Source)?);
                }
                if ours.as_deref() == Some(theirs.as_str()) {
                    return Ok((Place::Same(cand), n));
                }
            }
            Ok(_) => {}
        }
    }
    unreachable!("a name is found long before the numbers run out")
}

/// Copy `src` into `temp`, returning the hash of what was read.
fn stream_copy(src: &Path, temp: &Path) -> std::result::Result<String, Trouble> {
    let mut from = File::open(src).map_err(Trouble::Source)?;
    let mut to = File::create(temp).map_err(|e| Trouble::Master(e, temp.to_path_buf()))?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = from.read(&mut buf).map_err(Trouble::Source)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
        to.write_all(&buf[..n]).map_err(|e| Trouble::Master(e, temp.to_path_buf()))?;
    }
    to.sync_all().map_err(|e| Trouble::Master(e, temp.to_path_buf()))?;
    Ok(h.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

fn copy_into(src: &Path, dir: &Path, name: &OsStr, temp: &Path) -> std::result::Result<Step, Trouble> {
    fs::create_dir_all(dir).map_err(|e| Trouble::Master(e, dir.to_path_buf()))?;
    let meta = fs::metadata(src).map_err(Trouble::Source)?;
    let size = meta.len();
    let (place, n) = find_place(src, size, dir, name, 0)?;
    if let Place::Same(p) = place {
        return Ok(Step::AlreadyThere(p));
    }
    let hash = stream_copy(src, temp)?;
    let here = |e| Trouble::Master(e, temp.to_path_buf());
    let written = fs::metadata(temp).map_err(here)?.len();
    if written != size || full_hash(temp).map_err(|e| Trouble::Master(e, temp.to_path_buf()))? != hash {
        return Err(Trouble::Mismatch);
    }
    // The copy keeps the original's time, so the library reads it the same way.
    if let Ok(t) = meta.modified() {
        if let Ok(f) = OpenOptions::new().write(true).open(temp) {
            let _ = f.set_modified(t);
        }
    }
    match find_place(src, size, dir, name, n)? {
        (Place::Same(p), _) => Ok(Step::AlreadyThere(p)),
        // The name was free a moment ago. (Renaming onto a name reserved with an empty file is refused by some network
        // drives, which want more rights to replace a file than to create one.)
        (Place::Free(p), _) => {
            fs::rename(temp, &p).map_err(|e| Trouble::Master(e, p.clone()))?;
            Ok(Step::Copied(p))
        }
    }
}

/// A cloud file that is not on this computer yet: reading it would make the cloud app download it.
#[cfg(windows)]
fn online_only(path: &Path) -> bool {
    use std::os::windows::fs::MetadataExt;
    const OFFLINE: u32 = 0x1000;
    const RECALL_ON_OPEN: u32 = 0x4_0000;
    const RECALL_ON_DATA_ACCESS: u32 = 0x40_0000;
    fs::metadata(path).map(|m| m.file_attributes() & (OFFLINE | RECALL_ON_OPEN | RECALL_ON_DATA_ACCESS) != 0).unwrap_or(false)
}

#[cfg(not(windows))]
fn online_only(_path: &Path) -> bool {
    false
}

fn describe(e: &io::Error, path: &Path) -> String {
    if e.kind() == io::ErrorKind::StorageFull {
        "the master folder's drive is full".into()
    } else {
        format!("cannot write {} ({e})", path.display())
    }
}

/// Copy every approved item into the master folder. `progress` is told after each item. Stops early, with the reason in
/// the report, when asked to or when the master cannot be written to; items not reached stay approved.
pub fn run(db: &Db, stop: &AtomicBool, mut progress: impl FnMut(Progress)) -> Result<Report> {
    let mut report = Report::default();
    let master = match db.master_folder()? {
        Some(m) => PathBuf::from(m),
        None => {
            report.paused = Some("there is no master folder".into());
            return finish(db, report);
        }
    };
    if !master.is_dir() {
        report.paused = Some("the master folder cannot be reached".into());
        return finish(db, report);
    }
    let items = db.approved_items()?;
    let total = items.len();
    for (i, item) in items.iter().enumerate() {
        if stop.load(Ordering::Acquire) {
            report.paused = Some("stopped".into());
            break;
        }
        progress(Progress { done: i, total });
        let src = Path::new(&item.path);
        let name = src.file_name().unwrap_or(OsStr::new("unnamed"));
        let problem = |report: &mut Report, outcome: &str, why: String| -> Result<()> {
            if report.problems.len() < PROBLEMS_SHOWN {
                report.problems.push(format!("{}: {why}", name.to_string_lossy()));
            }
            db.log_copy_problem(item.id, &item.path, outcome, &why)
        };
        if online_only(src) {
            report.waiting += 1;
            problem(&mut report, "waiting", "not downloaded to this computer yet".into())?;
            continue;
        }
        let dir = dest_dir(&master, item.taken_at);
        // (the temporary file is in the folder the copy goes to, so only that folder has to be writable)
        let temp = dir.join(format!(".pixie-copy-{}.part", std::process::id()));
        let result = copy_into(src, &dir, name, &temp);
        let _ = fs::remove_file(&temp);
        match result {
            Ok(Step::Copied(p)) => {
                db.mark_copied(item.id, &item.path, &p.to_string_lossy(), "copied")?;
                report.copied += 1;
            }
            Ok(Step::AlreadyThere(p)) => {
                db.mark_copied(item.id, &item.path, &p.to_string_lossy(), "already there")?;
                report.already += 1;
            }
            Err(Trouble::Source(e)) => {
                report.failed += 1;
                problem(&mut report, "failed", format!("cannot read the original ({e})"))?;
            }
            Err(Trouble::Mismatch) => {
                report.failed += 1;
                problem(&mut report, "failed", "the copy did not match the original and was removed".into())?;
            }
            Err(Trouble::Master(e, at)) => {
                report.paused = Some(describe(&e, &at));
                break;
            }
        }
    }
    progress(Progress { done: total, total });
    finish(db, report)
}

fn finish(db: &Db, mut report: Report) -> Result<Report> {
    report.at = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    db.set_setting(LAST_RUN, &serde_json::to_string(&report).unwrap_or_default())?;
    Ok(report)
}

/// What the Settings page shows: what waits, what is done, and the last run.
#[derive(Debug, Clone, Serialize)]
pub struct Status {
    pub approved: i64,
    pub copied: i64,
    pub running: bool,
    pub last: Option<Report>,
}

pub fn status(db: &Db, job: &CopyJob) -> Result<Status> {
    let (approved, copied) = db.copy_counts()?;
    let last = db.get_setting(LAST_RUN)?.and_then(|s| serde_json::from_str(&s).ok());
    Ok(Status { approved, copied, running: job.is_running(), last })
}

#[cfg(test)]
mod tests {
    use super::*;
    use pixie_core::{PhotoMeta, SHARE_APPROVED, SHARE_COPIED};
    use std::sync::atomic::AtomicBool;

    fn meta(at: Option<i64>) -> PhotoMeta {
        PhotoMeta { taken_at: at, ..Default::default() }
    }

    /// A library with an inbox that feeds a master folder, both on disk; the items made are approved, with real files.
    struct Setup {
        db: Db,
        inbox: tempfile::TempDir,
        master: tempfile::TempDir,
        folder: i64,
    }

    fn setup() -> Setup {
        let inbox = tempfile::tempdir().unwrap();
        let master = tempfile::tempdir().unwrap();
        let db = Db::open_in_memory().unwrap();
        let folder = db.add_folder(&inbox.path().to_string_lossy()).unwrap();
        db.set_master_folder(Some(&master.path().to_string_lossy())).unwrap();
        db.set_folder_feeds(folder, true).unwrap();
        Setup { db, inbox, master, folder }
    }

    impl Setup {
        /// A file in the inbox, in the library and approved.
        fn approved(&self, name: &str, bytes: &[u8], taken_at: Option<i64>) -> i64 {
            let p = self.inbox.path().join(name);
            fs::write(&p, bytes).unwrap();
            let id = self.db.upsert_photo(self.folder, &p.to_string_lossy(), bytes.len() as i64, 1, &meta(taken_at)).unwrap();
            assert_eq!(self.db.approve_held(&[id]).unwrap(), 1);
            id
        }

        fn run(&self) -> Report {
            run(&self.db, &AtomicBool::new(false), |_| {}).unwrap()
        }

        fn master_file(&self, rel: &str) -> PathBuf {
            self.master.path().join(rel)
        }
    }

    // 2024-03-05 12:00:00 UTC
    const MARCH_5: i64 = 1_709_640_000;

    #[test]
    fn a_copy_is_filed_by_date_and_checked_and_the_original_is_untouched() {
        let s = setup();
        let id = s.approved("a.jpg", b"picture bytes", Some(MARCH_5));
        let r = s.run();
        assert_eq!((r.copied, r.already, r.failed, r.waiting), (1, 0, 0, 0));
        assert_eq!(r.paused, None);
        assert_eq!(fs::read(s.master_file("2024/2024-03-05/a.jpg")).unwrap(), b"picture bytes");
        assert_eq!(fs::read(s.inbox.path().join("a.jpg")).unwrap(), b"picture bytes", "the original is still there");
        assert_eq!(s.db.photo_share(id).unwrap(), SHARE_COPIED);
        assert_eq!(s.db.copy_counts().unwrap(), (0, 1));
    }

    #[test]
    fn an_item_with_no_date_goes_under_undated() {
        let s = setup();
        s.approved("x.jpg", b"x", None);
        s.run();
        assert!(s.master_file("Undated/x.jpg").is_file());
    }

    #[test]
    fn nothing_is_copied_until_it_is_approved_and_private_items_never_are() {
        let s = setup();
        let held = s.db.upsert_photo(s.folder, &s.inbox.path().join("h.jpg").to_string_lossy(), 1, 1, &meta(Some(MARCH_5))).unwrap();
        let private = s.approved("p.jpg", b"p", Some(MARCH_5));
        s.db.make_private(&[private]).unwrap();
        fs::write(s.inbox.path().join("h.jpg"), b"h").unwrap();
        let r = s.run();
        assert_eq!((r.copied, r.already), (0, 0));
        assert!(!s.master_file("2024").exists(), "nothing was written");
        assert_eq!(s.db.photo_share(held).unwrap(), pixie_core::SHARE_HELD);
    }

    #[test]
    fn an_identical_file_already_there_is_done_and_not_copied_again() {
        let s = setup();
        fs::create_dir_all(s.master_file("2024/2024-03-05")).unwrap();
        fs::write(s.master_file("2024/2024-03-05/a.jpg"), b"same").unwrap();
        let id = s.approved("a.jpg", b"same", Some(MARCH_5));
        let r = s.run();
        assert_eq!((r.copied, r.already), (0, 1));
        assert_eq!(s.db.photo_share(id).unwrap(), SHARE_COPIED);
        assert!(!s.master_file("2024/2024-03-05/a-1.jpg").exists());
    }

    #[test]
    fn a_different_file_with_the_same_name_is_never_overwritten() {
        let s = setup();
        fs::create_dir_all(s.master_file("2024/2024-03-05")).unwrap();
        fs::write(s.master_file("2024/2024-03-05/a.jpg"), b"someone else's").unwrap();
        s.approved("a.jpg", b"mine", Some(MARCH_5));
        let r = s.run();
        assert_eq!(r.copied, 1);
        assert_eq!(fs::read(s.master_file("2024/2024-03-05/a.jpg")).unwrap(), b"someone else's");
        assert_eq!(fs::read(s.master_file("2024/2024-03-05/a-1.jpg")).unwrap(), b"mine");
    }

    #[test]
    fn two_items_with_one_name_and_one_day_both_arrive() {
        let s = setup();
        let inbox2 = tempfile::tempdir().unwrap();
        let f2 = s.db.add_folder(&inbox2.path().to_string_lossy()).unwrap();
        s.db.set_folder_feeds(f2, true).unwrap();
        s.approved("a.jpg", b"one", Some(MARCH_5));
        let p2 = inbox2.path().join("a.jpg");
        fs::write(&p2, b"two").unwrap();
        let id2 = s.db.upsert_photo(f2, &p2.to_string_lossy(), 3, 1, &meta(Some(MARCH_5))).unwrap();
        s.db.approve_held(&[id2]).unwrap();
        let r = s.run();
        assert_eq!(r.copied, 2);
        let mut got = [fs::read(s.master_file("2024/2024-03-05/a.jpg")).unwrap(), fs::read(s.master_file("2024/2024-03-05/a-1.jpg")).unwrap()];
        got.sort();
        assert_eq!(got, [b"one".to_vec(), b"two".to_vec()]);
    }

    #[test]
    fn a_second_run_has_nothing_to_do() {
        let s = setup();
        s.approved("a.jpg", b"a", Some(MARCH_5));
        s.run();
        let r = s.run();
        assert_eq!((r.copied, r.already, r.failed), (0, 0, 0));
    }

    #[test]
    fn a_missing_original_fails_that_item_only_and_it_is_tried_again_next_time() {
        let s = setup();
        let gone = s.approved("gone.jpg", b"g", Some(MARCH_5));
        s.approved("here.jpg", b"h", Some(MARCH_5));
        fs::remove_file(s.inbox.path().join("gone.jpg")).unwrap();
        let r = s.run();
        assert_eq!((r.copied, r.failed), (1, 1));
        assert!(r.problems[0].starts_with("gone.jpg: cannot read the original"), "{:?}", r.problems);
        assert_eq!(s.db.photo_share(gone).unwrap(), SHARE_APPROVED, "still approved, to be tried again");
    }

    #[test]
    fn a_master_that_is_not_there_pauses_the_run_and_loses_nothing() {
        let s = setup();
        let id = s.approved("a.jpg", b"a", Some(MARCH_5));
        let path = s.master.path().to_path_buf();
        drop(s.master);
        assert!(!path.exists());
        let r = run(&s.db, &AtomicBool::new(false), |_| {}).unwrap();
        assert_eq!(r.copied, 0);
        assert_eq!(r.paused.as_deref(), Some("the master folder cannot be reached"));
        assert_eq!(s.db.photo_share(id).unwrap(), SHARE_APPROVED);
    }

    #[test]
    fn a_stop_request_ends_the_run_before_the_next_item() {
        let s = setup();
        s.approved("a.jpg", b"a", Some(MARCH_5));
        let r = run(&s.db, &AtomicBool::new(true), |_| {}).unwrap();
        assert_eq!(r.copied, 0);
        assert_eq!(r.paused.as_deref(), Some("stopped"));
    }

    #[test]
    fn progress_counts_up_to_the_total_and_the_run_is_remembered() {
        let s = setup();
        s.approved("a.jpg", b"a", Some(MARCH_5));
        s.approved("b.jpg", b"b", Some(MARCH_5));
        let mut seen = Vec::new();
        let r = run(&s.db, &AtomicBool::new(false), |p| seen.push((p.done, p.total))).unwrap();
        assert_eq!(seen.last(), Some(&(2, 2)));
        assert_eq!(seen.first(), Some(&(0, 2)));
        let st = status(&s.db, &CopyJob::default()).unwrap();
        assert_eq!((st.approved, st.copied), (0, 2));
        assert_eq!(st.last, Some(r));
    }

    #[test]
    fn the_copy_keeps_the_originals_time_and_no_temporary_file_is_left() {
        let s = setup();
        s.approved("a.jpg", b"a", Some(MARCH_5));
        let old = std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_000_000_000);
        File::options().write(true).open(s.inbox.path().join("a.jpg")).unwrap().set_modified(old).unwrap();
        s.run();
        let copy = s.master_file("2024/2024-03-05/a.jpg");
        assert_eq!(fs::metadata(&copy).unwrap().modified().unwrap(), old);
        let leftovers: Vec<_> = fs::read_dir(copy.parent().unwrap()).unwrap().filter_map(|e| e.ok()).filter(|e| e.file_name().to_string_lossy().ends_with(".part")).collect();
        assert!(leftovers.is_empty());
    }

    #[test]
    fn only_one_run_at_a_time_and_a_stop_asks_the_running_one() {
        let job = CopyJob::default();
        assert!(job.try_start());
        assert!(!job.try_start());
        assert!(job.is_running());
        job.stop();
        assert!(job.stop_flag().load(Ordering::Acquire));
        job.finish();
        assert!(!job.is_running());
        assert!(job.try_start());
        assert!(!job.stop_flag().load(Ordering::Acquire), "a new run starts without the old stop");
    }
}
