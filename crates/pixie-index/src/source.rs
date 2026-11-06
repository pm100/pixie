//! Temporary sources: a card, USB stick or folder that is looked at where it is before anything is copied from it. A source is a
//! `folders` row with `kind = 'source'`; its photos have `photos.hidden = 2` (`HIDDEN_SOURCE`), which every library listing leaves out
//! (the same way the photos the person hid, `hidden = 1`, are left out).

use crate::db::{db_err, Db};
use crate::scanner::{scan_folder, ScanProgress, ScanStats};
use pixie_core::{FolderId, PhotoId, PixieError, Result};
use rusqlite::params;
use std::sync::atomic::AtomicBool;

/// A temporary source, with how many items it holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceInfo {
    pub id: FolderId,
    pub path: String,
    /// Which drive it is (the volume's serial number or id), so it is known again when it is plugged in again.
    pub key: String,
    /// What to call it ("SONY card").
    pub label: String,
    pub count: i64,
}

/// A path as compared for overlap: forward slashes, no trailing slash, and lower case where the file system ignores case.
fn plain(path: &str) -> String {
    let p = path.replace('\\', "/");
    let p = p.trim_end_matches('/');
    if cfg!(any(windows, target_os = "macos")) { p.to_lowercase() } else { p.to_string() }
}

/// The path of `path` below `root` with forward slashes (the way a source's items are named in the import log), or the whole path when it is
/// not below `root`.
pub(crate) fn relative_to(root: &str, path: &str) -> String {
    let (root, path) = (root.replace('\\', "/"), path.replace('\\', "/"));
    let root = root.trim_end_matches('/');
    match path.strip_prefix(root) {
        Some(rest) if rest.starts_with('/') => rest.trim_start_matches('/').to_string(),
        _ => path,
    }
}

/// Whether two folder paths are the same folder or one is inside the other.
pub(crate) fn overlaps(a: &str, b: &str) -> bool {
    let (a, b) = (plain(a), plain(b));
    let inside = |inner: &str, outer: &str| outer.is_empty() || inner == outer || inner.starts_with(&format!("{outer}/"));
    inside(&a, &b) || inside(&b, &a)
}

impl Db {
    /// Adds a temporary source (or returns the one already added for this path). It cannot be, hold or sit inside a library folder (the
    /// scan would take over that folder's photos) or another source.
    pub fn add_source(&self, path: &str, key: &str, label: &str) -> Result<FolderId> {
        let c = self.conn();
        let mut s = c.prepare("SELECT id, path, kind, label FROM folders").map_err(db_err)?;
        let rows = s
            .query_map([], |r| Ok((r.get::<_, FolderId>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, Option<String>>(3)?)))
            .map_err(db_err)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(db_err)?;
        for (id, other, kind, other_label) in rows {
            if plain(&other) == plain(path) && kind == "source" {
                return Ok(id);
            }
            if !overlaps(&other, path) {
                continue;
            }
            return Err(PixieError::Invalid(if kind == "source" {
                format!("{} is already open (it overlaps this folder). Close it first.", other_label.unwrap_or(other))
            } else {
                format!("{path} is, or is inside, or holds, {other}, which is already in your library.")
            }));
        }
        drop(s);
        c.execute("INSERT INTO folders(path, kind, source_key, label) VALUES (?1, 'source', ?2, ?3)", params![path, key, label]).map_err(db_err)?;
        Ok(c.last_insert_rowid())
    }

    /// The sources, with how many items each holds.
    pub fn list_sources(&self) -> Result<Vec<SourceInfo>> {
        let c = self.conn();
        let mut s = c
            .prepare(
                "SELECT f.id, f.path, COALESCE(f.source_key, ''), COALESCE(f.label, ''), COUNT(p.id) FROM folders f
                 LEFT JOIN photos p ON p.folder_id = f.id AND p.missing = 0
                 WHERE f.kind = 'source' GROUP BY f.id ORDER BY f.id",
            )
            .map_err(db_err)?;
        let rows = s
            .query_map([], |r| Ok(SourceInfo { id: r.get(0)?, path: r.get(1)?, key: r.get(2)?, label: r.get(3)?, count: r.get(4)? }))
            .map_err(db_err)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(db_err)?;
        Ok(rows)
    }

    /// Forgets a source with its items, and with them their picks, tags and everything else kept about them (every table that points at a photo
    /// cascades). Returns the removed items' ids, so the caller can delete their thumbnails. Does nothing to a library folder.
    pub fn remove_source(&self, id: FolderId) -> Result<Vec<PhotoId>> {
        let c = self.conn();
        let is_source: bool = c.query_row("SELECT EXISTS(SELECT 1 FROM folders WHERE id = ?1 AND kind = 'source')", params![id], |r| r.get(0)).map_err(db_err)?;
        if !is_source {
            return Ok(Vec::new());
        }
        let ids: Vec<PhotoId> = {
            let mut s = c.prepare("SELECT id FROM photos WHERE folder_id = ?1 ORDER BY id").map_err(db_err)?;
            let rows = s.query_map(params![id], |r| r.get(0)).map_err(db_err)?.collect::<std::result::Result<Vec<_>, _>>().map_err(db_err)?;
            rows
        };
        // The temporary tables of the current search are not tied to photos by a key.
        for table in ["search_hits", "text_hits", "tag_hits", "person_hits", "camera_hits", "missing_hits", "share_hits", "utag_hits"] {
            c.execute(&format!("DELETE FROM {table} WHERE photo_id IN (SELECT id FROM photos WHERE folder_id = ?1)"), params![id]).map_err(db_err)?;
        }
        c.execute("DELETE FROM folders WHERE id = ?1 AND kind = 'source'", params![id]).map_err(db_err)?;
        Ok(ids)
    }

    /// Removes every source whose key is not in `present` (its drive is gone), at start-up and when a drive is unplugged. Returns the removed
    /// items' ids.
    pub fn remove_sources_except(&self, present: &[String]) -> Result<Vec<PhotoId>> {
        let gone: Vec<FolderId> = self.list_sources()?.into_iter().filter(|s| !present.contains(&s.key)).map(|s| s.id).collect();
        let mut removed = Vec::new();
        for id in gone {
            removed.extend(self.remove_source(id)?);
        }
        Ok(removed)
    }
}

/// Indexes a source where it is: each new or changed photo or video gets its row, with the date, the size and the dimensions (and a video's
/// length) read from the file's header, on many threads, with Stop. Nothing is hashed or analysed and the analysis pipeline is not told: the
/// source's photos are `hidden = 2` as the database adds them. Files that have gone from the source since the last scan are marked missing.
/// Refuses a library folder (those are read by the analysis pipeline).
pub fn scan_source(db: &Db, folder_id: FolderId, cancel: &AtomicBool, on_progress: impl FnMut(&ScanProgress)) -> Result<ScanStats> {
    let is_source: bool = db.conn().query_row("SELECT EXISTS(SELECT 1 FROM folders WHERE id = ?1 AND kind = 'source')", params![folder_id], |r| r.get(0)).map_err(db_err)?;
    if !is_source {
        return Err(PixieError::Invalid("That folder is in your library, not a source.".into()));
    }
    scan_folder(db, folder_id, cancel, on_progress)
}

#[cfg(test)]
mod tests {
    use super::{scan_source, SourceInfo};
    use std::sync::atomic::AtomicBool;
    use crate::db::{Db, HIDDEN_SOURCE};
    use pixie_core::{PhotoMeta, PhotoQuery};

    fn meta() -> PhotoMeta {
        PhotoMeta { taken_at: Some(1_700_000_000), gps_lat: Some(10.0), gps_lon: Some(20.0), camera: Some("Cam X".into()), ..Default::default() }
    }

    /// A library folder with one photo, and a source with one equivalent photo, both dated, placed, tagged and picked.
    fn library_and_source() -> (Db, i64, i64, i64, i64) {
        let db = Db::open_in_memory().unwrap();
        let lib = db.add_folder("/lib").unwrap();
        let l = db.upsert_photo(lib, "/lib/a.jpg", 10, 1000, &meta()).unwrap();
        db.conn().execute("INSERT INTO folders(path, kind, source_key, label) VALUES ('/card', 'source', 'K', 'Card')", []).unwrap();
        let src = db.conn().last_insert_rowid();
        let s = db.upsert_photo(src, "/card/b.jpg", 10, 1000, &meta()).unwrap();
        db.tag_photos(&[l, s], "holiday").unwrap();
        db.set_picks(&[l, s]).unwrap();
        (db, lib, l, src, s)
    }

    #[test]
    fn a_photo_added_to_a_source_is_marked_by_the_database_itself() {
        let (db, _, l, _, s) = library_and_source();
        let hidden = |id| db.conn().query_row("SELECT hidden FROM photos WHERE id = ?1", [id], |r| r.get::<_, i64>(0)).unwrap();
        assert_eq!(hidden(s), HIDDEN_SOURCE);
        assert_eq!(hidden(l), 0);
    }

    #[test]
    fn sources_never_appear_in_the_library() {
        let (db, lib, l, src, s) = library_and_source();
        let ids = |v: Vec<pixie_core::Photo>| v.into_iter().map(|p| p.id).collect::<Vec<_>>();
        let all = PhotoQuery::default();
        assert_eq!(ids(db.list_photos(0, 10).unwrap()), vec![l]);
        assert_eq!(db.count_photos().unwrap(), 1);
        assert_eq!(ids(db.list_photos_in(&all, 0, 10).unwrap()), vec![l]);
        assert_eq!(db.count_photos_in(&all).unwrap(), 1);
        assert_eq!(db.day_groups(&all).unwrap().iter().map(|g| g.count).sum::<i64>(), 1);
        assert_eq!(db.location_counts().unwrap().2, 1, "the tree's location counts");
        assert_eq!(db.hidden_count().unwrap(), 0, "a source's photos are not the ones the person hid");
        assert_eq!(db.list_folders().unwrap().iter().map(|f| f.id).collect::<Vec<_>>(), vec![lib]);
        assert_eq!(db.folder_infos().unwrap().iter().map(|f| f.id).collect::<Vec<_>>(), vec![lib]);
        assert_eq!(db.user_tags().unwrap().iter().map(|t| t.count).sum::<i64>(), 1, "the tree's tag counts");
        assert_eq!(db.trip_items(1_800_000_000).unwrap().len(), 1, "trips and the map");
        let work: Vec<_> = db.photos_needing_work(Some("m1"), Some("f1"), false, 10).unwrap().into_iter().map(|w| w.photo.id).collect();
        assert_eq!(work, vec![l], "the analysis never reads a source");
        assert_eq!(db.embedding_counts("m1").unwrap().1, 1, "nor counts it in its totals");
        assert_eq!(db.face_counts("f1").unwrap().1, 1);

        // The source's own view: asking for its folder lists it, and only it.
        let own = PhotoQuery { folder_id: Some(src), ..Default::default() };
        assert_eq!(ids(db.list_photos_in(&own, 0, 10).unwrap()), vec![s]);
        assert_eq!(db.count_photos_in(&own).unwrap(), 1);
        let library = PhotoQuery { folder_id: Some(lib), ..Default::default() };
        assert_eq!(ids(db.list_photos_in(&library, 0, 10).unwrap()), vec![l]);

        // "Show hidden" shows what the person hid, never a source.
        db.set_show_hidden(true);
        assert_eq!(ids(db.list_photos_in(&all, 0, 10).unwrap()), vec![l]);
        assert_eq!(ids(db.list_photos_in(&own, 0, 10).unwrap()), vec![s]);
    }

    #[test]
    fn paths_overlap_when_equal_or_one_is_inside_the_other() {
        use super::overlaps;
        assert!(overlaps("/card", "/card"));
        assert!(overlaps("/card/", "/card"));
        assert!(overlaps("/card/DCIM", "/card"));
        assert!(overlaps("/card", "/card/DCIM"));
        assert!(overlaps("C:\\Photos\\2024", "C:\\Photos"));
        assert!(!overlaps("/card2", "/card"), "a longer name is not inside");
        assert!(!overlaps("/a/b", "/a/c"));
        if cfg!(any(windows, target_os = "macos")) {
            assert!(overlaps("/Card/dcim", "/card"), "these systems ignore case");
        }
    }

    #[test]
    fn a_source_is_added_listed_and_counted() {
        let db = Db::open_in_memory().unwrap();
        let id = db.add_source("/card", "K", "Card").unwrap();
        let sources = db.list_sources().unwrap();
        assert_eq!(sources, vec![SourceInfo { id, path: "/card".into(), key: "K".into(), label: "Card".into(), count: 0 }]);
        assert_eq!(db.add_source("/card", "K", "Card").unwrap(), id, "the same path is the same source");
        db.upsert_photo(id, "/card/a.jpg", 1, 1, &meta()).unwrap();
        assert_eq!(db.list_sources().unwrap()[0].count, 1);
        assert!(db.list_folders().unwrap().is_empty());
    }

    #[test]
    fn a_path_that_overlaps_a_library_folder_or_another_source_cannot_be_a_source() {
        let db = Db::open_in_memory().unwrap();
        db.add_folder("/lib").unwrap();
        for path in ["/lib", "/lib/sub", "/"] {
            let e = db.add_source(path, "K", "Card").unwrap_err().user_message();
            assert!(e.contains("library"), "{path}: {e}");
        }
        db.add_source("/card", "K", "Card").unwrap();
        let e = db.add_source("/card/DCIM", "K2", "Inner").unwrap_err().user_message();
        assert!(e.contains("Card"), "names the source it overlaps: {e}");
        assert_eq!(db.list_sources().unwrap().len(), 1);
    }

    #[test]
    fn adding_a_library_folder_over_a_source_replaces_it() {
        let db = Db::open_in_memory().unwrap();
        let src = db.add_source("/card", "K", "Card").unwrap();
        db.upsert_photo(src, "/card/a.jpg", 1, 1, &meta()).unwrap();
        db.add_folder("/card").unwrap();
        assert!(db.list_sources().unwrap().is_empty());
        assert_eq!(db.list_folders().unwrap().iter().map(|f| f.path.as_str()).collect::<Vec<_>>(), vec!["/card"]);
        assert_eq!(db.conn().query_row("SELECT COUNT(*) FROM photos", [], |r| r.get::<_, i64>(0)).unwrap(), 0, "the temporary rows do not linger");
    }

    #[test]
    fn removing_a_source_leaves_nothing_behind() {
        let (db, _lib, l, src, s) = library_and_source();
        let s2 = db.upsert_photo(src, "/card/c.jpg", 20, 2000, &meta()).unwrap();
        for id in [s, s2] {
            db.conn().execute("INSERT INTO embeddings(photo_id, model_version, vector) VALUES (?1, 'm1', x'00')", [id]).unwrap();
            db.conn().execute("INSERT INTO photo_tags(photo_id, tag, score) VALUES (?1, 'beach', 0.5)", [id]).unwrap();
            db.conn().execute("INSERT INTO face_scans(photo_id, version) VALUES (?1, 'f1')", [id]).unwrap();
            db.conn().execute("INSERT INTO faces(photo_id, x, y, w, h, score, embedding) VALUES (?1, 0, 0, 1, 1, 1, x'00')", [id]).unwrap();
            db.conn().execute("INSERT INTO search_hits(photo_id) VALUES (?1)", [id]).unwrap();
        }
        let mut removed = db.remove_source(src).unwrap();
        removed.sort();
        assert_eq!(removed, vec![s, s2]);
        // No table, permanent or temporary, still has a row for either id.
        let conn = db.conn();
        let tables: Vec<String> = {
            let mut q = conn.prepare("SELECT name FROM sqlite_master WHERE type = 'table' UNION SELECT name FROM sqlite_temp_master WHERE type = 'table'").unwrap();
            q.query_map([], |r| r.get(0)).unwrap().map(|t| t.unwrap()).collect()
        };
        let mut checked = 0;
        for t in tables {
            let has_photo_id = {
                let mut q = conn.prepare(&format!("PRAGMA table_info({t})")).unwrap();
                let names: Vec<String> = q.query_map([], |r| r.get(1)).unwrap().map(|n| n.unwrap()).collect();
                names.iter().any(|n| n == "photo_id")
            };
            if !has_photo_id {
                continue;
            }
            checked += 1;
            let left: i64 = conn.query_row(&format!("SELECT COUNT(*) FROM {t} WHERE photo_id IN (?1, ?2)"), [s, s2], |r| r.get(0)).unwrap();
            assert_eq!(left, 0, "{t} still has rows for the removed source");
        }
        assert!(checked >= 8, "found the tables that point at photos ({checked})");
        let library_left: i64 = conn.query_row("SELECT COUNT(*) FROM photos WHERE id = ?1", [l], |r| r.get(0)).unwrap();
        assert_eq!(library_left, 1, "the library photo is untouched");
        let folders: i64 = conn.query_row("SELECT COUNT(*) FROM folders WHERE kind = 'source'", [], |r| r.get(0)).unwrap();
        assert_eq!(folders, 0);
    }

    #[test]
    fn remove_source_never_touches_a_library_folder() {
        let (db, lib, l, _, _) = library_and_source();
        assert!(db.remove_source(lib).unwrap().is_empty());
        assert_eq!(db.list_folders().unwrap().len(), 1);
        assert_eq!(db.count_photos().unwrap(), 1);
        assert_eq!(db.list_photos(0, 10).unwrap()[0].id, l);
    }

    #[test]
    fn sources_whose_drive_is_gone_are_removed() {
        let db = Db::open_in_memory().unwrap();
        let a = db.add_source("/a", "K1", "A").unwrap();
        let b = db.add_source("/b", "K2", "B").unwrap();
        let pb = db.upsert_photo(b, "/b/x.jpg", 1, 1, &meta()).unwrap();
        db.upsert_photo(a, "/a/y.jpg", 1, 1, &meta()).unwrap();
        assert_eq!(db.remove_sources_except(&["K1".to_string()]).unwrap(), vec![pb]);
        assert_eq!(db.list_sources().unwrap().iter().map(|s| s.id).collect::<Vec<_>>(), vec![a]);
        assert!(db.remove_sources_except(&["K1".to_string()]).unwrap().is_empty());
    }

    /// A temporary directory with these files (name, bytes).
    fn card(files: &[(&str, Vec<u8>)]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        for (name, bytes) in files {
            let path = dir.path().join(name);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, bytes).unwrap();
        }
        dir
    }
    fn source_of(db: &Db, dir: &tempfile::TempDir) -> i64 {
        db.add_source(dir.path().to_str().unwrap(), "K", "Card").unwrap()
    }
    fn own(src: i64) -> PhotoQuery {
        PhotoQuery { folder_id: Some(src), ..Default::default() }
    }

    #[test]
    fn a_scan_lists_the_photos_and_nothing_else() {
        let dir = card(&[("DCIM/a.jpg", pixie_testutil::jpeg(8, 6)), ("DCIM/b.jpg", pixie_testutil::jpeg(8, 6)), ("c.jpg", pixie_testutil::jpeg(8, 6)), ("notes.txt", b"hi".to_vec())]);
        let db = Db::open_in_memory().unwrap();
        let src = source_of(&db, &dir);
        let stats = scan_source(&db, src, &AtomicBool::new(false), |_| {}).unwrap();
        assert_eq!((stats.added, stats.unreadable, stats.cancelled), (3, 0, false));
        assert_eq!(db.count_photos_in(&own(src)).unwrap(), 3);
        assert_eq!(db.count_photos().unwrap(), 0, "none of it is in the library");
        let conn = db.conn();
        let count = |sql: &str| conn.query_row(sql, [], |r| r.get::<_, i64>(0)).unwrap();
        assert_eq!(count("SELECT COUNT(*) FROM photos WHERE hidden = 2"), 3, "marked by the database as they were added");
        assert_eq!(count("SELECT COUNT(*) FROM photos WHERE hash IS NOT NULL"), 0, "nothing is hashed");
        assert_eq!(count("SELECT COUNT(*) FROM embeddings") + count("SELECT COUNT(*) FROM face_scans") + count("SELECT COUNT(*) FROM photo_tags"), 0, "nothing is analysed");
    }

    #[test]
    fn a_second_scan_adds_only_what_is_new() {
        let dir = card(&[("a.jpg", pixie_testutil::jpeg(8, 6)), ("b.jpg", pixie_testutil::jpeg(8, 6))]);
        let db = Db::open_in_memory().unwrap();
        let src = source_of(&db, &dir);
        scan_source(&db, src, &AtomicBool::new(false), |_| {}).unwrap();
        std::fs::write(dir.path().join("c.jpg"), pixie_testutil::jpeg(8, 6)).unwrap();
        let stats = scan_source(&db, src, &AtomicBool::new(false), |_| {}).unwrap();
        assert_eq!((stats.added, stats.unchanged), (1, 2));
        assert_eq!(db.count_photos_in(&own(src)).unwrap(), 3);
    }

    #[test]
    fn files_with_no_date_take_the_files_modified_date() {
        let dir = card(&[("a.jpg", pixie_testutil::jpeg(8, 6))]);
        let db = Db::open_in_memory().unwrap();
        let src = source_of(&db, &dir);
        scan_source(&db, src, &AtomicBool::new(false), |_| {}).unwrap();
        let p = db.list_photos_in(&own(src), 0, 10).unwrap().remove(0);
        assert_eq!(p.meta.taken_at, Some(p.mtime / 1000));
        assert_eq!((p.meta.width, p.meta.height), (Some(8), Some(6)));
    }

    #[test]
    fn a_stopped_scan_comes_back_cancelled_and_the_source_can_still_be_removed() {
        let dir = card(&[("a.jpg", pixie_testutil::jpeg(8, 6)), ("b.jpg", pixie_testutil::jpeg(8, 6))]);
        let db = Db::open_in_memory().unwrap();
        let src = source_of(&db, &dir);
        let cancel = AtomicBool::new(false);
        let stats = scan_source(&db, src, &cancel, |_| cancel.store(true, std::sync::atomic::Ordering::Relaxed)).unwrap();
        assert!(stats.cancelled);
        db.remove_source(src).unwrap();
        assert!(db.list_sources().unwrap().is_empty());
    }

    #[test]
    fn a_source_that_vanishes_is_an_error_not_a_panic() {
        let dir = card(&[("a.jpg", pixie_testutil::jpeg(8, 6))]);
        let db = Db::open_in_memory().unwrap();
        let src = source_of(&db, &dir);
        drop(dir); // the card is pulled out
        assert!(scan_source(&db, src, &AtomicBool::new(false), |_| {}).is_err());
        db.remove_source(src).unwrap();
    }

    #[test]
    fn an_unreadable_picture_is_counted_and_the_rest_are_listed() {
        let dir = card(&[("good.jpg", pixie_testutil::jpeg(8, 6)), ("empty.jpg", Vec::new())]);
        let db = Db::open_in_memory().unwrap();
        let src = source_of(&db, &dir);
        let stats = scan_source(&db, src, &AtomicBool::new(false), |_| {}).unwrap();
        assert_eq!(stats.unreadable, 1);
        assert!(db.list_photos_in(&own(src), 0, 10).unwrap().iter().any(|p| p.path.ends_with("good.jpg")));
    }

    #[test]
    fn only_a_source_can_be_scanned_this_way() {
        let dir = card(&[("a.jpg", pixie_testutil::jpeg(8, 6))]);
        let db = Db::open_in_memory().unwrap();
        let lib = db.add_folder(dir.path().to_str().unwrap()).unwrap();
        assert!(scan_source(&db, lib, &AtomicBool::new(false), |_| {}).is_err(), "a library folder is read by the analysis pipeline");
        assert_eq!(db.count_photos().unwrap(), 0);
    }
}
