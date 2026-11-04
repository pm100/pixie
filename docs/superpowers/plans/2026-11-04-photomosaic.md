# Photomosaic Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A Photomosaic tool in the Create tab that rebuilds a chosen photo (or part of one) as a grid of square tiles taken from the library, with a live draft, a cancellable full-size Build, a zoomable look at the result, and Save by the Collage rules.

**Architecture:** `pixie-image` gets a new `mosaic` module (geometry, colour signatures, tile assignment, tinting, draft and full-size render, a tile pyramid for viewing). `pixie-index` stores one small signature per photo (average colour plus a 12 × 12 miniature) and answers "which photos can be tiles" for the three sources. `src-tauri` adds the commands and a `/mosaic/{z}/{x}/{y}` protocol route. The window gets `MosaicTool.svelte` beside `CollageTool.svelte`, a Leaflet `CRS.Simple` viewer for the built picture, and a small `mosaic.ts` model.

**Tech Stack:** Rust (image, rusqlite, tauri 2), Svelte 5, vitest, Leaflet (already a dependency). No new dependencies are planned; if one is added, run `just notices`.

**Spec:** `docs/superpowers/specs/2026-11-04-photomosaic-design.md`

## Global Constraints

- Never commit or push unless the user says so ("c & p"). This plan has no commit steps; leave changes uncommitted and ask at the end. Never commit `.claude/` or `target-claude/`.
- No compiler warnings (`cargo check --workspace --all-targets`), no `#[allow]`, no dead code.
- Cargo for Claude builds: `CARGO_TARGET_DIR=target-claude VCPKG_ROOT=C:/work/vcpkg-pixie`. Batch runs: test the changed crate per task, the whole workspace once at the end.
- UI copy: counts use `plural` from `ui/src/lib/format.ts`; no control in the middle of a sentence; units in the label ("Tile size (mm):"); padding from `--page-pad`; every control has a `title` (a whole sentence) and an `aria-label` when it has no visible words (`tooltips.test.ts` enforces); buttons white by default, `class="primary"` for the one main action.
- Reuse Collage code, not copies: `defaultDpi`, `pagePx`, `sizeProblem`, `MAX_SIDE_PX`, `defaultSavePath` (`ui/src/lib/collage.ts`); `paperGroups`, `findPaper` (`collagePapers.ts`); `cover_crop`, `page_px`, `save` (`crates/pixie-image/src/collage.rs`).
- Page limits: dpi 300 up to A3, 150 for A2 and larger (`defaultDpi`); longest side at most 16,384 px.
- Tile: the centred square of the photo, turned upright. Never video. Not private (`share != SHARE_PRIVATE`), not hidden as a duplicate (`dup = 0`), not missing.
- Originals are never changed. No UI freeze: index and build run on the blocking pool with progress and Cancel.

## Review Focus

- A library with fewer eligible photos than cells: still builds, repeats gap shrinks, tool says so (Task 2, 7).
- Zero eligible photos (empty source, all private, all videos): a plain message, Build disabled, no crash (Tasks 4, 7).
- An unreadable or deleted original at Build time: skipped from the index and counted; at Build the cell takes the next best photo, the build does not fail (Tasks 3, 5).
- A setting changed during or after a Build: the built picture is marked out of date and Save is greyed (Task 7); a Cancelled build leaves the previous built picture intact.
- A page near 16,384 px, a tile size larger than the page, a tile size that gives one cell, and 0 or 100% blend (Tasks 1, 3).

---

### Task 1: Geometry, signatures and cell colours (`pixie-image`)

**Files:**
- Create: `crates/pixie-image/src/mosaic.rs`
- Modify: `crates/pixie-image/src/lib.rs` (add `pub mod mosaic;`)
- Test: in-file `#[cfg(test)]` module

**Interfaces:**
- Produces:
  - `pub const SIG_EDGE: u32 = 12;`
  - `pub struct Signature { pub avg: [u8; 3], pub tiny: Vec<u8> }` (`tiny` is `SIG_EDGE * SIG_EDGE * 3` bytes, the centred square, upright)
  - `pub fn signature(img: &image::DynamicImage) -> Signature` (the image is already upright; `avg` is the mean of `tiny`)
  - `pub fn grid_size(page_w_px: u32, page_h_px: u32, tile_px: u32) -> (u32, u32)` (columns, rows; at least 1 each; whole tiles, the grid centred and any margin filled by the outer tiles stretching by under a tile)
  - `pub fn tile_px(tile_mm: f32, dpi: u32) -> u32` (at least 1)
  - `pub fn cell_colours(target: &image::RgbImage, cols: u32, rows: u32) -> Vec<[u8; 3]>` (mean colour of each cell, row by row)
  - `pub fn crop_target(img: &image::DynamicImage, page_w_px: u32, page_h_px: u32, focus_x: f32, focus_y: f32, zoom: f32) -> image::RgbImage` (uses `collage::cover_crop`, resized to the page shape)

- [ ] **Step 1: Write the failing tests**
  - `grid_size_fits_whole_tiles`: `grid_size(1000, 500, 100) == (10, 5)`; `grid_size(1050, 520, 100) == (10, 5)`; `grid_size(50, 50, 100) == (1, 1)`.
  - `tile_px_follows_mm_and_dpi`: `tile_px(25.4, 300) == 300`; `tile_px(0.0, 300) == 1`.
  - `signature_of_a_flat_picture_is_that_colour`: a 40 × 20 red image gives `avg == [255,0,0]` and `tiny.len() == 432`.
  - `signature_takes_the_centre_square`: a 30 × 10 image, left third blue, middle third green, right third red, gives `avg` green.
  - `cell_colours_average_each_cell`: a 4 × 2 image, left half black, right half white, with `cols = 2, rows = 1` gives `[[0,0,0],[255,255,255]]`.
  - `crop_target_keeps_the_page_shape_and_zoom_picks_the_part`: output dimensions equal the page; at zoom 4 with focus (0,0) the output is the top-left quarter of the cover crop (compare a corner pixel of a gradient image).
- [ ] **Step 2: Run to verify failure** — `cargo test -p pixie-image mosaic` — Expected: FAIL, items not defined.
- [ ] **Step 3: Implement** the interfaces above. `signature` resizes the centred square to 12 × 12 with `imageops::FilterType::Triangle`.
- [ ] **Step 4: Run to verify pass** — `cargo test -p pixie-image mosaic` — Expected: PASS.

---

### Task 2: Tile assignment (`pixie-image`)

**Files:**
- Modify: `crates/pixie-image/src/mosaic.rs`
- Test: same file

**Interfaces:**
- Consumes: `cell_colours` output (`&[[u8;3]]`, row by row) and candidate average colours from `Signature::avg`.
- Produces:
  - `pub struct Assignment { pub picks: Vec<usize>, pub gap_used: u32 }` (`picks[cell]` indexes the candidates; `gap_used` is the minimum distance in tiles, Chebyshev, between two cells showing the same candidate, after any shrinking)
  - `pub fn assign(cells: &[[u8; 3]], cols: u32, candidates: &[[u8; 3]], gap: u32) -> Assignment`
  - Contract: deterministic for the same input; each cell gets a candidate whose colour is as near as the gap rule allows (nearest in a perceptual space such as CIELAB); no candidate repeats within `gap` tiles of itself; if that is impossible with this many candidates, use the largest gap that works (never below 1 when there are at least 2 candidates, 0 only for exactly 1 candidate); empty `candidates` returns an empty `picks`. Must handle 100,000 candidates and 10,000 cells in a few seconds in a release build (use a spatial index such as a k-d tree over the candidates, asking for several nearest and taking the first not blocked).

- [ ] **Step 1: Write the failing tests**
  - `each_cell_gets_the_nearest_colour`: cells `[[250,0,0],[0,250,0]]`, candidates red, green, blue: picks `[0,1]`.
  - `the_same_photo_never_touches_itself`: 6 × 6 cells all one colour, candidates 4 near-identical colours, `gap = 1`: no two cells at Chebyshev distance 1 have the same pick.
  - `a_wider_gap_is_kept_when_there_are_enough_photos`: 10 × 10 cells of one colour, 100 candidates of near-identical colour, `gap = 3`: `gap_used == 3` and no repeat within 3.
  - `the_gap_shrinks_when_photos_run_out`: 10 × 10 cells, 3 candidates, `gap = 5`: `gap_used < 5`, `gap_used >= 1`, still no touching.
  - `one_candidate_fills_everything`: `gap_used == 0`, all picks `0`.
  - `no_candidates_gives_no_picks`.
  - `same_input_same_answer`: two runs equal.
  - `many_candidates_are_quick`: 50,000 random candidates, 2,500 cells completes (assert only that it returns the right length; run in debug it must finish within the default test timeout).
- [ ] **Step 2: Run to verify failure** — `cargo test -p pixie-image mosaic::` — Expected: FAIL.
- [ ] **Step 3: Implement** `assign` (approach is the builder's: greedy in a fixed cell order with a k-d tree; keep it deterministic).
- [ ] **Step 4: Run to verify pass** — Expected: PASS.

---

### Task 3: Tinting, draft and full-size render (`pixie-image`)

**Files:**
- Modify: `crates/pixie-image/src/mosaic.rs`
- Test: same file; use `pixie_testutil` for image files as the collage tests do

**Interfaces:**
- Consumes: `Signature`, `Assignment`, `grid_size`, `tile_px`, `crop_target`, `decode::decode_scaled`, `collage::page_px`, `collage::MAX_SIDE_PX`.
- Produces:
  - `pub fn tint(tile: &mut image::RgbImage, towards: [u8; 3], blend: f32)` (`blend` 0.0 to 1.0; 0.0 leaves it exactly as it was; 1.0 shifts the tile's mean fully to `towards`, keeping its detail)
  - `pub struct Layout { pub cols: u32, pub rows: u32, pub tile: u32, pub cell_colours: Vec<[u8;3]> }`
  - `pub fn draft(layout: &Layout, picks: &[usize], tinies: &[&[u8]], blend: f32, draw_tile_px: u32) -> image::RgbImage` (each cell drawn from the 12 × 12 miniature of its pick, scaled to `draw_tile_px`, tinted)
  - `pub struct BuildTile { pub path: std::path::PathBuf }` and `pub fn build(layout: &Layout, picks: &[usize], tiles: &[BuildTile], page_w_px: u32, page_h_px: u32, blend: f32, progress: &mut dyn FnMut(u32, u32) -> bool, spare: &mut dyn FnMut(usize) -> Option<usize>) -> Result<image::RgbImage>` (decodes each distinct photo once, crops its centred upright square at tile size, tints, places it in every cell that uses it; `progress(done, total)` returns false to cancel, which returns `Err(PixieError::Invalid("Cancelled."))`; when a file cannot be decoded, `spare(failed_index)` gives another candidate index to use for those cells, and `Err` naming the file is returned only if it gives None)

- [ ] **Step 1: Write the failing tests**
  - `tint_zero_changes_nothing`: bytes identical before and after.
  - `tint_full_moves_the_mean_to_the_target_and_keeps_detail`: a two-tone tile tinted fully towards grey has a mean within 2 of grey and its two halves still differ.
  - `draft_has_the_size_of_the_grid`: `draft(...)` for 5 × 4 cells at 8 px is 40 × 32.
  - `draft_cell_comes_from_its_pick`: two cells with picks of a red and a blue miniature are red and blue.
  - `build_places_each_photo_in_its_cells`: two small JPEG files (red, blue), a 2 × 1 layout with picks `[0,1]`, page 200 × 100: left half red, right half blue.
  - `build_decodes_a_repeated_photo_once`: picks `[0,0,0,0]` with a decode counter wrapper (count via a closure the test passes through the `spare`/path hooks, or by deleting the file after the first read in a copy-on-open arrangement); assert the build succeeds with all four cells equal.
  - `build_can_be_cancelled`: `progress` returns false after the first call: `Err`, and the message is "Cancelled."
  - `build_uses_a_spare_for_an_unreadable_file`: candidate 0's path does not exist, `spare(0) == Some(1)`: the cell shows candidate 1.
  - `build_fails_with_words_when_no_spare`: `spare` returns None: `Err` whose text names the file.
  - `a_page_at_the_limit_is_refused_before_any_work`: width 16,385 gives the same message as `collage::page_px`.
- [ ] **Step 2: Run to verify failure** — `cargo test -p pixie-image mosaic::` — Expected: FAIL.
- [ ] **Step 3: Implement** the functions. Decode each original at the smallest size that still gives a square of `tile` pixels from the shorter side (`image::image_dimensions` then `decode_scaled`, as `collage::render` does). Process distinct photos in turn so memory stays at the page plus one photo.
- [ ] **Step 4: Run to verify pass** — Expected: PASS.

---

### Task 4: The built picture: pyramid, cache and save (`pixie-image`)

**Files:**
- Modify: `crates/pixie-image/src/mosaic.rs`
- Test: same file (tempdir)

**Interfaces:**
- Consumes: `build`'s `RgbImage`, `collage::save`.
- Produces:
  - `pub const VIEW_TILE: u32 = 256;`
  - `pub struct Built { pub width: u32, pub height: u32, pub dpi: u32, pub levels: u32 }` (`levels` is the number of zoom levels, level 0 the whole picture in one tile, the last the full size)
  - `pub fn write_built(img: &image::RgbImage, dir: &std::path::Path, dpi: u32) -> Result<Built>` (writes `dir/full.png` with the dpi recorded, via `collage::save`'s encoder, and the JPEG pyramid `dir/z/{z}/{x}/{y}.jpg`; written into a temporary sibling folder and swapped in, so a failure or cancel leaves the previous built picture intact)
  - `pub fn view_tile(dir: &std::path::Path, z: u32, x: u32, y: u32) -> Option<Vec<u8>>`
  - `pub fn save_built(dir: &std::path::Path, dst: &std::path::Path, dpi: u32) -> Result<()>` (PNG: copy `full.png` to a temp name beside `dst` and rename; JPEG: decode `full.png` and `collage::save` at quality 92; any other extension: `Err` "Save the mosaic as a .png or a .jpg file.")

- [ ] **Step 1: Write the failing tests**
  - `write_built_makes_the_pyramid`: a 600 × 300 image gives `levels == 3`, the top level is one tile whose size is 256 on its longer side, and the last level tiles cover 600 × 300 (`view_tile(dir, 2, 2, 1)` exists; `(2, 3, 0)` is None).
  - `an_older_built_picture_survives_a_failed_write`: make the destination read-only-proof by writing a first built picture, then call `write_built` with a path that fails (a file in the way of `z`); the first picture's `full.png` is still there and unchanged.
  - `save_built_copies_a_png_and_records_dpi`: output bytes equal `full.png`.
  - `save_built_makes_a_jpeg`: `.jpg` output decodes to the same size.
  - `save_built_refuses_other_extensions_with_words`.
- [ ] **Step 2: Run to verify failure** — Expected: FAIL.
- [ ] **Step 3: Implement.** Pyramid levels halve until the whole picture fits one 256 px tile.
- [ ] **Step 4: Run to verify pass** — `cargo test -p pixie-image mosaic` — Expected: PASS.

---

### Task 5: The colour index and the candidates (`pixie-index`)

**Files:**
- Modify: `crates/pixie-index/src/db.rs` (append one entry to `MIGRATIONS`: table `tile_sigs(photo_id INTEGER PRIMARY KEY REFERENCES photos(id) ON DELETE CASCADE, mtime INTEGER NOT NULL, size INTEGER NOT NULL, avg BLOB NOT NULL, tiny BLOB NOT NULL)`)
- Create: `crates/pixie-index/src/mosaic.rs`; add `pub mod mosaic;` in `lib.rs`
- Test: in-file; migration test in `db.rs` following the existing `MIGRATIONS[..n]` pattern

**Interfaces:**
- Consumes: `Db::conn()`, `Filter`/`PhotoQuery` (`photo_ids_in`), `list_picked_ids`, `id_list`, `SHARE_PRIVATE`.
- Produces on `Db`:
  - `pub enum TileSource { Library, Ids(Vec<PhotoId>) }` (Browse filters and Picked are turned into `Ids` by the caller with `photo_ids_in` and `list_picked_ids`)
  - `pub fn tile_candidates(&self, src: &TileSource) -> Result<Vec<Candidate>>` where `pub struct Candidate { pub id: PhotoId, pub path: String, pub mtime: i64, pub size: i64 }`: only `kind = 0 AND missing = 0 AND dup = 0 AND share != SHARE_PRIVATE`, in id order, any number of ids (via `json_each`)
  - `pub fn tiles_without_sig(&self, cands: &[Candidate]) -> Result<Vec<Candidate>>` (those with no row, or a row whose mtime or size differs)
  - `pub fn set_tile_sigs(&self, rows: &[(PhotoId, i64, i64, [u8; 3], Vec<u8>)]) -> Result<()>`
  - `pub fn tile_sigs(&self, ids: &[PhotoId]) -> Result<Vec<(PhotoId, [u8; 3], Vec<u8>)>>` (same order as `ids`, skipping ids with none)
  - `pub fn forget_tile_sigs(&self) -> Result<()>` (not needed by the UI; omit if nothing uses it)

- [ ] **Step 1: Write the failing tests**
  - `candidates_leave_out_videos_private_duplicates_and_missing`: five photos each with one reason to leave out, one good one; `Library` returns just the good one.
  - `ids_source_keeps_only_eligible_ids`: `Ids` with the good one and an ineligible one returns the good one.
  - `a_long_id_list_works`: 5,000 ids as `Ids` (follow the existing 2,500-id test).
  - `a_changed_file_needs_a_new_signature`: after `set_tile_sigs`, `tiles_without_sig` is empty; after changing the photo's `mtime`, it returns it.
  - `signatures_come_back_in_order_and_round_trip`.
  - `a_database_from_before_this_version_gets_the_table` (migration test).
  - `deleting_a_photo_deletes_its_signature` (cascade).
- [ ] **Step 2: Run to verify failure** — `cargo test -p pixie-index mosaic` — Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run to verify pass** — `cargo test -p pixie-index` — Expected: PASS (whole crate, the migration counts change).

---

### Task 6: Commands and the viewer route (`src-tauri`)

**Files:**
- Create: `src-tauri/src/mosaic.rs` (`mod mosaic;` in `lib.rs`; register the commands in the handler list; route `Some("mosaic")` in the protocol handler beside `Some("poster")`)
- Modify: `src-tauri/src/state.rs` (field `mosaic: Arc<MosaicJob>` holding the cancel flag and one run at a time), `src-tauri/src/lib.rs`
- Test: in-file unit tests for the helpers (no window)

**Interfaces:**
- Consumes: Task 1 to 5 interfaces; `ThumbService`/`thumbs::ensure_thumbnail` (Grid class) to read each candidate's thumbnail for its signature; `query_of(...)` for the Browse source; `collage::say`-style error text.
- Produces (Tauri commands, all returning `Result<_, String>` like `save_collage`):
  - `get_mosaic() -> Option<String>` and `set_mosaic(doc: String)`: the window's settings kept as one text in the `settings` table, key `"mosaic"`, limit 1,000,000 bytes (the same pattern as `get_collage`/`set_collage`).
  - `mosaic_index(source: MosaicSource) -> IndexSummary`: makes missing signatures with bounded parallelism on the blocking pool, emits event `mosaic-index` `{done, total}`, returns `{ candidates: usize, skipped: usize }` (`skipped` = files that could not be read). `MosaicSource` is `{ kind: "library" | "browse" | "picked", order, folder_id, media_kind, bounds, max_id, since, until, search }` with the same fields as `pick_all_matching`.
  - `mosaic_draft(args: DraftArgs) -> DraftResult`: `DraftArgs` has the source, target photo id, `focus_x`, `focus_y`, `zoom`, page `w_mm`, `h_mm`, `dpi`, `tile_mm`, `gap`, `blend`; returns `{ png_base64: String, cols, rows, tiles, photos, gap_used }`. Cheap: signatures only, a draft of at most 1,200 px on its longer side.
  - `mosaic_build(args: DraftArgs) -> BuiltInfo` (`Built` plus the key it was made from): runs `pixie_image::mosaic::build` into the cache folder `<cache>/mosaic/` via `write_built`; emits `mosaic-build` `{done, total}`; stores `{key, width, height, dpi, levels}` in the settings table under `"mosaic_built"`.
  - `mosaic_cancel()`: sets the flag; a cancelled build returns the text "Cancelled." and leaves the previous built picture.
  - `get_mosaic_built() -> Option<BuiltInfo>` for the window on opening.
  - `mosaic_save(dst: String)`: `save_built`.
  - Protocol route `/mosaic/{z}/{x}/{y}` returns the JPEG from `view_tile` or 404.
- The `key` is a string of the settings that decide the picture (source description with the candidate count, target id, mtime and size, crop, page, dpi, tile size, gap, blend); the window compares it with its own to know the built picture is out of date, so build it in one function `mosaic_key(&DraftArgs, candidates)` that both draft and build use.

- [ ] **Step 1: Write the failing tests**
  - `the_key_changes_when_any_setting_changes` (one assertion per field).
  - `a_stored_mosaic_document_comes_back_and_a_huge_one_is_refused` (copy of the collage tests).
  - `an_empty_source_says_so_in_words` (the helper that turns candidates into an error: "No photos are available to use as tiles.").
  - `a_page_over_the_limit_is_refused_with_the_collage_message`.
  - `a_second_build_while_one_runs_is_refused`.
- [ ] **Step 2: Run to verify failure** — `cargo test -p pixie-app mosaic` (batch with the final run if the crate is slow to build) — Expected: FAIL.
- [ ] **Step 3: Implement** the commands. Do the signature work the way the thumbnail service does for background work: a few worker threads, `ensure_thumbnail` for the Grid class, `decode` of that JPEG, `mosaic::signature`. An unreadable file counts as `skipped`.
- [ ] **Step 4: Run to verify pass** — `cargo check --workspace --all-targets` clean, then `cargo test -p pixie-app mosaic` — Expected: PASS.

---

### Task 7: The tool in the window

**Files:**
- Create: `ui/src/lib/mosaic.ts`, `ui/src/lib/mosaic.test.ts`, `ui/src/lib/MosaicTool.svelte`, `ui/src/lib/MosaicViewer.svelte`
- Modify: `ui/src/lib/api.ts` (wrappers `getMosaic`, `setMosaic`, `mosaicIndex`, `mosaicDraft`, `mosaicBuild`, `mosaicCancel`, `getMosaicBuilt`, `mosaicSave`; listeners for `mosaic-index` and `mosaic-build`), `ui/src/lib/tools.ts` (entry `{ id: 'mosaic', group: 'create', label: 'Photomosaic', built: true, ... }` after Collage), `ui/src/lib/ToolsTab.svelte` (mount like Collage: it works on the whole library, so add it to whatever `worksOnWholeLibrary` and `wide` use), `ui/src/App.svelte` only if the Browse query must be passed down (it is already available to `ToolsTab` as `source`)
- Test: `mosaic.test.ts`; `tooltips.test.ts` runs unchanged

**Interfaces:**
- Consumes: Task 6 commands; `collage.ts` helpers; `collagePapers.ts`; `ColourPicker` is not needed.
- Produces in `mosaic.ts`:
  - `interface MosaicDoc { v: 1; target: number | null; focusX: number; focusY: number; zoom: number; paper: string; landscape: boolean; tileMm: number; source: 'library' | 'browse' | 'picked'; gap: number; blend: number }`
  - `newMosaicDoc(locale?: string): MosaicDoc` (defaults: no target, centre, zoom 1, `defaultPaperId`, portrait, `tileMm` 15, library, `gap` 8, `blend` 0)
  - `parseMosaicDoc(text: string | null): MosaicDoc` (bad or old text gives the defaults; every field clamped: zoom 1 to 4, focus 0 to 1, `tileMm` 3 to 100, `gap` 1 to 50, `blend` 0 to 1)
  - `serializeMosaicDoc(d: MosaicDoc): string`
  - `tileCount(pageWmm: number, pageHmm: number, tileMm: number): { cols: number; rows: number; tiles: number }`
  - `sourceLine(tiles: number, photos: number): string` ("About 3,200 tiles, from 18,000 photos.", using `plural`; with fewer photos than tiles: adds "Some photos will repeat.")
  - `isStale(doc: MosaicDoc, built: { key: string } | null, currentKey: string | null): boolean`

- [ ] **Step 1: Write the failing tests** in `mosaic.test.ts`: defaults; `parseMosaicDoc(null)` and garbage give defaults; clamping of each field; round trip; `tileCount(210, 297, 15) == { cols: 14, rows: 19, tiles: 266 }`; `sourceLine(1, 1)` and `sourceLine(3200, 18000)` wording and plurals; `isStale` is true with no built picture, false when keys match, true when they differ.
- [ ] **Step 2: Run to verify failure** — `cd ui && npx vitest run mosaic` — Expected: FAIL.
- [ ] **Step 3: Implement `mosaic.ts`**, then the components:
  - `MosaicTool.svelte` (props like `CollageTool`: `active`, `defaultDir`, plus the Browse `source` and the target `photo`). Left: the target (the chosen photo shown with drag-to-set-focus and a Zoom slider 1 to 4, a button "Use the photo chosen in Browse"), Paper (the Collage paper list and orientation), "Tile size (mm):", "Tiles from:" radio (Whole library, What Browse shows now, Picked photos), "Keep repeats apart (tiles):", "Blend with target:" slider with the percentage. Middle: the live draft (a `<img>` of the base64 PNG, redrawn 250 ms after the last change, ignoring an out-of-date reply), and `sourceLine`. Index progress shows "Preparing photos: n of N" with the first-time note "The first time takes a while; later mosaics start straight away." Buttons: **Build** (`class="primary"` until built; disabled when no target, no tiles or `sizeProblem`), **Cancel** while building, then **Save** (`class="primary"` once built; disabled and titled "Build again first: the settings have changed" when `isStale`, or "Build the picture first" when none). Save uses `@tauri-apps/plugin-dialog` `save` with the PNG and JPEG filters and `defaultSavePath`, then `mosaicSave`; "Saving…", "Saved." and errors as in `CollageTool`. Notes for skipped photos ("N photos could not be read and were left out.") and the shrunk gap ("Photos are repeated as little as the library allows: at least N tiles apart.").
  - `MosaicViewer.svelte`: Leaflet `L.map` with `crs: L.CRS.Simple`, `L.tileLayer` on `mosaicTileTemplate()` (add to `url.ts`, same `pixie.localhost` rule as `tileTemplate`), `tileSize: 256`, `minZoom: 0`, `maxZoom: levels - 1`, bounds from `width` and `height`, zoom and pan; shows when a built picture exists.
  - Persist the doc with `setMosaic` (debounced) and load it with `getMosaic` on mount, as `CollageTool` does.
  - Every control gets a whole-sentence `title`; icon-only ones an `aria-label`.
- [ ] **Step 4: Run to verify pass** — `cd ui && npx vitest run` — Expected: PASS (all tests, including `tooltips.test.ts`), then `npx svelte-check` — Expected: 0 errors, 0 warnings.

---

### Task 8: Ask Pixie guide, docs and the whole-suite run

**Files:**
- Modify: `ui/src/lib/askGuide.ts` (a Photomosaic section: where it is, the steps, the source choices, that the first run prepares photos, Build then Save), `ui/src/lib/askLocal.ts` (remove `'mosaic'` from the Collage entry's keywords; add a `mosaic` entry with `goTo: 'create-mosaic'` and make the id resolvable the way `create-collage` is), `ask-proxy/test/fixtures/context.json` (regenerate), `docs/superpowers/specs/2026-11-04-photomosaic-design.md` (add one line under "Matching": each photo also keeps a 12 × 12 miniature so the draft shows real tiles), `docs/superpowers/specs/2026-10-29-tool-tabs-design.md` (Photomosaic is built)
- Test: existing `askGuide.test.ts`, `askLocal` tests, plus a new case that "photomosaic" and "mosaic" questions reach the new entry and "collage" still reaches Collage

**Interfaces:**
- Consumes: the tool id `mosaic` and its `goTo` target.

- [ ] **Step 1: Write the failing test** in the ask tests: a question "how do I make a photomosaic" returns the `mosaic` entry; "make a collage" still returns the collage entry.
- [ ] **Step 2: Run to verify failure** — `cd ui && npx vitest run askLocal askGuide` — Expected: FAIL.
- [ ] **Step 3: Implement** the guide and keyword changes; regenerate the fixture with `cd ui && WRITE_ASK_FIXTURE=1 npx vitest run askSettings`.
- [ ] **Step 4: Run everything once** — `cd ui && npx vitest run`; `cd ask-proxy && npx vitest run`; `cargo check --workspace --all-targets`; `cargo test -p pixie-image -p pixie-index`; `cargo test -p pixie-app`; `cd ui && npx svelte-check` — Expected: all PASS, no warnings. Then start the app (`just dev` or the project's usual command, with the separate target dir) and try it: choose a photo, adjust, Build, zoom the result, Save as PNG and as JPEG, change a setting and see Save grey out.
- [ ] **Step 5: Stop and report.** Do not commit. List what changed and ask the user whether to "c & p".
