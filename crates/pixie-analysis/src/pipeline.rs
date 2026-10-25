//! The analysis pipeline. Each photo is read once and decoded once, and every step that needs it works from that:
//!
//! 1. read the file; hash it (so duplicates are found) and tell the duplicate flags;
//! 2. decode it once, at the size faces need;
//! 3. make its grid thumbnail;
//! 4. Auto tags: the picture vector and its tags;
//! 5. faces.
//!
//! Hashing and thumbnails always happen; the Auto tags and face steps happen once their models are loaded (the services
//! hand them over with `set_embedder` and `set_face_engine`), and a photo that is missing a step is picked up again.
//! Photos are handled by several workers at once. The results come back to one thread, which stores them and groups
//! people, because grouping is not safe to run in parallel. A hidden extra copy of a duplicate is not analysed.

use crate::art::{face_is_art, ArtIndex};
use crate::embedder::Embedder;
use crate::faces::{default_workers, FaceEngine};
use crate::tags::{vocab, TagIndex, TAGS_VERSION};
use image::DynamicImage;
use pixie_core::{FolderId, MediaKind, NewFace, Photo, PhotoId, PhotoMeta};
use pixie_image::decode::decode_scaled_bytes;
use pixie_image::media::read_media_meta;
use pixie_image::meta::read_meta_bytes;
use pixie_image::thumbs::{thumbnail_from_image, ThumbClass};
use pixie_index::dupes::{full_hash_bytes, quick_key_bytes};
use pixie_index::{Candidate, Db, LatestItem, WorkItem};
use std::collections::{HashMap, HashSet, VecDeque};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{self, TrySendError};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// Faces can be small, so the photo is decoded larger than Auto tags needs (the detector then works at 640 px).
const DECODE_EDGE: u32 = 1024;
/// Auto tags looks at 224 px; its input is the photo shrunk so the shorter side is this.
const CLIP_SHORT_SIDE: u32 = 256;
/// A longer batch keeps the readers busy between listings.
const BATCH: i64 = 64;
/// Files read ahead at once. A photo on a network share takes about as long to fetch as to analyse, and the share serves
/// several readers at once (about 3 times the speed of one), so reading runs ahead of the workers on its own threads.
const READERS: usize = 8;
const REPORT_EVERY: usize = 4;
const POLL: Duration = Duration::from_millis(500);
/// How long an idle pipeline sleeps before looking for new photos on its own.
const IDLE_WAIT: Duration = Duration::from_secs(5);
/// While photos are still being scanned, faces are regrouped each time this many new ones have been found (and once
/// more when the work runs out): regrouping reads every face that is not settled, so it is not done per photo.
const TIDY_EVERY: usize = 300;
/// This many photos in a row failing in a model (not in reading or decoding) means the model is the problem.
const ENGINE_FAILURES: usize = 3;
/// What a file that could not be read gets as its quick key: it is not offered for hashing again until it changes.
const UNREADABLE: &str = "!unreadable";

/// At most this often a finished photo is named for the "now reading" strip.
const LATEST_EVERY: Duration = Duration::from_millis(100);

pub type ScanningFn = Arc<dyn Fn() -> bool + Send + Sync>;
/// Told which photo was just finished (its thumbnail exists), for the strip on the loading screen.
pub type LatestListener = Arc<dyn Fn(LatestItem) + Send + Sync>;
/// Called when progress has moved, so a service can tell the window.
pub type Reporter = Arc<dyn Fn() + Send + Sync>;

pub struct PipelineConfig {
    pub db: Db,
    /// The thumbnail cache (the folder the thumbnail service reads), so grid thumbnails are made as a by-product.
    pub thumbs_dir: PathBuf,
    pub scanning: ScanningFn,
    pub poll: Duration,
    /// How many photos are worked on at once.
    pub workers: usize,
    pub vocab: Vec<String>,
}

impl PipelineConfig {
    pub fn new(db: Db, thumbs_dir: PathBuf, scanning: ScanningFn) -> PipelineConfig {
        PipelineConfig { db, thumbs_dir, scanning, poll: POLL, workers: default_workers(), vocab: vocab().iter().map(|l| l.to_string()).collect() }
    }
}

enum Outcome {
    /// Nothing to do after all (a step was switched off while the batch ran).
    Nothing,
    /// The file cannot be read or decoded: the steps asked for get a failure marker and are not offered again.
    Unusable,
    /// A hidden extra copy of another photo: its hash is saved and that is all.
    Duplicate,
    Analysed { vector: Option<Result<Vec<f32>, String>>, faces: Option<Result<Vec<NewFace>, String>>, art: ArtVerdicts },
}

/// What looking at the faces for artwork found. A face with no verdict (the check failed) is looked at again later.
#[derive(Default)]
struct ArtVerdicts {
    /// For the faces just found, in the same order.
    new: Vec<Option<bool>>,
    /// For faces stored earlier without a verdict: (face id, artwork).
    old: Vec<(i64, bool)>,
}

/// The picture model and the descriptions that tell artwork from a person.
type ArtCheck = (Arc<dyn Embedder>, Arc<ArtIndex>);

/// Files a walk found that are not in the library yet. The pipeline reads each once and does everything with it, starting
/// with its row, so a photo appears in the library when it has been processed. Nothing is lost if the app quits: the next
/// walk finds those files again, being still new.
#[derive(Default)]
struct Intake {
    queue: VecDeque<Candidate>,
    /// Paths in the queue or being worked on (a file offered twice is read once).
    queued: HashSet<String>,
    /// Per folder, the (photos, videos) not yet stored.
    outstanding: HashMap<FolderId, (i64, i64)>,
}

struct Shared {
    intake: Mutex<Intake>,
    latest: Mutex<Option<LatestListener>>,
    latest_at: Mutex<Instant>,
    db: Db,
    thumbs_dir: PathBuf,
    scanning: ScanningFn,
    poll: Duration,
    workers: usize,
    vocab: Vec<String>,
    embedder: Mutex<Option<Arc<dyn Embedder>>>,
    faces: Mutex<Option<Arc<dyn FaceEngine>>>,
    tag_index: Mutex<Option<Arc<TagIndex>>>,
    art_index: Mutex<Option<Arc<ArtIndex>>>,
    reporters: Mutex<Vec<Reporter>>,
    embed_error: Mutex<Option<String>>,
    face_error: Mutex<Option<String>>,
    /// Faces stored since the faces were last regrouped.
    new_faces: AtomicUsize,
    paused: AtomicBool,
    stop: AtomicBool,
    hold_reported: AtomicBool,
    kicked: Mutex<bool>,
    wake: Condvar,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

/// The photo shrunk so its shorter side is `CLIP_SHORT_SIDE` (what the Auto tags model is fed), unless it is smaller.
fn for_clip(img: &DynamicImage) -> DynamicImage {
    let (w, h) = (img.width(), img.height());
    let short = w.min(h);
    if short <= CLIP_SHORT_SIDE {
        return img.clone();
    }
    let scale = CLIP_SHORT_SIDE as f32 / short as f32;
    // `thumbnail` is the fast downscale (a few ms); a filtered `resize` of the same size took 25 ms per photo.
    img.thumbnail(((w as f32 * scale).round() as u32).max(1), ((h as f32 * scale).round() as u32).max(1))
}

fn find_faces(engine: &dyn FaceEngine, img: &DynamicImage) -> Result<Vec<NewFace>, String> {
    let mut out = Vec::new();
    for face in engine.detect(img)? {
        let embedding = engine.embed(img, &face)?;
        out.push(NewFace { x: face.x, y: face.y, w: face.w, h: face.h, score: face.score, embedding });
    }
    Ok(out)
}

impl Shared {
    fn report(&self) {
        let reporters: Vec<Reporter> = lock(&self.reporters).clone();
        for r in reporters {
            r();
        }
    }

    /// Sleep up to `d`, or until kicked or stopped.
    fn wait(&self, d: Duration) {
        let mut k = lock(&self.kicked);
        if !*k && !self.stop.load(Ordering::SeqCst) {
            k = self.wake.wait_timeout(k, d).unwrap_or_else(|p| p.into_inner()).0;
        }
        *k = false;
    }

    fn holding(&self) -> bool {
        self.paused.load(Ordering::SeqCst) || (self.scanning)()
    }

    /// Tag every stored vector that has not been tagged with the current vocabulary. Pure arithmetic on what is
    /// already in the database, so it is quick and never touches an image file.
    fn tag_pass(&self, embedder: &Arc<dyn Embedder>) -> Option<Arc<TagIndex>> {
        let index = {
            let mut slot = lock(&self.tag_index);
            if slot.is_none() {
                let labels: Vec<&str> = self.vocab.iter().map(String::as_str).collect();
                match TagIndex::build(&**embedder, &labels) {
                    Ok(i) => *slot = Some(Arc::new(i)),
                    Err(e) => {
                        eprintln!("[analysis] cannot prepare tags: {e}");
                        return None;
                    }
                }
            }
            slot.clone()?
        };
        let version = embedder.version().to_string();
        while !self.stop.load(Ordering::SeqCst) {
            let rows = match self.db.embeddings_needing_tags(&version, TAGS_VERSION, 500) {
                Ok(r) if !r.is_empty() => r,
                _ => break,
            };
            for (id, vector) in rows {
                if let Err(e) = self.db.set_tags(id, &version, TAGS_VERSION, &index.tags_for(&vector)) {
                    eprintln!("[analysis] cannot store tags for {id}: {e}");
                }
            }
        }
        Some(index)
    }

    /// Name a photo that was just finished for the loading screen's strip (its thumbnail exists), at most every
    /// `LATEST_EVERY`.
    fn report_latest(&self, p: &Photo) {
        let Some(listener) = lock(&self.latest).clone() else { return };
        {
            let mut at = lock(&self.latest_at);
            if at.elapsed() < LATEST_EVERY {
                return;
            }
            *at = Instant::now();
        }
        let name = Path::new(&p.path).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        listener(LatestItem { id: p.id, mtime: p.mtime, size: p.size, name, kind: p.meta.kind });
    }

    /// A file of the intake has been stored (or could not be): it is no longer outstanding.
    fn finished_intake(&self, c: &Photo) {
        let mut i = lock(&self.intake);
        i.queued.remove(&c.path);
        if let Some(n) = i.outstanding.get_mut(&c.folder_id) {
            if c.meta.kind == MediaKind::Video {
                n.1 = (n.1 - 1).max(0);
            } else {
                n.0 = (n.0 - 1).max(0);
            }
        }
    }

    /// Up to `n` files off the front of the intake (still counted as outstanding until they are stored).
    fn take_intake(&self, n: usize) -> Vec<Candidate> {
        let mut i = lock(&self.intake);
        let n = n.min(i.queue.len());
        i.queue.drain(..n).collect()
    }

    /// Files taken for a batch that was stopped before they were stored go back to the front, in order.
    fn requeue(&self, taken: Vec<Candidate>, done: &[bool]) {
        let mut i = lock(&self.intake);
        for (c, _) in taken.into_iter().zip(done).filter(|(_, d)| !**d).collect::<Vec<_>>().into_iter().rev() {
            i.queue.push_front(c);
        }
    }

    /// The artwork check, once the picture model is loaded (its descriptions are worked out once).
    fn art_check(&self, embedder: &Arc<dyn Embedder>) -> Option<ArtCheck> {
        let mut slot = lock(&self.art_index);
        if slot.is_none() {
            match ArtIndex::build(&**embedder) {
                Ok(i) => *slot = Some(Arc::new(i)),
                Err(e) => {
                    eprintln!("[analysis] cannot prepare the artwork check: {e}");
                    return None;
                }
            }
        }
        slot.clone().map(|i| (embedder.clone(), i))
    }

    /// One item of a batch, from one read. A file a walk found (`w.new`) first gets its row, made from what is in the
    /// bytes (a video, which is not read here, from its own header); then the photo is processed. Returns the photo's id,
    /// which a new file only has now (0: it could not be stored).
    fn process_one(&self, w: &WorkItem, bytes: std::io::Result<Vec<u8>>, embedder: Option<&dyn Embedder>, faces: Option<&dyn FaceEngine>, art: Option<&ArtCheck>) -> (PhotoId, Outcome) {
        if !w.new {
            return (w.photo.id, self.process_photo(w, &w.photo, bytes, embedder, faces, art));
        }
        let c = &w.photo;
        let path = Path::new(&c.path);
        let video = c.meta.kind == MediaKind::Video;
        let mut meta = if video {
            read_media_meta(path, MediaKind::Video)
        } else {
            match &bytes {
                Ok(b) => catch_unwind(AssertUnwindSafe(|| read_meta_bytes(path, b))).unwrap_or_default(),
                Err(_) => PhotoMeta::default(), // unreadable: it is still listed, as before
            }
        };
        meta.kind = c.meta.kind;
        if meta.taken_at.is_none() {
            meta.taken_at = Some(c.mtime / 1000);
        }
        let stored = self.db.upsert_photo(c.folder_id, &c.path, c.size, c.mtime, &meta);
        self.finished_intake(c);
        let id = match stored {
            Ok(id) => id,
            Err(e) => {
                eprintln!("[analysis] cannot store {}: {e}", c.path);
                return (0, Outcome::Nothing);
            }
        };
        let photo = Photo { id, meta, ..c.clone() };
        if video {
            self.report_latest(&photo);
            return (id, Outcome::Nothing);
        }
        (id, self.process_photo(w, &photo, bytes, embedder, faces, art))
    }

    /// Everything that needs the photo, from one read and one decode. A decoder or model that panics is a failure of
    /// that photo or of the model, never of this thread.
    fn process_photo(&self, w: &WorkItem, p: &Photo, bytes: std::io::Result<Vec<u8>>, embedder: Option<&dyn Embedder>, faces: Option<&dyn FaceEngine>, art: Option<&ArtCheck>) -> Outcome {
        if !w.hash && embedder.is_none() && faces.is_none() && !(w.art && art.is_some()) {
            return Outcome::Nothing;
        }
        let path = Path::new(&p.path);
        let bytes = match bytes {
            Ok(b) => b,
            Err(_) => {
                if w.hash {
                    let _ = self.db.set_file_keys(p.id, UNREADABLE, None);
                }
                return Outcome::Unusable;
            }
        };
        if w.hash {
            let (quick, full) = (quick_key_bytes(&bytes), full_hash_bytes(&bytes));
            let _ = self.db.set_file_keys(p.id, &quick, Some(&full));
            let _ = self.db.refresh_dup_group(&full);
            if self.db.is_dup(p.id).unwrap_or(false) {
                return Outcome::Duplicate;
            }
        }
        let img = match catch_unwind(AssertUnwindSafe(|| decode_scaled_bytes(path, &bytes, DECODE_EDGE))) {
            Ok(Ok(img)) => img,
            _ => return Outcome::Unusable,
        };
        drop(bytes);
        let _ = thumbnail_from_image(&self.thumbs_dir, ThumbClass::Grid, p.id, p.mtime, p.size, &img);
        self.report_latest(p);
        let vector = embedder.map(|e| {
            let small = for_clip(&img);
            catch_unwind(AssertUnwindSafe(|| e.embed_image(&small))).unwrap_or_else(|_| Err("the search model crashed".into()))
        });
        let faces = faces.map(|f| catch_unwind(AssertUnwindSafe(|| find_faces(f, &img))).unwrap_or_else(|_| Err("the face model crashed".into())));
        // Faces are looked at for being artwork: those just found, and any stored earlier without a verdict.
        let mut verdicts = ArtVerdicts::default();
        if let Some((e, index)) = art {
            let verdict = |b: [f32; 4]| catch_unwind(AssertUnwindSafe(|| face_is_art(&**e, index, &img, b))).ok().and_then(Result::ok);
            if let Some(Ok(list)) = &faces {
                verdicts.new = list.iter().map(|f| verdict([f.x, f.y, f.w, f.h])).collect();
            } else if w.art {
                for (id, b) in self.db.faces_needing_art(p.id).unwrap_or_default() {
                    if let Some(is_art) = verdict(b) {
                        verdicts.old.push((id, is_art));
                    }
                }
            }
        }
        Outcome::Analysed { vector, faces, art: verdicts }
    }

    /// Regroup the faces that are not settled when enough new ones have come in (`idle`: any at all). A pass that found
    /// the faces changing under it is tried again next time.
    fn tidy_faces(&self, idle: bool) {
        let n = self.new_faces.load(Ordering::SeqCst);
        if n == 0 || (!idle && n < TIDY_EVERY) {
            return;
        }
        self.new_faces.fetch_sub(n, Ordering::SeqCst);
        match self.db.tidy() {
            Ok(Some(_)) => {}
            Ok(None) => {
                self.new_faces.fetch_add(n, Ordering::SeqCst);
            }
            Err(e) => eprintln!("[people] cannot regroup faces: {e}"),
        }
    }

    fn run(&self) {
        let (mut embed_failures, mut face_failures) = (0usize, 0usize);
        while !self.stop.load(Ordering::SeqCst) {
            if self.holding() {
                if !self.hold_reported.swap(true, Ordering::SeqCst) {
                    self.report(); // once, when it starts waiting; not on every poll
                }
                self.wait(self.poll);
                continue;
            }
            self.hold_reported.store(false, Ordering::SeqCst);
            let embedder = lock(&self.embedder).clone();
            let faces = lock(&self.faces).clone();
            let index = embedder.as_ref().and_then(|e| self.tag_pass(e));
            let art = embedder.as_ref().and_then(|e| self.art_check(e));
            let (ev, fv) = (embedder.as_ref().map(|e| e.version().to_string()), faces.as_ref().map(|f| f.version().to_string()));
            let taken = self.take_intake(BATCH as usize);
            if !taken.is_empty() {
                let items: Vec<WorkItem> = taken.iter().map(|c| intake_item(c, ev.is_some(), fv.is_some(), art.is_some())).collect();
                let r = self.batch(&items, &embedder, &faces, index, art, (ev.as_deref(), fv.as_deref()), (&mut embed_failures, &mut face_failures));
                self.requeue(taken, &r.done);
                self.tidy_faces(false);
                self.report();
                if r.down {
                    self.wait(self.poll * 10);
                }
                continue;
            }
            let batch = match self.db.photos_needing_work(ev.as_deref(), fv.as_deref(), art.is_some(), BATCH) {
                Ok(b) => b,
                Err(e) => {
                    eprintln!("[analysis] cannot list photos: {e}");
                    self.wait(IDLE_WAIT);
                    continue;
                }
            };
            if batch.is_empty() {
                self.tidy_faces(true);
                self.report();
                self.wait(IDLE_WAIT);
                continue;
            }
            let down = self.batch(&batch, &embedder, &faces, index, art, (ev.as_deref(), fv.as_deref()), (&mut embed_failures, &mut face_failures)).down;
            self.tidy_faces(false);
            self.report();
            if down {
                self.wait(self.poll * 10);
            }
        }
    }

    /// Work through one batch with several workers. Returns true if a model kept failing.
    fn batch(
        &self,
        batch: &[WorkItem],
        embedder: &Option<Arc<dyn Embedder>>,
        faces: &Option<Arc<dyn FaceEngine>>,
        index: Option<Arc<TagIndex>>,
        art: Option<ArtCheck>,
        (ev, fv): (Option<&str>, Option<&str>),
        (embed_failures, face_failures): (&mut usize, &mut usize),
    ) -> BatchEnd {
        let next_read = AtomicUsize::new(0);
        let (embed_off, faces_off) = (AtomicBool::new(false), AtomicBool::new(false));
        // Files read ahead (bounded, so reading cannot run far in front of the work) and results coming back.
        let (btx, brx) = mpsc::sync_channel::<(usize, std::io::Result<Vec<u8>>)>(self.workers * 2);
        let brx = Mutex::new(brx);
        let (tx, rx) = mpsc::channel::<(usize, PhotoId, Outcome)>();
        let mut done = vec![false; batch.len()];
        let mut handled = 0usize;
        let mut down = false;
        std::thread::scope(|scope| {
            for _ in 0..READERS.min(batch.len()) {
                let btx = btx.clone();
                let next_read = &next_read;
                scope.spawn(move || loop {
                    if self.stop.load(Ordering::SeqCst) || self.holding() {
                        break;
                    }
                    let i = next_read.fetch_add(1, Ordering::SeqCst);
                    let Some(w) = batch.get(i) else { break };
                    let wanted = w.hash || w.embed || w.faces || w.art || (w.new && w.photo.meta.kind == MediaKind::Photo); // (a row to refresh needs the file's bytes too)
                    let mut msg = (i, if wanted { std::fs::read(&w.photo.path) } else { Ok(Vec::new()) });
                    loop {
                        match btx.try_send(msg) {
                            Ok(()) => break,
                            Err(TrySendError::Disconnected(_)) => return,
                            Err(TrySendError::Full(m)) => {
                                if self.stop.load(Ordering::SeqCst) || self.holding() {
                                    return;
                                }
                                msg = m;
                                std::thread::sleep(Duration::from_millis(5));
                            }
                        }
                    }
                });
            }
            drop(btx);
            for _ in 0..self.workers.clamp(1, batch.len()) {
                let tx = tx.clone();
                let (brx, embed_off, faces_off, art) = (&brx, &embed_off, &faces_off, &art);
                scope.spawn(move || loop {
                    let Ok((i, bytes)) = lock(brx).recv() else { break };
                    if self.stop.load(Ordering::SeqCst) || self.holding() {
                        break;
                    }
                    let w = &batch[i];
                    let e = embedder.as_deref().filter(|_| w.embed && !embed_off.load(Ordering::SeqCst));
                    let f = faces.as_deref().filter(|_| w.faces && !faces_off.load(Ordering::SeqCst));
                    let (id, outcome) = self.process_one(w, bytes, e, f, art.as_ref().filter(|_| w.art || w.faces || w.new));
                    if tx.send((i, id, outcome)).is_err() {
                        break;
                    }
                });
            }
            drop(tx);
            for (i, id, outcome) in rx {
                done[i] = true;
                let w = &batch[i];
                match outcome {
                    Outcome::Nothing | Outcome::Duplicate => {}
                    Outcome::Unusable => {
                        if let (true, Some(v)) = (w.embed, ev) {
                            let _ = self.db.put_embedding_failed(id, v);
                        }
                        if let (true, Some(v)) = (w.faces, fv) {
                            let _ = self.db.replace_faces(id, v, &[]);
                        }
                        if w.art {
                            let _ = self.db.settle_art(id); // its faces cannot be looked at: not offered again
                        }
                    }
                    Outcome::Analysed { vector, faces: found, art: verdicts } => {
                        if !verdicts.old.is_empty() {
                            self.new_faces.fetch_add(1, Ordering::SeqCst); // artwork leaves its group at the next regrouping
                        }
                        for (face, is_art) in verdicts.old {
                            let _ = self.db.set_face_art(face, is_art);
                        }
                        if let (Some(r), Some(v)) = (vector, ev) {
                            match r {
                                Ok(vec) => {
                                    *embed_failures = 0;
                                    *lock(&self.embed_error) = None;
                                    match self.db.put_embedding(id, v, &vec) {
                                        Ok(()) => {
                                            if let Some(idx) = &index {
                                                let _ = self.db.set_tags(id, v, TAGS_VERSION, &idx.tags_for(&vec));
                                            }
                                        }
                                        Err(e) => eprintln!("[analysis] cannot store result for {}: {e}", w.photo.path), // the photo vanished mid-run
                                    }
                                }
                                Err(e) => {
                                    // Not the photo's fault: leave it queued, and back off if the model keeps failing.
                                    *embed_failures += 1;
                                    if *embed_failures >= ENGINE_FAILURES && !embed_off.swap(true, Ordering::SeqCst) {
                                        eprintln!("[analysis] embedder failing: {e}");
                                        *lock(&self.embed_error) = Some(e);
                                        down = true;
                                    }
                                }
                            }
                        }
                        if let (Some(r), Some(v)) = (found, fv) {
                            match r {
                                Ok(list) => {
                                    *face_failures = 0;
                                    *lock(&self.face_error) = None;
                                    match self.db.replace_faces(id, v, &list) {
                                        Ok(ids) => {
                                            for (face, is_art) in ids.iter().zip(&verdicts.new) {
                                                if let Some(a) = is_art {
                                                    let _ = self.db.set_face_art(*face, *a);
                                                }
                                            }
                                            // a face that is clearly a confirmed person's joins them now (to be checked);
                                            // the rest wait to be grouped with the others (`tidy_faces`)
                                            if !ids.is_empty() {
                                                if let Err(e) = self.db.attach_confident(Some(&ids)) {
                                                    eprintln!("[people] cannot place new faces: {e}");
                                                }
                                                self.new_faces.fetch_add(ids.len(), Ordering::SeqCst);
                                            }
                                        }
                                        Err(e) => eprintln!("[people] cannot store result for {}: {e}", w.photo.path),
                                    }
                                }
                                Err(e) => {
                                    *face_failures += 1;
                                    if *face_failures >= ENGINE_FAILURES && !faces_off.swap(true, Ordering::SeqCst) {
                                        eprintln!("[people] face engine failing: {e}");
                                        *lock(&self.face_error) = Some(e);
                                        down = true;
                                    }
                                }
                            }
                        }
                    }
                }
                handled += 1;
                if handled % REPORT_EVERY == 0 {
                    self.report();
                }
            }
        });
        BatchEnd { down, done }
    }
}

/// How a batch ended: whether a model kept failing, and which of its items were handled (the rest were cut short).
struct BatchEnd {
    down: bool,
    done: Vec<bool>,
}

/// The work for a file a walk found. A new file gets everything that can be done now (the models that are loaded). A file
/// already in the library (it changed, or a Reload reads everything again) only has its row refreshed from the file: what
/// is already known about it (faces, and the names given to them, tags, hash) is kept, and anything its change made
/// stale is picked up by the database-driven pass that follows.
fn intake_item(c: &Candidate, embed: bool, faces: bool, art: bool) -> WorkItem {
    let photo = c.kind == MediaKind::Photo;
    let fresh = photo && !c.is_update;
    WorkItem {
        photo: Photo { id: 0, folder_id: c.folder_id, path: c.path.clone(), size: c.size, mtime: c.mtime, meta: PhotoMeta { kind: c.kind, ..Default::default() }, missing: false },
        hash: fresh,
        embed: fresh && embed,
        faces: fresh && faces,
        art: fresh && art,
        new: true,
    }
}

/// Reads, hashes, decodes and analyses photos in the background, newest first. Pauses itself while a scan runs.
pub struct PhotoPipeline {
    shared: Arc<Shared>,
    handle: Mutex<Option<JoinHandle<()>>>,
}

impl PhotoPipeline {
    pub fn start(cfg: PipelineConfig) -> PhotoPipeline {
        let shared = Arc::new(Shared {
            intake: Mutex::new(Intake::default()),
            latest: Mutex::new(None),
            latest_at: Mutex::new(Instant::now() - LATEST_EVERY),
            db: cfg.db,
            thumbs_dir: cfg.thumbs_dir,
            scanning: cfg.scanning,
            poll: cfg.poll,
            workers: cfg.workers.max(1),
            vocab: cfg.vocab,
            embedder: Mutex::new(None),
            faces: Mutex::new(None),
            tag_index: Mutex::new(None),
            art_index: Mutex::new(None),
            reporters: Mutex::new(Vec::new()),
            embed_error: Mutex::new(None),
            face_error: Mutex::new(None),
            // faces found by an earlier run (or grouped by an older version) are regrouped once, when the work runs out
            new_faces: AtomicUsize::new(1),
            paused: AtomicBool::new(false),
            stop: AtomicBool::new(false),
            hold_reported: AtomicBool::new(false),
            kicked: Mutex::new(false),
            wake: Condvar::new(),
        });
        let s = shared.clone();
        let handle = std::thread::spawn(move || s.run());
        PhotoPipeline { shared, handle: Mutex::new(Some(handle)) }
    }

    /// The Auto tags model is loaded: photos still without a vector are picked up.
    pub fn set_embedder(&self, e: Arc<dyn Embedder>) {
        *lock(&self.shared.tag_index) = None;
        *lock(&self.shared.art_index) = None;
        *lock(&self.shared.embedder) = Some(e);
        self.kick();
    }

    /// The face models are loaded: photos not yet scanned for faces are picked up.
    pub fn set_face_engine(&self, e: Arc<dyn FaceEngine>) {
        *lock(&self.shared.faces) = Some(e);
        self.kick();
    }

    /// Files a walk found: the pipeline reads each once and does everything with it, starting with its row in the library.
    /// Newest first, so the newest photos appear first.
    pub fn enqueue(&self, mut files: Vec<Candidate>) {
        files.sort_by(|a, b| b.mtime.cmp(&a.mtime));
        {
            let mut i = lock(&self.shared.intake);
            for c in files {
                if !i.queued.insert(c.path.clone()) {
                    continue;
                }
                let n = i.outstanding.entry(c.folder_id).or_default();
                if c.kind == MediaKind::Video {
                    n.1 += 1;
                } else {
                    n.0 += 1;
                }
                i.queue.push_back(c);
            }
        }
        self.kick();
    }

    /// The photos a walk found (in any folder) that are not stored yet: the analyses' totals include them, so a library
    /// that is still filling in does not look nearly done.
    pub fn pending_photos(&self) -> i64 {
        lock(&self.shared.intake).outstanding.values().map(|n| n.0).sum()
    }

    /// The (photos, videos) in these folders that a walk found and the pipeline has not stored yet.
    pub fn pending_in(&self, folders: &[FolderId]) -> (i64, i64) {
        let i = lock(&self.shared.intake);
        folders.iter().filter_map(|f| i.outstanding.get(f)).fold((0, 0), |a, n| (a.0 + n.0, a.1 + n.1))
    }

    /// Told which photo was just finished (its thumbnail exists), a few times a second at most: for the strip.
    pub fn set_latest_listener(&self, f: LatestListener) {
        *lock(&self.shared.latest) = Some(f);
    }

    /// `f` runs whenever progress has moved (a service uses it to tell the window).
    pub fn add_reporter(&self, f: Reporter) {
        lock(&self.shared.reporters).push(f);
    }

    /// What is wrong with the Auto tags model, if it keeps failing.
    pub fn embed_error(&self) -> Option<String> {
        lock(&self.shared.embed_error).clone()
    }

    /// What is wrong with the face models, if they keep failing.
    pub fn face_error(&self) -> Option<String> {
        lock(&self.shared.face_error).clone()
    }

    pub fn is_paused(&self) -> bool {
        self.shared.paused.load(Ordering::SeqCst)
    }

    pub fn set_paused(&self, paused: bool) {
        self.shared.paused.store(paused, Ordering::SeqCst);
        self.kick();
        self.shared.report();
    }

    /// Tell an idle pipeline to look for new photos now.
    pub fn kick(&self) {
        *lock(&self.shared.kicked) = true;
        self.shared.wake.notify_all();
    }

    pub fn stop(&self) {
        self.shared.stop.store(true, Ordering::SeqCst);
        self.kick();
        if let Some(h) = lock(&self.handle).take() {
            let _ = h.join();
        }
    }
}

impl Drop for PhotoPipeline {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::embedder::FakeEmbedder;
    use crate::faces::FakeFaceEngine;
    use image::{Rgb, RgbImage};
    use pixie_core::PhotoMeta;
    use pixie_image::thumbs::thumb_path;
    use std::sync::atomic::AtomicU8;
    use std::time::Instant;

    const EV: &str = "fake-1";
    const FV: &str = "fake-faces-1";

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

    struct Rig {
        dir: tempfile::TempDir,
        db: Db,
        folder: i64,
    }

    impl Rig {
        fn new() -> Rig {
            let dir = tempfile::tempdir().unwrap();
            let db = Db::open_in_memory().unwrap();
            let folder = db.add_folder(dir.path().to_str().unwrap()).unwrap();
            Rig { dir, db, folder }
        }
        fn thumbs(&self) -> PathBuf {
            self.dir.path().join("thumbs")
        }
        /// A PNG whose top-left red channel is `red` (the fake models read it); `tag` makes each file different.
        fn png(&self, name: &str, red: u8, tag: u8) -> i64 {
            let path = self.dir.path().join(name);
            RgbImage::from_pixel(16, 16, Rgb([red, tag, 0])).save(&path).unwrap();
            self.db.upsert_photo(self.folder, path.to_str().unwrap(), 1, 1, &PhotoMeta::default()).unwrap()
        }
        fn pngs(&self, reds: &[u8]) -> Vec<i64> {
            reds.iter().enumerate().map(|(i, r)| self.png(&format!("p{i}.png"), *r, i as u8)).collect()
        }
        /// A PNG file only (no row in the library): what a walk would find.
        fn file(&self, name: &str, red: u8, tag: u8) {
            RgbImage::from_pixel(16, 16, Rgb([red, tag, 0])).save(self.dir.path().join(name)).unwrap();
        }
        /// What a walk of the folder finds.
        fn walk(&self) -> Vec<Candidate> {
            pixie_index::walk_folder(&self.db, self.folder, pixie_index::ScanMode::Quick, &AtomicBool::new(false), |_| {}).unwrap().candidates
        }
        fn junk(&self, name: &str) -> i64 {
            let path = self.dir.path().join(name);
            std::fs::write(&path, b"<!DOCTYPE HTML><html>not a picture</html>").unwrap();
            self.db.upsert_photo(self.folder, path.to_str().unwrap(), 1, 1, &PhotoMeta::default()).unwrap()
        }
        fn pipeline(&self, workers: usize) -> PhotoPipeline {
            self.pipeline_with(workers, Arc::new(AtomicBool::new(false)))
        }
        fn pipeline_with(&self, workers: usize, scanning: Arc<AtomicBool>) -> PhotoPipeline {
            let mut cfg = PipelineConfig::new(self.db.clone(), self.thumbs(), Arc::new(move || scanning.load(Ordering::SeqCst)));
            cfg.poll = Duration::from_millis(10);
            cfg.workers = workers;
            cfg.vocab = vec!["reddish".into(), "bluish".into()];
            PhotoPipeline::start(cfg)
        }
        fn hashed_all(&self) -> bool {
            self.db.photos_needing_work(None, None, false, 1000).unwrap().is_empty()
        }
        fn counts(&self) -> ((i64, i64), (i64, i64)) {
            (self.db.embedding_counts(EV).unwrap(), self.db.face_counts(FV).unwrap())
        }
    }

    /// Red-ish pictures and the word "reddish" share one axis, blue-ish ones the other.
    struct Colour;
    impl Embedder for Colour {
        fn embed_image(&self, img: &DynamicImage) -> Result<Vec<f32>, String> {
            FakeEmbedder.embed_image(img)
        }
        fn embed_text(&self, t: &str) -> Result<Vec<f32>, String> {
            // (the descriptions of a real person's face, which the artwork check asks about, sit with the red faces the
            // fake face engine finds, so those faces are people)
            Ok(if t.contains("reddish") || ["real person", "selfie", "candid", "smiling"].iter().any(|w| t.contains(w)) {
                vec![1.0, 0.0]
            } else if t.contains("bluish") {
                vec![0.0, 1.0]
            } else {
                vec![0.0, 0.0]
            })
        }
        fn version(&self) -> &str {
            EV
        }
    }

    /// Like `Colour`, but the descriptions of artwork sit with the red pictures, so the red faces are paintings.
    struct Painted;
    impl Embedder for Painted {
        fn embed_image(&self, img: &DynamicImage) -> Result<Vec<f32>, String> {
            FakeEmbedder.embed_image(img)
        }
        fn embed_text(&self, t: &str) -> Result<Vec<f32>, String> {
            let art = ["painting", "drawing", "statue", "sculpture", "cartoon", "toy", "mask", "mural", "mosaic"].iter().any(|w| t.contains(w));
            Ok(if art { vec![1.0, 0.0] } else { vec![0.0, 1.0] })
        }
        fn version(&self) -> &str {
            EV
        }
    }

    #[test]
    fn files_from_a_walk_are_stored_hashed_thumbnailed_and_analysed_in_one_pass() {
        let r = Rig::new();
        r.file("a.png", 250, 1);
        r.file("b.png", 250, 2);
        r.file("c.png", 5, 3);
        let found = r.walk();
        assert_eq!(found.len(), 3);
        assert_eq!(r.db.count_photos_in(&Default::default()).unwrap(), 0, "the walk stored nothing");
        let p = r.pipeline(2);
        with_models(&p);
        p.enqueue(found);
        assert!(wait_until(5000, || r.counts() == ((3, 3), (3, 3))));
        assert!(wait_until(5000, || p.pending_in(&[r.folder]) == (0, 0)));
        assert!(r.hashed_all());
        let photos = r.db.list_photos_in(&Default::default(), 0, 10).unwrap();
        assert_eq!(photos.len(), 3);
        for ph in photos {
            assert_eq!((ph.meta.width, ph.meta.height), (Some(16), Some(16)), "read from the file's own bytes");
            assert_eq!(ph.meta.taken_at, Some(ph.mtime / 1000), "no EXIF date: the file's time");
            assert!(thumb_path(&r.thumbs(), ThumbClass::Grid, ph.id, ph.mtime, ph.size).exists(), "grid thumbnail of {}", ph.id);
        }
        p.stop();
    }

    #[test]
    fn reloading_a_folder_refreshes_its_rows_but_keeps_faces_and_what_is_known() {
        let r = Rig::new();
        r.file("a.png", 250, 1);
        r.file("b.png", 250, 2);
        let p = r.pipeline(2);
        with_models(&p);
        p.enqueue(r.walk());
        assert!(wait_until(5000, || p.pending_in(&[r.folder]) == (0, 0) && r.counts() == ((2, 2), (2, 2))));
        let photos = r.db.list_photos_in(&Default::default(), 0, 10).unwrap();
        let faces_before: Vec<Vec<i64>> = photos.iter().map(|ph| r.db.faces_of(ph.id).unwrap().iter().map(|f| f.id).collect()).collect();
        assert!(faces_before.iter().all(|f| f.len() == 1), "the fake engine finds a face in each red picture");
        let first = &photos[0];
        // make the stored details wrong, as if they had been read badly (same size and time, so nothing else is cleared)
        r.db.upsert_photo(first.folder_id, &first.path, first.size, first.mtime, &PhotoMeta { camera: Some("stale".into()), ..Default::default() }).unwrap();

        // a Reload: a Full walk lists every file, changed or not
        let mut all = pixie_index::walk_folder(&r.db, r.folder, pixie_index::ScanMode::Full, &AtomicBool::new(false), |_| {}).unwrap().candidates;
        all.retain(|c| !c.path.contains("thumbs")); // (the rig keeps its thumbnail cache inside the folder)
        assert_eq!(all.len(), 2);
        assert!(all.iter().all(|c| c.is_update));
        p.enqueue(all);
        assert!(wait_until(5000, || p.pending_in(&[r.folder]) == (0, 0)));
        std::thread::sleep(Duration::from_millis(300));
        let again = r.db.get_photo(first.id).unwrap().unwrap();
        assert_eq!(again.meta.width, Some(16), "the row was read from the file again");
        assert_eq!(again.meta.camera, None);
        let faces_after: Vec<Vec<i64>> = photos.iter().map(|ph| r.db.faces_of(ph.id).unwrap().iter().map(|f| f.id).collect()).collect();
        assert_eq!(faces_after, faces_before, "the same faces, not found again: the names given to them survive a Reload");
        assert_eq!(r.counts(), ((2, 2), (2, 2)));
        p.stop();
    }

    #[test]
    fn a_file_offered_twice_is_read_once_and_a_held_pipeline_keeps_what_it_was_given() {
        let r = Rig::new();
        for i in 0..4u8 {
            r.file(&format!("p{i}.png"), 250, i);
        }
        let hold = Arc::new(AtomicBool::new(true)); // a scan job is running: the pipeline waits
        let p = r.pipeline_with(2, hold.clone());
        with_models(&p);
        p.enqueue(r.walk());
        p.enqueue(r.walk()); // the same files again (a second scan found them too)
        assert_eq!(p.pending_in(&[r.folder]), (4, 0), "each file counts once");
        std::thread::sleep(Duration::from_millis(100));
        assert_eq!(r.db.count_photos_in(&Default::default()).unwrap(), 0, "nothing was read while held");
        hold.store(false, Ordering::SeqCst);
        p.kick();
        assert!(wait_until(5000, || p.pending_in(&[r.folder]) == (0, 0) && r.counts() == ((4, 4), (4, 4))));
        assert_eq!(r.db.count_photos_in(&Default::default()).unwrap(), 4);
        p.stop();
    }

    #[test]
    fn the_pipeline_names_finished_photos_for_the_strip_and_their_thumbnails_exist() {
        let r = Rig::new();
        for i in 0..6u8 {
            r.file(&format!("p{i}.png"), 250, i);
        }
        let p = r.pipeline(2);
        let seen: Arc<Mutex<Vec<LatestItem>>> = Arc::new(Mutex::new(Vec::new()));
        let sink = seen.clone();
        p.set_latest_listener(Arc::new(move |item| sink.lock().unwrap().push(item)));
        with_models(&p);
        p.enqueue(r.walk());
        assert!(wait_until(5000, || !seen.lock().unwrap().is_empty()));
        assert!(wait_until(5000, || p.pending_in(&[r.folder]) == (0, 0)));
        let items = seen.lock().unwrap().clone();
        assert!(items.len() <= 6);
        for it in items {
            assert!(thumb_path(&r.thumbs(), ThumbClass::Grid, it.id, it.mtime, it.size).exists(), "the picture shown has its thumbnail");
        }
        p.stop();
    }

    #[test]
    fn an_unreadable_picture_and_a_video_found_by_a_walk_are_still_listed() {
        let r = Rig::new();
        std::fs::write(r.dir.path().join("broken.jpg"), b"<!DOCTYPE HTML><html>not a picture</html>").unwrap();
        std::fs::write(r.dir.path().join("clip.mp4"), b"not a video either").unwrap();
        r.file("ok.png", 250, 1);
        let p = r.pipeline(2);
        with_models(&p);
        p.enqueue(r.walk());
        assert!(wait_until(5000, || p.pending_in(&[r.folder]) == (0, 0)));
        let all = r.db.list_photos_in(&Default::default(), 0, 10).unwrap();
        assert_eq!(all.len(), 3, "listed even though two cannot be read");
        assert_eq!(all.iter().filter(|x| x.meta.kind == MediaKind::Video).count(), 1);
        assert!(wait_until(5000, || r.hashed_all()), "and nothing is offered again");
        p.stop();
    }

    #[test]
    fn faces_found_in_artwork_are_marked_and_kept_out_of_unsure() {
        let r = Rig::new();
        r.pngs(&[250, 251, 252]);
        let p = r.pipeline(2);
        p.set_embedder(Arc::new(Painted));
        p.set_face_engine(Arc::new(FakeFaceEngine));
        assert!(wait_until(5000, || r.counts() == ((3, 3), (3, 3))));
        assert!(wait_until(5000, || r.db.faces_needing_art(1).unwrap().is_empty() && r.db.face_overview().unwrap().unsorted == 0));
        p.stop();
    }

    #[test]
    fn faces_found_before_the_picture_model_was_ready_are_checked_for_artwork_later() {
        let r = Rig::new();
        r.pngs(&[250, 251, 252]);
        let p = r.pipeline(2);
        p.set_face_engine(Arc::new(FakeFaceEngine)); // faces first: there is no picture model to ask yet
        assert!(wait_until(5000, || r.db.face_counts(FV).unwrap() == (3, 3)));
        assert!(wait_until(5000, || r.db.face_overview().unwrap().groups == 1), "three alike faces in three photos are a group");
        p.set_embedder(Arc::new(Painted));
        assert!(wait_until(5000, || r.db.face_overview().unwrap().groups == 0), "the faces already found were looked at: paintings are no group");
        assert_eq!(r.db.face_overview().unwrap().unsorted, 0, "and are not listed in Unsure either");
        p.stop();
    }

    fn with_models(p: &PhotoPipeline) {
        p.set_embedder(Arc::new(Colour));
        p.set_face_engine(Arc::new(FakeFaceEngine));
    }

    #[test]
    fn a_photo_is_hashed_thumbnailed_embedded_tagged_and_scanned_for_faces() {
        let r = Rig::new();
        let ids = r.pngs(&[250, 250, 5]);
        let p = r.pipeline(2);
        with_models(&p);
        assert!(wait_until(5000, || r.counts() == ((3, 3), (3, 3))));
        assert!(wait_until(5000, || r.db.tag_counts().unwrap() == vec![("reddish".to_string(), 2), ("bluish".to_string(), 1)]));
        assert!(r.hashed_all());
        for id in ids {
            let ph = r.db.get_photo(id).unwrap().unwrap();
            assert!(thumb_path(&r.thumbs(), ThumbClass::Grid, id, ph.mtime, ph.size).exists(), "grid thumbnail of {id}");
        }
        // The two red photos hold the same "person".
        assert!(wait_until(5000, || r.db.face_overview().unwrap().groups == 1)); // two photos of the same "person" are a group to name
        p.stop();
    }

    #[test]
    fn already_embedded_photos_are_tagged_from_the_stored_vector_without_reading_the_file() {
        let r = Rig::new();
        let id = r.db.upsert_photo(r.folder, "/nowhere/gone.jpg", 1, 1, &PhotoMeta::default()).unwrap();
        r.db.put_embedding(id, EV, &[0.0, 1.0]).unwrap(); // embedded earlier; the file is not even readable now
        let p = r.pipeline(2);
        p.set_embedder(Arc::new(Colour));
        assert!(wait_until(5000, || r.db.tags_for(id).unwrap().first().map(|t| t.0.clone()) == Some("bluish".to_string())));
        assert_eq!(r.db.embedding_counts(EV).unwrap().0, 1); // untouched
        p.stop();
    }

    #[test]
    fn a_new_vocabulary_version_retags_everything() {
        let r = Rig::new();
        let id = r.pngs(&[250])[0];
        let p = r.pipeline(1);
        p.set_embedder(Arc::new(Colour));
        assert!(wait_until(5000, || r.db.tag_counts().unwrap().len() == 1));
        p.stop();
        r.db.set_tags(id, EV, "old-vocabulary", &[("stale".into(), 1.0)]).unwrap();
        let p = r.pipeline(1);
        p.set_embedder(Arc::new(Colour));
        assert!(wait_until(5000, || r.db.tags_for(id).unwrap().first().map(|t| t.0.clone()) == Some("reddish".to_string())));
        p.stop();
    }

    #[test]
    fn hashing_and_thumbnails_need_no_models_and_a_model_arriving_later_fills_in_the_rest() {
        let r = Rig::new();
        let ids = r.pngs(&[250, 5]);
        let p = r.pipeline(2);
        assert!(wait_until(5000, || r.hashed_all()));
        let ph = r.db.get_photo(ids[0]).unwrap().unwrap();
        assert!(wait_until(5000, || thumb_path(&r.thumbs(), ThumbClass::Grid, ids[0], ph.mtime, ph.size).exists()));
        assert_eq!(r.db.embedding_counts(EV).unwrap().0, 0);
        assert_eq!(r.db.face_counts(FV).unwrap().0, 0);
        p.set_embedder(Arc::new(Colour));
        assert!(wait_until(5000, || r.db.embedding_counts(EV).unwrap() == (2, 2)));
        assert_eq!(r.db.face_counts(FV).unwrap().0, 0, "no face model yet");
        p.set_face_engine(Arc::new(FakeFaceEngine));
        assert!(wait_until(5000, || r.db.face_counts(FV).unwrap() == (2, 2)));
        p.stop();
    }

    #[test]
    fn identical_photos_are_one_visible_item_and_the_extra_copy_is_not_analysed() {
        let r = Rig::new();
        // Same pixels, so the same bytes. The newest (the one handled first) also sorts first, so it is the one shown.
        let hidden = r.png("z.png", 250, 7);
        let shown = r.png("a.png", 250, 7);
        let other = r.png("m.png", 5, 9);
        let p = r.pipeline(1);
        // Photos are read ahead, so which of two identical photos is handled first is not fixed. Hash everything first
        // (no models yet): which copy is shown is then settled before any of them is analysed.
        assert!(wait_until(5000, || r.hashed_all()));
        assert!(r.db.is_dup(hidden).unwrap() && !r.db.is_dup(shown).unwrap());
        with_models(&p);
        assert!(wait_until(5000, || r.counts() == ((2, 2), (2, 2))));
        assert!(r.db.is_dup(hidden).unwrap() && !r.db.is_dup(shown).unwrap() && !r.db.is_dup(other).unwrap());
        let embedded: Vec<i64> = r.db.load_embeddings(EV).unwrap().into_iter().map(|e| e.0).collect();
        assert!(embedded.contains(&shown) && embedded.contains(&other) && !embedded.contains(&hidden), "the hidden copy was not analysed");
        assert_eq!(r.db.list_photos(0, 10).unwrap().len(), 2);
        p.stop();
    }

    #[test]
    fn load_progress_counts_a_new_folders_own_photos_even_when_they_hide_older_copies() {
        let r = Rig::new();
        // An older folder with two photos, then a new folder whose first photo is an identical copy of one of them
        // and sorts first, so it becomes the shown copy and the older one is hidden.
        let old_dir = r.dir.path().join("old");
        std::fs::create_dir_all(&old_dir).unwrap();
        let old_folder = r.db.add_folder(old_dir.to_str().unwrap()).unwrap();
        let add_old = |name: &str, red: u8, tag: u8| {
            let path = old_dir.join(name);
            RgbImage::from_pixel(16, 16, Rgb([red, tag, 0])).save(&path).unwrap();
            r.db.upsert_photo(old_folder, path.to_str().unwrap(), 1, 1, &PhotoMeta::default()).unwrap()
        };
        let older_copy = add_old("zz.png", 250, 7);
        let _other_old = add_old("yy.png", 5, 9);
        let p = r.pipeline(1);
        with_models(&p);
        assert!(wait_until(5000, || r.counts() == ((2, 2), (2, 2))));
        assert_eq!(r.db.load_progress(&[old_folder], EV, FV).unwrap().photos, 2);

        // The new folder: a copy of the first old photo (sorts first), and one new photo.
        let new_copy = r.png("a.png", 250, 7);
        let new_one = r.png("b.png", 5, 33);
        p.kick();
        assert!(wait_until(5000, || r.db.load_progress(&[r.folder], EV, FV).unwrap().scanned == 2));
        let l = r.db.load_progress(&[r.folder], EV, FV).unwrap();
        assert_eq!((l.photos, l.shown, l.hashed, l.tagged, l.scanned), (2, 2, 2, 2, 2));
        assert_eq!(l.duplicates, 1, "one extra copy in the set that includes the new photo");
        assert_eq!(l.faces, 1, "only the red photo has a face");
        // The older copy is now the hidden one, so the library shows 3 photos, not 4; the new folder's own count is
        // unaffected and is what a load screen should use.
        assert!(r.db.is_dup(older_copy).unwrap() && !r.db.is_dup(new_copy).unwrap() && !r.db.is_dup(new_one).unwrap());
        assert_eq!(r.db.embedding_counts(EV).unwrap().1, 3);
        let old = r.db.load_progress(&[old_folder], EV, FV).unwrap();
        assert_eq!((old.photos, old.shown), (2, 1));
        assert_eq!(r.db.load_progress(&[], EV, FV).unwrap().photos, 0);
        p.stop();
    }

    #[test]
    fn if_the_shown_copy_is_removed_the_hidden_one_is_analysed_then() {
        let r = Rig::new();
        let hidden = r.png("z.png", 250, 7);
        let shown = r.png("a.png", 250, 7);
        let p = r.pipeline(1);
        assert!(wait_until(5000, || r.hashed_all())); // settle which copy is shown before anything is analysed
        with_models(&p);
        assert!(wait_until(5000, || r.counts() == ((1, 1), (1, 1))));
        assert!(r.db.is_dup(hidden).unwrap());
        r.db.mark_removed(shown).unwrap();
        r.db.refresh_dup_flags().unwrap();
        p.kick();
        assert!(wait_until(5000, || r.db.load_embeddings(EV).unwrap().iter().any(|e| e.0 == hidden)));
        p.stop();
    }

    #[test]
    fn a_file_that_is_not_a_picture_or_cannot_be_read_is_settled_once_and_the_rest_still_work() {
        let r = Rig::new();
        r.pngs(&[250]);
        r.junk("page.jpg");
        r.db.upsert_photo(r.folder, r.dir.path().join("gone.jpg").to_str().unwrap(), 1, 1, &PhotoMeta::default()).unwrap();
        let p = r.pipeline(2);
        with_models(&p);
        assert!(wait_until(5000, || r.counts() == ((3, 3), (3, 3))));
        assert!(r.db.photos_needing_work(Some(EV), Some(FV), false, 10).unwrap().is_empty(), "nothing is offered again");
        assert_eq!(r.db.load_embeddings(EV).unwrap().len(), 1, "only the real picture has a vector");
        p.stop();
    }

    struct Flaky(AtomicU8);
    impl Embedder for Flaky {
        fn embed_image(&self, img: &DynamicImage) -> Result<Vec<f32>, String> {
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
            EV
        }
    }

    #[test]
    fn a_failing_embedder_is_reported_and_leaves_photos_queued_while_faces_carry_on() {
        for fault in [1u8, 2] {
            let r = Rig::new();
            r.pngs(&[250, 5, 250, 5]);
            let p = r.pipeline(2);
            let flaky = Arc::new(Flaky(AtomicU8::new(fault)));
            p.set_embedder(flaky.clone());
            p.set_face_engine(Arc::new(FakeFaceEngine));
            assert!(wait_until(5000, || p.embed_error().is_some()), "fault={fault}");
            assert!(wait_until(5000, || r.db.face_counts(FV).unwrap() == (4, 4)), "faces are not held up by the other model");
            assert_eq!(r.db.embedding_counts(EV).unwrap().0, 0, "nothing was marked failed");
            flaky.0.store(0, Ordering::SeqCst); // the fault clears: the same photos are analysed
            assert!(wait_until(8000, || r.db.embedding_counts(EV).unwrap() == (4, 4)));
            assert!(wait_until(2000, || p.embed_error().is_none()));
            p.stop();
        }
    }

    #[test]
    fn pause_and_scan_hold_the_work_and_resume_continues() {
        let r = Rig::new();
        r.pngs(&[250, 5]);
        let scanning = Arc::new(AtomicBool::new(true));
        let p = r.pipeline_with(2, scanning.clone());
        with_models(&p);
        std::thread::sleep(Duration::from_millis(150));
        assert!(!r.hashed_all(), "a scan is running");
        scanning.store(false, Ordering::SeqCst);
        p.set_paused(true);
        std::thread::sleep(Duration::from_millis(100));
        let at_pause = r.db.photos_needing_work(Some(EV), Some(FV), false, 10).unwrap().len();
        std::thread::sleep(Duration::from_millis(150));
        assert_eq!(r.db.photos_needing_work(Some(EV), Some(FV), false, 10).unwrap().len(), at_pause);
        assert!(p.is_paused());
        p.set_paused(false);
        assert!(wait_until(5000, || r.counts() == ((2, 2), (2, 2)) && r.hashed_all()));
        p.stop();
    }

    #[test]
    fn a_holding_pipeline_does_not_keep_reporting() {
        let r = Rig::new();
        r.pngs(&[250]);
        let p = r.pipeline_with(1, Arc::new(AtomicBool::new(true)));
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        p.add_reporter(Arc::new(move || {
            c.fetch_add(1, Ordering::SeqCst);
        }));
        std::thread::sleep(Duration::from_millis(300));
        assert!(count.load(Ordering::SeqCst) <= 2, "reported {} times while only waiting", count.load(Ordering::SeqCst));
        p.stop();
    }

    #[test]
    fn progress_is_reported_and_kick_picks_up_photos_added_later() {
        let r = Rig::new();
        r.pngs(&[250]);
        let p = r.pipeline(2);
        let reports = Arc::new(AtomicUsize::new(0));
        let c = reports.clone();
        p.add_reporter(Arc::new(move || {
            c.fetch_add(1, Ordering::SeqCst);
        }));
        with_models(&p);
        assert!(wait_until(5000, || r.counts() == ((1, 1), (1, 1))));
        r.png("late.png", 250, 99);
        p.kick();
        assert!(wait_until(5000, || r.counts() == ((2, 2), (2, 2))));
        assert!(reports.load(Ordering::SeqCst) >= 1);
        p.stop();
    }

    #[test]
    fn many_photos_with_several_workers_are_each_stored_and_grouped_exactly_once() {
        let r = Rig::new();
        r.pngs(&[250u8; 40]);
        let p = r.pipeline(4);
        with_models(&p);
        assert!(wait_until(15000, || r.counts() == ((40, 40), (40, 40))));
        assert!(wait_until(5000, || r.db.list_face_people(pixie_core::FaceView::Groups).unwrap().iter().map(|c| c.faces).collect::<Vec<_>>() == vec![40]));
        assert!(r.hashed_all());
        p.stop();
    }

    #[test]
    fn stop_returns_promptly_even_when_idle() {
        let r = Rig::new();
        let p = r.pipeline(2);
        std::thread::sleep(Duration::from_millis(50));
        let t = Instant::now();
        p.stop();
        assert!(t.elapsed() < Duration::from_secs(2));
    }

    #[test]
    fn the_picture_handed_to_the_search_model_is_shrunk_but_never_enlarged() {
        let big = DynamicImage::ImageRgb8(RgbImage::new(1024, 768));
        let small = for_clip(&big);
        assert_eq!((small.width(), small.height()), (341, 256));
        let tiny = DynamicImage::ImageRgb8(RgbImage::new(100, 50));
        assert_eq!((for_clip(&tiny).width(), for_clip(&tiny).height()), (100, 50));
    }
}
