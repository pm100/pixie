# Location in the details pane: Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Show a small map and, when known, a landmark name in the details pane for a photo with GPS.

**Architecture:** A new Rust crate `pixie-geo` owns everything that talks to OpenStreetMap: a tile service (disk-cached, served to the webview through the existing `pixie:` URI scheme at `/tile/{z}/{x}/{y}`) and a place service (Nominatim reverse lookup, SQLite-cached, rate limited). Both take an injected `Fetcher` so tests never touch the network. The UI adds a Leaflet map component and a pure display-lines function.

**Tech Stack:** Rust (ureq 2, rusqlite, serde_json, thiserror), Tauri 2 URI scheme + command, Svelte 5, Leaflet 1.9.4.

**Spec:** `docs/superpowers/specs/2026-10-16-location-in-details-design.md`

## Global Constraints

- **AGENTS.md: never commit or push unless the user says so.** This plan has no commit steps. Leave everything uncommitted.
- Never kill the user's running `pixie-app.exe`; never touch `%APPDATA%\app.pixie.desktop\`. Do not launch the app against that directory.
- Use `npm run tauri ...` (local v2), never global `cargo tauri`. Never run plain `cargo update`.
- Rust env per session: `VCPKG_ROOT=C:/work/vcpkg-pixie`, `VCPKGRS_TRIPLET=x64-windows-static-md`, and `CARGO_RESOLVER_INCOMPATIBLE_RUST_VERSIONS=fallback` (Cargo.lock was made with it).
- User-Agent: `Pixie/<version> (+https://github.com/pm100/pixie)`. Nominatim: at most 1 request/second. Tile zoom 0-19, tile concurrency 2. Network timeout 5 s.
- Place cache key: lat,lon rounded to 4 decimals. Misses (no result) are cached; network failures are not.
- OSM attribution ("© OpenStreetMap contributors") must be visible under the map. No clickable Leaflet attribution link (it would navigate the webview).
- CSP already allows `img-src pixie: http://pixie.localhost`; do not loosen it.

## Review Focus

- Offline: tile and place failures show "Map unavailable" / no landmark line; no error popup, no retry storm.
- Clicking quickly through photos: a slow place response for an earlier photo must not overwrite the current photo's place.
- Photos whose GPS is out of range or (0,0) never reach the services (already nulled by earlier work; the command also validates).
- A result with a name but a mundane category (street, shop) must not be shown as a landmark.
- Dragging the pane divider resizes the map without grey/blank areas.

---

### Task 1: `pixie-geo` crate, `Place`, response parsing, cache key

**Files:**
- Create: `crates/pixie-geo/Cargo.toml`, `crates/pixie-geo/src/lib.rs`, `crates/pixie-geo/src/place.rs`

**Interfaces:**
- Produces: `Place { landmark: Option<String>, locality: Option<String> }` (Serialize, Deserialize, Debug, Clone, PartialEq, Eq); `parse_place(json: &str) -> Option<Place>`; `place_key(lat: f64, lon: f64) -> String`.

- [ ] **Step 1: Create the crate**

`crates/pixie-geo/Cargo.toml`:

```toml
[package]
name = "pixie-geo"
version.workspace = true
edition.workspace = true

[dependencies]
serde = { workspace = true }
serde_json = "1"
thiserror = { workspace = true }
rusqlite = { workspace = true }
ureq = "2"

[dev-dependencies]
tempfile = { workspace = true }
```

`crates/pixie-geo/src/lib.rs`:

```rust
mod place;

pub use place::{parse_place, place_key, Place};
```

- [ ] **Step 2: Write the failing tests** in `crates/pixie-geo/src/place.rs` (tests only, plus empty stubs so it compiles)

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Place {
    pub landmark: Option<String>,
    pub locality: Option<String>,
}

pub fn parse_place(_json: &str) -> Option<Place> {
    None
}

pub fn place_key(_lat: f64, _lon: f64) -> String {
    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    const COLOSSEUM: &str = r#"{"place_id":1,"category":"historic","type":"building","name":"Colosseum",
        "address":{"historic":"Colosseum","road":"Piazza del Colosseo","suburb":"Municipio Roma I","city":"Roma",
        "state":"Lazio","country":"Italia"}}"#;
    const BELLAGIO: &str = r#"{"category":"tourism","type":"hotel","name":"Bellagio",
        "address":{"tourism":"Bellagio","road":"Las Vegas Boulevard","city":"Las Vegas","state":"Nevada","country":"United States"}}"#;
    const STREET: &str = r#"{"category":"highway","type":"residential","name":"Rue de Rivoli",
        "address":{"road":"Rue de Rivoli","city":"Paris","state":"Île-de-France","country":"France"}}"#;
    const ERROR: &str = r#"{"error":"Unable to geocode"}"#;

    #[test]
    fn historic_site_is_a_landmark() {
        let p = parse_place(COLOSSEUM).unwrap();
        assert_eq!(p.landmark.as_deref(), Some("Colosseum"));
        assert_eq!(p.locality.as_deref(), Some("Roma, Lazio, Italia"));
    }

    #[test]
    fn hotel_is_a_landmark() {
        let p = parse_place(BELLAGIO).unwrap();
        assert_eq!(p.landmark.as_deref(), Some("Bellagio"));
        assert_eq!(p.locality.as_deref(), Some("Las Vegas, Nevada, United States"));
    }

    #[test]
    fn plain_street_has_locality_but_no_landmark() {
        let p = parse_place(STREET).unwrap();
        assert_eq!(p.landmark, None);
        assert_eq!(p.locality.as_deref(), Some("Paris, Île-de-France, France"));
    }

    #[test]
    fn error_garbage_and_empty_give_none() {
        assert_eq!(parse_place(ERROR), None);
        assert_eq!(parse_place("not json"), None);
        assert_eq!(parse_place("{}"), None);
    }

    #[test]
    fn repeated_parts_are_collapsed() {
        let j = r#"{"category":"highway","name":"X","address":{"city":"Singapore","country":"Singapore"}}"#;
        assert_eq!(parse_place(j).unwrap().locality.as_deref(), Some("Singapore"));
    }

    #[test]
    fn nameless_landmark_category_is_not_a_landmark() {
        let j = r#"{"category":"tourism","type":"hotel","name":"","address":{"town":"Bray","country":"Ireland"}}"#;
        let p = parse_place(j).unwrap();
        assert_eq!(p.landmark, None);
        assert_eq!(p.locality.as_deref(), Some("Bray, Ireland"));
    }

    #[test]
    fn key_rounds_to_four_decimals_and_has_no_negative_zero() {
        assert_eq!(place_key(41.890212, 12.492231), "41.8902,12.4922");
        assert_eq!(place_key(41.89021, 12.49223), place_key(41.890199, 12.492249));
        assert_eq!(place_key(-0.00001, 0.0), "0.0000,0.0000");
        assert_eq!(place_key(-33.8568, 151.2153), "-33.8568,151.2153");
    }
}
```

- [ ] **Step 3: Run to verify failure**

Run: `cargo test -p pixie-geo` (with the env from Global Constraints).
Expected: compiles (first build fetches ureq and friends), tests FAIL (the stubs return `None` / empty string).

- [ ] **Step 4: Implement** (replace the two stubs; add `use serde_json::Value;`)

```rust
use serde_json::Value;

const LANDMARK_CATEGORIES: &[&str] = &["tourism", "historic", "amenity", "leisure", "building"];
const TOWN_KEYS: &[&str] = &["city", "town", "village", "municipality", "hamlet", "suburb"];

fn text<'a>(v: &'a Value, key: &str) -> Option<&'a str> {
    v.get(key)?.as_str().map(str::trim).filter(|s| !s.is_empty())
}

/// Parse a Nominatim `format=jsonv2` reverse-lookup body. `None` when there is
/// nothing worth showing (error body, garbage, or no name and no address).
pub fn parse_place(json: &str) -> Option<Place> {
    let v: Value = serde_json::from_str(json).ok()?;
    if v.get("error").is_some() {
        return None;
    }
    let landmark = match (text(&v, "category"), text(&v, "name")) {
        (Some(c), Some(n)) if LANDMARK_CATEGORIES.contains(&c) => Some(n.to_string()),
        _ => None,
    };
    let locality = v.get("address").and_then(|a| {
        let town = TOWN_KEYS.iter().find_map(|k| text(a, k));
        let mut parts: Vec<&str> = [town, text(a, "state"), text(a, "country")].into_iter().flatten().collect();
        parts.dedup();
        if parts.is_empty() {
            None
        } else {
            Some(parts.join(", "))
        }
    });
    if landmark.is_none() && locality.is_none() {
        None
    } else {
        Some(Place { landmark, locality })
    }
}

/// Cache key: coordinates rounded to 4 decimals (about 11 m).
pub fn place_key(lat: f64, lon: f64) -> String {
    let f = |v: f64| {
        let s = format!("{v:.4}");
        if s == "-0.0000" {
            "0.0000".to_string()
        } else {
            s
        }
    };
    format!("{},{}", f(lat), f(lon))
}
```

- [ ] **Step 5: Run to verify pass**

Run: `cargo test -p pixie-geo`
Expected: 7 passed.

---

### Task 2: `Fetcher`, `GeoError`, `PlaceCache`, `PlaceService`

**Files:**
- Create: `crates/pixie-geo/src/fetch.rs`, `crates/pixie-geo/src/cache.rs`, `crates/pixie-geo/src/service.rs`
- Modify: `crates/pixie-geo/src/lib.rs`

**Interfaces:**
- Consumes: `Place`, `parse_place`, `place_key` (Task 1).
- Produces: `GeoError { Network(String), Io(String), BadTile(u8,u32,u32) }`; `trait Fetcher: Send + Sync { fn get(&self, url: &str) -> Result<Vec<u8>, GeoError>; }`; `HttpFetcher::new()`; `USER_AGENT`; `PlaceCache::{open(&Path), open_in_memory(), get(&str) -> Result<Option<Option<Place>>, GeoError>, put(&str, Option<&Place>) -> Result<(), GeoError>}` (outer `None` = not cached); `PlaceService::new(cache: PlaceCache, fetcher: Arc<dyn Fetcher>, gap: Duration)` and `place_for(&self, lat: f64, lon: f64) -> Result<Option<Place>, GeoError>`; test-only `FakeFetcher`.

- [ ] **Step 1: `fetch.rs`**

```rust
use std::io::Read;
use std::time::Duration;

pub const USER_AGENT: &str = concat!("Pixie/", env!("CARGO_PKG_VERSION"), " (+https://github.com/pm100/pixie)");

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum GeoError {
    #[error("network: {0}")]
    Network(String),
    #[error("io: {0}")]
    Io(String),
    #[error("bad tile {0}/{1}/{2}")]
    BadTile(u8, u32, u32),
}

/// One blocking HTTP GET. Injected so tests never use the network.
pub trait Fetcher: Send + Sync {
    fn get(&self, url: &str) -> Result<Vec<u8>, GeoError>;
}

pub struct HttpFetcher {
    agent: ureq::Agent,
}

impl HttpFetcher {
    pub fn new() -> Self {
        let agent = ureq::AgentBuilder::new().user_agent(USER_AGENT).timeout(Duration::from_secs(5)).build();
        HttpFetcher { agent }
    }
}

impl Default for HttpFetcher {
    fn default() -> Self {
        Self::new()
    }
}

impl Fetcher for HttpFetcher {
    fn get(&self, url: &str) -> Result<Vec<u8>, GeoError> {
        let resp = self.agent.get(url).call().map_err(|e| GeoError::Network(e.to_string()))?;
        let mut buf = Vec::new();
        resp.into_reader()
            .take(8 * 1024 * 1024)
            .read_to_end(&mut buf)
            .map_err(|e| GeoError::Network(e.to_string()))?;
        Ok(buf)
    }
}

#[cfg(test)]
pub(crate) mod testing {
    use super::*;
    use std::sync::Mutex;
    use std::time::Instant;

    /// Records every call; replies with whatever was last set.
    pub struct FakeFetcher {
        pub calls: Mutex<Vec<(String, Instant)>>,
        reply: Mutex<Result<Vec<u8>, GeoError>>,
    }

    impl FakeFetcher {
        pub fn new(reply: Result<Vec<u8>, GeoError>) -> Self {
            FakeFetcher { calls: Mutex::new(Vec::new()), reply: Mutex::new(reply) }
        }
        pub fn set(&self, reply: Result<Vec<u8>, GeoError>) {
            *self.reply.lock().unwrap() = reply;
        }
        pub fn count(&self) -> usize {
            self.calls.lock().unwrap().len()
        }
    }

    impl Fetcher for FakeFetcher {
        fn get(&self, url: &str) -> Result<Vec<u8>, GeoError> {
            self.calls.lock().unwrap().push((url.to_string(), Instant::now()));
            self.reply.lock().unwrap().clone()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_agent_names_the_app() {
        assert!(USER_AGENT.starts_with("Pixie/0.1.0 (+https://github.com/pm100/pixie"));
    }
}
```

- [ ] **Step 2: `cache.rs`** (with tests)

```rust
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
```

- [ ] **Step 3: `service.rs` with tests first** (implementation stubbed to `todo!()` for the RED run)

```rust
use crate::cache::PlaceCache;
use crate::fetch::{Fetcher, GeoError};
use crate::place::{parse_place, place_key, Place};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub struct PlaceService {
    cache: Mutex<PlaceCache>,
    fetcher: Arc<dyn Fetcher>,
    last_request: Mutex<Option<Instant>>,
    gap: Duration,
}

impl PlaceService {
    pub fn new(cache: PlaceCache, fetcher: Arc<dyn Fetcher>, gap: Duration) -> Self {
        PlaceService { cache: Mutex::new(cache), fetcher, last_request: Mutex::new(None), gap }
    }

    pub fn place_for(&self, lat: f64, lon: f64) -> Result<Option<Place>, GeoError> {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fetch::testing::FakeFetcher;

    const COLOSSEUM: &str = r#"{"category":"historic","name":"Colosseum","address":{"city":"Roma","country":"Italia"}}"#;
    const NOTHING: &str = r#"{"error":"Unable to geocode"}"#;

    fn service(f: &Arc<FakeFetcher>, gap_ms: u64) -> PlaceService {
        PlaceService::new(PlaceCache::open_in_memory().unwrap(), f.clone(), Duration::from_millis(gap_ms))
    }

    #[test]
    fn looks_up_once_then_serves_from_cache() {
        let f = Arc::new(FakeFetcher::new(Ok(COLOSSEUM.as_bytes().to_vec())));
        let s = service(&f, 0);
        let a = s.place_for(41.8902, 12.4922).unwrap().unwrap();
        assert_eq!(a.landmark.as_deref(), Some("Colosseum"));
        assert_eq!(s.place_for(41.8902, 12.4922).unwrap(), Some(a));
        assert_eq!(f.count(), 1);
    }

    #[test]
    fn points_within_rounding_share_one_lookup() {
        let f = Arc::new(FakeFetcher::new(Ok(COLOSSEUM.as_bytes().to_vec())));
        let s = service(&f, 0);
        s.place_for(41.89021, 12.49223).unwrap();
        s.place_for(41.89019, 12.49224).unwrap();
        assert_eq!(f.count(), 1);
    }

    #[test]
    fn a_miss_is_cached() {
        let f = Arc::new(FakeFetcher::new(Ok(NOTHING.as_bytes().to_vec())));
        let s = service(&f, 0);
        assert_eq!(s.place_for(10.0, 10.0).unwrap(), None);
        assert_eq!(s.place_for(10.0, 10.0).unwrap(), None);
        assert_eq!(f.count(), 1);
    }

    #[test]
    fn network_failure_is_reported_and_not_cached() {
        let f = Arc::new(FakeFetcher::new(Err(GeoError::Network("offline".into()))));
        let s = service(&f, 0);
        assert!(s.place_for(10.0, 10.0).is_err());
        f.set(Ok(COLOSSEUM.as_bytes().to_vec()));
        assert!(s.place_for(10.0, 10.0).unwrap().is_some());
        assert_eq!(f.count(), 2);
    }

    #[test]
    fn requests_are_spaced_by_the_gap() {
        let f = Arc::new(FakeFetcher::new(Ok(COLOSSEUM.as_bytes().to_vec())));
        let s = service(&f, 80);
        s.place_for(1.0, 1.0).unwrap();
        s.place_for(2.0, 2.0).unwrap();
        let calls = f.calls.lock().unwrap();
        assert!(calls[1].1.duration_since(calls[0].1) >= Duration::from_millis(80));
    }

    #[test]
    fn out_of_range_coordinates_never_hit_the_network() {
        let f = Arc::new(FakeFetcher::new(Ok(COLOSSEUM.as_bytes().to_vec())));
        let s = service(&f, 0);
        for (la, lo) in [(91.0, 0.0), (0.0, 181.0), (f64::NAN, 0.0), (0.0, f64::INFINITY)] {
            assert_eq!(s.place_for(la, lo).unwrap(), None);
        }
        assert_eq!(f.count(), 0);
    }
}
```

- [ ] **Step 4: Wire modules and run to verify failure**

`lib.rs`:

```rust
mod cache;
mod fetch;
mod place;
mod service;

pub use cache::PlaceCache;
pub use fetch::{Fetcher, GeoError, HttpFetcher, USER_AGENT};
pub use place::{parse_place, place_key, Place};
pub use service::PlaceService;
```

Run: `cargo test -p pixie-geo`
Expected: cache and fetch tests pass; the 6 service tests FAIL (panic `not yet implemented`).

- [ ] **Step 5: Implement `place_for`** (replace the `todo!()`)

```rust
    pub fn place_for(&self, lat: f64, lon: f64) -> Result<Option<Place>, GeoError> {
        if !(lat.is_finite() && lon.is_finite() && lat.abs() <= 90.0 && lon.abs() <= 180.0) {
            return Ok(None);
        }
        let key = place_key(lat, lon);
        if let Some(hit) = self.cache.lock().unwrap().get(&key)? {
            return Ok(hit);
        }
        // Holding this lock through the request serialises lookups, which is what
        // keeps us under the one-request-per-second policy.
        let mut last = self.last_request.lock().unwrap();
        // A caller ahead of us may have just filled this key.
        if let Some(hit) = self.cache.lock().unwrap().get(&key)? {
            return Ok(hit);
        }
        if let Some(t) = *last {
            let since = t.elapsed();
            if since < self.gap {
                std::thread::sleep(self.gap - since);
            }
        }
        let url = format!(
            "https://nominatim.openstreetmap.org/reverse?format=jsonv2&lat={lat:.5}&lon={lon:.5}&zoom=18&addressdetails=1&namedetails=1"
        );
        let result = self.fetcher.get(&url);
        *last = Some(Instant::now());
        let body = result?;
        let place = parse_place(&String::from_utf8_lossy(&body));
        self.cache.lock().unwrap().put(&key, place.as_ref())?;
        Ok(place)
    }
```

- [ ] **Step 6: Run to verify pass**

Run: `cargo test -p pixie-geo`
Expected: all pass (7 + 1 + 4 + 6 = 18).

---

### Task 3: `TileService` and a live landmark check

**Files:**
- Create: `crates/pixie-geo/src/tiles.rs`
- Modify: `crates/pixie-geo/src/lib.rs`

**Interfaces:**
- Consumes: `Fetcher`, `GeoError` (Task 2).
- Produces: `TileService::new(dir: PathBuf, fetcher: Arc<dyn Fetcher>, workers: usize)` (Clone); `request(&self, z: u8, x: u32, y: u32, reply: Box<dyn FnOnce(Result<Vec<u8>, GeoError>) + Send>)`.

- [ ] **Step 1: Write `tiles.rs` with tests and a `todo!()` body for `load`**

```rust
use crate::fetch::{Fetcher, GeoError};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Mutex};

pub type TileReply = Box<dyn FnOnce(Result<Vec<u8>, GeoError>) + Send>;

struct Job {
    z: u8,
    x: u32,
    y: u32,
    reply: TileReply,
}

/// Fetches OSM tiles on a small worker pool and caches them on disk.
#[derive(Clone)]
pub struct TileService {
    tx: Sender<Job>,
}

fn load(dir: &Path, fetcher: &dyn Fetcher, z: u8, x: u32, y: u32) -> Result<Vec<u8>, GeoError> {
    todo!()
}

impl TileService {
    pub fn new(dir: PathBuf, fetcher: Arc<dyn Fetcher>, workers: usize) -> Self {
        let (tx, rx) = channel::<Job>();
        let rx = Arc::new(Mutex::new(rx));
        for _ in 0..workers.max(1) {
            let (rx, dir, fetcher) = (rx.clone(), dir.clone(), fetcher.clone());
            std::thread::spawn(move || loop {
                let job = match rx.lock().unwrap().recv() {
                    Ok(job) => job,
                    Err(_) => return, // every sender dropped
                };
                let result = load(&dir, &*fetcher, job.z, job.x, job.y);
                (job.reply)(result);
            });
        }
        TileService { tx }
    }

    pub fn request(&self, z: u8, x: u32, y: u32, reply: TileReply) {
        if z > 19 || x >= (1u32 << z) || y >= (1u32 << z) {
            return reply(Err(GeoError::BadTile(z, x, y)));
        }
        if let Err(e) = self.tx.send(Job { z, x, y, reply }) {
            (e.0.reply)(Err(GeoError::Io("tile service stopped".into())));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fetch::testing::FakeFetcher;
    use std::sync::mpsc;
    use std::time::Duration;

    fn fetch(s: &TileService, z: u8, x: u32, y: u32) -> Result<Vec<u8>, GeoError> {
        let (tx, rx) = mpsc::channel();
        s.request(z, x, y, Box::new(move |r| tx.send(r).unwrap()));
        rx.recv_timeout(Duration::from_secs(5)).unwrap()
    }

    #[test]
    fn fetches_once_then_serves_from_disk() {
        let dir = tempfile::tempdir().unwrap();
        let f = Arc::new(FakeFetcher::new(Ok(vec![1, 2, 3])));
        let s = TileService::new(dir.path().to_path_buf(), f.clone(), 2);
        assert_eq!(fetch(&s, 5, 10, 12).unwrap(), vec![1, 2, 3]);
        assert_eq!(fetch(&s, 5, 10, 12).unwrap(), vec![1, 2, 3]);
        assert_eq!(f.count(), 1);
        assert!(dir.path().join("5").join("10").join("12.png").exists());
        assert_eq!(f.calls.lock().unwrap()[0].0, "https://tile.openstreetmap.org/5/10/12.png");
    }

    #[test]
    fn out_of_range_tiles_are_rejected_without_a_fetch() {
        let dir = tempfile::tempdir().unwrap();
        let f = Arc::new(FakeFetcher::new(Ok(vec![1])));
        let s = TileService::new(dir.path().to_path_buf(), f.clone(), 1);
        assert_eq!(fetch(&s, 20, 0, 0), Err(GeoError::BadTile(20, 0, 0)));
        assert_eq!(fetch(&s, 3, 8, 0), Err(GeoError::BadTile(3, 8, 0)));
        assert_eq!(fetch(&s, 3, 0, 8), Err(GeoError::BadTile(3, 0, 8)));
        assert_eq!(f.count(), 0);
    }

    #[test]
    fn failures_and_empty_bodies_are_not_cached() {
        let dir = tempfile::tempdir().unwrap();
        let f = Arc::new(FakeFetcher::new(Err(GeoError::Network("offline".into()))));
        let s = TileService::new(dir.path().to_path_buf(), f.clone(), 1);
        assert!(fetch(&s, 2, 1, 1).is_err());
        f.set(Ok(vec![]));
        assert!(fetch(&s, 2, 1, 1).is_err());
        f.set(Ok(vec![9]));
        assert_eq!(fetch(&s, 2, 1, 1).unwrap(), vec![9]);
        assert_eq!(f.count(), 3);
    }
}
```

Add to `lib.rs`: `mod tiles;` and `pub use tiles::{TileReply, TileService};`.

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p pixie-geo tiles`
Expected: the 3 tile tests: first two `fetches_once...` and `failures...` FAIL (panic `not yet implemented`); `out_of_range...` passes already (validation is in `request`).

- [ ] **Step 3: Implement `load`**

```rust
fn load(dir: &Path, fetcher: &dyn Fetcher, z: u8, x: u32, y: u32) -> Result<Vec<u8>, GeoError> {
    let path = dir.join(z.to_string()).join(x.to_string()).join(format!("{y}.png"));
    if let Ok(bytes) = std::fs::read(&path) {
        if !bytes.is_empty() {
            return Ok(bytes);
        }
    }
    let bytes = fetcher.get(&format!("https://tile.openstreetmap.org/{z}/{x}/{y}.png"))?;
    if bytes.is_empty() {
        return Err(GeoError::Network("empty tile".into()));
    }
    // A failed cache write must not lose the tile we already have.
    if let Some(parent) = path.parent() {
        if std::fs::create_dir_all(parent).is_ok() {
            let tmp = path.with_extension("tmp");
            if std::fs::write(&tmp, &bytes).is_ok() {
                let _ = std::fs::rename(&tmp, &path);
            }
        }
    }
    Ok(bytes)
}
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test -p pixie-geo`
Expected: 21 passed.

- [ ] **Step 5: Live check of landmark detection (network; run once, by hand)**

Append to `service.rs` tests module:

```rust
    /// Hits the real Nominatim. Run by hand: `cargo test -p pixie-geo live -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn live_landmarks() {
        let s = PlaceService::new(
            PlaceCache::open_in_memory().unwrap(),
            Arc::new(crate::fetch::HttpFetcher::new()),
            Duration::from_millis(1100),
        );
        for (name, lat, lon) in [
            ("Colosseum", 41.89021, 12.49223),
            ("Bellagio", 36.11265, -115.17669),
            ("Louvre", 48.86065, 2.33764),
            ("Mid-ocean", 0.0, -30.0),
        ] {
            println!("{name}: {:?}", s.place_for(lat, lon));
        }
    }
```

Run: `cargo test -p pixie-geo live -- --ignored --nocapture`
Expected: Colosseum, Bellagio and Louvre print `landmark: Some(..)` with a locality; Mid-ocean prints `Ok(None)` or an error. **If a famous place comes back without a landmark, read the printed result, decide whether the category list in `place.rs` needs widening, and ledger the finding** (OSM tagging varies; the spec accepts best-effort).

---

### Task 4: Tauri wiring (tile scheme path + `place_for` command)

**Files:**
- Modify: `src-tauri/Cargo.toml`, `src-tauri/src/state.rs`, `src-tauri/src/lib.rs`

**Interfaces:**
- Consumes: `TileService`, `PlaceService`, `HttpFetcher`, `PlaceCache`, `Place`, `GeoError` (Tasks 1-3).
- Produces: URI `pixie:/tile/{z}/{x}/{y}` returning `image/png`; command `place_for(lat, lon) -> Option<Place> | string error`.

- [ ] **Step 1: Read `src-tauri/src/state.rs`**, then add to `AppState` the fields `pub tiles: pixie_geo::TileService` and `pub places: Arc<pixie_geo::PlaceService>` (AppState is cloned out of managed state, so every field must be cheap to clone). Add `pixie-geo = { path = "../crates/pixie-geo" }` to `src-tauri/Cargo.toml` dependencies.

- [ ] **Step 2: Edit `src-tauri/src/lib.rs`**

Imports: `use pixie_geo::{GeoError, HttpFetcher, Place, PlaceCache, PlaceService, TileService};` and `use std::time::Duration;`.

Add after `print_page`:

```rust
#[tauri::command(async)] // off the main thread: may wait on the rate limit and the network
fn place_for(state: tauri::State<AppState>, lat: f64, lon: f64) -> Result<Option<Place>, String> {
    state.places.place_for(lat, lon).map_err(|e| e.to_string())
}
```

Add after `jpeg_response`:

```rust
fn png_response(bytes: Vec<u8>) -> Response<Vec<u8>> {
    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "image/png")
        .header("Cache-Control", "public, max-age=86400")
        .header("Access-Control-Allow-Origin", "*")
        .body(bytes)
        .unwrap()
}

/// `/tile/{z}/{x}/{y}`: an OpenStreetMap tile, fetched and cached by the backend.
fn handle_tile(state: &AppState, path: &str, responder: tauri::UriSchemeResponder) {
    let mut parts = path.trim_matches('/').split('/').skip(1); // skip "tile"
    let z = parts.next().and_then(|s| s.parse::<u8>().ok());
    let x = parts.next().and_then(|s| s.parse::<u32>().ok());
    let y = parts.next().and_then(|s| s.parse::<u32>().ok());
    let (Some(z), Some(x), Some(y)) = (z, x, y) else {
        return responder.respond(status(StatusCode::NOT_FOUND));
    };
    state.tiles.request(
        z,
        x,
        y,
        Box::new(move |result| {
            responder.respond(match result {
                Ok(bytes) => png_response(bytes),
                Err(GeoError::BadTile(..)) => status(StatusCode::NOT_FOUND),
                Err(e) => {
                    eprintln!("[tile] {z}/{x}/{y}: {e}");
                    status(StatusCode::BAD_GATEWAY)
                }
            });
        }),
    );
}
```

In `run()`: the protocol closure becomes

```rust
        .register_asynchronous_uri_scheme_protocol("pixie", |ctx, request, responder| {
            let state = ctx.app_handle().state::<AppState>().inner().clone();
            let path = request.uri().path();
            if path.trim_matches('/').starts_with("tile/") {
                handle_tile(&state, path, responder);
            } else {
                handle_thumb(&state, path, responder);
            }
        })
```

In `setup`, before `app.manage(...)`:

```rust
            let fetcher: Arc<dyn pixie_geo::Fetcher> = Arc::new(HttpFetcher::new());
            let places = Arc::new(PlaceService::new(
                PlaceCache::open(&dir.join("places.db"))?,
                fetcher.clone(),
                Duration::from_millis(1100),
            ));
            let tiles = TileService::new(dir.join("tiles"), fetcher, 2);
```

and add `tiles, places` to the `AppState { .. }` literal. Add `place_for` to `generate_handler![...]`.

- [ ] **Step 3: Build**

Run: `cargo build -p pixie-app` (env from Global Constraints) and `cargo test --workspace`.
Expected: builds; all tests pass (previous totals plus 21 in pixie-geo, 1 ignored).

---

### Task 5: UI pure modules

**Files:**
- Modify: `ui/src/lib/api.ts`, `ui/src/lib/url.ts`, `ui/src/lib/url.test.ts`
- Create: `ui/src/lib/placeLines.ts`, `ui/src/lib/placeLines.test.ts`, `ui/src/lib/latest.ts`, `ui/src/lib/latest.test.ts`

**Interfaces:**
- Produces: `interface Place { landmark: string | null; locality: string | null }` and `placeFor(lat, lon): Promise<Place | null>` (api.ts); `tileTemplate(isWindows?): string`; `placeLines(p: Place | null): { primary: string | null; secondary: string | null }`; `createLatest(): { next(): number; isCurrent(token: number): boolean }`.

- [ ] **Step 1: Write the failing tests**

`placeLines.test.ts`:

```ts
import { describe, it, expect } from 'vitest';
import { placeLines } from './placeLines';

describe('placeLines', () => {
  it('shows a landmark with its locality underneath', () => {
    expect(placeLines({ landmark: 'Colosseum', locality: 'Roma, Lazio, Italia' })).toEqual({
      primary: 'Colosseum',
      secondary: 'Roma, Lazio, Italia',
    });
  });
  it('shows just the locality when there is no landmark', () => {
    expect(placeLines({ landmark: null, locality: 'Paris, France' })).toEqual({ primary: 'Paris, France', secondary: null });
  });
  it('shows a landmark alone when the locality is unknown', () => {
    expect(placeLines({ landmark: 'Bellagio', locality: null })).toEqual({ primary: 'Bellagio', secondary: null });
  });
  it('shows nothing for no place', () => {
    expect(placeLines(null)).toEqual({ primary: null, secondary: null });
  });
});
```

`latest.test.ts`:

```ts
import { describe, it, expect } from 'vitest';
import { createLatest } from './latest';

describe('createLatest', () => {
  it('only the newest token is current', () => {
    const l = createLatest();
    const a = l.next();
    expect(l.isCurrent(a)).toBe(true);
    const b = l.next();
    expect(l.isCurrent(a)).toBe(false);
    expect(l.isCurrent(b)).toBe(true);
  });
});
```

Append to `url.test.ts` (import `tileTemplate` alongside `thumbUrl`):

```ts
describe('tileTemplate', () => {
  it('uses the windows host on windows', () => {
    expect(tileTemplate(true)).toBe('http://pixie.localhost/tile/{z}/{x}/{y}');
  });
  it('uses the pixie scheme elsewhere', () => {
    expect(tileTemplate(false)).toBe('pixie://localhost/tile/{z}/{x}/{y}');
  });
});
```

- [ ] **Step 2: Run to verify failure**

Run: `npm --prefix ui test`
Expected: FAIL (modules / export missing).

- [ ] **Step 3: Implement**

`latest.ts`:

```ts
/** Guards async results: only the response for the most recent request may be applied. */
export function createLatest() {
  let n = 0;
  return { next: () => ++n, isCurrent: (token: number) => token === n };
}
```

`placeLines.ts`:

```ts
import type { Place } from './api';

export interface PlaceLines {
  primary: string | null;
  secondary: string | null;
}

export function placeLines(p: Place | null): PlaceLines {
  if (!p) return { primary: null, secondary: null };
  if (p.landmark) return { primary: p.landmark, secondary: p.locality };
  return { primary: p.locality, secondary: null };
}
```

`url.ts` append:

```ts
/** Leaflet tile URL template served by the backend through the pixie: scheme. */
export function tileTemplate(isWindows: boolean = /Windows/i.test(navigator.userAgent)): string {
  const base = isWindows ? 'http://pixie.localhost' : 'pixie://localhost';
  return `${base}/tile/{z}/{x}/{y}`;
}
```

`api.ts`: add (match the file's existing `invoke` style)

```ts
export interface Place {
  landmark: string | null;
  locality: string | null;
}

export const placeFor = (lat: number, lon: number): Promise<Place | null> => invoke('place_for', { lat, lon });
```

- [ ] **Step 4: Run to verify pass**

Run: `npm --prefix ui test`
Expected: all pass (48 + 7 = 55).

---

### Task 6: Map component, details integration, verification

**Files:**
- Modify: `ui/package.json` (via npm), `ui/src/lib/Details.svelte`
- Create: `ui/src/lib/LocationMap.svelte`
- Modify (end): `docs/superpowers/specs/2026-10-16-location-in-details-design.md` only after the user confirms it works.

**Interfaces:**
- Consumes: `placeFor`, `Place` (api.ts), `placeLines`, `createLatest`, `tileTemplate` (Task 5).

- [ ] **Step 1: Install Leaflet**

Run: `npm --prefix ui install leaflet@1.9.4` then `npm --prefix ui install -D @types/leaflet`
Expected: both added to `ui/package.json`.

- [ ] **Step 2: Create `LocationMap.svelte`**

```svelte
<script lang="ts">
  import L from 'leaflet';
  import 'leaflet/dist/leaflet.css';
  import { onMount } from 'svelte';
  import { tileTemplate } from './url';

  let { lat, lon }: { lat: number; lon: number } = $props();

  let el: HTMLDivElement;
  let unavailable = $state(false);
  let map: L.Map | undefined;
  let marker: L.CircleMarker | undefined;
  let loaded = 0; // tiles shown for the current location

  onMount(() => {
    // No default attribution control: its link would navigate the webview. We show plain text below.
    map = L.map(el, { attributionControl: false }).setView([lat, lon], 15);
    const layer = L.tileLayer(tileTemplate(), { maxZoom: 19 }).addTo(map);
    layer.on('tileload', () => {
      loaded++;
      unavailable = false;
    });
    layer.on('tileerror', () => {
      if (loaded === 0) unavailable = true;
    });
    marker = L.circleMarker([lat, lon], { radius: 8, color: '#fff', weight: 2, fillColor: '#d33', fillOpacity: 1 }).addTo(map);
    // The pane can be resized by dragging the divider.
    const ro = new ResizeObserver(() => map?.invalidateSize());
    ro.observe(el);
    return () => {
      ro.disconnect();
      map?.remove();
      map = undefined;
    };
  });

  $effect(() => {
    const la = lat; // read first so the effect tracks them even before the map exists
    const lo = lon;
    if (map && marker) {
      loaded = 0;
      unavailable = false;
      map.setView([la, lo], 15);
      marker.setLatLng([la, lo]);
    }
  });
</script>

<div class="wrap">
  <div class="map" bind:this={el}></div>
  {#if unavailable}<div class="off">Map unavailable</div>{/if}
  <div class="attr">© OpenStreetMap contributors</div>
</div>

<style>
  .wrap { position: relative; margin: 0 0 8px; }
  .map { height: 220px; border-radius: 6px; overflow: hidden; background: #e8e8e8; }
  .off { position: absolute; inset: 0 0 18px 0; display: grid; place-items: center; color: #666; font-size: 13px; pointer-events: none; }
  .attr { font-size: 11px; color: #888; margin-top: 2px; }
</style>
```

- [ ] **Step 3: Edit `Details.svelte`**

Replace the `<script>` block with:

```svelte
<script lang="ts">
  import { placeFor, type Photo, type Place } from './api';
  import { basename, dirname, formatBytes, formatDateTime, formatGps } from './format';
  import { createLatest } from './latest';
  import { placeLines } from './placeLines';
  import LocationMap from './LocationMap.svelte';

  let { photo, count }: { photo: Photo | null; count: number } = $props();
  const gps = $derived(photo ? formatGps(photo.gps_lat, photo.gps_lon) : null);
  const lat = $derived(photo?.gps_lat ?? null);
  const lon = $derived(photo?.gps_lon ?? null);

  let place = $state<Place | null>(null);
  const latest = createLatest();
  const lines = $derived(placeLines(place));

  // Look the place up when the selected photo's location changes. A slow answer for a
  // photo that is no longer selected is dropped. Offline or failed: show nothing.
  $effect(() => {
    place = null;
    const token = latest.next();
    if (lat == null || lon == null) return;
    placeFor(lat, lon)
      .then((p) => {
        if (latest.isCurrent(token)) place = p;
      })
      .catch(() => {});
  });
</script>
```

In the markup, directly after `</dl>` (inside the `{:else if photo}` branch) add:

```svelte
    {#if lat != null && lon != null}
      <LocationMap {lat} {lon} />
      {#if lines.primary}<div class="place"><strong>{lines.primary}</strong>{#if lines.secondary}<br />{lines.secondary}{/if}</div>{/if}
    {/if}
```

Add style: `.place { margin: 0 0 16px; }`.

- [ ] **Step 4: Verify the UI builds**

Run: `npm --prefix ui test`, `npm --prefix ui run check`, `npm --prefix ui run build`
Expected: tests pass; svelte-check 0 errors / 0 warnings; build succeeds.

- [ ] **Step 5: Whole-workspace check**

Run: `cargo test --workspace` and `cargo build -p pixie-app` (env from Global Constraints).
Expected: all pass.

- [ ] **Step 6: Manual check (the user runs it: `just winlaunch`)**

1. Select a photo with GPS: map appears with a red pin; landmark/locality lines appear a moment later.
2. Select a photo without GPS or several photos: no map.
3. Click through photos quickly: the shown place always matches the selected photo.
4. Drag the right divider: the map fills the pane without grey strips.
5. Disconnect the network and select a photo at a not-yet-viewed location: "Map unavailable", coordinates still listed, no popups.
6. Reconnect, select again: the map returns.

- [ ] **Step 7: Final whole-branch review and spec note**

Dispatch the final reviewer per the executing-plans skill. After the user confirms Step 6, add "Verified on the real library on <date>" to the spec's open-items list. Do not commit.
