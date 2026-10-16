# Pixie: app shell and date tree (design)

Date: 2026-10-16. Builds on `2026-10-15-pixie-design.md` (Milestone 1 is done).

## Context and roadmap

The user wants Pixie to feel like a real photo manager rather than one grid of tiles. New feature set, agreed build order:

1. **App shell and date tree** (this spec)
2. **Map**: GPS from EXIF, clustered pins across all photos, drill in to see photos
3. **Email and contacts**: send selected photos to people by nickname (dad, simon)
4. **Windows background / lock screen slideshow** (needs research; the lock screen is restricted on Windows 11)

Pieces 2-4 each get their own design round. The Find milestone (search, tags, ratings, duplicates) stays separate and is not part of this spec.

## Goals

- Three-pane layout: tree (left), grid (middle), details (right).
- Browse by date through a Year > Month > Day tree with counts.
- Keep the grid fast on a library of 5,800+ photos (the user's real library).
- Lay the groundwork the later pieces plug into: a Map tree root, GPS stored per photo, a selection model, an Email button.

## Non-goals

Search, tags, ratings, duplicate finder, the map itself, email sending, wallpaper, editing.

## Layout

- Three resizable panes. Tree and details widths persist between runs.
- Left tree has three roots: **Dates**, **Folders**, **Map**. Map is a disabled placeholder until piece 2.
- Middle pane is the grid. Double-click (or Enter) opens the existing full-window viewer.
- Right pane is the details panel for the selected photo. With several selected it shows the count.

## Date tree

- Dates > Year > Month > Day, each node showing its photo count.
- Photos with no usable date go under an **Undated** node at the bottom.
- Newest first by default; a button flips the order. The choice persists.
- Source field is the existing `taken_at`. Known caveat from Milestone 1: EXIF local time is stored as if UTC, and the mtime fallback is real UTC. Grouping uses the stored value as-is; fixing that is out of scope.

## Grid

- One continuous scroll over the whole library, grouped under day headings such as "Sat 3 Jun 2024 · 14 photos".
- Clicking a tree node scrolls the grid to that day, month or year. The tree highlights the current position as the user scrolls.
- Selecting a **Folders** node filters the grid to that folder (headings stay).
- Each day group starts on a new row. Heading rows have a fixed height.
- Selection: click, ctrl-click, shift-click (range). Selection is by index within the current view (so shift-range works across pages that are not loaded) and is cleared whenever the sort order, folder filter or library changes. Later pieces (email, slideshow) resolve indexes to photo ids with a backend query.

## Details pane

Filename, date taken, dimensions, file size, folder, camera make/model, and GPS (when present). Includes an **Email...** button that does nothing until piece 3 (disabled with a tooltip).

## Backend changes

- New query `list_day_groups(order, folder_id?)` returning `[{ day: "YYYY-MM-DD" | null, count, first_index }]`. The tree and the grid headings are both built from it. `first_index` is the photo's position in the current ordering, so existing `list_photos(offset, limit)` paging and the thumbnail pipeline are reused unchanged.
- `list_photos` / `count_photos` accept the same `order` and optional `folder_id`.
- GPS: no schema change needed. `gps_lat` / `gps_lon` already exist (V1) and are filled by the scanner. This piece only tightens them: values must be in range (|lat| <= 90, |lon| <= 180), both present, and not exactly (0, 0) (a common camera "no fix" value); otherwise both are stored as null. A V2 migration applies the same rule to rows already stored.
- V2 migration also replaces the list index with two expression indexes (newest / oldest, `taken_at IS NULL` first) so paging stays index-ordered in both directions with Undated last.

## Frontend changes

- New components: `Shell` (panes, resizers), `Tree`, `Details`. `Grid` changes to take a layout with heading rows.
- Pure, unit-tested modules, in the style of the existing `gridMath`: a group-aware layout (row index to items or heading, scroll offset for a group, group at a scroll offset), tree-building from day groups, selection logic (click, ctrl, shift range).
- The page cache and retry logic keep working on photo index, not row.

## Error handling

- Photos with no date go to Undated; a library with no dates at all shows only Undated.
- An empty library shows the existing "Add photo folder" prompt.
- A failing query shows an inline error in the pane, not a blank screen.

## Testing

- Rust: day grouping (ordering, undated, counts, `first_index`), folder filter, GPS extraction (valid, absent, out of range), the migration.
- UI unit tests: group-aware layout maths, tree building, selection logic.
- Manual check on the real library: smooth scroll, tree click lands on the right day, no "!" tiles.

## Decisions made while planning

- Selection is index-based, not id-based (see Grid). Cost: selection is lost on re-sort or rescan.
- Pane resizer is a small custom component, not a library.
