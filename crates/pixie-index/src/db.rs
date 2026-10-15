use pixie_core::{Folder, FolderId, Photo, PhotoId, PhotoMeta, PixieError, Result};
use rusqlite::{params, Connection, OptionalExtension};
use std::collections::HashSet;
use std::path::Path;
use std::sync::{Arc, Mutex};

fn db_err<E: std::fmt::Display>(e: E) -> PixieError {
    PixieError::Db(e.to_string())
}

const MIGRATIONS: &[&str] = &[
    // V1
    "CREATE TABLE folders (
        id   INTEGER PRIMARY KEY,
        path TEXT NOT NULL UNIQUE
     );
     CREATE TABLE photos (
        id        INTEGER PRIMARY KEY,
        folder_id INTEGER NOT NULL REFERENCES folders(id) ON DELETE CASCADE,
        path      TEXT NOT NULL UNIQUE,
        size      INTEGER NOT NULL,
        mtime     INTEGER NOT NULL,
        width     INTEGER,
        height    INTEGER,
        taken_at  INTEGER,
        camera    TEXT,
        gps_lat   REAL,
        gps_lon   REAL,
        missing   INTEGER NOT NULL DEFAULT 0
     );
     CREATE INDEX idx_photos_list   ON photos(missing, taken_at DESC, id DESC);
     CREATE INDEX idx_photos_folder ON photos(folder_id);",
];

#[derive(Debug, Clone, PartialEq)]
pub struct PhotoStat {
    pub id: PhotoId,
    pub size: i64,
    pub mtime: i64,
    pub missing: bool,
}

#[derive(Clone)]
pub struct Db {
    conn: Arc<Mutex<Connection>>,
}

const PHOTO_COLS: &str =
    "id, folder_id, path, size, mtime, width, height, taken_at, camera, gps_lat, gps_lon, missing";

fn row_to_photo(r: &rusqlite::Row) -> rusqlite::Result<Photo> {
    Ok(Photo {
        id: r.get(0)?,
        folder_id: r.get(1)?,
        path: r.get(2)?,
        size: r.get(3)?,
        mtime: r.get(4)?,
        meta: PhotoMeta {
            width: r.get(5)?,
            height: r.get(6)?,
            taken_at: r.get(7)?,
            camera: r.get(8)?,
            gps_lat: r.get(9)?,
            gps_lon: r.get(10)?,
        },
        missing: r.get::<_, i64>(11)? != 0,
    })
}

impl Db {
    pub fn open(path: &Path) -> Result<Db> {
        Self::init(Connection::open(path).map_err(db_err)?)
    }

    pub fn open_in_memory() -> Result<Db> {
        Self::init(Connection::open_in_memory().map_err(db_err)?)
    }

    fn init(mut conn: Connection) -> Result<Db> {
        conn.pragma_update(None, "journal_mode", "WAL").map_err(db_err)?;
        conn.pragma_update(None, "foreign_keys", "ON").map_err(db_err)?;
        let version: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0)).map_err(db_err)?;
        let known = MIGRATIONS.len() as i64;
        if version > known {
            return Err(PixieError::Db(format!(
                "library was created by a newer Pixie (schema {version}, this build knows {known})"
            )));
        }
        for (i, sql) in MIGRATIONS.iter().enumerate().skip(version as usize) {
            let tx = conn.transaction().map_err(db_err)?;
            tx.execute_batch(sql).map_err(db_err)?;
            tx.pragma_update(None, "user_version", (i + 1) as i64).map_err(db_err)?;
            tx.commit().map_err(db_err)?;
        }
        Ok(Db { conn: Arc::new(Mutex::new(conn)) })
    }

    fn conn(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.conn.lock().unwrap_or_else(|p| p.into_inner())
    }

    pub fn add_folder(&self, path: &str) -> Result<FolderId> {
        let c = self.conn();
        c.execute("INSERT OR IGNORE INTO folders(path) VALUES (?1)", params![path]).map_err(db_err)?;
        c.query_row("SELECT id FROM folders WHERE path = ?1", params![path], |r| r.get(0)).map_err(db_err)
    }

    pub fn get_folder(&self, id: FolderId) -> Result<Option<Folder>> {
        self.conn()
            .query_row("SELECT id, path FROM folders WHERE id = ?1", params![id], |r| {
                Ok(Folder { id: r.get(0)?, path: r.get(1)? })
            })
            .optional()
            .map_err(db_err)
    }

    pub fn list_folders(&self) -> Result<Vec<Folder>> {
        let c = self.conn();
        let mut s = c.prepare("SELECT id, path FROM folders ORDER BY path").map_err(db_err)?;
        let rows = s
            .query_map([], |r| Ok(Folder { id: r.get(0)?, path: r.get(1)? }))
            .map_err(db_err)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(db_err)?;
        Ok(rows)
    }

    pub fn find_photo_stat(&self, path: &str) -> Result<Option<PhotoStat>> {
        self.conn()
            .query_row("SELECT id, size, mtime, missing FROM photos WHERE path = ?1", params![path], |r| {
                Ok(PhotoStat { id: r.get(0)?, size: r.get(1)?, mtime: r.get(2)?, missing: r.get::<_, i64>(3)? != 0 })
            })
            .optional()
            .map_err(db_err)
    }

    pub fn upsert_photo(&self, folder_id: FolderId, path: &str, size: i64, mtime: i64, meta: &PhotoMeta) -> Result<PhotoId> {
        self.conn()
            .query_row(
                "INSERT INTO photos (folder_id, path, size, mtime, width, height, taken_at, camera, gps_lat, gps_lon, missing)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 0)
                 ON CONFLICT(path) DO UPDATE SET
                    folder_id = excluded.folder_id, size = excluded.size, mtime = excluded.mtime,
                    width = excluded.width, height = excluded.height, taken_at = excluded.taken_at,
                    camera = excluded.camera, gps_lat = excluded.gps_lat, gps_lon = excluded.gps_lon,
                    missing = 0
                 RETURNING id",
                params![folder_id, path, size, mtime, meta.width, meta.height, meta.taken_at, meta.camera, meta.gps_lat, meta.gps_lon],
                |r| r.get(0),
            )
            .map_err(db_err)
    }

    pub fn get_photo(&self, id: PhotoId) -> Result<Option<Photo>> {
        self.conn()
            .query_row(&format!("SELECT {PHOTO_COLS} FROM photos WHERE id = ?1"), params![id], row_to_photo)
            .optional()
            .map_err(db_err)
    }

    pub fn mark_missing_except(&self, folder_id: FolderId, seen: &HashSet<String>) -> Result<usize> {
        let mut c = self.conn();
        let tx = c.transaction().map_err(db_err)?;
        let stale: Vec<PhotoId> = {
            let mut s = tx
                .prepare("SELECT id, path FROM photos WHERE folder_id = ?1 AND missing = 0")
                .map_err(db_err)?;
            let rows = s
                .query_map(params![folder_id], |r| Ok((r.get::<_, PhotoId>(0)?, r.get::<_, String>(1)?)))
                .map_err(db_err)?
                .collect::<std::result::Result<Vec<_>, _>>()
                .map_err(db_err)?;
            rows.into_iter().filter(|(_, p)| !seen.contains(p)).map(|(id, _)| id).collect()
        };
        for id in &stale {
            tx.execute("UPDATE photos SET missing = 1 WHERE id = ?1", params![id]).map_err(db_err)?;
        }
        tx.commit().map_err(db_err)?;
        Ok(stale.len())
    }

    pub fn list_photos(&self, offset: i64, limit: i64) -> Result<Vec<Photo>> {
        let c = self.conn();
        let mut s = c
            .prepare(&format!(
                "SELECT {PHOTO_COLS} FROM photos WHERE missing = 0
                 ORDER BY taken_at DESC, id DESC LIMIT ?1 OFFSET ?2"
            ))
            .map_err(db_err)?;
        let rows = s
            .query_map(params![limit, offset], row_to_photo)
            .map_err(db_err)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(db_err)?;
        Ok(rows)
    }

    pub fn count_photos(&self) -> Result<i64> {
        self.conn()
            .query_row("SELECT COUNT(*) FROM photos WHERE missing = 0", [], |r| r.get(0))
            .map_err(db_err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn meta(taken: Option<i64>) -> PhotoMeta {
        PhotoMeta { width: Some(10), height: Some(5), taken_at: taken, ..Default::default() }
    }

    #[test]
    fn migrations_run_once_and_reopen_cleanly() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("t.db");
        let db = Db::open(&p).unwrap();
        db.add_folder("/a").unwrap();
        drop(db);
        let db = Db::open(&p).unwrap(); // reopening must not re-run V1
        assert_eq!(db.list_folders().unwrap().len(), 1);
    }

    #[test]
    fn opening_a_newer_database_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("t.db");
        Connection::open(&p).unwrap().pragma_update(None, "user_version", 999).unwrap();
        assert!(matches!(Db::open(&p), Err(PixieError::Db(_))));
    }

    #[test]
    fn add_folder_is_idempotent() {
        let db = Db::open_in_memory().unwrap();
        let a = db.add_folder("/pics").unwrap();
        assert_eq!(db.add_folder("/pics").unwrap(), a);
        assert_eq!(db.get_folder(a).unwrap().unwrap().path, "/pics");
        assert!(db.get_folder(999).unwrap().is_none());
    }

    #[test]
    fn upsert_inserts_then_updates_same_row_and_clears_missing() {
        let db = Db::open_in_memory().unwrap();
        let f = db.add_folder("/p").unwrap();
        let id = db.upsert_photo(f, "/p/a.jpg", 100, 5, &meta(Some(1000))).unwrap();
        let id2 = db.upsert_photo(f, "/p/a.jpg", 200, 6, &meta(Some(2000))).unwrap();
        assert_eq!(id, id2);
        let p = db.get_photo(id).unwrap().unwrap();
        assert_eq!((p.size, p.mtime, p.meta.taken_at, p.missing), (200, 6, Some(2000), false));
        let st = db.find_photo_stat("/p/a.jpg").unwrap().unwrap();
        assert_eq!((st.id, st.size, st.mtime, st.missing), (id, 200, 6, false));
        assert!(db.find_photo_stat("/p/zzz.jpg").unwrap().is_none());
    }

    #[test]
    fn same_filename_in_different_folders_are_distinct() {
        let db = Db::open_in_memory().unwrap();
        let f = db.add_folder("/p").unwrap();
        let a = db.upsert_photo(f, "/p/x/IMG_1.jpg", 1, 1, &meta(None)).unwrap();
        let b = db.upsert_photo(f, "/p/y/IMG_1.jpg", 1, 1, &meta(None)).unwrap();
        assert_ne!(a, b);
    }

    #[test]
    fn mark_missing_hides_from_list_and_upsert_restores() {
        let db = Db::open_in_memory().unwrap();
        let f = db.add_folder("/p").unwrap();
        let g = db.add_folder("/q").unwrap();
        db.upsert_photo(f, "/p/a.jpg", 1, 1, &meta(Some(1))).unwrap();
        db.upsert_photo(f, "/p/b.jpg", 1, 1, &meta(Some(2))).unwrap();
        db.upsert_photo(g, "/q/c.jpg", 1, 1, &meta(Some(3))).unwrap();
        let seen: HashSet<String> = ["/p/a.jpg".to_string()].into();
        assert_eq!(db.mark_missing_except(f, &seen).unwrap(), 1);
        assert_eq!(db.count_photos().unwrap(), 2); // b hidden; other folder untouched
        db.upsert_photo(f, "/p/b.jpg", 1, 1, &meta(Some(2))).unwrap();
        assert_eq!(db.count_photos().unwrap(), 3);
    }

    #[test]
    fn list_orders_newest_first_with_null_dates_last_and_pages() {
        let db = Db::open_in_memory().unwrap();
        let f = db.add_folder("/p").unwrap();
        for (n, t) in [("a", Some(10)), ("b", Some(30)), ("c", Some(20)), ("d", None)] {
            db.upsert_photo(f, &format!("/p/{n}.jpg"), 1, 1, &meta(t)).unwrap();
        }
        let all: Vec<String> = db.list_photos(0, 10).unwrap().into_iter().map(|p| p.path).collect();
        assert_eq!(all, ["/p/b.jpg", "/p/c.jpg", "/p/a.jpg", "/p/d.jpg"]);
        let page: Vec<String> = db.list_photos(1, 2).unwrap().into_iter().map(|p| p.path).collect();
        assert_eq!(page, ["/p/c.jpg", "/p/a.jpg"]);
        assert!(db.list_photos(100, 10).unwrap().is_empty());
    }

    #[test]
    fn unicode_and_spaces_round_trip() {
        let db = Db::open_in_memory().unwrap();
        let f = db.add_folder("/Ferien 2024").unwrap();
        let id = db.upsert_photo(f, "/Ferien 2024/Café ☕ 1.jpg", 1, 1, &meta(None)).unwrap();
        assert_eq!(db.get_photo(id).unwrap().unwrap().path, "/Ferien 2024/Café ☕ 1.jpg");
    }
}
