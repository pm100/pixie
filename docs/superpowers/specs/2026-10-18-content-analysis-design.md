# Content analysis: semantic search and people (issue #34)

Local-only analysis of photo content. Nothing leaves the machine. Two features on one foundation:
semantic search ("dog on a beach") and people (faces clustered, named by the user).

## Decisions

- **Semantic search model: OpenAI CLIP ViT-B/32** (MIT). Image and text towers as ONNX, int8-quantised
  (about 90 MB image tower, 65 MB text tower plus tokenizer). Chosen over SigLIP: smaller, well-supported
  ONNX exports, fast on CPU, and good enough for personal-library search.
- **Face detector: YuNet** (OpenCV Zoo, MIT, under 1 MB). **Face recognizer: SFace** (OpenCV Zoo,
  Apache 2.0, about 37 MB, 128-d embeddings). Chosen over InsightFace/ArcFace weights because those are
  non-commercial research licences and Pixie is MIT.
- **No opt-in gate.** Analysis is part of the app: models download on first run and the work runs in the
  background. Settings -> Analysis still offers pause/resume and shows progress, so the user can stop the CPU use.
  (Assumption from "forget the opt-in question"; easy to add a switch later.)
- **Runtime:** ONNX Runtime via the `ort` crate, CPU only. DirectML / Core ML later.
- **No online analysis.** Out of scope.

## Architecture

New crate `pixie-analysis` (queue, model manager, CLIP, faces, vector search). `pixie-index` keeps the schema
and photo queries; `src-tauri` adds commands and events only.

### Job queue
- No jobs table: the queue is a query. A photo needs analysis when it has no `embeddings` row for the current
  `model_version` (a NULL vector row marks a photo that could not be analysed, so it is not retried). This survives
  restarts by construction, and a new `model_version` re-queues everything without extra bookkeeping.
- A finished scan wakes the worker (`kick`) so new photos are picked up.
- A worker pool of 1-2 threads (low priority) pulls jobs. It does not take `ScanGate`; it pauses itself while a
  scan runs and while the user has been interacting in the last few seconds, so Browse stays smooth (#32).
- Input is the existing thumbnail (large size) where one exists, else a decode of the original, so analysis
  does not re-read full-size files from network drives.
- One event, `analysis-status`, carries model download state and analysis progress.
- Pause, resume and cancel are commands; state survives restart (jobs are rows).

### Model manager
- Models live in the app data dir under `models/<name>-<version>/`.
- Download on first need, SHA-256 verified against a table compiled into the app, resumable, progress event.
- A model not yet downloaded leaves its jobs queued; failure shows in Settings -> Analysis with Retry.

### Storage
- `embeddings(photo_id, model_version, vector BLOB)` - 512 float32 (f16 later if size matters).
- `faces(id, photo_id, x, y, w, h, score, embedding BLOB, person_id NULL, model_version)`.
- `people(id, name NULL)`. Unnamed clusters have `name NULL` and show as "Unnamed person".
- A new `model_version` re-queues only the affected rows. Deleting a folder cascades.

### Semantic search
- Query text -> CLIP text tower -> vector; cosine against all embeddings, brute force in memory
  (50k x 512 f32 is about 100 MB; kept as a loaded matrix, dot-product scan well under 100 ms).
- The hits go into a TEMP table `search_hits`; `PhotoQuery.search` restricts listings to it, so the grid, viewer,
  map and When nav all work on a search result unchanged. Results show in date order (not relevance order), capped
  at the best 500 above the similarity floor.
- Results below a similarity floor are dropped so nonsense queries return "nothing found".

### People
- Face: YuNet detect -> align on landmarks -> SFace embed.
- Clustering: incremental greedy. A new face joins the nearest person whose centroid is within a cosine
  threshold, else starts a new person. A background "tidy" merges clusters that drifted together.
- User actions: name or rename, merge two people, "not this person" (face detaches and is pinned so it is not
  re-assigned), hide a person.
- Filter: a person filter joins `search_hits` like the text search and tag filter (photos must be in every active set), so no new `PhotoQuery` field.
- Built differently from the first sketch: faces get their own background pass and `PeopleService` (separate model download and status event) beside the CLIP one; a `face_scans` table (one row per photo and model version, also for photos with no faces) is the queue marker; face crops are cut on demand from the 1600 px viewer thumbnail (`pixie://face/{id}`), not stored; unnamed people seen in only one photo are not listed in the nav; there is no automatic "tidy" merge yet, manual Merge is the remedy.

## UI

- **Browse:** search box in the tools bar; clearing it restores the view. A result chip like the existing
  period chip ("Matching 'dog on beach' x").
- **Library nav:** "People" section listing named people (face crop, name, count); unnamed clusters under
  "Unnamed" sorted by size.
- **Settings -> Analysis:** per-feature status (model download, photos done of total), Pause/Resume, Retry.

## Build order (each stage ships and is tested)

1. Foundation: crate, job table, worker, model manager, Settings -> Analysis page, wired end to end with CLIP embeddings.
2. Semantic search: text tower, vector scan, Browse search box and chip.
3. Faces: detect and embed, clustering, People nav, naming, merge, filter.

## Testing

- Pure units: cosine ranking, clustering thresholds (synthetic vectors), job enqueue idempotence, download
  checksum and resume, pause/resume state, `model_version` re-queue.
- ONNX inference tests use tiny fixture models or are `#[ignore]` behind an env var pointing at real models, so CI
  needs no 350 MB download.
- UI helpers pure and vitest-tested like the existing ones; GUI behaviour verified by the user.

## Out of scope

Near-duplicate grouping, quality scores, OCR, online captions, GPU acceleration, per-person "pop-out" views.

## Open risks

- CPU time for a first pass over a large library (est. 50k photos: 1-2 h for CLIP, similar for faces); the
  pause control and low priority are the mitigation, and #32 must land to keep Browse smooth.
- Face clustering quality on children and across years; the merge / not-this-person tools are the fix.
- ONNX Runtime packaging on Mac and Windows (shared library bundling in Tauri).
