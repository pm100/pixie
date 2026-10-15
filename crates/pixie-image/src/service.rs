use crate::thumbs::{ensure_thumbnail, ThumbClass};
use pixie_core::Result;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Condvar, Mutex};

type Reply = Sender<Result<PathBuf>>;
type Key = (ThumbClass, i64, i64);

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
        let inner = Arc::new(Inner { cache_dir, state: Mutex::new(State::default()), cv: Condvar::new() });
        for _ in 0..workers.max(1) {
            let inner = inner.clone();
            std::thread::spawn(move || worker(inner));
        }
        ThumbService { _guard: Arc::new(Guard(inner.clone())), inner }
    }

    pub fn request(&self, class: ThumbClass, photo_id: i64, mtime: i64, src: PathBuf) -> Receiver<Result<PathBuf>> {
        let (tx, rx) = channel();
        let key = (class, photo_id, mtime);
        let mut st = self.inner.state.lock().unwrap_or_else(|p| p.into_inner());
        match st.pending.get_mut(&key) {
            Some(p) => p.replies.push(tx),
            None => {
                st.pending.insert(key, Pending { src, replies: vec![tx] });
                st.stack.push(key);
                self.inner.cv.notify_one();
            }
        }
        rx
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
        let (class, id, mtime) = key;
        let result = ensure_thumbnail(&inner.cache_dir, class, id, mtime, &src);
        let replies = inner
            .state
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .pending
            .remove(&key)
            .map(|p| p.replies)
            .unwrap_or_default();
        for r in replies {
            let _ = r.send(result.clone());
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
        let out = wait(svc.request(ThumbClass::Grid, 1, 1, src)).unwrap();
        assert!(out.exists());
    }

    #[test]
    fn duplicate_requests_share_one_result() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("a.jpg");
        std::fs::write(&src, jpeg(600, 300)).unwrap();
        let svc = ThumbService::new(dir.path().join("cache"), 1);
        let a = svc.request(ThumbClass::Grid, 1, 1, src.clone());
        let b = svc.request(ThumbClass::Grid, 1, 1, src);
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
        assert!(wait(svc.request(ThumbClass::Grid, 1, 1, bad)).is_err());
        assert!(wait(svc.request(ThumbClass::Grid, 2, 1, good)).is_ok());
    }

    #[test]
    fn many_requests_all_complete() {
        let dir = tempfile::tempdir().unwrap();
        let svc = ThumbService::new(dir.path().join("cache"), 4);
        let mut rxs = vec![];
        for i in 0..40 {
            let p = dir.path().join(format!("{i}.jpg"));
            std::fs::write(&p, jpeg(300, 200)).unwrap();
            rxs.push(svc.request(ThumbClass::Grid, i, 1, p));
        }
        for rx in rxs {
            assert!(wait(rx).is_ok());
        }
    }
}
