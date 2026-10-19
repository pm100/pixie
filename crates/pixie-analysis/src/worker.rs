use crate::embedder::Embedder;
use crate::tags::{TagIndex, TAGS_VERSION, VOCAB};
use pixie_image::decode::decode_scaled;
use pixie_index::Db;
use std::path::Path;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

/// Longest edge of the picture handed to the model; CLIP looks at 224 px, so a little more is plenty.
const DECODE_EDGE: u32 = 256;
const BATCH: i64 = 32;
const REPORT_EVERY: usize = 8;
const POLL: Duration = Duration::from_millis(500);
/// How long an idle worker sleeps before looking for new photos on its own.
const IDLE_WAIT: Duration = Duration::from_secs(5);
/// This many photos in a row failing in the embedder (not in decoding) means the embedder is the problem.
const EMBEDDER_FAILURES: usize = 3;

enum Outcome {
    Vector(Vec<f32>),
    /// The file cannot be decoded: it gets a failure marker and is not offered again.
    Undecodable,
    EmbedderFailed(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Progress {
    pub done: i64,
    pub total: i64,
    pub paused: bool,
    /// True while it is working through photos (false when finished, paused or waiting for a scan).
    pub busy: bool,
    /// The embedder is failing (not the photos): analysis keeps retrying, and this says why.
    pub error: Option<String>,
}

pub type ScanningFn = Arc<dyn Fn() -> bool + Send + Sync>;
pub type ProgressFn = Arc<dyn Fn(Progress) + Send + Sync>;

struct Shared {
    db: Db,
    embedder: Arc<dyn Embedder>,
    scanning: ScanningFn,
    on_progress: ProgressFn,
    poll: Duration,
    vocab: Vec<String>,
    paused: AtomicBool,
    stop: AtomicBool,
    hold_reported: AtomicBool,
    error: Mutex<Option<String>>,
    kicked: Mutex<bool>,
    wake: Condvar,
}

impl Shared {
    fn report(&self, busy: bool) {
        if let Ok((done, total)) = self.db.embedding_counts(self.embedder.version()) {
            let error = self.error.lock().unwrap_or_else(|p| p.into_inner()).clone();
            (self.on_progress)(Progress { done, total, paused: self.paused.load(Ordering::SeqCst), busy, error });
        }
    }

    /// Sleep up to `d`, or until kicked or stopped.
    fn wait(&self, d: Duration) {
        let mut k = self.kicked.lock().unwrap_or_else(|p| p.into_inner());
        if !*k && !self.stop.load(Ordering::SeqCst) {
            k = self.wake.wait_timeout(k, d).unwrap_or_else(|p| p.into_inner()).0;
        }
        *k = false;
    }

    fn holding(&self) -> bool {
        self.paused.load(Ordering::SeqCst) || (self.scanning)()
    }

    /// A decoder or model that panics is a failure of that photo or of the embedder, never of this thread.
    fn embed_one(&self, path: &Path) -> Outcome {
        let img = match catch_unwind(AssertUnwindSafe(|| decode_scaled(path, DECODE_EDGE))) {
            Ok(Ok(img)) => img,
            _ => return Outcome::Undecodable,
        };
        match catch_unwind(AssertUnwindSafe(|| self.embedder.embed_image(&img))) {
            Ok(Ok(v)) => Outcome::Vector(v),
            Ok(Err(e)) => Outcome::EmbedderFailed(e),
            Err(_) => Outcome::EmbedderFailed("the search model crashed".into()),
        }
    }

    /// Tag every stored vector that has not been tagged with the current vocabulary. Pure arithmetic on what is
    /// already in the database, so it is quick and never touches an image file.
    fn tag_pass(&self, index: &mut Option<TagIndex>, version: &str) {
        if index.is_none() {
            let labels: Vec<&str> = self.vocab.iter().map(String::as_str).collect();
            match TagIndex::build(&*self.embedder, &labels) {
                Ok(i) => *index = Some(i),
                Err(e) => return eprintln!("[analysis] cannot prepare tags: {e}"),
            }
        }
        let Some(idx) = index.as_ref() else { return };
        while !self.stop.load(Ordering::SeqCst) {
            let rows = match self.db.embeddings_needing_tags(version, TAGS_VERSION, 500) {
                Ok(r) if !r.is_empty() => r,
                _ => return,
            };
            for (id, vector) in rows {
                if let Err(e) = self.db.set_tags(id, version, TAGS_VERSION, &idx.tags_for(&vector)) {
                    eprintln!("[analysis] cannot store tags for {id}: {e}");
                }
            }
        }
    }

    fn set_error(&self, e: Option<String>) {
        *self.error.lock().unwrap_or_else(|p| p.into_inner()) = e;
    }

    fn run(&self) {
        let version = self.embedder.version().to_string();
        let mut failures = 0; // photos in a row the embedder failed on, across batches
        let mut tag_index: Option<TagIndex> = None;
        while !self.stop.load(Ordering::SeqCst) {
            if self.holding() {
                if !self.hold_reported.swap(true, Ordering::SeqCst) {
                    self.report(false); // once, when it starts waiting; not on every poll
                }
                self.wait(self.poll);
                continue;
            }
            self.hold_reported.store(false, Ordering::SeqCst);
            self.tag_pass(&mut tag_index, &version);
            let batch = match self.db.photos_needing_embedding(&version, BATCH) {
                Ok(b) => b,
                Err(e) => {
                    eprintln!("[analysis] cannot list photos: {e}");
                    self.wait(IDLE_WAIT);
                    continue;
                }
            };
            if batch.is_empty() {
                self.report(false);
                self.wait(IDLE_WAIT);
                continue;
            }
            for (i, p) in batch.iter().enumerate() {
                if self.stop.load(Ordering::SeqCst) || self.holding() {
                    break;
                }
                let stored = match self.embed_one(Path::new(&p.path)) {
                    Outcome::Vector(v) => {
                        failures = 0;
                        self.set_error(None);
                        self.db.put_embedding(p.id, &version, &v)
                    }
                    Outcome::Undecodable => self.db.put_embedding_failed(p.id, &version),
                    Outcome::EmbedderFailed(e) => {
                        // Not the photo's fault: leave it queued, and back off if the embedder keeps failing.
                        failures += 1;
                        if failures >= EMBEDDER_FAILURES {
                            eprintln!("[analysis] embedder failing: {e}");
                            self.set_error(Some(e));
                            self.report(false);
                            self.wait(self.poll * 10);
                            break;
                        }
                        continue;
                    }
                };
                if let Err(e) = stored {
                    // The photo vanished mid-run (folder removed): nothing to keep.
                    eprintln!("[analysis] cannot store result for {}: {e}", p.path);
                }
                if i % REPORT_EVERY == REPORT_EVERY - 1 {
                    self.report(true);
                }
            }
            self.report(true);
        }
    }
}

/// Embeds photos in the background, one at a time, newest first. Pauses itself while a scan runs.
pub struct Worker {
    shared: Arc<Shared>,
    handle: Mutex<Option<JoinHandle<()>>>,
}

impl Worker {
    pub fn start(db: Db, embedder: Arc<dyn Embedder>, scanning: ScanningFn, on_progress: ProgressFn) -> Worker {
        Self::start_with(db, embedder, scanning, on_progress, POLL)
    }

    pub fn start_with(db: Db, embedder: Arc<dyn Embedder>, scanning: ScanningFn, on_progress: ProgressFn, poll: Duration) -> Worker {
        Self::start_full(db, embedder, scanning, on_progress, poll, VOCAB)
    }

    /// `start_with` and a choice of tag vocabulary.
    pub fn start_full(db: Db, embedder: Arc<dyn Embedder>, scanning: ScanningFn, on_progress: ProgressFn, poll: Duration, vocab: &[&str]) -> Worker {
        let shared = Arc::new(Shared {
            db,
            embedder,
            scanning,
            on_progress,
            poll,
            vocab: vocab.iter().map(|l| l.to_string()).collect(),
            paused: AtomicBool::new(false),
            stop: AtomicBool::new(false),
            hold_reported: AtomicBool::new(false),
            error: Mutex::new(None),
            kicked: Mutex::new(false),
            wake: Condvar::new(),
        });
        let s = shared.clone();
        let handle = std::thread::spawn(move || s.run());
        Worker { shared, handle: Mutex::new(Some(handle)) }
    }

    pub fn set_paused(&self, paused: bool) {
        self.shared.paused.store(paused, Ordering::SeqCst);
        self.kick();
        self.shared.report(false);
    }

    /// Tell an idle worker to look for new photos now.
    pub fn kick(&self) {
        *self.shared.kicked.lock().unwrap_or_else(|p| p.into_inner()) = true;
        self.shared.wake.notify_all();
    }

    pub fn stop(&self) {
        self.shared.stop.store(true, Ordering::SeqCst);
        self.kick();
        if let Some(h) = self.handle.lock().unwrap_or_else(|p| p.into_inner()).take() {
            let _ = h.join();
        }
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::embedder::FakeEmbedder;
    use image::{Rgb, RgbImage};
    use pixie_core::PhotoMeta;
    use std::sync::atomic::AtomicI64;
    use std::time::Instant;

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

    /// A library of PNG files (`colours` = red channel of each) plus any extra raw files given as (name, bytes).
    fn library(reds: &[u8], junk: &[&str]) -> (tempfile::TempDir, Db) {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open_in_memory().unwrap();
        let f = db.add_folder(dir.path().to_str().unwrap()).unwrap();
        for (i, r) in reds.iter().enumerate() {
            let path = dir.path().join(format!("p{i}.png"));
            RgbImage::from_pixel(16, 16, Rgb([*r, 0, 0])).save(&path).unwrap();
            db.upsert_photo(f, path.to_str().unwrap(), 1, 1, &PhotoMeta::default()).unwrap();
        }
        for name in junk {
            let path = dir.path().join(name);
            std::fs::write(&path, b"not an image").unwrap();
            db.upsert_photo(f, path.to_str().unwrap(), 1, 1, &PhotoMeta::default()).unwrap();
        }
        (dir, db)
    }

    fn start(db: &Db, scanning: Arc<AtomicBool>, last: Arc<Mutex<Option<Progress>>>) -> Worker {
        Worker::start_with(
            db.clone(),
            Arc::new(FakeEmbedder),
            Arc::new(move || scanning.load(Ordering::SeqCst)),
            Arc::new(move |p| *last.lock().unwrap() = Some(p)),
            Duration::from_millis(10),
        )
    }

    fn idle() -> (Arc<AtomicBool>, Arc<Mutex<Option<Progress>>>) {
        (Arc::new(AtomicBool::new(false)), Arc::new(Mutex::new(None)))
    }

    /// Works like the fake until told to fail: mode 0 = fine, 1 = returns an error, 2 = panics.
    struct Flaky(std::sync::atomic::AtomicU8);

    impl Embedder for Flaky {
        fn embed_image(&self, img: &image::DynamicImage) -> Result<Vec<f32>, String> {
            match self.0.load(Ordering::SeqCst) {
                1 => Err("out of memory".into()),
                2 => panic!("runtime fault"),
                _ => FakeEmbedder.embed_image(img),
            }
        }
        fn embed_text(&self, t: &str) -> Result<Vec<f32>, String> {
            FakeEmbedder.embed_text(t)
        }
        fn version(&self) -> &str {
            "fake-1"
        }
    }

    fn start_flaky(db: &Db, e: Arc<Flaky>, last: Arc<Mutex<Option<Progress>>>) -> Worker {
        Worker::start_with(db.clone(), e, Arc::new(|| false), Arc::new(move |p| *last.lock().unwrap() = Some(p)), Duration::from_millis(10))
    }

    /// Red-ish pictures and the word "reddish" share one axis, blue-ish ones the other.
    struct Colour;

    impl Embedder for Colour {
        fn embed_image(&self, img: &image::DynamicImage) -> Result<Vec<f32>, String> {
            FakeEmbedder.embed_image(img)
        }
        fn embed_text(&self, t: &str) -> Result<Vec<f32>, String> {
            Ok(if t.contains("reddish") { vec![1.0, 0.0] } else if t.contains("bluish") { vec![0.0, 1.0] } else { vec![0.0, 0.0] }) // decoys match nothing
        }
        fn version(&self) -> &str {
            "fake-1"
        }
    }

    fn start_tagging(db: &Db) -> Worker {
        Worker::start_full(
            db.clone(),
            Arc::new(Colour),
            Arc::new(|| false),
            Arc::new(|_| {}),
            Duration::from_millis(10),
            &["reddish", "bluish"],
        )
    }

    #[test]
    fn photos_get_tags_after_they_are_embedded() {
        let (_d, db) = library(&[250, 250, 5], &[]);
        let w = start_tagging(&db);
        assert!(wait_until(5000, || db.tag_counts().unwrap() == vec![("reddish".to_string(), 2), ("bluish".to_string(), 1)]));
        w.stop();
    }

    #[test]
    fn already_embedded_photos_are_tagged_from_the_stored_vector_without_reading_the_file() {
        let db = Db::open_in_memory().unwrap();
        let f = db.add_folder("/nowhere").unwrap();
        let id = db.upsert_photo(f, "/nowhere/gone.jpg", 1, 1, &PhotoMeta::default()).unwrap();
        db.put_embedding(id, "fake-1", &[0.0, 1.0]).unwrap(); // embedded earlier; the file is not even readable now
        let w = start_tagging(&db);
        assert!(wait_until(5000, || db.tags_for(id).unwrap().first().map(|t| t.0.clone()) == Some("bluish".to_string())));
        assert_eq!(db.embedding_counts("fake-1").unwrap().0, 1); // untouched
        w.stop();
    }

    #[test]
    fn a_new_vocabulary_version_retags_everything() {
        let (_d, db) = library(&[250], &[]);
        let w = start_tagging(&db);
        assert!(wait_until(5000, || db.tag_counts().unwrap().len() == 1));
        w.stop();
        let id = db.list_photos(0, 1).unwrap()[0].id;
        db.set_tags(id, "fake-1", "old-vocabulary", &[("stale".into(), 1.0)]).unwrap();
        let w = start_tagging(&db);
        assert!(wait_until(5000, || db.tags_for(id).unwrap().first().map(|t| t.0.clone()) == Some("reddish".to_string())));
        w.stop();
    }

    #[test]
    fn embedder_errors_do_not_mark_photos_failed_and_are_reported() {
        let (_d, db) = library(&[250, 5, 250, 5], &[]);
        let e = Arc::new(Flaky(std::sync::atomic::AtomicU8::new(1)));
        let last = Arc::new(Mutex::new(None));
        let w = start_flaky(&db, e.clone(), last.clone());
        assert!(wait_until(3000, || last.lock().unwrap().as_ref().is_some_and(|p| p.error.is_some())));
        assert_eq!(db.embedding_counts("fake-1").unwrap().0, 0); // nothing was marked as failed
        assert_eq!(db.photos_needing_embedding("fake-1", 10).unwrap().len(), 4);
        e.0.store(0, Ordering::SeqCst); // the fault clears: the same photos are analysed
        assert!(wait_until(5000, || db.embedding_counts("fake-1").unwrap() == (4, 4)));
        assert!(wait_until(2000, || last.lock().unwrap().as_ref().is_some_and(|p| p.error.is_none())));
        w.stop();
    }

    #[test]
    fn a_panicking_embedder_does_not_kill_the_worker() {
        let (_d, db) = library(&[250, 5], &[]);
        let e = Arc::new(Flaky(std::sync::atomic::AtomicU8::new(2)));
        let last = Arc::new(Mutex::new(None));
        let w = start_flaky(&db, e.clone(), last.clone());
        assert!(wait_until(3000, || last.lock().unwrap().as_ref().is_some_and(|p| p.error.is_some())));
        e.0.store(0, Ordering::SeqCst);
        assert!(wait_until(5000, || db.embedding_counts("fake-1").unwrap() == (2, 2)));
        w.stop();
    }

    #[test]
    fn paused_worker_does_not_keep_reporting() {
        let (_d, db) = library(&[250], &[]);
        let count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let c = count.clone();
        let w = Worker::start_with(
            db.clone(),
            Arc::new(FakeEmbedder),
            Arc::new(|| true), // a scan is "running" forever
            Arc::new(move |_| {
                c.fetch_add(1, Ordering::SeqCst);
            }),
            Duration::from_millis(10),
        );
        std::thread::sleep(Duration::from_millis(300));
        assert!(count.load(Ordering::SeqCst) <= 2, "reported {} times while only waiting", count.load(Ordering::SeqCst));
        w.stop();
    }

    #[test]
    fn embeds_every_photo_and_reports_done() {
        let (_d, db) = library(&[250, 250, 5], &[]);
        let (scan, last) = idle();
        let w = start(&db, scan, last.clone());
        assert!(wait_until(5000, || db.embedding_counts("fake-1").unwrap() == (3, 3)));
        assert_eq!(db.load_embeddings("fake-1").unwrap().len(), 3);
        assert!(wait_until(2000, || last.lock().unwrap().as_ref().is_some_and(|p| p.done == 3 && p.total == 3 && !p.busy)));
        w.stop();
    }

    #[test]
    fn undecodable_file_is_marked_failed_once_and_others_still_embedded() {
        let (_d, db) = library(&[250, 5], &["broken.jpg"]);
        let (scan, last) = idle();
        let w = start(&db, scan, last);
        assert!(wait_until(5000, || db.photos_needing_embedding("fake-1", 10).unwrap().is_empty()));
        assert_eq!(db.load_embeddings("fake-1").unwrap().len(), 2); // the broken one has a marker but no vector
        assert_eq!(db.embedding_counts("fake-1").unwrap(), (3, 3));
        w.stop();
    }

    #[test]
    fn pause_holds_work_until_resumed() {
        let (_d, db) = library(&[250, 5], &[]);
        let (scan, last) = idle();
        let w = start(&db, scan, last);
        w.set_paused(true);
        std::thread::sleep(Duration::from_millis(100)); // let a started item finish
        let at_pause = db.embedding_counts("fake-1").unwrap().0;
        std::thread::sleep(Duration::from_millis(200));
        assert_eq!(db.embedding_counts("fake-1").unwrap().0, at_pause);
        w.set_paused(false);
        assert!(wait_until(5000, || db.embedding_counts("fake-1").unwrap() == (2, 2)));
        w.stop();
    }

    #[test]
    fn paused_from_the_start_does_nothing() {
        let (_d, db) = library(&[250], &[]);
        let (scan, last) = idle();
        let w = start(&db, scan, last.clone());
        w.set_paused(true);
        w.stop();
        // Whatever happened before the pause landed, the worker is joined and consistent.
        let (done, total) = db.embedding_counts("fake-1").unwrap();
        assert!(done <= total);
        assert!(last.lock().unwrap().as_ref().map_or(true, |p| p.total == 1));
    }

    #[test]
    fn waits_while_scanning() {
        let (_d, db) = library(&[250, 5], &[]);
        let (scan, last) = idle();
        scan.store(true, Ordering::SeqCst);
        let w = start(&db, scan.clone(), last);
        std::thread::sleep(Duration::from_millis(200));
        assert_eq!(db.embedding_counts("fake-1").unwrap().0, 0);
        scan.store(false, Ordering::SeqCst);
        assert!(wait_until(5000, || db.embedding_counts("fake-1").unwrap() == (2, 2)));
        w.stop();
    }

    #[test]
    fn kick_picks_up_photos_added_later() {
        let (dir, db) = library(&[250], &[]);
        let (scan, last) = idle();
        let w = start(&db, scan, last);
        assert!(wait_until(5000, || db.embedding_counts("fake-1").unwrap() == (1, 1)));
        let f = db.list_folders().unwrap()[0].id;
        let path = dir.path().join("late.png");
        RgbImage::from_pixel(8, 8, Rgb([5, 0, 0])).save(&path).unwrap();
        db.upsert_photo(f, path.to_str().unwrap(), 1, 1, &PhotoMeta::default()).unwrap();
        w.kick();
        assert!(wait_until(5000, || db.embedding_counts("fake-1").unwrap() == (2, 2)));
        w.stop();
    }

    #[test]
    fn folder_removed_midway_does_not_panic() {
        let (_d, db) = library(&[250, 5, 250, 5, 250, 5, 250, 5], &[]);
        let calls = Arc::new(AtomicI64::new(0));
        let c = calls.clone();
        let w = Worker::start_with(
            db.clone(),
            Arc::new(FakeEmbedder),
            Arc::new(|| false),
            Arc::new(move |_| {
                c.fetch_add(1, Ordering::SeqCst);
            }),
            Duration::from_millis(10),
        );
        let f = db.list_folders().unwrap()[0].id;
        db.remove_folder(f).unwrap();
        std::thread::sleep(Duration::from_millis(200));
        w.stop(); // joins: a panic in the thread would surface here as a lost result, not hang
        assert_eq!(db.embedding_counts("fake-1").unwrap(), (0, 0));
    }
}
