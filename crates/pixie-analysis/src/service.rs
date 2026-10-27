use crate::embedder::Embedder;
use crate::models::{ensure_models, Downloader, ModelFile};
use crate::search::{self, SEARCH_FLOOR, SEARCH_LIMIT};
use crate::pipeline::PhotoPipeline;
use pixie_index::Db;
use serde::Serialize;
use std::path::{Path, PathBuf};

use std::sync::{Arc, Mutex, Weak};

pub type Loader = Arc<dyn Fn(&Path) -> Result<Arc<dyn Embedder>, String> + Send + Sync>;

/// What the UI shows about analysis. `models` is "missing", "downloading", "ready" or "error".
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AnalysisStatus {
    pub models: String,
    pub model_done: u64,
    pub model_total: u64,
    pub done: i64,
    pub total: i64,
    pub paused: bool,
    pub error: Option<String>,
}

pub struct Config {
    pub db: Db,
    /// The model version whose progress is counted (the embedder's `version()`).
    pub version: String,
    pub dir: PathBuf,
    pub files: &'static [ModelFile],
    pub downloader: Arc<dyn Downloader>,
    pub loader: Loader,
    /// The pipeline that does the work: this service gives it the model once it is loaded, and shows its progress.
    pub pipeline: Arc<PhotoPipeline>,
    pub on_status: Arc<dyn Fn(AnalysisStatus) + Send + Sync>,
}

struct State {
    models: &'static str,
    model_done: u64,
    model_total: u64,
    error: Option<String>,
    paused: bool,
    loading: bool,
    /// Auto tags are switched on (when off the models are not fetched and the pipeline makes no tags).
    enabled: bool,
    embedder: Option<Arc<dyn Embedder>>,
}

/// Every stored vector, with the (analysed, total) counts they were loaded at.
struct Cache {
    key: (i64, i64),
    items: Arc<Vec<(i64, Vec<f32>)>>,
}

struct Inner {
    cfg: Config,
    state: Mutex<State>,
    cache: Mutex<Option<Cache>>,
    loads: std::sync::atomic::AtomicUsize,
}

/// Downloads the models, loads them, runs the background embedding worker and answers searches.
pub struct AnalysisService {
    inner: Arc<Inner>,
}

impl Inner {
    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|p| p.into_inner())
    }

    fn status(&self) -> AnalysisStatus {
        let (done, total) = self.cfg.db.embedding_counts(&self.cfg.version).unwrap_or((0, 0));
        let total = total + self.cfg.pipeline.pending_photos();
        let s = self.lock();
        AnalysisStatus {
            models: s.models.to_string(),
            model_done: s.model_done,
            model_total: s.model_total,
            done,
            total,
            paused: s.paused,
            error: s.error.clone().or_else(|| self.cfg.pipeline.embed_error()),
        }
    }

    /// Never call while holding the state lock: `status` takes it.
    fn emit(&self) {
        (self.cfg.on_status)(self.status());
    }

    /// Runs `setup_inner`; whatever happens inside (even a panic from the model runtime) ends in a clear state, so
    /// the UI never sits on "downloading" and Retry always works.
    fn setup(self: &Arc<Self>) {
        if let Err(p) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| self.setup_inner())) {
            let why = p.downcast_ref::<&str>().map(|s| s.to_string()).or_else(|| p.downcast_ref::<String>().cloned());
            {
                let mut s = self.lock();
                s.loading = false;
                s.models = "error";
                s.error = Some(why.unwrap_or_else(|| "the search model could not start".into()));
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
                Ok(embedder) => {
                    s.embedder = Some(embedder.clone());
                    if s.enabled {
                        s.models = "ready";
                        self.cfg.pipeline.set_embedder(embedder); // the pipeline now picks up the photos that lack a vector
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

impl AnalysisService {
    pub fn start(cfg: Config) -> AnalysisService {
        Self::start_with(cfg, true)
    }

    /// `enabled` false: Auto tags are off to begin with, and no model is fetched until they are switched on.
    pub fn start_with(cfg: Config, enabled: bool) -> AnalysisService {
        let svc = AnalysisService {
            inner: Arc::new(Inner {
                cfg,
                state: Mutex::new(State {
                    models: if enabled { "missing" } else { "off" },
                    model_done: 0,
                    model_total: 0,
                    error: None,
                    paused: false,
                    loading: false,
                    enabled,
                    embedder: None,
                }),
                cache: Mutex::new(None),
                loads: std::sync::atomic::AtomicUsize::new(0),
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
            if !s.enabled || s.loading || s.embedder.is_some() {
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

    pub fn status(&self) -> AnalysisStatus {
        self.inner.status()
    }

    /// Try the download and load again after an error.
    pub fn retry(&self) {
        self.begin();
    }

    /// Switch Auto tags on or off. Off: the pipeline makes no more tags and no model is fetched (the tags already made are
    /// kept). On: the model is fetched if it is not yet here, and the photos without tags are picked up.
    pub fn set_enabled(&self, on: bool) {
        let embedder = {
            let mut s = self.inner.lock();
            if s.enabled == on {
                return;
            }
            s.enabled = on;
            s.models = if !on { "off" } else if s.embedder.is_some() { "ready" } else { "missing" };
            s.embedder.clone()
        };
        if !on {
            self.inner.cfg.pipeline.clear_embedder();
        } else if let Some(e) = embedder {
            self.inner.cfg.pipeline.set_embedder(e);
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

    /// How many times the stored vectors were read from the database (they are cached between searches).
    pub fn vector_loads(&self) -> usize {
        self.inner.loads.load(std::sync::atomic::Ordering::SeqCst)
    }

    /// All vectors, reloaded only when the analysed/total counts have moved since the last load.
    fn vectors(&self) -> Result<Arc<Vec<(i64, Vec<f32>)>>, String> {
        let db = &self.inner.cfg.db;
        let key = db.embedding_counts(&self.inner.cfg.version).map_err(|e| e.to_string())?;
        let mut cache = self.inner.cache.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(c) = cache.as_ref().filter(|c| c.key == key) {
            return Ok(c.items.clone());
        }
        let items = Arc::new(db.load_embeddings(&self.inner.cfg.version).map_err(|e| e.to_string())?);
        self.inner.loads.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        *cache = Some(Cache { key, items: items.clone() });
        Ok(items)
    }

    /// New photos may have arrived (a scan finished): wake an idle worker.
    pub fn kick(&self) {
        self.inner.cfg.pipeline.kick();
    }

    /// Make `text` the current search; returns how many photos match. Blank text clears the search.
    pub fn search(&self, text: &str) -> Result<usize, String> {
        let embedder = self.inner.lock().embedder.clone();
        match embedder {
            Some(e) => {
                let items = if text.trim().is_empty() { Arc::new(Vec::new()) } else { self.vectors()? };
                search::run_with(&self.inner.cfg.db, &*e, text, SEARCH_LIMIT, SEARCH_FLOOR, &items)
            }
            None if text.trim().is_empty() => {
                self.inner.cfg.db.clear_search_hits().map_err(|e| e.to_string())?;
                Ok(0)
            }
            None => Err("The search model is not ready yet.".into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use crate::embedder::FakeEmbedder;
    use crate::models::ModelFile;
    use crate::pipeline::PipelineConfig;
    use image::{Rgb, RgbImage};
    use pixie_core::PhotoMeta;
    use sha2::{Digest, Sha256};
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
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
        svc: AnalysisService,
        db: Db,
        dl: Arc<FakeDl>,
        events: Arc<Mutex<Vec<AnalysisStatus>>>,
        loads: Arc<AtomicUsize>,
        _models: tempfile::TempDir,
        _photos: tempfile::TempDir,
    }

    fn rig(photos: usize, fail_download: bool) -> Rig {
        rig_with(photos, fail_download, true)
    }

    fn rig_with(photos: usize, fail_download: bool, enabled: bool) -> Rig {
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
        let loads = Arc::new(AtomicUsize::new(0));
        let (ev, ld) = (events.clone(), loads.clone());
        let svc = AnalysisService::start_with(Config {
            db: db.clone(),
            version: "fake-1".into(),
            dir: models.path().to_path_buf(),
            files: files(&data),
            downloader: dl.clone(),
            loader: Arc::new(move |_dir| {
                ld.fetch_add(1, Ordering::SeqCst);
                Ok(Arc::new(FakeEmbedder) as Arc<dyn Embedder>)
            }),
            pipeline: pipeline(&db, pdir.path()),
            on_status: Arc::new(move |s| ev.lock().unwrap().push(s)),
        }, enabled);
        Rig { svc, db, dl, events, loads, _models: models, _photos: pdir }
    }

    #[test]
    fn downloads_loads_and_analyses_everything() {
        let r = rig(3, false);
        assert!(wait_until(5000, || r.svc.status().models == "ready"));
        assert!(wait_until(5000, || r.db.embedding_counts("fake-1").unwrap() == (3, 3)));
        let s = r.svc.status();
        assert_eq!((s.done, s.total, s.error.clone()), (3, 3, None));
        assert!(r.events.lock().unwrap().iter().any(|e| e.models == "downloading"));
        assert_eq!(r.loads.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn switched_off_it_fetches_no_model_and_analyses_nothing_until_switched_on() {
        let r = rig_with(2, false, false);
        assert_eq!(r.svc.status().models, "off");
        std::thread::sleep(Duration::from_millis(150));
        assert_eq!(r.loads.load(Ordering::SeqCst), 0, "no model was loaded");
        assert_eq!(r.db.embedding_counts("fake-1").unwrap().0, 0);
        r.svc.set_enabled(true);
        assert!(wait_until(5000, || r.svc.status().models == "ready"));
        assert!(wait_until(5000, || r.db.embedding_counts("fake-1").unwrap() == (2, 2)));
    }

    #[test]
    fn switching_off_stops_new_analysis_and_on_again_carries_on_with_the_model_already_loaded() {
        let r = rig(2, false);
        assert!(wait_until(5000, || r.db.embedding_counts("fake-1").unwrap() == (2, 2)));
        r.svc.set_enabled(false);
        assert_eq!(r.svc.status().models, "off");
        let folder = r.db.list_folders().unwrap()[0].id;
        let late = r._photos.path().join("late.png");
        RgbImage::from_pixel(8, 8, Rgb([5, 250, 5])).save(&late).unwrap();
        r.db.upsert_photo(folder, late.to_str().unwrap(), 1, 1, &PhotoMeta::default()).unwrap();
        std::thread::sleep(Duration::from_millis(200));
        assert_eq!(r.db.embedding_counts("fake-1").unwrap(), (2, 3), "the new photo is left alone while off");
        r.svc.set_enabled(true);
        assert_eq!(r.svc.status().models, "ready", "ready at once: the model is still loaded");
        assert!(wait_until(5000, || r.db.embedding_counts("fake-1").unwrap() == (3, 3)));
        assert_eq!(r.loads.load(Ordering::SeqCst), 1, "the model was not loaded again");
    }

    #[test]
    fn failed_download_reports_error_and_retry_recovers() {
        let r = rig(1, true);
        assert!(wait_until(5000, || r.svc.status().models == "error"));
        assert!(r.svc.status().error.unwrap().contains("offline"));
        assert!(r.svc.search("cat").is_err()); // not ready: a clear error, not a crash
        r.dl.fail.store(false, Ordering::SeqCst);
        r.svc.retry();
        assert!(wait_until(5000, || r.svc.status().models == "ready"));
        assert_eq!(r.svc.status().error, None);
    }

    #[test]
    fn pause_survives_until_resumed() {
        let r = rig(2, false);
        r.svc.set_paused(true);
        assert!(r.svc.status().paused);
        assert!(wait_until(5000, || r.svc.status().models == "ready"));
        std::thread::sleep(Duration::from_millis(150));
        assert_eq!(r.db.embedding_counts("fake-1").unwrap().0, 0); // paused before the models were ready, so nothing ran
        r.svc.set_paused(false);
        assert!(wait_until(5000, || r.db.embedding_counts("fake-1").unwrap() == (2, 2)));
    }

    struct Failing;
    impl Embedder for Failing {
        fn embed_image(&self, _: &image::DynamicImage) -> Result<Vec<f32>, String> {
            Err("no memory".into())
        }
        fn embed_text(&self, t: &str) -> Result<Vec<f32>, String> {
            FakeEmbedder.embed_text(t)
        }
        fn version(&self) -> &str {
            "fake-1"
        }
    }

    fn rig_with_loader(photos: usize, loader: Loader) -> (AnalysisService, Db, tempfile::TempDir, tempfile::TempDir) {
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
        let svc = AnalysisService::start(Config {
            db: db.clone(),
            version: "fake-1".into(),
            dir: models.path().to_path_buf(),
            files: files(&data),
            downloader: Arc::new(FakeDl { data, fail: AtomicBool::new(false) }),
            loader,
            pipeline: pipeline(&db, pdir.path()),
            on_status: Arc::new(|_| {}),
        });
        (svc, db, models, pdir)
    }

    #[test]
    fn a_failing_embedder_shows_up_as_a_status_error_not_a_silent_stall() {
        let (svc, db, _m, _p) = rig_with_loader(3, Arc::new(|_| Ok(Arc::new(Failing) as Arc<dyn Embedder>)));
        assert!(wait_until(5000, || svc.status().error.is_some()));
        let s = svc.status();
        assert_eq!(s.models, "ready");
        assert!(s.error.unwrap().contains("no memory"));
        assert_eq!(db.embedding_counts("fake-1").unwrap().0, 0);
    }

    #[test]
    fn a_panicking_loader_is_an_error_and_retry_can_run_again() {
        let calls = Arc::new(AtomicUsize::new(0));
        let c = calls.clone();
        let (svc, _db, _m, _p) = rig_with_loader(
            1,
            Arc::new(move |_| {
                if c.fetch_add(1, Ordering::SeqCst) == 0 {
                    panic!("runtime failed to start");
                }
                Ok(Arc::new(FakeEmbedder) as Arc<dyn Embedder>)
            }),
        );
        assert!(wait_until(5000, || svc.status().models == "error"));
        svc.retry();
        assert!(wait_until(5000, || svc.status().models == "ready"));
    }

    #[test]
    fn repeated_searches_reuse_the_loaded_vectors_until_the_library_changes() {
        let (svc, db, _m, _p) = rig_with_loader(2, Arc::new(|_| Ok(Arc::new(FakeEmbedder) as Arc<dyn Embedder>)));
        assert!(wait_until(5000, || db.embedding_counts("fake-1").unwrap() == (2, 2)));
        svc.search("cat").unwrap();
        svc.search("dog").unwrap();
        svc.search("cat").unwrap();
        assert_eq!(svc.vector_loads(), 1);
        let f = db.list_folders().unwrap()[0].id;
        db.remove_folder(f).unwrap(); // the library changed: the next search must not use old vectors
        assert_eq!(svc.search("cat").unwrap(), 0);
        assert_eq!(svc.vector_loads(), 2);
    }

    #[test]
    fn search_works_once_ready_and_reports_hit_count() {
        let r = rig(2, false);
        assert!(wait_until(5000, || r.db.embedding_counts("fake-1").unwrap() == (2, 2)));
        assert_eq!(r.svc.search("cat").unwrap(), 2); // red photos are on the "cat" side of the fake space
        assert_eq!(r.svc.search("dog").unwrap(), 0);
        assert_eq!(r.svc.search("  ").unwrap(), 0);
    }
}
