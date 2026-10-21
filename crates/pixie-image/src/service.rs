use crate::thumbs::{ensure_thumbnail, ThumbClass};
use pixie_core::{PixieError, Result};
use std::panic::AssertUnwindSafe;
use std::path::Path;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver};
use std::sync::{Arc, Condvar, Mutex};

/// Completion callback, run on a worker thread.
pub type Callback = Box<dyn FnOnce(Result<PathBuf>) + Send>;
type Reply = Callback;
type Key = (ThumbClass, i64, i64, i64);
type Generator = fn(&Path, ThumbClass, i64, i64, i64, &Path) -> Result<PathBuf>;

struct Pending {
    src: PathBuf,
    replies: Vec<Reply>,
}

#[derive(Default)]
struct State {
    stack: Vec<Key>,
    pending: HashMap<Key, Pending>,
    shutdown: bool,
}

struct Inner {
    cache_dir: PathBuf,
    generate: Generator,
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

impl ThumbService {
    pub fn new(cache_dir: PathBuf, workers: usize) -> ThumbService {
        Self::with_generator(cache_dir, workers, ensure_thumbnail)
    }

    /// Test seam: lets tests inject a generator that fails or panics.
    fn with_generator(cache_dir: PathBuf, workers: usize, generate: Generator) -> ThumbService {
        let inner = Arc::new(Inner { cache_dir, generate, state: Mutex::new(State::default()), cv: Condvar::new() });
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
            if st.pending.contains_key(&key) || crate::thumbs::thumb_path(&self.inner.cache_dir, class, id, mtime, size).exists() {
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
        let replies = inner
            .state
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .pending
            .remove(&key)
            .map(|p| p.replies)
            .unwrap_or_default();
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
