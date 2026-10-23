//! Finding copies of the same video. (Photos are hashed by the analysis pipeline, which reads each one anyway to
//! decode it; videos are never decoded, so they are hashed here.) Reading every file is slow (on a network drive, very slow), so it is done in the
//! background in two steps: a cheap key for every file (its size plus its first and last 64 KB), then a hash of the
//! whole file only for files whose cheap key another file shares.

use crate::db::Db;
use pixie_core::DupStatus;
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

const EDGE: u64 = 64 * 1024;
const QUICK_BATCH: i64 = 64;
const HASH_BATCH: i64 = 8;
/// The kind (`photos.kind`) of what this service hashes.
const VIDEO: i64 = 1;
const BUSY: Duration = Duration::from_secs(1);
/// How soon a paused hasher notices it has been resumed.
const PAUSED_POLL: Duration = Duration::from_millis(200);
/// What a file that cannot be read gets as its quick key: it matches nothing and is not tried again until it changes.
/// While hashing goes on, how often the hidden copies are brought up to date.
const REFRESH_EVERY: Duration = Duration::from_secs(2);
/// The same when there is nothing to hash.
const IDLE_REFRESH_EVERY: Duration = Duration::from_secs(30);
/// While hashing goes on, how often the progress is worked out and reported.
const STATUS_EVERY: Duration = Duration::from_secs(1);
const UNREADABLE: &str = "!unreadable";

fn hex(d: &[u8]) -> String {
    d.iter().map(|b| format!("{b:02x}")).collect()
}

/// "size:hash of the first and last 64 KB". Equal files always have equal keys.
pub fn quick_key(path: &Path) -> io::Result<String> {
    let mut f = File::open(path)?;
    let size = f.metadata()?.len();
    let mut h = Sha256::new();
    let mut buf = vec![0u8; EDGE.min(size) as usize];
    f.read_exact(&mut buf)?;
    h.update(&buf);
    if size > EDGE {
        let tail = EDGE.min(size - EDGE);
        f.seek(SeekFrom::Start(size - tail))?;
        buf.truncate(tail as usize);
        buf.resize(tail as usize, 0);
        f.read_exact(&mut buf)?;
        h.update(&buf);
    }
    Ok(format!("{size}:{}", hex(&h.finalize())))
}

/// `quick_key` for a file whose bytes are already in memory.
pub fn quick_key_bytes(bytes: &[u8]) -> String {
    let size = bytes.len() as u64;
    let mut h = Sha256::new();
    h.update(&bytes[..EDGE.min(size) as usize]);
    if size > EDGE {
        let tail = EDGE.min(size - EDGE) as usize;
        h.update(&bytes[bytes.len() - tail..]);
    }
    format!("{size}:{}", hex(&h.finalize()))
}

/// `full_hash` for a file whose bytes are already in memory.
pub fn full_hash_bytes(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

/// The hash of the whole file.
pub fn full_hash(path: &Path) -> io::Result<String> {
    let mut f = File::open(path)?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(hex(&h.finalize()))
}

/// One round of work: give quick keys to a batch of files, or hash a batch of probable copies. Returns how many
/// files it handled (0 means there is nothing to do).
pub fn work_once(db: &Db, stop: &AtomicBool, kind: i64) -> usize {
    let quick = db.photos_needing_quick_key(kind, QUICK_BATCH).unwrap_or_default();
    if !quick.is_empty() {
        let mut n = 0;
        for (id, path) in quick {
            if stop.load(Ordering::Relaxed) {
                break;
            }
            let key = quick_key(Path::new(&path)).unwrap_or_else(|_| UNREADABLE.to_string());
            if db.set_quick_key(id, &key).is_ok() {
                n += 1;
            }
        }
        return n;
    }
    let mut n = 0;
    for (id, path) in db.photos_needing_hash(kind, HASH_BATCH).unwrap_or_default() {
        if stop.load(Ordering::Relaxed) {
            break;
        }
        match full_hash(Path::new(&path)) {
            Ok(h) => {
                if db.set_hash(id, &h).is_ok() {
                    n += 1;
                }
            }
            // Gone or locked since the scan: stop it being offered again until the file changes.
            Err(_) => {
                if db.set_quick_key(id, UNREADABLE).is_ok() {
                    n += 1;
                }
            }
        }
    }
    n
}

pub type StatusFn = Arc<dyn Fn(DupStatus) + Send + Sync>;

/// The background hasher. It waits while a scan is running or while paused, and stops when dropped.
pub struct DupeService {
    db: Db,
    on_status: StatusFn,
    /// Ends the thread for good.
    stopped: Arc<AtomicBool>,
    paused: Arc<AtomicBool>,
    /// Stops the round of work in progress: set when stopped or paused (the work checks it between files).
    halt: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl DupeService {
    pub fn start(db: Db, scanning: Arc<dyn Fn() -> bool + Send + Sync>, on_status: StatusFn) -> DupeService {
        Self::start_with(db, scanning, on_status, false)
    }

    pub fn start_with(db: Db, scanning: Arc<dyn Fn() -> bool + Send + Sync>, on_status: StatusFn, paused: bool) -> DupeService {
        let stopped = Arc::new(AtomicBool::new(false));
        let paused = Arc::new(AtomicBool::new(paused));
        let halt = Arc::new(AtomicBool::new(paused.load(Ordering::SeqCst)));
        let (t_db, t_status, t_stopped, t_paused, t_halt) = (db.clone(), on_status.clone(), stopped.clone(), paused.clone(), halt.clone());
        let handle = std::thread::spawn(move || {
            let (db, on_status) = (t_db, t_status);
            let mut last: Option<DupStatus> = None;
            let mut last_refresh: Option<Instant> = None;
            let mut last_status: Option<Instant> = None;
            while !t_stopped.load(Ordering::SeqCst) {
                if t_paused.load(Ordering::SeqCst) {
                    std::thread::sleep(PAUSED_POLL);
                    continue;
                }
                if scanning() {
                    std::thread::sleep(BUSY);
                    continue;
                }
                let did = work_once(&db, &t_halt, VIDEO);
                // Hide the extra copies of videos from Browse: every couple of seconds while hashing goes on, and now
                // and then when idle (the analysis pipeline keeps the flags of photos up to date itself). Browse then
                // refreshes in batches instead of on every file.
                let every = if did > 0 { REFRESH_EVERY } else { IDLE_REFRESH_EVERY };
                if last_refresh.map_or(true, |t| t.elapsed() > every) {
                    let _ = db.refresh_dup_flags();
                    last_refresh = Some(Instant::now());
                }
                // The progress line, about once a second (also while only the photo pipeline is working, so the window
                // follows its hashing). Working it out reads the whole library, and the window waits whenever the
                // database is busy.
                if last_status.map_or(true, |t| t.elapsed() > STATUS_EVERY) {
                    last_status = Some(Instant::now());
                    if let Ok(mut s) = db.dup_status() {
                        s.paused = t_paused.load(Ordering::SeqCst);
                        if last.as_ref() != Some(&s) {
                            on_status(s.clone());
                            last = Some(s);
                        }
                    }
                }
                std::thread::sleep(if did > 0 { Duration::from_millis(20) } else { STATUS_EVERY });
            }
        });
        DupeService { db, on_status, stopped, paused, halt, handle: Some(handle) }
    }

    pub fn is_paused(&self) -> bool {
        self.paused.load(Ordering::SeqCst)
    }

    /// Pause or resume. A round of work already running stops between files, and the new state is reported at once.
    pub fn set_paused(&self, paused: bool) {
        self.paused.store(paused, Ordering::SeqCst);
        self.halt.store(paused || self.stopped.load(Ordering::SeqCst), Ordering::SeqCst);
        if let Ok(mut s) = self.db.dup_status() {
            s.paused = paused;
            (self.on_status)(s);
        }
    }
}

impl Drop for DupeService {
    fn drop(&mut self) {
        self.stopped.store(true, Ordering::SeqCst);
        self.halt.store(true, Ordering::SeqCst);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pixie_core::{MediaKind, PhotoMeta, PhotoQuery};
    use std::io::Write;

    /// What the analysis pipeline does for every photo it reads: the keys, then the photo's duplicate group.
    fn hash_photos(db: &Db) {
        for w in db.photos_needing_work(None, None, 10_000).unwrap() {
            let path = Path::new(&w.photo.path);
            let (quick, full) = (quick_key(path).unwrap(), full_hash(path).unwrap());
            db.set_file_keys(w.photo.id, &quick, Some(&full)).unwrap();
            db.refresh_dup_group(&full).unwrap();
        }
    }

    fn file(dir: &Path, name: &str, bytes: &[u8]) -> String {
        let p = dir.join(name);
        File::create(&p).unwrap().write_all(bytes).unwrap();
        p.to_string_lossy().into_owned()
    }

    #[test]
    fn same_contents_same_keys_different_contents_different_keys() {
        let d = tempfile::tempdir().unwrap();
        let big: Vec<u8> = (0..300_000u32).map(|i| (i % 251) as u8).collect();
        let mut other = big.clone();
        other[150_000] ^= 1; // differs only in the middle: the quick key cannot see it, the full hash can
        let a = file(d.path(), "a.jpg", &big);
        let b = file(d.path(), "b.jpg", &big);
        let c = file(d.path(), "c.jpg", &other);
        assert_eq!(quick_key(Path::new(&a)).unwrap(), quick_key(Path::new(&b)).unwrap());
        assert_eq!(quick_key(Path::new(&a)).unwrap(), quick_key(Path::new(&c)).unwrap());
        assert_eq!(full_hash(Path::new(&a)).unwrap(), full_hash(Path::new(&b)).unwrap());
        assert_ne!(full_hash(Path::new(&a)).unwrap(), full_hash(Path::new(&c)).unwrap());
    }

    #[test]
    fn keys_worked_out_from_bytes_equal_the_ones_read_from_the_file() {
        let d = tempfile::tempdir().unwrap();
        for len in [0usize, 5, 65_536, 65_537, 100_000, 131_072, 300_000] {
            let bytes: Vec<u8> = (0..len as u32).map(|i| (i.wrapping_mul(31) % 253) as u8).collect();
            let path = file(d.path(), &format!("f{len}"), &bytes);
            assert_eq!(quick_key_bytes(&bytes), quick_key(Path::new(&path)).unwrap(), "quick key of {len} bytes");
            assert_eq!(full_hash_bytes(&bytes), full_hash(Path::new(&path)).unwrap(), "hash of {len} bytes");
        }
    }

    #[test]
    fn small_and_empty_files_have_keys() {
        let d = tempfile::tempdir().unwrap();
        let s = file(d.path(), "s", b"hello");
        let e = file(d.path(), "e", b"");
        assert!(quick_key(Path::new(&s)).unwrap().starts_with("5:"));
        assert!(quick_key(Path::new(&e)).unwrap().starts_with("0:"));
        assert_ne!(quick_key(Path::new(&s)).unwrap(), quick_key(Path::new(&file(d.path(), "t", b"hellp"))).unwrap());
    }

    #[test]
    fn finds_copies_but_not_look_alikes() {
        let d = tempfile::tempdir().unwrap();
        let db = Db::open_in_memory().unwrap();
        let folder = db.add_folder(&d.path().to_string_lossy()).unwrap();
        let big: Vec<u8> = (0..300_000u32).map(|i| (i % 251) as u8).collect();
        let mut look_alike = big.clone();
        look_alike[150_000] ^= 1;
        let add = |name: &str, bytes: &[u8]| {
            let path = file(d.path(), name, bytes);
            db.upsert_photo(folder, &path, bytes.len() as i64, 1, &PhotoMeta::default()).unwrap()
        };
        let a = add("a.jpg", &big);
        let b = add("b.jpg", &big);
        let c = add("c.jpg", &look_alike);
        let lone = add("lone.jpg", b"different size");

        hash_photos(&db);

        let groups = db.duplicate_groups().unwrap();
        assert_eq!(groups.len(), 1);
        let ids: Vec<_> = groups[0].photos.iter().map(|p| p.id).collect();
        assert!(ids.contains(&a) && ids.contains(&b) && !ids.contains(&c) && !ids.contains(&lone));
        let s = db.dup_status().unwrap();
        assert_eq!((s.done, s.total, s.groups, s.extra_files, s.extra_bytes), (4, 4, 1, 1, big.len() as i64));
        // The look-alike was hashed (shares a quick key) but is alone; the lone file never needed a full hash.
        assert!(db.photos_needing_hash(0, 10).unwrap().is_empty());
    }

    #[test]
    fn several_groups_come_back_biggest_waste_first_with_the_oldest_copy_first() {
        let d = tempfile::tempdir().unwrap();
        let db = Db::open_in_memory().unwrap();
        let folder = db.add_folder(&d.path().to_string_lossy()).unwrap();
        let add = |name: &str, bytes: &[u8], taken: i64| {
            let path = file(d.path(), name, bytes);
            let meta = PhotoMeta { taken_at: Some(taken), ..PhotoMeta::default() };
            db.upsert_photo(folder, &path, bytes.len() as i64, 1, &meta).unwrap()
        };
        let small_new = add("s1", b"small", 200);
        let small_old = add("s2", b"small", 100);
        let big_a = add("b1", &[7u8; 5000], 300);
        let big_b = add("b2", &[7u8; 5000], 400);
        add("solo", b"unique", 1);
        hash_photos(&db);
        let groups = db.duplicate_groups().unwrap();
        let ids: Vec<Vec<_>> = groups.iter().map(|g| g.photos.iter().map(|p| p.id).collect()).collect();
        assert_eq!(ids, vec![vec![big_a, big_b], vec![small_old, small_new]]);
    }

    /// Three copies (oldest first by date), plus one unrelated photo, all hashed and flagged.
    fn browse_library() -> (tempfile::TempDir, Db, i64, [i64; 3], i64) {
        let d = tempfile::tempdir().unwrap();
        let db = Db::open_in_memory().unwrap();
        let folder = db.add_folder(&d.path().to_string_lossy()).unwrap();
        let add = |name: &str, bytes: &[u8], taken: i64| {
            let path = file(d.path(), name, bytes);
            let meta = PhotoMeta { taken_at: Some(taken), ..PhotoMeta::default() };
            db.upsert_photo(folder, &path, bytes.len() as i64, 1, &meta).unwrap()
        };
        let oldest = add("a.jpg", b"same", 100);
        let middle = add("b.jpg", b"same", 200);
        let newest = add("c.jpg", b"same", 300);
        let other = add("other.jpg", b"different", 150);
        hash_photos(&db);
        db.refresh_dup_flags().unwrap();
        (d, db, folder, [oldest, middle, newest], other)
    }

    fn browse_ids(db: &Db) -> Vec<i64> {
        let mut ids: Vec<i64> = db.list_photos(0, 100).unwrap().iter().map(|p| p.id).collect();
        ids.sort();
        ids
    }

    #[test]
    fn browse_shows_one_item_per_set_and_every_count_agrees() {
        let (_d, db, folder, [oldest, ..], other) = browse_library();
        let mut want = vec![oldest, other];
        want.sort();
        assert_eq!(browse_ids(&db), want, "the oldest copy stands for its set");
        assert_eq!(db.count_photos().unwrap(), 2);
        assert_eq!(db.day_groups(&PhotoQuery::default()).unwrap().iter().map(|g| g.count).sum::<i64>(), 2);
        assert_eq!(db.folder_infos().unwrap().iter().find(|f| f.id == folder).unwrap().count, 2);
        let s = db.dup_status().unwrap();
        assert_eq!((s.groups, s.extra_files, s.hidden), (1, 2, 2));
        // The Duplicates tab still lists every copy.
        assert_eq!(db.duplicate_groups().unwrap()[0].photos.len(), 3);
    }

    #[test]
    fn choosing_another_copy_changes_which_one_browse_shows() {
        let (_d, db, _f, [oldest, _middle, newest], other) = browse_library();
        db.set_keep_pick(newest).unwrap();
        let mut want = vec![newest, other];
        want.sort();
        assert_eq!(browse_ids(&db), want);
        assert_eq!(db.duplicate_groups().unwrap()[0].keeper, newest);
        // Still the pick after another refresh (a rescan, more hashing).
        db.refresh_dup_flags().unwrap();
        assert!(browse_ids(&db).contains(&newest) && !browse_ids(&db).contains(&oldest));
    }

    #[test]
    fn losing_copies_brings_the_survivor_back_and_a_set_of_one_is_not_a_duplicate() {
        let (_d, db, _f, [oldest, middle, newest], _other) = browse_library();
        // The shown copy is removed: another takes its place.
        db.mark_removed(oldest).unwrap();
        db.refresh_dup_flags().unwrap();
        assert!(browse_ids(&db).contains(&middle) && !browse_ids(&db).contains(&newest));
        // Down to one live copy: it is just a photo again.
        db.mark_removed(middle).unwrap();
        db.refresh_dup_flags().unwrap();
        assert!(browse_ids(&db).contains(&newest));
        assert_eq!(db.dup_status().unwrap().hidden, 0);
        assert!(db.duplicate_groups().unwrap().is_empty());
    }

    #[test]
    fn an_edited_copy_is_no_longer_hidden() {
        let (d, db, folder, [_oldest, middle, _newest], _other) = browse_library();
        assert!(!browse_ids(&db).contains(&middle));
        let path = d.path().join("b.jpg");
        std::fs::write(&path, b"edited and bigger now").unwrap();
        db.upsert_photo(folder, &path.to_string_lossy(), 21, 2, &PhotoMeta::default()).unwrap(); // rescan sees new size
        assert!(browse_ids(&db).contains(&middle), "a changed file is not known to be a copy until it is hashed again");
    }

    #[test]
    fn tags_and_people_do_not_count_hidden_copies() {
        let (_d, db, _f, [oldest, _m, newest], _o) = browse_library();
        db.set_tags(oldest, "m", "t", &[("beach".to_string(), 0.9)]).unwrap();
        db.set_tags(newest, "m", "t", &[("beach".to_string(), 0.9)]).unwrap();
        assert_eq!(db.tag_counts().unwrap(), vec![("beach".to_string(), 1)], "two copies of one photo are one photo");
        assert_eq!(db.photos_with_any_tag(&["beach"]).unwrap(), 1);
    }

    #[test]
    fn hidden_copies_are_not_queued_for_faces_and_do_not_count_toward_their_progress() {
        let (_d, db, _f, [oldest, ..], other) = browse_library();
        let mut queued: Vec<i64> = db.photos_needing_faces("v", 100).unwrap().iter().map(|p| p.id).collect();
        queued.sort();
        let mut want = vec![oldest, other];
        want.sort();
        assert_eq!(queued, want);
        assert_eq!(db.face_counts("v").unwrap(), (0, 2));
        db.replace_faces(oldest, "v", &[]).unwrap();
        assert_eq!(db.face_counts("v").unwrap(), (1, 2));
    }

    #[test]
    fn pause_holds_the_work_and_resume_continues() {
        let d = tempfile::tempdir().unwrap();
        let db = Db::open_in_memory().unwrap();
        let folder = db.add_folder(&d.path().to_string_lossy()).unwrap();
        // Videos: this service hashes them (the analysis pipeline hashes photos as it reads them).
        let video = PhotoMeta { kind: MediaKind::Video, ..PhotoMeta::default() };
        for name in ["a", "b", "c"] {
            let path = file(d.path(), name, b"same bytes");
            db.upsert_photo(folder, &path, 10, 1, &video).unwrap();
        }
        let photo = file(d.path(), "p.jpg", b"same bytes");
        db.upsert_photo(folder, &photo, 10, 1, &PhotoMeta::default()).unwrap();
        let seen = Arc::new(std::sync::Mutex::new(Vec::<DupStatus>::new()));
        let s2 = seen.clone();
        let svc = DupeService::start_with(db.clone(), Arc::new(|| false), Arc::new(move |s| s2.lock().unwrap().push(s)), true);
        std::thread::sleep(Duration::from_millis(400));
        assert_eq!(db.photos_needing_quick_key(1, 10).unwrap().len(), 3, "nothing is looked at while paused");
        assert!(svc.is_paused());
        svc.set_paused(false);
        let t = Instant::now();
        while db.dup_status().unwrap().groups == 0 && t.elapsed() < Duration::from_secs(10) {
            std::thread::sleep(Duration::from_millis(50));
        }
        assert_eq!(db.dup_status().unwrap().groups, 1, "the three videos are one set; the photo is left to the pipeline");
        svc.set_paused(true);
        assert!(seen.lock().unwrap().last().unwrap().paused, "pausing is reported at once");
        drop(svc);
    }

    #[test]
    fn a_whole_group_can_never_be_removed() {
        let d = tempfile::tempdir().unwrap();
        let db = Db::open_in_memory().unwrap();
        let folder = db.add_folder(&d.path().to_string_lossy()).unwrap();
        let add = |name: &str| {
            let path = file(d.path(), name, b"same bytes");
            db.upsert_photo(folder, &path, 10, 1, &PhotoMeta::default()).unwrap()
        };
        let (a, b, c) = (add("a"), add("b"), add("c"));
        hash_photos(&db);
        assert_eq!(db.removable_copies(&[b, c]).unwrap().len(), 2);
        assert_eq!(db.removable_copies(&[a, b, c]).unwrap().len(), 0, "asking for every copy removes none");
        db.mark_removed(b).unwrap();
        assert_eq!(db.removable_copies(&[a]).unwrap().len(), 1, "c is still a live copy");
        assert_eq!(db.removable_copies(&[a, c]).unwrap().len(), 0);
    }

    #[test]
    fn a_changed_file_is_hashed_again() {
        let d = tempfile::tempdir().unwrap();
        let db = Db::open_in_memory().unwrap();
        let folder = db.add_folder(&d.path().to_string_lossy()).unwrap();
        let p1 = file(d.path(), "a", b"one");
        let p2 = file(d.path(), "b", b"one");
        db.upsert_photo(folder, &p1, 3, 1, &PhotoMeta::default()).unwrap();
        db.upsert_photo(folder, &p2, 3, 1, &PhotoMeta::default()).unwrap();
        hash_photos(&db);
        assert_eq!(db.duplicate_groups().unwrap().len(), 1);
        db.upsert_photo(folder, &p2, 4, 2, &PhotoMeta::default()).unwrap(); // rescanned after an edit
        assert_eq!(db.photos_needing_quick_key(0, 10).unwrap().len(), 1);
    }
}
