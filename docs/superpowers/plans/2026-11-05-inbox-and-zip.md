# Import: the Inbox and zip files Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A zip file can be opened in Import: its entries are read, shown with a summary and the space they need, extracted safely into an Inbox folder, and then browsed, kept (moved into a library folder) or discarded, like a card.

**Architecture:** An Inbox item is a *temporary source* (already built: `folders.kind = 'source'`, photos `hidden = 2`, light scan, import log) whose folder lives under Pixie's data directory (`inbox/`) and whose files were extracted from a zip. So Browse, the picked set, the source node in the tree and Keep already work for it. What is new: reading and safely extracting a zip (`zipsrc.rs`), the Inbox rules (a folder that persists across runs, free-space check, leftover cleanup, Keep that **moves** and a Discard that deletes) in `inbox.rs`, the commands and events, and a zip panel in the Import tool.

**Tech Stack:** Rust (`zip` 9, MIT, `default-features = false`, `deflate` only), rusqlite, Tauri 2, Svelte 5.

**Spec:** `docs/superpowers/specs/2026-11-01-import-export-design.md` ("The Inbox", "A zip file"). Issue #44. Later work (7z, rar, tar.gz, passwords, multi-part archives, Google Takeout and iCloud exports) is #127.

## Global Constraints

- Never commit or push; leave all changes uncommitted (AGENTS.md, CLAUDE.md).
- No compiler warnings (`cargo check --workspace --all-targets`) and no `svelte-check` warnings; fix them, do not silence them. Delete dead code.
- Use `CARGO_TARGET_DIR=target-claude` and test the changed crate while working; run the workspace once at the end.
- A new dependency means `just notices`, and `THIRD-PARTY-NOTICES.md` and `ui/src/generated/third-party.json` change with it.
- The zip file itself is only read, never changed or deleted. What Pixie writes is only under the Inbox folder, and nothing is ever written outside it, whatever an entry is called.
- Every control needs a `title` sentence; buttons follow the colour rules (`primary` for the one main action, `danger` for deleting); no control in the middle of a sentence; counts use `plural`.
- Some files are committed CRLF: use Edit, or open with `newline=''`; check `git diff --stat`. Python heredocs mangle backslashes in this environment: write Rust or TS containing `\` with the Edit tool, and read the result back.
- Verify in the real app (Task 6) before saying it works: the method is in "Verification" below.

## Review Focus

- **Zip-slip:** an entry named `../../evil.jpg`, `/etc/x`, `C:\x`, `\\server\share\x`, or one that becomes a parent path after normalising, never writes outside the Inbox folder.
- **Zip bombs:** a tiny zip that expands to terabytes, or millions of entries, is refused before extraction starts (caps on total uncompressed size, entry count and compression ratio), and extraction stops if the real size passes what the header claimed.
- **Names that collide:** two entries with the same name, or names that differ only by case (they are the same file on Windows and macOS), both survive (the second gets a number); reserved Windows names (`CON`, `NUL`, `COM1`), trailing dots and spaces, over-long paths and characters a file system cannot hold are made safe and listed.
- **A damaged zip:** truncated or with a wrong CRC: entries before the damage are kept, the damaged one is removed and listed, and the run ends with what it did.
- **Disk full or Stop part way:** no partial file is left in the Inbox; the source can still be browsed or discarded.
- **The same zip twice:** its identity (name, size, modified time) gives the same source key, so Keep's log recognises what was kept, and opening it again does not extract everything a second time into a second folder without saying so.

---

## File Structure

- Create `crates/pixie-index/src/zipsrc.rs`: read a zip's entries (`list_zip`), safe names, limits, and `extract_zip`.
- Create `crates/pixie-index/src/inbox.rs`: the Inbox folder, free space, leftover cleanup, `discard_source`, and the move variant of Keep.
- Modify `crates/pixie-index/src/keep.rs`: `run_keep` takes a `Keep` mode (`Copy` or `Move`).
- Modify `crates/pixie-index/src/lib.rs`, `crates/pixie-index/Cargo.toml` (the `zip` dependency).
- Modify `src-tauri/src/importer.rs`, `src-tauri/src/lib.rs`: commands and events, start-up cleanup.
- Modify `ui/src/lib/api.ts`, `importStore.svelte.ts`, `importer.ts` (+ test), `ImportTool.svelte`, `Nav.svelte`, `askLocal.ts`, `askGuide.ts`.
- Modify `THIRD-PARTY-NOTICES.md`, `ui/src/generated/third-party.json` (via `just notices`).

### Task 1: Reading a zip: its entries, safe names and limits

**Files:**
- Create: `crates/pixie-index/src/zipsrc.rs`
- Modify: `crates/pixie-index/Cargo.toml`, `crates/pixie-index/src/lib.rs`
- Test: `crates/pixie-index/src/zipsrc.rs`

**Interfaces:**
- Produces:
  ```rust
  pub const MAX_ENTRIES: usize = 500_000;
  pub const MAX_RATIO: u64 = 200;            // uncompressed / compressed, per entry, above which an entry is refused
  pub const MAX_TOTAL: u64 = 2 << 40;        // 2 TiB of uncompressed data in one zip
  #[derive(Debug, Clone, PartialEq, Eq)]
  pub enum Skip { Directory, NotMedia, Encrypted, Unsafe(String), TooBig, Link, Archive }   // why an entry is not extracted
  #[derive(Debug, Clone, PartialEq, Eq)]
  pub struct ZipEntry { pub index: usize, pub name: String /* the safe relative path, forward slashes */, pub original: String, pub size: u64, pub packed: u64, pub crc: u32, pub mtime: Option<i64> /* unix ms, the entry's own local time read as UTC */, pub kind: Option<MediaKind>, pub skip: Option<Skip> }
  #[derive(Debug, Default, PartialEq, Eq)]
  pub struct ZipListing { pub entries: Vec<ZipEntry>, pub photos: usize, pub videos: usize, pub other: usize, pub archives: usize, pub encrypted: usize, pub unsafe_names: usize, pub bytes: u64 /* of the entries to extract */ }
  pub fn list_zip(path: &Path) -> Result<ZipListing>;
  pub fn safe_name(raw: &str) -> std::result::Result<String, String>;   // Err(why) when it cannot be made safe
  ```
  `entries` lists every entry; `skip` says which are not extracted. `bytes` counts only entries with `skip == None`. `Err` for an unreadable zip, too many entries, or `MAX_TOTAL` passed (plain sentences).

- [ ] **Step 1: Add the dependency** to `crates/pixie-index/Cargo.toml`: `zip = { version = "9", default-features = false, features = ["deflate"] }`, and `pub mod zipsrc;` in `lib.rs`. Run `just notices`.
- [ ] **Step 2: Write the failing tests** (build zips in the test with `zip::ZipWriter` into a `tempfile` path, `SimpleFileOptions::default()`; a small helper `make_zip(&[(&str, &[u8])])`):
  - `safe_name` table: `"DCIM/a.jpg"` → `DCIM/a.jpg`; `"a\\b.jpg"` → `a/b.jpg`; `"./a.jpg"` → `a.jpg`; `"../../evil.jpg"`, `"/etc/x.jpg"`, `"C:/x.jpg"`, `"\\\\server\\share\\x.jpg"`, `""`, `"a/../../x.jpg"` → `Err`; `"CON.jpg"` and `"dir/NUL"` → a safe replacement (`_CON.jpg`); `"a. "` and `"x /y.jpg"` have trailing dots and spaces trimmed from each part; a part over 120 characters is shortened keeping its extension; control characters and `<>:"|?*` become `_`.
  - `a_listing_counts_photos_videos_and_other_and_sums_the_bytes_to_extract`: a zip with `a.jpg`, `b.JPG`, `c.mp4`, `notes.txt`, `dir/` and `inner.zip`: `photos 2, videos 1, other 1 (notes.txt), archives 1`, directory entries are `Skip::Directory` and not counted as other, `bytes` is the sum of the three media sizes.
  - `unsafe_names_are_skipped_with_a_reason_and_counted`: `../x.jpg` is `Skip::Unsafe(..)`, `unsafe_names == 1`, the others are listed normally.
  - `names_that_collide_are_kept_apart`: `A.jpg`, `a.jpg` and `a.jpg` again give three distinct `name`s (`A.jpg`, `a-1.jpg`, `a-2.jpg`, the later ones numbered, comparing case-insensitively).
  - `encrypted_entries_are_skipped` (write one with `.with_aes_encryption` only if the feature is not needed; otherwise build the zip bytes by hand or mark the entry encrypted through the writer's legacy `with_deprecated_encryption("x")`; if neither is possible without another feature, test `Skip::Encrypted` through a small pure function `skip_for(flags)` and say so here).
  - `a_zip_bomb_is_refused_by_ratio_and_by_total`: an entry of 10 MiB zeros compresses far above `MAX_RATIO` and is `Skip::TooBig`; a listing whose entries claim more than `MAX_TOTAL` in total is `Err` (test with a lowered limit through an internal `list_zip_with(path, limits)` that `list_zip` calls with the constants).
  - `a_file_that_is_not_a_zip_is_a_plain_error`: `list_zip` of a text file → `Err` whose sentence says it is not a zip file.
  - `mtime_is_read_as_the_entrys_own_local_time`: an entry written with a given `DateTime` reports the matching unix ms (as if UTC).
- [ ] **Step 3: Run** `cargo test -p pixie-index zipsrc`. Expected: FAIL (module missing).
- [ ] **Step 4: Implement** `list_zip`, `list_zip_with` and `safe_name`. Use `ZipArchive::new(File)`, `by_index_raw` (reading the directory only, no decompression), `enclosed_name()` as a first check and then your own `safe_name`; media kind from the extension via the same lists the scanner uses (`media_kind` in `scanner.rs`; make it `pub(crate)` if it is private); `.zip` entries are `Skip::Archive`; unix-mode symlinks (`unix_mode() & 0o170000 == 0o120000`) are `Skip::Link`; ratio check `size > packed.max(1) * MAX_RATIO && size > 1 MiB`. Collisions are numbered case-insensitively in a `HashSet<String>` of lower-cased names.
- [ ] **Step 5: Run** `cargo test -p pixie-index`. Expected: PASS.
- [ ] **Step 6: Leave uncommitted.**

### Task 2: Extracting safely

**Files:**
- Modify: `crates/pixie-index/src/zipsrc.rs`
- Test: `crates/pixie-index/src/zipsrc.rs`

**Interfaces:**
- Consumes: `list_zip`, `ZipEntry`, `Skip` (Task 1).
- Produces:
  ```rust
  #[derive(Debug, Clone, Default, serde::Serialize)]
  pub struct ExtractReport { pub extracted: usize, pub bytes: u64, pub failed: usize, pub problems: Vec<String> /* first five */, pub stopped: bool }
  #[derive(Debug, Clone, Copy, Default, serde::Serialize)]
  pub struct ExtractProgress { pub done: usize, pub total: usize, pub bytes_done: u64, pub bytes_total: u64 }
  /// Extracts the entries with `skip == None` (the listing's media) into `dest`, below their safe names.
  pub fn extract_zip(zip: &Path, listing: &ZipListing, dest: &Path, stop: &AtomicBool, progress: impl FnMut(&ExtractProgress)) -> Result<ExtractReport>;
  ```

- [ ] **Step 1: Write the failing tests** (with Task 1's `make_zip`, a `tempfile` destination):
  - `entries_land_below_the_destination_with_their_folders_and_dates`: `DCIM/a.jpg` and `b.jpg` are extracted to `dest/DCIM/a.jpg` and `dest/b.jpg`, bytes equal, and each file's modified time equals the entry's `mtime`; nothing else is created in `dest` (no `.part` files).
  - `nothing_is_written_outside_the_destination`: a zip with `../../evil.jpg`, `/abs.jpg`, `C:/x.jpg` plus one good file: only the good file exists, and the parent of `dest` has no new file (check by listing it before and after); the unsafe ones are not in `extracted` and are in the listing as `Skip::Unsafe`.
  - `an_existing_file_is_never_overwritten`: `dest/a.jpg` already holds other bytes: the entry is extracted as `a-1.jpg` and the original is untouched.
  - `a_wrong_crc_removes_the_file_and_is_a_problem_and_the_rest_go_on`: corrupt one entry's data in the zip file's bytes (flip a byte inside its stored data), extract: `failed == 1`, that file is not in `dest`, the others are, `problems[0]` names it.
  - `a_truncated_zip_is_an_error_when_listing_and_never_partly_extracts_a_bad_listing`: truncating the file makes `list_zip` return `Err` (the central directory is at the end).
  - `an_entry_larger_than_its_header_claimed_stops_at_the_claimed_size`: use a listing whose `size` for one entry is smaller than the real data (edit the listing, not the zip): extraction of that entry fails with a plain problem and leaves no file.
  - `stopping_leaves_nothing_behind_and_says_so`: set `stop` inside the first progress callback of a big entry (3 MiB): `stopped == true`, `dest` has no file for that entry and no `.part`.
  - `progress_counts_entries_and_bytes_and_ends_complete`.
- [ ] **Step 2: Run** `cargo test -p pixie-index zipsrc`. Expected: FAIL.
- [ ] **Step 3: Implement** `extract_zip`: for each entry to extract, `by_index(entry.index)`; create parent folders under `dest` (verify the final path `starts_with(dest)` after joining, as a second line of defence); write to `.pixie-extract-{pid}.part` in the entry's folder through a 1 MiB buffer, counting bytes (stop with a problem if the count passes `entry.size`), hashing CRC32 with the `crc32fast` crate already pulled in by `zip` (re-export it, or compute with `zip`'s own check: the `ZipFile` reader returns an error on a CRC mismatch at the end of the read: use that and map the error to "damaged"); set the modified time (`File::set_modified`) from `entry.mtime`; put it in place with a numbered name when taken (`copier`'s `numbered` logic: make it `pub(crate)` and reuse it); remove the `.part` on every non-success path; check `stop` between chunks.
- [ ] **Step 4: Run** `cargo test -p pixie-index`. Expected: PASS.
- [ ] **Step 5: Leave uncommitted.**

### Task 3: The Inbox: folder, space, Keep that moves, Discard

**Files:**
- Create: `crates/pixie-index/src/inbox.rs`
- Modify: `crates/pixie-index/src/keep.rs`, `crates/pixie-index/src/lib.rs`
- Test: `crates/pixie-index/src/inbox.rs`, `crates/pixie-index/src/keep.rs`

**Interfaces:**
- Consumes: `add_source`, `remove_source`, `list_sources` (sources); `run_keep` (Keep); `extract_zip` (Task 2).
- Produces:
  ```rust
  /// `<data>/inbox`, made if it is missing.
  pub fn inbox_root(data: &Path) -> std::io::Result<PathBuf>;
  /// A new empty folder for one zip: `inbox/<n>-<safe stem>` (the first free number).
  pub fn new_inbox_dir(root: &Path, label: &str) -> std::io::Result<PathBuf>;
  /// The identity of a zip: the same file again gives the same key, a changed one a different key.
  pub fn zip_key(zip: &Path) -> std::io::Result<String>;          // "zip:<size>:<mtime ms>:<file name lower>"
  /// Bytes free on the volume holding `path`.
  pub fn free_space(path: &Path) -> std::io::Result<u64>;
  pub fn is_in_inbox(root: &Path, path: &str) -> bool;
  /// Deletes what is left of an Inbox source: its files and folder, then the source (rows, picks). Refuses a source that is not in the Inbox.
  pub fn discard_source(db: &Db, root: &Path, source: FolderId) -> Result<Vec<PhotoId>>;
  /// Folders in the Inbox that no source points at (a crash, or a discard that was cut short): removes them; returns how many.
  pub fn remove_leftovers(db: &Db, root: &Path) -> std::io::Result<usize>;
  ```
  and in `keep.rs`: `pub enum Mode { Copy, Move }` and `run_keep(.., mode: Mode, ..)`; `Move` removes the source file and its row after the copy is verified (or the file was already there).

- [ ] **Step 1: Write the failing tests:**
  - `the_inbox_root_is_made_and_each_zip_gets_its_own_numbered_folder`: `new_inbox_dir` twice for `Holiday 2024.zip` gives `1-Holiday 2024` and `2-Holiday 2024`, characters that a file system cannot hold are replaced.
  - `a_zips_key_is_the_same_for_the_same_file_and_different_when_it_changes`: write, key, write the same bytes at another path with the same name and mtime → equal; change the size → different.
  - `free_space_is_a_sensible_number`: for a temp dir, `free_space > 0` and `< u64::MAX`.
  - `is_in_inbox_only_for_paths_below_the_root`, including a sibling `inbox2` and `..` tricks.
  - `discarding_an_inbox_source_deletes_its_files_rows_and_picks`: an Inbox source with two scanned photos (a temp root, files created, `add_source`, `scan_source`), one picked; `discard_source`: the folder is gone from disk, `list_sources` is empty, no picks; a library photo is untouched; `discard_source` of a source outside the Inbox (a card) is `Err` and deletes nothing.
  - `leftover_inbox_folders_with_no_source_are_removed`: three folders in the root, one with a source: two removed, one kept.
  - in `keep.rs`: `moving_keeps_into_the_library_folder_and_removes_the_inbox_file_and_row`: after `run_keep(.., Mode::Move, ..)` the file is in the library folder, gone from the source folder, the photo row is gone, `list_sources()[0].count` went down, and the import log has the item; and `a_move_of_a_file_already_there_still_removes_the_inbox_copy`; and `copy_mode_leaves_the_files_as_before` (the existing tests pass with `Mode::Copy`).
- [ ] **Step 2: Run** `cargo test -p pixie-index inbox keep`. Expected: FAIL.
- [ ] **Step 3: Implement.** `free_space`: on Windows `GetDiskFreeSpaceExW` through the `windows` crate already in the tree for `src-tauri` is not available to `pixie-index`; use a tiny `extern "system"` declaration like `drives.rs` does (`GetDiskFreeSpaceExW(dir: *const u16, free_to_caller: *mut u64, total: *mut u64, total_free: *mut u64) -> i32`), on unix `libc::statvfs` (`f_bavail * f_frsize`), `libc` is already a unix dependency of `src-tauri`: add it to `pixie-index` under `[target.'cfg(unix)'.dependencies]`. `discard_source` checks `is_in_inbox` on the source's path, removes the directory with `remove_dir_all`, then `remove_source`. `remove_leftovers` reads the root's directories and compares each with `list_sources()` paths (plain comparison from `source.rs`).
- [ ] **Step 4: Run** `cargo test -p pixie-index`. Expected: PASS.
- [ ] **Step 5: Leave uncommitted.**

### Task 4: Commands and events

**Files:**
- Modify: `src-tauri/src/importer.rs`, `src-tauri/src/lib.rs`
- Test: `cargo check --workspace --all-targets` clean, plus the live check in Task 6 (these commands need an `AppHandle`)

**Interfaces:**
- Consumes: Tasks 1 to 3; `import_open`'s `scan` helper; `data_dir` in `lib.rs`.
- Produces (Tauri commands, all in `importer.rs`):
  ```rust
  #[derive(Serialize)] pub struct ZipLook { pub name: String, pub photos: usize, pub videos: usize, pub other: usize, pub archives: usize, pub encrypted: usize, pub unsafe_names: usize, pub bytes: u64, pub free: u64, pub fits: bool, pub problems: Vec<String> }
  #[tauri::command] pub async fn import_zip_look(app: AppHandle, path: String) -> Result<ZipLook, PixieError>;   // reads the directory only; free is the space left where the Inbox is
  #[tauri::command] pub async fn import_zip_extract(app: AppHandle, path: String) -> Result<i64, PixieError>;     // makes the Inbox folder, the source (key from `zip_key`, label the zip's name), starts the extraction; returns the source id
  #[tauri::command] pub fn import_discard(app: AppHandle, id: i64) -> Result<(), ...>;                             // Inbox sources only; deletes the files
  ```
  Events: `import-extract` `{ source, done, total, bytes_done, bytes_total }` and `import-extract-done` `{ source, error: Option<String>, report: Option<ExtractReport> }`; when extraction ends cleanly the light scan runs as for any source (`import-scan`, `import-scan-done`). `OpenSource` gains `inbox: bool` and `extracting: bool`. `import_keep` takes `Mode::Move` when the source is in the Inbox. `import_stop` also stops an extraction. `import_close` (Done) refuses an Inbox source with a sentence ("Use Discard to remove what is left in the Inbox").
  Start-up: after `prune_sources`, `inbox::remove_leftovers` (and a source whose zip extraction was cut short stays browsable).

- [ ] **Step 1: Implement** the commands. `import_zip_extract` refuses when the listing does not fit (`bytes > free`, with the numbers in the sentence), when a Keep or another extraction is running (one job at a time, reuse `ImportState.job`'s guard or add `extracting: Mutex<Option<i64>>`), and when the same zip is already open (same key: return its id, saying so through `Err(Invalid("That zip is already open below."))` is wrong UX, so return the existing id and let the UI show it). Run `extract_zip` on a thread, emit progress at most every 200 ms, then call the existing `scan(app, id)`.
- [ ] **Step 2: Register** the commands in `lib.rs`; `OpenSource.inbox` is `inbox::is_in_inbox(root, &s.path)`.
- [ ] **Step 3: Run** `cargo check --workspace --all-targets`. Expected: no warnings; and `cargo test -p pixie-app`. Expected: PASS.
- [ ] **Step 4: Leave uncommitted.**

### Task 5: The UI

**Files:**
- Modify: `ui/src/lib/api.ts`, `ui/src/lib/importStore.svelte.ts`, `ui/src/lib/importer.ts`, `ui/src/lib/ImportTool.svelte`, `ui/src/lib/Nav.svelte`, `ui/src/lib/askLocal.ts`, `ui/src/lib/askGuide.ts`
- Test: `ui/src/lib/importer.test.ts`

**Interfaces:**
- Consumes: the commands and events of Task 4; `open` from `@tauri-apps/plugin-dialog` (file mode, filter `zip`).
- Produces in `importer.ts` (pure, tested): 
  ```ts
  export function zipLine(z: ZipLook): string;        // "412 photos and 18 videos, 6.1 GB. 3 other files will be left out."
  export function spaceLine(z: ZipLook): string;      // "Needs 6.1 GB; D: has 41 GB free." or the refusal: "Needs 6.1 GB but only 2.0 GB is free."
  export function zipProblems(z: ZipLook): string[];  // "2 entries have unsafe names and will be skipped.", "1 is encrypted and will be skipped." ...
  export function extractLine(done: number, total: number): string;
  ```
  and the wording rule for an Inbox source: **Keep** moves (the button title says "Moves the photos out of the Inbox into the folder"), **Discard what is left** (red, `class="danger"`) replaces "Done with this card" and asks for confirmation (use the app's `ConfirmDialog.svelte`).

- [ ] **Step 1: Write the failing tests** for the four functions in `importer.test.ts`: the summary sentence for photos only, videos only, both, with other files, with nothing to extract ("There are no photos or videos in this zip."); the space line when it fits and when it does not (binary units, as `formatBytes`); the problem sentences (singular and plural, none); the extract line.
- [ ] **Step 2: Run** `cd ui && npx vitest run src/lib/importer.test.ts`. Expected: FAIL.
- [ ] **Step 3: Implement** the functions; `api.ts` types and wrappers (`ZipLook`, `importZipLook`, `importZipExtract`, `importDiscard`, `onImportExtract`, `onImportExtractDone`; `OpenSource` gets `inbox` and `extracting`); the store keeps `extract` progress and refreshes on `import-extract-done`.
- [ ] **Step 4: `ImportTool.svelte`:** next to **A folder…** add **A zip file…** (title: "Choose a zip file of photos and videos to look at"). Choosing one calls `importZipLook` and shows a panel: the name, `zipLine`, `spaceLine`, the problem lines, and one primary button **Extract into the Inbox** (disabled when it does not fit, with the reason in its title) and **Cancel**. After extraction starts it becomes a normal source card with a progress bar while `extracting` (Stop button), then the usual Look summary, Open in Browse, Keep. For an Inbox source: the label "Inbox" tag beside the name, and **Discard what is left** (danger, with a confirmation naming the count). Nav's section title becomes "Cards, drives and zips" and an Inbox source shows a small "zip" mark (`title`: "Extracted from a zip file").
- [ ] **Step 5: Help text:** extend the Import topic in `askLocal.ts` (`goTo` stays `transfer-import`) and the Import line in `askGuide.ts` with zip files; the existing tests that require a topic per tool must still pass.
- [ ] **Step 6: Run** `npx vitest run`, `npx svelte-check --tsconfig ./tsconfig.json`. Expected: all pass, 0 errors, 0 warnings.
- [ ] **Step 7: Leave uncommitted.**

### Task 6: Verify in the real app, then fix what it shows

**Files:** none planned; fixes go where the bug is.

- [ ] **Step 1: Build my own instance** (never rebuild into the user's `target`): `CARGO_TARGET_DIR=target-claude cargo build -p pixie-app --no-default-features` (no vcpkg here), then launch `target-claude/debug/pixie-app.exe` with `PIXIE_DATA_DIR`, `WEBVIEW2_USER_DATA_FOLDER` and `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9333` pointing at a throwaway folder under the scratchpad (so it shares nothing with the user's own session). It loads the UI from the dev server on port 1420: use the user's if it is up, otherwise start `npm run dev` in `ui/` and stop it afterwards.
- [ ] **Step 2: Drive it** with a Node script over the DevTools Protocol (`fetch http://127.0.0.1:9333/json`, a `WebSocket` to the page, `Runtime.evaluate`, `Page.captureScreenshot`; Node 24 has `WebSocket` built in); the earlier scripts of this session in the scratchpad (`e2e/drive.mjs`) show the helpers (`click(label, scope)`, `until`, `invoke`). Case-insensitive text checks: CSS upper-cases some headings. The native file dialog cannot be driven, so call `import_zip_look` and `import_zip_extract` the way the button would.
- [ ] **Step 3: The flow to prove, with a screenshot of each:** make a zip with Python (`zipfile`) of six generated JPEGs (PIL, with EXIF dates) plus a text file, a nested zip and an entry named `../../evil.jpg`; Look (counts, space, the unsafe-name problem line); Extract (progress, then the source appears in Browse's tree with its photos and thumbnails); `lib` and the data folder: nothing outside the Inbox was written (list the parent folders before and after); Keep new items (the files arrive in the library folder filed by date, and are **gone** from the Inbox folder); Keep picked; Discard what is left (its folder is gone from disk); the same zip a second time (it is recognised: nothing new); a zip larger than the free space is refused with the numbers (use a tiny `PIXIE`-free temp volume or a lowered limit through a debug-only environment variable `PIXIE_INBOX_FREE_LIMIT` read in `free_space`'s caller, removed again if it is not needed); kill the app mid-extraction and start it again: the leftover folder is cleaned or the source is browsable.
- [ ] **Step 4: Stop** my instance and my dev server, and report what was seen: counts, file lists, and anything that looked wrong, even if it is small.
- [ ] **Step 5: Final checks:** `cargo check --workspace --all-targets` (no warnings), `cargo test -p pixie-index -p pixie-analysis -p pixie-app`, `npx vitest run`, `svelte-check`. If the Mac is reachable, copy the changed files over (`scp`) and `cargo check --workspace --all-targets` there (`ssh mac`, repo in `~/work/pixie`; the changed files are listed by `git status`); `git checkout -- .` there before the next `git pull`.
- [ ] **Step 6: Leave everything uncommitted** and tell the user what changed and what is still not built (Private and the Private photos folder, Delete from the card, Import on the Processing screen, the Inbox size setting and location, phones).

## Verification

A feature here is only done when step 3 of Task 6 has been run in the live app and its screenshots and file listings are in the report. Unit tests prove the pieces; they did not catch the filter chip saying "unknown", the stale picked count, or a misleading banner, all of which the live run showed.

## Self-review

- **Spec coverage:** the zip flow (Look, extract, browse, keep, discard) is Tasks 1 to 5; the safe-name, bomb, encrypted, nested, collision and date rules are Tasks 1 and 2 and their tests; free space and leftover cleanup are Task 3; the Inbox as the first step of phase 2 is the architecture. Left out on purpose and recorded in #127: other archive types, passwords, multi-part archives, Takeout. Left for later: the Inbox's location and size settings (it is `<data>/inbox`), the Inbox on the Processing screen, held-for-review arrivals and phones.
- **Types:** `ZipEntry`, `ZipListing`, `Skip`, `ExtractReport`, `ExtractProgress`, `Mode`, `ZipLook` are each defined once and used unchanged; `OpenSource.inbox`/`extracting` are added in Task 4 and consumed in Task 5.
- **Proportion:** six tasks, each a verifiable deliverable; the last is the live check the previous feature showed is needed.
