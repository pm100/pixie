# Collage Editor Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the form-based Collage tool with a free-form editor: a page in the middle with movable, resizable, croppable, overlapping frames, snapping, border and shadow, the Working-on photos in a list on the right, a settings pop-up behind a gear, three ways to start (Pixie lays them out, a template, blank), one collage kept in the database, and page sizes with the user's region first.

**Architecture:** The saved collage is one JSON document (`CollageDoc`, owned by the UI) stored by Rust as an opaque string in the `settings` table. All editing logic (frame geometry, snapping, layouts, region sizes, crop and dpi maths) is pure TypeScript in small tested modules; Svelte components only draw and forward pointer events. Rust draws the saved PNG/JPEG from a list of frames in the same order and with the same rules as the CSS preview.

**Tech Stack:** Rust (`image` 0.25, already used), Tauri commands in `src-tauri/src/collage.rs`, Svelte 5 + TypeScript + vitest, `@tauri-apps/plugin-dialog` `save()`.

**Spec:** `docs/superpowers/specs/2026-11-03-collage-editor-design.md`. It replaces `docs/superpowers/plans/2026-11-01-collage.md`, whose code is built but uncommitted: reuse what is named below, delete what is replaced.

## Global Constraints

- Never commit or push (AGENTS.md); there are no commit steps. Leave changes uncommitted.
- No compiler warnings: `cargo check --workspace --all-targets` stays clean; no `#[allow]`; delete dead code (the old cell/gap renderer goes).
- Build and test with a separate cargo target dir, and the vcpkg environment: `export VCPKG_ROOT=C:/work/vcpkg-pixie PATH="/c/work/vcpkg-pixie/installed/x64-windows/bin:$PATH" CARGO_TARGET_DIR=target-claude`. Batch Rust runs: test the changed crate, run `pixie-app` and the full suite once at the end.
- Buttons: white by default; `class="primary"` for the one main action (Save in the bar, Done in the pop-up); `class="danger"` for destructive ones. No per-component button styling.
- Never put a control mid-sentence; label then control, units in the label ("Gap (mm)").
- Page padding from `--page-pad` / `--page-pad-x` / `--page-pad-y`; no padding numbers of its own.
- Every control has a whole-sentence `title`; icon-only or unlabelled controls also an `aria-label` (`ui/src/lib/tooltips.test.ts` enforces this).
- Only pluralise a count above one: `plural` in `ui/src/lib/format.ts`.
- The button is called **Save**, not "Save collage…".
- Only `kind === 'photo'` items go in a collage (videos are left out with a note, as Print does).
- No new dependencies, so `just notices` is not needed.
- Files are LF except where `git diff --stat` shows otherwise; edit with the Edit tool, not a script that rewrites line endings.

## Review Focus

1. A frame whose photo has left the library: drawn as an empty grey frame with a note; Save stops and names the photo, writes nothing (Tasks 2, 8).
2. A stored collage that is corrupt, empty, from a newer version, or has out-of-range numbers: opens as a fresh page, never throws (Task 3).
3. Frames dragged or resized off the page, to nothing, or on a page whose paper changes: stay inside the page and at least 5% in size (Task 4); no snap pulls a frame off the page (Task 5).
4. Overlap: the saved file stacks frames and shadows in the same order as the preview (Task 1).
5. A poster-size page with several large photos: one photo decoded at a time, no out-of-memory; 24-photo cap on the automatic layout (Tasks 1, 4).
6. A language with no country (`en`) or none at all: metric sizes, never inches (Task 3).

---

### Task 1: Render frames (Rust)

**Files:**
- Modify: `crates/pixie-image/src/collage.rs` (tests in the same file)

**Interfaces:**
- Produces:
  - `pub struct Frame { pub path: PathBuf, pub x: f32, pub y: f32, pub w: f32, pub h: f32, pub focus_x: f32, pub focus_y: f32, pub zoom: f32 }` (position and size as fractions of the page)
  - `pub struct Look { pub corner_mm: f32, pub border_mm: f32, pub border: [u8; 3], pub shadow: bool }`
  - `pub struct CollageRequest { pub page_w_mm: f32, pub page_h_mm: f32, pub dpi: u32, pub background: [u8; 3], pub look: Look, pub frames: Vec<Frame> }` (frames bottom to top)
  - `pub fn cover_crop(src_w: u32, src_h: u32, cell_w: u32, cell_h: u32, focus_x: f32, focus_y: f32, zoom: f32) -> (u32, u32, u32, u32)` (zoom 1 is the old rule; zoom above 1 shrinks the visible rectangle by that factor around the focus point; zoom is clamped to 1..=4)
  - `pub fn render(req: &CollageRequest) -> Result<RgbImage>`
- Keeps unchanged: `templates()`, `CellFrac`, `Template`, `PxRect`, `MAX_SIDE_PX`, `page_px`, `save`, the PNG pHYs writer, `inside_corners`.
- Deletes: `cell_rects`, `effective_dpi`, `CellPhoto`, the old `CollageRequest`, and their tests (the TypeScript side now does cell insets and dpi).

- [ ] **Step 1: Update and add tests** (replace the tests of the deleted items; keep the page, saving and dpi-recording tests; the `request`/`photo` helpers become frame builders). Names and assertions:
  - `cover_crop_with_zoom_two_shows_half_the_width_and_height`: `cover_crop(400, 200, 100, 100, 0.5, 0.5, 2.0) == (150, 50, 100, 100)`; zoom 1 gives the old results; zoom 9 behaves as 4; zoom 0.2 as 1.
  - `frames_are_drawn_bottom_to_top`: a red frame, then a blue frame overlapping its right half; the overlap pixel is blue; the order reversed makes it red.
  - `a_frame_is_placed_by_its_fractions_and_the_rest_is_background`: one frame at x .25, y .25, w .5, h .5 on a 100×100 px page (dpi 72, 25.4 mm): centre red, pixel (5,5) background.
  - `a_border_is_a_band_inside_the_frame_edge_around_the_photo`: border 5 px in blue on a red photo: pixel at the frame edge is blue, 8 px in is red.
  - `corners_are_rounded_to_the_background_with_or_without_a_border` (outline rounded, border included).
  - `a_shadow_darkens_below_and_right_of_a_frame_only_when_on`: with `shadow: true` a pixel 3 px below-right outside the frame is darker than the white background and a pixel 40% of the page away is unchanged; with `shadow: false` both equal the background.
  - `a_shadow_falls_under_the_frames_above_it_not_on_them`: two overlapping frames; the upper frame's pixels are its own photo colour.
  - `zoom_and_focus_choose_the_visible_part` (left half red, right half blue photo, zoom 2, focus 1.0 shows blue only).
  - `a_frame_off_the_page_is_clamped_and_a_zero_size_frame_is_one_pixel` (x .9 w .5; w 0): no panic, pixels inside the page.
  - `a_photo_that_cannot_be_read_fails_naming_the_file` (kept, with frames).
  - `a_poster_size_page_renders_and_gives_back_its_physical_size` (kept, 4 frames).
- [ ] **Step 2: Run** `cargo test -p pixie-image collage`. Expected: FAIL (does not compile / assertions fail).
- [ ] **Step 3: Implement** `render`: page filled with `background`; for each frame in order: pixel rect from the fractions (rounded, clamped inside the page, at least 1×1); if `look.shadow`, draw its shadow first (offset and blur radius both 1.5% of the shorter page side in pixels, 35% black, from the frame's rounded-rect mask blurred with `imageops::blur` on a `GrayImage` covering only the frame plus 3 blur radii, composited onto the page); fill the rect with the border colour when `border_mm > 0` (inset the photo by the border width, at least 1 px of photo left); decode one photo (`decode_scaled` with the edge as before, from `image_dimensions` and the cover scale and zoom), `cover_crop`, `Lanczos3` resize to the inset rect, paste. Corner radius `corner_mm` (clamped to half the shorter side of the frame) applies to the outer outline: pixels outside the rounded rect are skipped for border and photo alike. Errors name the file (`"{name} could not be read: …"`).
- [ ] **Step 4: Run** the tests again and `cargo check --workspace --all-targets`. Expected: PASS, no warnings (the app crate will not compile until Task 2: check `-p pixie-image` here).

### Task 2: Commands and storage (Rust)

**Files:**
- Modify: `src-tauri/src/collage.rs`, `src-tauri/src/lib.rs` (the `invoke_handler` list: remove `collage::collage_dpi_check`, add `collage::get_collage`, `collage::set_collage`)

**Interfaces:**
- Consumes: Task 1's `CollageRequest`, `Frame`, `Look`, `render`, `save`, `templates`.
- Produces (Tauri):
  - `collage_templates() -> Vec<Template>` (unchanged)
  - `get_collage(state) -> Result<Option<String>, String>`: the stored document text.
  - `set_collage(state, doc: String) -> Result<(), String>`: stores it under the settings key `"collage"`; refuses text over 1,000,000 bytes with a message.
  - `save_collage(state, args: SaveArgs, dst: String) -> Result<(), String>` with `SaveArgs { paper_w_mm, paper_h_mm, dpi, background: String, corner_mm, border_mm, border_colour: String, shadow: bool, frames: Vec<FrameArg> }` and `FrameArg { photo_id: i64, x, y, w, h, focus_x, focus_y, zoom }` (all `f32` but the id), `#[derive(Deserialize)]`.
- Deletes: `collage_dpi_check`, `CollageArgs`, `CellArg`.

- [ ] **Step 1: Write failing tests** in `src-tauri/src/collage.rs`: `colours_are_read_from_hex` (kept); `a_stored_document_comes_back_as_it_was` and `a_document_over_the_limit_is_refused` against a small helper pair `store(db, doc)` / `load(db)` that the commands call (use the `Db` test constructor used elsewhere in `src-tauri`; if none is reachable from this crate, test only the size rule as `check_doc_size(&str) -> Result<(), String>`).
- [ ] **Step 2: Run** `cargo test -p pixie-app collage`. Expected: FAIL.
- [ ] **Step 3: Implement** the commands. `save_collage` looks every `photo_id` up with `state.db.get_photo`; a photo that is gone or `missing` returns `"A photo in the collage is no longer in the library."` and nothing is written; builds the `CollageRequest` and runs `render` + `save(…, 92)` in `spawn_blocking`, mapping errors with the existing `say`.
- [ ] **Step 4: Run** `cargo check --workspace --all-targets` (no warnings), then `cargo test -p pixie-image -p pixie-app`. Expected: PASS.

### Task 3: Document model and paper sizes (TypeScript)

**Files:**
- Create: `ui/src/lib/collageDoc.ts`, `ui/src/lib/collageDoc.test.ts`, `ui/src/lib/collagePapers.ts`, `ui/src/lib/collagePapers.test.ts`
- Modify: `ui/src/lib/api.ts` (replace the collage block from the first build: keep `CollageCell`, `CollageTemplate`, `collageTemplates`; add `getCollage = () => invoke<string | null>('get_collage')`, `setCollage = (doc: string) => invoke<void>('set_collage', { doc })`, `saveCollage = (args: SaveArgs, dst: string) => invoke<void>('save_collage', { args, dst })`, and the `SaveArgs`/`FrameArg` types mirroring Task 2; remove `collageDpiCheck`, `CollageArgs`, `CollagePhoto`)

**Interfaces:**
- Produces in `collagePapers.ts`:
  - `export interface PaperSize { id: string; label: string; w: number; h: number }` (mm, portrait)
  - `export interface PaperGroup { label: string; sizes: PaperSize[] }`
  - `export function usesInches(locale: string | undefined): boolean`: true only for an explicit country in US, CA, MX, PH (`Intl.Locale(locale).region`, **not** `maximize()`, so plain `en` is metric); false for undefined or unparsable.
  - `export function paperGroups(locale?: string): PaperGroup[]`: groups "Prints (inches)", "Prints (cm)", "Posters and frames", "Square (books, calendars)" with the sizes of the spec's Page sizes section (Letter added to the inches group), the region's print group first; custom is not in the list.
  - `export function defaultPaperId(locale?: string): string` (`'8x10in'` or `'20x30cm'`) and `export function findPaper(id: string): PaperSize | undefined`.
- Produces in `collageDoc.ts`:
  - `export interface CollageFrame { id: string; photo: number; x: number; y: number; w: number; h: number; fx: number; fy: number; zoom: number }`
  - `export interface CollageDoc { v: 1; paper: string /* a paper id or 'custom' */; customW: number; customH: number /* cm */; landscape: boolean; dpi: number | null; background: string; gap: number; corner: number; border: number; borderColour: string; shadow: boolean; snapPage: boolean; snapFrames: boolean; template: string | null; frames: CollageFrame[] }`
  - `export function newDoc(locale?: string): CollageDoc` (starting values from the spec: gap 4, corner 0, border 0, `#ffffff`, shadow off, both snaps on, paper `defaultPaperId(locale)`, portrait, `dpi: null` meaning the default for the size)
  - `export function parseDoc(text: string | null, locale?: string): CollageDoc`: never throws; null, bad JSON, a wrong `v`, or non-object gives `newDoc`; each number is clamped to its range (gap 0..50, corner 0..100, border 0..50, custom 1..100 cm, zoom 1..4, frame fractions into the page, w/h at least 0.05), an unknown paper id becomes the default, frames that are not objects are dropped.
  - `export const serializeDoc = (d: CollageDoc): string`
  - `export function pageMm(d: CollageDoc): { w: number; h: number }` (portrait size swapped when `landscape`; custom from cm)
  - `export function toSaveArgs(d: CollageDoc): SaveArgs` (frames in array order, resolution from `d.dpi ?? defaultDpi(...)`).
- Consumes: `defaultDpi` from `collage.ts` (kept).

- [ ] **Step 1: Write failing tests.** `collagePapers.test.ts`: `usesInches('en-US')`, `'en-CA'`, `'es-MX'`, `'fil-PH'` true; `'en'`, `'en-GB'`, `'de-DE'`, `''`, `undefined`, `'xx_??'` false; `paperGroups('en-US')[0].label === 'Prints (inches)'` and `paperGroups('de-DE')[0].label === 'Prints (cm)'`; every size appears in exactly one group and ids are unique; `findPaper('8x10in')` is 203.2 × 254; `defaultPaperId` per region. `collageDoc.test.ts`: `parseDoc(null)`, `parseDoc('')`, `parseDoc('{')`, `parseDoc('{"v":2}')`, `parseDoc('[]')` all equal `newDoc()`; a doc round-trips through `serializeDoc`/`parseDoc`; `parseDoc` clamps `{"v":1,"gap":-5,"frames":[{"id":"a","photo":1,"x":0.9,"y":0.9,"w":0.5,"h":0.0,"zoom":9}]}` into the page with w/h ≥ 0.05, zoom 4, gap 0; drops frames that are strings; `pageMm` swaps for landscape; `toSaveArgs` keeps frame order and uses 150 dpi for a 70 × 100 cm page.
- [ ] **Step 2: Run** `npx vitest run collageDoc collagePapers` in `ui/`. Expected: FAIL.
- [ ] **Step 3: Implement** the two modules and the `api.ts` changes.
- [ ] **Step 4: Run** the tests. Expected: PASS. (`svelte-check` will complain about `CollageTool.svelte` until Task 8; that is expected here.)

### Task 4: Frame geometry, crop and layouts (TypeScript)

**Files:**
- Create: `ui/src/lib/collageFrames.ts`, `ui/src/lib/collageFrames.test.ts`, `ui/src/lib/collageLayout.ts`, `ui/src/lib/collageLayout.test.ts`
- Modify: `ui/src/lib/collage.ts` and `collage.test.ts` (keep `MAX_SIDE_PX`, `defaultDpi`, `pagePx`, `sizeProblem`, `LOW_DPI`, `focusFromDrag`; change `overflow(srcW, srcH, cellW, cellH, zoom = 1)` to include zoom; delete `COLLAGE_PAPERS`, `defaultTemplate`, `fill`, `leftOut`; add `coverCrop` and `frameDpi` below)

**Interfaces:**
- Produces in `collage.ts`:
  - `coverCrop(srcW, srcH, cellW, cellH, fx, fy, zoom): { x: number; y: number; w: number; h: number }` mirrors Task 1's `cover_crop` and must give the same numbers (the same four test cases).
  - `frameDpi(photoW: number, photoH: number, frame: { w: number; h: number; fx: number; fy: number; zoom: number }, pageMm: { w: number; h: number }): number`: the pixels of the visible part divided by the printed width of the frame in inches. Examples: 3000×2000 photo in a 150×100 mm frame, zoom 1: 508; in 600×400 mm: 127; zoom 2 halves each.
- Produces in `collageFrames.ts` (rects are page fractions `Rect { x, y, w, h }`):
  - `export const MIN_FRAME = 0.05`; `export type Rect`; `export type Handle = 'n' | 'ne' | 'e' | 'se' | 's' | 'sw' | 'w' | 'nw'`
  - `moveRect(r: Rect, dx: number, dy: number): Rect` (clamped inside the page)
  - `resizeRect(r: Rect, handle: Handle, dx: number, dy: number, pageAspect: number, keepShape: boolean): Rect` (`pageAspect` = page width / page height so a corner can keep the frame's real shape; opposite edge stays put; at least `MIN_FRAME`; inside the page)
  - `newFrameRect(photoW: number, photoH: number, pageMm: { w: number; h: number }, at: { x: number; y: number }): Rect` (the photo's shape, about a quarter of the page area, centred on `at`, moved inside the page)
  - `firstFreeSpot(existing: Rect[], size: Rect): { x: number; y: number }` (first position on a coarse grid, left to right then down, that overlaps no existing rect; the centre if none is free)
  - `topFrameAt<T extends Rect>(frames: T[], x: number, y: number): T | null` (frames are bottom to top; the last that contains the point)
  - `bringToFront<T extends { id: string }>(frames: T[], ids: string[]): T[]`, `sendToBack` (same signature; keep the relative order of the moved ones), `swapPhotos(frames, aId, bId)` (swaps `photo`, resets `fx`, `fy` to .5 and `zoom` to 1 on both).
- Produces in `collageLayout.ts`:
  - `export const AUTO_LAYOUT_MAX = 24`
  - `templateFrames(cells: CollageCell[], pageMm: { w: number; h: number }, gapMm: number): Rect[]` (each cell inset by half the gap in mm on every side, never below `MIN_FRAME`, as the first build's `cell_rects` did)
  - `justifiedRows(shapes: { w: number; h: number }[], pageMm: { w: number; h: number }, gapMm: number): { rects: Rect[]; used: number }`: photos in order, each keeping its shape; the number of rows is the one that covers the most area of the page; each row scaled to the page width, the block shrunk uniformly if too tall, centred; at most `AUTO_LAYOUT_MAX` are used (`used` says how many).

- [ ] **Step 1: Write failing tests.** `collage.test.ts`: `coverCrop` returns `(100,0,200,200)` for `(400,200,100,100,.5,.5,1)`, `(150,50,100,100)` with zoom 2, same as Rust; `frameDpi` examples above; `overflow` with zoom 2 doubles the overflow plus the cell size. `collageFrames.test.ts`: `moveRect` stops at every page edge; `resizeRect` from `se` with `keepShape` keeps the pixel aspect on a 2:3 page; `resizeRect` never goes below `MIN_FRAME` (drag past the opposite edge) and never leaves the page; `newFrameRect` for a 3:2 photo on a 2:3 page is about 25% of the area and inside the page when placed at a corner; `firstFreeSpot` skips occupied rects; `topFrameAt` returns the later of two overlapping frames and null on empty space; `bringToFront`/`sendToBack` keep relative order; `swapPhotos` resets crops. `collageLayout.test.ts`: `templateFrames` of a 2×2 grid with gap 4 mm on 100×100 mm gives four 0.46×0.46 inset rects 0.04 apart; a gap bigger than a cell gives `MIN_FRAME`; `justifiedRows` of 4 square photos on a square page gives a 2×2 block, in order, each square, no overlaps, inside the page, centred; a panorama and a portrait keep their shapes; 30 photos give `used === 24`; zero photos give no rects.
- [ ] **Step 2: Run** `npx vitest run collage` in `ui/`. Expected: FAIL.
- [ ] **Step 3: Implement** the modules.
- [ ] **Step 4: Run** the tests. Expected: PASS.

### Task 5: Snapping (TypeScript)

**Files:**
- Create: `ui/src/lib/collageSnap.ts`, `ui/src/lib/collageSnap.test.ts`

**Interfaces:**
- Consumes: `Rect`, `Handle` from Task 4.
- Produces:
  - `export interface Guide { axis: 'x' | 'y'; at: number; from: number; to: number; gapMm?: number }` (a line at fraction `at` across the page, drawn between `from` and `to` on the other axis; `gapMm` when the guide marks an equal gap)
  - `export interface SnapContext { others: Rect[]; pageMm: { w: number; h: number }; marginMm: number; page: boolean; frames: boolean; thresholdPx: number; pagePx: { w: number; h: number } }` (`marginMm` is half the gap; `pagePx` is the page as drawn on screen, to turn the threshold into fractions)
  - `snapMove(r: Rect, ctx: SnapContext): { rect: Rect; guides: Guide[] }`: the left, centre and right (and top, centre, bottom) of `r` snap to the page edges, the margin, the page centre lines (when `ctx.page`) and to the edges and centres of `ctx.others` (when `ctx.frames`), on each axis independently, to the nearest within `thresholdPx`; also snaps so the gap to a neighbour equals the gap between two other frames or the gap setting.
  - `snapResize(r: Rect, handle: Handle, ctx: SnapContext): { rect: Rect; guides: Guide[] }`: only the moving edges snap.

- [ ] **Step 1: Write failing tests:** a frame whose left edge is 3 px from the page edge snaps to 0 with one vertical guide at 0; at 10 px it does not move and has no guide; with `page: false` it never snaps to the page; left edge snaps to another frame's right edge plus the margin; centres align to a neighbour's centre with a guide; equal spacing: three frames in a row, moving the third to within threshold of the gap between the first two snaps to that gap and the guide carries `gapMm`; `snapResize` with the `e` handle moves only the right edge; a snap never moves a frame outside the page (a neighbour at the page edge cannot pull a frame past it); two near candidates choose the nearer.
- [ ] **Step 2: Run** `npx vitest run collageSnap`. Expected: FAIL.
- [ ] **Step 3: Implement** with a helper that collects candidate positions per axis and picks the nearest within the threshold.
- [ ] **Step 4: Run** the tests. Expected: PASS.

### Task 6: The compose surface (Svelte)

**Files:**
- Create: `ui/src/lib/CollagePage.svelte`

**Interfaces:**
- Consumes: Tasks 3-5.
- Produces: `CollagePage` with props `{ doc: CollageDoc; photos: Map<number, Photo>; selected: string[]; onselect: (ids: string[]) => void; onchange: (frames: CollageFrame[]) => void; ondrop: (photoId: number, at: { x: number; y: number }, onFrame: string | null) => void }` (`doc` is read only here; every edit goes out through `onchange`, so the parent owns the state and the autosave).

- [ ] **Step 1: Draw the page.** Sized to fit its container (bind its available width and height; keep the page's aspect), on a grey surround; `pxPerMm` from the drawn width. Frames in array order as absolutely positioned boxes: the photo with `object-fit: cover`, scaled by `zoom` and positioned by the focus point; border as the box's padding in the border colour; rounded corners as `border-radius` on the box; shadow as `box-shadow` with the Task 1 values (offset and blur 1.5% of the shorter drawn side, 35% black). A frame whose photo is not in `photos` shows a grey box with "Photo missing". A "Low resolution" badge on a frame under `LOW_DPI` (`frameDpi`). Snap guides drawn as thin pink lines while dragging.
- [ ] **Step 2: Pointer behaviour,** all through the pure modules: click selects (`topFrameAt`), shift-click toggles, click on the surround clears; drag the body moves (`moveRect` then `snapMove`, Alt skips snapping); drag a handle resizes (`resizeRect`, Shift keeps shape on corners, then `snapResize`); double-click enters crop mode on that frame (drag pans with `focusFromDrag` and `overflow`, wheel changes `zoom` within 1..4, Escape or a click outside leaves it); Delete and Backspace remove the selection; `]` and `[` call `bringToFront` / `sendToBack`; dropping a photo dragged from the list (HTML5 drag, data type `application/x-pixie-photo` holding the id) calls `ondrop`, and dragging one frame onto another swaps them (`swapPhotos`). Pointer capture on the dragged element; a drag never leaves the page. Every control has a `title`; the handles are not controls but have `aria-hidden`.
- [ ] **Step 3: Check** `npx svelte-check` in `ui/` reports no problems in this file and `npx vitest run tooltips` passes.

### Task 7: The image list and the settings pop-up (Svelte)

**Files:**
- Create: `ui/src/lib/CollageList.svelte`, `ui/src/lib/CollageSettings.svelte`

**Interfaces:**
- Produces:
  - `CollageList` props `{ photos: Photo[]; onPage: ReadonlySet<number>; selected: ReadonlySet<number>; onselect: (ids: Set<number>) => void; onadd: (photoId: number) => void; onstart: () => void; startLabel: string }`: thumbnails (`thumbUrl('view', …)` or `'grid'`) in the Working-on order, a tick on those on the page, "N photos · M on the page" with `plural`; click, ctrl-click and shift-click select; double-click calls `onadd`; thumbnails are draggable with data type `application/x-pixie-photo`; the **Start with selected** button under the list calls `onstart` (it is labelled "Start with all" when nothing is selected).
  - `CollageSettings` props `{ doc: CollageDoc; templates: CollageTemplate[]; locale: string; onchange: (d: CollageDoc) => void; onstart: (how: 'auto' | 'template' | 'blank', templateId: string | null) => void; ondone: () => void }`: a dialog in the style of `ConfirmDialog.svelte` (scrim, Escape closes), sections *Start from* (three choices; with *A template* the 12 outline buttons from the first build), *Paper* (select over `paperGroups(locale)` plus "Custom size" with *Width (cm)* and *Height (cm)*), *Landscape*, *Resolution (dpi)* (150, 200, 300, default per size), *Gap (mm)*, *Corner rounding (mm)*, *Border (mm)* and *Border colour*, *Shadow*, *Background*, and the two snap switches. **Done** is `class="primary"`.

- [ ] **Step 1: Write both components** following the spec. The list scrolls on its own; the pop-up edits `doc` through `onchange` immediately (no Apply), and the *Start from* choice calls `onstart` then closes.
- [ ] **Step 2: Check** `npx svelte-check` and `npx vitest run tooltips` pass.

### Task 8: The tool, wiring and cleanup (Svelte)

**Files:**
- Rewrite: `ui/src/lib/CollageTool.svelte`
- Modify: `ui/src/lib/ToolsTab.svelte` (for the Collage tool the content area to the right of the tool list is the whole tool: the 3-column layout used for `whole` tools, with the "Working on" bar still on top; the right-hand photo grid and the options column are not shown; videos left out with the existing note), `ui/src/lib/tools.ts` and `tools.test.ts` if the `blurb`/`mock` for Collage is now unused
- Delete: nothing else; remove what Tasks 3-4 made unused (`collage.ts` leftovers) and any now-dead `api.ts` code.

**Interfaces:**
- Consumes: everything above. `CollageTool` props `{ photos: Photo[] }` (the Working-on photos, videos already removed).

- [ ] **Step 1: Write a small tested helper first** in `ui/src/lib/collageStart.ts` with `collageStart.test.ts`: `framesFromTemplate(cells: CollageCell[], doc: CollageDoc, photos: Photo[]): CollageFrame[]` (photos in order into the template's frames; spaces with no photo get no frame), `framesFromAuto(doc: CollageDoc, photos: Photo[]): { frames: CollageFrame[]; leftOut: number }` (uses `justifiedRows` with each photo's `width`/`height`, 3:2 when unknown), and `newFrameId(): string`. Tests: 4 photos into a 2×2 template give 4 frames with the photos in order; 3 photos into 4 spaces give 3 frames; auto with 30 photos gives 24 frames and `leftOut` 6; ids are unique.
- [ ] **Step 2: Run** `npx vitest run collageStart`. Expected: FAIL, then implement, then PASS.
- [ ] **Step 3: Write `CollageTool.svelte`:** load `getCollage()` and `collageTemplates()` on mount (`parseDoc`, `navigator.language` as the locale); resolve every frame's photo into a `Map<number, Photo>` from the Working-on photos plus `getPhoto(id)` for ids not among them; show the settings pop-up on first open when the page has no frames; the bar across the top of the middle area: gear (`title`, `aria-label` "Settings"), the summary `8 × 10 in · portrait · 2400 × 3000 px · 300 dpi · Snap on/off`, **Clear** (opens `ConfirmDialog`: "Clear the collage?" / "The photos stay in your library."; keeps page and look settings), **Save** (`class="primary"`; the save dialog with PNG and JPEG filters, default name `collage.png`; cancel is silent; shows "Saved." or the error text; disabled with no frames, while saving, or on `sizeProblem`); **Start with selected** asks to confirm first when the page already has frames; the ToolsTab-level row of the page below the bar is `CollagePage`, and `CollageList` sits on its right with a fixed width (240 px, not resizable). Autosave: every change to `doc` schedules `setCollage(serializeDoc(doc))` 400 ms after the last change, and a pending save is flushed when the component is destroyed. Dropping a photo from the list adds a frame at the drop point (`newFrameRect`); double-click in the list adds at `firstFreeSpot`; dropping on a frame replaces that frame's photo and resets its crop. The "N left out" note shows after an automatic or template start.
- [ ] **Step 4: Verify the whole suite:** `cargo check --workspace --all-targets` (no warnings), `cargo test -p pixie-image -p pixie-app`, then in `ui/`: `npx svelte-check` (0 errors, 0 warnings) and `npx vitest run` (all pass, including `tooltips`).
- [ ] **Step 5: Run the app** (use the `run` skill, separate cargo target): in Create > Collage with 4 to 8 photos, including a phone portrait: start with selected; move, resize and crop a frame; check snapping lines; overlap two frames and turn on border and shadow; Save as PNG and as JPEG and compare them with the preview (stacking, border, shadow, crop); close Pixie and reopen: the collage is as left; Clear. Report what was seen; do not claim success without it.

### Task 9: Docs

**Files:**
- Modify: `README.md` (the Create > Collage bullet: free-form page, snapping, overlap, border and shadow, kept between sessions), `docs/superpowers/plans/2026-11-01-collage.md` (a first line: "Superseded by 2026-11-03-collage-editor.md; the code of Tasks 1-3 was reused."), `docs/superpowers/specs/2026-10-29-tool-tabs-design.md` (Collage line mentions the editor)

- [ ] **Step 1: Edit the three files.**
- [ ] **Step 2: Final checks** as in Task 8 Step 4, with the output quoted in the report.

---

## Self-review

- **Spec coverage:** page and three areas (Tasks 6-8); settings pop-up with every setting (7); starts: auto, template, blank (4, 8); frames: add, move, resize, crop with zoom, order, delete, swap, overlap (4, 6); snapping with guides, Alt, switches (5, 6); border, shadow, corners, background (1, 6); sharpness badge (4, 6); region sizes (3); kept collage with autosave and missing photos (2, 3, 8); Clear (8); Save wording (8); non-goals untouched.
- **Types:** `Rect`/`Handle` (Task 4) are what Task 5 consumes; `CollageDoc`/`CollageFrame` (Task 3) are what Tasks 4, 6-8 use; `SaveArgs`/`FrameArg` (Tasks 2-3) match; `cover_crop` and `coverCrop` carry the same four test cases.
- **Open for you:** none beyond the calls marked in the spec.
