//! Keeps the library up to date without being asked. A folder on this computer is watched: the operating system says when
//! a file appears, changes or goes, and once the folder has been quiet for a moment it is scanned. A network folder cannot
//! be watched reliably, so (when the user wants it) it is scanned again every so often instead.
//!
//! This only decides *when* to ask for a scan; the scan itself is the app's usual one (a running scan takes the folder into
//! its queue, otherwise one starts).
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use pixie_core::{media_kind, FolderId};
use pixie_index::Db;
use std::collections::HashMap;
use std::path::Path;
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// Asks for a scan of these folders.
pub type ScanRequest = Arc<dyn Fn(Vec<FolderId>) + Send + Sync>;

/// How long the service waits on things, in the app and (shorter) in tests.
#[derive(Clone, Copy)]
pub struct Timing {
    /// A changed folder is scanned once nothing in it has changed for this long (a copy that is still going keeps adding).
    pub quiet: Duration,
    /// How often the service looks at the clock.
    pub tick: Duration,
    /// How often the list of folders is read again, to start or stop watching as folders are added and removed.
    pub sync: Duration,
}

impl Timing {
    pub const APP: Timing = Timing { quiet: Duration::from_secs(3), tick: Duration::from_secs(1), sync: Duration::from_secs(5) };
}

enum Msg {
    /// Something changed in this watched folder.
    Changed(FolderId),
    /// Scan network folders every so often (None: not at all).
    Network(Option<Duration>),
    Stop,
}

pub struct AutoScan {
    tx: Sender<Msg>,
    handle: Mutex<Option<JoinHandle<()>>>,
}

/// Does this change mean the library may be out of date? A file that is (or may be) a picture or video came, went or
/// changed, or a folder did (its name has no extension). Reads and attribute changes are not.
fn relevant(ev: &notify::Event) -> bool {
    matches!(ev.kind, EventKind::Create(_) | EventKind::Remove(_) | EventKind::Modify(notify::event::ModifyKind::Data(_) | notify::event::ModifyKind::Name(_) | notify::event::ModifyKind::Any))
        && ev.paths.iter().any(|p| media_kind(p).is_some() || p.extension().is_none())
}

struct Worker {
    db: Db,
    is_network: Arc<dyn Fn(&Path) -> bool + Send + Sync>,
    scan: ScanRequest,
    timing: Timing,
    tx: Sender<Msg>,
    watchers: HashMap<FolderId, (String, RecommendedWatcher)>,
    network_ids: Vec<FolderId>,
}

impl Worker {
    /// Watch the folders on this computer, forget the ones that were removed or are on the network.
    fn sync(&mut self) {
        let Ok(folders) = self.db.list_folders() else { return };
        self.network_ids.clear();
        let mut keep: Vec<FolderId> = Vec::new();
        for f in folders {
            if (self.is_network)(Path::new(&f.path)) {
                self.network_ids.push(f.id);
                continue;
            }
            keep.push(f.id);
            if self.watchers.get(&f.id).is_some_and(|(path, _)| *path == f.path) {
                continue;
            }
            let id = f.id;
            let tx = self.tx.clone();
            let made = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
                if res.as_ref().is_ok_and(relevant) {
                    let _ = tx.send(Msg::Changed(id));
                }
            })
            .and_then(|mut w| w.watch(Path::new(&f.path), RecursiveMode::Recursive).map(|_| w));
            match made {
                Ok(w) => {
                    self.watchers.insert(f.id, (f.path, w));
                }
                Err(_) => {
                    self.watchers.remove(&f.id); // not reachable now: tried again at the next sync
                }
            }
        }
        self.watchers.retain(|id, _| keep.contains(id));
    }

    fn run(mut self, rx: mpsc::Receiver<Msg>) {
        let mut changed: HashMap<FolderId, Instant> = HashMap::new();
        let mut network_every: Option<Duration> = None;
        let mut next_network = Instant::now();
        let mut last_sync: Option<Instant> = None;
        loop {
            match rx.recv_timeout(self.timing.tick) {
                Ok(Msg::Changed(id)) => {
                    changed.insert(id, Instant::now());
                }
                Ok(Msg::Network(every)) => {
                    network_every = every;
                    next_network = Instant::now() + every.unwrap_or_default();
                }
                Ok(Msg::Stop) | Err(RecvTimeoutError::Disconnected) => return,
                Err(RecvTimeoutError::Timeout) => {}
            }
            let now = Instant::now();
            if last_sync.map_or(true, |t| now.duration_since(t) >= self.timing.sync) {
                self.sync();
                last_sync = Some(now);
            }
            let due: Vec<FolderId> = changed.iter().filter(|(_, at)| now.duration_since(**at) >= self.timing.quiet).map(|(id, _)| *id).collect();
            if !due.is_empty() {
                for id in &due {
                    changed.remove(id);
                }
                (self.scan)(due);
            }
            if let Some(every) = network_every {
                if now >= next_network {
                    next_network = now + every;
                    if !self.network_ids.is_empty() {
                        (self.scan)(self.network_ids.clone());
                    }
                }
            }
        }
    }
}

impl AutoScan {
    /// Start watching. `is_network` says whether a folder is on the network; `scan` is called (from this service's thread)
    /// to ask for a scan of folders.
    pub fn start(db: Db, is_network: Arc<dyn Fn(&Path) -> bool + Send + Sync>, scan: ScanRequest, timing: Timing) -> AutoScan {
        let (tx, rx) = mpsc::channel();
        let worker = Worker { db, is_network, scan, timing, tx: tx.clone(), watchers: HashMap::new(), network_ids: Vec::new() };
        let handle = std::thread::spawn(move || worker.run(rx));
        AutoScan { tx, handle: Mutex::new(Some(handle)) }
    }

    /// Scan the network folders every `every` (None: never). The first scan is one interval from now: the launch scan has
    /// just covered them.
    pub fn set_network_rescan(&self, every: Option<Duration>) {
        let _ = self.tx.send(Msg::Network(every));
    }
}

impl Drop for AutoScan {
    fn drop(&mut self) {
        let _ = self.tx.send(Msg::Stop);
        if let Some(h) = self.handle.lock().unwrap_or_else(|p| p.into_inner()).take() {
            let _ = h.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    const FAST: Timing = Timing { quiet: Duration::from_millis(150), tick: Duration::from_millis(30), sync: Duration::from_millis(40) };

    fn wait_until(ms: u64, f: impl Fn() -> bool) -> bool {
        let end = Instant::now() + Duration::from_millis(ms);
        while Instant::now() < end {
            if f() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        f()
    }

    type Calls = Arc<Mutex<Vec<Vec<FolderId>>>>;

    fn recorder() -> (ScanRequest, Calls) {
        let calls: Calls = Arc::new(Mutex::new(Vec::new()));
        let c = calls.clone();
        (Arc::new(move |ids| c.lock().unwrap().push(ids)), calls)
    }

    #[test]
    fn a_new_picture_in_a_folder_on_this_computer_gets_the_folder_scanned_once_it_goes_quiet() {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open_in_memory().unwrap();
        let id = db.add_folder(dir.path().to_str().unwrap()).unwrap();
        let (scan, calls) = recorder();
        let _auto = AutoScan::start(db, Arc::new(|_| false), scan, FAST);
        std::thread::sleep(Duration::from_millis(300)); // watching has begun
        assert!(calls.lock().unwrap().is_empty());
        // a copy in progress: several files over a little while
        for i in 0..3 {
            std::fs::write(dir.path().join(format!("p{i}.jpg")), b"x").unwrap();
            std::thread::sleep(Duration::from_millis(60));
        }
        assert!(wait_until(3000, || !calls.lock().unwrap().is_empty()), "the folder was scanned");
        std::thread::sleep(Duration::from_millis(500));
        let c = calls.lock().unwrap().clone();
        assert!(c.iter().all(|ids| ids == &vec![id]));
        assert_eq!(c.len(), 1, "one scan for the whole burst, not one per file: {c:?}");
    }

    #[test]
    fn something_that_is_not_a_picture_does_not_cause_a_scan() {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open_in_memory().unwrap();
        db.add_folder(dir.path().to_str().unwrap()).unwrap();
        let (scan, calls) = recorder();
        let _auto = AutoScan::start(db, Arc::new(|_| false), scan, FAST);
        std::thread::sleep(Duration::from_millis(300));
        std::fs::write(dir.path().join("notes.txt"), b"x").unwrap();
        std::thread::sleep(Duration::from_millis(700));
        assert!(calls.lock().unwrap().is_empty());
    }

    #[test]
    fn a_folder_added_later_is_watched_and_one_that_is_removed_is_not() {
        let a = tempfile::tempdir().unwrap();
        let b = tempfile::tempdir().unwrap();
        let db = Db::open_in_memory().unwrap();
        let ida = db.add_folder(a.path().to_str().unwrap()).unwrap();
        let (scan, calls) = recorder();
        let _auto = AutoScan::start(db.clone(), Arc::new(|_| false), scan, FAST);
        std::thread::sleep(Duration::from_millis(200));
        let idb = db.add_folder(b.path().to_str().unwrap()).unwrap();
        std::thread::sleep(Duration::from_millis(300)); // picked up at the next sync
        std::fs::write(b.path().join("new.png"), b"x").unwrap();
        assert!(wait_until(3000, || calls.lock().unwrap().iter().any(|ids| ids.contains(&idb))));
        assert!(!calls.lock().unwrap().iter().any(|ids| ids.contains(&ida)));
        db.remove_folder(idb).unwrap();
        std::thread::sleep(Duration::from_millis(300));
        let before = calls.lock().unwrap().len();
        std::fs::write(b.path().join("later.png"), b"x").unwrap();
        std::thread::sleep(Duration::from_millis(700));
        assert_eq!(calls.lock().unwrap().len(), before, "a removed folder is no longer watched");
    }

    #[test]
    fn a_network_folder_is_scanned_on_the_timer_when_that_is_on_and_not_watched() {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open_in_memory().unwrap();
        let id = db.add_folder(dir.path().to_str().unwrap()).unwrap();
        let (scan, calls) = recorder();
        let auto = AutoScan::start(db, Arc::new(|_| true), scan, FAST);
        // off by default: nothing is scanned, and a change in it is not noticed
        std::fs::write(dir.path().join("a.jpg"), b"x").unwrap();
        std::thread::sleep(Duration::from_millis(600));
        assert!(calls.lock().unwrap().is_empty(), "a network folder is not watched");
        auto.set_network_rescan(Some(Duration::from_millis(200)));
        assert!(wait_until(3000, || calls.lock().unwrap().len() >= 2), "scanned again and again");
        assert!(calls.lock().unwrap().iter().all(|ids| ids == &vec![id]));
        auto.set_network_rescan(None);
        std::thread::sleep(Duration::from_millis(300));
        let n = calls.lock().unwrap().len();
        std::thread::sleep(Duration::from_millis(700));
        assert_eq!(calls.lock().unwrap().len(), n, "switched off");
    }

    #[test]
    fn stopping_the_service_does_not_hang() {
        let db = Db::open_in_memory().unwrap();
        let (scan, _) = recorder();
        let n = Arc::new(AtomicUsize::new(0));
        let auto = AutoScan::start(db, Arc::new(|_| false), scan, FAST);
        drop(auto);
        assert_eq!(n.load(Ordering::SeqCst), 0);
    }
}
