use crate::faces::FaceEngine;
use crate::models::{ensure_models, Downloader, ModelFile};
use crate::pipeline::PhotoPipeline;
use pixie_index::Db;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, Weak};

pub type FaceLoader = Arc<dyn Fn(&Path) -> Result<Arc<dyn FaceEngine>, String> + Send + Sync>;

/// What the UI shows about the people search. `models` is "missing", "downloading", "ready" or "error";
/// `done` / `total` count photos scanned for faces.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PeopleStatus {
    pub models: String,
    pub model_done: u64,
    pub model_total: u64,
    pub done: i64,
    pub total: i64,
    pub paused: bool,
    /// Changes whenever who-is-who changes (a regrouping ran in the background, or the user edited someone).
    pub rev: i64,
    pub error: Option<String>,
}

pub struct PeopleConfig {
    pub db: Db,
    /// The engine's `version()`, whose progress is counted.
    pub version: String,
    pub dir: PathBuf,
    pub files: &'static [ModelFile],
    pub downloader: Arc<dyn Downloader>,
    pub loader: FaceLoader,
    /// The pipeline that does the work: this service gives it the models once they are loaded, and shows its progress.
    pub pipeline: Arc<PhotoPipeline>,
    pub on_status: Arc<dyn Fn(PeopleStatus) + Send + Sync>,
}

struct State {
    models: &'static str,
    model_done: u64,
    model_total: u64,
    error: Option<String>,
    paused: bool,
    loading: bool,
    loaded: bool,
    /// People are switched on (when off the models are not fetched and the pipeline looks for no faces).
    enabled: bool,
    engine: Option<Arc<dyn FaceEngine>>,
}

struct Inner {
    cfg: PeopleConfig,
    state: Mutex<State>,
}

/// Downloads the face models, loads them and runs the background face scan.
pub struct PeopleService {
    inner: Arc<Inner>,
}

impl Inner {
    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|p| p.into_inner())
    }

    fn status(&self) -> PeopleStatus {
        let (done, total) = self.cfg.db.face_counts(&self.cfg.version).unwrap_or((0, 0));
        let total = total + self.cfg.pipeline.pending_photos();
        let s = self.lock();
        PeopleStatus {
            models: s.models.to_string(),
            model_done: s.model_done,
            model_total: s.model_total,
            done,
            total,
            paused: s.paused,
            rev: self.cfg.db.face_rev().unwrap_or(0),
            error: s.error.clone().or_else(|| self.cfg.pipeline.face_error()),
        }
    }

    /// Never call while holding the state lock: `status` takes it.
    fn emit(&self) {
        (self.cfg.on_status)(self.status());
    }

    /// Runs `setup_inner`; whatever happens inside (even a panic from the model runtime) ends in a clear state.
    fn setup(self: &Arc<Self>) {
        if let Err(p) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| self.setup_inner())) {
            let why = p.downcast_ref::<&str>().map(|s| s.to_string()).or_else(|| p.downcast_ref::<String>().cloned());
            {
                let mut s = self.lock();
                s.loading = false;
                s.models = "error";
                s.error = Some(why.unwrap_or_else(|| "the face models could not start".into()));
            }
            self.emit();
        }
    }

    fn setup_inner(self: &Arc<Self>) {
        let mut last_pct = u64::MAX;
        let fetched = ensure_models(&self.cfg.dir, self.cfg.files, &*self.cfg.downloader, &mut |d, t| {
            {
                let mut s = self.lock();
                s.model_done = d;
                s.model_total = t;
            }
            let pct = d * 100 / t.max(1);
            if pct != last_pct {
                last_pct = pct;
                self.emit();
            }
            true
        });
        let result = fetched.and_then(|_| (self.cfg.loader)(&self.cfg.dir));
        {
            let mut s = self.lock();
            s.loading = false;
            match result {
                Ok(engine) => {
                    s.loaded = true;
                    s.engine = Some(engine.clone());
                    if s.enabled {
                        s.models = "ready";
                        self.cfg.pipeline.set_face_engine(engine); // the pipeline now scans the photos that have not been
                    } else {
                        s.models = "off"; // switched off while it was loading: kept, and handed over when switched on
                    }
                }
                Err(e) => {
                    s.models = "error";
                    s.error = Some(e);
                }
            }
        }
        self.emit();
    }
}

impl PeopleService {
    pub fn start(cfg: PeopleConfig) -> PeopleService {
        Self::start_with(cfg, true)
    }

    /// `enabled` false: People are off to begin with, and no model is fetched until they are switched on.
    pub fn start_with(cfg: PeopleConfig, enabled: bool) -> PeopleService {
        let svc = PeopleService {
            inner: Arc::new(Inner {
                cfg,
                state: Mutex::new(State {
                    models: if enabled { "missing" } else { "off" },
                    model_done: 0,
                    model_total: 0,
                    error: None,
                    paused: false,
                    loading: false,
                    loaded: false,
                    enabled,
                    engine: None,
                }),
            }),
        };
        let weak = Arc::downgrade(&svc.inner);
        svc.inner.cfg.pipeline.add_reporter(Arc::new(move || {
            if let Some(i) = Weak::upgrade(&weak) {
                i.emit();
            }
        }));
        svc.begin();
        svc
    }

    fn begin(&self) {
        {
            let mut s = self.inner.lock();
            if !s.enabled || s.loading || s.loaded {
                return;
            }
            s.loading = true;
            s.models = "downloading";
            s.error = None;
            s.model_done = 0;
            s.model_total = self.inner.cfg.files.iter().map(|f| f.bytes).sum();
        }
        self.inner.emit();
        let inner = self.inner.clone();
        std::thread::spawn(move || inner.setup());
    }

    /// Tell the window now (something changed that the status shows, such as faces being regrouped).
    pub fn announce(&self) {
        self.inner.emit();
    }

    pub fn status(&self) -> PeopleStatus {
        self.inner.status()
    }

    /// Try the download and load again after an error.
    pub fn retry(&self) {
        self.begin();
    }

    /// Switch People on or off. Off: the pipeline looks for no more faces and no model is fetched (the faces and names
    /// already found are kept). On: the models are fetched if they are not yet here, and the photos not yet scanned are picked up.
    pub fn set_enabled(&self, on: bool) {
        let engine = {
            let mut s = self.inner.lock();
            if s.enabled == on {
                return;
            }
            s.enabled = on;
            s.models = if !on { "off" } else if s.engine.is_some() { "ready" } else { "missing" };
            s.engine.clone()
        };
        if !on {
            self.inner.cfg.pipeline.clear_face_engine();
        } else if let Some(e) = engine {
            self.inner.cfg.pipeline.set_face_engine(e);
        } else {
            self.begin();
        }
        self.inner.emit();
    }

    /// The one Pause: the pipeline stops taking photos (for every step), and the window is told.
    pub fn set_paused(&self, paused: bool) {
        self.inner.lock().paused = paused;
        self.inner.cfg.pipeline.set_paused(paused);
    }

    /// New photos may have arrived (a scan finished): wake an idle worker.
    pub fn kick(&self) {
        self.inner.cfg.pipeline.kick();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use crate::faces::FakeFaceEngine;
    use crate::pipeline::PipelineConfig;
    use image::{Rgb, RgbImage};
    use pixie_core::PhotoMeta;
    use sha2::{Digest, Sha256};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::Instant;

    struct FakeDl {
        data: Vec<u8>,
        fail: AtomicBool,
    }

    impl Downloader for FakeDl {
        fn fetch(&self, _url: &str, sink: &mut dyn FnMut(&[u8]) -> bool) -> Result<(), String> {
            if self.fail.load(Ordering::SeqCst) {
                return Err("offline".into());
            }
            for c in self.data.chunks(2) {
                if !sink(c) {
                    break;
                }
            }
            Ok(())
        }
    }

    fn files(data: &[u8]) -> &'static [ModelFile] {
        let sha: String = Sha256::digest(data).iter().map(|b| format!("{b:02x}")).collect();
        Box::leak(vec![ModelFile { name: "m.bin", url: "u://m", sha256: Box::leak(sha.into_boxed_str()), bytes: data.len() as u64 }].into_boxed_slice())
    }

    fn wait_until(ms: u64, f: impl Fn() -> bool) -> bool {
        let t = Instant::now();
        while t.elapsed() < Duration::from_millis(ms) {
            if f() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        f()
    }


    /// The pipeline the service hands its model to, working on the photos in `dir`.
    fn pipeline(db: &Db, dir: &std::path::Path) -> Arc<PhotoPipeline> {
        let mut cfg = PipelineConfig::new(db.clone(), dir.join("thumbs"), Arc::new(|| false));
        cfg.poll = Duration::from_millis(10);
        cfg.workers = 2;
        Arc::new(PhotoPipeline::start(cfg))
    }

    struct Rig {
        svc: PeopleService,
        db: Db,
        dl: Arc<FakeDl>,
        events: Arc<Mutex<Vec<PeopleStatus>>>,
        _models: tempfile::TempDir,
        _photos: tempfile::TempDir,
    }

    fn rig(photos: usize, fail_download: bool, loader: FaceLoader) -> Rig {
        let models = tempfile::tempdir().unwrap();
        let pdir = tempfile::tempdir().unwrap();
        let db = Db::open_in_memory().unwrap();
        let f = db.add_folder(pdir.path().to_str().unwrap()).unwrap();
        for i in 0..photos {
            let p = pdir.path().join(format!("{i}.png"));
            RgbImage::from_pixel(8, 8, Rgb([250, i as u8, 0])).save(&p).unwrap(); // each different: identical files are duplicates
            db.upsert_photo(f, p.to_str().unwrap(), 1, 1, &PhotoMeta::default()).unwrap();
        }
        let data = b"model-bytes".to_vec();
        let dl = Arc::new(FakeDl { data: data.clone(), fail: AtomicBool::new(fail_download) });
        let events = Arc::new(Mutex::new(Vec::new()));
        let ev = events.clone();
        let svc = PeopleService::start(PeopleConfig {
            db: db.clone(),
            version: "fake-faces-1".into(),
            dir: models.path().to_path_buf(),
            files: files(&data),
            downloader: dl.clone(),
            loader,
            pipeline: pipeline(&db, pdir.path()),
            on_status: Arc::new(move |s| ev.lock().unwrap().push(s)),
        });
        Rig { svc, db, dl, events, _models: models, _photos: pdir }
    }

    fn fake_loader() -> FaceLoader {
        Arc::new(|_| Ok(Arc::new(FakeFaceEngine) as Arc<dyn FaceEngine>))
    }

    #[test]
    fn downloads_loads_and_scans_everything() {
        let r = rig(3, false, fake_loader());
        assert!(wait_until(5000, || r.svc.status().models == "ready"));
        assert!(wait_until(5000, || r.db.face_counts("fake-faces-1").unwrap() == (3, 3)));
        let s = r.svc.status();
        assert_eq!((s.done, s.total, s.error.clone()), (3, 3, None));
        assert!(r.events.lock().unwrap().iter().any(|e| e.models == "downloading"));
        assert!(wait_until(5000, || r.db.face_overview().unwrap().groups == 1)); // three photos of the same "person": one group to name
    }

    #[test]
    fn failed_download_reports_error_and_retry_recovers() {
        let r = rig(1, true, fake_loader());
        assert!(wait_until(5000, || r.svc.status().models == "error"));
        assert!(r.svc.status().error.unwrap().contains("offline"));
        r.dl.fail.store(false, Ordering::SeqCst);
        r.svc.retry();
        assert!(wait_until(5000, || r.svc.status().models == "ready"));
        assert_eq!(r.svc.status().error, None);
    }

    #[test]
    fn a_panicking_loader_is_an_error_and_retry_can_run_again() {
        let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let c = calls.clone();
        let r = rig(
            1,
            false,
            Arc::new(move |_| {
                if c.fetch_add(1, Ordering::SeqCst) == 0 {
                    panic!("runtime failed to start");
                }
                Ok(Arc::new(FakeFaceEngine) as Arc<dyn FaceEngine>)
            }),
        );
        assert!(wait_until(5000, || r.svc.status().models == "error"));
        r.svc.retry();
        assert!(wait_until(5000, || r.svc.status().models == "ready"));
    }

    #[test]
    fn pause_requested_before_the_models_are_ready_holds_the_scan_until_resumed() {
        let r = rig(2, false, fake_loader());
        r.svc.set_paused(true);
        assert!(r.svc.status().paused);
        assert!(wait_until(5000, || r.svc.status().models == "ready"));
        std::thread::sleep(Duration::from_millis(150));
        assert_eq!(r.db.face_counts("fake-faces-1").unwrap().0, 0);
        r.svc.set_paused(false);
        assert!(wait_until(5000, || r.db.face_counts("fake-faces-1").unwrap() == (2, 2)));
    }
}
