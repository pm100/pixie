use crate::fetch::{Fetcher, GeoError};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub type TileReply = Box<dyn FnOnce(Result<Vec<u8>, GeoError>) + Send>;

/// A job that has waited longer than this is for a map view the user has already left.
const DEFAULT_MAX_WAIT: Duration = Duration::from_secs(3);

struct Job {
    z: u8,
    x: u32,
    y: u32,
    reply: TileReply,
    queued: Instant,
}

/// Fetches OSM tiles on a small worker pool and caches them on disk.
#[derive(Clone)]
pub struct TileService {
    tx: Sender<Job>,
}

/// `stale`: the job waited too long; serve it from disk if we can but do not go to the network.
fn load(dir: &Path, fetcher: &dyn Fetcher, z: u8, x: u32, y: u32, stale: bool) -> Result<Vec<u8>, GeoError> {
    let path = dir.join(z.to_string()).join(x.to_string()).join(format!("{y}.png"));
    if let Ok(bytes) = std::fs::read(&path) {
        if !bytes.is_empty() {
            return Ok(bytes);
        }
    }
    if stale {
        return Err(GeoError::Network("tile request went stale".into()));
    }
    let bytes = fetcher.get(&format!("https://tile.openstreetmap.org/{z}/{x}/{y}.png"))?;
    if bytes.is_empty() {
        return Err(GeoError::Network("empty tile".into()));
    }
    // A failed cache write must not lose the tile we already have.
    if let Some(parent) = path.parent() {
        if std::fs::create_dir_all(parent).is_ok() {
            let tmp = path.with_extension("tmp");
            if std::fs::write(&tmp, &bytes).is_ok() {
                let _ = std::fs::rename(&tmp, &path);
            }
        }
    }
    Ok(bytes)
}

impl TileService {
    pub fn new(dir: PathBuf, fetcher: Arc<dyn Fetcher>, workers: usize) -> Self {
        Self::with_max_wait(dir, fetcher, workers, DEFAULT_MAX_WAIT)
    }

    pub fn with_max_wait(dir: PathBuf, fetcher: Arc<dyn Fetcher>, workers: usize, max_wait: Duration) -> Self {
        let (tx, rx) = channel::<Job>();
        let rx = Arc::new(Mutex::new(rx));
        for _ in 0..workers.max(1) {
            let (rx, dir, fetcher) = (rx.clone(), dir.clone(), fetcher.clone());
            std::thread::spawn(move || loop {
                let job = match rx.lock().unwrap().recv() {
                    Ok(job) => job,
                    Err(_) => return, // every sender dropped
                };
                let result = load(&dir, &*fetcher, job.z, job.x, job.y, job.queued.elapsed() > max_wait);
                (job.reply)(result);
            });
        }
        TileService { tx }
    }

    pub fn request(&self, z: u8, x: u32, y: u32, reply: TileReply) {
        if z > 19 || x >= (1u32 << z) || y >= (1u32 << z) {
            return reply(Err(GeoError::BadTile(z, x, y)));
        }
        if let Err(e) = self.tx.send(Job { z, x, y, reply, queued: Instant::now() }) {
            (e.0.reply)(Err(GeoError::Io("tile service stopped".into())));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fetch::testing::FakeFetcher;
    use std::sync::mpsc;
    use std::time::Duration;

    fn fetch(s: &TileService, z: u8, x: u32, y: u32) -> Result<Vec<u8>, GeoError> {
        let (tx, rx) = mpsc::channel();
        s.request(z, x, y, Box::new(move |r| tx.send(r).unwrap()));
        rx.recv_timeout(Duration::from_secs(5)).unwrap()
    }

    #[test]
    fn fetches_once_then_serves_from_disk() {
        let dir = tempfile::tempdir().unwrap();
        let f = Arc::new(FakeFetcher::new(Ok(vec![1, 2, 3])));
        let s = TileService::new(dir.path().to_path_buf(), f.clone(), 2);
        assert_eq!(fetch(&s, 5, 10, 12).unwrap(), vec![1, 2, 3]);
        assert_eq!(fetch(&s, 5, 10, 12).unwrap(), vec![1, 2, 3]);
        assert_eq!(f.count(), 1);
        assert!(dir.path().join("5").join("10").join("12.png").exists());
        assert_eq!(f.calls.lock().unwrap()[0].0, "https://tile.openstreetmap.org/5/10/12.png");
    }

    #[test]
    fn out_of_range_tiles_are_rejected_without_a_fetch() {
        let dir = tempfile::tempdir().unwrap();
        let f = Arc::new(FakeFetcher::new(Ok(vec![1])));
        let s = TileService::new(dir.path().to_path_buf(), f.clone(), 1);
        assert_eq!(fetch(&s, 20, 0, 0), Err(GeoError::BadTile(20, 0, 0)));
        assert_eq!(fetch(&s, 3, 8, 0), Err(GeoError::BadTile(3, 8, 0)));
        assert_eq!(fetch(&s, 3, 0, 8), Err(GeoError::BadTile(3, 0, 8)));
        assert_eq!(f.count(), 0);
    }

    struct Gated {
        first: std::sync::atomic::AtomicBool,
        release: Mutex<mpsc::Receiver<()>>,
        calls: std::sync::atomic::AtomicUsize,
    }

    impl Fetcher for Gated {
        fn get(&self, _url: &str) -> Result<Vec<u8>, GeoError> {
            use std::sync::atomic::Ordering::SeqCst;
            self.calls.fetch_add(1, SeqCst);
            if self.first.swap(false, SeqCst) {
                self.release.lock().unwrap().recv().unwrap();
            }
            Ok(vec![7])
        }
    }

    #[test]
    fn jobs_that_waited_too_long_are_dropped_without_a_fetch() {
        use std::sync::atomic::Ordering::SeqCst;
        let dir = tempfile::tempdir().unwrap();
        let (release, rx) = mpsc::channel();
        let g = Arc::new(Gated { first: true.into(), release: Mutex::new(rx), calls: 0.into() });
        let s = TileService::with_max_wait(dir.path().to_path_buf(), g.clone(), 1, Duration::from_millis(100));
        let (tx1, rx1) = mpsc::channel();
        s.request(5, 1, 1, Box::new(move |r| tx1.send(r).unwrap())); // occupies the only worker
        std::thread::sleep(Duration::from_millis(50));
        let (tx2, rx2) = mpsc::channel();
        s.request(5, 2, 2, Box::new(move |r| tx2.send(r).unwrap())); // queued behind it
        std::thread::sleep(Duration::from_millis(300)); // now older than max_wait
        release.send(()).unwrap();
        assert_eq!(rx1.recv_timeout(Duration::from_secs(5)).unwrap().unwrap(), vec![7]);
        assert!(rx2.recv_timeout(Duration::from_secs(5)).unwrap().is_err());
        assert_eq!(g.calls.load(SeqCst), 1);
    }

    #[test]
    fn a_stale_job_is_still_served_from_disk() {
        let dir = tempfile::tempdir().unwrap();
        let tile = dir.path().join("5").join("2");
        std::fs::create_dir_all(&tile).unwrap();
        std::fs::write(tile.join("2.png"), [9, 9]).unwrap();
        let (release, rx) = mpsc::channel();
        let g = Arc::new(Gated { first: true.into(), release: Mutex::new(rx), calls: 0.into() });
        let s = TileService::with_max_wait(dir.path().to_path_buf(), g, 1, Duration::from_millis(100));
        let (tx1, rx1) = mpsc::channel();
        s.request(5, 1, 1, Box::new(move |r| tx1.send(r).unwrap()));
        std::thread::sleep(Duration::from_millis(50));
        let (tx2, rx2) = mpsc::channel();
        s.request(5, 2, 2, Box::new(move |r| tx2.send(r).unwrap()));
        std::thread::sleep(Duration::from_millis(300));
        release.send(()).unwrap();
        rx1.recv_timeout(Duration::from_secs(5)).unwrap().unwrap();
        assert_eq!(rx2.recv_timeout(Duration::from_secs(5)).unwrap().unwrap(), vec![9, 9]);
    }

    #[test]
    fn failures_and_empty_bodies_are_not_cached() {
        let dir = tempfile::tempdir().unwrap();
        let f = Arc::new(FakeFetcher::new(Err(GeoError::Network("offline".into()))));
        let s = TileService::new(dir.path().to_path_buf(), f.clone(), 1);
        assert!(fetch(&s, 2, 1, 1).is_err());
        f.set(Ok(vec![]));
        assert!(fetch(&s, 2, 1, 1).is_err());
        f.set(Ok(vec![9]));
        assert_eq!(fetch(&s, 2, 1, 1).unwrap(), vec![9]);
        assert_eq!(f.count(), 3);
    }
}
