use crate::cluster::JOIN_THRESHOLD;
use crate::faces::FaceEngine;
use crate::people::assign_new_faces;
use crate::worker::ScanningFn;
use pixie_core::NewFace;
use pixie_image::decode::decode_scaled;
use pixie_index::Db;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

/// Faces can be small, so the photo is read larger than CLIP needs (the detector then works at 640 px).
const DECODE_EDGE: u32 = 1024;
const BATCH: i64 = 16;
const REPORT_EVERY: usize = 4;
const POLL: Duration = Duration::from_millis(500);
/// How long an idle worker sleeps before looking for new photos on its own.
const IDLE_WAIT: Duration = Duration::from_secs(5);
/// This many photos in a row failing in the engine (not in decoding) means the engine is the problem.
const ENGINE_FAILURES: usize = 3;

#[derive(Debug, Clone, PartialEq)]
pub struct FaceProgress {
    pub done: i64,
    pub total: i64,
    pub paused: bool,
    /// True while it is working through photos (false when finished, paused or waiting for a scan).
    pub busy: bool,
    /// The engine is failing (not the photos): scanning keeps retrying, and this says why.
    pub error: Option<String>,
}

pub type FaceProgressFn = Arc<dyn Fn(FaceProgress) + Send + Sync>;

enum Outcome {
    Faces(Vec<NewFace>),
    /// The file cannot be decoded: it is marked scanned (with no faces) and not offered again.
    Undecodable,
    EngineFailed(String),
}

struct Shared {
    db: Db,
    engine: Arc<dyn FaceEngine>,
    scanning: ScanningFn,
    on_progress: FaceProgressFn,
    poll: Duration,
    paused: AtomicBool,
    stop: AtomicBool,
    hold_reported: AtomicBool,
    error: Mutex<Option<String>>,
    kicked: Mutex<bool>,
    wake: Condvar,
}

impl Shared {
    fn report(&self, busy: bool) {
        if let Ok((done, total)) = self.db.face_counts(self.engine.version()) {
            let error = self.error.lock().unwrap_or_else(|p| p.into_inner()).clone();
            (self.on_progress)(FaceProgress { done, total, paused: self.paused.load(Ordering::SeqCst), busy, error });
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

    fn set_error(&self, e: Option<String>) {
        *self.error.lock().unwrap_or_else(|p| p.into_inner()) = e;
    }

    /// A decoder or model that panics is a failure of that photo or of the engine, never of this thread.
    fn scan_one(&self, path: &Path) -> Outcome {
        let img = match catch_unwind(AssertUnwindSafe(|| decode_scaled(path, DECODE_EDGE))) {
            Ok(Ok(img)) => img,
            _ => return Outcome::Undecodable,
        };
        let found = catch_unwind(AssertUnwindSafe(|| -> Result<Vec<NewFace>, String> {
            let mut out = Vec::new();
            for face in self.engine.detect(&img)? {
                let embedding = self.engine.embed(&img, &face)?;
                out.push(NewFace { x: face.x, y: face.y, w: face.w, h: face.h, score: face.score, embedding });
            }
            Ok(out)
        }));
        match found {
            Ok(Ok(faces)) => Outcome::Faces(faces),
            Ok(Err(e)) => Outcome::EngineFailed(e),
            Err(_) => Outcome::EngineFailed("the face model crashed".into()),
        }
    }

    fn run(&self) {
        let version = self.engine.version().to_string();
        let mut failures = 0; // photos in a row the engine failed on, across batches
        while !self.stop.load(Ordering::SeqCst) {
            if self.holding() {
                if !self.hold_reported.swap(true, Ordering::SeqCst) {
                    self.report(false); // once, when it starts waiting; not on every poll
                }
                self.wait(self.poll);
                continue;
            }
            self.hold_reported.store(false, Ordering::SeqCst);
            let batch = match self.db.photos_needing_faces(&version, BATCH) {
                Ok(b) => b,
                Err(e) => {
                    eprintln!("[people] cannot list photos: {e}");
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
                let stored = match self.scan_one(Path::new(&p.path)) {
                    Outcome::Faces(faces) => {
                        failures = 0;
                        self.set_error(None);
                        let r = self.db.replace_faces(p.id, &version, &faces);
                        if r.is_ok() && !faces.is_empty() {
                            if let Err(e) = assign_new_faces(&self.db, JOIN_THRESHOLD) {
                                eprintln!("[people] cannot group faces: {e}");
                            }
                        }
                        r.map(|_| ())
                    }
                    Outcome::Undecodable => self.db.replace_faces(p.id, &version, &[]).map(|_| ()),
                    Outcome::EngineFailed(e) => {
                        // Not the photo's fault: leave it queued, and back off if the engine keeps failing.
                        failures += 1;
                        if failures >= ENGINE_FAILURES {
                            eprintln!("[people] face engine failing: {e}");
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
                    eprintln!("[people] cannot store result for {}: {e}", p.path);
                }
                if i % REPORT_EVERY == REPORT_EVERY - 1 {
                    self.report(true);
                }
            }
            self.report(true);
        }
    }
}

/// Looks for faces in photos in the background, newest first, and groups them into people. Pauses itself while a
/// scan runs.
pub struct FaceWorker {
    shared: Arc<Shared>,
    handle: Mutex<Option<JoinHandle<()>>>,
}

impl FaceWorker {
    pub fn start(db: Db, engine: Arc<dyn FaceEngine>, scanning: ScanningFn, on_progress: FaceProgressFn) -> FaceWorker {
        Self::start_with(db, engine, scanning, on_progress, POLL)
    }

    pub fn start_with(db: Db, engine: Arc<dyn FaceEngine>, scanning: ScanningFn, on_progress: FaceProgressFn, poll: Duration) -> FaceWorker {
        let shared = Arc::new(Shared {
            db,
            engine,
            scanning,
            on_progress,
            poll,
            paused: AtomicBool::new(false),
            stop: AtomicBool::new(false),
            hold_reported: AtomicBool::new(false),
            error: Mutex::new(None),
            kicked: Mutex::new(false),
            wake: Condvar::new(),
        });
        let s = shared.clone();
        let handle = std::thread::spawn(move || s.run());
        FaceWorker { shared, handle: Mutex::new(Some(handle)) }
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

impl Drop for FaceWorker {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::faces::FakeFaceEngine;
    use crate::yunet::Face;
    use image::{Rgb, RgbImage};
    use pixie_core::PhotoMeta;
    use std::sync::atomic::AtomicUsize;
    use std::time::Instant;

    const V: &str = "fake-faces-1";

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

    /// PNG photos whose top-left red channel is each of `reds` (the fake engine finds a face above 128), plus junk files.
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

    fn start(db: &Db, engine: Arc<dyn FaceEngine>, scanning: Arc<AtomicBool>, last: Arc<Mutex<Option<FaceProgress>>>) -> FaceWorker {
        FaceWorker::start_with(
            db.clone(),
            engine,
            Arc::new(move || scanning.load(Ordering::SeqCst)),
            Arc::new(move |p| *last.lock().unwrap() = Some(p)),
            Duration::from_millis(10),
        )
    }

    fn idle() -> (Arc<AtomicBool>, Arc<Mutex<Option<FaceProgress>>>) {
        (Arc::new(AtomicBool::new(false)), Arc::new(Mutex::new(None)))
    }

    #[test]
    fn photos_with_faces_get_people_and_photos_without_are_still_marked_scanned() {
        let (_d, db) = library(&[250, 250, 5], &[]);
        let (scan, last) = idle();
        let w = start(&db, Arc::new(FakeFaceEngine), scan, last.clone());
        assert!(wait_until(5000, || db.face_counts(V).unwrap() == (3, 3)));
        // The scan row is written just before the photo's faces are grouped, so give the grouping a moment.
        assert!(wait_until(5000, || db.people_centroids().unwrap().iter().map(|c| c.2).collect::<Vec<_>>() == vec![2])); // both red photos hold the same "person"
        assert!(wait_until(2000, || last.lock().unwrap().as_ref().is_some_and(|p| p.done == 3 && !p.busy)));
        w.stop();
    }

    #[test]
    fn an_undecodable_file_is_marked_scanned_once_and_the_others_still_work() {
        let (_d, db) = library(&[250], &["broken.png"]);
        let (scan, last) = idle();
        let w = start(&db, Arc::new(FakeFaceEngine), scan, last);
        assert!(wait_until(5000, || db.face_counts(V).unwrap() == (2, 2)));
        assert!(db.photos_needing_faces(V, 10).unwrap().is_empty());
        w.stop();
    }

    struct Flaky {
        panic: bool,
        calls: AtomicUsize,
    }
    impl FaceEngine for Flaky {
        fn detect(&self, _: &image::DynamicImage) -> Result<Vec<Face>, String> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            if self.panic {
                panic!("runtime blew up");
            }
            Err("out of memory".into())
        }
        fn embed(&self, _: &image::DynamicImage, _: &Face) -> Result<Vec<f32>, String> {
            Err("unused".into())
        }
        fn version(&self) -> &str {
            V
        }
    }

    #[test]
    fn engine_errors_and_panics_leave_photos_queued_and_are_reported() {
        for panic in [false, true] {
            let (_d, db) = library(&[250, 250, 250, 250], &[]);
            let (scan, last) = idle();
            let w = start(&db, Arc::new(Flaky { panic, calls: AtomicUsize::new(0) }), scan, last.clone());
            assert!(wait_until(5000, || last.lock().unwrap().as_ref().is_some_and(|p| p.error.is_some())), "panic={panic}");
            assert_eq!(db.face_counts(V).unwrap().0, 0); // nothing marked scanned
            w.stop(); // and the worker thread is still alive to be stopped
        }
    }

    #[test]
    fn pause_and_scan_hold_the_work_and_resume_continues() {
        let (_d, db) = library(&[250, 5], &[]);
        let (scan, last) = idle();
        scan.store(true, Ordering::SeqCst);
        let w = start(&db, Arc::new(FakeFaceEngine), scan.clone(), last);
        std::thread::sleep(Duration::from_millis(150));
        assert_eq!(db.face_counts(V).unwrap().0, 0); // a scan is running
        scan.store(false, Ordering::SeqCst);
        w.set_paused(true);
        std::thread::sleep(Duration::from_millis(100));
        let at_pause = db.face_counts(V).unwrap().0;
        std::thread::sleep(Duration::from_millis(150));
        assert_eq!(db.face_counts(V).unwrap().0, at_pause);
        w.set_paused(false);
        assert!(wait_until(5000, || db.face_counts(V).unwrap() == (2, 2)));
        w.stop();
    }

    #[test]
    fn a_holding_worker_does_not_keep_reporting() {
        let (_d, db) = library(&[250], &[]);
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        let w = FaceWorker::start_with(
            db.clone(),
            Arc::new(FakeFaceEngine),
            Arc::new(|| true),
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
    fn kick_picks_up_photos_added_later_and_scanned_photos_are_not_rescanned() {
        let (dir, db) = library(&[250], &[]);
        let (scan, last) = idle();
        let w = start(&db, Arc::new(FakeFaceEngine), scan, last);
        assert!(wait_until(5000, || db.face_counts(V).unwrap() == (1, 1)));
        let f = db.list_folders().unwrap()[0].id;
        let path = dir.path().join("late.png");
        RgbImage::from_pixel(16, 16, Rgb([250, 0, 0])).save(&path).unwrap();
        db.upsert_photo(f, path.to_str().unwrap(), 1, 1, &PhotoMeta::default()).unwrap();
        w.kick();
        assert!(wait_until(5000, || db.face_counts(V).unwrap() == (2, 2)));
        assert!(wait_until(5000, || db.people_centroids().unwrap().iter().map(|c| c.2).collect::<Vec<_>>() == vec![2])); // the first photo was not counted twice
        w.stop();
    }

    #[test]
    fn stop_returns_promptly_even_when_idle() {
        let (_d, db) = library(&[], &[]);
        let (scan, last) = idle();
        let w = start(&db, Arc::new(FakeFaceEngine), scan, last);
        std::thread::sleep(Duration::from_millis(50));
        let t = Instant::now();
        w.stop();
        assert!(t.elapsed() < Duration::from_secs(2));
    }
}
