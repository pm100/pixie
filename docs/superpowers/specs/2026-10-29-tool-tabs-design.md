# Tool tabs: Send, Create, Organize, Clean up (design record)

Written 2026-10-29 to keep the whole discussion, so it can be picked up later. Branch `tabs`, **based on `main`** (it first got built on `ask-help`, then was moved to `main` at the user's request: the Ask Pixie tab must not be in this build, because the ? replaces it). `ask` (the full Ask Pixie) and `ask-help` (help only) are kept as they are. The first build is done and uncommitted at the time of writing; see "What is built".

Mockups (private Claude artifact, boards for every option below): https://claude.ai/artifact/4trWr16y3se9cuPcfVHUgM

## Why this came up

The user was not happy with how Email is chosen. Two problems, in their words "1 and 2":

1. **Getting to Email.** It was reached from a button in Browse's details pane (one photo) or by going to the Tools tab.
2. **Choosing the photos to send.** The Tools tab had a "Working on" drop-down (Picked / Current photo / a tag) that people did not notice, and it meant leaving Browse and choosing again.

Related worries raised along the way:

- A right-click menu helps, but some users do not use or like right-click, so it cannot be the only way. There is no menu bar (a Windows menu bar was ruled out; a Mac menu bar is a later idea).
- The open issues list many more tools than the Tools tab was designed for (see the inventory below), so whatever we choose must grow.

## Options considered (mockups A to D, then 1 and 2)

| Option | What | Verdict |
|---|---|---|
| A | An action bar beside the Picked pill in Browse: "3 picked: Email · Print · Tag… · Clear", or "Current: …" when nothing is picked. | Kept (as Email, Print, Tag…, More). |
| B | A right-click menu on a photo. Acts on the picked set if the photo is in it, else on that photo. | Wanted, but second pass (shares the More menu). |
| C | A ⋯ button in the photo's hover corner opening the same menu (for touchpads and touch). | Second pass with B. |
| D | In Tools, "Working on" as a visible segmented choice with counts instead of a drop-down. | Kept. |
| 1 | One Tools tab as the catalogue, grouped Send / Make / Organise / Whole library, with shortcuts from Browse. | Superseded by 2. |
| 2 | Four tabs: **Send, Create, Organize, Clean up**, replacing the Tools and Duplicates tabs. Ask Pixie moves behind a **?** button as a side panel. | **Chosen.** |

Reasoning for 2 over 1: each tab is one verb, so people can guess where a feature lives; each list is short (3 to 5 tools); Duplicates stops being the odd one out and joins Bursts and Thumbnails in Clean up; Send, Create and Organize act on a set of photos and share the "Working on" bar, while Clean up works on the whole library and has none.

The earlier idea of dissolving the Tools tab into panels over Browse was dropped: with about 25 tools a catalogue is needed for discoverability, and menus cannot show what Pixie can do.

## Decisions (the user's answers)

1. The **?** is a placeholder icon for now (Ask Pixie as a side panel comes later). Later the user also said the tabs rework is to be **based off `main`**, which removes the Ask Pixie tab from this build (the ? is meant to replace it); keep the `ask-help` and `ask` branches.
2. First build: the tab restructure with the shared "Working on" bar, the Browse action bar with More, and the placeholder ?. The right-click menu and ⋯ button come in a second pass. No new tools.
3. Show placeholder tabs with mocked-up placeholders in the right-hand area (Create is all placeholders).
4. Unbuilt tools are listed **greyed out** (marked "soon").
5. "Working on" carries across Send, Create and Organize; with nothing picked it defaults to **Current**. The Email and Print buttons in Browse's details pane were first relabelled "Email this photo" / "Print this photo", then **removed** at the user's request: the buttons beside Picked (which act on the chosen photo when nothing is picked) replace them. Print gets the same treatment as Email.
6. **Tag…** is a popover over Browse (the grid stays in view), not a trip to another tab.
7. A placeholder icon for the help panel for now.

8. (Later, replacing an earlier rule) **Two action rows.** The row beside Picked in Browse acts on the **picked photos only** (no label: Email · Print · Tag… · More). The details pane has its own row for the **current photo** (Email · Print · More; no Tag…, since its tags are right there, and no label). This replaced two earlier ideas: one row that switched between picked and current photo (confusing once anything was picked, since the user could no longer print just the chosen photo) and gating the row on the Picked pill. Options considered for that problem: a Picked / Current switch on the bar, two button sets, right-click menus, restoring the old details buttons.

9. (Later) The **Picked pill** on Browse's second line was an oddity (a pill among check boxes), so it became a check box, **Filter by picked (n)**, like Map and Filter by map. Turning it on shows only the picked photos and puts a **★ Picked** pill (with ✕) in the filter line above; Clear stays beside it.

10. (Later) **Pick all shown (n)** next to Filter by picked, so "filter, then act on everything" works without a new scope: it picks every photo the filters show, and the action row then acts on the picked set. Considered and rejected: a button that silently acts on everything shown (too easy to tag 10,000 photos by mistake). Limit 2,000 for now (`lib/pickAll.ts`); off while only the picked photos are shown, when nothing is shown, and over the limit. Still wanted before raising the limit: paging for the Tools lists (#79), a confirmation naming the count for bulk tagging, and Undo (#88).

11. (Later) **The second line is two boxed sections, each with a title**, built to take future features: **★ Picked n** (Filter check box, Pick all, Clear, and the action buttons, which act on the picked photos, so they sit inside its box) and **Map** (Display and Filter check boxes; later a View menu for sort, thumbnail size, group bursts, show hidden, tree layout may become a third section; search goes on the first line). Rule: **new tools never get a button on the line; they go under More.** Free space decides how many chosen buttons show; the rest fold into More (`directCount` in `lib/actionBar.ts`, then the line wraps as a last resort). **Pick all** sits on the first line beside the count (it picks what the filters show), and the **Picked section is shown only while something is picked**, since with nothing picked it offers nothing. Tried and dropped: thin vertical bars between zones (a bar between the picked controls and the buttons suggested the buttons do not act on the picked photos). A caution: if ratings or favourites (#6) arrive they will want a star too, so use another symbol for them or change what picked uses.
12. (Later) **Settings, Browsing, Action bar:** the user chooses which actions are buttons (checkboxes over Email, Print, Tag… and every other Send / Create / Organize tool; unbuilt ones are greyed). Default Email, Print, Tag…. Stored as the preference `actionBar`. It applies to the row beside Picked and to the row in the details pane (which has no Tag… and shows at most three buttons); the rest are under More.

Defaults applied without being asked: US spelling ("Organize", matching issue #13); Settings stays a tab on the right.

## Layout

- Tab bar: Browse, People, Send, Create, Organize, Clean up, Settings, then a greyed **?** at the far right (no Ask Pixie tab in this build). People is shown only while face analysis is on. Duplicates is listed in Clean up only while the duplicate check is on.
- Send, Create and Organize: "Working on" bar on top (Picked n / Current / A tag…, the photo count, Clear picks); tools list on the left; the tool's options in the middle; the photos on the right. Widths are draggable and remembered, as before. Each tab remembers the tool it was last left at.
- Clean up: a yellow banner "Whole library. <tool> works on every photo, not on the ones you picked."; tools list on the left; the tool on the right. No photos column.
- Browse's second line: the Filter by picked check box and Clear, then **Email · Print · Tag… · More ▾** (no label; always acting on the picked photos), then the Map checkboxes. The details pane has a second row, **Email · Print · More ▾**, for the current photo. Email and Print open that tool in the Send tab. More lists the other tools by tab, unbuilt ones greyed, plus "All tools…".

## Where each tool lives (from the open issues)

**Send** (get photos out): Email (#4, #59 Thunderbird, #80 contacts) *built*; Print (#11, #85 print bug) *built*; Share by cloud drive (#81); Copy to clipboard (#74); Open with… (#84); Slideshow (#10). Also Desktop background (#5) and showing on a photo frame (#46, #43) if they come.

**Create** (make something new): Collage and mosaic templates (#12); Animated GIF (#83); Frames from a video (#82; part of it is a "Save this frame" button in the viewer); Edit, non-destructive (#9).

**Organize** (change what Pixie knows or where files are): Tags (#24) *built*; Fix dates for a selection (#27); Hide (#71); Copy or move into folders (#13); also Tags from folder names (#25, with a review screen), Master repository (#31), Import/export (#44), and rename or split people (#38, #62).

**Clean up** (whole library, no selection): Duplicates *built*; Bursts (#51); Thumbnails and cache (#33). Import/export (#44) could go here or in Organize.

Things that shape all of this: Undo (#88), keyboard navigation (#67; some users will not right-click), the rule for button colours (#76), scale limits of Tools lists and Print at 100,000 photos (#79).

## What is built (branch `tabs`)

Files: `ui/src/lib/tools.ts` (the tool list and placeholders, with tests), `tabs.ts`, `ToolsTab.svelte` (now one component for all four tabs, taking a `group`), `WorkingOn.svelte`, `PlaceholderTool.svelte`, `PhotoActions.svelte`, a placeholder `?` in `Tabs.svelte`, `Details.svelte` (a second action row for the current photo), `App.svelte` wiring. 
Checks run on `main`'s base: svelte-check 0 errors / 0 warnings; UI tests 354 pass. **The app has not been run.**

**Ask Pixie edits held back** (they only apply on `ask-help`, where Ask Pixie exists). When `ask-help` is merged after `tabs`, redo them: in `ui/src/lib/askGoTo.ts` rename the go-to targets (`tools-email` to `send-email`, `tools-print` to `send-print`, `tools-tags` to `organize-tags`, `duplicates` to `cleanup-duplicates`, drop `tools-details`) with a `Place` that carries `tab` and `tool`; update the proxy's `GO_TO_TARGETS` in `ask-proxy/src/tools.ts` (a test compares the two lists), the guide (Tabs, Browse, Working on, Tools, Tags, Duplicates sections: Tools becomes Send / Create / Organize, Duplicates is in Clean up, "Current photo" is "Current", unbuilt tools are greyed), the prompt examples and `scripts/examples.mjs`; then `ToolsTab`'s old `start` prop is replaced by App's `toolStart`/`openTool`. A copy of that work (as a diff on `ask-help`) is in the session scratchpad only; it is easy to redo. Then redeploy the worker.

**Not done:**
- Not committed or pushed.
- Not exercised in the real app: the Tag… popover closing, the Current default, the Duplicates screen inside Clean up, the ? placeholder.
- No test for the new Svelte components (the logic is in `tools.ts`).

## Still to do or decide

- **Ask Pixie as a side panel** from the ? (mockup board "Ask Pixie as a side panel"): docked on the right, about 300 px, F1 as well as the ? button, chat kept until Pixie quits, shrinks or moves when on Settings so Apply cards do not hide the settings. Then remove the Ask Pixie tab.
- **Right-click menu and ⋯ button** (second pass), using the same menu as More. Keyboard use (#67) and Undo (#88) matter more once there are bulk actions in menus.
- **Whether Settings becomes a gear icon** if eight tabs feel crowded.
- Whether unbuilt tools should stay visible or be hidden when a tab is released (the user chose visible and greyed for now).
- Where Import/export goes (#44).
- Whether `Tag…` should also appear in the right-click menu (yes, per mockup B).
- Post a summary on #24 (and notes on #75 and #45); the user did not answer this. Worker redeploy needs the user's say.

## Other threads from the same session (for context)

- **Ask Pixie** was wound back to a help system (branch `ask-help`, pushed); the full version with find/pick/tag is on branch `ask` at `c3ce19e`. See `2026-10-28-ask-pixie-design.md`.
- **Cloudflare KV free tier:** the worker uses KV for a per-install daily question count (`q:<install id>:<date>`, one read and one write per question) and for feedback records. Live test runs with random install ids created about 750 keys in one day, which reached 50% of the daily write allowance. Fixes proposed, not done: send the dev key from `examples.mjs`; make quota read and write failures non-fatal (fail open recommended); expire or delete sent feedback so the daily digest stops re-reading old records. The app's real users cost at most 10 writes a day each.
- **Linux:** the platform code already has fallbacks; missing are a CI job, deb/AppImage bundle targets, Email on Linux, and a WebKitGTK check for Print. **Web version:** a browser-only app reading OneDrive (then Google Drive) was preferred; iCloud is effectively impossible; avoid server-side processing. Neither was started.
