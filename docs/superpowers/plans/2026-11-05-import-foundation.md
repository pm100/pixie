# Import foundation: temporary sources Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A card, USB stick or folder can be added to Pixie as a *temporary source*: indexed where it is (dates, sizes, dimensions only), kept out of every library listing, classified as new or already known, and removed cleanly. No UI, no copying yet (those are the next plan).

**Architecture:** A source is a `folders` row with `kind = 'source'`. Its photos get `photos.hidden = 2`, set atomically by a trigger. Pixie already leaves `hidden <> 0` photos out of Browse and everything built on it (the shared `Filter` and direct queries such as People, tags, trips and the map use `hidden = 0`), so sources inherit that coverage; the few queries that did not check `hidden` get `hidden < 2`. A source's own node lists `hidden = 2` photos by asking for its folder. All of it is in `pixie-index`, which already depends on `pixie-image` for reading metadata.

**Tech Stack:** Rust, rusqlite (`crates/pixie-index`), tempfile and `pixie-testutil` for tests.

**Spec:** `docs/superpowers/specs/2026-11-01-import-export-design.md` ("Two ways to look at a source", "A source, browsed in place", "Knowing what is new", "Architecture"). Issue #44. The next plan covers detecting drives, the Tauri commands, Keep, Private and the UI.

## Global Constraints

- Never commit or push; leave all changes uncommitted (AGENTS.md, CLAUDE.md).
- No compiler warnings (`cargo check --workspace --all-targets`); fix them, do not `#[allow]`. Delete dead code.
- Use the separate target dir (`CARGO_TARGET_DIR=target-claude`) and test only the changed crate while working (`cargo test -p pixie-index`); run the whole workspace once at the end.
- A source is temporary: its rows, picks, tags and thumbnails must all go when it is removed; nothing about it may remain in the library.
- Sources get **no analysis**: no hashing, embeddings, faces or tags while they are only on a source.
- Errors shown to the person use `PixieError::Invalid(..)` with a plain sentence (see `db.rs`).
- Line endings: some files are committed CRLF (see memory `feedback_python_edit_line_endings`): use Edit, or open with `newline=''`; check `git diff --stat`.

## Review Focus

- A source photo leaking into some library listing, count, facet, map or People query: the exclusion test (Task 1) must name every public function that answers a library question, and a new such function later must be added to it.
- A library folder and a source with the same path (adding a folder that is already open as a source, and the reverse).
- A source whose drive disappears mid-scan: the scan stops with an error, leaves what it found, and the source can still be removed.
- Removing a source leaves nothing behind: picks, user tags, search hits, embeddings, faces and scan rows for its photos are gone (check every table with a `photo_id`).
- A card with reused file names (camera numbering restarts): "kept before" needs the same path, size **and** modified time, so a new card with the same names is not wrongly known.

---

## File Structure

- Modify `crates/pixie-index/src/db.rs`: migrations V31 and V32, `HIDDEN_*` constants, `Filter::sql`, the queries that need `hidden < 2`, `add_folder`, `list_folders`, `folder_infos`.
- Create `crates/pixie-index/src/source.rs`: the source API on `Db`, `scan_source`, and their tests.
- Create `crates/pixie-index/src/import.rs`: the import log, batches and the new / kept-before / looks-the-same classification, and their tests.
- Modify `crates/pixie-index/src/lib.rs`: `pub mod source; pub mod import;` and re-exports in the style already there.
- Modify `crates/pixie-analysis/src/pipeline.rs` only if a query there lists photos for analysis without going through `photos_needing_work` (check, Task 1).

### Task 1: The source flag, and keeping sources out of every library listing

**Files:**
- Modify: `crates/pixie-index/src/db.rs` (migration list near line 31, `Filter` near line 456, `add_folder`/`list_folders`/`folder_infos` near line 720, `photos_needing_work_with` near line 840, the direct queries found by the audit)
- Test: `crates/pixie-index/src/source.rs` (a `#[cfg(test)] mod tests`, created here with just this test)

**Interfaces:**
- Produces:
  ```rust
  pub const HIDDEN_BY_USER: i64 = 1;   // photos.hidden: the person hid it (existing meaning)
  pub const HIDDEN_SOURCE: i64 = 2;    // photos.hidden: it is on a temporary source
  ```
  and the columns `folders.kind TEXT NOT NULL DEFAULT 'library'` (`'library'` or `'source'`), `folders.source_key TEXT`, `folders.label TEXT`.
  `list_folders()` and `folder_infos()` return library folders only.

- [ ] **Step 1: Write the failing test** `sources_never_appear_in_the_library` in `source.rs`. Build a `Db` the way `db.rs` tests do (look at the helper at the top of its test module), add a library folder A with one photo (with a date, a GPS position, a camera, tagged with a user tag and picked) and a source folder B inserted with `INSERT INTO folders(path, kind, source_key, label) VALUES (..,'source','K','Card')` holding one equivalent photo. Assert, for each public `Db` function that answers a library question, that the source photo is absent and the library photo present: `list_photos`, `count_photos`, `list_day_groups`, `kind_counts`, `location_counts`, `hidden_count` (stays 0), `list_folders`, `folder_infos`, `photos_needing_work`, the tag and user-tag usage and the picks, the map-points and trips queries, and the People lists. Then assert the source's own view: a `PhotoQuery` with `folder_id = B` lists exactly its photo; one with `folder_id = A` does not; with `show_hidden` on, the library listing still leaves the source out.
  The trigger that sets `hidden = 2` does not exist yet, so insert the source photo with `hidden = 2` explicitly in this test (Task 3 relies on the trigger instead).
- [ ] **Step 2: Run** `cd C:\work\pixie && set CARGO_TARGET_DIR=target-claude && cargo test -p pixie-index sources_never_appear`. Expected: FAIL (no `kind` column).
- [ ] **Step 3: Implement.**
  1. Migration V31 appended to `MIGRATIONS`, in the style of the V30 entry: `ALTER TABLE folders ADD COLUMN kind TEXT NOT NULL DEFAULT 'library'; ALTER TABLE folders ADD COLUMN source_key TEXT; ALTER TABLE folders ADD COLUMN label TEXT;` plus the trigger `CREATE TRIGGER photos_in_source AFTER INSERT ON photos WHEN (SELECT kind FROM folders WHERE id = NEW.folder_id) = 'source' BEGIN UPDATE photos SET hidden = 2 WHERE id = NEW.id; END;` (it makes a source photo hidden in the same statement that inserts it, so there is no moment it is listed).
  2. `Filter::sql`: where it adds `AND hidden = 0` (not `show_hidden`), allow `hidden IN (0, 2)` when a folder is set (`self.folder.is_some()`; a source's photos live only in its own folder) and `hidden = 0` otherwise; when `show_hidden`, `hidden < 2` without a folder and `hidden IN (0, 1, 2)` with one. Add a one-line comment saying why.
  3. `list_folders` and `folder_infos`: `WHERE kind = 'library'`. `get_folder` stays as it is (a source is a folder).
  4. `photos_needing_work_with`: add `AND p.hidden < 2` to the inner `WHERE`.
  5. **The audit.** Run `grep -n "FROM photos\|JOIN photos" crates/pixie-index/src/*.rs crates/pixie-analysis/src/*.rs src-tauri/src/*.rs`. Every query that lists or counts photos for the library either goes through `Filter`, has `hidden = 0`, or is a lookup by id or path (fine). Each of the rest gets `AND hidden < 2` (known ones: `location_counts` near line 2238 and `kind_counts`; check `utags.rs`, `mosaic.rs`, `dupes.rs`, `trips.rs`, `regroup.rs`). People's `VISIBLE` needs nothing: sources have no faces. Add every function you touched to the Step 1 test.
- [ ] **Step 4: Run** `cargo test -p pixie-index`. Expected: PASS (the whole crate, not only the new test).
- [ ] **Step 5: Leave uncommitted.**

### Task 2: The source lifecycle on `Db`

**Files:**
- Modify: `crates/pixie-index/src/source.rs`, `crates/pixie-index/src/db.rs` (`add_folder`)
- Modify: `crates/pixie-index/src/lib.rs`
- Test: `crates/pixie-index/src/source.rs`

**Interfaces:**
- Consumes: the columns and constants from Task 1.
- Produces:
  ```rust
  pub struct SourceInfo { pub id: FolderId, pub path: String, pub key: String, pub label: String, pub count: i64 }
  impl Db {
      /// Adds a temporary source (or returns the one already added for this path). Fails when the path is, or is inside, a library folder.
      pub fn add_source(&self, path: &str, key: &str, label: &str) -> Result<FolderId>;
      /// The sources, with how many items each holds.
      pub fn list_sources(&self) -> Result<Vec<SourceInfo>>;
      /// Forgets a source with its items, picks, tags and everything else about them; returns the removed items' ids (the caller deletes their thumbnails). Does nothing to a library folder.
      pub fn remove_source(&self, id: FolderId) -> Result<Vec<PhotoId>>;
      /// Removes every source whose key is not in `present` (the drive is gone); returns the removed items' ids.
      pub fn remove_sources_except(&self, present: &[String]) -> Result<Vec<PhotoId>>;
  }
  ```

- [ ] **Step 1: Write the failing tests** in `source.rs`:
  - `a_source_is_added_listed_and_counted`: add a source; `list_sources` shows it with count 0; adding the same path again returns the same id.
  - `a_path_that_is_a_library_folder_cannot_be_a_source`: `add_folder(A)` then `add_source(A or a subfolder of A, ..)` is `Err(Invalid)` with a sentence naming the folder.
  - `adding_a_library_folder_over_a_source_replaces_it`: `add_source(P)`, give it a photo, then `add_folder(P)`: `list_folders` has P, `list_sources` is empty, and the old source photo is gone (the person chose to keep it as a library folder, so its temporary rows must not linger as hidden).
  - `removing_a_source_leaves_nothing_behind`: a source with two photos that are picked, user-tagged, and have a row in every other table that has a `photo_id` column (embeddings, photo_tags, face_scans, faces, search_hits, and so on: find them with `SELECT name FROM sqlite_master WHERE sql LIKE '%photo_id%'` and insert one valid row in each, or skip a table only if it cannot hold a row without more setup and say which); `remove_source` returns both ids and afterwards no table has a row for either id. A library photo and its rows are untouched.
  - `remove_source_never_touches_a_library_folder`: `remove_source(library_id)` returns an empty list and the folder and photos remain.
  - `sources_whose_drive_is_gone_are_removed`: two sources (keys K1, K2); `remove_sources_except(&["K1"])` removes K2 only.
- [ ] **Step 2: Run** `cargo test -p pixie-index source`. Expected: FAIL (functions missing).
- [ ] **Step 3: Implement** the four `Db` methods (in `source.rs`, an `impl Db` block, using `self.conn()`), and in `db.rs` change `add_folder` so an existing `kind = 'source'` row with that path is removed first (through `remove_source`). Photos reference their folder with `ON DELETE CASCADE`, and `picks` cascades from `photos`; for any table that does not cascade, delete its rows by `photo_id IN (...)` inside the same transaction before removing the folder. Return the ids collected before deleting. The overlap check compares paths case-insensitively on Windows and macOS and treats a path as inside a folder when it starts with the folder's path followed by a separator.
- [ ] **Step 4: Run** `cargo test -p pixie-index`. Expected: PASS.
- [ ] **Step 5: Leave uncommitted.**

### Task 3: The light scan

**Files:**
- Modify: `crates/pixie-index/src/source.rs`
- Test: `crates/pixie-index/src/source.rs`

**Interfaces:**
- Consumes: `add_source`, `list_sources`, `remove_source` (Task 2); `walk_folder`, `finish_walk`, `ScanMode`, `ScanProgress` from `scanner.rs`; `Db::upsert_photo`; `pixie_image::meta::read_meta`; the video-metadata reader the analysis pipeline uses (`video_meta_with` in `pixie-analysis` calls into `pixie_image::video_meta`; use the same function).
- Produces:
  ```rust
  pub struct SourceScan { pub found: usize, pub added: usize, pub unreadable: usize, pub cancelled: bool }
  /// Indexes a source where it is: for each new or changed photo or video, its row with the date, size and dimensions (and a video's length), read from the file's header. No hashing, no analysis, nothing handed to the pipeline.
  pub fn scan_source(db: &Db, folder_id: FolderId, cancel: &AtomicBool, on_progress: impl FnMut(&ScanProgress)) -> Result<SourceScan>;
  ```

- [ ] **Step 1: Write the failing tests** (a `tempfile::tempdir` source with files made by `pixie_testutil::jpeg`):
  - `a_scan_lists_the_photos_with_their_dates_and_nothing_else`: three JPEGs and a `.txt`; `add_source`, `scan_source`; the source's own query lists the three (`found == 3`, `added == 3`); the library listing shows none; `hash` is NULL, no row in `embeddings`/`face_scans`/`photo_tags`; every row has `hidden = 2` (set by the trigger from Task 1, not by the scan).
  - `a_second_scan_adds_only_what_is_new`: add a fourth file, scan again: `added == 1`; touch no other row.
  - `files_whose_date_is_missing_take_the_files_modified_date`: a JPEG without EXIF: `taken_at == mtime / 1000` (the same rule as the pipeline).
  - `a_stopped_scan_comes_back_cancelled_and_keeps_what_it_found`: set `cancel` after the first progress callback; `cancelled == true`; the source can still be removed.
  - `a_source_that_vanishes_is_an_error_not_a_panic`: remove the temp directory, `scan_source` returns `Err`; `remove_source` still works.
  - `an_unreadable_file_is_counted_and_skipped`: a zero-byte `.jpg` among good ones: `unreadable == 1` and the others are listed. (A file whose header cannot be read is still listed if the extension is supported, with no date taken, as the pipeline does for an unreadable file; if you find the pipeline skips it entirely, follow the pipeline and say so here.)
- [ ] **Step 2: Run** `cargo test -p pixie-index source`. Expected: FAIL (`scan_source` missing).
- [ ] **Step 3: Implement** `scan_source`: `walk_folder(db, folder_id, ScanMode::Quick, cancel, on_progress)` gives the new and changed files as candidates; for each, read the meta (`read_meta(path)` for a photo; the video reader for a video), set `meta.kind` from the candidate, apply the pipeline's rule `taken_at = Some(mtime / 1000)` when there is none, and `db.upsert_photo(folder_id, &c.path, c.size, c.mtime, &meta)`. Check `cancel` between files. Finish with `finish_walk` so files that went from the source are marked missing. Count unreadable files and keep going.
- [ ] **Step 4: Run** `cargo test -p pixie-index`. Expected: PASS.
- [ ] **Step 5: Leave uncommitted.**

### Task 4: The import log and what is new

**Files:**
- Modify: `crates/pixie-index/src/db.rs` (migration V32)
- Create: `crates/pixie-index/src/import.rs`
- Modify: `crates/pixie-index/src/lib.rs`
- Test: `crates/pixie-index/src/import.rs`

**Interfaces:**
- Consumes: sources and the light scan (Tasks 2 and 3).
- Produces:
  ```rust
  pub type BatchId = i64;
  #[derive(Debug, Clone, PartialEq, Eq)]
  pub enum ItemState { New, KeptBefore, LooksSame { library_path: String } }
  #[derive(Debug, Default, PartialEq, Eq)]
  pub struct SourceSummary { pub photos: i64, pub videos: i64, pub bytes: i64, pub new: i64, pub kept_before: i64, pub looks_same: i64, pub first: Option<i64>, pub last: Option<i64> }
  impl Db {
      /// Starts a batch: one Keep run from one source into one folder.
      pub fn start_batch(&self, source_key: &str, label: &str, dest_folder: FolderId) -> Result<BatchId>;
      /// Records an item that was kept (copied and verified), so it is known next time even if the library copy is deleted.
      pub fn log_kept(&self, batch: BatchId, source_key: &str, path: &str, size: i64, mtime: i64, hash: Option<&str>, dest_path: &str) -> Result<()>;
      /// For each item of a source: new, kept before (same source, path, size and modified time in the log), or looks the same as a library item (same name ignoring case and same size).
      pub fn source_item_states(&self, folder: FolderId) -> Result<Vec<(PhotoId, ItemState)>>;
      pub fn source_summary(&self, folder: FolderId) -> Result<SourceSummary>;
  }
  ```
  Migration V32: `import_batch(id INTEGER PRIMARY KEY, started INTEGER NOT NULL, source_key TEXT NOT NULL, label TEXT NOT NULL, dest_folder INTEGER NOT NULL, items INTEGER NOT NULL DEFAULT 0, bytes INTEGER NOT NULL DEFAULT 0)` and `import_log(batch INTEGER NOT NULL REFERENCES import_batch(id) ON DELETE CASCADE, source_key TEXT NOT NULL, path TEXT NOT NULL, size INTEGER NOT NULL, mtime INTEGER NOT NULL, hash TEXT, dest_path TEXT NOT NULL)` with an index on `(source_key, path, size, mtime)`. `log_kept` also adds the item's size to the batch's `items` and `bytes`.

- [ ] **Step 1: Write the failing tests:**
  - `an_item_kept_before_is_known_by_source_path_size_and_time`: add a source with key `K` and a scanned photo; `source_item_states` says `New`; `log_kept(.. "K", path, size, mtime ..)`; now it says `KeptBefore`; deleting the library copy of it (it is not in `photos` at all in this test) makes no difference.
  - `reused_file_names_on_a_new_card_are_not_known`: a second source with the same relative names but a different size or modified time, key `K2` (or the same key, different mtime): its items are `New`.
  - `an_item_with_the_name_and_size_of_a_library_photo_looks_the_same`: library photo `IMG_0451.JPG` of 100 bytes; a source item `img_0451.jpg` of 100 bytes is `LooksSame { library_path }`; with 101 bytes it is `New`; the match ignores case and the folders; a hidden or missing library photo does not count; another source's photos do not count.
  - `the_summary_counts_photos_videos_bytes_new_and_the_date_range`: a mixed source with one kept-before, one looks-same and the rest new: the numbers match and `first` and `last` are the earliest and latest `taken_at`.
  - `a_batch_adds_up_what_was_logged`: `start_batch`, two `log_kept` calls: `import_batch.items == 2` and `bytes` is the sum.
- [ ] **Step 2: Run** `cargo test -p pixie-index import`. Expected: FAIL.
- [ ] **Step 3: Implement** the migration and the four methods. For "looks the same", do not run a query per item: collect the source's distinct sizes, fetch `(path, size)` of non-hidden, non-missing library photos with those sizes in one query (`size IN (SELECT value FROM json_each(?))`, in the style used by `hidden_among`), reduce each path to its lower-cased file name in Rust, and match from a `HashMap<(String, i64), String>`.
- [ ] **Step 4: Run** `cargo test -p pixie-index`, then `cargo check --workspace --all-targets` and `cargo test -p pixie-analysis` (the pipeline query changed in Task 1). Expected: PASS and no warnings.
- [ ] **Step 5: Leave uncommitted** and tell the user which files changed and what the next plan covers (drive detection, Tauri commands, Keep and Private, the UI).

## Self-review

- **Spec coverage:** a temporary source kept out of the library (Task 1), its lifecycle and removal (Task 2), the light scan with no analysis (Task 3), the import log and "knowing what is new" levels 1 and 2 (Task 4). Level 3 (identical hash after copying) belongs with Keep. Left for the next plan: drive detection (`removable_roots`), the Tauri commands, Keep and the Private photos folder, Delete from the card, and all UI.
- **Types:** `SourceInfo`, `SourceScan`, `ItemState`, `SourceSummary` and `BatchId` are each defined once and used unchanged. `HIDDEN_BY_USER`/`HIDDEN_SOURCE` are the only numbers for `photos.hidden`.
- **Proportion:** four tasks, each a testable Rust deliverable; no UI.
