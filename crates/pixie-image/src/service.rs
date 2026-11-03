use crate::thumbs::{ensure_thumbnail, ThumbClass};
use pixie_core::{PixieError, Result};
use std::panic::AssertUnwindSafe;
use std::path::Path;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

/// Completion callback, run on a worker thread.
pub type Callback = Box<dyn FnOnce(Result<PathBuf>) + Send>;
type Reply = Callback;
type Key = (ThumbClass, i64, i64, i64);
type Generator = fn(&Path, ThumbClass, i64, i64, i64, &Path) -> Result<PathBuf>;

/// A thumbnail that failed is not asked for again for this long: the tile keeps its warning, and a file the cloud drive cannot give (or
/// a drive that is away) is not asked for again each time the tile is drawn.
const RETRY_AFTER: Duration = Duration::from_secs(300);

struct Pending {
    src: PathBuf,
    replies: Vec<Reply>,
}

#[derive(Default)]
struct State {
    stack: Vec<Key>,
    pending: HashMap<Key, Pending>,
    /// Thumbnails that failed, when and why: answered from here until `retry_after` has passed.
    failed: HashMap<Key, (Instant, PixieError)>,
    shutdown: bool,
}

struct Inner {
    cache_dir: PathBuf,
    generate: Generator,
    retry_after: Duration,
    state: Mutex<State>,
    cv: Condvar,
}

/// Dropping the last clone stops the workers.
#[derive(Clone)]
pub struct ThumbService {
    inner: Arc<Inner>,
    _guard: Arc<Guard>,
}

struct Guard(Arc<Inner>);
impl Drop for Guard {
    fn drop(&mut self) {
        self.0.state.lock().unwrap_or_else(|p| p.into_inner()).shutdown = true;
        self.0.cv.notify_all();
    }
}

impl State {
    /// Why this thumbnail failed, if it did within `retry_after`; a failure older than that is forgotten, so it is tried again.
    fn recent_failure(&mut self, key: &Key, retry_after: Duration) -> Option<PixieError> {
        match self.failed.get(key) {
            Some((at, e)) if at.elapsed() < retry_after => Some(e.clone()),
            Some(_) => {
                self.failed.remove(key);
                None
            }
            None => None,
        }
    }
}

impl ThumbService {
    pub fn new(cache_dir: PathBuf, workers: usize) -> ThumbService {
        Self::with_generator(cache_dir, workers, ensure_thumbnail)
    }

    /// Test seam: lets tests inject a generator that fails or panics.
    fn with_generator(cache_dir: PathBuf, workers: usize, generate: Generator) -> ThumbService {
        Self::with_generator_and_retry(cache_dir, workers, generate, RETRY_AFTER)
    }

    fn with_generator_and_retry(cache_dir: PathBuf, workers: usize, generate: Generator, retry_after: Duration) -> ThumbService {
        let inner = Arc::new(Inner { cache_dir, generate, retry_after, state: Mutex::new(State::default()), cv: Condvar::new() });
        for _ in 0..workers.max(1) {
            let inner = inner.clone();
            std::thread::spawn(move || worker(inner));
        }
        ThumbService { _guard: Arc::new(Guard(inner.clone())), inner }
    }

    /// Channel flavour, convenient for tests and one-off callers.
    pub fn request(&self, class: ThumbClass, photo_id: i64, mtime: i64, size: i64, src: PathBuf) -> Receiver<Result<PathBuf>> {
        let (tx, rx) = channel();
        self.request_with(class, photo_id, mtime, size, src, Box::new(move |r| {
            let _ = tx.send(r);
        }));
        rx
    }

    /// Ask for a thumbnail; `done` runs on a worker thread when it is ready (or failed).
    /// Newest requests are served first, and a request jumps ahead of any queued prefetch.
    pub fn request_with(&self, class: ThumbClass, photo_id: i64, mtime: i64, size: i64, src: PathBuf, done: Callback) {
        let key = (class, photo_id, mtime, size);
        let mut st = self.inner.state.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(e) = st.recent_failure(&key, self.inner.retry_after) {
            drop(st);
            return done(Err(e));
        }
        match st.pending.get_mut(&key) {
            Some(p) => {
                p.replies.push(done);
                // Already queued (maybe by prefetch): bring it to the front.
                if let Some(pos) = st.stack.iter().position(|k| *k == key) {
                    st.stack.remove(pos);
                    st.stack.push(key);
                }
            }
            None => {
                st.pending.insert(key, Pending { src, replies: vec![done] });
                st.stack.push(key);
            }
        }
        self.inner.cv.notify_one();
    }

    /// How many thumbnails are waiting or being made right now (on-demand and prefetched alike).
    pub fn pending_count(&self) -> usize {
        self.inner.state.lock().unwrap_or_else(|p| p.into_inner()).pending.len()
    }

    /// Queue thumbnails at the lowest priority, in the given order, skipping ones
    /// already on disk or already queued. Nobody waits for the result.
    pub fn prefetch(&self, jobs: Vec<(ThumbClass, i64, i64, i64, PathBuf)>) {
        let mut st = self.inner.state.lock().unwrap_or_else(|p| p.into_inner());
        let mut keys = Vec::new();
        for (class, id, mtime, size, src) in jobs {
            let key = (class, id, mtime, size);
            if st.pending.contains_key(&key) || st.recent_failure(&key, self.inner.retry_after).is_some() || crate::thumbs::thumb_path(&self.inner.cache_dir, class, id, mtime, size).exists() {
                continue;
            }
            st.pending.insert(key, Pending { src, replies: vec![] });
            keys.push(key);
        }
        let n = keys.len();
        // The stack pops from the end, so lowest priority goes at the front,
        // reversed so the first job is the first prefetch popped.
        st.stack.splice(0..0, keys.into_iter().rev());
        drop(st);
        for _ in 0..n.min(64) {
            self.inner.cv.notify_one();
        }
    }
}

fn worker(inner: Arc<Inner>) {
    loop {
        let (key, src) = {
            let mut st = inner.state.lock().unwrap_or_else(|p| p.into_inner());
            loop {
                if st.shutdown {
                    return;
                }
                if let Some(key) = st.stack.pop() {
                    let src = st.pending[&key].src.clone();
                    break (key, src);
                }
                st = inner.cv.wait(st).unwrap_or_else(|p| p.into_inner());
            }
        };
        let (class, id, mtime, size) = key;
        // A panicking decoder must not strand waiting requests or kill the worker.
        let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
            (inner.generate)(&inner.cache_dir, class, id, mtime, size, &src)
        }))
        .unwrap_or_else(|_| Err(PixieError::Image("decoder crashed".into())));
        if let Err(e) = &result {
            eprintln!("[thumb] FAILED {}: {e} (not tried again for {} min)", src.display(), inner.retry_after.as_secs().div_ceil(60));
        }
        let replies = {
            let mut st = inner.state.lock().unwrap_or_else(|p| p.into_inner());
            match &result {
                Ok(_) => {
                    st.failed.remove(&key);
                }
                Err(e) => {
                    st.failed.insert(key, (Instant::now(), e.clone()));
                }
            }
            st.pending.remove(&key).map(|p| p.replies).unwrap_or_default()
        };
        for r in replies {
            r(result.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pixie_testutil::jpeg;
    use std::time::Duration;

    fn wait(rx: Receiver<Result<PathBuf>>) -> Result<PathBuf> {
        rx.recv_timeout(Duration::from_secs(10)).expect("thumbnail result in time")
    }

    #[test]
    fn generates_thumbnail_for_request() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("a.jpg");
        std::fs::write(&src, jpeg(600, 300)).unwrap();
        let svc = ThumbService::new(dir.path().join("cache"), 2);
        let out = wait(svc.request(ThumbClass::Grid, 1, 1, 0, src)).unwrap();
        assert!(out.exists());
    }

    #[test]
    fn duplicate_requests_share_one_result() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("a.jpg");
        std::fs::write(&src, jpeg(600, 300)).unwrap();
        let svc = ThumbService::new(dir.path().join("cache"), 1);
        let a = svc.request(ThumbClass::Grid, 1, 1, 0, src.clone());
        let b = svc.request(ThumbClass::Grid, 1, 1, 0, src);
        assert_eq!(wait(a).unwrap(), wait(b).unwrap());
    }

    #[test]
    fn corrupt_file_yields_error_and_service_keeps_working() {
        let dir = tempfile::tempdir().unwrap();
        let bad = dir.path().join("bad.jpg");
        std::fs::write(&bad, b"junk").unwrap();
        let good = dir.path().join("good.jpg");
        std::fs::write(&good, jpeg(100, 100)).unwrap();
        let svc = ThumbService::new(dir.path().join("cache"), 1);
        assert!(wait(svc.request(ThumbClass::Grid, 1, 1, 0, bad)).is_err());
        assert!(wait(svc.request(ThumbClass::Grid, 2, 1, 0, good)).is_ok());
    }

    #[test]
    fn many_requests_all_complete() {
        let dir = tempfile::tempdir().unwrap();
        let svc = ThumbService::new(dir.path().join("cache"), 4);
        let mut rxs = vec![];
        for i in 0..40 {
            let p = dir.path().join(format!("{i}.jpg"));
            std::fs::write(&p, jpeg(300, 200)).unwrap();
            rxs.push(svc.request(ThumbClass::Grid, i, 1, 0, p));
        }
        for rx in rxs {
            assert!(wait(rx).is_ok());
        }
    }

    #[test]
    fn decoder_panic_returns_error_and_worker_survives() {
        fn boom(_: &std::path::Path, _: ThumbClass, id: i64, _: i64, _: i64, _: &std::path::Path) -> Result<PathBuf> {
            if id == 1 { panic!("decoder blew up"); }
            Ok(PathBuf::from("ok"))
        }
        let dir = tempfile::tempdir().unwrap();
        let svc = ThumbService::with_generator(dir.path().to_path_buf(), 1, boom);
        assert!(wait(svc.request(ThumbClass::Grid, 1, 1, 0, PathBuf::from("x"))).is_err());
        // the single worker must still be alive
        assert!(wait(svc.request(ThumbClass::Grid, 2, 1, 0, PathBuf::from("x"))).is_ok());
    }

    #[test]
    fn callback_request_invokes_callback_once_with_the_result() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("a.jpg");
        std::fs::write(&src, jpeg(300, 200)).unwrap();
        let svc = ThumbService::new(dir.path().join("cache"), 1);
        let (tx, rx) = channel();
        svc.request_with(ThumbClass::Grid, 1, 1, 0, src, Box::new(move |r| tx.send(r).unwrap()));
        assert!(wait(rx).is_ok());
    }

    #[test]
    fn prefetch_generates_thumbnails_in_the_background() {
        let dir = tempfile::tempdir().unwrap();
        let svc = ThumbService::new(dir.path().join("cache"), 2);
        let mut jobs = vec![];
        for i in 0..6 {
            let p = dir.path().join(format!("{i}.jpg"));
            std::fs::write(&p, jpeg(300, 200)).unwrap();
            jobs.push((ThumbClass::Grid, i, 1, 0, p));
        }
        svc.prefetch(jobs);
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        let all_there = || (0..6).all(|i| crate::thumbs::thumb_path(&dir.path().join("cache"), ThumbClass::Grid, i, 1, 0).exists());
        while !all_there() && std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(all_there());
    }

    static ORDER: Mutex<Vec<i64>> = Mutex::new(Vec::new());
    fn slow_recording(_: &Path, _: ThumbClass, id: i64, _: i64, _: i64, _: &Path) -> Result<PathBuf> {
        ORDER.lock().unwrap().push(id);
        std::thread::sleep(Duration::from_millis(40));
        Ok(PathBuf::from("ok"))
    }

    #[test]
    fn on_demand_request_jumps_ahead_of_prefetch() {
        let dir = tempfile::tempdir().unwrap();
        let svc = ThumbService::with_generator(dir.path().to_path_buf(), 1, slow_recording);
        svc.prefetch((1..=5).map(|i| (ThumbClass::Grid, i, 1, 0, PathBuf::from("x"))).collect());
        let rx = svc.request(ThumbClass::Grid, 5, 1, 0, PathBuf::from("x"));
        assert!(wait(rx).is_ok());
        std::thread::sleep(Duration::from_millis(300)); // let the rest drain
        let order = ORDER.lock().unwrap().clone();
        let pos = |id: i64| order.iter().position(|&x| x == id).unwrap();
        assert!(pos(5) < pos(2), "on-demand 5 must beat queued prefetch 2, got {order:?}");
    }

    static CALLS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    fn counting(_: &Path, _: ThumbClass, _: i64, _: i64, _: i64, _: &Path) -> Result<PathBuf> {
        CALLS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(PathBuf::from("ok"))
    }

    #[test]
    fn prefetch_skips_thumbnails_that_already_exist() {
        let dir = tempfile::tempdir().unwrap();
        let cache = dir.path().to_path_buf();
        let existing = crate::thumbs::thumb_path(&cache, ThumbClass::Grid, 9, 1, 0);
        std::fs::create_dir_all(existing.parent().unwrap()).unwrap();
        std::fs::write(&existing, b"x").unwrap();
        let svc = ThumbService::with_generator(cache, 1, counting);
        svc.prefetch(vec![(ThumbClass::Grid, 9, 1, 0, PathBuf::from("x"))]);
        std::thread::sleep(Duration::from_millis(200));
        assert_eq!(CALLS.load(std::sync::atomic::Ordering::SeqCst), 0);
    }

    static FAILS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    fn always_fails(_: &Path, _: ThumbClass, _: i64, _: i64, _: i64, _: &Path) -> Result<PathBuf> {
        FAILS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Err(PixieError::Io("The cloud operation is invalid.".into()))
    }

    #[test]
    fn a_thumbnail_that_failed_is_not_tried_again_straight_away() {
        let dir = tempfile::tempdir().unwrap();
        let svc = ThumbService::with_generator(dir.path().to_path_buf(), 1, always_fails);
        let first = wait(svc.request(ThumbClass::Grid, 7, 1, 0, PathBuf::from("x")));
        let again = wait(svc.request(ThumbClass::Grid, 7, 1, 0, PathBuf::from("x")));
        assert!(first.is_err() && again.is_err(), "the tile keeps its warning");
        assert_eq!(FAILS.load(std::sync::atomic::Ordering::SeqCst), 1, "the file was asked for once, not each time the tile was shown");
        svc.prefetch(vec![(ThumbClass::Grid, 7, 1, 0, PathBuf::from("x"))]);
        std::thread::sleep(Duration::from_millis(150));
        assert_eq!(FAILS.load(std::sync::atomic::Ordering::SeqCst), 1, "a prefetch does not ask again either");
    }

    static LATER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    fn fails_then_works(_: &Path, _: ThumbClass, _: i64, _: i64, _: i64, _: &Path) -> Result<PathBuf> {
        if LATER.fetch_add(1, std::sync::atomic::Ordering::SeqCst) == 0 {
            Err(PixieError::Io("away".into()))
        } else {
            Ok(PathBuf::from("ok"))
        }
    }

    #[test]
    fn a_failed_thumbnail_is_tried_again_once_the_wait_is_over() {
        let dir = tempfile::tempdir().unwrap();
        let svc = ThumbService::with_generator_and_retry(dir.path().to_path_buf(), 1, fails_then_works, Duration::from_millis(60));
        assert!(wait(svc.request(ThumbClass::Grid, 8, 1, 0, PathBuf::from("x"))).is_err());
        std::thread::sleep(Duration::from_millis(100));
        assert!(wait(svc.request(ThumbClass::Grid, 8, 1, 0, PathBuf::from("x"))).is_ok(), "the file is asked for again and has come back");
    }

    static RELEASE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

    fn blocked_until_released(_: &Path, _: ThumbClass, _: i64, _: i64, _: i64, src: &Path) -> Result<PathBuf> {
        while !RELEASE.load(std::sync::atomic::Ordering::SeqCst) {
            std::thread::sleep(Duration::from_millis(5));
        }
        Ok(src.to_path_buf())
    }

    #[test]
    fn pending_count_rises_with_prefetch_and_returns_to_zero() {
        let dir = tempfile::tempdir().unwrap();
        let svc = ThumbService::with_generator(dir.path().join("cache"), 2, blocked_until_released);
        assert_eq!(svc.pending_count(), 0);
        svc.prefetch((0..5).map(|i| (ThumbClass::Grid, i, 1, 1, PathBuf::from(format!("/nowhere/{i}.jpg")))).collect());
        assert_eq!(svc.pending_count(), 5, "queued and in-flight jobs both count");
        RELEASE.store(true, std::sync::atomic::Ordering::SeqCst);
        let start = std::time::Instant::now();
        while svc.pending_count() > 0 {
            assert!(start.elapsed() < Duration::from_secs(5), "the queue never drained");
            std::thread::sleep(Duration::from_millis(5));
        }
    }
}
