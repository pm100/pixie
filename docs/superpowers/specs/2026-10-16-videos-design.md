# Pixie: videos (design)

Date: 2026-10-16. Builds on the shell and date tree (`2026-10-16-shell-and-date-tree-design.md`) and location (`2026-10-16-location-in-details-design.md`). GitHub issue #1.

## Context

Photos and videos live together in one library. Decision (user): v1 handles **phone and camera videos** (MP4 / MOV / M4V, H.264 and HEVC), played by the webview's own player, with **no ffmpeg**. AVI, MKV, WMV and friends are not indexed in v1.

## Goals

- Videos appear in the same grid, date tree and folder tree as photos, with a play badge and duration.
- Double-click (or Enter) plays a video in the viewer; arrows still move to the next item.
- Details pane shows duration and resolution, and the map and place name when the file has GPS.
- An All | Photos | Videos filter above the grid.
- No platform-specific native code (it cannot be tested on macOS here).

## Non-goals

Editing, collage use, slideshow playback of videos, trimming, transcoding, AVI/MKV/WMV, Live Photo pairing, hover previews, ffmpeg.

## Library

- `pixie-core`: `VIDEO_EXTENSIONS = ["mp4", "mov", "m4v"]`; `MediaKind { Photo, Video }` (serde lowercase); `media_kind(path) -> Option<MediaKind>` replacing `is_supported_image` at the scanner call site (`is_supported_image` stays, for image-only callers). `._name` resource-fork files are still ignored. `Photo` gains `kind: MediaKind` and `duration_ms: Option<i64>`. (The struct keeps the name `Photo`; renaming is out of scope.)
- V3 migration: `ALTER TABLE photos ADD COLUMN kind INTEGER NOT NULL DEFAULT 0` (0 photo, 1 video), `ADD COLUMN duration_ms INTEGER`, plus `idx_photos_kind_newest` / `idx_photos_kind_oldest` on `(kind, missing, (taken_at IS NULL), taken_at, id)` so the filtered listing pages in index order. Folder+kind relies on the existing folder-leading indexes with `kind` as a row predicate.
- `pixie-image` (or a new `meta_video` module beside `read_meta`): `read_video_meta(path) -> PhotoMeta + duration_ms`. `nom-exif` 3 (pure Rust), checked against four real phone/camera videos from the user's library (720p to 4K, GPS present on three): it returns duration, width, height, creation time and GPS. It does not expose rotation, so a small own parser reads the `tkhd` box. Fields:
  - duration from `mvhd`; width/height from the first video track `tkhd`, **swapped when the rotation matrix is 90 or 270 degrees** (phone portrait videos);
  - taken date: the container creation time, which is UTC (a Hawaii clip recorded at 09:41 local reads 19:41). Photo dates are stored as local-as-UTC, so when GPS is present the UTC value is shifted by `round(longitude / 15)` hours to approximate local time and land on the same day as photos; without GPS it stays UTC. An unset (zero) creation time falls back to file mtime. `nom-exif` may take the date from Apple's `creationdate` key when present; its instant is used the same way (UTC, then the longitude estimate), so the key's exact local offset is not used. Some cameras (GoPro, many compacts) write local time into `mvhd`; for those the longitude shift double-shifts the date, a known limitation;
  - GPS from `©xyz` / `com.apple.quicktime.location.ISO6709`, passed through the existing `sanitize_gps`;
  - camera: Apple `make`/`model` keys when present.
- A corrupt, truncated, zero-length or non-media `.mp4` is still indexed (kind video, no metadata, mtime date) and the scan continues, like corrupt JPEGs today.
- `PhotoQuery` gains `kind: Option<MediaKind>`. `list_photos`, `count_photos`, `list_day_groups` take an optional `kind`. Counts, day headings and the tree follow the filter.

## Thumbnails (poster frames)

The webview makes them, with the same decoder that plays the file, so "plays" and "has a thumbnail" agree.

- `pixie:/poster/{id}` serves a stored JPEG poster, or 404 when none exists yet. Posters live in the thumbs cache dir under a name keyed by id, mtime and size (same invalidation as photo thumbnails).
- A video grid tile requests its poster. On 404 it queues generation in a single serial queue (one video decoded at a time, visible tiles first): a detached `<video muted preload="metadata">` is created, seeks to `min(1 s, duration / 2)`, draws one frame to a canvas scaled to the grid size, encodes JPEG, calls `save_poster(id, bytes)`, then reloads the `<img>`. The `<video>` is always torn down (`removeAttribute('src')`, `load()`) afterwards.
- If the player cannot decode the file (error event, or no frame within 8 s), the tile shows a placeholder with the play badge for the rest of the session, and no retry loop.
- The existing grid/viewer thumbnail pipeline and `prefetch_all` skip videos (they decode images only).
- `save_poster` validates the id exists, the data is a JPEG under 2 MB and the dimensions are at most 512 px.

## Playback

- `pixie:/video/{id}` streams the file with HTTP range support: `Range: bytes=a-b`, `a-`, and `-n` forms; 206 with `Content-Range`, `Accept-Ranges: bytes`; an open-ended range answers at most 4 MiB at a time; unsatisfiable range 416; missing file or id 404. All three extensions are served as `video/mp4` (Chromium plays H.264 `.mov` that way; the content type is the only thing it keys on). Work happens off the protocol handler thread. Looked up by id, so the webview never sees paths.
- Viewer: a `<video controls>` with the browser's controls for a video item, no autoplay. Left/right arrows move to the neighbour (focus is returned to the viewer, not the player); Space toggles play/pause; Escape closes. Neighbour preloading is skipped for videos. Leaving a video tears the element down so playback stops.
- Undecodable (typically iPhone HEVC on Windows without Microsoft's "HEVC Video Extensions"): the viewer shows a plain message and an **Open in default player** button. That button calls `open_in_default_player(id)`, which opens the file with the operating system (new dependency: `tauri-plugin-opener`, Rust side only; the path comes from the database by id).

## Grid, tree and details

- Tile: poster (or placeholder), play badge, duration label ("0:42", "1:03:07").
- Filter: a segmented control **All | Photos | Videos** above the grid, persisted in prefs. Changing it clears the selection and refreshes like a sort change.
- Details pane: for a video adds **Duration**; size shows rotated dimensions; map and place name work unchanged; the **Email...** button stays disabled.

## Error handling

- Missing or unreadable file: the viewer shows the existing "can't be opened" state; range requests give 404/500 with a console line, never a crash.
- Unsupported codec: see Playback. No spinner that never ends: the 8 s timeout applies to the viewer too.
- Disk full or write error saving a poster: the tile keeps its placeholder; no popup.

## Testing

- Rust: kind detection (extensions, case, resource forks); V3 migration (existing rows become photos; reopen does not re-run); `kind` filter in listing, counts, day groups and the EXPLAIN check for the kind indexes; metadata from small fixture clips (iPhone-style with Apple keys and rotation, plain MP4, truncated file, empty file, no metadata); range parsing and response building (all three forms, past-end, zero-length, 416); poster validation; scanner indexes videos and survives a corrupt one.
- UI: duration label formatting; kind filter state and its persistence; range-free pure helpers; poster queue ordering and teardown logic as a pure/injectable unit.
- Manual on the real library: a phone video plays and seeks; its poster appears; portrait video has the right shape; HEVC without the extension shows the message and the button works; Photos/Videos filter; date tree counts.

## Review Focus

- Large files (multi-GB 4K): range serving must never read a whole file into memory.
- Corrupt, truncated and zero-byte videos must not stop a scan or crash the grid.
- Portrait (rotated) videos get the right dimensions and poster orientation.
- Poster generation must not pile up hidden `<video>` elements or decoders when scrolling fast.
- Switching away from a playing video (arrow, Escape, filter change) must stop it.
- `save_poster` and `/video/{id}` must not be usable to read or write arbitrary paths.

## Decisions made while writing

- Posters are made in the webview, not natively (single code path, same decoder as playback). Cost: only generated while the app is open and only for visible tiles.
- Every video is served as `video/mp4`.
- `nom-exif` is confirmed on real files; rotation comes from an own `tkhd` parser; dimensions come from `tkhd` (nom-exif's width/height are only a fallback).
- Video dates use a longitude-based local-time estimate (see Library). It is approximate near time-zone borders and in places with unusual offsets.
- Test fixtures are synthetic MP4s built in code; no personal videos are committed to the public repo.
