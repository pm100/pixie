# Tags, Picked and Tools Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Flat user tags on photos, Quick pick shown as an always-present **Picked** tag in the browse tree, a Details panel where tags are edited (single photo and picked group), and a **Tools** tab that replaces the Selected tab.

**Architecture:** User tags live in a new `user_tags` table (independent of the existing Auto tags in `photo_tags`). Choosing Picked or a tag in the tree is one more filter, wired like the existing tag/person/camera filters (a temp hit table intersected into `search_hits`, plus a clause in the tree's facet counts). The UI gets pure helpers (unit tested), a `TagAs` drop-down used by Details and Tools, and `SelectedTab` becomes `ToolsTab`.

**Tech Stack:** Rust (rusqlite, Tauri 2 commands), Svelte 5 + TypeScript, vitest.

**Spec:** `docs/superpowers/specs/` has none; the design is the section "Design (agreed in the design round, 2026-10-27)" of GitHub issue #24 (`gh issue view 24`), with the picking method in #60. Read it first.

## Global Constraints

- Never commit or push unless the user says so (AGENTS.md). The "Commit" steps below are for when they have.
- Builds stay warning-free: `cargo check --workspace --all-targets` and `npx svelte-check` (AGENTS.md). Run `just notices` only if dependencies change (none planned).
- Run Claude's cargo commands with `CARGO_TARGET_DIR=C:/work/pixie-claude-target VCPKG_ROOT=C:/work/vcpkg-pixie VCPKGRS_TRIPLET=x64-windows VCPKGRS_DYNAMIC=1` and `C:/work/vcpkg-pixie/installed/x64-windows/bin` on `PATH`.
- Only pluralise when a count is more than one (`plural` in `ui/src/lib/format.ts`).
- Tags are **flat**; names are case-insensitive and unique. A tag exists only while a photo has it.
- Thumbnails look as today: no tag or people pills on them. Tags and people are shown and edited only in the Details panel.
- Picked is the existing `picks` table (it already survives a restart); it is a tag only in the UI and in the filter.
- The tags are in the library database only; originals are never changed.
- Tools tab: only tools that exist today are listed (Email, Tag as…, Save as tag…). Collage, Print, Effects, Copy / Move, Share are added by their own issues (#12, #11, #9, #13); the page follows the existing rule "a feature that has not shipped is not shown".

## Not in this plan

Renaming, merging or deleting a tag (#24); editing people in Details (#62); Hide / hidden (#71); keyboard use (#67); writing tags to files; the other tools above.

## Review Focus

Each line has its test in the task named.

1. Tag names that differ only by case, spaces or a leading `#` are one tag, and the first spelling used is kept ("holiday" onto an existing "Holiday") — Tasks 1, 4.
2. Tagging ids that are not in the library (removed, stale in the UI) does nothing and does not fail the rest — Task 1.
3. A tag or Picked that is the active filter while photos are tagged, untagged, picked or cleared: the listing and counts follow at once, not on the next click — Task 2.
4. Taking the last photo off the active tag removes the tag and clears the filter, so no empty view with a dangling chip — Tasks 2, 5.
5. A very large selection (10,000 ids) is tagged in one transaction and one call — Task 1.

---

### Task 1: User tags in the database

**Files:**
- Modify: `crates/pixie-index/src/db.rs` (add migration V20 to `MIGRATIONS`, after V19 `settings`)
- Create: `crates/pixie-index/src/utags.rs` (`impl Db`), register with `mod utags;` in `crates/pixie-index/src/lib.rs`
- Modify: `crates/pixie-core/src/lib.rs` (types and `normalize_tag`)
- Test: in `utags.rs` (`#[cfg(test)]`) and `pixie-core`

**Interfaces:**
- Produces (pixie-core): `pub fn normalize_tag(raw: &str) -> Option<String>` (trim, collapse inner whitespace to single spaces, drop one leading `#`, trim again; `None` if empty or longer than 60 chars); `pub struct UserTagCount { pub name: String, pub count: i64 }`; `pub struct TagUse { pub name: String, pub on: i64 }` (both `Serialize`, `Debug`, `Clone`, `PartialEq`).
- Produces (`Db`): `tag_photos(&self, ids: &[PhotoId], tag: &str) -> Result<String>` (canonical name; error `PixieError::Invalid` (the user-fixable variant) for a name `normalize_tag` rejects); `untag_photos(&self, ids: &[PhotoId], tag: &str) -> Result<()>`; `user_tags(&self) -> Result<Vec<UserTagCount>>` (visible photos only: `missing = 0 AND dup = 0`; tags with 0 visible photos left out; sorted by name, case-insensitive); `tag_usage(&self, ids: &[PhotoId]) -> Result<Vec<TagUse>>` (every tag in the library with how many of `ids` have it; same sort); `list_tagged(&self, tag: &str) -> Result<Vec<Photo>>` (newest first, like `list_picked`).

- [ ] **Step 1: Write failing tests** in `utags.rs` using `Db::open_in_memory()` with two or three `upsert_photo` rows:
  - `tag_names_ignore_case_spaces_and_hash`: `normalize_tag("  #Holiday  2024 ")` is `Some("Holiday 2024")`; `normalize_tag("  # ")` and a 61-character name are `None`; in `pixie-core`.
  - `the_first_spelling_is_kept`: `tag_photos(&[a], "Holiday")` then `tag_photos(&[b], "holiday")` returns `"Holiday"` both times and `user_tags()` is `[{Holiday, 2}]`.
  - `ids_that_are_not_in_the_library_are_ignored`: `tag_photos(&[a, 9999], "x")` is `Ok`, only `a` is tagged.
  - `a_big_selection_is_one_call`: 10,000 ids (most unknown) finishes (assert only that it returns `Ok` and counts the real ones).
  - `counts_leave_out_missing_and_duplicate_photos`: mark one tagged photo `missing`, another `dup`; `user_tags()` counts only the visible one; a tag whose photos are all hidden is absent.
  - `usage_says_how_many_of_the_given_photos_have_each_tag`: `tag_usage(&[a,b])` gives `on: 2` for a tag on both, `on: 1` for one on `a`, `on: 0` for a tag only on `c`.
  - `untag_removes_and_the_last_removal_removes_the_tag`.
- [ ] **Step 2: Run** `cargo test -p pixie-core -p pixie-index utags` — expect FAIL (missing items).
- [ ] **Step 3: Implement.** Migration V20: `CREATE TABLE user_tags (photo_id INTEGER NOT NULL REFERENCES photos(id) ON DELETE CASCADE, tag TEXT NOT NULL COLLATE NOCASE, PRIMARY KEY (photo_id, tag)); CREATE INDEX idx_user_tags_tag ON user_tags(tag);` Canonical spelling: look up an existing row `WHERE tag = ?1` (NOCASE) and use its stored spelling, else the normalised input. Insert with `INSERT OR IGNORE ... SELECT id, ?2 FROM photos WHERE id = ?1` inside one transaction. Update the migration tests that count `MIGRATIONS` if any assert the version.
- [ ] **Step 4: Run** the same command — expect PASS; then `cargo check --workspace --all-targets` — no warnings.
- [ ] **Step 5: Commit** `feat: user tags in the library database`

### Task 2: Picked and tags as a filter

**Files:**
- Modify: `crates/pixie-index/src/db.rs` (temp table, `refresh_hits`, `active_filters`, `FacetBase`/`FacetAsk`, the temp-table reset list near line 1107, `set_pick`/`set_picks`/`clear_picks`)
- Modify: `crates/pixie-index/src/utags.rs` (the filter), `crates/pixie-core/src/lib.rs` (type)
- Test: `crates/pixie-index/src/utags.rs`

**Interfaces:**
- Consumes: Task 1 (`user_tags` table, `tag_photos`, `untag_photos`).
- Produces: `pub enum UserTagFilter { Picked, Tag(String) }` (pixie-core, with `fn parse(s: &str) -> Option<UserTagFilter>` and `fn key(&self) -> String`: `"picked"` or `"tag:<name>"`); `Db::set_user_tag_filter(&self, f: Option<&UserTagFilter>) -> Result<()>`; `Db::active_filters` returns a struct `ActiveFilters { tag: Option<String>, person: Option<i64>, camera: Option<String>, utag: Option<String> }` (replace the tuple; update `src-tauri/src/lib.rs` `active_filters` and `ActiveFilters` JSON to carry `utag`); `FacetAsk` gains `pub utag: Option<&'a UserTagFilter>`.

- [ ] **Step 1: Write failing tests:**
  - `choosing_picked_lists_only_picked_photos`: `set_user_tag_filter(Some(&Picked))`, `list_photos(&PhotoQuery{search:true,..})` is the picked ones; `set_user_tag_filter(None)` brings all back.
  - `the_filter_follows_picks_and_tags_at_once`: with `Picked` active, `set_pick(c,true)` then the listing includes `c`; `clear_picks()` empties it; with `Tag("x")` active, `tag_photos`/`untag_photos` change the listing immediately.
  - `it_combines_with_the_other_filters`: user-tag filter plus `set_camera_filter` gives the intersection.
  - `the_last_photo_off_the_active_tag_clears_the_filter`: after `untag_photos` removes the only photo of the active `Tag`, `active_filters().utag` is `None` and the listing is the unfiltered one.
  - `facet_counts_narrow_to_the_chosen_tag`: `FacetAsk{utag: Some(&Tag("x")),..}` counts only tagged photos in the other sections.
- [ ] **Step 2: Run** `cargo test -p pixie-index utags` — expect FAIL.
- [ ] **Step 3: Implement.** Add `CREATE TEMP TABLE IF NOT EXISTS utag_hits (photo_id INTEGER PRIMARY KEY)` beside the other temp tables, add `("utag", "utag_hits")` to the loop in `refresh_hits`, and `utag_hits` to the reset list. `set_user_tag_filter` fills `utag_hits` (`SELECT photo_id FROM picks` or `SELECT photo_id FROM user_tags WHERE tag = ?`) and writes `filter_state('utag', key)`. A private `refresh_utag(&tx)` re-runs that fill from `filter_state` and then `refresh_hits`; call it at the end of `set_pick`, `set_picks`, `clear_picks`, `tag_photos`, `untag_photos` (those that use `&self.conn()` become transactions). If the active filter is `Tag(name)` and no photo has it any more, delete the `utag` filter state first. `FacetBase::new` gets one more argument, `utag: Option<&UserTagFilter>`, with the same subselects as above in its SQL and `:utag` in `params()`; pass `ask.utag` at all its call sites.
- [ ] **Step 4: Run** `cargo test -p pixie-index` — all PASS; `cargo check --workspace --all-targets` — no warnings.
- [ ] **Step 5: Commit** `feat: Picked and user tags as a browse filter`

### Task 3: Tauri commands and the TypeScript API

**Files:**
- Modify: `src-tauri/src/lib.rs` (commands, `facet_counts`, `active_filters`, `generate_handler!`), `ui/src/lib/api.ts`
- Test: `ui/src/lib/api.test.ts` is not used in this repo; the commands are one-line wrappers over Task 1 and 2 (covered there). The check is the build.

**Interfaces:**
- Consumes: Tasks 1 and 2.
- Produces (Rust commands): `tag_photos(ids: Vec<i64>, tag: String) -> Result<String, PixieError>`, `untag_photos(ids: Vec<i64>, tag: String)`, `user_tags() -> Vec<UserTagCount>`, `tag_usage(ids: Vec<i64>) -> Vec<TagUse>`, `list_tagged(tag: String) -> Vec<Photo>`, `set_user_tag_filter(filter: Option<String>)` (the `key()` string; unknown strings are an error); `facet_counts` takes an extra `utag: Option<String>`; `active_filters` returns `utag`.
- Produces (api.ts): `tagPhotos(ids: number[], tag: string): Promise<string>`, `untagPhotos(ids, tag): Promise<void>`, `userTags(): Promise<UserTagCount[]>`, `tagUsage(ids): Promise<TagUse[]>`, `listTagged(tag): Promise<Photo[]>`, `setUserTagFilter(key: string | null): Promise<void>`, types `UserTagCount = { name: string; count: number }`, `TagUse = { name: string; on: number }`; `facetCounts(...)` gets a `utag: string | null` argument; `activeFilters()` result gains `utag: string | null`.

- [ ] **Step 1: Implement** the commands (large id lists run on `spawn_blocking`, like `facet_counts`), register them, and add the api.ts functions. Pass `utag` through `facetCounts` in `App.svelte` as `null` for now.
- [ ] **Step 2: Verify** `cargo check --workspace --all-targets` (no warnings), `npx svelte-check` (0 errors).
- [ ] **Step 3: Commit** `feat: commands for user tags and the Picked filter`

### Task 4: Pure UI helpers

**Files:**
- Create: `ui/src/lib/userTags.ts`, `ui/src/lib/userTags.test.ts`

**Interfaces:**
- Produces: `normalizeTag(raw: string): string | null` (same rules as Rust); `type TagAsRow = { name: string; on: number; all: boolean }`; `tagAsRows(usage: TagUse[], count: number, query: string, limit = 6): { rows: TagAsRow[]; more: number; create: string | null }` (rows: tags whose name contains the query, case-insensitive, in `usage` order, first `limit`; `more` = matches beyond `limit`; `all` = `on === count && count > 0`; `create` = the normalised query when it is non-empty and no tag has that name ignoring case); `tagsOnAll(usage: TagUse[], count: number): string[]` (names with `on === count`, `count > 0`); `filterKey(f: { picked: true } | { tag: string }): string` (`"picked"` / `"tag:<name>"`) and `parseFilterKey(key: string | null): { picked: true } | { tag: string } | null`.

- [ ] **Step 1: Write failing tests:** `normalizeTag('  #Holiday  2024 ')` is `'Holiday 2024'`; `' # '` and 61 chars are `null`. `tagAsRows` with 9 tags and query `''` gives 6 rows and `more: 3`; query `'ho'` matches case-insensitively; a tag on every counted photo has `all: true`; `create` is `null` when the query equals an existing tag in another case, is the normalised text otherwise, and `null` for an empty query. `tagsOnAll(usage, 0)` is `[]`. `parseFilterKey(filterKey(x))` round-trips for both kinds, `parseFilterKey('nonsense')` is `null`.
- [ ] **Step 2: Run** `npx vitest run src/lib/userTags.test.ts` (from `ui/`) — FAIL.
- [ ] **Step 3: Implement** the functions in `userTags.ts` to the signatures above.
- [ ] **Step 4: Run** the test — PASS.
- [ ] **Step 5: Commit** `feat: helpers for tag names, the Tag as list and the filter key`

### Task 5: The browse tree: Picked and your tags

**Files:**
- Modify: `ui/src/lib/Nav.svelte` (a **Tags** section above Auto tags), `ui/src/App.svelte` (state, filter, chips, empty message, "Show all"), `ui/src/lib/tagNav.ts` (`hitsActive` takes the user-tag filter too), `ui/src/lib/tagNav.test.ts`
- Test: `ui/src/lib/tagNav.test.ts`

**Interfaces:**
- Consumes: Tasks 3 and 4.
- Produces: Nav props `userTags: UserTagCount[]`, `pickedCount: number`, `utag: string | null` (the active `filterKey`), `onutag: (key: string | null) => void`; `hitsActive(tag, person, camera, utag)` with `utag` a fourth optional argument.

- [ ] **Step 1: Write failing tests** in `tagNav.test.ts`: `hitsActive(null, null, null, 'picked')` and `hitsActive(null, null, null, 'tag:x')` are `true`; with all four `null` it is `false`; the existing three-argument calls still pass.
- [ ] **Step 2: Run** `npx vitest run src/lib/tagNav.test.ts` — FAIL; then implement and PASS.
- [ ] **Step 3: Tree.** In `Nav.svelte` add a section "Tags": a row **★ Picked (n)** first, a divider, then each user tag with its count (a row of the same kind as the Auto tags rows, current one highlighted, click again to clear), no "+ New tag" row (tags are made with Tag as…). Show Picked even when `pickedCount` is 0 (dimmed, as entries that come to 0 are elsewhere in this tree).
- [ ] **Step 4: App wiring.** In `App.svelte`: `let userTags = $state<UserTagCount[]>([])`, `let activeUtag = $state<string | null>(null)`; `refreshUserTags()` reads `userTags()`; `setUtag(key)` calls `setUserTagFilter`, sets `activeUtag`, and sets `view.search = hitsActive(activeTag, activePerson, activeCamera, activeUtag)` exactly as `setTag` does for the Auto tag; the `activeFilters()` sync (line ~176) also takes `utag`, so a filter the backend dropped (the last photo untagged) clears here too; a chip "★ Picked" / "Tagged “X”" with a clear button next to the existing tag chip; "Show all" also calls `setUserTagFilter(null)`; the empty message for an active user tag is "No photos with this tag here." / "Nothing is picked."; pass `utag` to `facetCounts` and re-read the tree counts when it changes. Re-read `refreshUserTags()` and the listing when picks or tags change (after Task 6/7 actions, and in `togglePick`).
- [ ] **Step 5: Verify** `npx vitest run` (all pass), `npx svelte-check` (0 errors); run the app (`just winlaunch`): pick two photos, click ★ Picked in the tree — only those show; click again — all return.
- [ ] **Step 6: Commit** `feat: Picked and your tags in the browse tree`

### Task 6: Tag as… and the Details panel

**Files:**
- Create: `ui/src/lib/TagAs.svelte`
- Modify: `ui/src/lib/Details.svelte`, `ui/src/App.svelte` (props)
- Test: logic is in Task 4 (`tagAsRows`); this task is the component (checked by `svelte-check` and a run).

**Interfaces:**
- Consumes: Tasks 3, 4, 5 (`tagUsage`, `tagPhotos`, `untagPhotos`, `tagAsRows`, `tagsOnAll`).
- Produces: `TagAs` props `{ ids: number[]; onchange: () => void }` (a **Tag as…** button; opens a box with a text field and the list from `tagAsRows` — each row `# name` with "on all" / "on N" and disabled when `all`; a **+ Create tag "…"** row from `create`; Enter applies the single matching tag or creates; Esc and a click elsewhere close it; calls `tagPhotos(ids, name)` then `onchange()`). `Details` props gain `picks: ReadonlySet<number>`, `onpicksclear: () => void`, `ontagschange: () => void`.

- [ ] **Step 1: TagAs.svelte** to the interface above; load `tagUsage(ids)` when it opens and after each change; follow the design: six rows at most with "N more, keep typing".
- [ ] **Step 2: Details, one photo.** Under the existing facts add **Tags**: the photo's user tags as `# name` pills each with × (`untagPhotos([id], name)`), then `TagAs ids=[photo.id]`; Auto tags ("Shows") stay read-only and visually distinct as today. Thumbnails are not touched.
- [ ] **Step 3: Details, picked group.** When `picks.size > 0` and no single photo is shown, or when the user chose it: heading "N picked photos" (use `plural`) with a **Clear** link beside it (`onpicksclear`), the line "Changes here apply to all of them.", **Tags on all of them** (from `tagsOnAll(usage, picks.size)`, each with × that untags all picked), `TagAs ids=[...picks]`; no People list. When both a single photo and picks exist show a two-button switch **This photo | Picked (n)** (default: This photo). Under the panel, whenever anything is picked: "Use the **Tools tab** to work with these pics." where the words Tools tab switch to that tab (`ontools` prop). This replaces the current "N photos selected" hint.
- [ ] **Step 4: App.** Pass `picks`, `onpicksclear` (`clearAllPicks`), `ontagschange` (`refreshUserTags` + re-list if a user-tag filter is active), `ontools`.
- [ ] **Step 5: Verify** `npx svelte-check` (0 errors), `npx vitest run`; run the app: tag one photo "Holiday" (new), tag a second with "holiday" (matches), pick both, group view shows "Holiday" under Tags on all of them, × removes it from both and it disappears from the tree.
- [ ] **Step 6: Commit** `feat: Tag as… and tags in the Details panel`

### Task 7: The Tools tab

**Files:**
- Rename: `ui/src/lib/SelectedTab.svelte` → `ui/src/lib/ToolsTab.svelte`
- Modify: `ui/src/lib/tabs.ts`, `ui/src/lib/tabs.test.ts`, `ui/src/App.svelte`, `ui/src/lib/Nav.svelte` copy that says "Selected" if any (`grep -rn "Selected" ui/src`)
- Test: `ui/src/lib/tabs.test.ts`

**Interfaces:**
- Consumes: Tasks 3, 4, 6 (`listTagged`, `listPicked`, `TagAs`, `tagPhotos`).
- Produces: `TabId` `'selected'` becomes `'tools'`; `visibleTabs` labels it `Tools` / `Tools (n)` (n = picked count, as `Selected (n)` today). `ToolsTab` props `{ active; picks: ReadonlySet<number>; onunpick; onclear; emailLimitMb; userTags: UserTagCount[]; onchange: () => void }`.

- [ ] **Step 1: Update tests first** in `tabs.test.ts`: the second tab is `{ id: 'tools', label: 'Tools' }`, and `Tools (3)` with 3 picked; no `selected` id remains. Run — FAIL; change `tabs.ts` — PASS.
- [ ] **Step 2: ToolsTab.** Header: label **Working with** and a `<select>` of "★ Picked (n)" plus each user tag with its count (default Picked), the photo count, and **Clear picks** at the right when Picked is chosen and not empty. Photos: tiles as in `SelectedTab` today (viewer opens on double-click; × to unpick only for Picked); source is `listPicked()` for Picked and `listTagged(name)` otherwise, re-read when picks or tags change. Empty states: Picked — the existing "Nothing selected yet…" text reworded to "Nothing is picked…"; a tag with no photos — "No photos have this tag yet." Right column titled **Tools**: `EmailButton` (ids of the shown photos), **Tag as…** (`TagAs` on the shown ids), and for Picked only **Save as tag…** (a text field; Enter calls `tagPhotos(ids, name)` and then `onchange`; Picked is left as it is). Nothing else is listed (see Global Constraints).
- [ ] **Step 3: App.** Replace the `selected` tab wiring with `tools`; the Details footer link and any "Selected" text (`grep`) now say Tools; after any change call `refreshUserTags()`.
- [ ] **Step 4: Verify** `npx vitest run` (all pass), `npx svelte-check` (0 errors), `cargo check --workspace --all-targets` (no warnings). Run the app: pick 3 photos → Tools (3) → Email still opens its dialog → Save as tag "Trip" → the tree shows Trip (3) → choose Trip in "Working with" → the same photos.
- [ ] **Step 5: Commit** `feat: Tools tab replaces Selected`

### Task 8: Finish

**Files:** Modify `docs` only if wording changed; GitHub issues #24 and #60 (comment, not edit).

- [ ] **Step 1: Whole-repo checks:** `npx vitest run`, `npx svelte-check`, `cargo test --workspace`, `cargo check --workspace --all-targets` (zero warnings).
- [ ] **Step 2: Mac:** on the Mac (`ssh mac`, see `just mac-build` for the environment, plus `DYLD_LIBRARY_PATH=$HOME/work/vcpkg-pixie/installed/arm64-osx-dynamic/lib`), `git pull --ff-only`, `cargo check --workspace --all-targets` and `cargo test -p pixie-app --lib`.
- [ ] **Step 3:** Comment on #24 and #60 saying what shipped and the "Not in this plan" list; do not close them.
- [ ] **Step 4: Commit and push** only if the user says so.
