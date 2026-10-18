use pixie_core::{validate_email, validate_header_text, validate_host, validate_nickname, Bounds, Contact, DayGroup, Folder, FolderId, FolderInfo, GeoPoint, MailAccount, MailSecurity, MediaKind, NewContact, Photo, PhotoId, PhotoMeta, PhotoQuery, PixieError, Result, SortOrder};
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
    // V2: Undated-last ordering in both directions served by an index; GPS sanity.
    "DROP INDEX idx_photos_list;
     CREATE INDEX idx_photos_newest ON photos(missing, (taken_at IS NULL), taken_at DESC, id DESC);
     CREATE INDEX idx_photos_oldest ON photos(missing, (taken_at IS NULL), taken_at ASC, id ASC);
     CREATE INDEX idx_photos_folder_newest ON photos(folder_id, missing, (taken_at IS NULL), taken_at DESC, id DESC);
     CREATE INDEX idx_photos_folder_oldest ON photos(folder_id, missing, (taken_at IS NULL), taken_at ASC, id ASC);
     UPDATE photos SET gps_lat = NULL, gps_lon = NULL
      WHERE gps_lat IS NULL OR gps_lon IS NULL
         OR abs(gps_lat) > 90 OR abs(gps_lon) > 180
         OR (gps_lat = 0 AND gps_lon = 0);",
    // V3: videos. Existing rows are photos.
    "ALTER TABLE photos ADD COLUMN kind INTEGER NOT NULL DEFAULT 0;
     ALTER TABLE photos ADD COLUMN duration_ms INTEGER;
     CREATE INDEX idx_photos_kind_newest ON photos(kind, missing, (taken_at IS NULL), taken_at DESC, id DESC);
     CREATE INDEX idx_photos_kind_oldest ON photos(kind, missing, (taken_at IS NULL), taken_at ASC, id ASC);",
    // V4: email contacts and the one mail account (the password is not here; it is in the OS credential store).
    "CREATE TABLE contacts (
        id       INTEGER PRIMARY KEY,
        nickname TEXT NOT NULL COLLATE NOCASE UNIQUE,
        name     TEXT,
        email    TEXT NOT NULL
     );
     CREATE TABLE mail_account (
        id           INTEGER PRIMARY KEY CHECK (id = 1),
        display_name TEXT,
        email        TEXT NOT NULL,
        provider     TEXT NOT NULL,
        host         TEXT NOT NULL,
        port         INTEGER NOT NULL,
        security     TEXT NOT NULL,
        limit_mb     INTEGER NOT NULL
     );",
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
    "id, folder_id, path, size, mtime, width, height, taken_at, camera, gps_lat, gps_lon, missing, kind, duration_ms";

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
            kind: if r.get::<_, i64>(12)? == 1 { MediaKind::Video } else { MediaKind::Photo },
            duration_ms: r.get(13)?,
        },
        missing: r.get::<_, i64>(11)? != 0,
    })
}

fn order_sql(o: SortOrder) -> &'static str {
    match o {
        SortOrder::Newest => "taken_at IS NULL, taken_at DESC, id DESC",
        SortOrder::Oldest => "taken_at IS NULL, taken_at ASC, id ASC",
    }
}

/// The optional folder, kind and bounding-box filters of a query, as SQL text and bound values.
struct Filter {
    folder: Option<i64>,
    kind: Option<i64>,
    /// `Some(None)` is an invalid box, which matches nothing.
    bounds: Option<Option<Bounds>>,
    max_id: Option<i64>,
    since: Option<i64>,
    until: Option<i64>,
}

impl Filter {
    fn of(q: &PhotoQuery) -> Filter {
        Filter {
            folder: q.folder_id,
            kind: q.kind.map(|k| if k == MediaKind::Video { 1 } else { 0 }),
            bounds: q.bounds.map(|b| b.normalized()),
            max_id: q.max_id,
            since: q.since,
            until: q.until,
        }
    }
    fn sql(&self) -> String {
        let mut s = String::new();
        if self.folder.is_some() {
            s.push_str(" AND folder_id = :folder");
        }
        if self.kind.is_some() {
            s.push_str(" AND kind = :kind");
        }
        if self.max_id.is_some() {
            s.push_str(" AND id <= :max_id");
        }
        if self.since.is_some() {
            s.push_str(" AND taken_at >= :since");
        }
        if self.until.is_some() {
            s.push_str(" AND taken_at < :until");
        }
        match &self.bounds {
            None => {}
            Some(None) => s.push_str(" AND 0"),
            Some(Some(b)) if b.wraps() => {
                s.push_str(" AND gps_lat BETWEEN :south AND :north AND (gps_lon >= :west OR gps_lon <= :east)")
            }
            Some(Some(_)) => s.push_str(" AND gps_lat BETWEEN :south AND :north AND gps_lon BETWEEN :west AND :east"),
        }
        s
    }
    fn params(&self) -> Vec<(&'static str, &dyn rusqlite::ToSql)> {
        let mut v: Vec<(&'static str, &dyn rusqlite::ToSql)> = Vec::new();
        if let Some(f) = &self.folder {
            v.push((":folder", f));
        }
        if let Some(k) = &self.kind {
            v.push((":kind", k));
        }
        if let Some(m) = &self.max_id {
            v.push((":max_id", m));
        }
        if let Some(t) = &self.since {
            v.push((":since", t));
        }
        if let Some(t) = &self.until {
            v.push((":until", t));
        }
        if let Some(Some(b)) = &self.bounds {
            v.push((":south", &b.south));
            v.push((":north", &b.north));
            v.push((":west", &b.west));
            v.push((":east", &b.east));
        }
        v
    }
}

fn list_sql(q: &PhotoQuery, f: &Filter) -> String {
    format!(
        "SELECT {PHOTO_COLS} FROM photos WHERE missing = 0{} ORDER BY {} LIMIT :limit OFFSET :offset",
        f.sql(),
        order_sql(q.order)
    )
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

    pub fn folder_infos(&self) -> Result<Vec<FolderInfo>> {
        let c = self.conn();
        let mut s = c
            .prepare(
                "SELECT f.id, f.path, COUNT(p.id) FROM folders f
                 LEFT JOIN photos p ON p.folder_id = f.id AND p.missing = 0
                 GROUP BY f.id ORDER BY f.path",
            )
            .map_err(db_err)?;
        let rows = s
            .query_map([], |r| Ok(FolderInfo { id: r.get(0)?, path: r.get(1)?, count: r.get(2)? }))
            .map_err(db_err)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(db_err)?;
        Ok(rows)
    }

    /// Forget a folder and its items (the `ON DELETE CASCADE` on photos). Files are never touched.
    pub fn remove_folder(&self, id: FolderId) -> Result<()> {
        self.conn().execute("DELETE FROM folders WHERE id = ?1", params![id]).map_err(db_err)?;
        Ok(())
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
                "INSERT INTO photos (folder_id, path, size, mtime, width, height, taken_at, camera, gps_lat, gps_lon, missing, kind, duration_ms)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 0, ?11, ?12)
                 ON CONFLICT(path) DO UPDATE SET
                    folder_id = excluded.folder_id, size = excluded.size, mtime = excluded.mtime,
                    width = excluded.width, height = excluded.height, taken_at = excluded.taken_at,
                    camera = excluded.camera, gps_lat = excluded.gps_lat, gps_lon = excluded.gps_lon,
                    kind = excluded.kind, duration_ms = excluded.duration_ms,
                    missing = 0
                 RETURNING id",
                params![folder_id, path, size, mtime, meta.width, meta.height, meta.taken_at, meta.camera, meta.gps_lat, meta.gps_lon, if meta.kind == MediaKind::Video { 1i64 } else { 0 }, meta.duration_ms],
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
        self.list_photos_in(&PhotoQuery::default(), offset, limit)
    }

    pub fn list_photos_in(&self, q: &PhotoQuery, offset: i64, limit: i64) -> Result<Vec<Photo>> {
        let c = self.conn();
        let filter = Filter::of(q);
        let mut s = c.prepare(&list_sql(q, &filter)).map_err(db_err)?;
        let mut params = filter.params();
        params.push((":limit", &limit as &dyn rusqlite::ToSql));
        params.push((":offset", &offset as &dyn rusqlite::ToSql));
        let rows = s
            .query_map(params.as_slice(), row_to_photo)
            .map_err(db_err)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(db_err)?;
        Ok(rows)
    }

    pub fn count_photos(&self) -> Result<i64> {
        self.count_photos_in(&PhotoQuery::default())
    }

    pub fn count_photos_in(&self, q: &PhotoQuery) -> Result<i64> {
        let filter = Filter::of(q);
        let sql = format!("SELECT COUNT(*) FROM photos WHERE missing = 0{}", filter.sql());
        self.conn()
            .query_row(&sql, filter.params().as_slice(), |r| r.get(0))
            .map_err(db_err)
    }

    pub fn list_contacts(&self) -> Result<Vec<Contact>> {
        let c = self.conn();
        let mut s = c
            .prepare("SELECT id, nickname, name, email FROM contacts ORDER BY nickname COLLATE NOCASE")
            .map_err(db_err)?;
        let rows = s
            .query_map([], |r| Ok(Contact { id: r.get(0)?, nickname: r.get(1)?, name: r.get(2)?, email: r.get(3)? }))
            .map_err(db_err)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(db_err)?;
        Ok(rows)
    }

    pub fn get_contact(&self, id: i64) -> Result<Option<Contact>> {
        self.conn()
            .query_row("SELECT id, nickname, name, email FROM contacts WHERE id = ?1", params![id], |r| {
                Ok(Contact { id: r.get(0)?, nickname: r.get(1)?, name: r.get(2)?, email: r.get(3)? })
            })
            .optional()
            .map_err(db_err)
    }

    pub fn save_contact(&self, c: &NewContact) -> Result<Contact> {
        let nickname = validate_nickname(&c.nickname)?;
        let email = validate_email(&c.email)?;
        let name = match c.name.as_deref().map(str::trim).filter(|n| !n.is_empty()) {
            Some(n) => Some(validate_header_text(n, 100, "name")?),
            None => None,
        };
        let id = {
            let conn = self.conn();
            let res = match c.id {
                Some(id) => conn
                    .execute(
                        "UPDATE contacts SET nickname = ?1, name = ?2, email = ?3 WHERE id = ?4",
                        params![nickname, name, email, id],
                    )
                    .map(|_| id),
                None => conn
                    .execute("INSERT INTO contacts (nickname, name, email) VALUES (?1, ?2, ?3)", params![nickname, name, email])
                    .map(|_| conn.last_insert_rowid()),
            };
            res.map_err(|e| match e {
                rusqlite::Error::SqliteFailure(f, _) if f.code == rusqlite::ErrorCode::ConstraintViolation => {
                    PixieError::Invalid(format!("There is already a contact called \"{nickname}\"."))
                }
                other => db_err(other),
            })?
        };
        self.get_contact(id)?.ok_or_else(|| PixieError::NotFound("contact".into()))
    }

    pub fn delete_contact(&self, id: i64) -> Result<()> {
        self.conn().execute("DELETE FROM contacts WHERE id = ?1", params![id]).map_err(db_err)?;
        Ok(())
    }

    pub fn get_mail_account(&self) -> Result<Option<MailAccount>> {
        self.conn()
            .query_row(
                "SELECT display_name, email, provider, host, port, security, limit_mb FROM mail_account WHERE id = 1",
                [],
                |r| {
                    let security: String = r.get(5)?;
                    Ok(MailAccount {
                        display_name: r.get(0)?,
                        email: r.get(1)?,
                        provider: r.get(2)?,
                        host: r.get(3)?,
                        port: r.get::<_, i64>(4)? as u16,
                        security: if security == "ssl" { MailSecurity::Ssl } else { MailSecurity::Starttls },
                        limit_mb: r.get::<_, i64>(6)? as u32,
                    })
                },
            )
            .optional()
            .map_err(db_err)
    }

    pub fn save_mail_account(&self, a: &MailAccount) -> Result<()> {
        let email = validate_email(&a.email)?;
        let host = validate_host(&a.host)?;
        let display = match a.display_name.as_deref().map(str::trim).filter(|n| !n.is_empty()) {
            Some(n) => Some(validate_header_text(n, 100, "name")?),
            None => None,
        };
        if a.port == 0 {
            return Err(PixieError::Invalid("The port must be between 1 and 65535.".into()));
        }
        if !(1..=100).contains(&a.limit_mb) {
            return Err(PixieError::Invalid("The size limit must be between 1 and 100 MB.".into()));
        }
        let provider = validate_header_text(&a.provider, 20, "provider")?;
        let security = if a.security == MailSecurity::Ssl { "ssl" } else { "starttls" };
        self.conn()
            .execute(
                "INSERT OR REPLACE INTO mail_account (id, display_name, email, provider, host, port, security, limit_mb)
                 VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![display, email, provider, host, a.port as i64, security, a.limit_mb as i64],
            )
            .map_err(db_err)?;
        Ok(())
    }

    pub fn delete_mail_account(&self) -> Result<()> {
        self.conn().execute("DELETE FROM mail_account WHERE id = 1", []).map_err(db_err)?;
        Ok(())
    }

    /// The items at these positions of the current listing, in the order asked. Positions past the end
    /// or negative are dropped, so the caller must compare the count with what it asked for (selection
    /// is by position and the library can change underneath it). At most 200 positions.
    pub fn photos_at(&self, q: &PhotoQuery, indexes: &[i64]) -> Result<Vec<Photo>> {
        if indexes.len() > 200 {
            return Err(PixieError::Invalid("Too many items selected.".into()));
        }
        let mut out = Vec::with_capacity(indexes.len());
        for &i in indexes {
            if i < 0 {
                continue;
            }
            if let Some(p) = self.list_photos_in(q, i, 1)?.into_iter().next() {
                out.push(p);
            }
        }
        Ok(out)
    }

    /// The highest photo id, or 0 for an empty library. Pass it as `PhotoQuery::max_id` to freeze a listing:
    /// items added later (by a running scan) have higher ids and do not shift any position.
    pub fn max_photo_id(&self) -> Result<i64> {
        self.conn().query_row("SELECT COALESCE(MAX(id), 0) FROM photos", [], |r| r.get(0)).map_err(db_err)
    }

    /// Every non-missing item with a location, for the map. Honours the folder, kind, since and until filters.
    pub fn list_geo_points(&self, folder_id: Option<FolderId>, kind: Option<MediaKind>, since: Option<i64>, until: Option<i64>) -> Result<Vec<GeoPoint>> {
        let q = PhotoQuery { folder_id, kind, since, until, ..Default::default() };
        let filter = Filter::of(&q);
        let sql = format!(
            "SELECT id, gps_lat, gps_lon FROM photos WHERE missing = 0 AND gps_lat IS NOT NULL AND gps_lon IS NOT NULL{} ORDER BY id",
            filter.sql()
        );
        let c = self.conn();
        let mut s = c.prepare(&sql).map_err(db_err)?;
        let rows = s
            .query_map(filter.params().as_slice(), |r| Ok(GeoPoint { id: r.get(0)?, lat: r.get(1)?, lon: r.get(2)? }))
            .map_err(db_err)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(db_err)?;
        Ok(rows)
    }

    /// One entry per calendar day (UTC, as stored), in listing order, Undated last.
    pub fn day_groups(&self, q: &PhotoQuery) -> Result<Vec<DayGroup>> {
        let dir = match q.order {
            SortOrder::Newest => "DESC",
            SortOrder::Oldest => "ASC",
        };
        let filter = Filter::of(q);
        let sql = format!(
            "SELECT CASE WHEN taken_at IS NULL THEN NULL ELSE date(taken_at, 'unixepoch') END AS day, COUNT(*)
             FROM photos WHERE missing = 0{} GROUP BY day ORDER BY day IS NULL, day {dir}",
            filter.sql()
        );
        let c = self.conn();
        let mut s = c.prepare(&sql).map_err(db_err)?;
        let rows = s
            .query_map(filter.params().as_slice(), |r| Ok((r.get::<_, Option<String>>(0)?, r.get::<_, i64>(1)?)))
            .map_err(db_err)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(db_err)?;
        let mut first_index = 0;
        Ok(rows
            .into_iter()
            .map(|(day, count)| {
                let g = DayGroup { day, count, first_index };
                first_index += count;
                g
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn meta(taken: Option<i64>) -> PhotoMeta {
        PhotoMeta { width: Some(10), height: Some(5), taken_at: taken, ..Default::default() }
    }

    fn video_meta(taken: i64, ms: i64) -> PhotoMeta {
        PhotoMeta { width: Some(1920), height: Some(1080), taken_at: Some(taken), kind: MediaKind::Video, duration_ms: Some(ms), ..Default::default() }
    }

    fn mixed_library() -> (Db, FolderId) {
        let db = Db::open_in_memory().unwrap();
        let f = db.add_folder("/p").unwrap();
        db.upsert_photo(f, "/p/a.jpg", 1, 1, &meta(Some(100))).unwrap();
        db.upsert_photo(f, "/p/b.mp4", 1, 1, &video_meta(200, 4000)).unwrap();
        db.upsert_photo(f, "/p/c.jpg", 1, 1, &meta(Some(300))).unwrap();
        db.upsert_photo(f, "/p/d.mov", 1, 1, &video_meta(400, 61_000)).unwrap();
        (db, f)
    }

    #[test]
    fn kind_and_duration_round_trip() {
        let (db, _f) = mixed_library();
        let all = db.list_photos_in(&PhotoQuery::default(), 0, 10).unwrap();
        let v = all.iter().find(|p| p.path == "/p/d.mov").unwrap();
        assert_eq!(v.meta.kind, MediaKind::Video);
        assert_eq!(v.meta.duration_ms, Some(61_000));
        let p = all.iter().find(|p| p.path == "/p/a.jpg").unwrap();
        assert_eq!((p.meta.kind, p.meta.duration_ms), (MediaKind::Photo, None));
    }

    #[test]
    fn kind_filter_applies_to_listing_counts_and_day_groups() {
        let (db, f) = mixed_library();
        let q = |kind, folder_id| PhotoQuery { kind, folder_id, ..Default::default() };
        let videos = db.list_photos_in(&q(Some(MediaKind::Video), None), 0, 10).unwrap();
        assert_eq!(videos.iter().map(|p| p.path.as_str()).collect::<Vec<_>>(), ["/p/d.mov", "/p/b.mp4"]);
        assert_eq!(db.count_photos_in(&q(Some(MediaKind::Video), None)).unwrap(), 2);
        assert_eq!(db.count_photos_in(&q(Some(MediaKind::Photo), None)).unwrap(), 2);
        assert_eq!(db.count_photos_in(&q(None, None)).unwrap(), 4);
        assert_eq!(db.count_photos_in(&q(Some(MediaKind::Video), Some(f))).unwrap(), 2);
        let groups = db.day_groups(&q(Some(MediaKind::Video), None)).unwrap();
        assert_eq!(groups.iter().map(|g| g.count).sum::<i64>(), 2);
        let second = db.list_photos_in(&q(Some(MediaKind::Video), None), 1, 1).unwrap();
        assert_eq!(second[0].path, "/p/b.mp4");
    }

    #[test]
    fn kind_filtered_listing_is_served_from_an_index_without_sorting() {
        let (db, f) = mixed_library();
        for (order, folder_id) in [
            (SortOrder::Newest, None), (SortOrder::Oldest, None),
            (SortOrder::Newest, Some(f)), (SortOrder::Oldest, Some(f)),
        ] {
            let q = PhotoQuery { order, folder_id, kind: Some(MediaKind::Video), bounds: None, max_id: None, since: None, until: None };
            let c = db.conn();
            let filter = Filter::of(&q);
            let sql = list_sql(&q, &filter);
            let mut s = c.prepare(&format!("EXPLAIN QUERY PLAN {sql}")).unwrap();
            let (limit, offset) = (10i64, 0i64);
            let mut params = filter.params();
            params.push((":limit", &limit as &dyn rusqlite::ToSql));
            params.push((":offset", &offset as &dyn rusqlite::ToSql));
            let plan: Vec<String> = s.query_map(params.as_slice(), |r| r.get::<_, String>(3)).unwrap().map(|r| r.unwrap()).collect();
            let plan = plan.join(" | ");
            assert!(plan.contains("idx_photos_"), "{order:?} {folder_id:?}: {plan}");
            assert!(!plan.contains("TEMP B-TREE"), "{order:?} {folder_id:?} must not sort: {plan}");
            if folder_id.is_none() {
                assert!(plan.contains("idx_photos_kind_"), "{order:?} should use the kind index: {plan}");
            }
        }
    }

    #[test]
    fn v3_migration_keeps_existing_rows_as_photos() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("v2.db");
        {
            let c = Connection::open(&p).unwrap();
            c.execute_batch(MIGRATIONS[0]).unwrap();
            c.execute_batch(MIGRATIONS[1]).unwrap();
            c.pragma_update(None, "user_version", 2).unwrap();
            c.execute("INSERT INTO folders(id, path) VALUES (1, '/p')", []).unwrap();
            c.execute("INSERT INTO photos(folder_id, path, size, mtime) VALUES (1, '/p/a.jpg', 1, 1)", []).unwrap();
        }
        let db = Db::open(&p).unwrap();
        let rows = db.list_photos_in(&PhotoQuery::default(), 0, 10).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!((rows[0].meta.kind, rows[0].meta.duration_ms), (MediaKind::Photo, None));
    }

    fn geo_meta(taken: i64, lat: f64, lon: f64) -> PhotoMeta {
        PhotoMeta { taken_at: Some(taken), gps_lat: Some(lat), gps_lon: Some(lon), ..Default::default() }
    }

    fn geo_library() -> (Db, FolderId) {
        let db = Db::open_in_memory().unwrap();
        let f = db.add_folder("/p").unwrap();
        db.upsert_photo(f, "/p/paris.jpg", 1, 1, &geo_meta(100, 48.86, 2.35)).unwrap();
        db.upsert_photo(f, "/p/rome.jpg", 1, 1, &geo_meta(200, 41.90, 12.50)).unwrap();
        db.upsert_photo(f, "/p/suva.jpg", 1, 1, &geo_meta(300, -18.14, 178.44)).unwrap(); // Fiji, east of the 180 line
        db.upsert_photo(f, "/p/taveuni.jpg", 1, 1, &geo_meta(400, -16.85, -179.95)).unwrap(); // west of the line
        db.upsert_photo(f, "/p/pole.jpg", 1, 1, &geo_meta(500, -89.99, 0.0)).unwrap();
        db.upsert_photo(f, "/p/nogps.jpg", 1, 1, &meta(Some(600))).unwrap();
        db.upsert_photo(f, "/p/clip.mp4", 1, 1, &PhotoMeta { kind: MediaKind::Video, ..geo_meta(700, 48.85, 2.34) }).unwrap();
        (db, f)
    }

    #[test]
    fn until_is_an_exclusive_upper_bound_and_combines_with_since() {
        let (db, _f) = mixed_library(); // taken 100, 200, 300, 400
        let q = PhotoQuery { until: Some(300), ..Default::default() };
        let got = db.list_photos_in(&q, 0, 10).unwrap();
        assert_eq!(got.iter().map(|p| p.path.as_str()).collect::<Vec<_>>(), ["/p/b.mp4", "/p/a.jpg"]);
        assert_eq!(db.count_photos_in(&q).unwrap(), 2);
        assert_eq!(db.day_groups(&q).unwrap().iter().map(|g| g.count).sum::<i64>(), 2);
        assert_eq!(db.photos_at(&q, &[1]).unwrap()[0].path, "/p/a.jpg");
        let both = PhotoQuery { since: Some(200), until: Some(400), ..Default::default() };
        assert_eq!(db.count_photos_in(&both).unwrap(), 2); // 200 and 300; 400 is excluded
    }

    #[test]
    fn geo_points_honour_the_until_bound() {
        let (db, _f) = geo_library(); // located items taken at 100, 200, 300, 400, 500 (photos) and 700 (video)
        assert_eq!(db.list_geo_points(None, None, None, Some(300)).unwrap().len(), 2);
        assert_eq!(db.list_geo_points(None, None, Some(200), Some(500)).unwrap().len(), 3); // 200, 300, 400
    }

    #[test]
    fn geo_points_honour_the_since_bound() {
        let (db, _f) = geo_library(); // located items taken at 100, 200, 300, 400, 500 (photos) and 700 (video)
        assert_eq!(db.list_geo_points(None, None, Some(300), None).unwrap().len(), 4);
        assert_eq!(db.list_geo_points(None, Some(MediaKind::Video), Some(300), None).unwrap().len(), 1);
        assert!(db.list_geo_points(None, None, Some(10_000), None).unwrap().is_empty());
        assert_eq!(db.list_geo_points(None, None, None, None).unwrap().len(), 6);
    }

    fn bq(south: f64, west: f64, north: f64, east: f64) -> PhotoQuery {
        PhotoQuery { bounds: Some(Bounds { south, west, north, east }), ..Default::default() }
    }

    #[test]
    fn bounds_select_what_is_inside_edges_included_and_skip_items_without_gps() {
        let (db, _f) = geo_library();
        assert_eq!(file_names(&db, &bq(40.0, 0.0, 50.0, 15.0)), ["clip.mp4", "rome.jpg", "paris.jpg"]); // newest first
        assert_eq!(file_names(&db, &bq(48.86, 2.35, 48.86, 2.35)), ["paris.jpg"]); // a point box: edges inclusive
        assert!(file_names(&db, &bq(0.0, 0.0, 10.0, 10.0)).is_empty());
        let all = file_names(&db, &bq(-90.0, -180.0, 90.0, 180.0));
        assert_eq!(all.len(), 6); // everything with GPS; nogps.jpg never matches
        assert!(!all.contains(&"nogps.jpg".to_string()));
    }

    #[test]
    fn bounds_across_the_antimeridian_and_at_the_poles() {
        let (db, _f) = geo_library();
        assert_eq!(file_names(&db, &bq(-20.0, 170.0, -10.0, -170.0)), ["taveuni.jpg", "suva.jpg"]); // west > east
        assert_eq!(file_names(&db, &bq(-20.0, -180.0, -10.0, -170.0)), ["taveuni.jpg"]); // only the western side
        assert_eq!(file_names(&db, &bq(-95.0, -10.0, -85.0, 10.0)), ["pole.jpg"]); // latitudes clamp
    }

    #[test]
    fn invalid_bounds_match_nothing() {
        let (db, _f) = geo_library();
        assert!(file_names(&db, &bq(f64::NAN, 0.0, 10.0, 10.0)).is_empty());
        assert!(file_names(&db, &bq(20.0, 0.0, 10.0, 10.0)).is_empty()); // south > north
        assert_eq!(db.count_photos_in(&bq(20.0, 0.0, 10.0, 10.0)).unwrap(), 0);
    }

    #[test]
    fn bounds_combine_with_kind_and_folder_and_counts_and_groups_agree() {
        let (db, f) = geo_library();
        let other = db.add_folder("/q").unwrap();
        db.upsert_photo(other, "/q/paris2.jpg", 1, 1, &geo_meta(150, 48.9, 2.3)).unwrap();
        let europe = Bounds { south: 40.0, west: 0.0, north: 50.0, east: 15.0 };
        let q = |kind, folder_id| PhotoQuery { bounds: Some(europe), kind, folder_id, ..Default::default() };
        assert_eq!(file_names(&db, &q(Some(MediaKind::Video), None)), ["clip.mp4"]);
        assert_eq!(file_names(&db, &q(None, Some(other))), ["paris2.jpg"]);
        assert_eq!(file_names(&db, &q(Some(MediaKind::Photo), Some(f))), ["rome.jpg", "paris.jpg"]);
        for query in [q(None, None), q(Some(MediaKind::Photo), Some(f)), q(None, Some(other))] {
            let listed = db.list_photos_in(&query, 0, 100).unwrap().len() as i64;
            assert_eq!(db.count_photos_in(&query).unwrap(), listed);
            assert_eq!(db.day_groups(&query).unwrap().iter().map(|g| g.count).sum::<i64>(), listed);
        }
    }

    #[test]
    fn geo_points_are_the_located_items_honouring_filters() {
        let (db, f) = geo_library();
        let all = db.list_geo_points(None, None, None, None).unwrap();
        assert_eq!(all.len(), 6);
        assert!(all.iter().all(|p| p.lat.abs() <= 90.0 && p.lon.abs() <= 180.0));
        let paris = db.list_photos_in(&bq(48.0, 2.0, 49.0, 3.0), 0, 10).unwrap();
        assert!(all.iter().any(|p| p.id == paris[0].id));
        assert_eq!(db.list_geo_points(None, Some(MediaKind::Video), None, None).unwrap().len(), 1);
        assert_eq!(db.list_geo_points(Some(f), Some(MediaKind::Photo), None, None).unwrap().len(), 5);
        let other = db.add_folder("/q").unwrap();
        assert!(db.list_geo_points(Some(other), None, None, None).unwrap().is_empty());
    }

    #[test]
    fn missing_items_are_not_on_the_map() {
        let (db, f) = geo_library();
        db.mark_missing_except(f, &HashSet::new()).unwrap();
        assert!(db.list_geo_points(None, None, None, None).unwrap().is_empty());
        assert!(file_names(&db, &bq(-90.0, -180.0, 90.0, 180.0)).is_empty());
    }

    /// Timing on a big library, by hand in release:
    /// `cargo test --release -p pixie-index bounds_paging_100k -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn bounds_paging_100k() {
        let db = Db::open_in_memory().unwrap();
        let f = db.add_folder("/p").unwrap();
        {
            let c = db.conn();
            c.execute_batch("BEGIN").unwrap();
            for i in 0..100_000i64 {
                let (lat, lon) = (-60.0 + (i % 1000) as f64 * 0.12, -170.0 + (i / 1000) as f64 * 3.4);
                c.execute(
                    "INSERT INTO photos (folder_id, path, size, mtime, taken_at, gps_lat, gps_lon, missing, kind) VALUES (?1, ?2, 1, 1, ?3, ?4, ?5, 0, 0)",
                    params![f, format!("/p/{i}.jpg"), 1_500_000_000 + i * 600, lat, lon],
                )
                .unwrap();
            }
            c.execute_batch("COMMIT").unwrap();
        }
        let q = bq(0.0, -50.0, 40.0, 40.0);
        let time = |label: &str, f: &dyn Fn() -> usize| {
            let t = std::time::Instant::now();
            let n = f();
            let ms = t.elapsed().as_secs_f64() * 1000.0;
            println!("{label}: {n} rows, {ms:.1} ms");
            ms
        };
        let slowest = [
            time("count in box", &|| db.count_photos_in(&q).unwrap() as usize),
            time("day groups in box", &|| db.day_groups(&q).unwrap().len()),
            time("page of 200 at offset 500", &|| db.list_photos_in(&q, 500, 200).unwrap().len()),
            time("geo points", &|| db.list_geo_points(None, None, None, None).unwrap().len()),
        ]
        .into_iter()
        .fold(0.0, f64::max);
        assert!(slowest < 1500.0, "too slow: {slowest} ms");
    }

    fn contact(nick: &str, email: &str) -> NewContact {
        NewContact { id: None, nickname: nick.into(), name: None, email: email.into() }
    }

    #[test]
    fn contacts_are_saved_listed_edited_and_deleted() {
        let db = Db::open_in_memory().unwrap();
        let dad = db.save_contact(&contact("dad", "dad@example.com")).unwrap();
        let si = db.save_contact(&NewContact { name: Some("Simon Brown".into()), ..contact("Simon", "si@example.com") }).unwrap();
        let all = db.list_contacts().unwrap();
        assert_eq!(all.iter().map(|c| c.nickname.as_str()).collect::<Vec<_>>(), ["dad", "Simon"]); // case-insensitive order
        assert_eq!(si.name.as_deref(), Some("Simon Brown"));
        let edited = db.save_contact(&NewContact { id: Some(dad.id), ..contact("Dad", "dad@new.com") }).unwrap();
        assert_eq!((edited.id, edited.nickname.as_str(), edited.email.as_str()), (dad.id, "Dad", "dad@new.com"));
        db.delete_contact(si.id).unwrap();
        assert_eq!(db.list_contacts().unwrap().len(), 1);
        assert!(db.get_contact(si.id).unwrap().is_none());
    }

    #[test]
    fn nicknames_are_unique_ignoring_case_and_input_is_validated() {
        let db = Db::open_in_memory().unwrap();
        db.save_contact(&contact("dad", "dad@example.com")).unwrap();
        let e = db.save_contact(&contact("DAD", "other@example.com")).unwrap_err();
        assert!(e.user_message().contains("already"), "{}", e.user_message());
        assert!(db.save_contact(&contact("mum", "not-an-address")).is_err());
        assert!(db.save_contact(&contact("a,b", "a@b.com")).is_err());
        assert_eq!(db.list_contacts().unwrap().len(), 1);
    }

    fn account() -> MailAccount {
        MailAccount {
            display_name: Some("Paul".into()), email: "me@outlook.com".into(), provider: "outlook".into(),
            host: "smtp-mail.outlook.com".into(), port: 587, security: MailSecurity::Starttls, limit_mb: 20,
        }
    }

    #[test]
    fn the_mail_account_is_one_row_that_can_be_replaced_and_deleted() {
        let db = Db::open_in_memory().unwrap();
        assert_eq!(db.get_mail_account().unwrap(), None);
        db.save_mail_account(&account()).unwrap();
        assert_eq!(db.get_mail_account().unwrap(), Some(account()));
        db.save_mail_account(&MailAccount { limit_mb: 25, ..account() }).unwrap();
        assert_eq!(db.get_mail_account().unwrap().unwrap().limit_mb, 25);
        db.delete_mail_account().unwrap();
        assert_eq!(db.get_mail_account().unwrap(), None);
    }

    #[test]
    fn account_input_is_validated() {
        let db = Db::open_in_memory().unwrap();
        for bad in [
            MailAccount { email: "nope".into(), ..account() },
            MailAccount { host: "a b".into(), ..account() },
            MailAccount { port: 0, ..account() },
            MailAccount { limit_mb: 0, ..account() },
            MailAccount { limit_mb: 500, ..account() },
            MailAccount { display_name: Some("A\nBcc: x@y.com".into()), ..account() },
            MailAccount { display_name: Some("x".repeat(101)), ..account() },
        ] {
            assert!(db.save_mail_account(&bad).is_err(), "{bad:?}");
        }
        assert_eq!(db.get_mail_account().unwrap(), None);
    }

    #[test]
    fn photos_at_returns_the_items_at_those_positions_in_the_current_order() {
        let (db, _f) = geo_library(); // 7 items with distinct dates
        let q = PhotoQuery::default();
        let all = db.list_photos_in(&q, 0, 100).unwrap();
        let got = db.photos_at(&q, &[2, 0, 5]).unwrap();
        assert_eq!(got.iter().map(|p| p.id).collect::<Vec<_>>(), [all[2].id, all[0].id, all[5].id]);
        assert!(db.photos_at(&q, &[99]).unwrap().is_empty()); // past the end: nothing, the caller compares counts
        assert!(db.photos_at(&q, &[-1]).unwrap().is_empty());
        assert!(db.photos_at(&q, &(0..201).collect::<Vec<_>>()).is_err()); // more than 200 positions
    }

    #[test]
    fn v4_migration_keeps_existing_data() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("v3.db");
        {
            let c = Connection::open(&p).unwrap();
            for m in &MIGRATIONS[..3] {
                c.execute_batch(m).unwrap();
            }
            c.pragma_update(None, "user_version", 3).unwrap();
            c.execute("INSERT INTO folders(id, path) VALUES (1, '/p')", []).unwrap();
            c.execute("INSERT INTO photos(folder_id, path, size, mtime) VALUES (1, '/p/a.jpg', 1, 1)", []).unwrap();
        }
        let db = Db::open(&p).unwrap();
        assert_eq!(db.count_photos().unwrap(), 1);
        assert!(db.list_contacts().unwrap().is_empty());
        assert_eq!(db.get_mail_account().unwrap(), None);
    }

    #[test]
    fn a_snapshot_keeps_the_listing_and_its_positions_stable_while_photos_are_added() {
        let (db, f) = mixed_library(); // 4 items
        let snap = db.max_photo_id().unwrap();
        let q = PhotoQuery { max_id: Some(snap), ..Default::default() };
        let before = db.list_photos_in(&q, 0, 100).unwrap();
        assert_eq!(before.len(), 4);
        // a scan adds a photo that sorts first (newest)
        db.upsert_photo(f, "/p/new.jpg", 1, 1, &meta(Some(9_999_999))).unwrap();
        // the snapshot does not see it: same items, same positions, same counts and day groups
        let after = db.list_photos_in(&q, 0, 100).unwrap();
        assert_eq!(after.iter().map(|p| p.id).collect::<Vec<_>>(), before.iter().map(|p| p.id).collect::<Vec<_>>());
        assert_eq!(db.count_photos_in(&q).unwrap(), 4);
        assert_eq!(db.day_groups(&q).unwrap().iter().map(|g| g.count).sum::<i64>(), 4);
        assert_eq!(db.photos_at(&q, &[0]).unwrap()[0].id, before[0].id);
        // without a snapshot the new photo is there and shifts everything down
        assert_eq!(db.count_photos_in(&PhotoQuery::default()).unwrap(), 5);
        assert_eq!(db.list_photos_in(&PhotoQuery::default(), 0, 1).unwrap()[0].path, "/p/new.jpg");
        // a later snapshot includes it
        let q2 = PhotoQuery { max_id: Some(db.max_photo_id().unwrap()), ..Default::default() };
        assert_eq!(db.count_photos_in(&q2).unwrap(), 5);
    }

    #[test]
    fn since_keeps_items_taken_at_or_after_it_and_drops_undated_ones() {
        let (db, f) = mixed_library(); // taken 100, 200, 300, 400
        db.upsert_photo(f, "/p/undated.jpg", 1, 1, &meta(None)).unwrap();
        let q = PhotoQuery { since: Some(300), ..Default::default() };
        let got = db.list_photos_in(&q, 0, 10).unwrap();
        assert_eq!(got.iter().map(|p| p.path.as_str()).collect::<Vec<_>>(), ["/p/d.mov", "/p/c.jpg"]);
        assert_eq!(db.count_photos_in(&q).unwrap(), 2);
        assert_eq!(db.day_groups(&q).unwrap().iter().map(|g| g.count).sum::<i64>(), 2);
        assert_eq!(db.photos_at(&q, &[1]).unwrap()[0].path, "/p/c.jpg");
        // combines with kind
        let qv = PhotoQuery { since: Some(300), kind: Some(MediaKind::Video), ..Default::default() };
        assert_eq!(db.count_photos_in(&qv).unwrap(), 1);
    }

    #[test]
    fn folder_infos_count_live_items_per_folder() {
        let (db, f) = mixed_library();
        let g = db.add_folder("/q").unwrap();
        db.upsert_photo(g, "/q/x.jpg", 1, 1, &meta(Some(5))).unwrap();
        let seen: HashSet<String> = ["/p/a.jpg".to_string()].into_iter().collect();
        db.mark_missing_except(f, &seen).unwrap(); // 3 of folder f's 4 items go missing
        let infos = db.folder_infos().unwrap();
        assert_eq!(infos.iter().map(|i| (i.path.as_str(), i.count)).collect::<Vec<_>>(), [("/p", 1), ("/q", 1)]);
        let empty = db.add_folder("/empty").unwrap();
        assert!(db.folder_infos().unwrap().iter().any(|i| i.id == empty && i.count == 0));
    }

    #[test]
    fn removing_a_folder_removes_its_items_only() {
        let (db, f) = mixed_library();
        let g = db.add_folder("/q").unwrap();
        db.upsert_photo(g, "/q/x.jpg", 1, 1, &meta(Some(5))).unwrap();
        db.remove_folder(f).unwrap();
        assert_eq!(db.count_photos_in(&PhotoQuery::default()).unwrap(), 1);
        assert_eq!(db.list_folders().unwrap().len(), 1);
        db.remove_folder(9999).unwrap(); // unknown id: nothing to do, not an error
    }

    #[test]
    fn the_snapshot_combines_with_the_other_filters_and_an_empty_library_has_id_zero() {
        let empty = Db::open_in_memory().unwrap();
        assert_eq!(empty.max_photo_id().unwrap(), 0);
        let (db, f) = geo_library();
        let snap = db.max_photo_id().unwrap();
        db.upsert_photo(f, "/p/later.jpg", 1, 1, &geo_meta(800, 48.86, 2.35)).unwrap(); // inside the Europe box
        let q = PhotoQuery {
            max_id: Some(snap),
            bounds: Some(Bounds { south: 40.0, west: 0.0, north: 50.0, east: 15.0 }),
            ..Default::default()
        };
        assert_eq!(file_names(&db, &q), ["clip.mp4", "rome.jpg", "paris.jpg"]); // not later.jpg
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

    // 2024-06-01, 06-02, 06-03 at 00:00 UTC.
    const D1: i64 = 1_717_200_000;
    const D3: i64 = 1_717_372_800;

    /// a, b on Jun 3; c on Jun 1; d, e undated. Folder /q holds f on Jun 2.
    fn dated_library() -> (Db, FolderId, FolderId) {
        let db = Db::open_in_memory().unwrap();
        let f = db.add_folder("/p").unwrap();
        let g = db.add_folder("/q").unwrap();
        for (n, t) in [("a", Some(D3 + 3600)), ("b", Some(D3 + 7200)), ("c", Some(D1 + 10)), ("d", None), ("e", None)] {
            db.upsert_photo(f, &format!("/p/{n}.jpg"), 1, 1, &meta(t)).unwrap();
        }
        db.upsert_photo(g, "/q/f.jpg", 1, 1, &meta(Some(D1 + 86_400 + 5))).unwrap();
        (db, f, g)
    }

    fn file_names(db: &Db, q: &PhotoQuery) -> Vec<String> {
        db.list_photos_in(q, 0, 100).unwrap().into_iter().map(|p| p.path.rsplit('/').next().unwrap().to_string()).collect()
    }

    fn group(day: Option<&str>, count: i64, first_index: i64) -> DayGroup {
        DayGroup { day: day.map(String::from), count, first_index }
    }

    #[test]
    fn newest_lists_and_groups_newest_first_with_undated_last() {
        let (db, _, _) = dated_library();
        let q = PhotoQuery::default();
        assert_eq!(file_names(&db, &q), ["b.jpg", "a.jpg", "f.jpg", "c.jpg", "e.jpg", "d.jpg"]);
        assert_eq!(
            db.day_groups(&q).unwrap(),
            [group(Some("2024-06-03"), 2, 0), group(Some("2024-06-02"), 1, 2), group(Some("2024-06-01"), 1, 3), group(None, 2, 4)]
        );
    }

    #[test]
    fn oldest_lists_and_groups_oldest_first_but_undated_still_last() {
        let (db, _, _) = dated_library();
        let q = PhotoQuery { order: SortOrder::Oldest, folder_id: None, kind: None, bounds: None, max_id: None, since: None, until: None };
        assert_eq!(file_names(&db, &q), ["c.jpg", "f.jpg", "a.jpg", "b.jpg", "d.jpg", "e.jpg"]);
        assert_eq!(
            db.day_groups(&q).unwrap(),
            [group(Some("2024-06-01"), 1, 0), group(Some("2024-06-02"), 1, 1), group(Some("2024-06-03"), 2, 2), group(None, 2, 4)]
        );
    }

    #[test]
    fn group_counts_add_up_and_first_index_points_at_the_group() {
        let (db, _, _) = dated_library();
        for order in [SortOrder::Newest, SortOrder::Oldest] {
            let q = PhotoQuery { order, folder_id: None, kind: None, bounds: None, max_id: None, since: None, until: None };
            let groups = db.day_groups(&q).unwrap();
            assert_eq!(groups.iter().map(|g| g.count).sum::<i64>(), db.count_photos_in(&q).unwrap());
            for g in &groups {
                let members = db.list_photos_in(&q, g.first_index, g.count).unwrap();
                assert_eq!(members.len() as i64, g.count, "{g:?}");
                let days: std::collections::HashSet<Option<i64>> =
                    members.iter().map(|p| p.meta.taken_at.map(|t| t.div_euclid(86_400))).collect();
                assert_eq!(days.len(), 1, "a group must be exactly one day: {g:?}");
                assert_eq!(days.into_iter().next().unwrap().is_none(), g.day.is_none(), "{g:?}");
            }
        }
    }

    #[test]
    fn folder_filter_applies_to_list_count_and_groups() {
        let (db, f, g) = dated_library();
        let q = PhotoQuery { order: SortOrder::Newest, folder_id: Some(g), kind: None, bounds: None, max_id: None, since: None, until: None };
        assert_eq!(file_names(&db, &q), ["f.jpg"]);
        assert_eq!(db.count_photos_in(&q).unwrap(), 1);
        assert_eq!(db.day_groups(&q).unwrap(), [group(Some("2024-06-02"), 1, 0)]);
        let q = PhotoQuery { order: SortOrder::Newest, folder_id: Some(f), kind: None, bounds: None, max_id: None, since: None, until: None };
        assert_eq!(db.count_photos_in(&q).unwrap(), 5);
    }

    #[test]
    fn empty_folder_and_empty_library_have_no_groups() {
        let db = Db::open_in_memory().unwrap();
        let f = db.add_folder("/empty").unwrap();
        let q = PhotoQuery { order: SortOrder::Newest, folder_id: Some(f), kind: None, bounds: None, max_id: None, since: None, until: None };
        assert!(db.day_groups(&q).unwrap().is_empty());
        assert_eq!(db.count_photos_in(&q).unwrap(), 0);
        assert!(db.day_groups(&PhotoQuery::default()).unwrap().is_empty());
    }

    #[test]
    fn missing_photos_are_left_out_of_groups() {
        let (db, f, _) = dated_library();
        let seen: HashSet<String> = ["/p/a.jpg".to_string()].into();
        db.mark_missing_except(f, &seen).unwrap();
        let total: i64 = db.day_groups(&PhotoQuery::default()).unwrap().iter().map(|g| g.count).sum();
        assert_eq!(total, 2); // a.jpg and f.jpg (other folder)
    }

    #[test]
    fn listing_is_served_from_an_index_in_both_directions_with_and_without_a_folder() {
        let (db, f, _) = dated_library();
        for (order, folder_id) in [
            (SortOrder::Newest, None), (SortOrder::Oldest, None),
            (SortOrder::Newest, Some(f)), (SortOrder::Oldest, Some(f)),
        ] {
            let q = PhotoQuery { order, folder_id, kind: None, bounds: None, max_id: None, since: None, until: None };
            let c = db.conn();
            let filter = Filter::of(&q);
            let mut s = c.prepare(&format!("EXPLAIN QUERY PLAN {}", list_sql(&q, &filter))).unwrap();
            let (limit, offset) = (10i64, 0i64);
            let mut params = filter.params();
            params.push((":limit", &limit as &dyn rusqlite::ToSql));
            params.push((":offset", &offset as &dyn rusqlite::ToSql));
            let plan: Vec<String> = s.query_map(params.as_slice(), |r| r.get::<_, String>(3)).unwrap().map(|r| r.unwrap()).collect();
            let plan = plan.join(" | ");
            if folder_id.is_some() {
                assert!(plan.contains("idx_photos_folder_"), "{order:?} must seek by folder first: {plan}");
            }
            assert!(plan.contains("idx_photos_"), "{order:?} {folder_id:?} should use an index: {plan}");
            assert!(!plan.contains("TEMP B-TREE"), "{order:?} {folder_id:?} must not sort: {plan}");
        }
    }

    #[test]
    fn v2_migration_upgrades_a_v1_database_and_clears_bad_gps() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("v1.db");
        {
            let c = Connection::open(&p).unwrap();
            c.execute_batch(MIGRATIONS[0]).unwrap();
            c.execute("INSERT INTO folders(id, path) VALUES (1, '/p')", []).unwrap();
            for (path, lat, lon) in [
                ("/p/good.jpg", Some(51.5), Some(-0.12)),
                ("/p/zero.jpg", Some(0.0), Some(0.0)),
                ("/p/range.jpg", Some(95.0), Some(10.0)),
                ("/p/half.jpg", Some(10.0), None),
            ] {
                c.execute(
                    "INSERT INTO photos(folder_id, path, size, mtime, gps_lat, gps_lon) VALUES (1, ?1, 1, 1, ?2, ?3)",
                    params![path, lat, lon],
                )
                .unwrap();
            }
            c.pragma_update(None, "user_version", 1).unwrap();
        }
        let db = Db::open(&p).unwrap();
        let gps = |path: &str| {
            let id = db.find_photo_stat(path).unwrap().unwrap().id;
            let m = db.get_photo(id).unwrap().unwrap().meta;
            (m.gps_lat, m.gps_lon)
        };
        assert_eq!(gps("/p/good.jpg"), (Some(51.5), Some(-0.12)));
        assert_eq!(gps("/p/zero.jpg"), (None, None));
        assert_eq!(gps("/p/range.jpg"), (None, None));
        assert_eq!(gps("/p/half.jpg"), (None, None));
    }
}
