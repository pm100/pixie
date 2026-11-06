use crate::db::{db_err, Db};
use crate::source::relative_to;
use pixie_core::{FolderId, PhotoId, PixieError, Result};
use rusqlite::params;
use std::collections::{HashMap, HashSet};

/// One Keep run from one source into one folder.
pub type BatchId = i64;

/// What an item on a source is, as far as the library knows without reading it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ItemState {
    New,
    /// Kept in an earlier run: the same source, path below it, size and modified time are in the import log (even if the library copy was deleted since).
    KeptBefore,
    /// The name (ignoring case) and size of a photo in the library. Only a hint: nothing was compared.
    LooksSame { library_path: String },
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct SourceSummary {
    pub photos: i64,
    pub videos: i64,
    pub bytes: i64,
    pub new: i64,
    pub kept_before: i64,
    pub looks_same: i64,
    /// The earliest and latest date taken, unix seconds.
    pub first: Option<i64>,
    pub last: Option<i64>,
}

/// The file name of a path, lower case (a path may use either separator).
fn lower_name(path: &str) -> String {
    path.rsplit(['/', '\\']).next().unwrap_or(path).to_lowercase()
}

impl Db {
    /// Starts a batch: one Keep run from one source into one folder.
    pub fn start_batch(&self, source_key: &str, label: &str, dest_folder: FolderId) -> Result<BatchId> {
        let c = self.conn();
        c.execute(
            "INSERT INTO import_batch(started, source_key, label, dest_folder) VALUES (CAST(strftime('%s', 'now') AS INTEGER), ?1, ?2, ?3)",
            params![source_key, label, dest_folder],
        )
        .map_err(db_err)?;
        Ok(c.last_insert_rowid())
    }

    /// Records an item that was kept (copied and verified), so it is known next time even if the library copy is deleted. `path` is the item's path
    /// below the source (forward slashes), `dest_path` where the copy went.
    pub fn log_kept(&self, batch: BatchId, source_key: &str, path: &str, size: i64, mtime: i64, hash: Option<&str>, dest_path: &str) -> Result<()> {
        let mut c = self.conn();
        let tx = c.transaction().map_err(db_err)?;
        tx.execute(
            "INSERT INTO import_log(batch, source_key, path, size, mtime, hash, dest_path) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![batch, source_key, path, size, mtime, hash, dest_path],
        )
        .map_err(db_err)?;
        tx.execute("UPDATE import_batch SET items = items + 1, bytes = bytes + ?2 WHERE id = ?1", params![batch, size]).map_err(db_err)?;
        tx.commit().map_err(db_err)
    }

    /// For each item of a source: new, kept before, or looks the same as a library item. Nothing on the source is read, and the library is asked
    /// once (by the sizes the source has), not once for each item.
    pub fn source_item_states(&self, folder: FolderId) -> Result<Vec<(PhotoId, ItemState)>> {
        let c = self.conn();
        let (root, key): (String, String) = c
            .query_row("SELECT path, COALESCE(source_key, '') FROM folders WHERE id = ?1 AND kind = 'source'", params![folder], |r| Ok((r.get(0)?, r.get(1)?)))
            .map_err(|_| PixieError::Invalid("That is not a source.".into()))?;
        let items: Vec<(PhotoId, String, i64, i64)> = {
            let mut s = c.prepare("SELECT id, path, size, mtime FROM photos WHERE folder_id = ?1 AND missing = 0 ORDER BY id").map_err(db_err)?;
            let rows = s.query_map(params![folder], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))).map_err(db_err)?.collect::<std::result::Result<Vec<_>, _>>().map_err(db_err)?;
            rows
        };
        let logged: HashSet<(String, i64, i64)> = {
            let mut s = c.prepare("SELECT path, size, mtime FROM import_log WHERE source_key = ?1").map_err(db_err)?;
            let rows = s.query_map(params![key], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).map_err(db_err)?.collect::<std::result::Result<HashSet<_>, _>>().map_err(db_err)?;
            rows
        };
        let sizes = serde_json::to_string(&items.iter().map(|i| i.2).collect::<HashSet<_>>().into_iter().collect::<Vec<_>>()).unwrap_or_else(|_| "[]".into());
        let mut library: HashMap<(String, i64), String> = HashMap::new();
        {
            // Photos the library shows (not hidden, not missing, not on a source), of a size the source has.
            let mut s = c
                .prepare("SELECT path, size FROM photos WHERE hidden = 0 AND missing = 0 AND size IN (SELECT value FROM json_each(?1)) ORDER BY id")
                .map_err(db_err)?;
            let rows = s.query_map(params![sizes], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))).map_err(db_err)?;
            for row in rows {
                let (path, size) = row.map_err(db_err)?;
                library.entry((lower_name(&path), size)).or_insert(path);
            }
        }
        Ok(items
            .into_iter()
            .map(|(id, path, size, mtime)| {
                let state = if logged.contains(&(relative_to(&root, &path), size, mtime)) {
                    ItemState::KeptBefore
                } else if let Some(library_path) = library.get(&(lower_name(&path), size)) {
                    ItemState::LooksSame { library_path: library_path.clone() }
                } else {
                    ItemState::New
                };
                (id, state)
            })
            .collect())
    }

    /// What a source holds, and how much of it is new: for the line under the Look step ("412 photos and 18 videos, 6.1 GB, 380 new").
    pub fn source_summary(&self, folder: FolderId) -> Result<SourceSummary> {
        let states = self.source_item_states(folder)?;
        let mut out = SourceSummary::default();
        for (_, state) in &states {
            match state {
                ItemState::New => out.new += 1,
                ItemState::KeptBefore => out.kept_before += 1,
                ItemState::LooksSame { .. } => out.looks_same += 1,
            }
        }
        let c = self.conn();
        let (photos, videos, bytes, first, last) = c
            .query_row(
                "SELECT COALESCE(SUM(kind = 0), 0), COALESCE(SUM(kind = 1), 0), COALESCE(SUM(size), 0), MIN(taken_at), MAX(taken_at) FROM photos WHERE folder_id = ?1 AND missing = 0",
                params![folder],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
            )
            .map_err(db_err)?;
        (out.photos, out.videos, out.bytes, out.first, out.last) = (photos, videos, bytes, first, last);
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::{ItemState, SourceSummary};
    use crate::db::Db;
    use pixie_core::{MediaKind, PhotoMeta};

    fn meta(taken: i64) -> PhotoMeta {
        PhotoMeta { taken_at: Some(taken), ..Default::default() }
    }
    fn video(taken: i64) -> PhotoMeta {
        PhotoMeta { taken_at: Some(taken), kind: MediaKind::Video, ..Default::default() }
    }
    fn states(db: &Db, src: i64) -> Vec<(String, ItemState)> {
        let mut out: Vec<(String, ItemState)> = db
            .source_item_states(src)
            .unwrap()
            .into_iter()
            .map(|(id, st)| (db.get_photo(id).unwrap().unwrap().path, st))
            .collect();
        out.sort_by(|a, b| a.0.cmp(&b.0));
        out
    }

    #[test]
    fn an_item_kept_before_is_known_by_source_path_size_and_time() {
        let db = Db::open_in_memory().unwrap();
        let src = db.add_source("/card", "K", "Card").unwrap();
        db.upsert_photo(src, "/card/DCIM/a.jpg", 100, 5000, &meta(1)).unwrap();
        assert_eq!(states(&db, src)[0].1, ItemState::New);
        let dest = db.add_folder("/lib").unwrap();
        let batch = db.start_batch("K", "Card", dest).unwrap();
        db.log_kept(batch, "K", "DCIM/a.jpg", 100, 5000, None, "/lib/2024/a.jpg").unwrap();
        assert_eq!(states(&db, src)[0].1, ItemState::KeptBefore);
    }

    #[test]
    fn a_card_that_comes_back_under_another_path_is_still_recognised() {
        let db = Db::open_in_memory().unwrap();
        let dest = db.add_folder("/lib").unwrap();
        let batch = db.start_batch("K", "Card", dest).unwrap();
        db.log_kept(batch, "K", "DCIM/a.jpg", 100, 5000, None, "/lib/a.jpg").unwrap();
        let src = db.add_source("/Volumes/OTHER", "K", "Card").unwrap();
        db.upsert_photo(src, "/Volumes/OTHER/DCIM/a.jpg", 100, 5000, &meta(1)).unwrap();
        assert_eq!(states(&db, src)[0].1, ItemState::KeptBefore, "the log keeps the path below the source, not the drive letter");
    }

    #[test]
    fn reused_file_names_on_a_new_card_are_not_known() {
        let db = Db::open_in_memory().unwrap();
        let dest = db.add_folder("/lib").unwrap();
        let batch = db.start_batch("K", "Card", dest).unwrap();
        db.log_kept(batch, "K", "DCIM/a.jpg", 100, 5000, None, "/lib/a.jpg").unwrap();
        // The same card, formatted and used again: the camera restarted its numbering, so the name is the same but the picture is not.
        let src = db.add_source("/card", "K", "Card").unwrap();
        db.upsert_photo(src, "/card/DCIM/a.jpg", 100, 9000, &meta(1)).unwrap();
        assert_eq!(states(&db, src)[0].1, ItemState::New, "a different time");
        db.upsert_photo(src, "/card/DCIM/a.jpg", 101, 5000, &meta(1)).unwrap();
        assert_eq!(states(&db, src)[0].1, ItemState::New, "a different size");
        // Another source never shares a log.
        db.upsert_photo(src, "/card/DCIM/a.jpg", 100, 5000, &meta(1)).unwrap();
        let other = db.add_source("/other", "K2", "Other").unwrap();
        db.upsert_photo(other, "/other/DCIM/a.jpg", 100, 5000, &meta(1)).unwrap();
        assert_eq!(states(&db, other)[0].1, ItemState::New);
        assert_eq!(states(&db, src)[0].1, ItemState::KeptBefore);
    }

    #[test]
    fn an_item_with_the_name_and_size_of_a_library_photo_looks_the_same() {
        let db = Db::open_in_memory().unwrap();
        let lib = db.add_folder("/lib").unwrap();
        db.upsert_photo(lib, "/lib/2024/IMG_0451.JPG", 100, 1, &meta(1)).unwrap();
        let src = db.add_source("/card", "K", "Card").unwrap();
        db.upsert_photo(src, "/card/DCIM/img_0451.jpg", 100, 1, &meta(1)).unwrap();
        db.upsert_photo(src, "/card/DCIM/IMG_0452.JPG", 100, 1, &meta(1)).unwrap();
        db.upsert_photo(src, "/card/DCIM/IMG_0451.JPG.bak.jpg", 100, 1, &meta(1)).unwrap();
        let st = states(&db, src);
        assert_eq!(st[1].0, "/card/DCIM/IMG_0452.JPG");
        assert_eq!(st[0].0, "/card/DCIM/IMG_0451.JPG.bak.jpg");
        assert_eq!(st[2].0, "/card/DCIM/img_0451.jpg");
        assert_eq!(st[2].1, ItemState::LooksSame { library_path: "/lib/2024/IMG_0451.JPG".into() }, "case and folders do not matter");
        assert_eq!(st[0].1, ItemState::New, "a different name");
        assert_eq!(st[1].1, ItemState::New, "a different name");
        db.upsert_photo(src, "/card/DCIM/img_0451.jpg", 101, 1, &meta(1)).unwrap();
        assert_eq!(states(&db, src)[2].1, ItemState::New, "a different size");
    }

    #[test]
    fn a_hidden_or_missing_library_photo_or_another_source_does_not_make_an_item_look_the_same() {
        let db = Db::open_in_memory().unwrap();
        let lib = db.add_folder("/lib").unwrap();
        let hidden = db.upsert_photo(lib, "/lib/a.jpg", 100, 1, &meta(1)).unwrap();
        db.hide(&[hidden]).unwrap();
        let gone = db.upsert_photo(lib, "/lib/b.jpg", 100, 1, &meta(1)).unwrap();
        db.conn().execute("UPDATE photos SET missing = 1 WHERE id = ?1", [gone]).unwrap();
        let other = db.add_source("/other", "K2", "Other").unwrap();
        db.upsert_photo(other, "/other/c.jpg", 100, 1, &meta(1)).unwrap();
        let src = db.add_source("/card", "K", "Card").unwrap();
        for name in ["a", "b", "c"] {
            db.upsert_photo(src, &format!("/card/{name}.jpg"), 100, 1, &meta(1)).unwrap();
        }
        assert!(states(&db, src).iter().all(|(_, st)| *st == ItemState::New));
    }

    #[test]
    fn the_summary_counts_photos_videos_bytes_what_is_new_and_the_date_range() {
        let db = Db::open_in_memory().unwrap();
        let lib = db.add_folder("/lib").unwrap();
        db.upsert_photo(lib, "/lib/same.jpg", 30, 1, &meta(1)).unwrap();
        let src = db.add_source("/card", "K", "Card").unwrap();
        db.upsert_photo(src, "/card/new.jpg", 10, 1, &meta(300)).unwrap();
        db.upsert_photo(src, "/card/kept.jpg", 20, 2, &meta(100)).unwrap();
        db.upsert_photo(src, "/card/same.jpg", 30, 3, &meta(200)).unwrap();
        db.upsert_photo(src, "/card/clip.mp4", 40, 4, &video(400)).unwrap();
        let batch = db.start_batch("K", "Card", lib).unwrap();
        db.log_kept(batch, "K", "kept.jpg", 20, 2, None, "/lib/kept.jpg").unwrap();
        assert_eq!(
            db.source_summary(src).unwrap(),
            SourceSummary { photos: 3, videos: 1, bytes: 100, new: 2, kept_before: 1, looks_same: 1, first: Some(100), last: Some(400) }
        );
        let empty = db.add_source("/empty", "K3", "Empty").unwrap();
        assert_eq!(db.source_summary(empty).unwrap(), SourceSummary::default());
    }

    #[test]
    fn a_batch_adds_up_what_was_logged() {
        let db = Db::open_in_memory().unwrap();
        let dest = db.add_folder("/lib").unwrap();
        let batch = db.start_batch("K", "Card", dest).unwrap();
        db.log_kept(batch, "K", "a.jpg", 10, 1, Some("h1"), "/lib/a.jpg").unwrap();
        db.log_kept(batch, "K", "b.jpg", 25, 2, None, "/lib/b.jpg").unwrap();
        let (items, bytes): (i64, i64) = db.conn().query_row("SELECT items, bytes FROM import_batch WHERE id = ?1", [batch], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
        assert_eq!((items, bytes), (2, 35));
    }
}
