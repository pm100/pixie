//! The Inbox: a folder in Pixie's data directory that holds what was extracted from zip files (and, later, copied from phones) until it is kept
//! or discarded. Each item is a temporary source (see `source.rs`) whose folder is below the Inbox, so Browse, picking and Keep work on it as on a
//! card; Keep moves its files out, and Discard deletes what is left.

use crate::db::Db;
use crate::source::plain;
use pixie_core::{FolderId, PhotoId, PixieError, Result};
use std::path::{Path, PathBuf};

/// `<data>/inbox`, made if it is missing.
pub fn inbox_root(data: &Path) -> std::io::Result<PathBuf> {
    let root = data.join("inbox");
    std::fs::create_dir_all(&root)?;
    Ok(root)
}

/// A new empty folder for one zip: `inbox/<n>-<its name>`, with the first number above those already used.
pub fn new_inbox_dir(root: &Path, label: &str) -> std::io::Result<PathBuf> {
    let stem = match label.rsplit_once('.') {
        Some((stem, ext)) if ext.eq_ignore_ascii_case("zip") => stem,
        _ => label,
    };
    let name = crate::zipsrc::safe_name(stem).map(|n| n.replace('/', "_")).unwrap_or_else(|_| "zip".to_string());
    let used = std::fs::read_dir(root)?
        .flatten()
        .filter_map(|e| e.file_name().to_string_lossy().split('-').next().and_then(|n| n.parse::<u32>().ok()))
        .max()
        .unwrap_or(0);
    let dir = root.join(format!("{}-{name}", used + 1));
    std::fs::create_dir(&dir)?;
    Ok(dir)
}

/// The identity of a zip: the same file again gives the same key, and a changed one a different key (so what was kept from it is remembered).
pub fn zip_key(zip: &Path) -> std::io::Result<String> {
    let meta = std::fs::metadata(zip)?;
    let ms = meta.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_millis()).unwrap_or(0);
    let name = zip.file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default();
    Ok(format!("zip:{}:{ms}:{name}", meta.len()))
}

#[cfg(windows)]
extern "system" {
    fn GetDiskFreeSpaceExW(dir: *const u16, free_to_caller: *mut u64, total: *mut u64, total_free: *mut u64) -> i32;
}

/// Bytes free on the volume that holds `path` (or the nearest folder of it that exists).
pub fn free_space(path: &Path) -> std::io::Result<u64> {
    let mut at = path;
    while !at.exists() {
        match at.parent() {
            Some(p) => at = p,
            None => break,
        }
    }
    free_space_of(at)
}

#[cfg(windows)]
fn free_space_of(dir: &Path) -> std::io::Result<u64> {
    let wide: Vec<u16> = dir.to_string_lossy().encode_utf16().chain(std::iter::once(0)).collect();
    let mut free = 0u64;
    // SAFETY: `wide` is NUL-terminated and outlives the call; `free` is a writable u64; the other outputs are optional and passed as null.
    let ok = unsafe { GetDiskFreeSpaceExW(wide.as_ptr(), &mut free, std::ptr::null_mut(), std::ptr::null_mut()) };
    if ok == 0 { Err(std::io::Error::last_os_error()) } else { Ok(free) }
}

#[cfg(unix)]
fn free_space_of(dir: &Path) -> std::io::Result<u64> {
    use std::os::unix::ffi::OsStrExt;
    let c = std::ffi::CString::new(dir.as_os_str().as_bytes()).map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidInput, "the path has a NUL in it"))?;
    // SAFETY: `c` is a NUL-terminated path and `stat` is a zeroed struct that statvfs fills in.
    let mut stat: libc::statvfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::statvfs(c.as_ptr(), &mut stat) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(stat.f_bavail as u64 * stat.f_frsize as u64)
}

/// Whether `path` is an item of the Inbox: strictly below its root.
pub fn is_in_inbox(root: &Path, path: &str) -> bool {
    if path.replace('\\', "/").split('/').any(|p| p == "..") {
        return false;
    }
    plain(path).starts_with(&format!("{}/", plain(&root.to_string_lossy())))
}

/// Deletes what is left of an Inbox source (its files and folder), then the source with its rows and picks. Returns the removed items' ids.
/// Refuses a source that is not in the Inbox (a card is never deleted from).
pub fn discard_source(db: &Db, root: &Path, source: FolderId) -> Result<Vec<PhotoId>> {
    let src = db.list_sources()?.into_iter().find(|s| s.id == source).ok_or_else(|| PixieError::Invalid("That is not an open source.".into()))?;
    if !is_in_inbox(root, &src.path) {
        return Err(PixieError::Invalid("That is not in the Inbox, so Pixie will not delete it.".into()));
    }
    match std::fs::remove_dir_all(&src.path) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(PixieError::Invalid(format!("Some of the Inbox could not be deleted ({e})."))),
    }
    db.remove_source(source)
}

/// Removes the Inbox folders that no source points at (after a crash, or a discard that was cut short). Returns how many.
pub fn remove_leftovers(db: &Db, root: &Path) -> std::io::Result<usize> {
    let known: Vec<String> = db.list_sources().unwrap_or_default().iter().map(|s| plain(&s.path)).collect();
    let mut removed = 0;
    for entry in std::fs::read_dir(root)?.flatten() {
        let path = entry.path();
        if path.is_dir() && !known.contains(&plain(&path.to_string_lossy())) {
            std::fs::remove_dir_all(&path)?;
            removed += 1;
        }
    }
    Ok(removed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Db;
    use crate::source::scan_source;
    use std::sync::atomic::AtomicBool;

    #[test]
    fn the_inbox_root_is_made_and_each_zip_gets_its_own_numbered_folder() {
        let data = tempfile::tempdir().unwrap();
        let root = inbox_root(data.path()).unwrap();
        assert!(root.is_dir() && root.ends_with("inbox"));
        let a = new_inbox_dir(&root, "Holiday 2024.zip").unwrap();
        let b = new_inbox_dir(&root, "Holiday 2024.zip").unwrap();
        assert_eq!(a.file_name().unwrap().to_string_lossy(), "1-Holiday 2024");
        assert_eq!(b.file_name().unwrap().to_string_lossy(), "2-Holiday 2024");
        assert!(a.is_dir() && b.is_dir());
        let odd = new_inbox_dir(&root, "a<b>:c?.zip").unwrap();
        assert_eq!(odd.file_name().unwrap().to_string_lossy(), "3-a_b__c_");
        let nameless = new_inbox_dir(&root, "...").unwrap();
        assert!(nameless.file_name().unwrap().to_string_lossy().starts_with("4-"), "a name that is nothing still gets a folder");
    }

    #[test]
    fn a_zips_key_is_the_same_for_the_same_file_and_different_when_it_changes() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("Trip.ZIP");
        std::fs::write(&p, b"12345").unwrap();
        let first = zip_key(&p).unwrap();
        assert_eq!(zip_key(&p).unwrap(), first);
        assert!(first.starts_with("zip:5:") && first.ends_with(":trip.zip"), "{first}");
        std::fs::write(&p, b"1234567").unwrap();
        assert_ne!(zip_key(&p).unwrap(), first);
    }

    #[test]
    fn free_space_is_a_sensible_number_even_for_a_folder_not_made_yet() {
        let dir = tempfile::tempdir().unwrap();
        let free = free_space(dir.path()).unwrap();
        assert!(free > 0 && free < u64::MAX);
        assert_eq!(free_space(&dir.path().join("not").join("made").join("yet")).unwrap() > 0, true);
    }

    #[test]
    fn only_paths_below_the_root_are_in_the_inbox() {
        let root = Path::new("/data/inbox");
        assert!(is_in_inbox(root, "/data/inbox/1-trip"));
        assert!(is_in_inbox(root, "/data/inbox/1-trip/DCIM"));
        assert!(!is_in_inbox(root, "/data/inbox"), "the root itself is not an item");
        assert!(!is_in_inbox(root, "/data/inbox2/1-trip"));
        assert!(!is_in_inbox(root, "/data/other"));
        assert!(!is_in_inbox(root, "/data/inbox/../other"), "dot-dot does not count as below");
    }

    /// An Inbox with one source holding two scanned photos.
    fn inbox_source() -> (Db, tempfile::TempDir, std::path::PathBuf, i64) {
        let data = tempfile::tempdir().unwrap();
        let root = inbox_root(data.path()).unwrap();
        let dir = new_inbox_dir(&root, "x.zip").unwrap();
        for i in 0..2 {
            std::fs::write(dir.join(format!("p{i}.jpg")), pixie_testutil::jpeg(8 + i, 6)).unwrap();
        }
        let db = Db::open_in_memory().unwrap();
        let src = db.add_source(dir.to_str().unwrap(), "zip:1:2:x.zip", "x").unwrap();
        scan_source(&db, src, &AtomicBool::new(false), |_| {}).unwrap();
        (db, data, root, src)
    }

    #[test]
    fn discarding_an_inbox_source_deletes_its_files_rows_and_picks() {
        let (db, _data, root, src) = inbox_source();
        let lib = db.add_folder("/lib").unwrap();
        let l = db.upsert_photo(lib, "/lib/a.jpg", 1, 1, &Default::default()).unwrap();
        let ids = db.picked_in(src).unwrap();
        assert!(ids.is_empty());
        let first = db.list_photos_in(&pixie_core::PhotoQuery { folder_id: Some(src), ..Default::default() }, 0, 10).unwrap()[0].id;
        db.set_picks(&[first, l]).unwrap();
        let dir = std::path::PathBuf::from(&db.list_sources().unwrap()[0].path);
        let removed = discard_source(&db, &root, src).unwrap();
        assert_eq!(removed.len(), 2);
        assert!(!dir.exists(), "its folder is gone from the disk");
        assert!(db.list_sources().unwrap().is_empty());
        assert_eq!(db.pick_ids().unwrap(), vec![l], "only the library photo is still picked");
        assert_eq!(db.count_photos().unwrap(), 1);
    }

    #[test]
    fn a_source_outside_the_inbox_is_never_discarded() {
        let card = tempfile::tempdir().unwrap();
        std::fs::write(card.path().join("a.jpg"), pixie_testutil::jpeg(8, 6)).unwrap();
        let data = tempfile::tempdir().unwrap();
        let root = inbox_root(data.path()).unwrap();
        let db = Db::open_in_memory().unwrap();
        let src = db.add_source(card.path().to_str().unwrap(), "K", "Card").unwrap();
        scan_source(&db, src, &AtomicBool::new(false), |_| {}).unwrap();
        assert!(discard_source(&db, &root, src).is_err());
        assert!(card.path().join("a.jpg").is_file(), "the card is untouched");
        assert_eq!(db.list_sources().unwrap().len(), 1);
    }

    #[test]
    fn inbox_folders_that_no_source_points_at_are_removed() {
        let (db, _data, root, _src) = inbox_source();
        let orphan_a = new_inbox_dir(&root, "gone.zip").unwrap();
        std::fs::write(orphan_a.join("x.jpg"), b"x").unwrap();
        let orphan_b = new_inbox_dir(&root, "cut short.zip").unwrap();
        assert_eq!(remove_leftovers(&db, &root).unwrap(), 2);
        assert!(!orphan_a.exists() && !orphan_b.exists());
        assert_eq!(std::fs::read_dir(&root).unwrap().count(), 1, "the one with a source is kept");
    }
}
