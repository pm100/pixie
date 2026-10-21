# First run, and a modal scan with analysis

Status: implemented on branch first-run-flow; waiting for the author's first-run check in the real app. Date: 2026-10-20. Replaces nothing; narrows issue #32 (see Scope).

## Why

A new user today has to know to open Settings, add a folder, and wait. While the scan runs, Browse fills in under their
hands and flickers. Progress is shown only in Settings. A second folder added during a scan is accepted and then not
scanned, with a small error. Users do not know which folders to add (OneDrive in particular).

The wanted experience: on first run Pixie suggests where the photos probably are, the user ticks what they want, and a
foreground, modal flow loads everything, then analyses it, then hands over a fully loaded Browse screen and says what
it found.

## Goals

- A welcome screen on every launch while no folders exist, suggesting likely photo and video folders, pre-ticked where
  the guess is confident, with "you can change this any time in Settings, Photo folders".
- A modal scan, shown as a slideshow strip of real photos as they are read, with counts and a progress bar. The
  Browse screen is not visible, and so cannot flicker, until it ends.
- Adding a folder later (Settings) skips the welcome and goes straight to the same modal scan. It must be impossible
  to add a folder that is then not scanned.
- After loading, the same modal continues as "Pictures loaded, now analysing": content analysis (search and Auto
  tags) and face extraction run in the foreground, each with its own progress bar and a plain sentence saying what it is
  for. A "Carry on in the background" button ends the modal and opens Browse; the analysis keeps running there.
- When both finish, an "All set" screen: counts per folder (photos and videos), and a "Faces found, open the People
  tab" box only if faces were found, with an "Open People" and a "Start browsing" button.

## Non-goals and anti-goals

- No change to launch rescans, periodic rescans or watchers: scans Pixie starts by itself stay in the background and
  keep the app responsive (issue #32). The author may revisit this after trying the new flow.
- Not a wizard for settings, email or anything else.
- Must not scan or list whole drives, system or application folders, or anything it is not confident is media.
- Must not probe slow or network locations at startup (a mapped drive such as `P:\` can stall for a long time).
- Videos are found and counted but not analysed (unchanged; see issue #39).
- Do not hide an error: a failed model download shows the error and a Retry (the same retry as Settings).

## Flow

1. **Launch, no folders:** Welcome. Folders exist: no welcome, launch rescan in the background as today.
2. **Welcome:** list of suggestions (checkbox, name, path, approximate count), "Add another folder...", the Settings
   note, buttons "Skip for now" (closes to the existing empty Browse state; the welcome returns on the next launch) and
   "Start" (disabled with nothing ticked, labelled with the number of folders).
3. **Scan (modal):** add the ticked folders, then one scan job over all of them. Shows the folder being walked, running
   totals, a progress bar once the number to read is known, and a strip of photos: the one just read large, neighbours
   fading either side, with its file name. A "Stop" button cancels; the user lands in Browse with what was found.
4. **Analysing (modal):** same dialog, new title. Three bars: "Preparing previews", "Understanding your photos"
   (search and Auto tags) and "Finding faces" (with a count of people so far). The models were already downloading since launch, so a bar may
   first read "Getting the model... 62%", then switch to counting by itself; no separate screen. A failed download
   shows the error and Retry in that bar. "This can take a while for a big library; you do not have to wait here."
5. **All set:** as above. "Start browsing" closes it.
6. **Add folder later (Settings, folders exist):** pick a folder, then steps 3 to 5, skipping the welcome.

## Where the code is today (so the builder starts in the right place)

- `src-tauri/src/lib.rs`: `add_folder`, `start_scan(folder_id|all, full)` (one job at a time, refused when busy via
  `ScanGate`), `cancel_scan`; events `scan-progress {folder_id, seen, indexed, to_index}`, `scan-error`, `scan-done`.
  A scan job walks its folders one after another, then `analysis.kick()`, `people.kick()`, then `prefetch_all`
  (thumbnails) and only then releases the gate.
- `crates/pixie-analysis`: `AnalysisService` and `PeopleService` start at app launch and download their models
  there; their workers wait while the scan gate is held. Status events `analysis-status` and `people-status` carry
  models state, done/total and errors.
- `ui/src/App.svelte`: `chooseFolder`, `runScan`, launch rescan, the empty state (`folders.length === 0`), and the
  scan listeners. `Nav.svelte` shows `scanLine` for background scans (keep that for them).

Two consequences of that code that the design depends on:

- (Corrected after reading `prefetch_all`: it only queues thumbnail jobs and returns at once, so the scan gate is
  released right after the scan and the analysis workers start then, competing with the thumbnail pool for CPU. They do
  not wait for thumbnails.) Browse would still open with previews being made, so the analysing step shows a third line,
  "Preparing previews", driven by the thumbnail queue's length (the thumbnail service needs a public pending count;
  the line's total is the largest count seen since the step began). The step ends when previews, search analysis and
  faces are all done. (My call.)
- `scan-progress` carries counts only. The strip needs real photos: some way to learn the id of a recently stored photo
  at a modest rate (a few per second at most) so the UI can ask for its thumbnail. Mechanism is the builder's choice.

## Suggesting folders (guesses marked)

- Known places from the OS: Pictures, Videos (Movies on macOS), Camera Roll and Screenshots under Pictures, and, on
  Windows, OneDrive's Pictures (from the OneDrive environment variables and `~/OneDrive*/Pictures`); Dropbox
  and iCloud Photos where present. A folder nested in another suggestion is listed as "included in ..." under the
  parent and not offered separately.
- Approximate counts come from a bounded, time-boxed walk per suggestion run off the UI thread; the screen appears at
  once and counts fill in, with a cap shown as "10,000+". (My guess at limits: about 2 seconds per folder.)
- Beyond the standard places, "look a little further" (my guess, bounded): folders up to three levels under the home
  folder and the roots of fixed local drives that contain many media files, skipping hidden, system, application,
  cache and dependency folders (`AppData`, `node_modules`, `Library`, `.git`, ...) and skipping network and removable
  drives. Offered **unticked**, with the file kind that made it look likely. Downloads and Desktop are offered
  unticked.
- Confidence rule (my guess): the OS Pictures/Videos folders and OneDrive/Dropbox/iCloud photo folders are pre-ticked;
  everything else starts unticked.

## Adding a folder while a scan is running

A background scan (a launch rescan) may be running when the user adds a folder. The modal attaches to the running scan,
shows its progress, and then scans the new folder. The user never sees "will not be scanned". While the modal is open
nothing else can start a scan.

## Behaviour to keep

- Pause and Retry for analysis in Settings keep working. Stop during the modal scan keeps everything found so far.
- Closing the app mid-flow is safe: the next launch rescans in the background and the analysis resumes.
- "Reload" of a folder (Settings) and launch rescans stay as they are, in the background (my call: they are not first
  loads).

## Left to the builder

The mechanism for streaming sample photos to the strip; the folder-walk implementation and limits; whether the strip
subsamples when photos arrive faster than it can draw; exact wording and visual styling within the mockups
(`.superpowers/brainstorm/.../content/*.html`: welcome-and-scan, finish-screen, analysing-screen); mac paths.

## Tests expected

Pure logic with fakes: folder suggestion (nesting, skipping rules, counts capped), the flow state machine (welcome,
scan, analysing, all set, stop, background handoff, busy attach). A manual run on a real first start (reset the data
folder through Settings, Developer, Open data folder).
