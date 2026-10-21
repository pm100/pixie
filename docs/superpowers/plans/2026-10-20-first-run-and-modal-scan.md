# First run and modal scan Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A welcome screen that suggests photo folders on every launch while none exist, then a modal flow (scan strip, analysing, all set) that replaces the hidden, flickering background load for first loads and for folders added later.

**Architecture:** Pure folder-suggestion logic in `pixie-index` (fakeable with temp dirs), a scan-job queue so a folder added during a running job is scanned by it, scan progress that carries a recently stored photo for the strip, and a thumbnail queue length for the "Preparing previews" line. The UI gets one pure state/progress module (`loadFlow.ts`) and three components (welcome, loading modal, all set) wired into `App.svelte`. Launch rescans, periodic rescans and Reload stay in the background.

**Tech Stack:** Rust (`pixie-index`, `pixie-image`, Tauri 2 commands/events), Svelte 5 + TS, vitest.

**Spec:** `docs/superpowers/specs/2026-10-20-first-run-and-modal-scan-design.md` (mockups: `.superpowers/brainstorm/891-1792539589/content/{welcome-and-scan,finish-screen,analysing-screen}.html`).

## Global Constraints

- No `git commit` or `git push` unless the user says so (AGENTS.md): "Commit" steps below mean "stage and leave ready".
- `src-tauri/src/lib.rs` and `ui/src/lib/api.ts` use CRLF line endings: edit with the Edit tool or a script that preserves them (never `sed -i`).
- Test commands: `cargo test --workspace --exclude pixie-app --no-fail-fast`, `cargo check -p pixie-app --no-default-features`, `npm --prefix ui run test`, `npx svelte-check` in `ui/`, `npm --prefix ui run build`. Windows launch for manual checks: `just winlaunch` (vcpkg env is set in the `justfile`).
- Welcome shows on every launch while no folders exist; "Skip for now" closes it, it returns next launch. Adding a folder later skips the welcome.
- Launch rescans, periodic rescans, watchers and Reload stay background (issue #32); `Nav.svelte`'s `scanLine` stays for them.
- Never probe network or removable drives automatically; skip hidden, system, app, cache and dependency folders (`AppData`, `Library`, `node_modules`, `.git`, names starting with `.`). Counts are time-boxed (about 2 s per folder) and capped (shown as "10,000+").
- Pre-ticked: OS Pictures and Videos (Movies on macOS), OneDrive/Dropbox/iCloud photo folders. Unticked: Downloads, Desktop, everything from "look a little further". A folder nested in another suggestion is "included in ..." and not offered separately.
- Models already download at launch; the analysing step shows their state, never a separate screen. Failed download: error text plus Retry (`analysisRetry()`, which retries both the search and the face service).
- Copy is fixed by the mockups: "Welcome to Pixie", "Pictures loaded — now analysing", "Carry on in the background", "All set — N items found", "Faces found." box with "Open People" and "Start browsing", note "You can add, remove or change these any time in Settings → Photo folders."

## Review Focus

1. A folder with no media (or unreadable) among the ticked ones: its scan error is shown, the rest proceed; if the total found is 0 the end screen says nothing was found and offers "Add another folder…" instead of "All set" (Tasks 6, 9).
2. The same folder added twice, or a folder already known: no second scan job, no duplicate row (Tasks 4, 9).
3. A huge library: the strip's events are throttled (at most about five a second) and the UI never queues them up (Tasks 3, 6).
4. Offline or failed model download, or analysis paused in Settings: the analysing step shows the error/"Paused" with its Retry/Resume and "Carry on in the background" always works (Tasks 6, 8).
5. Stop pressed mid-scan, or the app closed mid-flow: what was stored stays; next launch's background rescan completes it (Tasks 4, 9).

---

### Task 1: Folder suggestions (pure)

**Files:**
- Create: `crates/pixie-index/src/suggest.rs`; Modify: `crates/pixie-index/src/lib.rs` (`pub mod suggest;`)
- Test: in `suggest.rs` (`#[cfg(test)]`, `tempfile` dirs)

**Interfaces:**
- Produces:
  - `#[derive(Serialize, Clone, PartialEq, Debug)] pub struct Suggestion { pub path: String, pub name: String, pub kind: SuggestionKind, pub ticked: bool, pub note: Option<String>, pub included_in: Option<String> }`
  - `#[derive(Serialize, Clone, Copy, PartialEq, Debug)] #[serde(rename_all = "lowercase")] pub enum SuggestionKind { Known, Further }`
  - `pub struct Places { pub pictures: Option<PathBuf>, pub videos: Option<PathBuf>, pub downloads: Option<PathBuf>, pub desktop: Option<PathBuf>, pub home: Option<PathBuf>, pub onedrive: Vec<PathBuf> }`
  - `pub fn known_places(p: &Places) -> Vec<Suggestion>` (only directories that exist; order: Pictures, Videos, OneDrive Pictures, Dropbox, iCloud Photos, Camera Roll/Screenshots (as `included_in` when under another suggestion), Downloads, Desktop)
  - `pub struct MediaCount { pub n: usize, pub capped: bool }` and `pub fn count_media(dir: &Path, budget: Duration, cap: usize) -> MediaCount`
  - `pub fn look_further(roots: &[PathBuf], known: &[Suggestion], budget: Duration, min_files: usize) -> Vec<Suggestion>` (depth at most 3 below each root; `kind: Further`, `ticked: false`, `note` like "412 photos"; skips names in the skip list; never returns a path equal to, inside or containing a `known` suggestion)
- Consumes: the scanner's media-extension test (find it in `scanner.rs`; expose it `pub(crate)` if private).

- [ ] **Step 1: Write failing tests** in `suggest.rs`: `known_places_lists_only_existing_dirs_and_ticks_the_confident_ones` (Pictures/Videos/OneDrive ticked; Downloads/Desktop unticked; a missing Videos dir absent); `a_folder_inside_another_suggestion_is_included_not_offered` (Camera Roll under Pictures has `included_in == Some("Pictures")`); `count_media_counts_media_files_only_and_caps` (5 jpg + 2 txt, cap 3 gives `{n:3, capped:true}`); `count_media_stops_at_the_time_budget` (budget `Duration::ZERO` returns promptly with `capped:true`); `look_further_finds_dense_media_folders_and_skips_the_rest` (a folder with 60 jpgs found; `node_modules`, `.hidden`, `AppData` skipped even with media inside; a folder under a known suggestion not returned; depth 4 not found).
- [ ] **Step 2: Run** `cargo test -p pixie-index --lib suggest` → FAIL (items not defined).
- [ ] **Step 3: Implement** the signatures above. `count_media`/`look_further` are iterative walks (explicit stack) that check `Instant::now()` against the budget per directory entry and never follow symlinks.
- [ ] **Step 4: Run** the same command → PASS. **Step 5: Stage** `feat: folder suggestions`.

### Task 2: Suggest commands (OS places, counts, look further)

**Files:** Modify `src-tauri/src/lib.rs` (commands + handler list), `ui/src/lib/api.ts`.

**Interfaces:**
- Consumes: Task 1.
- Produces (Rust): `#[tauri::command] async fn suggest_folders(app: AppHandle) -> Result<Vec<Suggestion>, PixieError>` returns `known_places(..)` at once (Places from `app.path()`: `picture_dir`, `video_dir`, `download_dir`, `desktop_dir`, `home_dir`, plus OneDrive from the env vars `OneDrive`, `OneDriveConsumer`, `OneDriveCommercial` joined with `Pictures`), then spawns one thread that emits `suggestion-count` `{ path: String, n: usize, capped: bool }` for each suggestion (`count_media(dir, 2 s, 10_000)`), then runs `look_further` over the home folder and the roots of fixed local drives (Windows: `GetDriveTypeW == DRIVE_FIXED`; macOS: none beyond home) and emits `suggestion-found` (a `Suggestion`), then `suggestions-done`.
- Produces (TS, `api.ts`): `export type Suggestion = { path: string; name: string; kind: 'known' | 'further'; ticked: boolean; note: string | null; included_in: string | null }`, `export const suggestFolders = () => invoke<Suggestion[]>('suggest_folders')`.

- [ ] **Step 1: Implement** the command and the `api.ts` pieces; register `suggest_folders` in `generate_handler!`.
- [ ] **Step 2: Run** `cargo check -p pixie-app --no-default-features` and `npx svelte-check` (in `ui/`) → clean.
- [ ] **Step 3: Manual check** later in Task 9 (the command needs a running app). **Step 4: Stage** `feat: suggest_folders command`.

### Task 3: Scan progress carries a recent photo (throttled)

**Files:** Modify `crates/pixie-index/src/scanner.rs`; Test: in the same file's tests.

**Interfaces:**
- Produces: `ScanProgress` gains `pub latest: Option<LatestItem>`; `#[derive(Debug, Clone, Serialize)] pub struct LatestItem { pub id: PhotoId, pub mtime: i64, pub size: i64, pub name: String }`. While reading metadata, a progress call is made whenever at least 200 ms passed since the last one (in addition to the existing every-50 call), with `latest` set to the photo just stored; walk-phase and final calls have `latest: None`.
- Consumes: `Db::upsert_photo` returns the photo id (already the case).

- [ ] **Step 1: Write failing tests:** `progress_during_reading_names_a_stored_photo` (400 tiny jpegs; every progress with `latest: Some(l)` has `db.get_photo(l.id)` present and `l.name` equal to that file's name); `progress_is_throttled` (the number of progress calls with `latest` is less than 100 for the 400 files, which a per-photo call would exceed).
- [ ] **Step 2: Run** `cargo test -p pixie-index --lib scanner` → FAIL (field missing).
- [ ] **Step 3: Implement** in the consuming loop of `scan_folder_with`: keep `last_emit: Instant`; after each stored photo, if `last_emit.elapsed() >= 200 ms`, call `on_progress` with `latest` and reset the clock. Update the other `ScanProgress { .. }` constructions with `latest: None`.
- [ ] **Step 4: Run** `cargo test --workspace --exclude pixie-app` → PASS. **Step 5: Stage** `feat: scan progress names a recent photo`.

### Task 4: Scan queue: a folder added during a running job is scanned by it

**Files:** Modify `crates/pixie-index/src/scanner.rs` (add `ScanQueue`), `src-tauri/src/lib.rs`, `src-tauri/src/state.rs`, `ui/src/lib/api.ts`.

**Interfaces:**
- Produces (Rust): `#[derive(Clone, Default)] pub struct ScanQueue(Arc<Mutex<QueueState>>)` with `pub fn start_job(&self)` (marks running, clears), `pub fn push(&self, id: FolderId) -> bool` (true and queued only while a job runs and has not ended; false otherwise), `pub fn take(&self) -> Vec<FolderId>` (drains), `pub fn end_job(&self) -> Vec<FolderId>` (atomically drains what is left and marks not running, so a `push` is either returned here or answered `false`).
- Produces (Tauri): `#[tauri::command] fn scan_folders(app, state, paths: Vec<String>) -> Result<ScanStart, PixieError>` with `#[derive(Serialize)] pub struct ScanStart { pub folder_ids: Vec<i64>, pub queued: bool }`: `add_folder` for each path (known paths just return their id), then if a job runs `push` all ids (`queued: true`), otherwise start a job over them (`queued: false`). The job loop drains `take()` after its own list until `end_job()` returns empty, before `scan-done`. `AppState` gains `pub queue: ScanQueue`.
- Produces (TS): `export type ScanStart = { folder_ids: number[]; queued: boolean }`, `export const scanFolders = (paths: string[]) => invoke<ScanStart>('scan_folders', { paths })`.

- [ ] **Step 1: Write failing tests** (`ScanQueue`): `push_is_refused_when_no_job_runs`; `push_while_running_is_returned_by_take_then_end_job`; `an_id_pushed_around_end_job_is_either_returned_by_it_or_refused` (loop 1,000 times with a racing thread; every id is accepted-and-returned exactly once or refused, never lost).
- [ ] **Step 2: Run** `cargo test -p pixie-index --lib scanner::queue` → FAIL. **Step 3: Implement** `ScanQueue` and the command; the existing `start_scan` stays for launch rescans and Reload (it calls `start_job`/`end_job` too so the queue stays correct).
- [ ] **Step 4: Run** `cargo test --workspace --exclude pixie-app`, `cargo check -p pixie-app --no-default-features` → PASS. **Step 5: Stage** `feat: scan queue and scan_folders`.

### Task 5: Preview queue length

**Files:** Modify `crates/pixie-image/src/service.rs`, `src-tauri/src/lib.rs`, `ui/src/lib/api.ts`.

**Interfaces:** Produces `ThumbService::pending_count(&self) -> usize` (queued plus in-flight jobs); `#[tauri::command] fn previews_pending(state) -> usize`; `export const previewsPending = () => invoke<number>('previews_pending')`.

- [ ] **Step 1: Write failing test** in `service.rs` (use the `with_generator` seam with a generator that blocks on a channel): `pending_count_rises_with_prefetch_and_returns_to_zero` (prefetch 5, count is 5; release; wait until 0).
- [ ] **Step 2: Run** `cargo test -p pixie-image --lib pending_count` → FAIL. **Step 3: Implement** (count `st.pending` entries plus jobs being generated). **Step 4: Run** `cargo test --workspace --exclude pixie-app` → PASS. **Step 5: Stage** `feat: preview queue length`.

### Task 6: Flow and progress logic (pure TS)

**Files:** Create `ui/src/lib/loadFlow.ts`, `ui/src/lib/loadFlow.test.ts`.

**Interfaces:**
- Produces:
  - `export type Phase = 'off' | 'welcome' | 'scanning' | 'analysing' | 'allset'`
  - `export function phaseAfterScan(o: { stopped: boolean; found: number }): Phase` (stopped: `'off'`; found 0: `'allset'` with the nothing-found copy chosen by `allSetSummary`; otherwise `'analysing'`)
  - `export type Line = { key: 'previews' | 'search' | 'faces'; title: string; why: string; text: string; fraction: number | null; error: string | null; paused: boolean; done: boolean }`
  - `export function analysingLines(a: AnalysisStatus | null, p: AnalysisStatus | null, previews: { left: number; max: number }, peopleCount: number): Line[]` (three lines, titles "Preparing previews", "Understanding your photos", "Finding faces"; model downloading shows `Getting the search model… 62%` / `Getting the face model… 62%` with that fraction; `error` from a failed model or `status.error`; `done` when previews left 0, or `done >= total` with models ready)
  - `export const analysingFinished = (lines: Line[]): boolean` (every line `done`)
  - `export function allSetSummary(o: { folders: { name: string; count: number }[]; photos: number; videos: number; seconds: number; facesFound: boolean }): { title: string; detail: string; rows: { name: string; text: string }[]; showPeopleBox: boolean }` (title `All set — 16,497 items found`; zero items: title `Nothing was found`, `showPeopleBox` false; detail `14,230 photos and 2,267 videos, in 4 folders. That took 3 minutes.` with minute/second rounding)
  - `export function startLabel(ticked: number): { label: string; disabled: boolean }` (`Start — look in 4 folders`, singular for 1, disabled for 0)
  - `export const throttleSamples = <T>(): ((item: T, now: number) => T | null)` returning the item only if at least 200 ms passed since the last one it returned.
- Consumes: `AnalysisStatus` from `analysis.ts` (the people status has the same shape).

- [ ] **Step 1: Write failing tests** for each function with the exact values above, plus: `paused_shows_paused_and_is_not_done`, `a_failed_model_is_an_error_line`, `a_stopped_scan_ends_the_flow`, `zero_found_goes_to_the_end_screen_with_nothing_found`, `throttle_drops_samples_inside_200ms`.
- [ ] **Step 2: Run** `npm --prefix ui run test -- loadFlow` → FAIL. **Step 3: Implement.** **Step 4: Run** → PASS. **Step 5: Stage** `feat: first-run flow logic`.

### Task 7: Suggestions state (pure TS) and API events

**Files:** Create `ui/src/lib/suggestions.ts`, `ui/src/lib/suggestions.test.ts`; Modify `ui/src/lib/api.ts`.

**Interfaces:**
- Produces: `export type Row = Suggestion & { ticked: boolean; count: { n: number; capped: boolean } | null }`; `export function applyCount(rows: Row[], c: { path: string; n: number; capped: boolean }): Row[]`; `export function addFound(rows: Row[], s: Suggestion): Row[]` (ignores a path already present or one inside/containing a present row); `export function toggle(rows: Row[], path: string): Row[]`; `export const tickedPaths = (rows: Row[]): string[]` (excludes rows with `included_in`); `export const countText = (c: Row['count'], kind: Suggestion['kind']): string` (`about 3,400 photos`/`10,000+`/`counting…`).
- `api.ts`: `onSuggestionCount`, `onSuggestionFound`, `onSuggestionsDone` thin `listen` wrappers.

- [ ] **Step 1: Write failing tests:** `a_count_fills_the_matching_row`, `a_found_folder_inside_a_row_is_ignored`, `rows_included_elsewhere_are_never_returned_as_ticked`, `capped_counts_read_10000_plus`, `toggle_flips_one_row_only`.
- [ ] **Step 2: Run** `npm --prefix ui run test -- suggestions` → FAIL. **Step 3: Implement. Step 4: Run** → PASS. **Step 5: Stage** `feat: suggestions state`.

### Task 8: Components

**Files:** Create `ui/src/lib/Welcome.svelte`, `ui/src/lib/LoadModal.svelte`, `ui/src/lib/AllSet.svelte`.

**Interfaces:**
- `Welcome` props: `{ rows: Row[]; ontoggle: (path: string) => void; onadd: () => void; onskip: () => void; onstart: () => void }`; renders the copy in Global Constraints, the "We found these" list and a "Look a little further" group.
- `LoadModal` props: `{ phase: 'scanning' | 'analysing'; scan: { seen: number; indexed: number; toIndex: number; folder: string | null }; strip: { id: number; mtime: number; size: number; name: string }[]; lines: Line[]; onstop: () => void; onbackground: () => void; onretry: () => void; onresume: () => void }`; `scanning` shows the slideshow strip (centre photo large via `thumbUrl('grid', id, mtime, size)`, neighbours fading), `scanText`, a progress bar from `readFraction`, and "Stop"; `analysing` shows the three `Line`s with their `why` text, an error row with "Retry" or a "Paused" row with "Resume", and "Carry on in the background".
- `AllSet` props: `{ summary: ReturnType<typeof allSetSummary>; onpeople: () => void; onbrowse: () => void; onadd: () => void }`; the blue "Faces found." box only when `showPeopleBox`; zero-found shows "Add another folder…".

- [ ] **Step 1: Implement** the three components against the mockups, using the existing look (`ConfirmDialog.svelte` scrim for the modal, 14 px text, `#2f6fed` bars). **Step 2: Run** `npx svelte-check` (in `ui/`) → 0 errors. **Step 3: Stage** `feat: welcome, loading and all-set screens`. (Behaviour is covered by Tasks 6, 7 and the manual run in Task 9.)

### Task 9: Wire it into `App.svelte`

**Files:** Modify `ui/src/App.svelte`, `ui/src/lib/Settings.svelte` (its `onadd` stays `chooseFolder`), `ui/src/lib/PhotoFolders.svelte` only if its copy mentions scanning in Settings.

**Interfaces:** Consumes everything above. State added to `App.svelte`: `phase: Phase`, `rows: Row[]`, `strip` (last 5 samples through `throttleSamples`), `foundBefore` (item count before the flow, to compute what this run found), `startedAt`.

- [ ] **Step 1: Welcome.** After `listFolders()` on mount, when empty set `phase = 'welcome'`, call `suggestFolders()` into `rows` and subscribe to the three suggestion events until the phase leaves `welcome`. "Skip for now" sets `phase = 'off'` (existing empty state remains). "Add another folder…" uses the dialog `open` and appends a ticked row. "Start" calls `scanFolders(tickedPaths(rows))` and sets `phase = 'scanning'`.
- [ ] **Step 2: Add later.** `chooseFolder` (Settings) becomes: dialog, `scanFolders([dir])`, `phase = 'scanning'` (no welcome). The old `runScan(id)` call there is removed, which removes the "already running ... will not be scanned" path for adds; `runScan` stays for Reload.
- [ ] **Step 3: Scanning.** While `phase === 'scanning'`, render only `LoadModal` (Browse is not rendered, so nothing flickers), feed `scan-progress` (counts, `folder_id` to a folder name, `latest` through `throttleSamples` into `strip`), "Stop" calls `cancelScan()`. On `scan-done`, `phase = phaseAfterScan({ stopped, found })` where `found` is the library total minus `foundBefore`; stopped or `off` runs the existing refresh and shows Browse.
- [ ] **Step 4: Analysing.** Poll `previewsPending()` every second (track `max`), use the existing `analysis`/`peopleState` and `peopleList.length`; `lines = analysingLines(..)`; when `analysingFinished(lines)` set `phase = 'allset'`; "Retry" calls `analysisRetry()`; "Resume" calls `analysisSetPaused(false)`; "Carry on in the background" sets `phase = 'off'` and refreshes Browse once.
- [ ] **Step 5: All set.** `AllSet` with `allSetSummary` from `listFolderInfo()` counts, photo/video totals (`countPhotos` with kind), elapsed seconds and `facesFound = peopleState?.done > 0 && peopleList.length > 0`; "Open People" sets `tab = 'faces'` and `phase = 'off'`; "Start browsing" sets `phase = 'off'`.
- [ ] **Step 6: Launch behaviour unchanged.** The existing launch `startScan(null)` for known folders stays in the background with the `scanLine` in the nav; confirm no modal appears when folders exist.
- [ ] **Step 7: Run** `npx svelte-check`, `npm --prefix ui run test`, `npm --prefix ui run build`, `cargo check -p pixie-app --no-default-features` → all clean.
- [ ] **Step 8: Manual first run** (Windows, `just winlaunch`; delete `%LOCALAPPDATA%\app.pixie.desktop` first, Settings → Developer → Open data folder shows where): welcome lists your Pictures/OneDrive; untick one; Start; strip moves; analysing shows three lines; background button reaches Browse; add a folder from Settings goes straight to the strip; adding a second folder during a launch rescan scans it after the rescan. Record what you saw. **Step 9: Stage** `feat: first-run welcome and modal scan`.

### Task 10: Docs and cross-platform check

**Files:** Modify the spec (status "implemented"), README (first-run paragraph), issue #32 text (comment: user-started first loads are now modal; hidden scans still background).

- [ ] **Step 1: Run** the full suite on Windows (the Global Constraints commands) and `cargo test -p pixie-analysis --test clip_real --test faces_real -- --ignored`.
- [ ] **Step 2: Mac:** `ssh mac`, apply the same working tree, run the Rust and UI suites there (Node is v26 via Homebrew), and check `suggest_folders` returns `~/Pictures` and `~/Movies` (macOS) with `cargo test -p pixie-index --lib suggest` (path-style cases are covered by temp dirs).
- [ ] **Step 3:** Tell the user what to look at: whether the guessed folders felt right, the strip's pace, the analysing wording and how long it took on a large library. Do not commit or push until told. **Step 4: Stage** `docs: first-run flow`.
