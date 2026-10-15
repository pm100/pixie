# Pixie: Design Spec

Date: 2026-10-15

## 1. Purpose

Pixie is a desktop photo manager for non-professional users on **Windows and macOS**, built with **Tauri (Rust backend) and a Svelte + TypeScript frontend**. It helps people find, view, print and lightly edit the photos already on their computer, and make collages.

Success looks like: a non-technical user points Pixie at their photo folders, can find any photo or clean up duplicates quickly, and can run a slideshow, print photos at standard sizes, make a basic edit, or build a collage without reading any documentation.

## 2. Scope

### In v1

| Area | v1 | Later |
|---|---|---|
| Library | Index watched folders in place; originals never modified; optional organize step (move/rename by date) | |
| Find | Search/filter by date, folder, filename, camera, GPS; tags, favourites, ratings; exact and near-duplicate finder | Content search (e.g. CLIP) and face grouping |
| Slideshow | Fullscreen playback of album, folder or search result; per-photo duration; fade/slide transitions; shuffle; pause/skip; optional local music | Ken Burns, captions/overlays, multi-monitor, video export |
| Print | Standard sizes (4x6, 5x7, 8x10, A4, Letter); N per page; crop-to-fill or fit-with-borders; margins; system print dialog | PDF export, contact sheets, ordering from a lab |
| Edit | Rotate, crop, straighten, auto-enhance, brightness/contrast/saturation/warmth, red-eye, resize; non-destructive | Filters/presets, text overlay, borders, retouching |
| Collage | Template layouts, spacing, corner rounding, background colour, per-cell crop adjust; save as image or print | Photomosaic (tile-matched) |

### Formats

JPEG, PNG, WebP, TIFF and **HEIC** in v1. RAW is later.

### Assumptions

- Libraries of tens of thousands of photos; scanning must be incremental and in the background.
- Fully offline; no accounts, no cloud.

### Non-goals for v1

Cloud sync, RAW decoding, face/content search, video files, photo lab ordering, video export of slideshows.

## 3. Architecture

A Tauri project with a Rust workspace of focused crates.

- **`pixie-core`**: shared types (photo, album, edit recipe, collage layout, errors). No I/O.
- **`pixie-index`**: SQLite, folder scanning, change detection (path, size, mtime), EXIF extraction, tags/ratings, search queries, job queue.
- **`pixie-image`**: the single image pipeline. Decode, apply edit recipe, resize, encode. Used by edit preview, export, print and collage, so on-screen output matches printed output.
- **`pixie-layout`**: collage templates and print page layout (sizes, margins, crop/fit). Produces a render description that `pixie-image` draws.
- **`src-tauri`**: thin command layer exposing the crates to the UI, plus a custom `pixie://` protocol serving thumbnails and previews as image URLs.
- **`ui/`**: Svelte + TypeScript. Views: Library, Search, Viewer/Edit, Slideshow, Collage, Print. Virtualized photo grid.

Chosen approach: **Rust core with a thin UI.** All image processing happens in Rust; the webview only displays results. Rejected: browser-side (Canvas/WebGL) editing, because print would need a second renderer and colour handling differs between WebView2 and macOS WebKit; and a hybrid, because two implementations of each edit drift apart.

**Non-destructive edits.** Each photo's edit is a small recipe stored in SQLite. "Save a copy" renders a new file next to the original; originals are never changed.

**HEIC dependency.** HEIC decoding links `libheif` (a C library), which complicates Windows and macOS builds. The implementation plan must settle how it is built and bundled (vendored static build vs. prebuilt) in Milestone 1.

## 4. Library, search and duplicates

**Scanning.** Users add watched folders. Per image the scan records path, size, mtime, dimensions and EXIF (date taken, camera, GPS). Date taken falls back to file mtime. Re-scans only touch files whose size or mtime changed. Vanished files are marked **missing**, not deleted, so tags and ratings survive disconnected drives. A file watcher picks up new photos while the app is open.

**Tables.** `photos`, `folders`, `tags`, `photo_tags`, `edits`, `albums`, `hashes`. Indexes on date, folder, camera; FTS5 for filename and tag text. The schema is migration-versioned so content-search embeddings can be added later as a new table.

**Thumbnails.** Generated in background at ~256px (grid) and ~1600px (viewer), cached on disk under the app data folder keyed by photo id and mtime. Visible rows are prioritised. Multiple workers, because HEIC and large JPEGs are slow.

**Duplicates.** Exact: blake3 content hash. Near: perceptual hash (dHash) on the thumbnail, grouped by Hamming distance. The UI shows groups side by side with a suggested keeper (highest resolution, then oldest). Nothing is deleted automatically; the user ticks what to remove, and removal goes to Recycle Bin / Trash.

**Organize (optional).** Preview-first tool to move or rename files by date (e.g. `Pictures/2024/06/`). Shows exactly what will happen; records moves so they can be undone.

## 5. Edit, collage, print, slideshow

**Edit pipeline.** A recipe is an ordered struct: crop rect, rotation (90° steps plus straighten angle), colour adjustments (brightness, contrast, saturation, warmth), red-eye regions, resize target. EXIF orientation is always applied first. In the editor, Rust renders the recipe on the ~1600px preview and returns it via `pixie://`; export renders the same recipe at full resolution. Auto-enhance produces recipe values (histogram stretch plus mild saturation lift) which the user can see and tweak. Red-eye: the user marks an eye; Rust desaturates and darkens red inside that circle.

**Collage.** A template is data: a list of cells as fractions of the page, so it works at any size. The user picks photos and a template, sets spacing, corner rounding and background colour, and can drag a photo within its cell to adjust the crop. Output is a render description drawn by `pixie-image`; saved as an image or sent to print.

**Print.** The user chooses paper, photos per page, crop-to-fill or fit-with-borders, and margins. `pixie-layout` computes page layouts; pages are rendered to 300 DPI images placed in a print-styled view; the webview's print dialog opens the system dialog. **Known risk:** print scaling and margins differ between WebView2 and WebKit. Milestone 1 includes a real print test page verified on a physical printer and PDF on both platforms.

**Slideshow.** Frontend only. Preloads the next two previews, crossfades or slides, supports shuffle, per-photo duration, pause/skip by keyboard or mouse, optional local music via an audio element, and keeps the screen awake during playback.

## 6. Errors and testing

**Errors.**
- Corrupt/unreadable photos get a placeholder and a logged reason; they never stop a scan.
- Missing drives and moved files show as "missing" with a one-click re-locate.
- Every command returns a typed error with a plain-language message; raw Rust errors are never shown.
- Write operations (organize, save-a-copy, remove duplicates) never overwrite an existing file; collisions get a suffix.
- Slow jobs show progress and are cancellable.

**Testing.**
- Unit tests for pure logic: recipe maths, collage/print layout geometry, search queries, duplicate grouping.
- Fixture photo set: EXIF orientations, HEIC, a corrupt file, a very large panorama.
- Golden-image tests: render a recipe and compare to a stored reference within tolerance.
- Benchmark: scan and thumbnail a generated 50,000-photo library.
- Manual release checklist: real printer output and WebView2 vs WebKit differences.

## 7. Delivery milestones

Each milestone is independently usable.

1. **Skeleton and library**: project setup, scan, SQLite, thumbnail grid, viewer, CI on Windows and macOS, HEIC build solved, print test page.
2. **Find**: search/filters, tags, ratings, favourites, duplicates.
3. **Edit**: recipe pipeline, editor UI, save-a-copy.
4. **Slideshow.**
5. **Print.**
6. **Collage** (reuses the print layout engine).
7. **Organize tool, installers, signing, polish.**

**Packaging.** Tauri installers (MSI/NSIS for Windows, DMG for macOS). Code signing and macOS notarization require paid developer accounts, to be arranged before any public release.

## 8. Open items for the plan

- How `libheif` is built and bundled on each platform.
- Specific crates (e.g. `image`, `fast_image_resize`, `rusqlite`, `kamadak-exif`, `notify`, `blake3`) to be chosen and verified in Milestone 1.
- Collage template set for v1 (suggest ~10 layouts).
