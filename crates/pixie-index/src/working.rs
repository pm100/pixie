//! Which file each kind of background work is on right now, so the progress screen can say what it is doing when one file takes a
//! while (a big video being hashed, say). Only the file's name is kept, not its folder, and it never leaves this computer.

use std::path::Path;
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Work {
    /// The duplicate check hashing a file.
    Hashing,
    /// The loading pipeline reading a file.
    Reading,
    /// The sync copy.
    Copying,
}

/// A file being worked on: its name, its size, and how long it has been going.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Doing {
    pub name: String,
    pub bytes: u64,
    pub millis: u64,
}

struct Entry {
    id: u64,
    work: Work,
    name: String,
    bytes: u64,
    since: Instant,
}

#[derive(Default)]
struct Inner {
    next_id: u64,
    entries: Vec<Entry>,
}

#[derive(Default)]
pub struct WorkMeter(Mutex<Inner>);

impl WorkMeter {
    /// A file is being started on. The id goes back to `end` when it is done.
    pub fn begin(&self, work: Work, path: &Path, bytes: u64) -> u64 {
        let mut m = self.0.lock().unwrap();
        m.next_id += 1;
        let id = m.next_id;
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        m.entries.push(Entry { id, work, name, bytes, since: Instant::now() });
        id
    }

    pub fn end(&self, id: u64) {
        self.0.lock().unwrap().entries.retain(|e| e.id != id);
    }

    /// Of the files this kind of work is on at the moment (several may be, side by side), the one that has been going longest.
    pub fn slowest(&self, work: Work) -> Option<Doing> {
        let m = self.0.lock().unwrap();
        m.entries.iter().filter(|e| e.work == work).min_by_key(|e| e.since).map(|e| Doing { name: e.name.clone(), bytes: e.bytes, millis: e.since.elapsed().as_millis() as u64 })
    }
}

/// The meter every part of Pixie reports to.
pub fn meter() -> &'static WorkMeter {
    static METER: OnceLock<WorkMeter> = OnceLock::new();
    METER.get_or_init(WorkMeter::default)
}

/// Held while a file is worked on; dropping it says the work is over.
pub struct Busy<'a> {
    meter: &'a WorkMeter,
    id: u64,
}

impl Drop for Busy<'_> {
    fn drop(&mut self) {
        self.meter.end(self.id);
    }
}

/// Say that `path` (of `bytes`) is being worked on until the guard is dropped.
pub fn busy(work: Work, path: &Path, bytes: u64) -> Busy<'static> {
    busy_with(meter(), work, path, bytes)
}

/// `busy` with the meter given (so a test can look at its own).
pub fn busy_with<'a>(meter: &'a WorkMeter, work: Work, path: &Path, bytes: u64) -> Busy<'a> {
    Busy { meter, id: meter.begin(work, path, bytes) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use std::time::Duration;

    #[test]
    fn it_says_which_file_a_kind_of_work_is_on_by_name_only() {
        let m = WorkMeter::default();
        assert_eq!(m.slowest(Work::Hashing), None);
        let id = m.begin(Work::Hashing, Path::new("C:/photos/trips/big.mp4"), 5_800_000_000);
        let d = m.slowest(Work::Hashing).unwrap();
        assert_eq!((d.name.as_str(), d.bytes), ("big.mp4", 5_800_000_000), "the folder is not part of it");
        m.end(id);
        assert_eq!(m.slowest(Work::Hashing), None, "and nothing once it is done");
    }

    #[test]
    fn kinds_of_work_are_kept_apart() {
        let m = WorkMeter::default();
        let _a = m.begin(Work::Reading, Path::new("a.jpg"), 1);
        assert_eq!(m.slowest(Work::Copying), None);
        assert_eq!(m.slowest(Work::Reading).unwrap().name, "a.jpg");
    }

    #[test]
    fn with_several_at_once_it_is_the_one_that_has_been_going_longest() {
        let m = WorkMeter::default();
        let first = m.begin(Work::Reading, Path::new("slow.mp4"), 9);
        std::thread::sleep(Duration::from_millis(30));
        let second = m.begin(Work::Reading, Path::new("quick.jpg"), 1);
        assert_eq!(m.slowest(Work::Reading).unwrap().name, "slow.mp4");
        assert!(m.slowest(Work::Reading).unwrap().millis >= 30);
        m.end(first);
        assert_eq!(m.slowest(Work::Reading).unwrap().name, "quick.jpg");
        m.end(second);
    }

    #[test]
    fn the_guard_says_it_is_done_when_dropped() {
        let m = WorkMeter::default();
        {
            let _g = busy_with(&m, Work::Copying, Path::new("x/y.jpg"), 10);
            assert_eq!(m.slowest(Work::Copying).unwrap().name, "y.jpg");
        }
        assert_eq!(m.slowest(Work::Copying), None);
    }
}
