use crate::trips::{compute_route, compute_trips, day_of, distance_km, infer_home, DayItem, Route, Trip, TripSettings, EARLIEST_REAL};
use pixie_core::{DupGroup, DupStatus, validate_email, validate_header_text, validate_host, validate_nickname, Bounds, Contact, DayGroup, FaceInfo, FacePerson, FaceTile, Folder, FolderId, FolderInfo, GeoPoint, MailAccount, MailSecurity, MediaKind, NewContact, NewFace, PersonInfo, Photo, PhotoId, PhotoMeta, PhotoQuery, PixieError, Result, SortOrder};
use rusqlite::{params, Connection, OptionalExtension};
use std::collections::{HashMap, HashSet};
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
    // V5: image embeddings for search. A NULL vector marks a photo that could not be analysed, so it is not retried.
    "CREATE TABLE embeddings (
        photo_id      INTEGER NOT NULL REFERENCES photos(id) ON DELETE CASCADE,
        model_version TEXT NOT NULL,
        vector        BLOB,
        PRIMARY KEY (photo_id, model_version)
     );",
    // V6: content tags ("beach", "dog", ...) derived from the stored vectors. `tags_version` on an embedding says
    // which tag vocabulary it was last tagged with, so a new vocabulary re-tags without decoding any image.
    "ALTER TABLE embeddings ADD COLUMN tags_version TEXT;
     CREATE TABLE photo_tags (
        photo_id INTEGER NOT NULL REFERENCES photos(id) ON DELETE CASCADE,
        tag      TEXT NOT NULL,
        score    REAL NOT NULL,
        PRIMARY KEY (photo_id, tag)
     );
     CREATE INDEX idx_photo_tags_tag ON photo_tags(tag);",
    // V7: faces and the people they are grouped into. Boxes are fractions of the upright image. `face_scans` says a
    // photo was scanned for faces with a given model version (also when it has none), so it is not scanned again.
    // `pinned` marks a face the user said is not that person: it is never grouped automatically again.
    "CREATE TABLE people (
        id       INTEGER PRIMARY KEY,
        name     TEXT,
        hidden   INTEGER NOT NULL DEFAULT 0,
        centroid BLOB NOT NULL,
        n        INTEGER NOT NULL DEFAULT 0
     );
     CREATE TABLE faces (
        id        INTEGER PRIMARY KEY,
        photo_id  INTEGER NOT NULL REFERENCES photos(id) ON DELETE CASCADE,
        x REAL NOT NULL, y REAL NOT NULL, w REAL NOT NULL, h REAL NOT NULL,
        score     REAL NOT NULL,
        embedding BLOB NOT NULL,
        person_id INTEGER REFERENCES people(id) ON DELETE SET NULL,
        pinned    INTEGER NOT NULL DEFAULT 0
     );
     CREATE INDEX idx_faces_photo ON faces(photo_id);
     CREATE INDEX idx_faces_person ON faces(person_id);
     CREATE TABLE face_scans (
        photo_id INTEGER PRIMARY KEY REFERENCES photos(id) ON DELETE CASCADE,
        version  TEXT NOT NULL
     );",
    // V8: "not this person" no longer parks a face for good. The face is guessed again automatically, but never as a
    // person it was rejected from: those rejections are kept here.
    "CREATE TABLE face_rejections (
        face_id   INTEGER NOT NULL REFERENCES faces(id) ON DELETE CASCADE,
        person_id INTEGER NOT NULL REFERENCES people(id) ON DELETE CASCADE,
        PRIMARY KEY (face_id, person_id)
     );",
    // V9: names the user gave to trips. A trip is found again on every look, so a name is kept against the day the trip
    // started and applies to whichever trip contains that day.
    "CREATE TABLE trip_names (
        start_day INTEGER PRIMARY KEY,
        name      TEXT NOT NULL
     );",
    // V10: content hashes for finding copies. `quick_key` is cheap (size plus the start and end of the file); `hash` is
    // the whole file and is only worked out for files whose quick key another file shares. Both are cleared when the
    // file changes. A quick key starting with "!" marks a file that could not be read (it matches nothing).
    "ALTER TABLE photos ADD COLUMN quick_key TEXT;
     ALTER TABLE photos ADD COLUMN hash TEXT;
     CREATE INDEX idx_photos_quick ON photos(quick_key) WHERE quick_key IS NOT NULL;
     CREATE INDEX idx_photos_hash  ON photos(hash) WHERE hash IS NOT NULL;",
    // V11: `dup` marks a live item that is an extra copy of another, so Browse shows one item per set of identical
    // files. `keep_pick` marks the copy the user chose to show for its set (without one, the oldest is shown).
    "ALTER TABLE photos ADD COLUMN dup INTEGER NOT NULL DEFAULT 0;
     ALTER TABLE photos ADD COLUMN keep_pick INTEGER NOT NULL DEFAULT 0;",
    // V12: the listing indexes include `dup`, so counting or paging what Browse shows (`missing = 0 AND dup = 0`) is
    // answered from the index alone. Without it every count read every row of the library (50 to 100 ms each, on the
    // thread the window runs on).
    "DROP INDEX idx_photos_newest;
     DROP INDEX idx_photos_oldest;
     DROP INDEX idx_photos_folder_newest;
     DROP INDEX idx_photos_folder_oldest;
     DROP INDEX idx_photos_kind_newest;
     DROP INDEX idx_photos_kind_oldest;
     CREATE INDEX idx_photos_newest ON photos(missing, dup, (taken_at IS NULL), taken_at DESC, id DESC);
     CREATE INDEX idx_photos_oldest ON photos(missing, dup, (taken_at IS NULL), taken_at ASC, id ASC);
     CREATE INDEX idx_photos_folder_newest ON photos(folder_id, missing, dup, (taken_at IS NULL), taken_at DESC, id DESC);
     CREATE INDEX idx_photos_folder_oldest ON photos(folder_id, missing, dup, (taken_at IS NULL), taken_at ASC, id ASC);
     CREATE INDEX idx_photos_kind_newest ON photos(kind, missing, dup, (taken_at IS NULL), taken_at DESC, id DESC);
     CREATE INDEX idx_photos_kind_oldest ON photos(kind, missing, dup, (taken_at IS NULL), taken_at ASC, id ASC);",
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

fn row_to_tile(r: &rusqlite::Row) -> rusqlite::Result<FaceTile> {
    Ok(FaceTile { id: r.get(0)?, photo_id: r.get(1)?, mtime: r.get(2)?, size: r.get(3)?, x: r.get(4)?, y: r.get(5)?, w: r.get(6)?, h: r.get(7)? })
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
    search: bool,
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
            search: q.search,
        }
    }
    fn sql(&self) -> String {
        // Extra copies of a duplicate are not listed: one item stands for each set of identical files.
        let mut s = String::from(" AND dup = 0");
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
        if self.search {
            s.push_str(" AND id IN (SELECT photo_id FROM search_hits)");
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

/// Rebuild `search_hits` as the photos in every active hit set (text search, tag filter).
fn refresh_hits(tx: &rusqlite::Transaction) -> Result<()> {
    tx.execute("DELETE FROM search_hits", []).map_err(db_err)?;
    let mut sets = Vec::new();
    for (name, table) in [("text", "text_hits"), ("tag", "tag_hits"), ("person", "person_hits")] {
        let n: i64 = tx.query_row("SELECT COUNT(*) FROM filter_state WHERE name = ?1", params![name], |r| r.get(0)).map_err(db_err)?;
        if n > 0 {
            sets.push(format!("SELECT photo_id FROM {table}"));
        }
    }
    if sets.is_empty() {
        return Ok(());
    }
    tx.execute(&format!("INSERT INTO search_hits {}", sets.join(" INTERSECT ")), []).map_err(db_err)?;
    Ok(())
}

fn blob_of(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|x| x.to_le_bytes()).collect()
}

fn vector_of(b: &[u8]) -> Vec<f32> {
    b.chunks_exact(4).map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect()
}

/// A person's centroid and face count from the faces they have now. A person with no faces is left as it is.
fn recompute_person(tx: &Connection, id: i64) -> Result<()> {
    let mut sum: Vec<f32> = Vec::new();
    let mut n = 0i64;
    {
        let mut st = tx.prepare("SELECT embedding FROM faces WHERE person_id = ?1").map_err(db_err)?;
        let rows = st.query_map(params![id], |r| r.get::<_, Vec<u8>>(0)).map_err(db_err)?;
        for row in rows {
            let v = vector_of(&row.map_err(db_err)?);
            if sum.is_empty() {
                sum = vec![0.0; v.len()];
            }
            for (a, b) in sum.iter_mut().zip(&v) {
                *a += b;
            }
            n += 1;
        }
    }
    if n == 0 {
        return Ok(()); // no faces left: `n` stays as it was, which is how `prune_people` tells an emptied person from a new one
    }
    let norm = sum.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        sum.iter_mut().for_each(|x| *x /= norm);
    }
    tx.execute("UPDATE people SET centroid = ?2, n = ?3 WHERE id = ?1", params![id, blob_of(&sum), n]).map_err(db_err)?;
    Ok(())
}

/// The people with faces in the photos matched by `where_sql` (a condition on `photos p`), taken before those photos
/// or their faces are deleted: the cascade does not tell their centroids and counts.
fn people_in(c: &Connection, where_sql: &str, args: impl rusqlite::Params) -> Result<Vec<i64>> {
    let mut st = c
        .prepare(&format!("SELECT DISTINCT f.person_id FROM faces f JOIN photos p ON p.id = f.photo_id WHERE f.person_id IS NOT NULL AND {where_sql}"))
        .map_err(db_err)?;
    let rows = st.query_map(args, |r| r.get(0)).map_err(db_err)?;
    rows.collect::<rusqlite::Result<Vec<_>>>().map_err(db_err)
}

/// Bring the given people up to date after faces were deleted, and drop unnamed ones left with none.
fn refresh_people(c: &Connection, ids: &[i64]) -> Result<()> {
    for id in ids {
        recompute_person(c, *id)?;
    }
    prune_people(c)
}

/// "Not this person": the face leaves its person and is remembered as not theirs. It then waits to be guessed again
/// (see `unassigned_faces`), and that guess never lands on a person it was rejected from.
fn reject_face_in(tx: &Connection, face_id: i64) -> Result<()> {
    let person: Option<i64> = tx.query_row("SELECT person_id FROM faces WHERE id = ?1", params![face_id], |r| r.get(0)).optional().map_err(db_err)?.flatten();
    tx.execute("UPDATE faces SET person_id = NULL WHERE id = ?1", params![face_id]).map_err(db_err)?;
    if let Some(p) = person {
        tx.execute("INSERT OR IGNORE INTO face_rejections(face_id, person_id) VALUES (?1, ?2)", params![face_id, p]).map_err(db_err)?;
        recompute_person(tx, p)?;
        prune_people(tx)?;
    }
    Ok(())
}

/// Put a face in a person (clearing any "not this person" mark) and bring both people up to date.
fn move_face_in(tx: &Connection, face_id: i64, person_id: i64) -> Result<()> {
    let old: Option<i64> = tx.query_row("SELECT person_id FROM faces WHERE id = ?1", params![face_id], |r| r.get(0)).optional().map_err(db_err)?.flatten();
    tx.execute("UPDATE faces SET person_id = ?2, pinned = 0 WHERE id = ?1", params![face_id, person_id]).map_err(db_err)?;
    tx.execute("DELETE FROM face_rejections WHERE face_id = ?1 AND person_id = ?2", params![face_id, person_id]).map_err(db_err)?; // put back by hand
    recompute_person(tx, person_id)?;
    if let Some(o) = old.filter(|o| *o != person_id) {
        recompute_person(tx, o)?;
    }
    prune_people(tx)
}

/// Unnamed people left with no faces are not worth keeping (`n = 0` ones are brand new, not yet given their first face).
fn prune_people(tx: &Connection) -> Result<()> {
    tx.execute(
        "DELETE FROM people WHERE name IS NULL AND n > 0 AND NOT EXISTS (SELECT 1 FROM faces WHERE person_id = people.id)",
        [],
    )
    .map_err(db_err)?;
    Ok(())
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
        conn.execute_batch(
            "CREATE TEMP TABLE IF NOT EXISTS search_hits (photo_id INTEGER PRIMARY KEY);
             CREATE TEMP TABLE IF NOT EXISTS text_hits (photo_id INTEGER PRIMARY KEY);
             CREATE TEMP TABLE IF NOT EXISTS tag_hits (photo_id INTEGER PRIMARY KEY);
             CREATE TEMP TABLE IF NOT EXISTS person_hits (photo_id INTEGER PRIMARY KEY);
             CREATE TEMP TABLE IF NOT EXISTS filter_state (name TEXT PRIMARY KEY, value TEXT);",
        )
        .map_err(db_err)?;
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
                 LEFT JOIN photos p ON p.folder_id = f.id AND p.missing = 0 AND p.dup = 0
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
        let mut c = self.conn();
        let tx = c.transaction().map_err(db_err)?;
        let affected = people_in(&tx, "p.folder_id = ?1", params![id])?; // its photos' faces go with the cascade
        tx.execute("DELETE FROM folders WHERE id = ?1", params![id]).map_err(db_err)?;
        refresh_people(&tx, &affected)?;
        tx.commit().map_err(db_err)?;
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
        let c = self.conn();
        // A file that changed on disk is a different picture: whatever was learned from the old one (including a
        // "could not analyse" marker from a half-copied file) no longer applies.
        let affected = people_in(&c, "p.path = ?1 AND (p.size != ?2 OR p.mtime != ?3)", params![path, size, mtime])?;
        for table in ["embeddings", "photo_tags", "face_scans", "faces"] {
            c.execute(
                &format!("DELETE FROM {table} WHERE photo_id = (SELECT id FROM photos WHERE path = ?1 AND (size != ?2 OR mtime != ?3))"),
                params![path, size, mtime],
            )
            .map_err(db_err)?;
        }
        refresh_people(&c, &affected)?;
        c
            .query_row(
                "INSERT INTO photos (folder_id, path, size, mtime, width, height, taken_at, camera, gps_lat, gps_lon, missing, kind, duration_ms)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 0, ?11, ?12)
                 ON CONFLICT(path) DO UPDATE SET
                    folder_id = excluded.folder_id, size = excluded.size, mtime = excluded.mtime,
                    width = excluded.width, height = excluded.height, taken_at = excluded.taken_at,
                    camera = excluded.camera, gps_lat = excluded.gps_lat, gps_lon = excluded.gps_lon,
                    kind = excluded.kind, duration_ms = excluded.duration_ms,
                    quick_key = CASE WHEN photos.size != excluded.size OR photos.mtime != excluded.mtime THEN NULL ELSE photos.quick_key END,
                    hash      = CASE WHEN photos.size != excluded.size OR photos.mtime != excluded.mtime THEN NULL ELSE photos.hash END,
                    dup       = CASE WHEN photos.size != excluded.size OR photos.mtime != excluded.mtime THEN 0 ELSE photos.dup END,
                    keep_pick = CASE WHEN photos.size != excluded.size OR photos.mtime != excluded.mtime THEN 0 ELSE photos.keep_pick END,
                    missing = 0
                 RETURNING id",
                params![folder_id, path, size, mtime, meta.width, meta.height, meta.taken_at, meta.camera, meta.gps_lat, meta.gps_lon, if meta.kind == MediaKind::Video { 1i64 } else { 0 }, meta.duration_ms],
                |r| r.get(0),
            )
            .map_err(db_err)
    }

    // ---- copies of the same file ----

    /// Live items not yet given a quick key, newest first: (id, path).
    pub fn photos_needing_quick_key(&self, limit: i64) -> Result<Vec<(PhotoId, String)>> {
        let c = self.conn();
        let mut s = c.prepare("SELECT id, path FROM photos WHERE missing = 0 AND quick_key IS NULL ORDER BY id DESC LIMIT ?1").map_err(db_err)?;
        let rows = s.query_map(params![limit], |r| Ok((r.get(0)?, r.get(1)?))).map_err(db_err)?.collect::<std::result::Result<Vec<_>, _>>().map_err(db_err)?;
        Ok(rows)
    }

    pub fn set_quick_key(&self, id: PhotoId, key: &str) -> Result<()> {
        self.conn().execute("UPDATE photos SET quick_key = ?2 WHERE id = ?1", params![id, key]).map_err(db_err)?;
        Ok(())
    }

    /// Live items with no full hash whose quick key another live item shares: the only ones that can be copies.
    pub fn photos_needing_hash(&self, limit: i64) -> Result<Vec<(PhotoId, String)>> {
        let c = self.conn();
        let mut s = c
            .prepare(
                "SELECT p.id, p.path FROM photos p
                 WHERE p.missing = 0 AND p.hash IS NULL AND p.quick_key IS NOT NULL AND substr(p.quick_key, 1, 1) != '!'
                   AND p.quick_key IN (SELECT quick_key FROM photos WHERE missing = 0 AND quick_key IS NOT NULL GROUP BY quick_key HAVING count(*) > 1)
                 ORDER BY p.id DESC LIMIT ?1",
            )
            .map_err(db_err)?;
        let rows = s.query_map(params![limit], |r| Ok((r.get(0)?, r.get(1)?))).map_err(db_err)?.collect::<std::result::Result<Vec<_>, _>>().map_err(db_err)?;
        Ok(rows)
    }

    pub fn set_hash(&self, id: PhotoId, hash: &str) -> Result<()> {
        self.conn().execute("UPDATE photos SET hash = ?2 WHERE id = ?1", params![id, hash]).map_err(db_err)?;
        Ok(())
    }

    /// Groups of two or more live items with the same content hash, most wasted space first; each group with the
    /// copy to keep first (the one the user chose, else the earliest date taken, then the shorter path). One query
    /// for every group: asking once per group makes SQLite scan all photos each time, which takes seconds for a
    /// library with many duplicates.
    pub fn duplicate_groups(&self) -> Result<Vec<DupGroup>> {
        let c = self.conn();
        let mut s = c
            .prepare(&format!(
                "SELECT {PHOTO_COLS}, hash, keep_pick FROM photos
                 WHERE missing = 0 AND hash IN (SELECT hash FROM photos WHERE missing = 0 AND hash IS NOT NULL GROUP BY hash HAVING count(*) > 1)
                 ORDER BY hash, (taken_at IS NULL), taken_at, length(path), path"
            ))
            .map_err(db_err)?;
        let rows = s
            .query_map([], |r| Ok((row_to_photo(r)?, r.get::<_, String>(14)?, r.get::<_, i64>(15)? != 0)))
            .map_err(db_err)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(db_err)?;
        let mut out: Vec<DupGroup> = Vec::new();
        let mut picked: Option<PhotoId> = None;
        for (photo, hash, pick) in rows {
            let new_group = out.last().map_or(true, |g| g.hash != hash);
            if new_group {
                if let (Some(g), Some(p)) = (out.last_mut(), picked) {
                    g.keeper = p;
                }
                picked = None;
                out.push(DupGroup { hash, keeper: photo.id, photos: vec![photo.clone()] });
            } else if let Some(g) = out.last_mut() {
                g.photos.push(photo.clone());
            }
            if pick && picked.is_none() {
                picked = Some(photo.id);
            }
        }
        if let (Some(g), Some(p)) = (out.last_mut(), picked) {
            g.keeper = p;
        }
        let waste = |g: &DupGroup| g.photos.iter().map(|p| p.size).max().unwrap_or(0) * (g.photos.len() as i64 - 1);
        out.sort_by(|x, y| waste(y).cmp(&waste(x)).then_with(|| x.hash.cmp(&y.hash)));
        Ok(out)
    }

    /// Make the `dup` flags match the sets of identical files: in each set every copy but the keeper is hidden from
    /// Browse, and everything else is shown. Returns how many items changed.
    pub fn refresh_dup_flags(&self) -> Result<usize> {
        let want: HashSet<PhotoId> = self
            .duplicate_groups()?
            .into_iter()
            .flat_map(|g| {
                let keeper = g.keeper;
                g.photos.into_iter().map(|p| p.id).filter(move |id| *id != keeper)
            })
            .collect();
        let mut c = self.conn();
        let have: HashSet<PhotoId> = {
            let mut s = c.prepare("SELECT id FROM photos WHERE dup = 1").map_err(db_err)?;
            let rows = s.query_map([], |r| r.get(0)).map_err(db_err)?.collect::<std::result::Result<HashSet<_>, _>>().map_err(db_err)?;
            rows
        };
        let hide: Vec<PhotoId> = want.difference(&have).copied().collect();
        let show: Vec<PhotoId> = have.difference(&want).copied().collect();
        if hide.is_empty() && show.is_empty() {
            return Ok(0);
        }
        let tx = c.transaction().map_err(db_err)?;
        for id in &hide {
            tx.execute("UPDATE photos SET dup = 1 WHERE id = ?1", params![id]).map_err(db_err)?;
        }
        for id in &show {
            tx.execute("UPDATE photos SET dup = 0 WHERE id = ?1", params![id]).map_err(db_err)?;
        }
        tx.commit().map_err(db_err)?;
        Ok(hide.len() + show.len())
    }

    /// Show this copy in Browse for its set of identical files (and keep it when the others are removed).
    pub fn set_keep_pick(&self, id: PhotoId) -> Result<()> {
        {
            let c = self.conn();
            c.execute(
                "UPDATE photos SET keep_pick = 0 WHERE hash IS NOT NULL AND hash = (SELECT hash FROM photos WHERE id = ?1)",
                params![id],
            )
            .map_err(db_err)?;
            c.execute("UPDATE photos SET keep_pick = 1 WHERE id = ?1 AND hash IS NOT NULL", params![id]).map_err(db_err)?;
        }
        self.refresh_dup_flags()?;
        Ok(())
    }

    pub fn dup_status(&self) -> Result<DupStatus> {
        let c = self.conn();
        let total: i64 = c.query_row("SELECT count(*) FROM photos WHERE missing = 0", [], |r| r.get(0)).map_err(db_err)?;
        // Still to do: no quick key yet, or a shared quick key without a full hash.
        let todo: i64 = c
            .query_row(
                // The shared quick keys are found once, as a group: asking per photo ("does another photo share this
                // key?") makes SQLite compare every photo with every other, which took seconds.
                "SELECT count(*) FROM photos p WHERE p.missing = 0 AND (p.quick_key IS NULL OR (
                    p.hash IS NULL AND substr(p.quick_key, 1, 1) != '!'
                    AND p.quick_key IN (SELECT quick_key FROM photos WHERE missing = 0 AND quick_key IS NOT NULL GROUP BY quick_key HAVING count(*) > 1)))",
                [],
                |r| r.get(0),
            )
            .map_err(db_err)?;
        let (groups, extra_files, extra_bytes): (i64, i64, i64) = c
            .query_row(
                "SELECT count(*), coalesce(sum(n - 1), 0), coalesce(sum((n - 1) * sz), 0) FROM (
                    SELECT count(*) AS n, max(size) AS sz FROM photos WHERE missing = 0 AND hash IS NOT NULL GROUP BY hash HAVING count(*) > 1)",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .map_err(db_err)?;
        let hidden: i64 = c.query_row("SELECT count(*) FROM photos WHERE missing = 0 AND dup = 1", [], |r| r.get(0)).map_err(db_err)?;
        Ok(DupStatus { done: total - todo, total, groups, extra_files, extra_bytes, hidden, paused: false })
    }

    /// Of `ids`, those that may be removed: live, hashed, and with another live copy that is not itself in `ids`.
    /// A whole group can never be removed this way, however the request is built.
    pub fn removable_copies(&self, ids: &[PhotoId]) -> Result<Vec<Photo>> {
        let c = self.conn();
        let mut out = Vec::new();
        for id in ids {
            let p: Option<Photo> = c
                .query_row(&format!("SELECT {PHOTO_COLS} FROM photos WHERE id = ?1 AND missing = 0 AND hash IS NOT NULL"), params![id], row_to_photo)
                .optional()
                .map_err(db_err)?;
            let Some(p) = p else { continue };
            let mut s = c.prepare("SELECT id FROM photos WHERE hash = (SELECT hash FROM photos WHERE id = ?1) AND hash IS NOT NULL AND missing = 0 AND id != ?1").map_err(db_err)?;
            let others: Vec<PhotoId> = s.query_map(params![id], |r| r.get(0)).map_err(db_err)?.collect::<std::result::Result<Vec<_>, _>>().map_err(db_err)?;
            if others.iter().any(|o| !ids.contains(o)) {
                out.push(p);
            }
        }
        Ok(out)
    }

    /// An item the user removed (to the Recycle Bin): hidden like any missing file.
    pub fn mark_removed(&self, id: PhotoId) -> Result<()> {
        self.conn().execute("UPDATE photos SET missing = 1 WHERE id = ?1", params![id]).map_err(db_err)?;
        Ok(())
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

    /// Store the embedding of one photo for a model version (replacing any earlier one).
    pub fn put_embedding(&self, photo_id: PhotoId, model_version: &str, vector: &[f32]) -> Result<()> {
        let blob: Vec<u8> = vector.iter().flat_map(|x| x.to_le_bytes()).collect();
        self.conn()
            .execute(
                "INSERT OR REPLACE INTO embeddings(photo_id, model_version, vector) VALUES (?1, ?2, ?3)",
                params![photo_id, model_version, blob],
            )
            .map_err(db_err)?;
        Ok(())
    }

    /// Record that a photo could not be analysed, so it is not offered again for this model version.
    pub fn put_embedding_failed(&self, photo_id: PhotoId, model_version: &str) -> Result<()> {
        self.conn()
            .execute(
                "INSERT OR REPLACE INTO embeddings(photo_id, model_version, vector) VALUES (?1, ?2, NULL)",
                params![photo_id, model_version],
            )
            .map_err(db_err)?;
        Ok(())
    }

    /// Photos (not videos, not missing) with no embedding row for this model version, newest id first.
    pub fn photos_needing_embedding(&self, model_version: &str, limit: i64) -> Result<Vec<Photo>> {
        let c = self.conn();
        let mut s = c
            .prepare(&format!(
                "SELECT {PHOTO_COLS} FROM photos p WHERE missing = 0 AND kind = 0
                   AND NOT EXISTS (SELECT 1 FROM embeddings e WHERE e.photo_id = p.id AND e.model_version = ?1)
                 ORDER BY id DESC LIMIT ?2"
            ))
            .map_err(db_err)?;
        let rows = s.query_map(params![model_version, limit], row_to_photo).map_err(db_err)?;
        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(db_err)
    }

    /// (photos with a row for this model version, photos that should have one).
    pub fn embedding_counts(&self, model_version: &str) -> Result<(i64, i64)> {
        let c = self.conn();
        let total: i64 = c.query_row("SELECT COUNT(*) FROM photos WHERE missing = 0 AND kind = 0", [], |r| r.get(0)).map_err(db_err)?;
        let done: i64 = c
            .query_row(
                "SELECT COUNT(*) FROM embeddings e JOIN photos p ON p.id = e.photo_id
                  WHERE e.model_version = ?1 AND p.missing = 0 AND p.kind = 0",
                params![model_version],
                |r| r.get(0),
            )
            .map_err(db_err)?;
        Ok((done, total))
    }

    /// Every stored vector for this model version (failed markers excluded) of photos that are not missing.
    pub fn load_embeddings(&self, model_version: &str) -> Result<Vec<(PhotoId, Vec<f32>)>> {
        let c = self.conn();
        let mut s = c
            .prepare(
                "SELECT e.photo_id, e.vector FROM embeddings e JOIN photos p ON p.id = e.photo_id
                  WHERE e.model_version = ?1 AND e.vector IS NOT NULL AND p.missing = 0",
            )
            .map_err(db_err)?;
        let rows = s
            .query_map(params![model_version], |r| {
                let blob: Vec<u8> = r.get(1)?;
                let v = blob.chunks_exact(4).map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]])).collect();
                Ok((r.get(0)?, v))
            })
            .map_err(db_err)?;
        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(db_err)
    }

    /// Make these photos the text-search result. Together with the tag filter (if any) they are what
    /// `PhotoQuery::search` restricts listings to: a photo must be in every active set.
    pub fn set_search_hits(&self, ids: &[PhotoId]) -> Result<()> {
        let mut c = self.conn();
        let tx = c.transaction().map_err(db_err)?;
        tx.execute("DELETE FROM text_hits", []).map_err(db_err)?;
        {
            let mut ins = tx.prepare("INSERT OR IGNORE INTO text_hits(photo_id) VALUES (?1)").map_err(db_err)?;
            for id in ids {
                ins.execute(params![id]).map_err(db_err)?;
            }
        }
        tx.execute("INSERT OR IGNORE INTO filter_state(name) VALUES ('text')", []).map_err(db_err)?;
        refresh_hits(&tx)?;
        tx.commit().map_err(db_err)?;
        Ok(())
    }

    /// The text search is off (the tag filter, if any, stays).
    pub fn clear_search_hits(&self) -> Result<()> {
        let mut c = self.conn();
        let tx = c.transaction().map_err(db_err)?;
        tx.execute("DELETE FROM text_hits", []).map_err(db_err)?;
        tx.execute("DELETE FROM filter_state WHERE name = 'text'", []).map_err(db_err)?;
        refresh_hits(&tx)?;
        tx.commit().map_err(db_err)?;
        Ok(())
    }

    /// The tag and the person the listings are really restricted to (the UI draws its chips from these).
    pub fn active_filters(&self) -> Result<(Option<String>, Option<i64>)> {
        let c = self.conn();
        let value = |name: &str| -> Result<Option<String>> {
            c.query_row("SELECT value FROM filter_state WHERE name = ?1", params![name], |r| r.get::<_, Option<String>>(0)).optional().map_err(db_err).map(Option::flatten)
        };
        Ok((value("tag")?, value("person")?.and_then(|v| v.parse().ok())))
    }

    /// Show only photos with this tag; `None` turns the tag filter off.
    pub fn set_tag_filter(&self, tag: Option<&str>) -> Result<()> {
        let mut c = self.conn();
        let tx = c.transaction().map_err(db_err)?;
        tx.execute("DELETE FROM tag_hits", []).map_err(db_err)?;
        match tag {
            Some(t) => {
                tx.execute("INSERT INTO tag_hits(photo_id) SELECT photo_id FROM photo_tags WHERE tag = ?1", params![t]).map_err(db_err)?;
                tx.execute("INSERT OR REPLACE INTO filter_state(name, value) VALUES ('tag', ?1)", params![t]).map_err(db_err)?;
            }
            None => {
                tx.execute("DELETE FROM filter_state WHERE name = 'tag'", []).map_err(db_err)?;
            }
        }
        refresh_hits(&tx)?;
        tx.commit().map_err(db_err)?;
        Ok(())
    }

    // ---- faces and people ----

    /// Photos (not videos, not missing) not yet scanned for faces with this version, newest id first.
    pub fn photos_needing_faces(&self, version: &str, limit: i64) -> Result<Vec<Photo>> {
        let c = self.conn();
        let mut s = c
            .prepare(&format!(
                "SELECT {PHOTO_COLS} FROM photos p WHERE missing = 0 AND dup = 0 AND kind = 0
                   AND NOT EXISTS (SELECT 1 FROM face_scans s WHERE s.photo_id = p.id AND s.version = ?1)
                 ORDER BY id DESC LIMIT ?2"
            ))
            .map_err(db_err)?;
        let rows = s.query_map(params![version, limit], row_to_photo).map_err(db_err)?;
        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(db_err)
    }

    /// (photos scanned with this version, photos that should be). A hidden extra copy of a duplicate is neither: the
    /// copy Browse shows is scanned instead.
    pub fn face_counts(&self, version: &str) -> Result<(i64, i64)> {
        let c = self.conn();
        let total: i64 = c.query_row("SELECT COUNT(*) FROM photos WHERE missing = 0 AND dup = 0 AND kind = 0", [], |r| r.get(0)).map_err(db_err)?;
        let done: i64 = c
            .query_row(
                "SELECT COUNT(*) FROM face_scans s JOIN photos p ON p.id = s.photo_id
                  WHERE s.version = ?1 AND p.missing = 0 AND p.dup = 0 AND p.kind = 0",
                params![version],
                |r| r.get(0),
            )
            .map_err(db_err)?;
        Ok((done, total))
    }

    /// Store the faces found in a photo (replacing any it had) and mark the photo scanned. Returns the new face ids.
    pub fn replace_faces(&self, photo_id: PhotoId, version: &str, faces: &[NewFace]) -> Result<Vec<i64>> {
        let mut c = self.conn();
        let tx = c.transaction().map_err(db_err)?;
        let old: Vec<i64> = {
            let mut st = tx.prepare("SELECT DISTINCT person_id FROM faces WHERE photo_id = ?1 AND person_id IS NOT NULL").map_err(db_err)?;
            let rows = st.query_map(params![photo_id], |r| r.get(0)).map_err(db_err)?;
            rows.collect::<rusqlite::Result<Vec<_>>>().map_err(db_err)?
        };
        tx.execute("DELETE FROM faces WHERE photo_id = ?1", params![photo_id]).map_err(db_err)?;
        for p in old {
            recompute_person(&tx, p)?;
        }
        prune_people(&tx)?;
        let mut ids = Vec::with_capacity(faces.len());
        for f in faces {
            tx.execute(
                "INSERT INTO faces(photo_id, x, y, w, h, score, embedding) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![photo_id, f.x, f.y, f.w, f.h, f.score, blob_of(&f.embedding)],
            )
            .map_err(db_err)?;
            ids.push(tx.last_insert_rowid());
        }
        tx.execute("INSERT OR REPLACE INTO face_scans(photo_id, version) VALUES (?1, ?2)", params![photo_id, version]).map_err(db_err)?;
        tx.commit().map_err(db_err)?;
        Ok(ids)
    }

    /// For each face, the people it was rejected from: it is never grouped with them.
    pub fn face_rejections(&self) -> Result<HashMap<i64, Vec<i64>>> {
        let c = self.conn();
        let mut s = c.prepare("SELECT face_id, person_id FROM face_rejections ORDER BY face_id, person_id").map_err(db_err)?;
        let rows = s.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?))).map_err(db_err)?;
        let mut out: HashMap<i64, Vec<i64>> = HashMap::new();
        for row in rows {
            let (face, person) = row.map_err(db_err)?;
            out.entry(face).or_default().push(person);
        }
        Ok(out)
    }

    /// Faces not yet grouped: no person, and not parked in the rejects. Oldest first.
    pub fn unassigned_faces(&self) -> Result<Vec<(i64, Vec<f32>)>> {
        let c = self.conn();
        let mut s = c.prepare("SELECT id, embedding FROM faces WHERE person_id IS NULL AND pinned = 0 ORDER BY id").map_err(db_err)?;
        let rows = s.query_map([], |r| Ok((r.get(0)?, vector_of(&r.get::<_, Vec<u8>>(1)?)))).map_err(db_err)?;
        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(db_err)
    }

    /// Every person (hidden ones too: a face still joins them rather than starting a new person) as (id, centroid, faces).
    pub fn people_centroids(&self) -> Result<Vec<(i64, Vec<f32>, i64)>> {
        let c = self.conn();
        let mut s = c.prepare("SELECT id, centroid, n FROM people ORDER BY id").map_err(db_err)?;
        let rows = s.query_map([], |r| Ok((r.get(0)?, vector_of(&r.get::<_, Vec<u8>>(1)?), r.get(2)?))).map_err(db_err)?;
        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(db_err)
    }

    /// A new, unnamed person with no faces yet.
    pub fn create_person(&self, centroid: &[f32]) -> Result<i64> {
        let c = self.conn();
        c.execute("INSERT INTO people(centroid, n) VALUES (?1, 0)", params![blob_of(centroid)]).map_err(db_err)?;
        Ok(c.last_insert_rowid())
    }

    /// Put a face in a person and store that person's updated centroid and face count.
    pub fn assign_face(&self, face_id: i64, person_id: i64, centroid: &[f32], n: i64) -> Result<()> {
        let mut c = self.conn();
        let tx = c.transaction().map_err(db_err)?;
        tx.execute("UPDATE faces SET person_id = ?2 WHERE id = ?1", params![face_id, person_id]).map_err(db_err)?;
        tx.execute("UPDATE people SET centroid = ?2, n = ?3 WHERE id = ?1", params![person_id, blob_of(centroid), n]).map_err(db_err)?;
        tx.commit().map_err(db_err)?;
        Ok(())
    }

    /// The people to list in the nav: not hidden, in at least one visible photo, and either named or in two or more
    /// photos (a face seen once is mostly noise). Named people first, then the most photos.
    pub fn list_people(&self) -> Result<Vec<PersonInfo>> {
        let c = self.conn();
        let mut s = c
            .prepare(
                "SELECT p.id, p.name, COUNT(DISTINCT f.photo_id) AS cnt,
                        (SELECT f2.id FROM faces f2 JOIN photos h2 ON h2.id = f2.photo_id
                          WHERE f2.person_id = p.id AND h2.missing = 0 AND h2.dup = 0
                          ORDER BY f2.score * f2.w * f2.h DESC, f2.id LIMIT 1)
                   FROM people p JOIN faces f ON f.person_id = p.id JOIN photos h ON h.id = f.photo_id AND h.missing = 0 AND h.dup = 0
                  WHERE p.hidden = 0
                  GROUP BY p.id
                 HAVING p.name IS NOT NULL
                  ORDER BY cnt DESC, p.id",
            )
            .map_err(db_err)?;
        let rows = s.query_map([], |r| Ok(PersonInfo { id: r.get(0)?, name: r.get(1)?, count: r.get(2)?, face_id: r.get(3)? })).map_err(db_err)?;
        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(db_err)
    }

    /// The faces in one photo.
    pub fn faces_of(&self, photo_id: PhotoId) -> Result<Vec<FaceInfo>> {
        let c = self.conn();
        let mut s = c
            .prepare(
                "SELECT f.id, f.person_id, p.name, f.x, f.y, f.w, f.h FROM faces f LEFT JOIN people p ON p.id = f.person_id
                  WHERE f.photo_id = ?1 ORDER BY f.id",
            )
            .map_err(db_err)?;
        let rows = s
            .query_map(params![photo_id], |r| {
                Ok(FaceInfo { id: r.get(0)?, person_id: r.get(1)?, person_name: r.get(2)?, x: r.get(3)?, y: r.get(4)?, w: r.get(5)?, h: r.get(6)? })
            })
            .map_err(db_err)?;
        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(db_err)
    }

    /// The photo a face is in and its `[x, y, w, h]` box (fractions of the upright image).
    pub fn face_location(&self, face_id: i64) -> Result<Option<(Photo, [f32; 4])>> {
        let found: Option<(PhotoId, [f32; 4])> = {
            let c = self.conn();
            c.query_row("SELECT photo_id, x, y, w, h FROM faces WHERE id = ?1", params![face_id], |r| {
                Ok((r.get(0)?, [r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?]))
            })
            .optional()
            .map_err(db_err)?
        };
        match found {
            Some((pid, b)) => Ok(self.get_photo(pid)?.map(|p| (p, b))),
            None => Ok(None),
        }
    }

    /// Everyone for the Faces tab, most faces first: counts over visible photos and up to three best faces each.
    /// Hidden people only when `include_hidden`.
    pub fn list_face_people(&self, include_hidden: bool) -> Result<Vec<FacePerson>> {
        let c = self.conn();
        let mut s = c
            .prepare(
                "SELECT p.id, p.name, p.hidden, COUNT(DISTINCT f.photo_id), COUNT(f.id)
                   FROM people p JOIN faces f ON f.person_id = p.id JOIN photos h ON h.id = f.photo_id AND h.missing = 0 AND h.dup = 0
                  WHERE (?1 OR p.hidden = 0)
                  GROUP BY p.id ORDER BY COUNT(f.id) DESC, p.id",
            )
            .map_err(db_err)?;
        let rows: Vec<(i64, Option<String>, bool, i64, i64)> = s
            .query_map(params![include_hidden], |r| Ok((r.get(0)?, r.get(1)?, r.get::<_, i64>(2)? != 0, r.get(3)?, r.get(4)?)))
            .map_err(db_err)?
            .collect::<rusqlite::Result<_>>()
            .map_err(db_err)?;
        let mut samples = c
            .prepare(
                "SELECT f.id FROM faces f JOIN photos h ON h.id = f.photo_id AND h.missing = 0 AND h.dup = 0
                  WHERE f.person_id = ?1 ORDER BY f.score * f.w * f.h DESC, f.id LIMIT 3",
            )
            .map_err(db_err)?;
        let mut out = Vec::with_capacity(rows.len());
        for (id, name, hidden, photos, faces) in rows {
            let samples = samples.query_map(params![id], |r| r.get(0)).map_err(db_err)?.collect::<rusqlite::Result<Vec<i64>>>().map_err(db_err)?;
            let listed = !hidden && name.is_some();
            out.push(FacePerson { id, name, hidden, listed, photos, faces, samples });
        }
        Ok(out)
    }

    /// A person's faces in visible photos: the best (largest, most certain) first, or with `doubtful_first` the ones
    /// least like the rest of the person first, which is where the wrongly grouped faces are.
    pub fn faces_of_person(&self, person_id: i64, limit: i64, doubtful_first: bool) -> Result<Vec<FaceTile>> {
        let c = self.conn();
        if !doubtful_first {
            let mut s = c
                .prepare(
                    "SELECT f.id, f.photo_id, h.mtime, h.size, f.x, f.y, f.w, f.h FROM faces f JOIN photos h ON h.id = f.photo_id AND h.missing = 0 AND h.dup = 0
                      WHERE f.person_id = ?1 ORDER BY f.score * f.w * f.h DESC, f.id LIMIT ?2",
                )
                .map_err(db_err)?;
            let rows = s.query_map(params![person_id, limit], row_to_tile).map_err(db_err)?;
            return rows.collect::<rusqlite::Result<Vec<_>>>().map_err(db_err);
        }
        let centroid: Vec<f32> = c
            .query_row("SELECT centroid FROM people WHERE id = ?1", params![person_id], |r| r.get::<_, Vec<u8>>(0))
            .optional()
            .map_err(db_err)?
            .map(|b| vector_of(&b))
            .unwrap_or_default();
        let mut s = c
            .prepare(
                "SELECT f.id, f.photo_id, h.mtime, h.size, f.x, f.y, f.w, f.h, f.embedding FROM faces f JOIN photos h ON h.id = f.photo_id AND h.missing = 0 AND h.dup = 0
                  WHERE f.person_id = ?1",
            )
            .map_err(db_err)?;
        let rows = s
            .query_map(params![person_id], |r| {
                let sim: f32 = vector_of(&r.get::<_, Vec<u8>>(8)?).iter().zip(&centroid).map(|(a, b)| a * b).sum();
                Ok((sim, row_to_tile(r)?))
            })
            .map_err(db_err)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(db_err)?;
        let mut rows = rows;
        rows.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.id.cmp(&b.1.id)));
        Ok(rows.into_iter().take(limit.max(0) as usize).map(|(_, t)| t).collect())
    }

    /// Faces marked "not this person" that have not been given to anyone since, best first.
    pub fn rejected_faces(&self, limit: i64) -> Result<Vec<FaceTile>> {
        let c = self.conn();
        let mut s = c
            .prepare(
                "SELECT f.id, f.photo_id, h.mtime, h.size, f.x, f.y, f.w, f.h FROM faces f JOIN photos h ON h.id = f.photo_id AND h.missing = 0 AND h.dup = 0
                  WHERE f.person_id IS NULL AND f.pinned = 1 ORDER BY f.score * f.w * f.h DESC, f.id LIMIT ?1",
            )
            .map_err(db_err)?;
        let rows = s.query_map(params![limit], row_to_tile).map_err(db_err)?;
        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(db_err)
    }

    /// Park a face nobody matched in the "not this person" rejects, until the user says who it is.
    pub fn park_face(&self, face_id: i64) -> Result<()> {
        self.conn().execute("UPDATE faces SET pinned = 1 WHERE id = ?1 AND person_id IS NULL", params![face_id]).map_err(db_err)?;
        Ok(())
    }

    /// Give a face to a person (from another person, or from "not this person").
    pub fn move_face(&self, face_id: i64, person_id: i64) -> Result<()> {
        let mut c = self.conn();
        let tx = c.transaction().map_err(db_err)?;
        move_face_in(&tx, face_id, person_id)?;
        tx.commit().map_err(db_err)?;
        Ok(())
    }

    /// Start a new person from this face (optionally named) and return their id.
    pub fn new_person_from_face(&self, face_id: i64, name: Option<&str>) -> Result<i64> {
        let name = name.map(str::trim).filter(|n| !n.is_empty());
        let mut c = self.conn();
        let tx = c.transaction().map_err(db_err)?;
        let embedding: Vec<u8> = tx.query_row("SELECT embedding FROM faces WHERE id = ?1", params![face_id], |r| r.get(0)).map_err(db_err)?;
        tx.execute("INSERT INTO people(name, centroid, n) VALUES (?1, ?2, 0)", params![name, embedding]).map_err(db_err)?;
        let id = tx.last_insert_rowid();
        move_face_in(&tx, face_id, id)?;
        tx.commit().map_err(db_err)?;
        Ok(id)
    }

    /// Bring a hidden person back into the nav.
    pub fn unhide_person(&self, id: i64) -> Result<()> {
        self.conn().execute("UPDATE people SET hidden = 0 WHERE id = ?1", params![id]).map_err(db_err)?;
        Ok(())
    }

    /// Name a person; `None` (or blank) clears the name.
    pub fn rename_person(&self, id: i64, name: Option<&str>) -> Result<()> {
        let name = name.map(str::trim).filter(|n| !n.is_empty());
        self.conn().execute("UPDATE people SET name = ?2 WHERE id = ?1", params![id, name]).map_err(db_err)?;
        Ok(())
    }

    /// Leave a person out of the nav. Their faces stay theirs, so they are not grouped into a new person.
    pub fn hide_person(&self, id: i64) -> Result<()> {
        self.conn().execute("UPDATE people SET hidden = 1 WHERE id = ?1", params![id]).map_err(db_err)?;
        Ok(())
    }

    /// Fold `from` into `into`: its faces move, the centroid is recomputed, an unnamed `into` takes `from`'s name.
    pub fn merge_people(&self, from: i64, into: i64) -> Result<()> {
        if from == into {
            return Ok(());
        }
        let mut c = self.conn();
        let tx = c.transaction().map_err(db_err)?;
        tx.execute("UPDATE faces SET person_id = ?2 WHERE person_id = ?1", params![from, into]).map_err(db_err)?;
        tx.execute("UPDATE people SET name = (SELECT name FROM people WHERE id = ?1) WHERE id = ?2 AND name IS NULL", params![from, into]).map_err(db_err)?;
        // what was rejected as `from` is now rejected as `into` (rows that would clash go with `from`)
        tx.execute("UPDATE OR IGNORE face_rejections SET person_id = ?2 WHERE person_id = ?1", params![from, into]).map_err(db_err)?;
        tx.execute("DELETE FROM people WHERE id = ?1", params![from]).map_err(db_err)?;
        recompute_person(&tx, into)?;
        tx.commit().map_err(db_err)?;
        Ok(())
    }

    /// "Not this person": the face leaves its person and is never grouped automatically again.
    pub fn reject_face(&self, face_id: i64) -> Result<()> {
        self.reject_faces(&[face_id])
    }

    /// `reject_face` for several faces at once.
    pub fn reject_faces(&self, face_ids: &[i64]) -> Result<()> {
        let mut c = self.conn();
        let tx = c.transaction().map_err(db_err)?;
        for id in face_ids {
            reject_face_in(&tx, *id)?;
        }
        tx.commit().map_err(db_err)?;
        Ok(())
    }

    /// `move_face` for several faces at once.
    pub fn move_faces(&self, face_ids: &[i64], person_id: i64) -> Result<()> {
        let mut c = self.conn();
        let tx = c.transaction().map_err(db_err)?;
        for id in face_ids {
            move_face_in(&tx, *id, person_id)?;
        }
        tx.commit().map_err(db_err)?;
        Ok(())
    }

    /// Show only photos this person is in; `None` turns the person filter off.
    pub fn set_person_filter(&self, person: Option<i64>) -> Result<()> {
        let mut c = self.conn();
        let tx = c.transaction().map_err(db_err)?;
        tx.execute("DELETE FROM person_hits", []).map_err(db_err)?;
        match person {
            Some(p) => {
                tx.execute("INSERT INTO person_hits(photo_id) SELECT DISTINCT photo_id FROM faces WHERE person_id = ?1", params![p]).map_err(db_err)?;
                tx.execute("INSERT OR REPLACE INTO filter_state(name, value) VALUES ('person', ?1)", params![p.to_string()]).map_err(db_err)?;
            }
            None => {
                tx.execute("DELETE FROM filter_state WHERE name = 'person'", []).map_err(db_err)?;
            }
        }
        refresh_hits(&tx)?;
        tx.commit().map_err(db_err)?;
        Ok(())
    }

    /// Replace a photo's tags (best first by score) and record the vocabulary that produced them.
    pub fn set_tags(&self, photo_id: PhotoId, model_version: &str, tags_version: &str, tags: &[(String, f32)]) -> Result<()> {
        let mut c = self.conn();
        let tx = c.transaction().map_err(db_err)?;
        tx.execute("DELETE FROM photo_tags WHERE photo_id = ?1", params![photo_id]).map_err(db_err)?;
        {
            let mut ins = tx.prepare("INSERT INTO photo_tags(photo_id, tag, score) VALUES (?1, ?2, ?3)").map_err(db_err)?;
            for (tag, score) in tags {
                ins.execute(params![photo_id, tag, score]).map_err(db_err)?;
            }
        }
        tx.execute(
            "UPDATE embeddings SET tags_version = ?3 WHERE photo_id = ?1 AND model_version = ?2",
            params![photo_id, model_version, tags_version],
        )
        .map_err(db_err)?;
        tx.commit().map_err(db_err)?;
        Ok(())
    }

    /// Stored vectors (of photos that are not missing) not yet tagged with this vocabulary version.
    pub fn embeddings_needing_tags(&self, model_version: &str, tags_version: &str, limit: i64) -> Result<Vec<(PhotoId, Vec<f32>)>> {
        let c = self.conn();
        let mut s = c
            .prepare(
                "SELECT e.photo_id, e.vector FROM embeddings e JOIN photos p ON p.id = e.photo_id
                  WHERE e.model_version = ?1 AND e.vector IS NOT NULL AND p.missing = 0
                    AND (e.tags_version IS NULL OR e.tags_version != ?2)
                  LIMIT ?3",
            )
            .map_err(db_err)?;
        let rows = s
            .query_map(params![model_version, tags_version, limit], |r| {
                let blob: Vec<u8> = r.get(1)?;
                Ok((r.get(0)?, blob.chunks_exact(4).map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]])).collect()))
            })
            .map_err(db_err)?;
        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(db_err)
    }

    /// A photo's tags, best first.
    pub fn tags_for(&self, photo_id: PhotoId) -> Result<Vec<(String, f32)>> {
        let c = self.conn();
        let mut s = c.prepare("SELECT tag, score FROM photo_tags WHERE photo_id = ?1 ORDER BY score DESC, tag").map_err(db_err)?;
        let rows = s.query_map(params![photo_id], |r| Ok((r.get(0)?, r.get::<_, f64>(1)? as f32))).map_err(db_err)?;
        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(db_err)
    }

    /// How many different photos carry at least one of these tags.
    pub fn photos_with_any_tag(&self, tags: &[&str]) -> Result<i64> {
        if tags.is_empty() {
            return Ok(0);
        }
        let marks = vec!["?"; tags.len()].join(",");
        let c = self.conn();
        c.query_row(
            &format!(
                "SELECT COUNT(DISTINCT t.photo_id) FROM photo_tags t JOIN photos p ON p.id = t.photo_id
                  WHERE p.missing = 0 AND p.dup = 0 AND t.tag IN ({marks})"
            ),
            rusqlite::params_from_iter(tags.iter()),
            |r| r.get(0),
        )
        .map_err(db_err)
    }

    /// Every tag in use with how many photos carry it, most common first.
    pub fn tag_counts(&self) -> Result<Vec<(String, i64)>> {
        let c = self.conn();
        let mut s = c
            .prepare(
                "SELECT t.tag, COUNT(*) AS n FROM photo_tags t JOIN photos p ON p.id = t.photo_id
                  WHERE p.missing = 0 AND p.dup = 0 GROUP BY t.tag ORDER BY n DESC, t.tag",
            )
            .map_err(db_err)?;
        let rows = s.query_map([], |r| Ok((r.get(0)?, r.get(1)?))).map_err(db_err)?;
        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(db_err)
    }

    /// The highest photo id, or 0 for an empty library. Pass it as `PhotoQuery::max_id` to freeze a listing:
    /// items added later (by a running scan) have higher ids and do not shift any position.
    pub fn max_photo_id(&self) -> Result<i64> {
        self.conn().query_row("SELECT COALESCE(MAX(id), 0) FROM photos", [], |r| r.get(0)).map_err(db_err)
    }

    /// Every non-missing item with a location, for the map. Honours the folder, kind, since and until filters.
    pub fn list_geo_points(&self, folder_id: Option<FolderId>, kind: Option<MediaKind>, since: Option<i64>, until: Option<i64>) -> Result<Vec<GeoPoint>> {
        self.list_geo_points_in(&PhotoQuery { folder_id, kind, since, until, ..Default::default() })
    }

    /// Located items matching the query's folder, kind, date and search filters (position and order are ignored).
    pub fn list_geo_points_in(&self, q: &PhotoQuery) -> Result<Vec<GeoPoint>> {
        let filter = Filter::of(q);
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

    /// Every item with a real date (not a placeholder such as 1980, and not from the future), for finding trips.
    pub fn trip_items(&self, now: i64) -> Result<Vec<DayItem>> {
        let c = self.conn();
        let mut s = c
            .prepare("SELECT taken_at, gps_lat, gps_lon, kind FROM photos WHERE missing = 0 AND dup = 0 AND taken_at >= ?1 AND taken_at < ?2")
            .map_err(db_err)?;
        let rows = s
            .query_map(params![EARLIEST_REAL, now + 86_400], |r| {
                let (lat, lon): (Option<f64>, Option<f64>) = (r.get(1)?, r.get(2)?);
                Ok(DayItem { day: day_of(r.get(0)?), pos: lat.zip(lon), video: r.get::<_, i64>(3)? == 1 })
            })
            .map_err(db_err)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(db_err)?;
        Ok(rows)
    }

    /// The trips for these settings, newest first, with the names the user gave them.
    pub fn trips(&self, settings: &TripSettings, now: i64) -> Result<Vec<Trip>> {
        let mut trips = compute_trips(&self.trip_items(now)?, settings);
        let names: Vec<(i64, String)> = {
            let c = self.conn();
            let mut s = c.prepare("SELECT start_day, name FROM trip_names ORDER BY start_day").map_err(db_err)?;
            let rows = s
                .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
                .map_err(db_err)?
                .collect::<std::result::Result<Vec<_>, _>>()
                .map_err(db_err)?;
            rows
        };
        for t in &mut trips {
            let (first, last) = (day_of(t.since), day_of(t.until) - 1);
            t.name = names.iter().find(|(d, _)| *d >= first && *d <= last).map(|(_, n)| n.clone());
        }
        Ok(trips)
    }

    /// Where a trip went: the places its located items were taken, leaving out anything within the home radius (a day
    /// away can also hold a few photos taken at home). `since` is inclusive and `until` exclusive, as `trips` reports.
    pub fn trip_route(&self, since: i64, until: i64, settings: &TripSettings, stop_km: f64) -> Result<Route> {
        let rows: Vec<(i64, f64, f64)> = {
            let c = self.conn();
            let mut s = c
                .prepare(
                    "SELECT taken_at, gps_lat, gps_lon FROM photos
                      WHERE missing = 0 AND dup = 0 AND taken_at >= ?1 AND taken_at < ?2 AND gps_lat IS NOT NULL AND gps_lon IS NOT NULL
                      ORDER BY taken_at, id",
                )
                .map_err(db_err)?;
            let rows = s
                .query_map(params![since.max(EARLIEST_REAL), until], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
                .map_err(db_err)?
                .collect::<std::result::Result<Vec<_>, _>>()
                .map_err(db_err)?;
            rows
        };
        let away: Vec<(i64, f64, f64)> =
            rows.into_iter().filter(|&(_, la, lo)| distance_km((settings.home_lat, settings.home_lon), (la, lo)) > settings.radius_km).collect();
        Ok(compute_route(&away, stop_km))
    }

    /// Where home probably is, from where the photos were taken. None when no item has a position.
    pub fn suggest_home(&self, now: i64) -> Result<Option<(f64, f64)>> {
        Ok(infer_home(&self.trip_items(now)?))
    }

    /// Names the trip covering `since` (inclusive) to `until` (exclusive), replacing any name it had; an empty name
    /// removes the name.
    pub fn rename_trip(&self, since: i64, until: i64, name: &str) -> Result<()> {
        let (first, last) = (day_of(since), day_of(until) - 1);
        let name = name.trim();
        let c = self.conn();
        c.execute("DELETE FROM trip_names WHERE start_day BETWEEN ?1 AND ?2", params![first, last]).map_err(db_err)?;
        if !name.is_empty() {
            c.execute("INSERT INTO trip_names (start_day, name) VALUES (?1, ?2)", params![first, name]).map_err(db_err)?;
        }
        Ok(())
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
            let q = PhotoQuery { order, folder_id, kind: Some(MediaKind::Video), bounds: None, max_id: None, since: None, until: None, search: false };
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

    /// A library for trip tests: items at (day number, position) with the day number turned into a time of day 12:00.
    fn trip_library(items: &[(i64, Option<(f64, f64)>)]) -> Db {
        let db = Db::open_in_memory().unwrap();
        let f = db.add_folder("/p").unwrap();
        for (i, (day, pos)) in items.iter().enumerate() {
            let taken = day * 86_400 + 43_200;
            let m = match pos {
                Some((la, lo)) => geo_meta(taken, *la, *lo),
                None => meta(Some(taken)),
            };
            db.upsert_photo(f, &format!("/p/{i}.jpg"), 1, 1, &m).unwrap();
        }
        db
    }

    const T_PARIS: (f64, f64) = (48.86, 2.35);
    const T_ROME: (f64, f64) = (41.90, 12.50);
    const T_NOW: i64 = 20_000 * 86_400;
    fn t_settings() -> TripSettings {
        TripSettings { home_lat: T_PARIS.0, home_lon: T_PARIS.1, radius_km: 50.0, min_nights: 0 }
    }

    #[test]
    fn trips_come_from_dated_located_items_and_ignore_placeholder_dates() {
        let db = trip_library(&[(18_000, Some(T_PARIS)), (18_001, Some(T_ROME)), (18_002, Some(T_ROME)), (18_003, Some(T_PARIS))]);
        // A 1980 placeholder date in Rome, a far-future date in Rome, and a missing date must not make trips.
        let f = db.list_folders().unwrap()[0].id;
        db.upsert_photo(f, "/p/y1980.jpg", 1, 1, &geo_meta(315_532_800, T_ROME.0, T_ROME.1)).unwrap();
        db.upsert_photo(f, "/p/future.jpg", 1, 1, &geo_meta(T_NOW + 90 * 86_400, T_ROME.0, T_ROME.1)).unwrap();
        db.upsert_photo(f, "/p/undated.jpg", 1, 1, &PhotoMeta { gps_lat: Some(T_ROME.0), gps_lon: Some(T_ROME.1), ..Default::default() }).unwrap();
        let t = db.trips(&t_settings(), T_NOW).unwrap();
        assert_eq!(t.len(), 1);
        assert_eq!((t[0].since, t[0].until, t[0].days, t[0].photos), (18_001 * 86_400, 18_003 * 86_400, 2, 2));
    }

    #[test]
    fn a_trips_route_leaves_out_what_was_taken_at_home_and_what_is_not_located() {
        let db = trip_library(&[(18_000, Some(T_PARIS)), (18_001, Some(T_ROME)), (18_001, Some(T_PARIS)), (18_001, None), (18_002, Some(T_ROME)), (18_003, Some(T_PARIS))]);
        let t = &db.trips(&t_settings(), T_NOW).unwrap()[0];
        let r = db.trip_route(t.since, t.until, &t_settings(), 25.0).unwrap();
        assert_eq!(r.stops.len(), 1);
        assert_eq!((r.stops[0].items, r.stops[0].days), (2, 2));
        assert_eq!(r.path, vec![0]);
    }

    #[test]
    fn missing_files_are_not_in_trips() {
        let db = trip_library(&[(18_000, Some(T_ROME))]);
        let f = db.list_folders().unwrap()[0].id;
        db.mark_missing_except(f, &HashSet::new()).unwrap();
        assert!(db.trips(&t_settings(), T_NOW).unwrap().is_empty());
    }

    #[test]
    fn a_trips_name_is_kept_across_looks_and_can_be_changed_or_removed() {
        let db = trip_library(&[(18_000, Some(T_PARIS)), (18_001, Some(T_ROME)), (18_002, Some(T_ROME)), (18_003, Some(T_PARIS)), (18_010, Some(T_ROME))]);
        let t = db.trips(&t_settings(), T_NOW).unwrap();
        assert_eq!(t.len(), 2);
        assert!(t.iter().all(|x| x.name.is_none()));
        db.rename_trip(t[1].since, t[1].until, "  Rome  ").unwrap();
        let t = db.trips(&t_settings(), T_NOW).unwrap();
        assert_eq!((t[0].name.as_deref(), t[1].name.as_deref()), (None, Some("Rome")));
        db.rename_trip(t[1].since, t[1].until, "Rome 2019").unwrap();
        assert_eq!(db.trips(&t_settings(), T_NOW).unwrap()[1].name.as_deref(), Some("Rome 2019"));
        // A name sticks to the trip when it grows (a later away day joins it) because the start day is still inside it.
        let f = db.list_folders().unwrap()[0].id;
        db.upsert_photo(f, "/p/more.jpg", 1, 1, &geo_meta(18_002 * 86_400 + 100, T_ROME.0, T_ROME.1)).unwrap();
        assert_eq!(db.trips(&t_settings(), T_NOW).unwrap()[1].name.as_deref(), Some("Rome 2019"));
        db.rename_trip(t[1].since, t[1].until, "").unwrap();
        assert!(db.trips(&t_settings(), T_NOW).unwrap()[1].name.is_none());
    }

    #[test]
    fn home_is_suggested_from_the_photos() {
        let db = trip_library(&[(1_000_000_000 / 86_400, Some(T_PARIS)), (1_000_000_000 / 86_400 + 1, Some(T_PARIS)), (1_000_000_000 / 86_400 + 2, Some(T_ROME))]);
        let h = db.suggest_home(T_NOW).unwrap().unwrap();
        assert!(crate::trips::distance_km(h, T_PARIS) < 15.0);
        assert!(trip_library(&[(18_000, None)]).suggest_home(T_NOW).unwrap().is_none());
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
        let q = PhotoQuery { order: SortOrder::Oldest, folder_id: None, kind: None, bounds: None, max_id: None, since: None, until: None, search: false };
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
            let q = PhotoQuery { order, folder_id: None, kind: None, bounds: None, max_id: None, since: None, until: None, search: false };
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
        let q = PhotoQuery { order: SortOrder::Newest, folder_id: Some(g), kind: None, bounds: None, max_id: None, since: None, until: None, search: false };
        assert_eq!(file_names(&db, &q), ["f.jpg"]);
        assert_eq!(db.count_photos_in(&q).unwrap(), 1);
        assert_eq!(db.day_groups(&q).unwrap(), [group(Some("2024-06-02"), 1, 0)]);
        let q = PhotoQuery { order: SortOrder::Newest, folder_id: Some(f), kind: None, bounds: None, max_id: None, since: None, until: None, search: false };
        assert_eq!(db.count_photos_in(&q).unwrap(), 5);
    }

    #[test]
    fn empty_folder_and_empty_library_have_no_groups() {
        let db = Db::open_in_memory().unwrap();
        let f = db.add_folder("/empty").unwrap();
        let q = PhotoQuery { order: SortOrder::Newest, folder_id: Some(f), kind: None, bounds: None, max_id: None, since: None, until: None, search: false };
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
            let q = PhotoQuery { order, folder_id, kind: None, bounds: None, max_id: None, since: None, until: None, search: false };
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

    #[test]
    fn embeddings_round_trip_and_pending_query() {
        let db = Db::open_in_memory().unwrap();
        let f = db.add_folder("/p").unwrap();
        let a = db.upsert_photo(f, "/p/a.jpg", 1, 1, &meta(None)).unwrap();
        let b = db.upsert_photo(f, "/p/b.jpg", 1, 1, &meta(None)).unwrap();
        assert_eq!(db.photos_needing_embedding("m1", 10).unwrap().len(), 2);
        db.put_embedding(a, "m1", &[0.5, 0.25]).unwrap();
        db.put_embedding_failed(b, "m1").unwrap();
        assert!(db.photos_needing_embedding("m1", 10).unwrap().is_empty());
        assert_eq!(db.photos_needing_embedding("m2", 10).unwrap().len(), 2); // a new model version re-queues
        assert_eq!(db.load_embeddings("m1").unwrap(), vec![(a, vec![0.5, 0.25])]); // the failed marker is skipped
        assert_eq!(db.embedding_counts("m1").unwrap(), (2, 2));
    }

    #[test]
    fn videos_and_missing_photos_are_not_queued_for_embedding() {
        let (db, f) = mixed_library(); // 2 photos, 2 videos
        assert_eq!(db.photos_needing_embedding("m", 10).unwrap().len(), 2);
        db.mark_missing_except(f, &HashSet::new()).unwrap();
        assert!(db.photos_needing_embedding("m", 10).unwrap().is_empty());
        assert_eq!(db.embedding_counts("m").unwrap(), (0, 0));
    }

    #[test]
    fn a_changed_file_is_analysed_again_but_an_unchanged_one_is_not() {
        let db = Db::open_in_memory().unwrap();
        let f = db.add_folder("/p").unwrap();
        let a = db.upsert_photo(f, "/p/a.jpg", 10, 100, &meta(None)).unwrap();
        db.put_embedding(a, "m", &[1.0]).unwrap();
        db.upsert_photo(f, "/p/a.jpg", 10, 100, &meta(None)).unwrap(); // rescan, nothing changed
        assert_eq!(db.load_embeddings("m").unwrap().len(), 1);
        db.upsert_photo(f, "/p/a.jpg", 10, 200, &meta(None)).unwrap(); // edited: new mtime
        assert!(db.load_embeddings("m").unwrap().is_empty());
        assert_eq!(db.photos_needing_embedding("m", 10).unwrap().len(), 1);
        db.put_embedding_failed(a, "m").unwrap(); // e.g. it was half-copied when first read
        db.upsert_photo(f, "/p/a.jpg", 20, 200, &meta(None)).unwrap(); // finished copying: new size
        assert_eq!(db.photos_needing_embedding("m", 10).unwrap().len(), 1); // the failure marker expired
    }

    #[test]
    fn embeddings_go_when_the_folder_goes() {
        let db = Db::open_in_memory().unwrap();
        let f = db.add_folder("/p").unwrap();
        let a = db.upsert_photo(f, "/p/a.jpg", 1, 1, &meta(None)).unwrap();
        db.put_embedding(a, "m", &[1.0]).unwrap();
        db.remove_folder(f).unwrap();
        assert!(db.load_embeddings("m").unwrap().is_empty());
    }

    #[test]
    fn map_points_follow_the_search_result() {
        let db = Db::open_in_memory().unwrap();
        let f = db.add_folder("/p").unwrap();
        let at = PhotoMeta { gps_lat: Some(10.0), gps_lon: Some(20.0), ..Default::default() };
        let a = db.upsert_photo(f, "/p/a.jpg", 1, 1, &at).unwrap();
        let _b = db.upsert_photo(f, "/p/b.jpg", 1, 1, &at).unwrap();
        db.set_search_hits(&[a]).unwrap();
        let q = PhotoQuery { search: true, ..Default::default() };
        let pts = db.list_geo_points_in(&q).unwrap();
        assert_eq!(pts.iter().map(|p| p.id).collect::<Vec<_>>(), vec![a]);
        assert_eq!(db.list_geo_points_in(&PhotoQuery::default()).unwrap().len(), 2);
    }

    #[test]
    fn search_hits_filter_listing() {
        let db = Db::open_in_memory().unwrap();
        let f = db.add_folder("/p").unwrap();
        let a = db.upsert_photo(f, "/p/a.jpg", 1, 1, &meta(None)).unwrap();
        let _b = db.upsert_photo(f, "/p/b.jpg", 1, 1, &meta(None)).unwrap();
        db.set_search_hits(&[a]).unwrap();
        let q = PhotoQuery { search: true, ..Default::default() };
        assert_eq!(db.count_photos_in(&q).unwrap(), 1);
        assert_eq!(db.list_photos_in(&q, 0, 10).unwrap()[0].id, a);
        db.set_search_hits(&[]).unwrap();
        assert_eq!(db.count_photos_in(&q).unwrap(), 0);
        assert_eq!(db.count_photos_in(&PhotoQuery::default()).unwrap(), 2); // flag off: unfiltered
    }

    #[test]
    fn tags_are_stored_counted_and_expire_with_the_file() {
        let db = Db::open_in_memory().unwrap();
        let f = db.add_folder("/p").unwrap();
        let a = db.upsert_photo(f, "/p/a.jpg", 1, 1, &meta(None)).unwrap();
        let b = db.upsert_photo(f, "/p/b.jpg", 1, 1, &meta(None)).unwrap();
        db.put_embedding(a, "m", &[1.0, 0.0]).unwrap();
        db.put_embedding(b, "m", &[0.0, 1.0]).unwrap();
        db.put_embedding_failed(db.upsert_photo(f, "/p/c.jpg", 1, 1, &meta(None)).unwrap(), "m").unwrap();
        // Both analysed photos need tags; the one that failed has no vector to tag from.
        let need = db.embeddings_needing_tags("m", "t1", 10).unwrap();
        assert_eq!(need.iter().map(|(id, _)| *id).collect::<Vec<_>>().len(), 2);
        db.set_tags(a, "m", "t1", &[("beach".into(), 0.9), ("people".into(), 0.5)]).unwrap();
        db.set_tags(b, "m", "t1", &[("beach".into(), 0.4)]).unwrap();
        assert!(db.embeddings_needing_tags("m", "t1", 10).unwrap().is_empty());
        assert_eq!(db.embeddings_needing_tags("m", "t2", 10).unwrap().len(), 2); // a new vocabulary re-tags from the stored vectors
        assert_eq!(db.tags_for(a).unwrap(), vec![("beach".to_string(), 0.9), ("people".to_string(), 0.5)]);
        assert_eq!(db.tag_counts().unwrap(), vec![("beach".to_string(), 2), ("people".to_string(), 1)]);
        assert_eq!(db.photos_with_any_tag(&["beach", "people"]).unwrap(), 2); // a photo with both counts once
        assert_eq!(db.photos_with_any_tag(&["people"]).unwrap(), 1);
        assert_eq!(db.photos_with_any_tag(&["unicorn"]).unwrap(), 0);
        assert_eq!(db.photos_with_any_tag(&[]).unwrap(), 0);
        db.set_tags(a, "m", "t1", &[]).unwrap(); // re-tagging replaces; "no tag cleared the bar" is a valid result
        assert!(db.tags_for(a).unwrap().is_empty());
        db.set_tags(a, "m", "t1", &[("dog".into(), 0.7)]).unwrap();
        db.upsert_photo(f, "/p/a.jpg", 1, 99, &meta(None)).unwrap(); // the file changed
        assert!(db.tags_for(a).unwrap().is_empty());
        db.remove_folder(f).unwrap();
        assert!(db.tag_counts().unwrap().is_empty());
    }

    #[test]
    fn missing_photos_do_not_count_toward_tags() {
        let db = Db::open_in_memory().unwrap();
        let f = db.add_folder("/p").unwrap();
        let a = db.upsert_photo(f, "/p/a.jpg", 1, 1, &meta(None)).unwrap();
        db.put_embedding(a, "m", &[1.0]).unwrap();
        db.set_tags(a, "m", "t", &[("beach".into(), 0.9)]).unwrap();
        db.mark_missing_except(f, &HashSet::new()).unwrap();
        assert!(db.tag_counts().unwrap().is_empty());
    }

    #[test]
    fn tag_filter_and_text_search_combine_and_clear_independently() {
        let db = Db::open_in_memory().unwrap();
        let f = db.add_folder("/p").unwrap();
        let ids: Vec<_> = (0..3).map(|i| db.upsert_photo(f, &format!("/p/{i}.jpg"), 1, 1, &meta(None)).unwrap()).collect();
        for id in &ids {
            db.put_embedding(*id, "m", &[1.0]).unwrap();
        }
        db.set_tags(ids[0], "m", "t", &[("beach".into(), 0.9)]).unwrap();
        db.set_tags(ids[1], "m", "t", &[("beach".into(), 0.9)]).unwrap();
        let q = PhotoQuery { search: true, ..Default::default() };
        let hits = |db: &Db| {
            let mut v: Vec<_> = db.list_photos_in(&q, 0, 10).unwrap().iter().map(|p| p.id).collect();
            v.sort();
            v
        };
        db.set_tag_filter(Some("beach")).unwrap();
        assert_eq!(hits(&db), vec![ids[0], ids[1]]);
        db.set_search_hits(&[ids[1], ids[2]]).unwrap(); // text search on top: only photos in both
        assert_eq!(hits(&db), vec![ids[1]]);
        db.set_tag_filter(None).unwrap(); // dropping the tag leaves the text search
        assert_eq!(hits(&db), vec![ids[1], ids[2]]);
        db.clear_search_hits().unwrap();
        assert!(hits(&db).is_empty()); // nothing active: the flag matches nothing (the UI turns the flag off)
        db.set_tag_filter(Some("nothing-has-this")).unwrap();
        assert!(hits(&db).is_empty());
    }

    // ---- faces and people ----

    fn nf(score: f32, v: [f32; 2]) -> NewFace {
        NewFace { x: 0.1, y: 0.1, w: 0.2, h: 0.2, score, embedding: v.to_vec() }
    }

    fn photos(db: &Db, n: usize) -> (FolderId, Vec<PhotoId>) {
        let f = db.add_folder("/p").unwrap();
        (f, (0..n).map(|i| db.upsert_photo(f, &format!("/p/{i}.jpg"), 1, 1, &meta(None)).unwrap()).collect())
    }

    /// A person made of the given photos' faces (one face each).
    fn person_of(db: &Db, ids: &[PhotoId], v: [f32; 2]) -> i64 {
        let p = db.create_person(&v).unwrap();
        for id in ids {
            let f = db.replace_faces(*id, "fv", &[nf(0.9, v)]).unwrap()[0];
            db.assign_face(f, p, &v, 1).unwrap();
        }
        p
    }

    #[test]
    fn photos_needing_faces_and_counts() {
        let db = Db::open_in_memory().unwrap();
        let (f, ids) = photos(&db, 3);
        assert_eq!(db.face_counts("fv").unwrap(), (0, 3));
        assert_eq!(db.photos_needing_faces("fv", 10).unwrap().iter().map(|p| p.id).collect::<Vec<_>>(), vec![ids[2], ids[1], ids[0]]);
        db.replace_faces(ids[2], "fv", &[]).unwrap(); // scanned, no faces: still done
        db.replace_faces(ids[1], "old", &[]).unwrap(); // scanned with another version: still needed
        assert_eq!(db.photos_needing_faces("fv", 10).unwrap().iter().map(|p| p.id).collect::<Vec<_>>(), vec![ids[1], ids[0]]);
        assert_eq!(db.face_counts("fv").unwrap(), (1, 3));
        db.mark_missing_except(f, &HashSet::new()).unwrap();
        assert_eq!(db.face_counts("fv").unwrap(), (0, 0));
    }

    #[test]
    fn replace_faces_replaces_and_returns_ids() {
        let db = Db::open_in_memory().unwrap();
        let (_f, ids) = photos(&db, 1);
        let first = db.replace_faces(ids[0], "fv", &[nf(0.9, [1.0, 0.0]), nf(0.8, [0.0, 1.0])]).unwrap();
        assert_eq!(db.faces_of(ids[0]).unwrap().iter().map(|f| f.id).collect::<Vec<_>>(), first);
        let second = db.replace_faces(ids[0], "fv", &[nf(0.7, [1.0, 0.0])]).unwrap();
        assert_eq!(second.len(), 1);
        assert_eq!(db.faces_of(ids[0]).unwrap().len(), 1);
        assert_eq!(db.face_location(second[0]).unwrap().unwrap().0.id, ids[0]);
        assert!(db.face_location(9999).unwrap().is_none());
    }

    #[test]
    fn a_changed_file_loses_its_faces_and_scan_row() {
        let db = Db::open_in_memory().unwrap();
        let (f, ids) = photos(&db, 1);
        db.replace_faces(ids[0], "fv", &[nf(0.9, [1.0, 0.0])]).unwrap();
        db.upsert_photo(f, "/p/0.jpg", 1, 99, &meta(None)).unwrap();
        assert!(db.faces_of(ids[0]).unwrap().is_empty());
        assert_eq!(db.photos_needing_faces("fv", 10).unwrap().len(), 1);
    }

    #[test]
    fn unassigned_faces_exclude_assigned_rejected_and_hidden_ones() {
        let db = Db::open_in_memory().unwrap();
        let (_f, ids) = photos(&db, 4);
        let ff: Vec<i64> = ids.iter().map(|i| db.replace_faces(*i, "fv", &[nf(0.9, [1.0, 0.0])]).unwrap()[0]).collect();
        assert_eq!(db.unassigned_faces().unwrap().len(), 4);
        let p = db.create_person(&[1.0, 0.0]).unwrap();
        db.assign_face(ff[0], p, &[1.0, 0.0], 1).unwrap();
        db.reject_face(ff[1]).unwrap(); // rejected: waiting to be guessed again, so it counts as unassigned
        db.hide_person(p).unwrap();
        let left: Vec<i64> = db.unassigned_faces().unwrap().into_iter().map(|f| f.0).collect();
        assert_eq!(left, vec![ff[1], ff[2], ff[3]]);
        assert_eq!(db.people_centroids().unwrap().len(), 1); // the hidden person is still a candidate to join
    }

    #[test]
    fn list_people_lists_only_named_people_biggest_first_and_counts_photos() {
        let db = Db::open_in_memory().unwrap();
        let (_f, ids) = photos(&db, 6);
        let big = person_of(&db, &ids[0..3], [1.0, 0.0]);
        let pair = person_of(&db, &ids[3..5], [0.0, 1.0]);
        let single = person_of(&db, &ids[5..6], [0.6, 0.8]);
        assert!(db.list_people().unwrap().is_empty()); // nobody is named yet: nobody is listed
        db.rename_person(single, Some("Gran")).unwrap();
        db.rename_person(big, Some("Sam")).unwrap();
        let l = db.list_people().unwrap();
        assert_eq!(l.iter().map(|p| (p.id, p.count)).collect::<Vec<_>>(), vec![(big, 3), (single, 1)]); // biggest first; the unnamed pair is left out
        assert_eq!(l[1].name.as_deref(), Some("Gran"));
        assert!(l[1].face_id.is_some());
        db.rename_person(single, None).unwrap();
        assert_eq!(db.list_people().unwrap().len(), 1);
        db.hide_person(big).unwrap();
        assert!(db.list_people().unwrap().is_empty());
        let _ = pair;
    }

    #[test]
    fn merge_moves_faces_recomputes_the_centroid_and_keeps_a_name() {
        let db = Db::open_in_memory().unwrap();
        let (_f, ids) = photos(&db, 4);
        let a = person_of(&db, &ids[0..2], [1.0, 0.0]);
        let b = person_of(&db, &ids[2..4], [0.0, 1.0]);
        db.rename_person(b, Some("Sam")).unwrap();
        db.merge_people(b, a).unwrap();
        let cs = db.people_centroids().unwrap();
        assert_eq!(cs.len(), 1);
        assert_eq!((cs[0].0, cs[0].2), (a, 4));
        assert!((cs[0].1[0] - cs[0].1[1]).abs() < 1e-5);
        let l = db.list_people().unwrap();
        assert_eq!((l[0].id, l[0].count, l[0].name.as_deref()), (a, 4, Some("Sam"))); // the empty name adopts the other's
    }

    #[test]
    fn rejecting_a_face_detaches_pins_and_recomputes() {
        let db = Db::open_in_memory().unwrap();
        let (_f, ids) = photos(&db, 3);
        let p = person_of(&db, &ids, [1.0, 0.0]);
        let f0 = db.faces_of(ids[0]).unwrap()[0].id;
        db.reject_face(f0).unwrap();
        assert_eq!(db.faces_of(ids[0]).unwrap()[0].person_id, None);
        assert_eq!(db.unassigned_faces().unwrap().len(), 1); // waiting to be guessed again
        assert_eq!(db.face_rejections().unwrap().get(&f0), Some(&vec![p])); // but never as the person it was rejected from
        assert_eq!(db.people_centroids().unwrap()[0].2, 2);
        for id in &ids[1..] {
            db.reject_face(db.faces_of(*id).unwrap()[0].id).unwrap();
        }
        assert!(db.people_centroids().unwrap().iter().all(|c| c.0 != p)); // an unnamed person with no faces goes
    }

    #[test]
    fn person_filter_lists_their_photos_and_combines_with_search() {
        let db = Db::open_in_memory().unwrap();
        let (_f, ids) = photos(&db, 4);
        let p = person_of(&db, &ids[0..3], [1.0, 0.0]);
        let q = PhotoQuery { search: true, ..Default::default() };
        let hits = |db: &Db| {
            let mut v: Vec<_> = db.list_photos_in(&q, 0, 10).unwrap().iter().map(|p| p.id).collect();
            v.sort();
            v
        };
        db.set_person_filter(Some(p)).unwrap();
        assert_eq!(hits(&db), vec![ids[0], ids[1], ids[2]]);
        db.set_search_hits(&[ids[2], ids[3]]).unwrap();
        assert_eq!(hits(&db), vec![ids[2]]);
        db.set_person_filter(None).unwrap();
        assert_eq!(hits(&db), vec![ids[2], ids[3]]);
        db.clear_search_hits().unwrap();
        db.set_person_filter(Some(p)).unwrap();
        db.set_tag_filter(Some("nothing")).unwrap();
        assert!(hits(&db).is_empty());
    }

    #[test]
    fn the_active_tag_and_person_can_be_read_back() {
        let db = Db::open_in_memory().unwrap();
        assert_eq!(db.active_filters().unwrap(), (None, None));
        db.set_tag_filter(Some("museum")).unwrap();
        db.set_person_filter(Some(7)).unwrap();
        assert_eq!(db.active_filters().unwrap(), (Some("museum".to_string()), Some(7)));
        db.set_tag_filter(None).unwrap();
        assert_eq!(db.active_filters().unwrap(), (None, Some(7)));
        db.set_person_filter(None).unwrap();
        assert_eq!(db.active_filters().unwrap(), (None, None));
    }

    #[test]
    fn removing_a_folder_updates_the_people_who_had_faces_in_it() {
        let db = Db::open_in_memory().unwrap();
        let a = db.add_folder("/a").unwrap();
        let b = db.add_folder("/b").unwrap();
        let pa: Vec<_> = (0..2).map(|i| db.upsert_photo(a, &format!("/a/{i}.jpg"), 1, 1, &meta(None)).unwrap()).collect();
        let pb = db.upsert_photo(b, "/b/0.jpg", 1, 1, &meta(None)).unwrap();
        let only_a = person_of(&db, &pa, [1.0, 0.0]); // unnamed, faces only in /a
        let both = db.create_person(&[0.0, 1.0]).unwrap();
        for (n, id) in [pa[0], pb].into_iter().enumerate() {
            let f = db.replace_faces(id, "fv", &[nf(0.9, [0.0, 1.0])]).unwrap()[0];
            db.assign_face(f, both, &[0.0, 1.0], n as i64 + 1).unwrap(); // the person now has 2 faces
        }
        db.remove_folder(a).unwrap();
        let cs = db.people_centroids().unwrap();
        assert!(cs.iter().all(|c| c.0 != only_a), "an unnamed person with no faces left is removed");
        let c = cs.iter().find(|c| c.0 == both).unwrap();
        assert_eq!(c.2, 1, "the person who is still in /b is counted from the faces that remain");
    }

    #[test]
    fn a_changed_file_updates_the_person_who_was_in_it() {
        let db = Db::open_in_memory().unwrap();
        let (f, ids) = photos(&db, 2);
        let alone = person_of(&db, &ids[0..1], [1.0, 0.0]);
        let shared = person_of(&db, &ids[1..2], [0.0, 1.0]);
        let extra = db.replace_faces(db.upsert_photo(f, "/p/x.jpg", 1, 1, &meta(None)).unwrap(), "fv", &[nf(0.9, [0.0, 1.0])]).unwrap()[0];
        db.assign_face(extra, shared, &[0.0, 1.0], 2).unwrap();
        db.upsert_photo(f, "/p/0.jpg", 1, 99, &meta(None)).unwrap(); // the only photo of `alone` changed
        db.upsert_photo(f, "/p/1.jpg", 1, 99, &meta(None)).unwrap(); // one of two photos of `shared` changed
        let cs = db.people_centroids().unwrap();
        assert!(cs.iter().all(|c| c.0 != alone));
        assert_eq!(cs.iter().find(|c| c.0 == shared).unwrap().2, 1);
    }

    #[test]
    fn face_people_lists_everyone_with_best_samples_and_counts() {
        let db = Db::open_in_memory().unwrap();
        let (_f, ids) = photos(&db, 6);
        let big = db.create_person(&[1.0, 0.0]).unwrap();
        for (i, id) in ids[0..4].iter().enumerate() {
            // scores rise with i, so the best faces are the last ones
            let f = db.replace_faces(*id, "fv", &[NewFace { x: 0.1, y: 0.1, w: 0.2, h: 0.2, score: 0.5 + i as f32 / 10.0, embedding: vec![1.0, 0.0] }]).unwrap()[0];
            db.assign_face(f, big, &[1.0, 0.0], i as i64 + 1).unwrap();
        }
        let single = person_of(&db, &ids[4..5], [0.0, 1.0]);
        let hidden = person_of(&db, &ids[5..6], [0.6, 0.8]);
        db.hide_person(hidden).unwrap();
        let l = db.list_face_people(false).unwrap();
        assert_eq!(l.iter().map(|p| p.id).collect::<Vec<_>>(), vec![big, single]); // most faces first; the hidden one is left out
        assert_eq!((l[0].photos, l[0].faces, l[0].listed), (4, 4, false)); // listed in the nav only once named
        assert_eq!(l[0].samples.len(), 3);
        let best = db.faces_of(ids[3]).unwrap()[0].id;
        assert_eq!(l[0].samples[0], best);
        assert!(!l[1].listed);
        let all = db.list_face_people(true).unwrap();
        assert_eq!(all.len(), 3);
        assert!(all.iter().find(|p| p.id == hidden).unwrap().hidden);
        db.unhide_person(hidden).unwrap();
        assert_eq!(db.list_face_people(false).unwrap().len(), 3);
    }

    #[test]
    fn a_persons_faces_and_the_rejected_ones_are_listed_best_first() {
        let db = Db::open_in_memory().unwrap();
        let (_f, ids) = photos(&db, 3);
        let p = db.create_person(&[1.0, 0.0]).unwrap();
        let mut fs = Vec::new();
        for (i, id) in ids.iter().enumerate() {
            let f = db.replace_faces(*id, "fv", &[NewFace { x: 0.1, y: 0.1, w: 0.2, h: 0.2, score: 0.6 + i as f32 / 10.0, embedding: vec![1.0, 0.0] }]).unwrap()[0];
            db.assign_face(f, p, &[1.0, 0.0], i as i64 + 1).unwrap();
            fs.push(f);
        }
        let tiles = db.faces_of_person(p, 10, false).unwrap();
        assert_eq!(tiles.iter().map(|t| t.id).collect::<Vec<_>>(), vec![fs[2], fs[1], fs[0]]);
        assert_eq!(tiles[0].photo_id, ids[2]);
        // what the viewer needs to show the whole photo and outline the face
        assert_eq!((tiles[0].mtime, tiles[0].size), (1, 1));
        assert_eq!((tiles[0].x, tiles[0].y, tiles[0].w, tiles[0].h), (0.1, 0.1, 0.2, 0.2));
        assert_eq!(db.faces_of_person(p, 2, false).unwrap().len(), 2);
        assert!(db.rejected_faces(10).unwrap().is_empty());
        db.reject_face(fs[1]).unwrap();
        db.park_face(fs[1]).unwrap();
        assert_eq!(db.rejected_faces(10).unwrap().len(), 1);
        assert!(db.unassigned_faces().unwrap().is_empty(), "a parked face is not grouped again");
        assert_eq!(db.faces_of_person(p, 10, false).unwrap().len(), 2);
    }

    #[test]
    fn a_face_can_move_to_a_person_or_start_a_new_one_and_both_sides_are_updated() {
        let db = Db::open_in_memory().unwrap();
        let (_f, ids) = photos(&db, 4);
        let a = person_of(&db, &ids[0..2], [1.0, 0.0]);
        let b = person_of(&db, &ids[2..4], [0.0, 1.0]);
        let fa = db.faces_of(ids[0]).unwrap()[0].id;
        db.move_face(fa, b).unwrap();
        assert_eq!(db.faces_of(ids[0]).unwrap()[0].person_id, Some(b));
        let cs = db.people_centroids().unwrap();
        assert_eq!(cs.iter().find(|c| c.0 == a).unwrap().2, 1);
        assert_eq!(cs.iter().find(|c| c.0 == b).unwrap().2, 3);
        // a rejected face can be put back, and is no longer pinned away
        db.reject_face(fa).unwrap();
        db.move_face(fa, a).unwrap(); // put back by hand: allowed, and the rejection of `a`... (see below)
        assert_eq!(db.faces_of(ids[0]).unwrap()[0].person_id, Some(a));
        // or it can become someone new, with a name
        let n = db.new_person_from_face(fa, Some(" Gran ")).unwrap();
        assert_ne!(n, a);
        assert_eq!(db.faces_of(ids[0]).unwrap()[0].person_name.as_deref(), Some("Gran"));
        assert_eq!(db.people_centroids().unwrap().iter().find(|c| c.0 == n).unwrap().2, 1);
        // moving the last face out of an unnamed person removes that person
        let lone = person_of(&db, &ids[0..1], [0.6, 0.8]);
        let fl = db.faces_of(ids[0]).unwrap()[0].id;
        db.move_face(fl, b).unwrap();
        assert!(db.people_centroids().unwrap().iter().all(|c| c.0 != lone)); // (the named one stays)
    }

    #[test]
    fn faces_can_be_listed_most_doubtful_first() {
        let db = Db::open_in_memory().unwrap();
        let (_f, ids) = photos(&db, 3);
        let p = db.create_person(&[1.0, 0.0]).unwrap();
        let embeddings = [[1.0f32, 0.0], [0.0, 1.0], [0.8, 0.6]]; // the middle one is nothing like the centroid
        let mut fs = Vec::new();
        for (i, (id, e)) in ids.iter().zip(embeddings).enumerate() {
            let f = db.replace_faces(*id, "fv", &[NewFace { x: 0.1, y: 0.1, w: 0.2, h: 0.2, score: 0.9, embedding: e.to_vec() }]).unwrap()[0];
            db.assign_face(f, p, &[1.0, 0.0], i as i64 + 1).unwrap();
            fs.push(f);
        }
        let order = |doubtful| db.faces_of_person(p, 10, doubtful).unwrap().iter().map(|t| t.id).collect::<Vec<_>>();
        assert_eq!(order(true), vec![fs[1], fs[2], fs[0]]);
        assert_eq!(db.faces_of_person(p, 2, true).unwrap().iter().map(|t| t.id).collect::<Vec<_>>(), vec![fs[1], fs[2]]);
    }

    #[test]
    fn several_faces_can_be_moved_or_rejected_at_once() {
        let db = Db::open_in_memory().unwrap();
        let (_f, ids) = photos(&db, 6);
        let a = person_of(&db, &ids[0..4], [1.0, 0.0]);
        let b = person_of(&db, &ids[4..6], [0.0, 1.0]);
        let fa: Vec<i64> = ids[0..4].iter().map(|i| db.faces_of(*i).unwrap()[0].id).collect();
        db.move_faces(&fa[0..2], b).unwrap();
        let cs = db.people_centroids().unwrap();
        assert_eq!(cs.iter().find(|c| c.0 == a).unwrap().2, 2);
        assert_eq!(cs.iter().find(|c| c.0 == b).unwrap().2, 4);
        db.reject_faces(&fa[2..4]).unwrap();
        assert_eq!(db.unassigned_faces().unwrap().len(), 2);
        assert!(db.people_centroids().unwrap().iter().all(|c| c.0 != a)); // an unnamed person with no faces left is gone
        db.move_faces(&[], b).unwrap(); // nothing to do is fine
    }

    #[test]
    fn rejections_follow_a_merge_and_are_lifted_by_putting_the_face_back() {
        let db = Db::open_in_memory().unwrap();
        let (_f, ids) = photos(&db, 4);
        let a = person_of(&db, &ids[0..2], [1.0, 0.0]);
        let b = person_of(&db, &ids[2..4], [0.0, 1.0]);
        let fa = db.faces_of(ids[0]).unwrap()[0].id;
        db.reject_face(fa).unwrap();
        assert_eq!(db.face_rejections().unwrap().get(&fa), Some(&vec![a]));
        db.merge_people(a, b).unwrap(); // `a` is now part of `b`: the face must not be guessed into `b` either
        assert_eq!(db.face_rejections().unwrap().get(&fa), Some(&vec![b]));
        db.move_face(fa, b).unwrap(); // the user puts it back
        assert!(db.face_rejections().unwrap().get(&fa).is_none());
    }
}
