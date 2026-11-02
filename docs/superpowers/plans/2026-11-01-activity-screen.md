# One Progress Screen ("Processing") Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the many scattered progress displays with one screen that shows every background operation (reading folders, hashing, thumbnails, Auto tags, faces, duplicates, copy to the master, model downloads, waiting for cloud drives) with counts, totals, throughput and an estimated time left. It opens from the pill at the top right, which always says "Processing", and it is the same screen the Add folder popup shows.

**Architecture:** A new `Activity` registry in `pixie-core` holds one entry per operation: done, total, bytes, state, and a rate and time-left worked out from samples over a sliding window. The existing workers feed it absolute counts (most already have them: `AnalysisStatus`, `PeopleStatus`, `DupStatus`, `ScanProgress`, `LoadProgress`, copier `Status`), plus new counters where nothing is counted today (cloud-drive downloads, bytes hashed). `src-tauri` emits one `activity` event about once a second. The UI has one pure view-model (`activity.ts`) and one component (`ActivityScreen.svelte`) used by both the pill's page and the Add folder popup; every other progress bar, percentage and counter is deleted.

**Tech Stack:** Rust (`pixie-core`, `pixie-index`, `pixie-analysis`, `src-tauri`), Tauri events, Svelte 5 + TypeScript + vitest.

**Spec:** No separate spec; this plan comes from the request in the conversation of 2026-11-01. Calls marked *(my call)* are mine. Related code: `ui/src/lib/backgroundProgress.ts`, `BackgroundProgress.svelte`, `LoadModal.svelte`, `analysisView.ts`, `AnalysisSettings.svelte`, `crates/pixie-index/src/copier.rs` (the only place that looks at online-only cloud files today: Copy to master skips them and counts them as "waiting").

## Global Constraints

- Never commit or push unless the user says so (AGENTS.md); "Commit" steps below mean "only if asked".
- No compiler warnings: `cargo check --workspace --all-targets` clean, no `#[allow]`; delete dead code or gate it with `#[cfg(...)]`.
- Pluralise only when a count is more than one: use `plural` in `ui/src/lib/format.ts`.
- Buttons white by default; `class="primary"` only for the screen's one main action; `class="danger"` for destructive ones. No per-component button styling.
- No control in the middle of a sentence; label then control. Units go in the label.
- Page padding from `--page-pad` / `--page-pad-x` / `--page-pad-y` only.
- Every control has a whole-sentence `title`; icon-only or unlabelled controls also need `aria-label` (`ui/src/lib/tooltips.test.ts`).
- No new dependencies. If one is added, run `just notices` and commit its outputs.
- Claude builds use a separate cargo target dir.
- Windows and macOS both supported: anything platform-specific is `#[cfg(...)]`-gated with a no-op elsewhere.

## Scope *(my call)*

- **One pill, one label.** The pill always reads "Processing" (spinner while working, pause icon when paused, warning icon on error; no percentage and no bar in the pill). It is absent when nothing is running, as now.
- **One screen** shows: an overall line ("About 12 min left · 4,210 of 18,300 items"), then a row per operation (name, bar, "done of total", throughput, time left, state), then finished operations with their totals and how long they took. Pause/Resume and Retry live here, replacing the ones in Settings.
- **Time left is an estimate and says so**: shown as "about …", withheld ("Estimating…") until at least 10 seconds of samples exist or when the total is not known yet, and the overall time left is the longest of the running operations (they run side by side).
- **Removed elsewhere:** the Analysing screen's bar and counters in the Add folder flow (keeping its photo strip), the Settings > Picture analysis progress bar and counts, the per-folder "% analysed" in Photo folders, the scan line in Settings, the People tab and Duplicates tab progress bars, the Sync tool's live copy line (its last-run summary stays), and the old pill. Settings keeps the analysis on/off choices.
- **Nothing skips cloud-only files** *(decided by the user)*. The scan, hashing, thumbnails, analysis and Copy to master all read them and wait for the cloud app to download them. Pixie only measures and reports the delay. This means removing the skip in `copier.rs`.
- **Out of scope:** changing what is analysed or in what order; cancelling individual operations (Stop for the folder scan and the copy stay where they are but also appear on the screen); history of past sessions.

## Review Focus

- A time-left that jumps wildly at the start, when speed changes (a fast burst then a slow video), or when total grows (the folder walk is still finding files): withhold or smooth it; never show "0 seconds" or "3 days" from two samples.
- The computer sleeps or the app is paused for hours: the rate window must not collapse to nothing, and the time left must recover within a window afterwards.
- Copy to master no longer skips online-only files: a run with online-only files copies them after the download, and a run where the download never arrives does not hang the whole screen (the wait shows, Stop still works).
- Cloud files with no internet or a stuck cloud app: the operation shows "Waiting for your cloud drive" with how long it has waited, never a fake time left.
- A paused or errored operation: no rate, no time left, the reason is shown; Retry works from the screen.
- 100,000-photo libraries: the snapshot stays small (one row per operation, not per file) and the event is sent at most once a second.
- One huge file (a 5.8 GB video to a USB drive took many minutes and the screen sat at "0 of 5"): the copy row moves while the file is being written and while it is read back to check it, shows bytes and MB/s, and gives a time left that includes the check (the check is as long as the write again). It must never look frozen, and Stop must work within a second or two, not at the end of the file.
- The Add folder popup and the pill's screen show the same numbers at the same time (one source), and the popup still finishes into "All set" exactly as before.

## File Structure

- Create `crates/pixie-core/src/activity.rs`: `Activity` registry, `TaskKind`, `TaskState`, `TaskSnapshot`, `Snapshot`, `RateWindow`, injectable clock. No I/O.
- Modify `crates/pixie-core/src/lib.rs`: `pub mod activity;`.
- Create `crates/pixie-index/src/cloud.rs`: `is_online_only(path)` (moved out of `copier.rs`) and the cloud-wait counters.
- Modify `crates/pixie-index/src/scanner.rs`, `src-tauri/src/lib.rs`, `src-tauri/src/state.rs`, `crates/pixie-analysis/src/pipeline.rs` (feed the registry; add byte and cloud counters).
- Create `ui/src/lib/activity.ts` (+ `activity.test.ts`): view-model and text.
- Create `ui/src/lib/ActivityScreen.svelte`, `ui/src/lib/ProcessingPill.svelte`.
- Modify `ui/src/lib/format.ts` (+ test): `formatDuration`, `formatRate`, `formatBytes` if absent.
- Modify `ui/src/App.svelte`, `LoadModal.svelte`, `Settings.svelte`, `AnalysisSettings.svelte`, `PhotoFolders.svelte`, `PeopleTab.svelte`, `DuplicatesTab.svelte`, `SyncTool.svelte`, `BackgroundNote.svelte`, `analysisView.ts`.
- Delete `ui/src/lib/BackgroundProgress.svelte` and `backgroundProgress.ts` (+ tests) once nothing uses them.

---

### Task 1: The activity registry and rate window (pixie-core)

**Files:**
- Create: `crates/pixie-core/src/activity.rs`
- Modify: `crates/pixie-core/src/lib.rs`

**Interfaces:**
- Produces:
  - `pub enum TaskKind { Scan, Read, Thumbnails, AutoTags, Faces, FaceGroups, Duplicates, CopyToMaster, Models, CloudWait, Email }` (`Serialize`, `Clone`, `Copy`, `PartialEq`, `Debug`; serialised as snake_case)
  - `pub enum RunState { Running, Waiting, Paused, Error, Done }` (`Waiting` = blocked on something outside Pixie, e.g. the cloud drive)
  - `pub struct TaskUpdate { pub kind: TaskKind, pub state: RunState, pub done: u64, pub total: Option<u64>, pub bytes_done: u64, pub bytes_total: Option<u64>, pub note: Option<String> }`
  - `pub struct TaskSnapshot { pub kind: TaskKind, pub state: RunState, pub done: u64, pub total: Option<u64>, pub bytes_done: u64, pub bytes_total: Option<u64>, pub per_sec: Option<f64>, pub bytes_per_sec: Option<f64>, pub eta_secs: Option<u64>, pub started_ms: i64, pub finished_ms: Option<i64>, pub waiting_secs: Option<u64>, pub note: Option<String> }`
  - `pub struct Snapshot { pub tasks: Vec<TaskSnapshot>, pub eta_secs: Option<u64>, pub now_ms: i64 }`
  - `pub struct Activity` with `pub fn new(clock: Arc<dyn Clock>) -> Self`, `pub fn report(&self, u: TaskUpdate)`, `pub fn finish(&self, kind: TaskKind)`, `pub fn snapshot(&self) -> Snapshot`
  - `pub trait Clock: Send + Sync { fn now_ms(&self) -> i64; }` and `pub struct SystemClock;`
  - Constants `WINDOW_SECS: u64 = 30`, `MIN_SAMPLE_SECS: u64 = 10`, `GAP_RESET_SECS: u64 = 60`.

- [ ] **Step 1: Write failing tests** (a `TestClock` with `set(ms)`): `rate_is_done_per_second_over_the_window` (100 items per 10 s for 20 s gives 10/s and eta = remaining/10); `no_rate_or_eta_before_ten_seconds_of_samples`; `no_eta_when_the_total_is_unknown`; `a_total_that_grows_moves_the_eta_out_but_never_negative`; `a_gap_over_sixty_seconds_drops_old_samples` (sleep case); `paused_and_error_tasks_have_no_rate_or_eta`; `waiting_counts_up_waiting_secs_and_gives_no_eta`; `finish_keeps_the_task_with_its_totals_and_duration`; `overall_eta_is_the_longest_running_eta_and_none_if_any_running_task_has_none`; `a_second_report_for_a_finished_kind_starts_a_new_run`.
- [ ] **Step 2: Run** `cargo test -p pixie-core activity`. Expected: FAIL (module missing).
- [ ] **Step 3: Implement.** One `RateWindow` per task: a deque of `(ms, done, bytes_done)` samples trimmed to `WINDOW_SECS` and cleared on a gap over `GAP_RESET_SECS`; rate = (newest - oldest) / elapsed once elapsed ≥ `MIN_SAMPLE_SECS`; eta = remaining / rate rounded up, `None` when rate is 0 or total unknown. Use bytes for the rate and eta when `bytes_total` is known and larger signal than items *(my call: hashing and copying are better judged in bytes)*.
- [ ] **Step 4: Run tests.** Expected: PASS. `cargo check --workspace --all-targets` clean.
- [ ] **Step 5: Commit** (only if asked): `feat: activity registry with throughput and time-left estimates`.

### Task 2: Feed the registry from what is already counted

**Files:**
- Modify: `src-tauri/src/state.rs` (own an `Arc<Activity>`), `src-tauri/src/lib.rs` (map each source to a `TaskUpdate`; emit; command)
- Create (inside `src-tauri/src/`): `activity_feed.rs` with pure mapping functions

**Interfaces:**
- Consumes: Task 1 types; `pixie_analysis::service::AnalysisStatus`, `people_service::PeopleStatus`, `pixie_core::{DupStatus, LoadProgress}`, `pixie_index::scanner::ScanProgress`, `pixie_index::copier::Status`, the `mail-progress` payload.
- Produces:
  - `pub fn from_analysis(s: &AnalysisStatus, kind: TaskKind) -> Vec<TaskUpdate>` (a model download becomes a `Models` update with bytes; otherwise the pass; `paused`, `error` map to `Paused`, `Error`)
  - `pub fn from_people(s: &PeopleStatus) -> Vec<TaskUpdate>`, `pub fn from_dupes(s: &DupStatus) -> TaskUpdate`, `pub fn from_scan(p: &ScanProgress) -> Vec<TaskUpdate>` (`Scan` with `total: None` until `to_index` is fixed; `Read` for the metadata pass), `pub fn from_load(l: &LoadProgress) -> Vec<TaskUpdate>` (hashed -> `Read`; thumbnails from the pending count), `pub fn from_copy(s: &copier::Status, p: &copier::Progress) -> TaskUpdate` (items done/total, plus `bytes_done`/`bytes_total` from the copier's byte progress, and `note` "Checking the copy" while a file is being read back)
  - Tauri command `activity_snapshot() -> Snapshot`; event `activity` (payload `Snapshot`) sent by a ticker at most once a second, and only while something is running or just changed.

- [ ] **Step 1: Write failing tests** in `activity_feed.rs`: one per mapper with the examples that matter (paused analysis maps to `Paused`; `models == "error"` maps to `Error` with `note`; a scan with `to_index == 0` has `total == None`; a model download maps to `Models` with bytes; copy `running == false` and `pending == 0` maps to `Done`; a copy with bytes `bytes_done` 4 GB of `bytes_total` 6 GB while on item 0 of 5 maps to `done 0`, `bytes_done` 4 GB, and the verify phase maps to the "Checking the copy" note).
- [ ] **Step 2: Run** `cargo test -p pixie-app activity_feed` (use the package name from `src-tauri/Cargo.toml`). Expected: FAIL.
- [ ] **Step 3: Implement** the mappers and call them wherever the app already emits `scan-progress`, `analysis-status`, `people-status`, `duplicate-status`, `load-status`, `master-copy-progress` and `mail-progress` (one added line each). Add the ticker beside the other background tasks in `setup`. Register `activity_snapshot` in the invoke handler.
- [ ] **Step 4: Run tests and `cargo check --workspace --all-targets`.** Expected: PASS, clean.
- [ ] **Step 5: Commit** (only if asked): `feat: all background work reports to one activity registry`.

### Task 3: Count what is not counted: bytes read and cloud-drive waits

**Files:**
- Create: `crates/pixie-index/src/cloud.rs` (move the private `online_only` out of `copier.rs:229`, rename `is_online_only`, keep the Windows attribute test and add macOS `SF_DATALESS` `#[cfg(target_os = "macos")]`, no-op elsewhere)
- Modify: `crates/pixie-index/src/copier.rs` (remove the skip at line 283, the `waiting` count in `Report` (line 80) and its use in `master.ts` / `SyncTool.svelte`; reads go through the cloud meter instead), `crates/pixie-index/src/scanner.rs`, `crates/pixie-analysis/src/pipeline.rs`, `crates/pixie-index/src/lib.rs`

**Interfaces:**
- Produces:
  - `pub fn is_online_only(path: &Path) -> bool` in `pixie_index::cloud`
  - `pub struct CloudMeter` (`Arc`-shared, atomics) with `pub fn found(&self, bytes: u64)` (an online-only file seen), `pub fn arrived(&self, bytes: u64, waited_ms: u64)` (it was read, so it is local now), `pub fn snapshot(&self) -> CloudCounts { files_total, files_done, bytes_total, bytes_done, longest_wait_ms }`
  - Byte counters on the read/hash path: `pub fn add_bytes_read(&self, n: u64)` on the same meter family, so `Read` can report `bytes_done`

- [ ] **Step 1: Write failing tests**: `copy_to_master_copies_a_file_that_is_online_only_instead_of_skipping_it` (inject the check as a function parameter so the test can say "this file is online-only"; the file is copied, `Report` has no waiting count, and the cloud meter saw it arrive); `online_only_files_are_counted_when_found_and_when_they_arrive` (CloudMeter arithmetic; `longest_wait_ms` is the max); `a_normal_file_is_not_online_only` (tempfile); the existing copier tests still pass with the function moved.
- [ ] **Step 2: Run** `cargo test -p pixie-index cloud`. Expected: FAIL.
- [ ] **Step 3: Implement.** Remove the skip in the copier. In the scanner, pipeline and copier, check `is_online_only` before the first read of each file; if true, call `found`, time the read, then call `arrived`. Report a `CloudWait` update from the meter (state `Waiting` while any file has been waiting for more than 5 s, `note` = the longest wait). Totals only cover files met in the current session *(my call: no database column; after a restart the totals start again from the next scan)*. Add the byte counts to the `Read` update.
- [ ] **Step 4: Run tests and `cargo check --workspace --all-targets`.** Expected: PASS, clean. On the user's machine, check with a OneDrive "online-only" folder that the `CloudWait` row appears and its rate is believable (Task 8).
- [ ] **Step 5: Commit** (only if asked): `feat: count cloud-drive downloads and bytes read in the activity registry`.

### Task 3b: Byte progress inside one file (the Sync copier)

The copier (now the Sync feature: "sync folder", `Status { pending, copied, running, last }`, `Report { bytes, seconds, .. }`) reports only between files, so one big file shows nothing for minutes. Line numbers in Task 3 are from before the Sync changes; find the code by name.

**Files:**
- Modify: `crates/pixie-index/src/copier.rs`, `crates/pixie-index/src/db.rs` (`PendingItem` gains `size`), `ui/src/lib/master.ts` (+ test) and `SyncTool.svelte` if they still show a copy line

**Interfaces:**
- Produces: `Progress { done: usize, total: usize, bytes_done: u64, bytes_total: u64, phase: Phase }` and `enum Phase { Copying, Checking }`; `run` calls `progress` at most about every 250 ms while a file is being written and while it is read back, as well as after each file. `stream_copy` and the verify hash take a callback fed with bytes. `bytes_total` is the sum of the pending sizes; `bytes_done` counts a file's bytes once for the write and its progress through the check counts as the second pass (so total is twice the size, or the rate stays honest some other way: decide and say in the ledger).
- The stop flag is also looked at inside the copy and check loops, so Stop ends a 6 GB file within about a second (the temp file is removed, nothing is marked copied).

- [ ] **Step 1: Write failing tests** (temp dirs only, never `P:\` or `E:\`): `progress_is_reported_while_one_large_file_is_copied` (a few MB with a tiny chunk size or injectable chunk; the callback sees `bytes_done` rise through more than one value before the item count moves); `progress_says_checking_while_the_copy_is_read_back`; `stop_ends_a_copy_in_the_middle_of_a_file_and_leaves_no_partial_file_and_no_copied_mark`; `the_callback_is_not_called_more_often_than_the_interval` (a fake clock or a count bound).
- [ ] **Step 2: Run** `cargo test -p pixie-index copier`. Expected: new tests FAIL, the rest pass.
- [ ] **Step 3: Implement.** Chunk loop already exists in `stream_copy` (1 MiB); add the callback and stop check there, and replace the one-shot `full_hash(temp)` check by a chunked read with the same callback. Keep verification by hash and the rename; the speed work done before (no per-file `sync_all`) stays.
- [ ] **Step 4: Run tests and `cargo check --workspace --all-targets`.** Expected: PASS, clean.
- [ ] **Step 5: Commit** (only if asked): `feat: the sync copy reports bytes and "checking" while one file is copied`.

### Task 4: UI view-model and formatting

**Files:**
- Create: `ui/src/lib/activity.ts`, `ui/src/lib/activity.test.ts`
- Modify: `ui/src/lib/format.ts`, `ui/src/lib/format.test.ts`, `ui/src/lib/api.ts` (types `ActivitySnapshot`, `ActivityTask`; `activitySnapshot()`; `onActivity(cb)` wrapping the event)

**Interfaces:**
- Produces in `format.ts`: `formatDuration(secs: number): string` ("under a minute", "about 5 min", "about 1 h 20 min"), `formatRate(perSec: number, unit: string): string` ("12 photos/s" using `plural` rules is not needed: units are fixed words), `formatBytes(n: number): string` and `formatByteRate(n: number): string` (reuse an existing helper if one is already there).
- Produces in `activity.ts`:
  - `export type Row = { kind: TaskKind; label: string; state: RunState; fraction: number | null; counts: string; speed: string; left: string; explain: string }`
  - `export function view(s: ActivitySnapshot | null): { running: Row[]; finished: Row[]; overall: string; pill: 'working' | 'paused' | 'error' | null }`
  - `export function label(kind: TaskKind): string` ("Finding your files", "Reading and fingerprinting files", "Making thumbnails", "Auto tags", "Finding faces", "Grouping people", "Looking for duplicates", "Syncing to your sync folder" (the feature is called Sync in the UI; `CopyToMaster` is only the code name), "Getting models", "Waiting for your cloud drive", "Preparing email")

- [ ] **Step 1: Write failing tests**: `formatDuration` boundaries (59 s, 90 s, 3,600 s, 4,800 s); `view(null)` has `pill === null` and "Nothing is running."; an unknown total gives `fraction === null`, `left === 'Estimating…'` and no overall time; a paused row has `left === ''` and `explain` says paused; `CloudWait` in `Waiting` shows how long it has waited and no time left; errors sort first, then running, then paused, finished last; `pill` is `error` if any row is, else `paused` if every unfinished row is, else `working`; counts use `plural` ("1 file", "2 files").
- [ ] **Step 2: Run** `npm --prefix ui run test -- activity format`. Expected: FAIL.
- [ ] **Step 3: Implement** the helpers and the view-model.
- [ ] **Step 4: Run** `npm --prefix ui run test`. Expected: PASS.
- [ ] **Step 5: Commit** (only if asked): `feat: activity view-model and duration formatting`.

### Task 5: The screen and the "Processing" pill

**Files:**
- Create: `ui/src/lib/ActivityScreen.svelte`, `ui/src/lib/ProcessingPill.svelte`
- Modify: `ui/src/App.svelte` (replace `<BackgroundProgress progress={bg} onclick={openAnalysisDetails} />` at ~line 1366 with `<ProcessingPill …>`; add the screen as an overlay page opened by it; subscribe once to `onActivity` and keep the latest snapshot in state)

**Interfaces:**
- Consumes: `view`, `ActivitySnapshot` (Task 4); existing handlers for pause/resume and retry (today passed to `AnalysisSettings` as `onpause` / `onretry`).
- Produces: `ActivityScreen` with props `{ snapshot: ActivitySnapshot | null; onpause: (paused: boolean) => void; onretry: () => void; onstop?: () => void; compact?: boolean }` (`compact` is for the Add folder popup: no page chrome); `ProcessingPill` with `{ pill: 'working' | 'paused' | 'error' | null; onclick: () => void }` whose text is always `Processing`.

- [ ] **Step 1: Write the components.** Screen: overall line at the top; a row per running operation (name, a `<progress>` or an indeterminate bar, counts, speed, left, a one-line `explain` such as "OneDrive is downloading these files before Pixie can read them"); a "Finished" list; Pause/Resume (white) and Retry (white, shown only on error); `onstop` shows "Stop scanning" when a scan runs. Every control has a `title`.
- [ ] **Step 2: Wire the pill** (`title`: "Pixie is working in the background. Click to see what it is doing and how long it will take."; `aria-label` "Processing") and the overlay in `App.svelte`; Escape and a Close button return.
- [ ] **Step 3: Run** `npm --prefix ui run test` (includes `tooltips.test.ts`) and `npm --prefix ui run check`. Expected: PASS, no warnings.
- [ ] **Step 4: Commit** (only if asked): `feat: Processing pill and the one progress screen`.

### Task 6: The Add folder popup shows the same screen

**Files:**
- Modify: `ui/src/lib/LoadModal.svelte`, `ui/src/lib/App.svelte` (the `scanning` / `analysing` phase props), `ui/src/lib/analysisView.ts` (remove display strings no longer used; keep what decides "finished")

**Interfaces:**
- Consumes: `ActivityScreen` with `compact` (Task 5).
- Produces: `LoadModal` props lose `view` (the analysing bar and counters); gain `snapshot: ActivitySnapshot | null` and `onpause`, `onretry`. The photo strip, current file name, notice, "Carry on in the background" and Stop buttons stay.

- [ ] **Step 1: Update `loadFlow.test.ts` / component expectations first**: the flow still moves welcome -> scanning -> analysing -> all set, and "finished" is still decided from `LoadProgress` (not from the new screen), so existing `loadFlow.ts` tests must pass unchanged.
- [ ] **Step 2: Replace** the bar and counters block in `LoadModal.svelte` with `<ActivityScreen compact … />`.
- [ ] **Step 3: Run** `npm --prefix ui run test` and `npm --prefix ui run check`. Expected: PASS.
- [ ] **Step 4: Commit** (only if asked): `feat: the Add folder popup shows the shared progress screen`.

### Task 7: Remove the old progress displays

**Files:**
- Modify: `ui/src/lib/Settings.svelte`, `AnalysisSettings.svelte` (keep the on/off choices; drop bar, counts, Pause and Retry), `PhotoFolders.svelte` (drop the `analysing` prop and "% analysed"), `PeopleTab.svelte` (progress bar near line 274), `DuplicatesTab.svelte` (line 97), `SyncTool.svelte` (live copy line; keep Copy/Stop and the last-run line), `BackgroundNote.svelte` (its "progress indicator" text now says the "Processing" button), `App.svelte`
- Delete: `ui/src/lib/BackgroundProgress.svelte`, `ui/src/lib/backgroundProgress.ts` and their tests, plus any helper (`scanProgress.ts` pieces, `analysisView.ts` strings) left unused

- [ ] **Step 1: Remove** each display and the props that fed only it; follow the compiler and `svelte-check` to the dead code and delete it.
- [ ] **Step 2: Update tests** that asserted the old text (`filterSummary`, `master.test.ts` copy line, `backgroundProgress.test.ts` deleted).
- [ ] **Step 3: Run** `npm --prefix ui run test`, `npm --prefix ui run check`, `cargo check --workspace --all-targets`. Expected: PASS, no warnings, no unused exports.
- [ ] **Step 4: Commit** (only if asked): `refactor: remove the scattered progress displays`.

### Task 8: Run it and check the numbers

- [ ] **Step 1:** Run the app (use the `run` skill) with a separate cargo target dir. Add a folder of a few hundred photos: the popup shows every operation with throughput and a time left that settles after about 10 s; compare the elapsed time with the estimate.
- [ ] **Step 2:** Choose "Carry on in the background": the pill says "Processing"; clicking it shows the same rows; it disappears when everything finishes.
- [ ] **Step 3:** Add a OneDrive folder with online-only files: the "Waiting for your cloud drive" row appears with a believable rate, and a disconnected network shows the wait growing, not a time left.
- [ ] **Step 4:** Pause and resume from the screen; force a model error (offline first run) and Retry.
- [ ] **Step 5:** Run `cargo test --workspace`, `npm --prefix ui run test`, `npm --prefix ui run check`, `cargo check --workspace --all-targets`; quote the results in the report. Check README / specs for mentions of the old progress indicator and update them.

---

## Self-review

- **Coverage:** one screen with counts, totals, throughput, time left (Tasks 1, 4, 5); pill always "Processing" (5); Add folder popup (6); old displays removed (7); uncounted things (cloud sync, bytes read) counted (3).
- **Types:** `TaskKind` / `RunState` / `TaskSnapshot` in Rust mirror `ActivityTask` in `api.ts`; the pill state names `working | paused | error | null` are the same in Tasks 4 and 5.
- **Open for you:** (1) the pill hides when nothing is running (as now), so the screen cannot be opened to look at "finished" totals; say if you want it always visible. (2) Reading online-only cloud files makes OneDrive download them; this plan measures it but does not change it. (3) Totals for cloud waits restart each session. (4) A macOS check of `SF_DATALESS` is needed on the Mac.
