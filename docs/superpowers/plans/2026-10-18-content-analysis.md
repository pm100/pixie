# Content Analysis (foundation + semantic search) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Background CLIP embedding of every photo plus a Browse search box that finds photos by description, all local.

**Architecture:** New crate `pixie-analysis` holds vector math, CLIP preprocessing, the `Embedder` trait (fake for tests, ONNX for real), a model downloader and the embedding worker. `pixie-index` stores embeddings and a per-session "search hits" TEMP table that a new `PhotoQuery.search` flag filters on, so the existing grid, viewer, map and When nav all work on search results unchanged. Tauri wires a worker, progress events and three commands; the UI adds Settings -> Analysis and a search box.

**Tech Stack:** Rust, `ort` (ONNX Runtime), `tokenizers`, `sha2`, `ureq`, rusqlite; Svelte 5 / TS.

**Spec:** `docs/superpowers/specs/2026-10-18-content-analysis-design.md`

**Scope note:** The spec's Stage 3 (faces / People) is a separate plan, written after this one lands, because it adds a second model family, clustering and its own UI. Rulings against the spec, taken here: (1) the pending queue is derived by query (photos without an embedding for the current `model_version`), not a `jobs` table: fewer moving parts, restart-safe by construction. (2) Search results display in date order within the existing filters, capped at the top 500 matches above a similarity floor; relevance ordering is a later refinement.

## Global Constraints

- Commit and push only when the user says so (AGENTS.md); the commit steps below are for when they have.
- Everything local; no network use except the one-time model download, and only from the pinned URLs.
- CPU only. Workers run at low priority (1 thread) and pause while a scan runs.
- Cargo: `cargo test --workspace`; UI: `cd ui && npx vitest run && npx svelte-check`. `cargo build` fails while `pixie-app.exe` runs; use `cargo check -p pixie-app`.
- Code style: match surrounding code (short doc comments, no section banners). `src-tauri/src/lib.rs` and `ui/src/App.svelte` use CRLF; keep it.
- Model: OpenAI CLIP ViT-B/32, int8 ONNX (image tower, text tower, `tokenizer.json`), `MODEL_VERSION = "clip-b32-q8-1"`, 512-d, L2-normalised on store.

## Review Focus

- Photo that fails to decode (corrupt, unsupported): worker must mark it done-with-no-embedding so it is not retried forever and does not stall the queue.
- Model not downloaded / download interrupted or checksum mismatch: nothing is stored, the file is not left half-valid, Settings shows the error and Retry.
- Search while embeddings are incomplete: results cover what is embedded so far and the UI says "N of M analysed".
- Empty or whitespace query clears the search; a query with no match above the floor shows an empty result, not an error.
- Folder removed while analysing: cascade deletes embeddings; worker skips jobs whose photo vanished.

---

### Task 1: Embedding storage and the search-hits filter

**Files:**
- Modify: `crates/pixie-core/src/lib.rs` (PhotoQuery)
- Modify: `crates/pixie-index/src/db.rs` (V5 migration, Filter, new methods)
- Test: `crates/pixie-index/src/db.rs` (`#[cfg(test)]` module, same place as existing tests)

**Interfaces:**
- Produces: `PhotoQuery.search: bool`; `Db::put_embedding(photo_id, model_version: &str, vector: &[f32]) -> Result<()>` (stores `None`-vector marker via `put_embedding_failed`); `Db::put_embedding_failed(photo_id, model_version)`; `Db::photos_needing_embedding(model_version, limit) -> Result<Vec<Photo>>` (non-missing photos with no row for that version, kind Photo only); `Db::embedding_counts(model_version) -> Result<(i64 /*done*/, i64 /*total photos*/)>`; `Db::load_embeddings(model_version) -> Result<Vec<(PhotoId, Vec<f32>)>>` (skips failed markers); `Db::set_search_hits(ids: &[PhotoId]) -> Result<()>`.

- [ ] **Step 1: Write failing tests** (in the db.rs test module, using the existing helpers that add a folder and `upsert_photo`)

```rust
#[test]
fn embeddings_round_trip_and_pending_query() {
    let db = Db::open_in_memory().unwrap();
    let f = db.add_folder("/p").unwrap();
    let a = db.upsert_photo(f, "/p/a.jpg", 1, 1, &PhotoMeta::default()).unwrap();
    let b = db.upsert_photo(f, "/p/b.jpg", 1, 1, &PhotoMeta::default()).unwrap();
    assert_eq!(db.photos_needing_embedding("m1", 10).unwrap().len(), 2);
    db.put_embedding(a, "m1", &[0.5, 0.25]).unwrap();
    db.put_embedding_failed(b, "m1").unwrap();
    assert!(db.photos_needing_embedding("m1", 10).unwrap().is_empty());
    assert_eq!(db.photos_needing_embedding("m2", 10).unwrap().len(), 2); // new model version re-queues
    assert_eq!(db.load_embeddings("m1").unwrap(), vec![(a, vec![0.5, 0.25])]); // failed marker skipped
    assert_eq!(db.embedding_counts("m1").unwrap(), (2, 2));
}

#[test]
fn search_hits_filter_listing() {
    let db = Db::open_in_memory().unwrap();
    let f = db.add_folder("/p").unwrap();
    let a = db.upsert_photo(f, "/p/a.jpg", 1, 1, &PhotoMeta::default()).unwrap();
    let _b = db.upsert_photo(f, "/p/b.jpg", 1, 1, &PhotoMeta::default()).unwrap();
    db.set_search_hits(&[a]).unwrap();
    let q = PhotoQuery { search: true, ..Default::default() };
    assert_eq!(db.count_photos_in(&q).unwrap(), 1);
    assert_eq!(db.list_photos_in(&q, 0, 10).unwrap()[0].id, a);
    db.set_search_hits(&[]).unwrap();
    assert_eq!(db.count_photos_in(&q).unwrap(), 0);
    assert_eq!(db.count_photos_in(&PhotoQuery::default()).unwrap(), 2); // flag off: unfiltered
}
```

- [ ] **Step 2: Run** `cargo test -p pixie-index embeddings_round_trip search_hits` -> Expected: FAIL (compile error: no such methods / field).

- [ ] **Step 3: Implement.**
  - `PhotoQuery`: add `/// Only items in the current search result (see Db::set_search_hits).\n pub search: bool,`; fix every struct-literal construction (`query_of` in `src-tauri/src/lib.rs` takes a new `search: Option<bool>` argument and sets `search: search.unwrap_or(false)`; `grep -n "PhotoQuery {" -r crates src-tauri` for the rest).
  - Migration V5 (append to `MIGRATIONS`):
    ```sql
    CREATE TABLE embeddings (
        photo_id      INTEGER NOT NULL REFERENCES photos(id) ON DELETE CASCADE,
        model_version TEXT NOT NULL,
        vector        BLOB,           -- NULL = analysis failed for this photo; do not retry
        PRIMARY KEY (photo_id, model_version)
    );
    ```
  - In `Db::init` after migrations: `CREATE TEMP TABLE IF NOT EXISTS search_hits (photo_id INTEGER PRIMARY KEY);`
  - `Filter`: add `search: bool`; `sql()` appends ` AND id IN (SELECT photo_id FROM search_hits)` when set; `Filter::of` copies `q.search`. (Check `day_groups`, `photos_at`, `list_geo_points` build their SQL from `Filter`; geo points take no `PhotoQuery`, leave it.)
  - Vector codec: `fn vec_to_blob(v:&[f32])->Vec<u8>` (little-endian f32 bytes) and `fn blob_to_vec(b:&[u8])->Vec<f32>` as private fns in db.rs.
  - Methods per Interfaces. `photos_needing_embedding`: `SELECT {PHOTO_COLS} FROM photos p WHERE missing=0 AND kind=0 AND NOT EXISTS (SELECT 1 FROM embeddings e WHERE e.photo_id=p.id AND e.model_version=?1) ORDER BY id DESC LIMIT ?2` (newest first). `embedding_counts`: done = rows for version joined to non-missing photos; total = non-missing photos of kind 0. `set_search_hits`: `DELETE FROM search_hits` then insert in a transaction.

- [ ] **Step 4: Run** `cargo test -p pixie-index` -> Expected: all pass (existing tests untouched by the added field via `..Default::default()`).

- [ ] **Step 5: Commit** `feat: embeddings table and search-hits filter`

---

### Task 2: Vector math

**Files:**
- Create: `crates/pixie-analysis/Cargo.toml`, `crates/pixie-analysis/src/lib.rs`, `crates/pixie-analysis/src/vectors.rs`
- Test: in `vectors.rs`

**Interfaces:**
- Produces: `normalize(v: &mut [f32])`; `dot(a:&[f32], b:&[f32]) -> f32`; `struct Hit { pub id: i64, pub score: f32 }`; `top_k(query:&[f32], items:&[(i64, Vec<f32>)], k: usize, floor: f32) -> Vec<Hit>` (descending score, only `score >= floor`, assumes unit vectors).

- [ ] **Step 1: Create the crate** (`Cargo.toml`: `[package] name="pixie-analysis" version.workspace=true edition.workspace=true`; deps filled in later tasks; `[dependencies] pixie-core={path="../pixie-core"}`). Add `pub mod vectors;` to lib.rs.

- [ ] **Step 2: Write failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn normalize_makes_unit_length() {
        let mut v = vec![3.0, 4.0];
        normalize(&mut v);
        assert!((dot(&v, &v) - 1.0).abs() < 1e-6);
    }
    #[test]
    fn normalize_zero_vector_is_left_alone() {
        let mut v = vec![0.0, 0.0];
        normalize(&mut v);
        assert_eq!(v, vec![0.0, 0.0]);
    }
    #[test]
    fn top_k_orders_limits_and_applies_floor() {
        let items = vec![(1, vec![1.0, 0.0]), (2, vec![0.8, 0.6]), (3, vec![0.0, 1.0])];
        let hits = top_k(&[1.0, 0.0], &items, 2, 0.1);
        assert_eq!(hits.iter().map(|h| h.id).collect::<Vec<_>>(), vec![1, 2]);
        assert!(top_k(&[1.0, 0.0], &items, 5, 0.9).iter().all(|h| h.score >= 0.9));
        assert_eq!(top_k(&[1.0, 0.0], &items, 5, 0.9).len(), 1);
    }
}
```

- [ ] **Step 3: Run** `cargo test -p pixie-analysis` -> FAIL (not defined).

- [ ] **Step 4: Implement**

```rust
pub fn dot(a: &[f32], b: &[f32]) -> f32 { a.iter().zip(b).map(|(x, y)| x * y).sum() }

pub fn normalize(v: &mut [f32]) {
    let n = dot(v, v).sqrt();
    if n > 0.0 { v.iter_mut().for_each(|x| *x /= n); }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Hit { pub id: i64, pub score: f32 }

/// The `k` best matches at or above `floor`, best first. Vectors are expected to be unit length.
pub fn top_k(query: &[f32], items: &[(i64, Vec<f32>)], k: usize, floor: f32) -> Vec<Hit> {
    let mut hits: Vec<Hit> = items.iter().map(|(id, v)| Hit { id: *id, score: dot(query, v) }).filter(|h| h.score >= floor).collect();
    hits.sort_by(|a, b| b.score.total_cmp(&a.score));
    hits.truncate(k);
    hits
}
```

- [ ] **Step 5: Run** `cargo test -p pixie-analysis` -> PASS. **Commit** `feat: pixie-analysis crate with vector math`.

---

### Task 3: CLIP image preprocessing and the Embedder trait

**Files:**
- Create: `crates/pixie-analysis/src/clip_pre.rs`, `crates/pixie-analysis/src/embedder.rs`
- Modify: `crates/pixie-analysis/Cargo.toml` (add `image = { workspace = true }`), `lib.rs`
- Test: in each file

**Interfaces:**
- Produces: `clip_pre::to_tensor(img: &image::DynamicImage) -> Vec<f32>` (len 3*224*224, CHW, resize shortest side to 224 with Catmull-Rom, centre crop 224x224, per-channel `(x/255 - mean)/std` with mean `[0.48145466,0.4578275,0.40821073]`, std `[0.26862954,0.26130258,0.27577711]`); `embedder::Embedder` trait: `fn embed_image(&self, img:&DynamicImage)->Result<Vec<f32>, String>; fn embed_text(&self, text:&str)->Result<Vec<f32>, String>; fn version(&self)->&str;` (both return unit vectors); `embedder::FakeEmbedder` (test helper, `pub`): deterministic - `embed_text("cat")` and an image whose average red > 128 map to `[1,0]`, otherwise `[0,1]`.

- [ ] **Step 1: Failing tests**

```rust
// clip_pre.rs
#[test]
fn tensor_has_chw_shape_and_normalised_values() {
    let img = image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(640, 480, image::Rgb([255, 255, 255])));
    let t = to_tensor(&img);
    assert_eq!(t.len(), 3 * 224 * 224);
    assert!((t[0] - (1.0 - 0.48145466) / 0.26862954).abs() < 1e-4);                 // R plane first
    assert!((t[224 * 224] - (1.0 - 0.4578275) / 0.26130258).abs() < 1e-4);           // then G
}
#[test]
fn tall_image_is_centre_cropped_not_squashed() {
    // top half black, bottom half white, 100x400: the 100x100 centre crop straddles the seam.
    let mut im = image::RgbImage::new(100, 400);
    for (_, y, p) in im.enumerate_pixels_mut() { *p = if y < 200 { image::Rgb([0, 0, 0]) } else { image::Rgb([255, 255, 255]) }; }
    let t = to_tensor(&image::DynamicImage::ImageRgb8(im));
    assert!(t[10 * 224] < t[200 * 224]); // top of crop darker than bottom
}
```

- [ ] **Step 2: Run** -> FAIL. **Step 3: Implement** `to_tensor` with `img.resize_to_fill(224, 224, FilterType::CatmullRom)` (does cover+centre-crop), `to_rgb8()`, then fill planes. Write `Embedder` and `FakeEmbedder` (with a test asserting the fake's two behaviours). **Step 4: Run** `cargo test -p pixie-analysis` -> PASS. **Step 5: Commit** `feat: CLIP preprocessing and Embedder trait`.

---

### Task 4: Model files: pinned manifest, download, checksum

**Files:**
- Create: `crates/pixie-analysis/src/models.rs`
- Modify: `crates/pixie-analysis/Cargo.toml` (`sha2 = "0.10"`, `ureq = "2"`, `tempfile` dev)
- Test: in `models.rs`

**Interfaces:**
- Produces: `struct ModelFile { pub name: &'static str, pub url: &'static str, pub sha256: &'static str, pub bytes: u64 }`; `pub const CLIP_FILES: &[ModelFile]` (image tower, text tower, tokenizer); `trait Downloader: Send + Sync { fn fetch(&self, url:&str, sink:&mut dyn FnMut(&[u8]) -> bool) -> Result<(), String>; }` (sink returns false to cancel); `struct HttpDownloader` (ureq, streaming 64 KB reads); `fn ensure_models(dir:&Path, files:&[ModelFile], dl:&dyn Downloader, progress:&mut dyn FnMut(u64 /*done*/, u64 /*total*/)) -> Result<(), String>`: skips files whose on-disk sha256 matches, downloads others to `name.part`, verifies sha256, renames; a mismatch deletes the `.part` and returns `Err("checksum mismatch for <name>")`; `fn models_ready(dir, files) -> bool` (sizes and hashes OK).

- [ ] **Step 1: Pin the manifest.** Verify the licence on the model card (must be MIT/Apache; if not, stop and report). Download the three Xenova/clip-vit-base-patch32 files (`onnx/vision_model_quantized.onnx`, `onnx/text_model_quantized.onnx`, `tokenizer.json`) from `https://huggingface.co/Xenova/clip-vit-base-patch32/resolve/main/...` into the scratchpad, compute `sha256sum` and sizes, and write them into `CLIP_FILES`. Record the values in the commit message. Never invent a hash.

- [ ] **Step 2: Failing tests** with a `FakeDownloader` that serves fixed byte slices keyed by URL:

```rust
#[test] fn downloads_verifies_and_skips_when_present() { /* ensure_models twice; second call fetches 0 times */ }
#[test] fn checksum_mismatch_leaves_no_file_and_errors() { /* wrong sha -> Err contains "checksum mismatch", dir has no name or name.part */ }
#[test] fn cancel_stops_and_removes_part_file() { /* sink returns false after first chunk */ }
#[test] fn progress_reaches_total() { /* last progress call == (total, total) */ }
```
(Write the bodies with `tempfile::tempdir()` and a `FakeDownloader { files: HashMap<String, Vec<u8>>, calls: Cell<usize> }` using `Mutex<usize>` since the trait is `Sync`; compute expected sha256 in the test with the `sha2` crate.)

- [ ] **Step 3: Run** -> FAIL. **Step 4: Implement** per Interfaces (hash while streaming; fsync not required). **Step 5: Run** `cargo test -p pixie-analysis` -> PASS. **Commit** `feat: model manifest and checksummed download`.

---

### Task 5: ONNX CLIP embedder

**Files:**
- Create: `crates/pixie-analysis/src/clip.rs`
- Modify: `crates/pixie-analysis/Cargo.toml` (`ort = "2.0.0-rc.10"` with default features; `tokenizers = { version = "0.20", default-features = false, features = ["onig"] }`, `ndarray = "0.16"`)
- Test: `crates/pixie-analysis/tests/clip_real.rs` (`#[ignore]`)

**Interfaces:**
- Consumes: `Embedder` (Task 3), `to_tensor`, `normalize`, `CLIP_FILES`.
- Produces: `ClipEmbedder::load(model_dir:&Path) -> Result<ClipEmbedder, String>` implementing `Embedder` with `version() == MODEL_VERSION` (`pub const MODEL_VERSION: &str = "clip-b32-q8-1"`). Text: tokenizer from `tokenizer.json`, pad/truncate to 77 ids (pad id 0 as in the tokenizer config; truncate keeps the end-of-text token), run the text tower with `input_ids` (int64 [1,77]) and take output `text_embeds`/`last_hidden_state` pooled as the model card specifies (inspect output names with `session.outputs` and use the 512-d projected embedding). Image: `pixel_values` f32 [1,3,224,224] -> `image_embeds`. Both normalised.

- [ ] **Step 1: Write the ignored real-model test** (needs `PIXIE_MODEL_DIR` pointing at the downloaded files):

```rust
#[test] #[ignore = "needs PIXIE_MODEL_DIR with the CLIP files"]
fn text_matches_the_right_image() {
    let e = ClipEmbedder::load(std::path::Path::new(&std::env::var("PIXIE_MODEL_DIR").unwrap())).unwrap();
    let red = DynamicImage::ImageRgb8(RgbImage::from_pixel(224, 224, Rgb([220, 20, 20])));
    let blue = DynamicImage::ImageRgb8(RgbImage::from_pixel(224, 224, Rgb([20, 20, 220])));
    let q = e.embed_text("a plain red image").unwrap();
    assert!(dot(&q, &e.embed_image(&red).unwrap()) > dot(&q, &e.embed_image(&blue).unwrap()));
}
```

- [ ] **Step 2: Run** `cargo test -p pixie-analysis --test clip_real -- --ignored` -> FAIL (not defined).
- [ ] **Step 3: Implement** `ClipEmbedder` (sessions in `Mutex<Session>`; `Session::builder()?.with_intra_threads(1)?.commit_from_file(..)`; confirm output names/dims from the loaded model and adjust, noting the ort rc API may differ slightly from memory: follow the compiler and `ort` docs for the exact `inputs!`/`try_extract_tensor` calls). Keep thread count a constructor arg defaulting to 1.
- [ ] **Step 4: Run** the ignored test with the real files -> PASS. Run `cargo test -p pixie-analysis` (non-ignored) -> PASS.
- [ ] **Step 5: Commit** `feat: ONNX CLIP embedder`.

---

### Task 6: Embedding worker

**Files:**
- Create: `crates/pixie-analysis/src/worker.rs`
- Modify: `crates/pixie-analysis/Cargo.toml` (`pixie-index`, `pixie-image` path deps)
- Test: in `worker.rs` using `Db::open_in_memory`, `FakeEmbedder`, small PNGs written to a tempdir

**Interfaces:**
- Consumes: `Db` methods (Task 1), `Embedder`, `pixie_image::decode::decode_scaled(path, edge)`.
- Produces: `struct Progress { pub done: i64, pub total: i64, pub paused: bool, pub busy: bool }`; `struct Worker`; `Worker::start(db: Db, embedder: Arc<dyn Embedder>, scanning: Arc<dyn Fn() -> bool + Send + Sync>, on_progress: Arc<dyn Fn(Progress) + Send + Sync>) -> Worker`; `Worker::set_paused(bool)`; `Worker::kick()` (wake to look for new work); `Worker::stop()` (joins). Behaviour: loop { if paused or `scanning()` -> sleep 500 ms and continue; fetch up to 32 `photos_needing_embedding(embedder.version())`; none -> emit progress, wait for kick (or 5 s); for each: decode to 256 px edge, embed, `put_embedding` (normalised by the embedder); decode/embed error -> `put_embedding_failed`; emit progress every item }.

- [ ] **Step 1: Failing tests**

```rust
#[test] fn embeds_every_photo_and_reports_done()      { /* 3 PNGs (2 red, 1 blue) -> counts (3,3), load_embeddings has 3 */ }
#[test] fn undecodable_file_is_marked_failed_once()   { /* garbage bytes named .jpg -> no vector, not in photos_needing_embedding, others still embedded */ }
#[test] fn pause_holds_work_until_resumed()           { /* set_paused(true) before start work; counts stay (0,N); resume -> (N,N) within 5 s (poll with timeout) */ }
#[test] fn waits_while_scanning()                     { /* scanning() true -> no progress; flip false -> completes */ }
#[test] fn photo_removed_midway_is_skipped()          { /* remove_folder during run does not panic; worker idles */ }
```
Write bodies with a helper `fn wait_until(timeout_ms, f: impl Fn()->bool)`.

- [ ] **Step 2: Run** -> FAIL. **Step 3: Implement** with `std::thread`, a `Mutex<bool>`/`Condvar` for kick, `AtomicBool` paused/stop. **Step 4: Run** `cargo test -p pixie-analysis` -> PASS. **Step 5: Commit** `feat: background embedding worker`.

---

### Task 7: Search service and Tauri wiring

**Files:**
- Create: `crates/pixie-analysis/src/search.rs`
- Modify: `crates/pixie-analysis/src/lib.rs`, `src-tauri/Cargo.toml` (dep), `src-tauri/src/state.rs`, `src-tauri/src/lib.rs`
- Test: `search.rs` unit tests

**Interfaces:**
- Consumes: `top_k`, `Embedder`, `Db::load_embeddings/set_search_hits`.
- Produces: `search::run(db:&Db, embedder:&dyn Embedder, text:&str, k:usize, floor:f32) -> Result<usize /*hit count*/, String>` (blank/whitespace text -> `set_search_hits(&[])`, returns 0, and the caller clears the flag); `pub const SEARCH_FLOOR: f32 = 0.2;` `pub const SEARCH_LIMIT: usize = 500;`. Tauri: `AppState.analysis: Arc<AnalysisState>` (models ready flag, worker handle in `Mutex<Option<Worker>>`, embedder `Mutex<Option<Arc<dyn Embedder>>>`, last error); commands `analysis_status() -> AnalysisStatus{models: "missing"|"downloading"|"ready"|"error", model_done, model_total, done, total, paused, error}`, `analysis_set_paused(paused: bool)`, `analysis_retry()`, `search_photos(text: String) -> usize`; events `analysis-progress` (AnalysisStatus) and `analysis-models`. `count_photos`/`list_photos`/`list_day_groups`/`photos_at` gain `search: Option<bool>`.

- [ ] **Step 1: Failing tests** (`search.rs`, FakeEmbedder + in-memory Db with embeddings put by hand):

```rust
#[test] fn search_sets_hits_to_matching_photos_only() { /* text "cat" -> hits = photos stored with [1,0] */ }
#[test] fn blank_query_clears_hits()                  { /* run with "  " -> count_photos_in(search:true)==0, Ok(0) */ }
#[test] fn nothing_above_floor_returns_zero_hits()    { /* all stored [0,1], query "cat" [1,0], floor 0.2 -> 0 */ }
```
- [ ] **Step 2: Run** -> FAIL. **Step 3: Implement `search::run`.**
- [ ] **Step 4: Wire Tauri** (no unit test; verify with `cargo check -p pixie-app`):
  - In `setup`: build `AnalysisState`; spawn a thread that runs `ensure_models` into `<app_data>/models/clip-b32-q8-1/` (emitting `analysis-models` with bytes), then `ClipEmbedder::load`, then `Worker::start(db, embedder, scanning = || !gate.is_free(), emit analysis-progress)`. Add `ScanGate::is_busy()` in pixie-index if missing (test it). On error store the message; `analysis_retry` reruns the thread.
  - After every `scan-done`, call `worker.kick()`.
  - `search_photos`: needs the embedder; returns `Err("Analysis models are not ready yet")` otherwise.
  - Register commands in `generate_handler!`; thread `search` through `query_of`.
- [ ] **Step 5: Run** `cargo test --workspace` -> PASS; `cargo check -p pixie-app` -> clean. **Commit** `feat: semantic search service and Tauri wiring`.

---

### Task 8: UI: search box, chip, Settings -> Analysis

**Files:**
- Create: `ui/src/lib/analysis.ts` (+ `analysis.test.ts`), `ui/src/lib/AnalysisSettings.svelte`
- Modify: `ui/src/lib/api.ts`, `ui/src/lib/Settings.svelte` (new "Analysis" page), `ui/src/App.svelte` (search state, box, chip, pass `search` into the existing list/count/day-group/photosAt calls), `docs/superpowers/specs/2026-10-18-content-analysis-design.md` (record the two rulings)
- Test: `ui/src/lib/analysis.test.ts`

**Interfaces:**
- Produces (`analysis.ts`, pure): `type AnalysisStatus = { models: 'missing'|'downloading'|'ready'|'error'; model_done: number; model_total: number; done: number; total: number; paused: boolean; error: string | null }`; `analysisLine(s): string` -> "Downloading search model… 42%" / "Analysing photos… 1,234 of 50,000" / "All 50,000 photos analysed" / "Paused at 1,234 of 50,000" / error text; `analysisFraction(s): number | null`; `searchNote(s): string | null` -> "Searching the 1,234 of 50,000 photos analysed so far" while `done < total`, else null.

- [ ] **Step 1: Failing vitest** for `analysisLine`, `analysisFraction`, `searchNote` covering each state (missing, downloading 42%, working, paused, complete, error, zero total).
- [ ] **Step 2: Run** `cd ui && npx vitest run analysis` -> FAIL. **Step 3: Implement** `analysis.ts`; then `api.ts` additions (`analysisStatus`, `analysisSetPaused`, `analysisRetry`, `searchPhotos`, `search` param on the four list calls); `AnalysisSettings.svelte` (status line + `<progress>` + Pause/Resume + Retry on error; subscribes to `analysis-progress` / `analysis-models` events) and add it to `Settings.svelte` pages; in `App.svelte`: `searchText` state, a search input in the tools bar (Enter submits, Esc / ✕ clears), on submit call `searchPhotos(text)` then reload the list with `search: true`; chip "Matching “dog on beach” ✕" next to the period chip, `searchNote` beneath it when incomplete, "Nothing matches" empty state when count is 0.
- [ ] **Step 4: Run** `cd ui && npx vitest run && npx svelte-check && npx vite build` -> all pass, 0 errors.
- [ ] **Step 5: Ask the user to run the app** and try: model download progress in Settings -> Analysis, pause/resume, a search, clearing it. Report what they see; fix anything wrong. **Commit** `feat: semantic search UI and Analysis settings`.
