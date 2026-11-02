//! Cloud drives (OneDrive, iCloud Drive) keep some files "online only": the file is listed, but reading it makes the cloud app
//! download it first, which can take minutes for a video. Pixie reads such files like any other and waits; this counts them and times
//! the waiting so the progress screen can say what is going on.

use std::path::Path;
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

/// Windows file attributes that mean the bytes are not on this computer yet: offline, recall on open, recall on data access.
#[cfg(windows)]
fn attributes_mean_online_only(attrs: u32) -> bool {
    const OFFLINE: u32 = 0x1000;
    const RECALL_ON_OPEN: u32 = 0x4_0000;
    const RECALL_ON_DATA_ACCESS: u32 = 0x40_0000;
    attrs & (OFFLINE | RECALL_ON_OPEN | RECALL_ON_DATA_ACCESS) != 0
}

/// A cloud file that is not on this computer yet: reading it would make the cloud app download it.
#[cfg(windows)]
pub fn is_online_only(path: &Path) -> bool {
    use std::os::windows::fs::MetadataExt;
    std::fs::metadata(path).map(|m| attributes_mean_online_only(m.file_attributes())).unwrap_or(false)
}

#[cfg(not(windows))]
pub fn is_online_only(_path: &Path) -> bool {
    false
}

#[derive(Default)]
struct Counts {
    files_total: u64,
    files_done: u64,
    bytes_total: u64,
    bytes_done: u64,
    longest_wait_ms: u64,
    bytes_read: u64,
    next_ticket: u64,
    /// The reads waiting now.
    waiting: Vec<Waiting>,
}

struct Waiting {
    ticket: u64,
    name: String,
    source: &'static str,
    bytes: u64,
    since: Instant,
}

/// A file being waited for right now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WaitingFile {
    pub name: String,
    /// Which cloud it comes from: see `source_of`.
    pub source: &'static str,
    pub bytes: u64,
    pub millis: u64,
}

/// Which cloud drive a file lives in, from its folder: "onedrive", "icloud", "google_drive", or "other" (a drive Pixie has no picture for).
pub fn source_of(path: &Path) -> &'static str {
    for part in path.components() {
        let part = part.as_os_str().to_string_lossy().to_lowercase();
        if part.starts_with("onedrive") {
            return "onedrive";
        }
        if part.starts_with("icloud") {
            return "icloud";
        }
        if part.starts_with("google drive") || part.starts_with("googledrive") {
            return "google_drive";
        }
    }
    "other"
}

/// What has been seen of online-only files since Pixie started, and of the bytes read from disk.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CloudCounts {
    pub files_total: u64,
    pub files_done: u64,
    pub bytes_total: u64,
    pub bytes_done: u64,
    /// The longest any one file took to arrive.
    pub longest_wait_ms: u64,
    /// Files being waited for right now, and how long the longest of them has been.
    pub waiting_files: u64,
    pub waiting_ms: u64,
    /// Bytes read from files by the loading work (online-only or not).
    pub bytes_read: u64,
    /// The files being waited for right now, the biggest first.
    pub waiting_list: Vec<WaitingFile>,
}

#[derive(Default)]
pub struct CloudMeter(Mutex<Counts>);

impl CloudMeter {
    /// An online-only file (its name, not its folder, and its size) is about to be read. The ticket goes back to `arrived` when the
    /// read is over.
    pub fn found(&self, name: &str, bytes: u64) -> u64 {
        self.found_in(name, "other", bytes)
    }

    /// `found`, saying which cloud the file is in (see `source_of`).
    pub fn found_in(&self, name: &str, source: &'static str, bytes: u64) -> u64 {
        let mut c = self.0.lock().unwrap();
        c.files_total += 1;
        c.bytes_total += bytes;
        c.next_ticket += 1;
        let ticket = c.next_ticket;
        c.waiting.push(Waiting { ticket, name: name.to_string(), source, bytes, since: Instant::now() });
        ticket
    }

    /// Forget every count, and every read being waited for (the library they were about has been wiped).
    pub fn reset(&self) {
        let mut c = self.0.lock().unwrap();
        let next_ticket = c.next_ticket; // (so a read that began before cannot be mistaken for a new one)
        *c = Counts { next_ticket, ..Counts::default() };
    }

    /// The read of a file found earlier is over, so it is on this computer now. One that began before a reset is ignored.
    pub fn arrived(&self, ticket: u64, bytes: u64, waited_ms: u64) {
        let mut c = self.0.lock().unwrap();
        if !c.waiting.iter().any(|w| w.ticket == ticket) {
            return;
        }
        c.files_done += 1;
        c.bytes_done += bytes;
        c.longest_wait_ms = c.longest_wait_ms.max(waited_ms);
        c.waiting.retain(|w| w.ticket != ticket);
    }

    pub fn add_bytes_read(&self, n: u64) {
        self.0.lock().unwrap().bytes_read += n;
    }

    pub fn snapshot(&self) -> CloudCounts {
        let c = self.0.lock().unwrap();
        CloudCounts {
            files_total: c.files_total,
            files_done: c.files_done,
            bytes_total: c.bytes_total,
            bytes_done: c.bytes_done,
            longest_wait_ms: c.longest_wait_ms,
            waiting_files: c.waiting.len() as u64,
            waiting_ms: c.waiting.iter().map(|w| w.since.elapsed().as_millis() as u64).max().unwrap_or(0),
            bytes_read: c.bytes_read,
            waiting_list: {
                let mut list: Vec<WaitingFile> = c.waiting.iter().map(|w| WaitingFile { name: w.name.clone(), source: w.source, bytes: w.bytes, millis: w.since.elapsed().as_millis() as u64 }).collect();
                list.sort_by(|a, b| b.bytes.cmp(&a.bytes).then(b.millis.cmp(&a.millis)));
                list
            },
        }
    }
}

/// The meter every part of Pixie reports to.
pub fn meter() -> &'static CloudMeter {
    static METER: OnceLock<CloudMeter> = OnceLock::new();
    METER.get_or_init(CloudMeter::default)
}

/// Held while an online-only file is read; dropping it says the file has arrived.
pub struct Wait<'a> {
    meter: &'a CloudMeter,
    ticket: u64,
    bytes: u64,
    began: Instant,
}

impl Drop for Wait<'_> {
    fn drop(&mut self) {
        self.meter.arrived(self.ticket, self.bytes, self.began.elapsed().as_millis() as u64);
    }
}

/// Call before reading `path`: if it is online-only the wait is counted until the returned guard is dropped.
pub fn watch(path: &Path) -> Option<Wait<'static>> {
    watch_with(meter(), path, is_online_only)
}

/// `watch` with the meter and the check given (so a test can say which files are online-only).
pub fn watch_with<'a>(meter: &'a CloudMeter, path: &Path, online_only: impl Fn(&Path) -> bool) -> Option<Wait<'a>> {
    if !online_only(path) {
        return None;
    }
    let bytes = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    Some(Wait { meter, ticket: meter.found_in(&name, source_of(path), bytes), bytes, began: Instant::now() })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn a_normal_file_is_not_online_only() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("a.jpg");
        std::fs::write(&p, b"bytes").unwrap();
        assert!(!is_online_only(&p));
        assert!(!is_online_only(&dir.path().join("missing.jpg")), "a file that is not there is not waiting for a download either");
    }

    #[test]
    fn the_cloud_is_told_from_the_folder() {
        for (path, source) in [
            ("C:/Users/p/OneDrive/Camera Roll/a.mp4", "onedrive"),
            ("C:/Users/p/OneDrive - Contoso/Pictures/a.jpg", "onedrive"),
            ("C:/Users/p/iCloudPhotos/Photos/a.heic", "icloud"),
            ("C:/Users/p/iCloudDrive/a.heic", "icloud"),
            ("G:/Google Drive/My Drive/a.jpg", "google_drive"),
            ("C:/Users/p/GoogleDrive/a.jpg", "google_drive"),
            ("D:/Dropbox/a.jpg", "other"),
        ] {
            assert_eq!(source_of(Path::new(path)), source, "{path}");
        }
    }

    #[test]
    fn a_waiting_file_remembers_its_cloud() {
        let m = CloudMeter::default();
        m.found_in("a.mp4", "icloud", 10);
        assert_eq!(m.snapshot().waiting_list[0].source, "icloud");
    }

    #[test]
    fn files_are_counted_when_found_and_when_they_arrive() {
        let m = CloudMeter::default();
        let a = m.found("f.mp4", 100);
        let c = m.snapshot();
        assert_eq!((c.files_total, c.files_done, c.bytes_total, c.bytes_done, c.waiting_files), (1, 0, 100, 0, 1));
        m.arrived(a, 100, 4000);
        let c = m.snapshot();
        assert_eq!((c.files_total, c.files_done, c.bytes_done, c.longest_wait_ms, c.waiting_files), (1, 1, 100, 4000, 0));
        let b = m.found("f.mp4", 50);
        m.arrived(b, 50, 9000);
        let d = m.found("f.mp4", 10);
        m.arrived(d, 10, 100);
        let c = m.snapshot();
        assert_eq!((c.files_total, c.files_done, c.bytes_total, c.bytes_done), (3, 3, 160, 160));
        assert_eq!(c.longest_wait_ms, 9000, "the longest wait stays the longest");
    }

    #[test]
    fn a_reset_forgets_the_counts_and_a_read_that_ends_after_it_is_not_counted() {
        let m = CloudMeter::default();
        let old = m.found("f.mp4", 100);
        m.arrived(m.found("f.mp4", 50), 50, 10);
        m.add_bytes_read(7);
        m.reset();
        assert_eq!(m.snapshot(), CloudCounts::default(), "every count is back to nothing, including what is being waited for");
        m.arrived(old, 100, 5000); // a read that began before the reset ends after it
        assert_eq!(m.snapshot(), CloudCounts::default(), "it belongs to the old library");
        let fresh = m.found("f.mp4", 10);
        m.arrived(fresh, 10, 1);
        assert_eq!((m.snapshot().files_total, m.snapshot().files_done), (1, 1));
    }

    #[test]
    fn it_lists_the_files_being_waited_for_largest_first_with_their_names() {
        let m = CloudMeter::default();
        let _small = m.found("small.jpg", 10);
        let huge = m.found("huge.mp4", 3_000_000_000);
        let _mid = m.found("mid.mov", 50_000_000);
        let list = m.snapshot().waiting_list;
        assert_eq!(list.iter().map(|w| w.name.as_str()).collect::<Vec<_>>(), ["huge.mp4", "mid.mov", "small.jpg"]);
        assert_eq!(list[0].bytes, 3_000_000_000);
        m.arrived(huge, 3_000_000_000, 10);
        assert_eq!(m.snapshot().waiting_list.len(), 2, "a file that has arrived is not waited for any more");
        m.reset();
        assert!(m.snapshot().waiting_list.is_empty());
    }

    #[test]
    fn a_read_still_going_shows_how_long_it_has_waited() {
        let m = CloudMeter::default();
        assert_eq!(m.snapshot().waiting_ms, 0);
        let t = m.found("f.mp4", 1);
        std::thread::sleep(Duration::from_millis(40));
        assert!(m.snapshot().waiting_ms >= 40);
        m.arrived(t, 1, 40);
        assert_eq!(m.snapshot().waiting_ms, 0, "nothing is waiting any more");
    }

    #[test]
    fn two_reads_at_once_are_told_apart() {
        let m = CloudMeter::default();
        let old = m.found("f.mp4", 1);
        std::thread::sleep(Duration::from_millis(40));
        let newer = m.found("f.mp4", 1);
        m.arrived(old, 1, 40);
        let c = m.snapshot();
        assert_eq!(c.waiting_files, 1);
        assert!(c.waiting_ms < 40, "the one still waiting is the newer one, not the first");
        m.arrived(newer, 1, 1);
    }

    #[test]
    fn watching_counts_a_cloud_file_until_the_guard_is_dropped_and_ignores_others() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("a.mp4");
        std::fs::write(&p, vec![0u8; 2048]).unwrap();
        let m = CloudMeter::default();
        assert!(watch_with(&m, &p, |_| false).is_none());
        assert_eq!(m.snapshot().files_total, 0);
        let g = watch_with(&m, &p, |_| true);
        assert!(g.is_some());
        let c = m.snapshot();
        assert_eq!((c.files_total, c.files_done, c.bytes_total), (1, 0, 2048));
        assert_eq!(c.waiting_list[0].name, "a.mp4", "it is named by the file, not the folder");
        drop(g);
        let c = m.snapshot();
        assert_eq!((c.files_done, c.bytes_done, c.waiting_files), (1, 2048, 0));
    }

    #[test]
    fn bytes_read_add_up() {
        let m = CloudMeter::default();
        m.add_bytes_read(1000);
        m.add_bytes_read(24);
        assert_eq!(m.snapshot().bytes_read, 1024);
    }

    #[cfg(windows)]
    #[test]
    fn the_windows_attributes_that_mean_not_downloaded_are_recognised() {
        assert!(attributes_mean_online_only(0x1000), "offline");
        assert!(attributes_mean_online_only(0x4_0000), "recall on open");
        assert!(attributes_mean_online_only(0x40_0000), "recall on data access");
        assert!(!attributes_mean_online_only(0x20), "archive only: an ordinary file");
        assert!(!attributes_mean_online_only(0x8_0020), "pinned (kept on this computer) is not online-only");
    }
}
