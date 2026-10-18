# UI shell and navigation redesign

Date: 2026-10-17. Status: draft for review. Replaces the "Navigation on the left" part of
`2026-10-16-shell-and-date-tree-design.md`; everything else in that spec (resizable panes, details pane) stays.

## Why

The UI grew feature by feature: one window with a date tree, a folders list and a Map link on the left, the grid in the
middle, details on the right, and a few buttons. Email, tags, collage, print, edit and settings all need somewhere to
live. This spec fixes the structure first so those features have a home. It changes no library, scanning or metadata
behaviour.

Mockups the decisions come from (private pages):
https://claude.ai/artifact/1tDLce6kM4kMcY5VkSrFt9 (shell, viewing) and
https://claude.ai/artifact/Uc6v9sjhH11yEY1WJbyyi2 (navigation).

## Decisions (made by the project owner)

| Topic | Decision |
|---|---|
| Overall shell | **Task tabs** along the top: Browse, Map, Create, Share, Settings. |
| Viewing | **Full window with filmstrip** and **pop-out window**, both available. A setting chooses which one a double-click or Enter opens. |
| Menu bar | **None.** |
| Right-click menus | **None for now** (the webview menu stays switched off, as today). |
| Folders | **Removed from navigation.** They are plumbing and move to Settings. |
| Navigation | **Sections**: Library, Recent, When. |
| Date structure | **Years with month chips and busy-month bars; the selected month lists its days as date badges** (weekday over number, with the day's main place beside it). |
| Recent | **"Last 30 days" only.** No "This year". |
| Where | **No place tree.** Places are what the Map tab is for. |

## Shell

A tab bar across the top. Tabs are real screens, not panels:

- **Browse**: left navigation, thumbnail grid, details pane (the three panes we have today, still resizable).
- **Map**: today's full-library map, with the grid strip and details pane beneath/beside it as now.
- **Create**: home for collage, print, slideshow, edit, wallpaper, projects. Empty today.
- **Share**: home for email and later other sending. Empty today.
- **Settings**: a page, not a dialog (see below).

**A tab with nothing in it is not shown.** At the first build the tab bar reads Browse, Map, Settings. Create and Share
appear when their first feature ships, so no one meets a "coming soon" screen.

**Switching tabs must not lose context.** Selection, scroll position, filter and the open nav branch are held in
state above the tabs, not inside them. This was the main risk the mockup page flagged for task tabs.

**Selection bar (assumption, not yet confirmed).** While one or more items are selected on Browse, a slim bar appears
over the bottom of the grid with the actions that apply ("12 selected · Email · Clear"). Actions are added as their
features arrive; with only the existing features it holds just the count and Clear. This is what lets Create and
Share work on "what I selected" without the user re-choosing items.

## Navigation (Browse tab, left pane)

Three sections, top to bottom:

1. **Library**: All items, Photos only, Videos only. Replaces "Everything" and the kind filter's place in the UI.
2. **Recent**: "Last 30 days". A filter: the grid shows only items from the last 30 days until the user picks
   something else.
3. **When**: the date structure below.

Tags and Projects will become further sections when those features exist; the shell needs no change for that.

### When

- One row per year, newest first: year, a small bar chart of the twelve months (height = item count), and the total.
- Years older than the last several collapse into decades ("2010s") that expand to their years.
- **Undated** is a row at the end for items without a trustworthy date.
- Opening a year shows its months as a chip grid (4 columns). Months with no items are dashed and not clickable.
  The current month is filled. Only one year is open at a time.
- Opening a month lists its days under it as **date badges**: a small tile with the weekday above the day number
  (weekends tinted), the day's main place to its right, and the item count at the far right. Because the badge has a
  fixed size, nothing needs to line up by hand. A day without a place shows just the badge and count.
- Clicking a year, month or day **jumps the grid to it and highlights it**, and the open path follows the grid as it
  scrolls. This is what the current tree does (`Tree.svelte`, driven by `groups` and `activeGroup`) and is kept; it is
  not a filter.

### Scan status

While a scan runs, the bottom of the navigation shows one line ("Scanning P:\… 12,400 found") so a half-filled
library never looks empty without explanation.

## Viewing

Both viewers show the same content: picture or video, the date and place caption, previous/next, close.

- **Full-window viewer** (today's, plus a filmstrip): fills the Browse window. Arrow keys move on, Esc returns.
- **Pop-out viewer**: its own window for a second monitor. A "follow selection" switch keeps it in step with the grid;
  with it off it keeps showing its item. Arrow keys work in either window.
- A **"Pop out"** button in the full-window viewer moves the current item to the pop-out window; the pop-out has the
  reverse.
- Setting **Open items in**: Full window (default) or Pop-out window.

## Settings

A page with a list down the left:

- **Photo folders**: the folders Pixie watches, item count and last scan for each, Add, Remove, Rescan. Removing
  a folder never deletes files; it only stops Pixie showing them. (This is the old Folders list and the old "add
  folder" button. A first-run welcome, issue #20, will reuse the add-folder step.)
- **Viewing**: the "Open items in" choice.
- **Email**: the account settings dialog we have today (`MailSettings.svelte`), unchanged in behaviour.
- **About**: version, licence, link.

## What changes in the code

- `App.svelte` gains a tab state and moves view state (selection, filter, scroll, nav) into modules shared by all
  tabs; today it mixes the grid, map and nav concerns in one 360-line file.
- `Tree.svelte` is replaced by a `Nav` component (sections) with a `WhenNav` inside; `buildDateTree` in `tree.ts` is
  extended with per-month counts (for the bars) and per-day entries. `Shell.svelte` is reused for the three Browse
  panes.
- New `Settings` page component with sub-pages; `MailSettings.svelte` becomes one of them.
- A pop-out window: a second Tauri webview window that loads the same UI in a "viewer only" mode and listens for
  selection events from the main window (Tauri's event system). The Tauri capability file needs the new window label.
- New `since` bound on `PhotoQuery`/`Filter` for "Last 30 days".
- The existing "Folders", "Map" and "Dates" tree headings, the folder filter in the view, and its UI plumbing
  (`setFolder`) are removed. The `folder_id` filter in the backend stays (Settings may later show per-folder views).

## Out of scope here (separate work)

- **Place names on day rows.** Pixie looks a place up only when a photo is opened and does not store it. A place per
  day needs a stored place for every located photo, resolved in the background with caching by map cell (the lookup
  service allows about one request a second). That is its own design; it also unlocks searching by place. Until it
  exists, day badges show no place.
- Tags, search, projects, collage, print, edit, email delivery (these only gain a place to live).
- Right-click menus, a menu bar, keyboard shortcut sheet.

## Build order

1. Tab bar, state lifted above tabs, Settings page with Photo folders (Browse and Map keep working as now).
2. Navigation: sections, Recent filter, When with chips, bars, day badges (no places yet).
3. Filmstrip in the full-window viewer; "Open items in" setting.
4. Pop-out window with follow-selection.
5. Selection bar.

Each step leaves a working app.

## Testing

- Unit tests (vitest): date-tree extension (per-month counts, per-day entries, decade grouping, undated), view state
  kept across tab switches, "Open items in" preference.
- Rust: `since` filter test in `pixie-index`.
- Manual: switch tabs with a selection and scroll position and come back; scan with Settings open; remove a folder and
  confirm its items vanish and files stay; pop-out follows selection and survives its window being closed.

## Open questions

- Is the selection bar wanted, or do actions live only on the Create/Share tabs?
- Should the pop-out remember its size and position per monitor? (Proposed: yes, via the existing prefs.)
