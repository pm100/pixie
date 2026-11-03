//! Copying the items not yet synced into the sync folder (see docs/superpowers/specs/2026-10-31-master-repository-design.md).
//!
//! Safe by construction: an original is only ever read; a copy is written under a temporary name, read back and checked
//! (size and hash) against the original, and only then put in place (the check reads what the system holds, which is not proof that it
//! has reached a USB drive: nothing is flushed file by file, as that made thousands of photos take hours); a file already there is never overwritten (an
//! identical one counts as done, a different one with the same name makes the new copy take a numbered name). A master
//! that cannot be written (offline, disk full) pauses the run with the reason; the items stay waiting and the next run
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
/// The note kept in the sync folder's root: one line per file Pixie copied in, saying which computer copied it, from where, and when.
/// It is only a record for people (and other computers) reading the folder; what is in the folder is decided by the files themselves.
pub const NOTE_FILE: &str = ".pixie-sync";

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

/// What a copy is doing right now: writing a file, or reading the copy back to check it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Copying,
    Checking,
}

/// How far a run has got: items, and bytes (a file's writing and its check count as half each, so the bytes move steadily through
/// a big file and the speed is the speed of the whole job).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Progress {
    pub done: usize,
    pub total: usize,
    pub bytes_done: u64,
    pub bytes_total: u64,
    pub phase: Phase,
}

/// How often at most a run reports progress from inside a file.
const REPORT_EVERY: std::time::Duration = std::time::Duration::from_millis(250);

/// Lets a report through only once in an interval.
struct Throttle {
    every: std::time::Duration,
    last: Option<std::time::Instant>,
}

impl Throttle {
    fn new(every: std::time::Duration) -> Throttle {
        Throttle { every, last: None }
    }

    fn due(&mut self, now: std::time::Instant) -> bool {
        let due = self.last.is_none_or(|last| now.duration_since(last) >= self.every);
        if due {
            self.last = Some(now);
        }
        due
    }
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
    /// Why the run stopped early (master offline, disk full, stopped by the person); None when it went through.
    pub paused: Option<String>,
    /// The first few things that went wrong, one line each.
    pub problems: Vec<String>,
    /// How long the run took, in seconds, and how many bytes it copied.
    #[serde(default)]
    pub seconds: u64,
    #[serde(default)]
    pub bytes: u64,
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
    /// The person asked to stop, part way through a file.
    Stopped,
}

enum Step {
    /// Copied; the size in bytes.
    Copied(PathBuf, u64),
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

/// Copy `src` into `temp`, returning the hash of what was read. The copy gets `modified` as its time (so the library reads it the
/// same way). Nothing is flushed file by file: waiting for each file to reach a USB drive made a copy of thousands of photos take
/// hours, and the next step reads the file back to check it.
fn stream_copy(src: &Path, temp: &Path, modified: Option<std::time::SystemTime>, on_bytes: &mut dyn FnMut(u64) -> bool) -> std::result::Result<String, Trouble> {
    let mut from = File::open(src).map_err(Trouble::Source)?;
    let mut to = File::create(temp).map_err(|e| Trouble::Master(e, temp.to_path_buf()))?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    let mut written = 0u64;
    loop {
        let n = from.read(&mut buf).map_err(Trouble::Source)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
        to.write_all(&buf[..n]).map_err(|e| Trouble::Master(e, temp.to_path_buf()))?;
        written += n as u64;
        if !on_bytes(written) {
            return Err(Trouble::Stopped);
        }
    }
    if let Some(t) = modified {
        let _ = to.set_modified(t);
    }
    Ok(h.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

/// The hash of the copy read back from the sync folder, telling `on_bytes` how far it has read (and stopping when it says so).
fn checked_hash(path: &Path, on_bytes: &mut dyn FnMut(u64) -> bool) -> std::result::Result<String, Trouble> {
    let here = |e: io::Error| Trouble::Master(e, path.to_path_buf());
    let mut f = File::open(path).map_err(here)?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    let mut read = 0u64;
    loop {
        let n = f.read(&mut buf).map_err(here)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
        read += n as u64;
        if !on_bytes(read) {
            return Err(Trouble::Stopped);
        }
    }
    Ok(h.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

/// Copy one item into `dir`. `on` is told how far each phase has got (bytes), and says false to stop.
fn copy_into(src: &Path, dir: &Path, name: &OsStr, temp: &Path, on: &mut dyn FnMut(Phase, u64) -> bool) -> std::result::Result<Step, Trouble> {
    fs::create_dir_all(dir).map_err(|e| Trouble::Master(e, dir.to_path_buf()))?;
    let meta = fs::metadata(src).map_err(Trouble::Source)?;
    let size = meta.len();
    let (place, n) = find_place(src, size, dir, name, 0)?;
    if let Place::Same(p) = place {
        return Ok(Step::AlreadyThere(p));
    }
    let hash = stream_copy(src, temp, meta.modified().ok(), &mut |n| on(Phase::Copying, n))?;
    let here = |e| Trouble::Master(e, temp.to_path_buf());
    let written = fs::metadata(temp).map_err(here)?.len();
    if written != size || checked_hash(temp, &mut |n| on(Phase::Checking, n))? != hash {
        return Err(Trouble::Mismatch);
    }
    match find_place(src, size, dir, name, n)? {
        (Place::Same(p), _) => Ok(Step::AlreadyThere(p)),
        // The name was free a moment ago. (Renaming onto a name reserved with an empty file is refused by some network
        // drives, which want more rights to replace a file than to create one.)
        (Place::Free(p), _) => {
            fs::rename(temp, &p).map_err(|e| Trouble::Master(e, p.clone()))?;
            Ok(Step::Copied(p, size))
        }
    }
}

fn computer_name() -> String {
    ["COMPUTERNAME", "HOSTNAME"].iter().find_map(|k| std::env::var(k).ok().filter(|v| !v.is_empty())).unwrap_or_else(|| "unknown".into())
}

/// The line about a copy just made, for the note in the sync folder.
fn note_line(master: &Path, source: &Path, dest: &Path) -> String {
    let rel = dest.strip_prefix(master).unwrap_or(dest).to_string_lossy().replace('\\', "/");
    let at = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    serde_json::json!({ "at": at, "computer": computer_name(), "source": source.to_string_lossy(), "dest": rel }).to_string()
}

/// Add the lines of a run to the note in the sync folder, in one go. A note that cannot be written does not matter: the copies are made.
fn write_notes(master: &Path, lines: &[String]) {
    if lines.is_empty() {
        return;
    }
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(master.join(NOTE_FILE)) {
        let mut text = lines.join("\n");
        text.push('\n');
        let _ = f.write_all(text.as_bytes());
    }
}

fn describe(e: &io::Error, path: &Path) -> String {
    if e.kind() == io::ErrorKind::StorageFull {
        "the sync folder's drive is full".into()
    } else {
        format!("cannot write {} ({e})", path.display())
    }
}

/// Copy every item not yet synced into the sync folder. `progress` is told after each item. Stops early, with the reason in
/// the report, when asked to or when the master cannot be written to; items not reached stay waiting.
pub fn run(db: &Db, stop: &AtomicBool, progress: impl FnMut(Progress)) -> Result<Report> {
    run_with(db, stop, progress, crate::cloud::meter(), &crate::cloud::is_online_only, REPORT_EVERY)
}

/// `run` with the cloud meter, the check for online-only files and the interval between reports from inside a file given (so a test
/// can say which files are online-only and see every report). An online-only file is copied like any other: reading it makes the
/// cloud app download it, and the meter times that wait.
pub fn run_with(db: &Db, stop: &AtomicBool, mut progress: impl FnMut(Progress), meter: &crate::cloud::CloudMeter, online_only: &dyn Fn(&Path) -> bool, every: std::time::Duration) -> Result<Report> {
    let started = std::time::Instant::now();
    let mut report = Report::default();
    let mut notes: Vec<String> = Vec::new();
    let master = match db.master_folder()? {
        Some(m) => PathBuf::from(m),
        None => {
            report.paused = Some("there is no sync folder".into());
            return finish(db, report);
        }
    };
    if !master.is_dir() {
        report.paused = Some("the sync folder cannot be reached".into());
        return finish(db, report);
    }
    let items = db.pending_items()?;
    let total = items.len();
    let bytes_total: u64 = items.iter().map(|i| i.size.max(0) as u64).sum();
    let mut bytes_before = 0u64; // the items finished so far
    let mut throttle = Throttle::new(every);
    for (i, item) in items.iter().enumerate() {
        if stop.load(Ordering::Acquire) {
            report.paused = Some("stopped".into());
            break;
        }
        let size = item.size.max(0) as u64;
        progress(Progress { done: i, total, bytes_done: bytes_before, bytes_total, phase: Phase::Copying });
        let src = Path::new(&item.path);
        let name = src.file_name().unwrap_or(OsStr::new("unnamed"));
        let problem = |report: &mut Report, outcome: &str, why: String| -> Result<()> {
            if report.problems.len() < PROBLEMS_SHOWN {
                report.problems.push(format!("{}: {why}", name.to_string_lossy()));
            }
            db.log_copy_problem(item.id, &item.path, outcome, &why)
        };
        let dir = dest_dir(&master, item.taken_at);
        // (the temporary file is in the folder the copy goes to, so only that folder has to be writable)
        let temp = dir.join(format!(".pixie-copy-{}.part", std::process::id()));
        let wait = crate::cloud::watch_with(meter, src, online_only);
        let doing = crate::working::busy(crate::working::Work::Copying, src, size);
        let result = {
            let mut on = |phase: Phase, n: u64| -> bool {
                if stop.load(Ordering::Acquire) {
                    return false;
                }
                if throttle.due(std::time::Instant::now()) {
                    let part = match phase {
                        Phase::Copying => n / 2,
                        Phase::Checking => size / 2 + n / 2,
                    };
                    progress(Progress { done: i, total, bytes_done: (bytes_before + part).min(bytes_total), bytes_total, phase });
                }
                true
            };
            copy_into(src, &dir, name, &temp, &mut on)
        };
        drop(wait);
        drop(doing);
        if !matches!(result, Ok(Step::Copied(..))) {
            let _ = fs::remove_file(&temp); // (after a good copy the temporary name is gone: it was renamed; any other end leaves it behind)
        }
        if !matches!(result, Err(Trouble::Master(..) | Trouble::Stopped)) {
            bytes_before += size;
        }
        match result {
            Ok(Step::Copied(p, size)) => {
                report.bytes += size;
                notes.push(note_line(&master, src, &p));
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
                problem(&mut report, "failed", "the synced file did not match the original and was removed".into())?;
            }
            Err(Trouble::Master(e, at)) => {
                report.paused = Some(describe(&e, &at));
                break;
            }
            Err(Trouble::Stopped) => {
                report.paused = Some("stopped".into());
                break;
            }
        }
    }
    progress(Progress { done: total, total, bytes_done: bytes_before, bytes_total, phase: Phase::Copying });
    write_notes(&master, &notes);
    report.seconds = started.elapsed().as_secs();
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
    /// Items waiting to be copied.
    pub pending: i64,
    pub copied: i64,
    pub running: bool,
    pub last: Option<Report>,
}

pub fn status(db: &Db, job: &CopyJob) -> Result<Status> {
    let (pending, copied) = db.copy_counts()?;
    let last = db.get_setting(LAST_RUN)?.and_then(|s| serde_json::from_str(&s).ok());
    Ok(Status { pending, copied, running: job.is_running(), last })
}

#[cfg(test)]
mod tests {
    use super::*;
    use pixie_core::{PhotoMeta, SHARE_COPIED, SHARE_NONE};
    use std::sync::atomic::AtomicBool;

    fn meta(at: Option<i64>) -> PhotoMeta {
        PhotoMeta { taken_at: at, ..Default::default() }
    }

    /// A library with an inbox that feeds a master folder, both on disk; the items made are waiting, with real files.
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
        /// A file in the inbox and in the library, so waiting to be copied.
        fn waiting(&self, name: &str, bytes: &[u8], taken_at: Option<i64>) -> i64 {
            let p = self.inbox.path().join(name);
            fs::write(&p, bytes).unwrap();
            let id = self.db.upsert_photo(self.folder, &p.to_string_lossy(), bytes.len() as i64, 1, &meta(taken_at)).unwrap();
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
    fn each_copy_is_noted_in_the_sync_folder_with_who_copied_it_from_where() {
        let s = setup();
        s.waiting("a.jpg", b"picture bytes", Some(MARCH_5));
        s.waiting("b.jpg", b"other bytes", Some(MARCH_5));
        s.run();
        let note = fs::read_to_string(s.master_file(NOTE_FILE)).unwrap();
        let lines: Vec<serde_json::Value> = note.lines().map(|l| serde_json::from_str(l).unwrap()).collect();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0]["dest"], "2024/2024-03-05/a.jpg", "the place is given from the sync folder, with forward slashes");
        assert!(lines[0]["source"].as_str().unwrap().ends_with("a.jpg"));
        assert!(!lines[0]["computer"].as_str().unwrap().is_empty());
        assert!(lines[0]["at"].as_u64().unwrap() > 0);
        s.run();
        assert_eq!(fs::read_to_string(s.master_file(NOTE_FILE)).unwrap().lines().count(), 2, "a second run copies nothing, so notes nothing");
    }

    #[test]
    fn a_file_that_is_online_only_is_copied_not_skipped_and_the_wait_is_counted() {
        let s = setup();
        let id = s.waiting("cloud.mp4", b"cloud bytes", Some(MARCH_5));
        let meter = crate::cloud::CloudMeter::default();
        let r = run_with(&s.db, &AtomicBool::new(false), |_| {}, &meter, &|p: &Path| p.ends_with("cloud.mp4"), std::time::Duration::ZERO).unwrap();
        assert_eq!((r.copied, r.failed), (1, 0));
        assert_eq!(fs::read(s.master_file("2024/2024-03-05/cloud.mp4")).unwrap(), b"cloud bytes");
        assert_eq!(s.db.photo_share(id).unwrap(), SHARE_COPIED);
        let c = meter.snapshot();
        assert_eq!((c.files_total, c.files_done, c.bytes_done, c.waiting_files), (1, 1, 11, 0), "the cloud meter saw it arrive");
    }

    #[test]
    fn the_temporary_file_is_removed_when_the_same_file_turns_up_while_copying() {
        let s = setup();
        s.waiting("a.jpg", b"picture bytes", Some(MARCH_5));
        let there = s.master_file("2024/2024-03-05/a.jpg");
        let mut put = false;
        let copying = || fs::read_dir(there.parent().unwrap()).is_ok_and(|d| d.filter_map(|e| e.ok()).any(|e| e.file_name().to_string_lossy().ends_with(".part")));
        // another computer puts the identical file in place after the name was found free and while the copy is being written
        let r = run(&s.db, &AtomicBool::new(false), |_| {
            if !put && copying() {
                put = true;
                fs::write(&there, b"picture bytes").unwrap();
            }
        })
        .unwrap();
        assert!(put, "the test did not catch the copy being written");
        assert_eq!((r.copied, r.already, r.failed), (0, 1, 0));
        let leftovers: Vec<_> = fs::read_dir(there.parent().unwrap()).unwrap().filter_map(|e| e.ok()).map(|e| e.file_name().to_string_lossy().into_owned()).filter(|n| n.ends_with(".part")).collect();
        assert!(leftovers.is_empty(), "a hidden half-name was left in the sync folder: {leftovers:?}");
    }

    #[test]
    fn a_copy_is_filed_by_date_and_checked_and_the_original_is_untouched() {
        let s = setup();
        let id = s.waiting("a.jpg", b"picture bytes", Some(MARCH_5));
        let r = s.run();
        assert_eq!((r.copied, r.already, r.failed), (1, 0, 0));
        assert_eq!(r.paused, None);
        assert_eq!(fs::read(s.master_file("2024/2024-03-05/a.jpg")).unwrap(), b"picture bytes");
        assert_eq!(fs::read(s.inbox.path().join("a.jpg")).unwrap(), b"picture bytes", "the original is still there");
        assert_eq!(s.db.photo_share(id).unwrap(), SHARE_COPIED);
        assert_eq!(s.db.copy_counts().unwrap(), (0, 1));
    }

    #[test]
    fn an_item_with_no_date_goes_under_undated() {
        let s = setup();
        s.waiting("x.jpg", b"x", None);
        s.run();
        assert!(s.master_file("Undated/x.jpg").is_file());
    }

    #[test]
    fn private_items_and_folders_not_synced_from_are_never_copied() {
        let s = setup();
        let private = s.waiting("p.jpg", b"p", Some(MARCH_5));
        s.db.make_private(&[private]).unwrap();
        let other = tempfile::tempdir().unwrap();
        let other_folder = s.db.add_folder(&other.path().to_string_lossy()).unwrap();
        fs::write(other.path().join("o.jpg"), b"o").unwrap();
        let outside = s.db.upsert_photo(other_folder, &other.path().join("o.jpg").to_string_lossy(), 1, 1, &meta(Some(MARCH_5))).unwrap();
        let r = s.run();
        assert_eq!((r.copied, r.already), (0, 0));
        assert!(!s.master_file("2024").exists(), "nothing was written");
        assert_eq!(s.db.photo_share(outside).unwrap(), SHARE_NONE);
    }

    #[test]
    fn an_identical_file_already_there_is_done_and_not_copied_again() {
        let s = setup();
        fs::create_dir_all(s.master_file("2024/2024-03-05")).unwrap();
        fs::write(s.master_file("2024/2024-03-05/a.jpg"), b"same").unwrap();
        let id = s.waiting("a.jpg", b"same", Some(MARCH_5));
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
        s.waiting("a.jpg", b"mine", Some(MARCH_5));
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
        s.waiting("a.jpg", b"one", Some(MARCH_5));
        let p2 = inbox2.path().join("a.jpg");
        fs::write(&p2, b"two").unwrap();
        s.db.upsert_photo(f2, &p2.to_string_lossy(), 3, 1, &meta(Some(MARCH_5))).unwrap();
        let r = s.run();
        assert_eq!(r.copied, 2);
        let mut got = [fs::read(s.master_file("2024/2024-03-05/a.jpg")).unwrap(), fs::read(s.master_file("2024/2024-03-05/a-1.jpg")).unwrap()];
        got.sort();
        assert_eq!(got, [b"one".to_vec(), b"two".to_vec()]);
    }

    #[test]
    fn a_second_run_has_nothing_to_do() {
        let s = setup();
        s.waiting("a.jpg", b"a", Some(MARCH_5));
        s.run();
        let r = s.run();
        assert_eq!((r.copied, r.already, r.failed), (0, 0, 0));
    }

    #[test]
    fn a_missing_original_fails_that_item_only_and_it_is_tried_again_next_time() {
        let s = setup();
        let gone = s.waiting("gone.jpg", b"g", Some(MARCH_5));
        s.waiting("here.jpg", b"h", Some(MARCH_5));
        fs::remove_file(s.inbox.path().join("gone.jpg")).unwrap();
        let r = s.run();
        assert_eq!((r.copied, r.failed), (1, 1));
        assert!(r.problems[0].starts_with("gone.jpg: cannot read the original"), "{:?}", r.problems);
        assert_eq!(s.db.photo_share(gone).unwrap(), SHARE_NONE, "still waiting, to be tried again");
        assert_eq!(s.db.copy_counts().unwrap().0, 1);
    }

    #[test]
    fn a_master_that_is_not_there_pauses_the_run_and_loses_nothing() {
        let s = setup();
        let id = s.waiting("a.jpg", b"a", Some(MARCH_5));
        let path = s.master.path().to_path_buf();
        drop(s.master);
        assert!(!path.exists());
        let r = run(&s.db, &AtomicBool::new(false), |_| {}).unwrap();
        assert_eq!(r.copied, 0);
        assert_eq!(r.paused.as_deref(), Some("the sync folder cannot be reached"));
        assert_eq!(s.db.photo_share(id).unwrap(), SHARE_NONE, "still waiting");
    }

    /// Run with every progress report kept (none held back for time), and the stop flag the caller gives.
    fn run_noting(s: &Setup, stop: &AtomicBool, mut each: impl FnMut(&Progress)) -> Report {
        let meter = crate::cloud::CloudMeter::default();
        run_with(&s.db, stop, |p| each(&p), &meter, &|_: &Path| false, std::time::Duration::ZERO).unwrap()
    }

    const FOUR_MB: usize = 4 << 20;

    #[test]
    fn progress_is_reported_while_one_large_file_is_copied() {
        let s = setup();
        s.waiting("big.bin", &vec![1u8; FOUR_MB], Some(MARCH_5));
        let mut seen: Vec<Progress> = Vec::new();
        run_noting(&s, &AtomicBool::new(false), |p| seen.push(*p));
        assert!(seen.iter().any(|p| p.done == 0 && p.bytes_done > 0 && p.bytes_done < p.bytes_total), "bytes move before the item count does: {seen:?}");
        assert!(seen.windows(2).all(|w| w[0].bytes_done <= w[1].bytes_done), "never goes backwards");
        let last = seen.last().unwrap();
        assert_eq!((last.done, last.total, last.bytes_done, last.bytes_total), (1, 1, FOUR_MB as u64, FOUR_MB as u64));
    }

    #[test]
    fn progress_says_checking_while_the_copy_is_read_back() {
        let s = setup();
        s.waiting("big.bin", &vec![1u8; FOUR_MB], Some(MARCH_5));
        let mut phases: Vec<(Phase, u64)> = Vec::new();
        run_noting(&s, &AtomicBool::new(false), |p| phases.push((p.phase, p.bytes_done)));
        let first_check = phases.iter().position(|(ph, _)| *ph == Phase::Checking).expect("a checking report");
        assert!(phases[..first_check].iter().any(|(ph, n)| *ph == Phase::Copying && *n > 0), "copying came first");
        let last = phases.len() - 1; // (the report that closes the run is not about any one file)
        assert!(phases[first_check..last].iter().all(|(ph, _)| *ph == Phase::Checking), "and then checking, to the end of the file");
        let half = FOUR_MB as u64 / 2;
        assert!(phases[first_check].1 >= half, "the write is the first half of the file's progress and the check the second");
    }

    #[test]
    fn stop_ends_a_copy_in_the_middle_of_a_file_and_leaves_no_partial_file_and_no_copied_mark() {
        let s = setup();
        let id = s.waiting("big.bin", &vec![1u8; FOUR_MB], Some(MARCH_5));
        let stop = AtomicBool::new(false);
        let r = run_noting(&s, &stop, |p| {
            if p.phase == Phase::Copying && p.bytes_done > 0 {
                stop.store(true, Ordering::Release);
            }
        });
        assert_eq!((r.copied, r.paused.as_deref()), (0, Some("stopped")));
        assert!(!s.master_file("2024/2024-03-05/big.bin").exists(), "nothing was put in place");
        let leftovers: Vec<_> = fs::read_dir(s.master_file("2024/2024-03-05")).unwrap().filter_map(|e| e.ok()).map(|e| e.file_name()).collect();
        assert!(leftovers.is_empty(), "no temporary file is left: {leftovers:?}");
        assert_eq!(s.db.photo_share(id).unwrap(), SHARE_NONE, "still waiting, to be tried again");
    }

    #[test]
    fn reports_come_at_most_once_in_the_interval() {
        let t0 = std::time::Instant::now();
        let ms = std::time::Duration::from_millis;
        let mut t = Throttle::new(ms(250));
        assert!(t.due(t0), "the first is always sent");
        assert!(!t.due(t0 + ms(100)));
        assert!(t.due(t0 + ms(250)));
        assert!(!t.due(t0 + ms(300)));
        assert!(t.due(t0 + ms(500)));
    }

    #[test]
    fn a_stop_request_ends_the_run_before_the_next_item() {
        let s = setup();
        s.waiting("a.jpg", b"a", Some(MARCH_5));
        let r = run(&s.db, &AtomicBool::new(true), |_| {}).unwrap();
        assert_eq!(r.copied, 0);
        assert_eq!(r.paused.as_deref(), Some("stopped"));
    }

    #[test]
    fn progress_counts_up_to_the_total_and_the_run_is_remembered() {
        let s = setup();
        s.waiting("a.jpg", b"a", Some(MARCH_5));
        s.waiting("b.jpg", b"b", Some(MARCH_5));
        let mut seen = Vec::new();
        let r = run(&s.db, &AtomicBool::new(false), |p| seen.push((p.done, p.total))).unwrap();
        assert_eq!(seen.last(), Some(&(2, 2)));
        assert_eq!(seen.first(), Some(&(0, 2)));
        let st = status(&s.db, &CopyJob::default()).unwrap();
        assert_eq!((st.pending, st.copied), (0, 2));
        assert_eq!(st.last, Some(r));
    }

    #[test]
    fn the_copy_keeps_the_originals_time_and_no_temporary_file_is_left() {
        let s = setup();
        s.waiting("a.jpg", b"a", Some(MARCH_5));
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
