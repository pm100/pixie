# App Shell and Date Tree Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the single tile grid with a three-pane shell (tree, grouped grid, details) and a Year > Month > Day date tree, with newest/oldest sort and folder filtering.

**Architecture:** The backend gains a `PhotoQuery` (sort order + optional folder) used by `list_photos`, `count_photos` and a new `day_groups` query. The UI builds both the tree and the grid's heading rows from the day groups. All new layout, tree, selection and formatting logic is pure TypeScript with unit tests; Svelte components stay thin. Photo thumbnails and paging keep the existing `PageCache`/`pixie://` pipeline.

**Tech Stack:** Rust (rusqlite, kamadak-exif already in use), Tauri 2, Svelte 5 + TypeScript, vitest.

**Spec:** `docs/superpowers/specs/2026-10-16-shell-and-date-tree-design.md`

## Global Constraints

- **Do not commit or push.** `AGENTS.md`: never commit or push unless the user says so. Leave all changes in the working tree; steps below say "Checkpoint" where a commit would normally go.
- Use `npm run tauri ...` (local v2 CLI), never the global `cargo tauri` (v1).
- Never run plain `cargo update` (Cargo.lock was generated with `CARGO_RESOLVER_INCOMPATIBLE_RUST_VERSIONS=fallback`).
- Full workspace Rust runs need `$env:VCPKG_ROOT='C:/work/vcpkg-pixie'; $env:VCPKGRS_TRIPLET='x64-windows-static-md'` (PowerShell). Crate-level runs (`-p pixie-core`, `-p pixie-index`, `-p pixie-image`) do not.
- UI commands run from the repo root: `npm --prefix ui test`, `npm --prefix ui run check`, `npm --prefix ui run build`.
- Never kill the user's running `pixie-app.exe`. Do not touch `%APPDATA%\app.pixie.desktop\`.
- Match existing style: short comments that explain why, no emoji, Svelte 5 runes.
- Write multi-line files with the Write tool, not heredocs.

## Review Focus

- A library with no dated photos: the tree shows only "Undated" and the grid shows one "Undated" heading (Task 3 and 4 tests).
- A day with exactly `cols` photos, and `cols + 1`: row count must not be off by one (Task 3 test).
- Empty layout (no groups, e.g. a folder with no photos): no crash, height 0, no visible rows (Task 3 test; Task 1 test for empty groups).
- Photos with GPS `(0, 0)` or out of range must not appear to have a location (Task 1 migration test, Task 2 test).
- Changing sort or folder while an earlier request is still in flight must not show stale groups (Task 6, manual check).

---

### Task 1: Backend queries, sort order and V2 migration

**Files:**
- Modify: `crates/pixie-core/src/lib.rs`
- Modify: `crates/pixie-index/src/db.rs`
- Modify: `src-tauri/src/lib.rs`

**Interfaces:**
- Produces (pixie-core): `enum SortOrder { Newest, Oldest }` (serde lowercase, default `Newest`), `struct PhotoQuery { order: SortOrder, folder_id: Option<FolderId> }` (Default), `struct DayGroup { day: Option<String>, count: i64, first_index: i64 }` (Serialize, Deserialize).
- Produces (pixie-index `Db`): `list_photos_in(&PhotoQuery, offset: i64, limit: i64) -> Result<Vec<Photo>>`, `count_photos_in(&PhotoQuery) -> Result<i64>`, `day_groups(&PhotoQuery) -> Result<Vec<DayGroup>>`. Existing `list_photos(offset, limit)` and `count_photos()` stay and delegate with the default query.
- Produces (Tauri): `list_photos(offset, limit, order?, folder_id?)`, `count_photos(folder_id?)`, `list_day_groups(order?, folder_id?)`. Absent `order` means newest, absent `folder_id` means all folders, so the current UI keeps working.

- [ ] **Step 1: Add the shared types to pixie-core**

In `crates/pixie-core/src/lib.rs`, after the `Folder` struct, add:

```rust
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SortOrder {
    #[default]
    Newest,
    Oldest,
}

/// What the library view is showing: a sort order and an optional folder filter.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PhotoQuery {
    pub order: SortOrder,
    pub folder_id: Option<FolderId>,
}

/// All photos taken on one day (`day` is "YYYY-MM-DD"; `None` = no date).
/// `first_index` is the position of the group's first photo in the listing order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DayGroup {
    pub day: Option<String>,
    pub count: i64,
    pub first_index: i64,
}
```

Add this test inside the existing `mod tests` in the same file:

```rust
    #[test]
    fn sort_order_serializes_lowercase_and_defaults_to_newest() {
        assert_eq!(serde_json::to_string(&SortOrder::Oldest).unwrap(), "\"oldest\"");
        assert_eq!(serde_json::from_str::<SortOrder>("\"newest\"").unwrap(), SortOrder::Newest);
        assert_eq!(PhotoQuery::default().order, SortOrder::Newest);
        assert_eq!(PhotoQuery::default().folder_id, None);
    }
```

- [ ] **Step 2: Run it**

Run: `cargo test -p pixie-core`
Expected: PASS (all pixie-core tests, including the new one).

- [ ] **Step 3: Write the failing db tests**

In `crates/pixie-index/src/db.rs`, change the first import line to:

```rust
use pixie_core::{DayGroup, Folder, FolderId, Photo, PhotoId, PhotoMeta, PhotoQuery, PixieError, Result, SortOrder};
```

Append these tests inside `mod tests` (after `unicode_and_spaces_round_trip`):

```rust
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

    fn names(db: &Db, q: &PhotoQuery) -> Vec<String> {
        db.list_photos_in(q, 0, 100).unwrap().into_iter().map(|p| p.path.rsplit('/').next().unwrap().to_string()).collect()
    }

    fn group(day: Option<&str>, count: i64, first_index: i64) -> DayGroup {
        DayGroup { day: day.map(String::from), count, first_index }
    }

    #[test]
    fn newest_lists_and_groups_newest_first_with_undated_last() {
        let (db, _, _) = dated_library();
        let q = PhotoQuery::default();
        assert_eq!(names(&db, &q), ["b.jpg", "a.jpg", "f.jpg", "c.jpg", "e.jpg", "d.jpg"]);
        assert_eq!(
            db.day_groups(&q).unwrap(),
            [group(Some("2024-06-03"), 2, 0), group(Some("2024-06-02"), 1, 2), group(Some("2024-06-01"), 1, 3), group(None, 2, 4)]
        );
    }

    #[test]
    fn oldest_lists_and_groups_oldest_first_but_undated_still_last() {
        let (db, _, _) = dated_library();
        let q = PhotoQuery { order: SortOrder::Oldest, folder_id: None };
        assert_eq!(names(&db, &q), ["c.jpg", "f.jpg", "a.jpg", "b.jpg", "d.jpg", "e.jpg"]);
        assert_eq!(
            db.day_groups(&q).unwrap(),
            [group(Some("2024-06-01"), 1, 0), group(Some("2024-06-02"), 1, 1), group(Some("2024-06-03"), 2, 2), group(None, 2, 4)]
        );
    }

    #[test]
    fn group_counts_add_up_and_first_index_points_at_the_group() {
        let (db, _, _) = dated_library();
        for order in [SortOrder::Newest, SortOrder::Oldest] {
            let q = PhotoQuery { order, folder_id: None };
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
        let q = PhotoQuery { order: SortOrder::Newest, folder_id: Some(g) };
        assert_eq!(names(&db, &q), ["f.jpg"]);
        assert_eq!(db.count_photos_in(&q).unwrap(), 1);
        assert_eq!(db.day_groups(&q).unwrap(), [group(Some("2024-06-02"), 1, 0)]);
        let q = PhotoQuery { order: SortOrder::Newest, folder_id: Some(f) };
        assert_eq!(db.count_photos_in(&q).unwrap(), 5);
    }

    #[test]
    fn empty_folder_and_empty_library_have_no_groups() {
        let db = Db::open_in_memory().unwrap();
        let f = db.add_folder("/empty").unwrap();
        let q = PhotoQuery { order: SortOrder::Newest, folder_id: Some(f) };
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
    fn unfiltered_listing_is_served_from_an_index_in_both_directions() {
        let (db, _, _) = dated_library();
        for order in [SortOrder::Newest, SortOrder::Oldest] {
            let q = PhotoQuery { order, folder_id: None };
            let c = db.conn();
            let mut s = c.prepare(&format!("EXPLAIN QUERY PLAN {}", list_sql(&q))).unwrap();
            let (limit, offset) = (10i64, 0i64);
            let params: Vec<(&str, &dyn rusqlite::ToSql)> = vec![(":limit", &limit as &dyn rusqlite::ToSql), (":offset", &offset as &dyn rusqlite::ToSql)];
            let plan: Vec<String> = s.query_map(params.as_slice(), |r| r.get::<_, String>(3)).unwrap().map(|r| r.unwrap()).collect();
            let plan = plan.join(" | ");
            assert!(plan.contains("idx_photos_"), "{order:?} should use an index: {plan}");
            assert!(!plan.contains("TEMP B-TREE"), "{order:?} must not sort: {plan}");
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
```

- [ ] **Step 4: Run to verify they fail**

Run: `cargo test -p pixie-index db::`
Expected: FAIL to compile (`list_photos_in`, `count_photos_in`, `day_groups`, `list_sql` not found).

- [ ] **Step 5: Add the V2 migration**

In `db.rs`, extend `MIGRATIONS` (add a second string after the V1 string, inside the slice):

```rust
    // V2: Undated-last ordering in both directions served by an index; GPS sanity.
    "DROP INDEX idx_photos_list;
     CREATE INDEX idx_photos_newest ON photos(missing, (taken_at IS NULL), taken_at DESC, id DESC);
     CREATE INDEX idx_photos_oldest ON photos(missing, (taken_at IS NULL), taken_at ASC, id ASC);
     UPDATE photos SET gps_lat = NULL, gps_lon = NULL
      WHERE gps_lat IS NULL OR gps_lon IS NULL
         OR abs(gps_lat) > 90 OR abs(gps_lon) > 180
         OR (gps_lat = 0 AND gps_lon = 0);",
```

- [ ] **Step 6: Implement the queries**

In `db.rs`, add these free functions just above `impl Db`:

```rust
fn order_sql(o: SortOrder) -> &'static str {
    match o {
        SortOrder::Newest => "taken_at IS NULL, taken_at DESC, id DESC",
        SortOrder::Oldest => "taken_at IS NULL, taken_at ASC, id ASC",
    }
}

fn folder_sql(q: &PhotoQuery) -> &'static str {
    if q.folder_id.is_some() { " AND folder_id = :folder" } else { "" }
}

fn folder_param(q: &PhotoQuery) -> Vec<(&'static str, &dyn rusqlite::ToSql)> {
    match &q.folder_id {
        Some(f) => vec![(":folder", f as &dyn rusqlite::ToSql)],
        None => vec![],
    }
}

fn list_sql(q: &PhotoQuery) -> String {
    format!(
        "SELECT {PHOTO_COLS} FROM photos WHERE missing = 0{} ORDER BY {} LIMIT :limit OFFSET :offset",
        folder_sql(q),
        order_sql(q.order)
    )
}
```

Replace the existing `list_photos` and `count_photos` methods in `impl Db` with:

```rust
    pub fn list_photos(&self, offset: i64, limit: i64) -> Result<Vec<Photo>> {
        self.list_photos_in(&PhotoQuery::default(), offset, limit)
    }

    pub fn list_photos_in(&self, q: &PhotoQuery, offset: i64, limit: i64) -> Result<Vec<Photo>> {
        let c = self.conn();
        let mut s = c.prepare(&list_sql(q)).map_err(db_err)?;
        let mut params = folder_param(q);
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
        let sql = format!("SELECT COUNT(*) FROM photos WHERE missing = 0{}", folder_sql(q));
        self.conn()
            .query_row(&sql, folder_param(q).as_slice(), |r| r.get(0))
            .map_err(db_err)
    }

    /// One entry per calendar day (UTC, as stored), in listing order, Undated last.
    pub fn day_groups(&self, q: &PhotoQuery) -> Result<Vec<DayGroup>> {
        let dir = match q.order {
            SortOrder::Newest => "DESC",
            SortOrder::Oldest => "ASC",
        };
        let sql = format!(
            "SELECT CASE WHEN taken_at IS NULL THEN NULL ELSE date(taken_at, 'unixepoch') END AS day, COUNT(*)
             FROM photos WHERE missing = 0{} GROUP BY day ORDER BY day IS NULL, day {dir}",
            folder_sql(q)
        );
        let c = self.conn();
        let mut s = c.prepare(&sql).map_err(db_err)?;
        let rows = s
            .query_map(folder_param(q).as_slice(), |r| Ok((r.get::<_, Option<String>>(0)?, r.get::<_, i64>(1)?)))
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
```

- [ ] **Step 7: Run to verify they pass**

Run: `cargo test -p pixie-index`
Expected: PASS, including the 8 new tests and every existing scanner/db test. If `unfiltered_listing_is_served_from_an_index_in_both_directions` fails with a `TEMP B-TREE`, SQLite did not pick the expression index: fix the `CREATE INDEX` expressions or the `ORDER BY` so they match textually (`taken_at IS NULL` vs `(taken_at IS NULL)` is fine) before moving on; do not weaken the assertion.

- [ ] **Step 8: Expose the commands**

In `src-tauri/src/lib.rs`, change the import `use pixie_core::{Folder, Photo, PixieError};` to:

```rust
use pixie_core::{DayGroup, Folder, Photo, PhotoQuery, PixieError, SortOrder};
```

Replace the `list_photos` and `count_photos` commands with:

```rust
fn query_of(order: Option<SortOrder>, folder_id: Option<i64>) -> PhotoQuery {
    PhotoQuery { order: order.unwrap_or_default(), folder_id }
}

#[tauri::command]
fn list_photos(
    state: tauri::State<AppState>,
    offset: i64,
    limit: i64,
    order: Option<SortOrder>,
    folder_id: Option<i64>,
) -> Result<Vec<Photo>, PixieError> {
    state.db.list_photos_in(&query_of(order, folder_id), offset.max(0), limit.clamp(1, 1000))
}

#[tauri::command]
fn count_photos(state: tauri::State<AppState>, folder_id: Option<i64>) -> Result<i64, PixieError> {
    state.db.count_photos_in(&query_of(None, folder_id))
}

#[tauri::command]
fn list_day_groups(
    state: tauri::State<AppState>,
    order: Option<SortOrder>,
    folder_id: Option<i64>,
) -> Result<Vec<DayGroup>, PixieError> {
    state.db.day_groups(&query_of(order, folder_id))
}
```

Add `list_day_groups` to the `generate_handler!` list.

- [ ] **Step 9: Build the app crate**

Run (PowerShell): `$env:VCPKG_ROOT='C:/work/vcpkg-pixie'; $env:VCPKGRS_TRIPLET='x64-windows-static-md'; cargo build -p pixie-app`
Expected: builds with no errors. (Run in the background; the first build after a change can take minutes.)

- [ ] **Step 10: Checkpoint (no commit)**

Run: `git status --short` and confirm only the three files above changed. Do not commit.

---

### Task 2: Reject bad GPS values at scan time

**Files:**
- Modify: `crates/pixie-image/src/meta.rs`

**Interfaces:**
- Produces: `fn sanitize_gps(lat: Option<f64>, lon: Option<f64>) -> (Option<f64>, Option<f64>)` (private to `meta.rs`), used by `read_meta`.

- [ ] **Step 1: Write the failing test**

Append inside `mod tests` in `crates/pixie-image/src/meta.rs`:

```rust
    #[test]
    fn gps_must_be_a_complete_in_range_non_zero_fix() {
        assert_eq!(sanitize_gps(Some(51.5), Some(-0.12)), (Some(51.5), Some(-0.12)));
        assert_eq!(sanitize_gps(Some(-90.0), Some(180.0)), (Some(-90.0), Some(180.0)));
        assert_eq!(sanitize_gps(Some(0.0), Some(0.0)), (None, None)); // camera "no fix"
        assert_eq!(sanitize_gps(Some(0.0), Some(10.0)), (Some(0.0), Some(10.0))); // the equator is real
        assert_eq!(sanitize_gps(Some(90.5), Some(10.0)), (None, None));
        assert_eq!(sanitize_gps(Some(10.0), Some(-180.5)), (None, None));
        assert_eq!(sanitize_gps(Some(10.0), None), (None, None));
        assert_eq!(sanitize_gps(None, Some(10.0)), (None, None));
        assert_eq!(sanitize_gps(Some(f64::NAN), Some(10.0)), (None, None));
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p pixie-image gps_must_be`
Expected: FAIL to compile (`sanitize_gps` not found).

- [ ] **Step 3: Implement**

In `meta.rs`, just above `read_meta`, add:

```rust
/// Both coordinates present, finite and in range, and not the (0, 0) "no fix" value
/// many cameras write; otherwise neither is kept.
fn sanitize_gps(lat: Option<f64>, lon: Option<f64>) -> (Option<f64>, Option<f64>) {
    match (lat, lon) {
        (Some(a), Some(o))
            if a.is_finite() && o.is_finite() && a.abs() <= 90.0 && o.abs() <= 180.0 && !(a == 0.0 && o == 0.0) =>
        {
            (Some(a), Some(o))
        }
        _ => (None, None),
    }
}
```

In `read_meta`, replace the two `meta.gps_lat = ...` / `meta.gps_lon = ...` lines with:

```rust
        (meta.gps_lat, meta.gps_lon) = sanitize_gps(
            gps_coord(e, exif::Tag::GPSLatitude, exif::Tag::GPSLatitudeRef),
            gps_coord(e, exif::Tag::GPSLongitude, exif::Tag::GPSLongitudeRef),
        );
```

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p pixie-image`
Expected: PASS (all pixie-image tests; the HEIC test only runs with the `heic` feature).

- [ ] **Step 5: Checkpoint (no commit)**

`git status --short` shows only `meta.rs` changed in this task.

---

### Task 3: Group-aware grid layout (pure module)

**Files:**
- Modify: `ui/src/lib/api.ts`
- Modify: `ui/src/lib/gridMath.ts`, `ui/src/lib/gridMath.test.ts`
- Create: `ui/src/lib/layout.ts`, `ui/src/lib/layout.test.ts`

**Interfaces:**
- Produces (api.ts): `SortOrder = 'newest' | 'oldest'`, `View = { order: SortOrder; folderId: number | null }`, `DayGroup = { day: string | null; count: number; first_index: number }`; `listPhotos(offset, limit, order = 'newest', folderId = null)`, `countPhotos(folderId = null)`, `listDayGroups(order, folderId)`.
- Produces (layout.ts): `Row`, `Layout`, `buildLayout(groups, cols, cellH, headH)`, `rowAt(layout, y)`, `visibleRows(layout, scrollTop, viewH, overscan = 2): [start, end]`, `groupAt(layout, y)`, `groupTop(layout, group)`.
- Produces (gridMath.ts): `clamp(v, min, max)`.

- [ ] **Step 1: Extend the API wrapper**

Replace the contents of `ui/src/lib/api.ts` with:

```ts
import { invoke } from '@tauri-apps/api/core';

export interface Photo {
  id: number; folder_id: number; path: string; size: number; mtime: number;
  width: number | null; height: number | null; taken_at: number | null;
  camera: string | null; gps_lat: number | null; gps_lon: number | null; missing: boolean;
}
export interface Folder { id: number; path: string }
export type SortOrder = 'newest' | 'oldest';
/** What the library is showing: a sort order and an optional folder filter. */
export interface View { order: SortOrder; folderId: number | null }
/** Photos taken on one day ("YYYY-MM-DD"; null = no date), in listing order. */
export interface DayGroup { day: string | null; count: number; first_index: number }

export const listFolders = () => invoke<Folder[]>('list_folders');
export const addFolder = (path: string) => invoke<number>('add_folder', { path });
export const startScan = (folderId: number | null) => invoke<void>('start_scan', { folderId });
export const cancelScan = () => invoke<void>('cancel_scan');
export const listPhotos = (offset: number, limit: number, order: SortOrder = 'newest', folderId: number | null = null) =>
  invoke<Photo[]>('list_photos', { offset, limit, order, folderId });
export const countPhotos = (folderId: number | null = null) => invoke<number>('count_photos', { folderId });
export const listDayGroups = (order: SortOrder = 'newest', folderId: number | null = null) =>
  invoke<DayGroup[]>('list_day_groups', { order, folderId });
export const printPage = () => invoke<void>('print_page');
```

- [ ] **Step 2: Write the failing tests**

Append to `ui/src/lib/gridMath.test.ts` (and change its import line to `import { clamp, columnsFor, visibleRange } from './gridMath';`):

```ts
describe('clamp', () => {
  it('limits a value to the range', () => {
    expect(clamp(5, 1, 10)).toBe(5);
    expect(clamp(-3, 1, 10)).toBe(1);
    expect(clamp(99, 1, 10)).toBe(10);
  });
});
```

Create `ui/src/lib/layout.test.ts`:

```ts
import { describe, expect, it } from 'vitest';
import type { DayGroup } from './api';
import { buildLayout, groupAt, groupTop, rowAt, visibleRows } from './layout';

const g = (count: number, first_index: number, day: string | null = '2024-06-01'): DayGroup => ({ day, count, first_index });

// cols 4, cell row 100, heading 30:
//   heading 0..30, cells(0,4) 30..130, cells(4,1) 130..230,
//   heading 230..260, cells(5,2) 260..360
const layout = () => buildLayout([g(5, 0), g(2, 5, '2024-05-31')], 4, 100, 30);

describe('buildLayout', () => {
  it('puts a heading before each group and wraps its photos into rows', () => {
    const l = layout();
    expect(l.rows.map((r) => [r.kind, r.top, r.h])).toEqual([
      ['heading', 0, 30], ['cells', 30, 100], ['cells', 130, 100], ['heading', 230, 30], ['cells', 260, 100],
    ]);
    const cells = l.rows.filter((r) => r.kind === 'cells');
    expect(cells.map((r) => r.kind === 'cells' && [r.first, r.n])).toEqual([[0, 4], [4, 1], [5, 2]]);
    expect(l.height).toBe(360);
    expect(l.groupTops).toEqual([0, 230]);
  });

  it('does not add a row when a day has exactly cols photos, and does when it has cols + 1', () => {
    expect(buildLayout([g(4, 0)], 4, 100, 30).rows).toHaveLength(2);
    expect(buildLayout([g(5, 0)], 4, 100, 30).rows).toHaveLength(3);
  });

  it('is empty for no groups and treats cols < 1 as 1', () => {
    expect(buildLayout([], 4, 100, 30)).toEqual({ rows: [], height: 0, groupTops: [] });
    expect(buildLayout([g(2, 0)], 0, 100, 30).rows).toHaveLength(3);
  });

  it('handles a library that is only Undated', () => {
    const l = buildLayout([g(6, 0, null)], 4, 100, 30);
    expect(l.rows).toHaveLength(3);
    expect(l.height).toBe(230);
  });
});

describe('rowAt / groupAt / groupTop', () => {
  it('finds the row containing a y offset, clamped to the ends', () => {
    const l = layout();
    expect([-5, 0, 35, 229, 230, 9999].map((y) => rowAt(l, y))).toEqual([0, 0, 1, 2, 3, 4]);
  });
  it('reports the group at an offset and where a group starts', () => {
    const l = layout();
    expect([0, 200, 230, 359].map((y) => groupAt(l, y))).toEqual([0, 0, 1, 1]);
    expect(groupTop(l, 1)).toBe(230);
    expect(groupTop(l, 99)).toBe(360); // past the end: bottom of the list
  });
  it('is safe on an empty layout', () => {
    const l = buildLayout([], 4, 100, 30);
    expect(rowAt(l, 10)).toBe(-1);
    expect(groupAt(l, 10)).toBe(-1);
    expect(groupTop(l, 0)).toBe(0);
  });
});

describe('visibleRows', () => {
  it('returns the rows overlapping the viewport plus overscan', () => {
    const l = layout();
    expect(visibleRows(l, 0, 100, 0)).toEqual([0, 2]);
    expect(visibleRows(l, 0, 100, 1)).toEqual([0, 3]);
    expect(visibleRows(l, 240, 50, 0)).toEqual([3, 5]);
    expect(visibleRows(l, 240, 50, 5)).toEqual([0, 5]);
  });
  it('is [0, 0] when there is nothing to show', () => {
    expect(visibleRows(buildLayout([], 4, 100, 30), 0, 500)).toEqual([0, 0]);
  });
});
```

- [ ] **Step 3: Run to verify they fail**

Run: `npm --prefix ui test`
Expected: FAIL (`./layout` cannot be resolved; `clamp` is not exported).

- [ ] **Step 4: Implement**

Append to `ui/src/lib/gridMath.ts`:

```ts
export function clamp(v: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, v));
}
```

Create `ui/src/lib/layout.ts`:

```ts
import type { DayGroup } from './api';

/** One horizontal strip of the grid: a day heading, or up to `cols` thumbnails. */
export type Row =
  | { kind: 'heading'; group: number; top: number; h: number }
  | { kind: 'cells'; group: number; top: number; h: number; first: number; n: number };

export interface Layout {
  rows: Row[];
  height: number;
  /** Scroll offset of each group's heading. */
  groupTops: number[];
}

/** Lay out day groups: a heading row, then the group's photos wrapped to `cols` per row. */
export function buildLayout(groups: DayGroup[], cols: number, cellH: number, headH: number): Layout {
  const per = Math.max(1, cols);
  const rows: Row[] = [];
  const groupTops: number[] = [];
  let top = 0;
  groups.forEach((g, group) => {
    groupTops.push(top);
    rows.push({ kind: 'heading', group, top, h: headH });
    top += headH;
    for (let k = 0; k < g.count; k += per) {
      rows.push({ kind: 'cells', group, top, h: cellH, first: g.first_index + k, n: Math.min(per, g.count - k) });
      top += cellH;
    }
  });
  return { rows, height: top, groupTops };
}

/** Index of the row containing offset `y` (clamped to the ends); -1 when there are no rows. */
export function rowAt(layout: Layout, y: number): number {
  const { rows } = layout;
  if (rows.length === 0) return -1;
  let lo = 0;
  let hi = rows.length - 1;
  while (lo < hi) {
    const mid = (lo + hi + 1) >> 1;
    if (rows[mid].top <= y) lo = mid;
    else hi = mid - 1;
  }
  return lo;
}

/** Rows [start, end) overlapping the viewport, widened by `overscan` rows each side. */
export function visibleRows(layout: Layout, scrollTop: number, viewH: number, overscan = 2): [number, number] {
  if (layout.rows.length === 0) return [0, 0];
  const first = rowAt(layout, scrollTop);
  const last = rowAt(layout, scrollTop + viewH);
  return [Math.max(0, first - overscan), Math.min(layout.rows.length, last + 1 + overscan)];
}

/** The group shown at offset `y`, or -1 when empty. */
export function groupAt(layout: Layout, y: number): number {
  const i = rowAt(layout, y);
  return i < 0 ? -1 : layout.rows[i].group;
}

/** Scroll offset of a group's heading; the bottom of the list when the group does not exist. */
export function groupTop(layout: Layout, group: number): number {
  return layout.groupTops[group] ?? layout.height;
}
```

- [ ] **Step 5: Run to verify they pass**

Run: `npm --prefix ui test`
Expected: PASS (existing 18 tests plus the new ones).

Then run: `npm --prefix ui run check`
Expected: 0 errors. (`App.svelte` and `Viewer.svelte` still call `listPhotos`/`countPhotos` with the old argument counts; the new parameters are optional, so this still type-checks.)

- [ ] **Step 6: Checkpoint (no commit)**

---

### Task 4: Date tree, formatting, selection and prefs (pure modules)

**Files:**
- Create: `ui/src/lib/format.ts`, `ui/src/lib/format.test.ts`
- Create: `ui/src/lib/tree.ts`, `ui/src/lib/tree.test.ts`
- Create: `ui/src/lib/selection.ts`, `ui/src/lib/selection.test.ts`
- Create: `ui/src/lib/prefs.ts`, `ui/src/lib/prefs.test.ts`

**Interfaces:**
- Consumes: `DayGroup` from `api.ts` (Task 3).
- Produces (format.ts): `weekday(day)`, `monthName(m)`, `formatDay(day | null)`, `formatDateTime(seconds | null)`, `formatBytes(n)`, `basename(path)`, `dirname(path)`, `formatGps(lat, lon)`.
- Produces (tree.ts): `TreeNode { key, label, count, firstGroup, lastGroup, children }`, `buildDateTree(groups): TreeNode[]`, `activeKeys(tree, group): Set<string>`.
- Produces (selection.ts): `Selection { picked: ReadonlySet<number>; anchor: number | null }`, `emptySelection`, `click(sel, index, mods)`, `single(sel): number | null`.
- Produces (prefs.ts): `loadPref(key, fallback, kv?)`, `savePref(key, value, kv?)`.

- [ ] **Step 1: Write the failing tests**

Create `ui/src/lib/format.test.ts`:

```ts
import { describe, expect, it } from 'vitest';
import { basename, dirname, formatBytes, formatDateTime, formatDay, formatGps, weekday } from './format';

describe('dates', () => {
  it('knows the weekday of a day string', () => {
    expect(weekday('2024-06-01')).toBe('Sat');
    expect(weekday('2024-06-03')).toBe('Mon');
    expect(weekday('2023-12-25')).toBe('Mon');
  });
  it('formats a day heading, and Undated for null', () => {
    expect(formatDay('2024-06-03')).toBe('Mon 3 Jun 2024');
    expect(formatDay(null)).toBe('Undated');
  });
  it('formats a timestamp (stored seconds) as day and minute, or Unknown', () => {
    expect(formatDateTime(1_717_243_200)).toBe('1 Jun 2024, 12:00');
    expect(formatDateTime(null)).toBe('Unknown');
  });
});

describe('sizes and paths', () => {
  it('formats byte counts', () => {
    expect(formatBytes(0)).toBe('0 B');
    expect(formatBytes(1023)).toBe('1023 B');
    expect(formatBytes(1536)).toBe('1.5 KB');
    expect(formatBytes(5 * 1024 * 1024)).toBe('5.0 MB');
  });
  it('splits Windows and POSIX paths', () => {
    expect(basename('C:\\Users\\p\\a.jpg')).toBe('a.jpg');
    expect(dirname('C:\\Users\\p\\a.jpg')).toBe('C:\\Users\\p');
    expect(basename('/x/y/z.jpg')).toBe('z.jpg');
    expect(dirname('/x/y/z.jpg')).toBe('/x/y');
    expect(basename('plain.jpg')).toBe('plain.jpg');
    expect(dirname('plain.jpg')).toBe('');
  });
});

describe('formatGps', () => {
  it('shows hemispheres to 4 decimals, or null when either is missing', () => {
    expect(formatGps(51.5007, -0.1246)).toBe('51.5007° N, 0.1246° W');
    expect(formatGps(-33.8688, 151.2093)).toBe('33.8688° S, 151.2093° E');
    expect(formatGps(null, 5)).toBeNull();
    expect(formatGps(5, null)).toBeNull();
  });
});
```

Create `ui/src/lib/tree.test.ts`:

```ts
import { describe, expect, it } from 'vitest';
import type { DayGroup } from './api';
import { activeKeys, buildDateTree } from './tree';

const g = (day: string | null, count: number, first_index: number): DayGroup => ({ day, count, first_index });

const groups = [g('2024-06-03', 2, 0), g('2024-06-01', 3, 2), g('2023-12-25', 1, 5), g(null, 4, 6)];

describe('buildDateTree', () => {
  it('nests year > month > day with counts and group ranges, Undated last', () => {
    const t = buildDateTree(groups);
    expect(t.map((n) => [n.key, n.label, n.count, n.firstGroup, n.lastGroup])).toEqual([
      ['y:2024', '2024', 5, 0, 1],
      ['y:2023', '2023', 1, 2, 2],
      ['undated', 'Undated', 4, 3, 3],
    ]);
    const june = t[0].children[0];
    expect([june.key, june.label, june.count, june.firstGroup, june.lastGroup]).toEqual(['m:2024-06', 'June', 5, 0, 1]);
    expect(june.children.map((d) => [d.key, d.label, d.count, d.firstGroup])).toEqual([
      ['d:2024-06-03', 'Mon 3', 2, 0],
      ['d:2024-06-01', 'Sat 1', 3, 1],
    ]);
    expect(t[1].children[0].children[0].label).toBe('Mon 25');
    expect(t[2].children).toEqual([]);
  });

  it('keeps the order it is given (oldest first)', () => {
    const t = buildDateTree([g('2023-12-25', 1, 0), g('2024-06-01', 3, 1), g('2024-06-03', 2, 4)]);
    expect(t.map((n) => n.label)).toEqual(['2023', '2024']);
    expect(t[1].children[0].children.map((d) => d.label)).toEqual(['Sat 1', 'Mon 3']);
  });

  it('is empty for no groups and shows only Undated when nothing is dated', () => {
    expect(buildDateTree([])).toEqual([]);
    const t = buildDateTree([g(null, 7, 0)]);
    expect(t.map((n) => [n.key, n.count])).toEqual([['undated', 7]]);
  });
});

describe('activeKeys', () => {
  it('lists the year, month and day containing a group', () => {
    const t = buildDateTree(groups);
    expect([...activeKeys(t, 1)].sort()).toEqual(['d:2024-06-01', 'm:2024-06', 'y:2024']);
    expect([...activeKeys(t, 3)]).toEqual(['undated']);
    expect(activeKeys(t, 99).size).toBe(0);
    expect(activeKeys(t, -1).size).toBe(0);
  });
});
```

Create `ui/src/lib/selection.test.ts`:

```ts
import { describe, expect, it } from 'vitest';
import { click, emptySelection, single } from './selection';

const none = { ctrl: false, shift: false };
const ctrl = { ctrl: true, shift: false };
const shift = { ctrl: false, shift: true };
const both = { ctrl: true, shift: true };
const list = (s: ReturnType<typeof click>) => [...s.picked].sort((a, b) => a - b);

describe('selection', () => {
  it('plain click selects only that photo', () => {
    const s = click(click(emptySelection, 3, none), 7, none);
    expect(list(s)).toEqual([7]);
    expect(s.anchor).toBe(7);
  });
  it('ctrl-click toggles one photo and moves the anchor', () => {
    let s = click(emptySelection, 3, none);
    s = click(s, 5, ctrl);
    expect(list(s)).toEqual([3, 5]);
    s = click(s, 3, ctrl);
    expect(list(s)).toEqual([5]);
    expect(s.anchor).toBe(3);
  });
  it('shift-click selects the range from the anchor, in either direction', () => {
    let s = click(emptySelection, 4, none);
    expect(list(click(s, 7, shift))).toEqual([4, 5, 6, 7]);
    expect(list(click(s, 2, shift))).toEqual([2, 3, 4]);
    expect(click(s, 7, shift).anchor).toBe(4); // anchor stays for the next range
  });
  it('shift-click replaces the previous range; ctrl+shift adds to the selection', () => {
    let s = click(emptySelection, 4, none);
    s = click(s, 6, shift);
    s = click(s, 5, shift);
    expect(list(s)).toEqual([4, 5]);
    s = click(click(emptySelection, 0, none), 10, ctrl);
    expect(list(click(s, 12, both))).toEqual([0, 10, 11, 12]);
  });
  it('shift-click with no anchor acts like a plain click', () => {
    expect(list(click(emptySelection, 5, shift))).toEqual([5]);
  });
  it('single returns the index only when exactly one is picked', () => {
    expect(single(emptySelection)).toBeNull();
    expect(single(click(emptySelection, 9, none))).toBe(9);
    expect(single(click(click(emptySelection, 1, none), 3, ctrl))).toBeNull();
  });
  it('never mutates the previous selection', () => {
    const a = click(emptySelection, 1, none);
    click(a, 2, ctrl);
    expect(list(a)).toEqual([1]);
    expect(emptySelection.picked.size).toBe(0);
  });
});
```

Create `ui/src/lib/prefs.test.ts`:

```ts
import { describe, expect, it } from 'vitest';
import { loadPref, savePref, type KV } from './prefs';

const fake = (init: Record<string, string> = {}): KV & { data: Record<string, string> } => ({
  data: init,
  getItem(k) { return k in this.data ? this.data[k] : null; },
  setItem(k, v) { this.data[k] = v; },
});

describe('prefs', () => {
  it('round-trips JSON values', () => {
    const kv = fake();
    savePref('w', 300, kv);
    savePref('o', 'oldest', kv);
    expect(loadPref('w', 0, kv)).toBe(300);
    expect(loadPref('o', 'newest', kv)).toBe('oldest');
  });
  it('falls back when missing, corrupt, or storage is unavailable', () => {
    expect(loadPref('x', 7, fake())).toBe(7);
    expect(loadPref('x', 7, fake({ x: '{not json' }))).toBe(7);
    expect(loadPref('x', 7, null)).toBe(7);
    const throwing: KV = { getItem() { throw new Error('blocked'); }, setItem() { throw new Error('blocked'); } };
    expect(loadPref('x', 7, throwing)).toBe(7);
    expect(() => savePref('x', 1, throwing)).not.toThrow();
    expect(() => savePref('x', 1, null)).not.toThrow();
  });
});
```

- [ ] **Step 2: Run to verify they fail**

Run: `npm --prefix ui test`
Expected: FAIL (the four new modules cannot be resolved).

- [ ] **Step 3: Implement format.ts**

Create `ui/src/lib/format.ts`:

```ts
const MONTHS = ['January', 'February', 'March', 'April', 'May', 'June', 'July', 'August', 'September', 'October', 'November', 'December'];
const WEEKDAYS = ['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat'];

const short = (m: number) => MONTHS[m].slice(0, 3);

/** "YYYY-MM-DD" -> [year, month 1-12, day], or null when malformed. */
function parts(day: string): [number, number, number] | null {
  const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(day);
  return m ? [Number(m[1]), Number(m[2]), Number(m[3])] : null;
}

export const monthName = (month: number): string => MONTHS[month - 1] ?? '';

export function weekday(day: string): string {
  const p = parts(day);
  return p ? WEEKDAYS[new Date(Date.UTC(p[0], p[1] - 1, p[2])).getUTCDay()] : '';
}

/** "Sat 3 Jun 2024"; "Undated" for no date. */
export function formatDay(day: string | null): string {
  if (day === null) return 'Undated';
  const p = parts(day);
  return p ? `${weekday(day)} ${p[2]} ${short(p[1] - 1)} ${p[0]}` : day;
}

/** Stored seconds (EXIF local time kept as if UTC) -> "1 Jun 2024, 12:00". */
export function formatDateTime(seconds: number | null): string {
  if (seconds === null) return 'Unknown';
  const d = new Date(seconds * 1000);
  const hh = String(d.getUTCHours()).padStart(2, '0');
  const mm = String(d.getUTCMinutes()).padStart(2, '0');
  return `${d.getUTCDate()} ${short(d.getUTCMonth())} ${d.getUTCFullYear()}, ${hh}:${mm}`;
}

export function formatBytes(n: number): string {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  if (n < 1024 * 1024 * 1024) return `${(n / (1024 * 1024)).toFixed(1)} MB`;
  return `${(n / (1024 * 1024 * 1024)).toFixed(1)} GB`;
}

const cut = (p: string) => Math.max(p.lastIndexOf('/'), p.lastIndexOf('\\'));
export const basename = (p: string): string => p.slice(cut(p) + 1);
export const dirname = (p: string): string => (cut(p) < 0 ? '' : p.slice(0, cut(p)));

export function formatGps(lat: number | null, lon: number | null): string | null {
  if (lat === null || lon === null) return null;
  return `${Math.abs(lat).toFixed(4)}° ${lat < 0 ? 'S' : 'N'}, ${Math.abs(lon).toFixed(4)}° ${lon < 0 ? 'W' : 'E'}`;
}
```

- [ ] **Step 4: Implement tree.ts**

Create `ui/src/lib/tree.ts`:

```ts
import type { DayGroup } from './api';
import { monthName, weekday } from './format';

export interface TreeNode {
  key: string;
  label: string;
  count: number;
  /** Range of day-group indexes this node covers, inclusive. */
  firstGroup: number;
  lastGroup: number;
  children: TreeNode[];
}

const node = (key: string, label: string, group: number): TreeNode =>
  ({ key, label, count: 0, firstGroup: group, lastGroup: group, children: [] });

/** Year > Month > Day nodes in the order given, then Undated. Groups must already be in listing order. */
export function buildDateTree(groups: DayGroup[]): TreeNode[] {
  const roots: TreeNode[] = [];
  let year: TreeNode | undefined;
  let month: TreeNode | undefined;
  let undated: TreeNode | undefined;

  const add = (n: TreeNode, g: DayGroup, group: number) => {
    n.count += g.count;
    n.lastGroup = group;
  };

  groups.forEach((g, i) => {
    if (g.day === null) {
      undated ??= node('undated', 'Undated', i);
      add(undated, g, i);
      return;
    }
    const [y, m, d] = g.day.split('-');
    if (!year || year.key !== `y:${y}`) {
      year = node(`y:${y}`, y, i);
      roots.push(year);
      month = undefined;
    }
    if (!month || month.key !== `m:${y}-${m}`) {
      month = node(`m:${y}-${m}`, monthName(Number(m)), i);
      year.children.push(month);
    }
    const day = node(`d:${g.day}`, `${weekday(g.day)} ${Number(d)}`, i);
    month.children.push(day);
    add(day, g, i);
    add(month, g, i);
    add(year, g, i);
  });

  if (undated) roots.push(undated);
  return roots;
}

/** Keys of every node (year, month, day) whose range contains `group`. */
export function activeKeys(tree: TreeNode[], group: number): Set<string> {
  const keys = new Set<string>();
  const walk = (nodes: TreeNode[]) => {
    for (const n of nodes) {
      if (group >= n.firstGroup && group <= n.lastGroup) {
        keys.add(n.key);
        walk(n.children);
      }
    }
  };
  walk(tree);
  return keys;
}
```

- [ ] **Step 5: Implement selection.ts**

Create `ui/src/lib/selection.ts`:

```ts
/** Selected photos by index in the current view. Cleared by the caller when the view changes. */
export interface Selection {
  picked: ReadonlySet<number>;
  anchor: number | null;
}

export const emptySelection: Selection = { picked: new Set(), anchor: null };

export function click(sel: Selection, index: number, mods: { ctrl: boolean; shift: boolean }): Selection {
  if (mods.shift && sel.anchor !== null) {
    const [a, b] = sel.anchor < index ? [sel.anchor, index] : [index, sel.anchor];
    const next = new Set<number>(mods.ctrl ? sel.picked : []);
    for (let i = a; i <= b; i++) next.add(i);
    return { picked: next, anchor: sel.anchor };
  }
  if (mods.ctrl) {
    const next = new Set(sel.picked);
    if (!next.delete(index)) next.add(index);
    return { picked: next, anchor: index };
  }
  return { picked: new Set([index]), anchor: index };
}

/** The picked index when exactly one photo is selected. */
export function single(sel: Selection): number | null {
  return sel.picked.size === 1 ? [...sel.picked][0] : null;
}
```

- [ ] **Step 6: Implement prefs.ts**

Create `ui/src/lib/prefs.ts`:

```ts
/** The slice of Storage we use, so tests can pass a fake. */
export interface KV {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
}

function browserStore(): KV | null {
  try {
    return globalThis.localStorage ?? null;
  } catch {
    return null; // blocked or unavailable: preferences are a convenience only
  }
}

export function loadPref<T>(key: string, fallback: T, kv: KV | null = browserStore()): T {
  try {
    const s = kv?.getItem(key);
    return s == null ? fallback : (JSON.parse(s) as T);
  } catch {
    return fallback;
  }
}

export function savePref(key: string, value: unknown, kv: KV | null = browserStore()): void {
  try {
    kv?.setItem(key, JSON.stringify(value));
  } catch {
    /* ignore: not worth failing the UI over */
  }
}
```

- [ ] **Step 7: Run to verify they pass**

Run: `npm --prefix ui test` then `npm --prefix ui run check`
Expected: all tests PASS; svelte-check 0 errors.

- [ ] **Step 8: Checkpoint (no commit)**

---

### Task 5: Grid with day headings, Tree, Details components

**Files:**
- Modify: `ui/src/lib/Grid.svelte`
- Create: `ui/src/lib/Tree.svelte`, `ui/src/lib/Details.svelte`

**Interfaces:**
- Consumes: `buildLayout`, `visibleRows`, `groupAt`, `groupTop` (Task 3); `buildDateTree`, `activeKeys`, `formatDay`, `formatDateTime`, `formatBytes`, `formatGps`, `basename`, `dirname` (Task 4); `View`, `DayGroup`, `Folder`, `Photo`, `listPhotos` (api).
- Produces:
  - `Grid` props: `groups: DayGroup[]`, `version: number`, `view: View`, `selected: ReadonlySet<number>`, `scrollTo: { group: number; n: number } | null`, `onopen(index)`, `onselect(index, mods: { ctrl: boolean; shift: boolean })`, `onvisiblegroup(group)`.
  - `Tree` props: `groups: DayGroup[]`, `folders: Folder[]`, `activeGroup: number`, `folderId: number | null`, `ongroup(group)`, `onfolder(id: number | null)`.
  - `Details` props: `photo: Photo | null`, `count: number`.

These are Svelte components with no practical unit test; verification is `svelte-check`, the production build, and the manual check in Task 7. The logic they use is already covered by Tasks 3-4.

- [ ] **Step 1: Rewrite Grid.svelte**

Replace the contents of `ui/src/lib/Grid.svelte` with:

```svelte
<script lang="ts">
  import { untrack } from 'svelte';
  import { listPhotos, type DayGroup, type Photo, type View } from './api';
  import { formatDay } from './format';
  import { columnsFor } from './gridMath';
  import { buildLayout, groupAt, groupTop, visibleRows } from './layout';
  import { PageCache } from './pageCache';
  import { retryDelay } from './retry';
  import { thumbUrl } from './url';

  let { groups, version, view, selected, scrollTo, onopen, onselect, onvisiblegroup }: {
    groups: DayGroup[];
    version: number;
    view: View;
    selected: ReadonlySet<number>;
    scrollTo: { group: number; n: number } | null;
    onopen: (index: number) => void;
    onselect: (index: number, mods: { ctrl: boolean; shift: boolean }) => void;
    onvisiblegroup: (group: number) => void;
  } = $props();

  const CELL = 180, GAP = 6, PAGE = 200, ROW_H = CELL + GAP, HEAD_H = 40;
  let width = $state(800), height = $state(600), scrollTop = $state(0);
  let scroller: HTMLDivElement;
  let tick = $state(0); // bumped whenever the page cache changes
  let broken = $state<Record<number, boolean>>({});
  let nonce = $state<Record<number, number>>({}); // bumps the URL to retry a failed thumbnail
  let attempts: Record<number, number> = {};
  // `view` is read at call time, so a fetch always uses the current order and folder.
  const cache = new PageCache<Photo>((o, l) => listPhotos(o, l, view.order, view.folderId), PAGE, () => tick++);

  // `_` is read only so the template re-renders when the cache changes.
  // A failed load is often just a busy queue: retry with backoff before giving up.
  function onImgError(i: number) {
    const n = attempts[i] ?? 0;
    attempts[i] = n + 1;
    const delay = retryDelay(n);
    if (delay === null) broken[i] = true;
    else setTimeout(() => (nonce[i] = (nonce[i] ?? 0) + 1), delay);
  }

  const item = (i: number, _: number) => cache.items[i];

  const cols = $derived(columnsFor(width, CELL, GAP));
  const layout = $derived(buildLayout(groups, cols, ROW_H, HEAD_H));
  const span = $derived(visibleRows(layout, scrollTop, height));
  const shown = $derived(layout.rows.slice(span[0], span[1]));
  const current = $derived(groupAt(layout, scrollTop));

  // Reset first, then load: both depend on `version`, and effects run in order,
  // so a version bump always refetches the visible pages.
  $effect(() => {
    version;
    untrack(() => { cache.reset(); broken = {}; nonce = {}; attempts = {}; });
  });

  $effect(() => {
    version;
    let lo = Infinity, hi = -1;
    for (const r of shown) {
      if (r.kind !== 'cells') continue;
      lo = Math.min(lo, r.first);
      hi = Math.max(hi, r.first + r.n);
    }
    if (hi > lo) cache.ensure(lo, hi);
  });

  // Tree clicks ask to jump to a group's heading.
  $effect(() => {
    const t = scrollTo;
    if (!t) return;
    untrack(() => { scroller.scrollTop = groupTop(layout, t.group); });
  });

  // Tell the tree which group is at the top, so it can highlight it.
  $effect(() => {
    if (current >= 0) onvisiblegroup(current);
  });

  function key(e: KeyboardEvent, i: number) {
    if (e.key === 'Enter') { e.preventDefault(); onopen(i); }
  }
</script>

<div class="scroller" bind:this={scroller} bind:clientWidth={width} bind:clientHeight={height}
     onscroll={(e) => (scrollTop = e.currentTarget.scrollTop)}>
  <div class="inner" style:height="{layout.height}px">
    {#each shown as row (row.kind === 'heading' ? `h${row.group}` : `c${row.first}`)}
      {#if row.kind === 'heading'}
        <div class="head" style:top="{row.top}px" style:height="{row.h}px">
          <strong>{formatDay(groups[row.group].day)}</strong>
          <span>{groups[row.group].count} {groups[row.group].count === 1 ? 'photo' : 'photos'}</span>
        </div>
      {:else}
        {#each { length: row.n } as _, k (row.first + k)}
          {@const i = row.first + k}
          {@const p = item(i, tick)}
          <button class="cell" class:sel={selected.has(i)}
                  style:top="{row.top}px" style:left="{k * (CELL + GAP)}px"
                  onclick={(e) => onselect(i, { ctrl: e.ctrlKey || e.metaKey, shift: e.shiftKey })}
                  ondblclick={() => onopen(i)} onkeydown={(e) => key(e, i)}
                  aria-label={p ? p.path : 'photo'} aria-pressed={selected.has(i)}>
            {#if p && !broken[i]}
              <img loading="lazy" alt="" src={thumbUrl('grid', p.id, p.mtime, p.size, undefined, nonce[i] ?? 0)} onerror={() => onImgError(i)} />
            {:else if broken[i]}
              <span class="ph">⚠︎</span>
            {/if}
          </button>
        {/each}
      {/if}
    {/each}
  </div>
</div>

<style>
  .scroller { height: 100%; overflow-y: auto; }
  .inner { position: relative; }
  .head { position: absolute; left: 0; right: 0; display: flex; align-items: baseline; gap: 10px; padding: 14px 4px 4px; box-sizing: border-box; border-bottom: 1px solid #e4e4e8; }
  .head span { color: #777; font-size: 13px; }
  .cell { position: absolute; width: 180px; height: 180px; padding: 0; border: 0; background: #e8e8ec; border-radius: 6px; overflow: hidden; cursor: pointer; outline-offset: 0; }
  .cell.sel { outline: 3px solid #2f6fed; }
  img { width: 100%; height: 100%; object-fit: cover; display: block; }
  .ph { display: grid; place-items: center; height: 100%; color: #888; font-size: 28px; }
</style>
```

- [ ] **Step 2: Create Tree.svelte**

Create `ui/src/lib/Tree.svelte`:

```svelte
<script lang="ts">
  import { untrack } from 'svelte';
  import type { DayGroup, Folder } from './api';
  import { basename } from './format';
  import { activeKeys, buildDateTree, type TreeNode } from './tree';

  let { groups, folders, activeGroup, folderId, ongroup, onfolder }: {
    groups: DayGroup[];
    folders: Folder[];
    activeGroup: number;
    folderId: number | null;
    ongroup: (group: number) => void;
    onfolder: (id: number | null) => void;
  } = $props();

  const tree = $derived(buildDateTree(groups));
  const active = $derived(activeKeys(tree, activeGroup));
  let open = $state<Record<string, boolean>>({});

  // Follow the grid: open the path to whatever is at the top of the scroll.
  $effect(() => {
    const keys = active;
    untrack(() => { for (const k of keys) open[k] = true; });
  });

  const toggle = (n: TreeNode) => (open[n.key] = !open[n.key]);
</script>

{#snippet branch(n: TreeNode, depth: number)}
  <div class="row" class:active={active.has(n.key) && n.children.length === 0}
       style:padding-left="{8 + depth * 14}px">
    {#if n.children.length}
      <button class="twisty" onclick={() => toggle(n)} aria-label={open[n.key] ? 'Collapse' : 'Expand'}>{open[n.key] ? '▾' : '▸'}</button>
    {:else}
      <span class="twisty"></span>
    {/if}
    <button class="label" onclick={() => ongroup(n.firstGroup)}>{n.label}</button>
    <span class="n">{n.count}</span>
  </div>
  {#if open[n.key]}
    {#each n.children as c (c.key)}
      {@render branch(c, depth + 1)}
    {/each}
  {/if}
{/snippet}

<nav>
  <h3>Dates</h3>
  {#each tree as n (n.key)}
    {@render branch(n, 0)}
  {:else}
    <p class="none">No photos yet.</p>
  {/each}

  <h3>Folders</h3>
  <button class="item" class:active={folderId === null} onclick={() => onfolder(null)}>All photos</button>
  {#each folders as f (f.id)}
    <button class="item" class:active={folderId === f.id} title={f.path} onclick={() => onfolder(f.id)}>{basename(f.path) || f.path}</button>
  {/each}

  <h3>Map</h3>
  <button class="item" disabled title="Coming soon">Map</button>
</nav>

<style>
  nav { padding: 6px 4px 16px; font-size: 14px; }
  h3 { margin: 14px 8px 4px; font-size: 11px; letter-spacing: .08em; text-transform: uppercase; color: #777; }
  .row { display: flex; align-items: center; gap: 2px; padding-right: 8px; border-radius: 4px; }
  .row.active { background: #e3ecff; }
  .twisty { width: 18px; flex: none; padding: 0; border: 0; background: none; cursor: pointer; color: #555; }
  .label { flex: 1; text-align: left; padding: 3px 4px; border: 0; background: none; cursor: pointer; font: inherit; }
  .n { color: #888; font-size: 12px; }
  .item { display: block; width: 100%; text-align: left; padding: 3px 12px; border: 0; background: none; cursor: pointer; font: inherit; border-radius: 4px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .item.active { background: #e3ecff; }
  .item:disabled { color: #aaa; cursor: default; }
  .none { margin: 6px 12px; color: #888; }
</style>
```

- [ ] **Step 3: Create Details.svelte**

Create `ui/src/lib/Details.svelte`:

```svelte
<script lang="ts">
  import type { Photo } from './api';
  import { basename, dirname, formatBytes, formatDateTime, formatGps } from './format';

  let { photo, count }: { photo: Photo | null; count: number } = $props();
  const gps = $derived(photo ? formatGps(photo.gps_lat, photo.gps_lon) : null);
</script>

<div class="details">
  {#if count > 1}
    <p class="hint"><strong>{count}</strong> photos selected</p>
  {:else if photo}
    <h3 title={photo.path}>{basename(photo.path)}</h3>
    <dl>
      <dt>Taken</dt><dd>{formatDateTime(photo.taken_at)}</dd>
      {#if photo.width && photo.height}<dt>Size</dt><dd>{photo.width} × {photo.height}</dd>{/if}
      <dt>File</dt><dd>{formatBytes(photo.size)}</dd>
      <dt>Folder</dt><dd class="path" title={dirname(photo.path)}>{dirname(photo.path)}</dd>
      {#if photo.camera}<dt>Camera</dt><dd>{photo.camera}</dd>{/if}
      {#if gps}<dt>Location</dt><dd>{gps}</dd>{/if}
    </dl>
  {:else}
    <p class="hint">Select a photo to see its details.</p>
  {/if}
  <button disabled={count === 0} title="Coming soon">Email…</button>
</div>

<style>
  .details { padding: 12px 14px; font-size: 14px; }
  h3 { margin: 0 0 10px; word-break: break-all; font-size: 15px; }
  dl { display: grid; grid-template-columns: auto 1fr; gap: 4px 12px; margin: 0 0 16px; }
  dt { color: #777; }
  dd { margin: 0; min-width: 0; }
  .path { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; direction: rtl; text-align: left; }
  .hint { color: #777; }
</style>
```

- [ ] **Step 4: Check types and build**

`App.svelte` still renders `<Grid {total} {version} onopen=... />`, which no longer matches, so type-checking fails until Task 6 wires it. To verify these components now, run only the build of the leaf files:

Run: `npm --prefix ui run check`
Expected: the only errors are in `src/App.svelte` (props of `Grid`). `Tree.svelte`, `Details.svelte` and `Grid.svelte` themselves report none. Proceed to Task 6, which resolves them; Task 6 re-runs the full check.

- [ ] **Step 5: Checkpoint (no commit)**

---

### Task 6: Shell, resizers and App wiring

**Files:**
- Create: `ui/src/lib/Resizer.svelte`, `ui/src/lib/Shell.svelte`
- Modify: `ui/src/App.svelte`
- Modify: `ui/src/lib/Viewer.svelte`

**Interfaces:**
- Consumes: Grid/Tree/Details (Task 5); `click`, `single`, `emptySelection` (Task 4); `loadPref`, `savePref`; `listDayGroups`, `countPhotos`, `listPhotos`, `listFolders` (api).
- Produces: `Viewer` gains a required prop `view: View` and uses it for its `listPhotos` call.

- [ ] **Step 1: Create Resizer.svelte**

Create `ui/src/lib/Resizer.svelte`:

```svelte
<script lang="ts">
  import { clamp } from './gridMath';

  let { width = $bindable(), min, max, side, onchange }: {
    width: number; min: number; max: number;
    /** Which side of the handle the pane is on: dragging right grows a left pane, shrinks a right one. */
    side: 'left' | 'right';
    onchange?: (width: number) => void;
  } = $props();

  let dragging = $state(false);
  let startX = 0, startW = 0;

  function down(e: PointerEvent) {
    dragging = true;
    startX = e.clientX;
    startW = width;
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
  }
  function move(e: PointerEvent) {
    if (!dragging) return;
    const dx = e.clientX - startX;
    width = clamp(startW + (side === 'left' ? dx : -dx), min, max);
  }
  function up() {
    if (!dragging) return;
    dragging = false;
    onchange?.(width);
  }
</script>

<div class="bar" class:dragging role="separator" aria-orientation="vertical"
     onpointerdown={down} onpointermove={move} onpointerup={up} onpointercancel={up}></div>

<style>
  .bar { cursor: col-resize; background: #e4e4e8; touch-action: none; }
  .bar:hover, .bar.dragging { background: #2f6fed; }
</style>
```

- [ ] **Step 2: Create Shell.svelte**

Create `ui/src/lib/Shell.svelte`:

```svelte
<script lang="ts">
  import type { Snippet } from 'svelte';
  import { loadPref, savePref } from './prefs';
  import Resizer from './Resizer.svelte';

  let { left, center, right }: { left: Snippet; center: Snippet; right: Snippet } = $props();

  let leftW = $state(loadPref('pane.left', 230));
  let rightW = $state(loadPref('pane.right', 280));
</script>

<div class="shell" style:grid-template-columns="{leftW}px 5px minmax(0, 1fr) 5px {rightW}px">
  <aside>{@render left()}</aside>
  <Resizer bind:width={leftW} min={160} max={480} side="left" onchange={(w) => savePref('pane.left', w)} />
  <div class="center">{@render center()}</div>
  <Resizer bind:width={rightW} min={200} max={520} side="right" onchange={(w) => savePref('pane.right', w)} />
  <aside>{@render right()}</aside>
</div>

<style>
  .shell { display: grid; height: 100%; min-height: 0; }
  aside { overflow-y: auto; min-width: 0; background: #fafafb; }
  .center { min-width: 0; min-height: 0; padding: 0 8px; }
</style>
```

- [ ] **Step 3: Make Viewer follow the current view**

In `ui/src/lib/Viewer.svelte`:

Change the import `import { listPhotos, type Photo } from './api';` to `import { listPhotos, type Photo, type View } from './api';`.

Change the props block to:

```ts
  let { index, total, view, onclose, onnav }: {
    index: number; total: number; view: View; onclose: () => void; onnav: (i: number) => void;
  } = $props();
```

Change `listPhotos(lo, hi - lo).then(` to `listPhotos(lo, hi - lo, view.order, view.folderId).then(`.

- [ ] **Step 4: Rewrite App.svelte**

Replace the contents of `ui/src/App.svelte` with:

```svelte
<script lang="ts">
  import { open } from '@tauri-apps/plugin-dialog';
  import { listen } from '@tauri-apps/api/event';
  import { onMount } from 'svelte';
  import {
    addFolder, cancelScan, countPhotos, listDayGroups, listFolders, listPhotos, startScan,
    type DayGroup, type Folder, type Photo, type SortOrder, type View,
  } from './lib/api';
  import Details from './lib/Details.svelte';
  import Grid from './lib/Grid.svelte';
  import PrintTest from './lib/PrintTest.svelte';
  import { loadPref, savePref } from './lib/prefs';
  import { click, emptySelection, single } from './lib/selection';
  import Shell from './lib/Shell.svelte';
  import Tree from './lib/Tree.svelte';
  import Viewer from './lib/Viewer.svelte';

  let view = $state<View>({ order: loadPref<SortOrder>('order', 'newest'), folderId: null });
  let total = $state(0), version = $state(0);
  let groups = $state<DayGroup[]>([]);
  let folders = $state<Folder[]>([]);
  let selection = $state(emptySelection);
  let selectedPhoto = $state<Photo | null>(null);
  let activeGroup = $state(0);
  let scrollTo = $state<{ group: number; n: number } | null>(null);
  let jumps = 0;
  let scanning = $state(false), seen = $state(0), message = $state('');
  let openIndex = $state<number | null>(null);
  let page = $state<'library' | 'print'>('library');

  const jump = (group: number) => (scrollTo = { group, n: ++jumps });

  /** Reload the count and day groups for the current view; the grid refetches on `version`. */
  async function refresh(toTop = false) {
    const v = { ...view };
    const [n, g] = await Promise.all([countPhotos(v.folderId), listDayGroups(v.order, v.folderId)]);
    if (v.order !== view.order || v.folderId !== view.folderId) return; // a newer request owns the screen
    total = n;
    groups = g;
    selection = emptySelection;
    version++;
    if (toTop) jump(0);
  }

  function setOrder(order: SortOrder) {
    if (order === view.order) return;
    view = { ...view, order };
    savePref('order', order);
    refresh(true);
  }

  function setFolder(folderId: number | null) {
    if (folderId === view.folderId) return;
    view = { ...view, folderId };
    refresh(true);
  }

  async function chooseFolder() {
    const dir = await open({ directory: true, multiple: false });
    if (typeof dir !== 'string') return;
    const id = await addFolder(dir);
    folders = await listFolders();
    message = '';
    scanning = true; seen = 0; // before the call, so a fast scan-done cannot be overwritten
    try {
      await startScan(id);
    } catch (e) {
      message = String(e);
      if (!/already running/.test(message)) scanning = false; // busy => the other scan is still going
    }
  }

  // Details for a single selected photo (fetched by index in the current view).
  $effect(() => {
    const i = single(selection);
    const { order, folderId } = view;
    if (i === null) { selectedPhoto = null; return; }
    let live = true;
    listPhotos(i, 1, order, folderId).then((r) => { if (live) selectedPhoto = r[0] ?? null; });
    return () => { live = false; };
  });

  onMount(() => {
    refresh();
    // Re-scan all known folders on launch to pick up changes.
    listFolders().then((fs) => {
      folders = fs;
      if (!fs.length) return;
      scanning = true;
      startScan(null).catch(() => (scanning = false));
    });
    const subs = [
      // During a scan only the count is refreshed; the grid resets on scan-done.
      listen<{ seen: number }>('scan-progress', (e) => { seen = e.payload.seen; countPhotos(view.folderId).then((n) => (total = n)); }),
      listen('scan-done', () => { scanning = false; refresh(); }),
      listen<{ message: string }>('scan-error', (e) => { message = e.payload.message; /* scan-done always follows */ }),
    ];
    return () => subs.forEach((s) => s.then((un) => un()));
  });
</script>

<main>
  <header>
    <strong>Pixie</strong>
    <button onclick={chooseFolder}>Add photo folder…</button>
    <button onclick={() => setOrder(view.order === 'newest' ? 'oldest' : 'newest')}>
      {view.order === 'newest' ? 'Newest first ↓' : 'Oldest first ↑'}
    </button>
    <button onclick={() => (page = page === 'print' ? 'library' : 'print')}>Print test</button>
    {#if scanning}
      <span>Scanning… {seen} photos</span>
      <button onclick={() => cancelScan()}>Stop</button>
    {:else}
      <span>{total} photos</span>
    {/if}
    {#if message}<span class="err">{message}</span>{/if}
  </header>
  <section>
    {#if page === 'print'}
      <PrintTest />
    {:else if folders.length === 0 && total === 0 && !scanning}
      <p class="empty">Add a folder of photos to get started.</p>
    {:else}
      <Shell>
        {#snippet left()}
          <Tree {groups} {folders} {activeGroup} folderId={view.folderId} ongroup={jump} onfolder={setFolder} />
        {/snippet}
        {#snippet center()}
          {#if total === 0 && !scanning}
            <p class="empty">No photos here yet.</p>
          {:else}
            <Grid {groups} {version} {view} selected={selection.picked} {scrollTo}
                  onopen={(i) => (openIndex = i)}
                  onselect={(i, mods) => (selection = click(selection, i, mods))}
                  onvisiblegroup={(g) => (activeGroup = g)} />
          {/if}
        {/snippet}
        {#snippet right()}
          <Details photo={selectedPhoto} count={selection.picked.size} />
        {/snippet}
      </Shell>
    {/if}
  </section>
  {#if openIndex !== null}
    <Viewer index={openIndex} {total} {view} onclose={() => (openIndex = null)} onnav={(i) => (openIndex = i)} />
  {/if}
</main>

<style>
  :global(body) { margin: 0; font-family: system-ui, sans-serif; }
  main { display: flex; flex-direction: column; height: 100vh; }
  header { display: flex; gap: 12px; align-items: center; padding: 10px 14px; border-bottom: 1px solid #ddd; }
  section { flex: 1; min-height: 0; }
  .empty { text-align: center; margin-top: 20vh; color: #666; }
  .err { color: #b00020; }
</style>
```

- [ ] **Step 5: Verify**

Run: `npm --prefix ui run check`
Expected: 0 errors, 0 warnings that were not there before.

Run: `npm --prefix ui test`
Expected: PASS, all UI tests.

Run: `npm --prefix ui run build`
Expected: builds with no errors.

- [ ] **Step 6: Checkpoint (no commit)**

---

### Task 7: Full verification on the real library

**Files:** none (verification; fix anything found in the file that owns it).

- [ ] **Step 1: Full automated pass**

Run (PowerShell, in the background):
`$env:VCPKG_ROOT='C:/work/vcpkg-pixie'; $env:VCPKGRS_TRIPLET='x64-windows-static-md'; cargo test --workspace`
Expected: all Rust tests PASS (previously 55; now more).

Run: `npm --prefix ui test`, `npm --prefix ui run check`, `npm --prefix ui run build`
Expected: all PASS / 0 errors.

- [ ] **Step 2: Check the user's app is not running before touching it**

Run (PowerShell): `Get-Process pixie-app -ErrorAction SilentlyContinue`
If it is running, do not stop it. Tell the user the manual check needs them to restart the app (`just winlaunch`) and ask them to look at the items below; do not delete their database.

- [ ] **Step 3: Measure the new queries on the real library**

Add `crates/pixie-index/examples/groups.rs`:

```rust
//! Time the day-group and paging queries on a real folder (in-memory database).
//! Usage: cargo run --release -p pixie-index --example groups --features pixie-image/heic -- <folder>
use pixie_core::{PhotoQuery, SortOrder};
use pixie_index::{scan_folder, Db};
use std::sync::atomic::AtomicBool;
use std::time::Instant;

fn main() {
    let dir = std::env::args().nth(1).expect("folder to scan");
    let db = Db::open_in_memory().unwrap();
    let f = db.add_folder(&dir).unwrap();
    scan_folder(&db, f, &AtomicBool::new(false), |_| {}).unwrap();
    for order in [SortOrder::Newest, SortOrder::Oldest] {
        let q = PhotoQuery { order, folder_id: None };
        let t = Instant::now();
        let groups = db.day_groups(&q).unwrap();
        println!("{order:?}: day_groups {:.1} ms, {} groups, {} photos", t.elapsed().as_secs_f64() * 1000.0, groups.len(), groups.iter().map(|g| g.count).sum::<i64>());
        let t = Instant::now();
        let page = db.list_photos_in(&q, 3000, 200).unwrap();
        println!("{order:?}: page of {} at offset 3000: {:.1} ms", page.len(), t.elapsed().as_secs_f64() * 1000.0);
        println!("  undated: {:?}", groups.last().filter(|g| g.day.is_none()).map(|g| g.count));
    }
}
```

Run: `cargo run --release -p pixie-index --example groups --features pixie-image/heic -- "C:\Users\paulm\OneDrive\Pictures"` (with the VCPKG variables set; run in the background).
Expected: `day_groups` under 50 ms and a 200-photo page under 20 ms in both directions; the group photo counts sum to the number of photos found. Report the numbers. If either is slower, stop and investigate before continuing.

- [ ] **Step 4: Manual UI checklist (needs the user, or a launched dev app if none is running)**

Tell the user to restart the app and confirm each, then record the answers:
1. Three panes show; dragging both dividers resizes them, and the widths are still there after a restart.
2. Grid shows day headings ("Sat 3 Jun 2024 · 14 photos") with thumbnails under each; scrolling stays smooth and no "!" tiles appear.
3. Clicking a year, month or day in the tree scrolls the grid to it; scrolling the grid moves the tree highlight and opens the path.
4. "Newest first / Oldest first" flips the order, scrolls to the top, and keeps Undated at the bottom; the choice persists after restart.
5. Clicking a folder under Folders filters the grid and the date tree; "All photos" restores them.
6. Click selects one photo and the right pane shows its details (including a location for a photo with GPS); ctrl-click and shift-click select several and the pane shows the count; double-click opens the viewer, whose arrow keys follow the same order and folder.
7. Photos with no date appear under "Undated" at the bottom.

Fix anything that fails in the file that owns it, re-run the affected tests, and re-ask only for the failed items.

- [ ] **Step 5: Update docs**

In `docs/superpowers/specs/2026-10-16-shell-and-date-tree-design.md`, add a final line under "Decisions made while planning": `- Verified on the real library on <date>: query timings and manual checklist results as reported in the session.` (fill in the actual date), only after Step 4 passed.

- [ ] **Step 6: Checkpoint (no commit)**

Report to the user: what was built, the timings, the checklist results, and that nothing is committed (`git status --short` output), waiting for their word before committing or pushing.
