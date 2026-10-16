use crate::fetch::GeoError;
use crate::place::Place;
use rusqlite::{Connection, OptionalExtension};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

fn io(e: rusqlite::Error) -> GeoError {
    GeoError::Io(e.to_string())
}

pub struct PlaceCache {
    conn: Connection,
}

impl PlaceCache {
    pub fn open(path: &Path) -> Result<Self, GeoError> {
        Self::init(Connection::open(path).map_err(io)?)
    }

    pub fn open_in_memory() -> Result<Self, GeoError> {
        Self::init(Connection::open_in_memory().map_err(io)?)
    }

    fn init(conn: Connection) -> Result<Self, GeoError> {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS places (
                key TEXT PRIMARY KEY,
                found INTEGER NOT NULL,
                landmark TEXT,
                locality TEXT,
                fetched_at INTEGER NOT NULL
            )",
        )
        .map_err(io)?;
        Ok(PlaceCache { conn })
    }

    /// Outer `None`: never looked up. `Some(None)`: looked up, nothing there.
    pub fn get(&self, key: &str) -> Result<Option<Option<Place>>, GeoError> {
        let row = self
            .conn
            .query_row("SELECT found, landmark, locality FROM places WHERE key = ?1", [key], |r| {
                Ok((r.get::<_, i64>(0)?, r.get::<_, Option<String>>(1)?, r.get::<_, Option<String>>(2)?))
            })
            .optional()
            .map_err(io)?;
        Ok(row.map(|(found, landmark, locality)| (found == 1).then_some(Place { landmark, locality })))
    }

    pub fn put(&self, key: &str, place: Option<&Place>) -> Result<(), GeoError> {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
        self.conn
            .execute(
                "INSERT OR REPLACE INTO places (key, found, landmark, locality, fetched_at) VALUES (?1, ?2, ?3, ?4, ?5)",
                rusqlite::params![
                    key,
                    place.is_some() as i64,
                    place.and_then(|p| p.landmark.as_deref()),
                    place.and_then(|p| p.locality.as_deref()),
                    now
                ],
            )
            .map_err(io)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_key_is_not_cached() {
        let c = PlaceCache::open_in_memory().unwrap();
        assert_eq!(c.get("1,2").unwrap(), None);
    }

    #[test]
    fn stores_and_reads_a_place() {
        let c = PlaceCache::open_in_memory().unwrap();
        let p = Place { landmark: Some("Colosseum".into()), locality: Some("Roma, Italia".into()) };
        c.put("k", Some(&p)).unwrap();
        assert_eq!(c.get("k").unwrap(), Some(Some(p)));
    }

    #[test]
    fn stores_a_miss_distinct_from_not_cached() {
        let c = PlaceCache::open_in_memory().unwrap();
        c.put("k", None).unwrap();
        assert_eq!(c.get("k").unwrap(), Some(None));
    }

    #[test]
    fn persists_across_reopen() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("places.db");
        let p = Place { landmark: None, locality: Some("Paris".into()) };
        PlaceCache::open(&path).unwrap().put("k", Some(&p)).unwrap();
        assert_eq!(PlaceCache::open(&path).unwrap().get("k").unwrap(), Some(Some(p)));
    }
}
