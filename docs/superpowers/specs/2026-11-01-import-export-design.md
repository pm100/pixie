# Import and Export: design

Status: **revised 2026-11-05 after a talk with the user** (first written 2026-11-01 from a short chat, not built). Issue: #44. Items marked *(my call)* were not decided by the user; items marked *(agreed)* were. The two things the user asked for:

- **Import** from a variety of sources (a USB stick, a card, a directly connected phone, and so on) into a connected folder, the sync folder by default.
- **Export** a selection or everything, as files or zipped.

**What changed in the revision:** the first design had a purpose-built thumbnail grid for choosing what to import. Two ideas from the user replace it, *(agreed)*: (1) a card, stick or folder is **browsed where it is**, as a temporary source in Browse, so choosing uses the grid, viewer and right-click commands Pixie already has; (2) things that cannot be browsed in place (a phone or camera over a cable) are copied first into an **Inbox**, a holding area in Pixie's data folder, and browsed there. Both end in the same three commands: **Keep**, **Private** and **Discard**. A **zip file** is a third kind of source, expanded into the Inbox *(agreed, later the same day)*.

Related: #31 (Sync, the master folder; `docs/superpowers/specs/2026-10-31-master-repository-design.md`), #110 (private photos, closed: Make private and Make public exist), #105 (database in a shared folder), #99 / #68 / #113 (online-only cloud files), the Processing screen plan (`docs/superpowers/plans/2026-11-01-activity-screen.md`), #71 (hide), #7 (duplicates), #13 (copy or move into folders), #121 (the command registry the Keep, Private and Discard commands are made with), #117 (cleaning up leftovers).

## Intent

Getting photos in and out of Pixie should be as safe and as plain as Sync already is: originals are only read, every copy is checked before it counts, nothing is overwritten, and the person can see what happened. Today the only way in is to add a folder Pixie can already reach, and the only way out is Email or Print. A person with a camera card, a USB stick or a phone has no way in, and a person who wants a backup, a gift or a zip for someone else has no way out.

## Where it lives

The **Import/Export** tab (group `transfer` in `ui/src/lib/tools.ts`) gets two tools beside **Sync**: **Import** and **Export**. This settles "Where Import/export goes (#44)" from the tool-tabs design. **Import** is where a source is started (From, Look, Into, Result); the photos themselves are then looked at in **Browse**, under a node for the source. Export works from the Tools "Working on" set. Both report progress on the Processing screen (new kinds `Import` and `Export`).

## Goals

- Import from a card, a USB stick, any folder (including a network share or cloud folder) and, later, a connected phone, with a preview and a clear idea of what is new.
- Choose what to bring in by looking at the photos in the grid Pixie already has, with nothing copied (and no disk used) until the person says so.
- Never copy the same photo twice, even weeks later, even if the person deleted the first copy from the library.
- Export a selection, a filtered view or the whole library, as plain files or one zip, as originals or resized copies.
- Be safe: sources are only read (deleting from a source is a separate, explicit step), copies are verified, a stopped or interrupted run can be picked up again.
- Same rules as the rest of Pixie for private photos, hidden duplicates and online-only cloud files.

## Non-goals (first versions)

- Importing from the Apple Photos or Google Photos library, or from the cloud (that is Sync's other direction, and #81).
- Editing, renaming by pattern or fixing dates during import (Fix dates and Missing dates already exist after the photos are in).
- Showing RAW files and camera video formats (a separate piece of work, #116); importing copies them along when asked.
- Analysing photos while they are only on a source (see "Two ways to look at a source").
- Burning discs, FTP/SFTP, and writing tags into the photos themselves.

## Import

### Two ways to look at a source *(agreed)*

| | **Browse in place** (phase 1) | **Inbox** (phase 2) |
|---|---|---|
| For | A card, USB stick, folder, network share: anything with a path | A phone or camera over a cable (no path), a zip file, and photos held for review (#110) |
| What happens first | The source is indexed where it is: dates and thumbnails only | Everything chosen by the filters is copied into the Inbox, verified |
| Disk used before you choose | None | The copy (a free-space check and a size line come first) |
| The card or phone can be unplugged | After Keep (or Delete from the card) | Once the copy is verified |

Both appear in Browse's tree as their own node ("Card: SONY (412)", "Inbox (380)") and are looked at with the normal grid, viewer, details and right-click menu. Both end in the same commands, in the action bar and the photo right-click menu (registered with the command registry, #121):

- **Keep** (primary): copy (or, from the Inbox, move) the picked photos to their place in the chosen Pixie folder, with the chosen layout, using the safe copy.
- **Private**: send them to the local **Private photos folder** and mark them private, so they never reach the master.
- **Discard** (Inbox only; for a source, **Done with this card** removes the source from Pixie and does not touch the card): delete from the Inbox.

The photos to act on are the **Picked** set *(my call)*: pick what you want in the grid, then Keep. "Keep all new" is also offered.

#### A source, browsed in place

- A source is a folder Pixie indexes with a flag **temporary** (`folders.kind = 'source'`, a migration). It is scanned for dates, sizes and dimensions only, and thumbnails are made as rows come into view (cached; a slow USB read happens once).
- It gets **no analysis**: no tags, faces, embeddings or hashing (that is slow over USB, and the person may discard most of it). A photo that is kept is analysed like any new photo, in its new folder.
- It is **kept out of the library**: the default Browse, People, Trips, the map, the counts, Sync and Email's sets leave temporary folders out. Choosing the source's node shows only that source. It never feeds the master.
- It is **removed**, with its rows, thumbnails and picks, when the drive is unplugged, when **Done with this card** is pressed, and at start-up if the drive is gone *(agreed)*. Nothing of it lingers in the library.
- Several sources can be open at once, each its own node *(my call)*.
- **Not for phones:** an MTP device has no path, so a source cannot be a folder; that is what the Inbox is for.

#### The Inbox

- A folder under Pixie's data directory (`inbox/`, a Settings option to move it; ideally on the same drive as the destination so Keep is an instant rename). Items are copied in with the safe copy (temporary name, read back, size and hash compared, renamed in), then indexed with hash, metadata and thumbnails only; full analysis waits for Keep *(my call)*. A Private item is never analysed, so no faces or tags exist for it.
- Before copying, the tool shows what it will use ("This will take about 6.1 GB; D: has 41 GB free") and refuses if it will not fit.
- Items already in the library (identical hash) are marked, and **Discard already in library** clears them in one step.
- Once an item is verified in the Inbox, **Delete from the source** can be offered for it (see Rules).
- The Inbox is a general holding area: held-for-review arrivals (#110's "New, not yet shared"), downloads and Export staging can use it later. Leftovers in it are cleaned by #117.

#### A zip file *(agreed 2026-11-05)*

A zip (one sent by email, an export from a phone or a cloud service, a backup) is a source that cannot be browsed in place, since every thumbnail would mean decompressing a file, but whose list of entries is cheap to read. So it goes through the Inbox:

1. **From:** **A zip file…**. **Look:** the entries' names, sizes and dates are read from the zip's directory (nothing is extracted) and summarised like any source. Photos and videos only by default; other entries are counted.
2. **Import into the Inbox:** the chosen entries are extracted with the safe copy (a temporary name, the size and CRC checked against the zip's own record, then renamed in). The uncompressed total is known beforehand, so the free-space check is exact.
3. **Browse the Inbox:** Keep, Private and Discard as for any source. An item counts as kept before when the zip's identity (its size and modified time) and the entry's path, size and CRC are in the import log.

What a zip needs that a folder does not:
- **Unsafe names are made safe and listed:** `../` and absolute paths (nothing may be written outside the Inbox), Windows reserved and over-long names, backslashes, duplicate names, and non-UTF-8 names.
- **Zip bombs:** a cap on the total uncompressed size and on the compression ratio, as well as the free-space check.
- **Encrypted entries** are listed as skipped with the reason (passwords are later). Zip64 and archives over 4 GB are supported; multi-part archives (`.z01`, `.z02`) are not at first.
- **Zips inside zips** are counted and not opened by default.
- **Dates:** an entry's own date is local time with no time zone, so it is only the fallback after the photo's own date taken.
- **Other archive types** (7z, rar, tar.gz) and **Google Takeout and iCloud export zips** (an export split into several zips, with the real dates in JSON files beside each photo) are later work, see the issue for them.
- The `zip` crate is a new dependency (Export needs it too): run `just notices`.

### The flow (in place)

1. **From.** A list of places Pixie can import from, refreshed while the tool is open: cards and USB drives (with their label and size; a drive with a `DCIM` folder at the root is shown as a camera card), phones and cameras (phase 2), and **A folder…** for anything else. A source Pixie imported from before shows when it was last imported.
2. **Look.** Pixie opens the source (above) and shows the count as it goes ("Looking… 3,200 found"), with Stop. A summary follows: "412 photos and 18 videos, 6.1 GB, 3 March to 21 May. 380 are new." with **Open in Browse**.
3. **Choose, in Browse.** The source's node shows its photos. **New** items are marked as such; items the library already has (see "Knowing what is new") are marked **In your library** and are not picked by **Pick all new**. The usual filters apply (Photos, Videos, a date range). The person picks what to keep.
4. **Into.** A label-then-control row (no controls in mid-sentence), shown in the Import tool and in the Keep confirmation: *Folder:* one of Pixie's folders (default: the sync folder; otherwise the last one used, otherwise the first folder); *Layout:* "Year and date (2024/2024-03-05)" (default, the same as Sync), "Year and month", "One folder named…", "Same folders as on the source", "All in one folder".
5. **Keep** (the one primary button, labelled with the count: "Keep 380 items"). It copies, verifies, then leaves the folder to be read as usual: Pixie scans it, and the usual analysis follows (visible on the Processing screen).
6. **Result.** "Kept 362 photos and 18 videos (5.9 GB) in 3 min. 24 already in your library were skipped. 2 problems." with **Show them** (Browse filtered to this batch), the list of problems, **Done with this card**, and, only when the source supports it, **Delete from the card…** (red; see Rules).

### Rules

- **Originals on the source are only read.** Deleting is never part of the copy. After a run, the result screen can offer **Delete from the source…** for the items that were copied *and verified by size and hash*; it asks for confirmation with the count and says it cannot be undone (a card has no recycle bin). *(Agreed: a separate step after the result, not a tick box before the copy.)*
- **A copy is only put in place when it is complete and checked.** Each file is copied to a hidden temporary name inside the destination folder, read back and compared (size and hash), then renamed into its final place. A file that is already there is never overwritten: an identical file counts as done and the new copy is dropped; a different file with the same name makes the new copy take a numbered name (`IMG_0001 (2).jpg`). This is the same behaviour as Sync, and the code is shared (see Architecture).
- **The date folder comes from the copy.** The copy is made first under its temporary name, then its date taken is read from the copy (EXIF; otherwise the source's modified date), then it is renamed into `YYYY/YYYY-MM-DD`. This avoids reading each file twice from a slow source. *(My call.)*
- **Interruptions.** A card pulled out, a phone that locks, a full disk: the run pauses with the reason and what was done, partial files are removed, and a later run carries on (already-kept items are recognised by the import log below). One unreadable file (a bad card sector) is listed and skipped; it does not stop the run. A source whose drive disappears while it is open is removed, and says why.
- **Online-only and slow sources.** Nothing skips online-only cloud files (decided for the Processing screen plan); an import from a cloud folder downloads the files it reads and the delay shows on the Processing screen.
- **Supported files only by default.** Photos and videos Pixie supports (`SUPPORTED_EXTENSIONS`, `VIDEO_EXTENSIONS`). Other files on the source (RAW, `.AAE` and `.xmp` sidecars, `.THM`) are counted ("14 other files, not imported") with a tick box **Also copy the other files that go with the photos** (off by default) that copies them beside their photo when Kept. Pixie's own and the system's folders are not looked into (`.thumbnails`, `Android/data`, `System Volume Information`, `$RECYCLE.BIN`, macOS `._` files).
- **Live Photos and pairs.** A photo and the short video that goes with it (iPhone Live Photo: `IMG_1234.HEIC` and `IMG_1234.MOV`) are shown and copied as they are, as two items with the same base name; treating them as one is later work.

### Knowing what is new

Reading every file just to find out whether it is already in the library would be slow over USB, so a source is never hashed while it is browsed. Three levels, cheapest first:

1. **The import log** (new table `import_log`: source key, path on the source, size, modified time, hash, import time, batch). Same source, same path, same size and time means kept before, even if the library copy was later deleted. This makes "New since last time" instant. The source key is the volume's serial number or GUID with its label, or the device id for a phone.
2. **Looks the same as something in the library**: same file name (ignoring case) and same size as a library item. Only a hint: such items are marked "Looks like IMG_0451.jpg in your library" and are not picked by Pick all new, but the person can pick them.
3. **Identical by hash** after copying (Keep, or the Inbox's copy): the library's stored hashes (the duplicate check's `full_hash`) are compared with the copy's. An identical copy is dropped and counted as "already in your library". This is the only automatic skip that depends on content.

### Private photos and the sync folder

The sync folder is the shared master (#31). A photo that goes straight into it is shared at once, so Keep never does that blindly. #110 is done (Make private and Make public, the Private entry in the tree), which settles how: **Private** is one of the three commands, and a private photo is sent to the person's **Private photos folder** (a local folder chosen in the Sync settings, asked for once if none is set) and marked private, never to the master. Keep into a folder that does not feed the master needs no such care. The Private photos folder is the follow-up #110 named for "Make private on an item already in the master", so it is built once and used by both. *(My call: a Keep that would feed the master shows a plain line, "These will be shared through the sync folder. Use Private for any you want to keep to yourself.")*

### Sources

| Source | Windows | macOS | Phase |
|---|---|---|---|
| Card, USB stick, any folder, network share, cloud folder | A drive letter or path | A volume under `/Volumes` or a path | 1 (browse in place) |
| A zip file (emailed, exported, a backup) | Any path | Any path | 2 (Inbox) |
| Android phone (MTP) | Windows Portable Devices (WPD) | Not supported by the system; needs a libmtp-style helper | 2 (Windows, Inbox), later (Mac) |
| iPhone / iPad | Appears as a portable device when Apple's drivers (Apple Devices app / iTunes) are installed; photos under `DCIM` | ImageCaptureCore | 2 (Windows, Inbox), 3 (Mac) |
| Camera, through its card | A drive letter | A volume | 1 (browse in place) |
| Camera, through a USB cable (PTP/MTP) | WPD | ImageCaptureCore | 2 (Windows, Inbox), 3 (Mac) |

Notes on phones (to be checked in a spike, not yet tried):
- A phone must be **unlocked** and set to **File transfer** (Android) or must **Trust this computer** (iPhone). The tool says so in words when it finds a device that will not list its files.
- Listing thousands of files over MTP can take minutes. The list streams in, with Stop. A phone's own thumbnail is used where it offers one.
- The iPhone setting *Transfer to Mac or PC: Automatic* converts HEIC to JPEG while transferring, which changes the files; with *Keep Originals* they come across as they are. If Optimise iPhone Storage is on, the full-size file may be in iCloud and the phone only has a smaller version. The tool shows a note when it detects either (a HEIC listed as JPEG, or files much smaller than their dimensions suggest).
- A phone may show the same photo in several folders (`DCIM/Camera`, an app's media folder). Items are matched by size and name, so the second sighting is "looks the same".

Detecting sources: Windows already has `GetLogicalDrives` / `GetDriveTypeW` in `src-tauri/src/drives.rs`, which leaves removable drives out on purpose for the first-run look-around. A new `removable_roots()` lists type 2 (removable) drives, plus any fixed or network drive with a `DCIM` folder, polled every two seconds while the Import tool is open and while a source is open (to notice it being unplugged). A "card inserted" prompt outside the tool is later work.

## Cameras

Cameras reach Pixie in two ways, and both are Import sources:

1. **Through the card** (a card reader or the computer's card slot). The card is a drive, so this is browsed in place (phase 1); a drive with a `DCIM` folder is shown as a camera card, labelled with the camera's folder names (`100CANON`, `101NIKON`, `100MSDCF`).
2. **Through a USB cable to the camera** (phase 2). The camera appears as a portable device (PTP or MTP) through Windows Portable Devices, the same code as phones. Many cameras have a USB mode setting: *Mass storage* makes the camera appear as a drive (phase 1 works), *PTP / MTP* makes it a portable device (phase 2). The tool tells the person to try the other mode when a connected camera shows nothing. Tethered shooting and camera control are not part of this.

What is different from a phone:

- **RAW and the files that go with it.** Cameras write RAW (`.CR2`, `.CR3`, `.NEF`, `.ARW`, `.RAF`, `.ORF`, `.RW2`, `.DNG`), often as a RAW + JPEG pair with the same base name, and sidecars (`.XMP`, `.XML`, `.THM`). Pixie cannot show RAW yet, so it is a separate piece of work (#116). Until then, a camera source ticks **Also copy the other files that go with the photos** by default *(my call)*, so a RAW is copied beside its JPEG and is never left behind; Browse shows the JPEG and the count reads "412 photos, 412 RAW files copied with them". A RAW-only shooter would import files Pixie then cannot display: the tool warns about this before copying ("These are RAW files. Pixie can copy them but cannot show them yet.").
- **Camera video** is often not MP4/MOV: AVCHD (`PRIVATE/AVCHD/BDMV/STREAM/*.MTS`), XAVC and MXF. They are copied as they are when the other-files option is on, and are not shown in the grid until Pixie supports them.
- **Pairs stay together.** Files with the same base name in the same folder (`DSC01234.ARW` and `DSC01234.JPG`) are copied into the same destination folder, with the same numbered suffix if the name is already taken, so a pair never gets split apart by a collision.
- **Folder numbering and file names repeat.** Cameras restart file numbers (`DSC00001`) after a card is formatted, so the same name can mean a different photo. The import log keys on source, path, size and modified time, and "looks the same" needs the same size as well as the same name, so a new card with reused names is not wrongly treated as already imported.
- **Camera identity.** The EXIF camera make, model and serial number identify the camera. The source key for a card uses the volume serial; the camera model is shown as the source's label ("Sony A7 IV card"), and Browse's camera filter already uses the same make and model.
- **Wrong clocks.** Cameras often have the wrong date or time zone. The date folder uses the date taken as the camera recorded it; the existing Fix dates tool corrects it afterwards. An option to shift a whole import by an offset is later work.
- **Write-protected cards** can be imported from; the tool never needs to write to a card unless the person uses Delete from the card, which says so when the card's lock switch is on.
- **Formatting the card** is left to the camera; Pixie never formats anything.

### Settings and memory

Remembered per source key: the last folder and layout used, and when it was last imported. A source with a known history opens with "New since last time" selected.

## Export

### The flow

1. **What.** Three choices (radio buttons): **The N selected** (the Working-on set), **Everything shown in Browse** (the current filters, N items), **The whole library** (N items). Shown with count and total size from the database (no files are read).
2. **How.** *Copies:* "Originals, as they are" (default), "Smaller copies" (longest side and quality, the same two settings as Email; originals untouched), "Convert HEIC to JPEG" (for sharing with people whose software cannot read HEIC). Videos are always copied as they are. *Pack as:* "Files in a folder" or "One zip file". *Layout inside:* "Year and date folders", "Same folders as in Pixie" (relative to the watched folder), "All in one folder"; *Names:* "Keep the file names" (default), "Put the date first (2024-03-05 14.22.10 name.jpg)" so files sort by time.
3. **Leave out.** Tick boxes: **Private photos** (ticked by default when the export is a zip or goes somewhere the person has called shared; *my call*, see below), **Extra copies of the same picture** (ticked: only one of each set of duplicates, using the hidden-duplicates rule Browse already uses), **Hidden photos** (ticked by default, #71).
4. **Also.** **Include a list of tags and people** (a `pixie-list.csv` with file, date, place, tags, people; the names exist only in Pixie's database, so this is how they leave with the photos). Off by default. Writing them into the photos themselves (XMP) is later work.
5. **Where.** For files: a folder picker, offering to make a new subfolder named `Pixie export 2026-11-05`. For a zip: a Save dialog for the file name. Free space on the destination is checked against the estimated size before anything starts ("This needs about 6.1 GB; D: has 4.2 GB free").
6. **Export** (the one primary button). Progress, throughput and time left come from the Processing screen; Stop works.
7. **Result.** "Exported 1,204 photos (6.1 GB) to D:\Export." with **Show in Explorer / Finder**, and a note of anything skipped or failed.

### Rules

- **Originals are only read.** Files mode copies like Import does (temporary name, verify, rename, never overwrite; collisions get numbers). Zip mode writes to `name.zip.part` and renames at the end, so a stopped export leaves no broken zip.
- **Zip details.** Photos and videos are already compressed, so they are stored without compression (fast); text files such as the CSV are deflated. Zip64 so files and archives over 4 GB work. Names are UTF-8. Names that would be too long or contain characters the destination cannot hold are shortened or replaced, and listed in the result. Optionally split into parts of a chosen size (for email or old drives) is later work.
- **Private photos.** An export is usually made to be shared. The default is to leave private photos out, with the count shown ("3 private photos left out"); the person can untick it for a personal backup. *(My call; confirm.)*
- **Resized copies** use the same code as Email's smaller copies (`pixie_image::thumbs::email_copy`), so they are upright and carry the right orientation.
- **Missing and online-only files.** A file that has gone from disk is listed and skipped. An online-only cloud file is read, which makes the cloud app download it; the delay shows on the Processing screen.
- **Never leaves partial output.** A failed or stopped export removes its temporary files.

## Architecture *(my call)*

- **Phase 1 needs no new crate for reading.** A source is an ordinary folder row with `kind = 'source'`:
  - **Database:** migration adding `folders.kind` (`'library'` or `'source'`), `import_batch` (id, started, source label, source key, destination folder, counts, bytes) and `import_log` (batch id, source key, path, size, modified, hash, destination photo path), indexed on (source key, path, size, modified). Browse can filter by batch (a facet, like the Sync facets).
  - **Scanner:** a *light* mode for source folders (rows, dates and dimensions only; nothing is handed to the analysis pipeline; no hashing).
  - **Queries:** every listing, count and facet that makes up "the library" leaves `kind = 'source'` folders out; the source's node lists only its own. One place decides this (a shared SQL fragment), with a test that fails if a listing forgets it.
  - **Lifecycle:** a source is added with the drive's key and label; removed (rows, thumbnails, picks) when the drive goes, on Done, and at start-up if its drive is gone.
- **`pixie-transfer` crate**, when the Inbox and phones arrive (phase 2), depending on `pixie-core`, `pixie-index`, `pixie-image`:
  - `trait Source { fn key(&self) -> SourceKey; fn label(&self) -> String; fn list(&self, cb: &mut dyn FnMut(SourceItem)) -> Result<()>; fn open(&self, item: &SourceItem) -> Result<Box<dyn Read>>; fn thumbnail(&self, item: &SourceItem) -> Option<Vec<u8>>; fn delete(&self, item: &SourceItem) -> Result<()>; }`, with `SourceItem { id, name, rel_path, size, modified, kind }`.
  - `ZipSource` (the entries of a zip, read with the `zip` crate; extracts into the Inbox), `WpdSource` (Windows, `#[cfg(windows)]`, COM on its own thread), `ImageCaptureSource` (macOS, phase 3, `#[cfg(target_os = "macos")]`), and the Inbox itself.
  - `plan_import(source items, import_log, library) -> Plan` (pure: new / kept before / looks the same), `run_import(...)`, `plan_export(...)`, `run_export(...)`.
- **Keep, Private, Discard** are commands in the registry (`photoCommands.ts`-style, with a context for what they act on) in the action bar and the photo right-click menu, visible only for photos that are in a source or the Inbox.
- **Safe copy is shared.** The part of `crates/pixie-index/src/copier.rs` that copies, verifies, never overwrites and numbers collisions is extracted into a function that takes a `Read` (today it takes a path), so Sync, Keep, the Inbox and Export all use it. Sync's tests must pass unchanged.
- **The `zip` crate** reads zip files for Import and writes them for Export (new dependency: run `just notices` and commit `THIRD-PARTY-NOTICES.md` and `ui/src/generated/third-party.json`).
- **Tauri commands** in `src-tauri`: `import_sources`, `import_open(source)`, `import_close(source)`, `import_keep(ids, folder, layout)`, `import_private(ids)`, `import_stop`, `import_delete_from_source`, `export_estimate`, `export_run`, `export_stop`; events for progress until the Processing screen shows them.
- **UI:** `ImportTool.svelte` (From, Look, Into, Result), `ExportTool.svelte`, view-model files `importer.ts` and `exporter.ts` with unit tests, a Browse tree node for each source, entries in `tools.ts` and `ToolsTab.svelte`. All controls get tooltips; buttons follow the colour rules; text never has a control in the middle of a sentence.

## Phasing

1. **Phase 1.** Browse in place for drives, cards and folders (Windows and macOS): the temporary source, the import log, Keep and Private, Done with this card, Delete from the card. The Private photos folder. Export as files and zip may follow as its own step.
2. **Phase 2.** The Inbox, with **zip files as its first source** (simpler than a phone, and it settles the Inbox's copy, hash, thumbnail, Keep and free-space parts). Held-for-review arrivals (#110) can use the Inbox.
3. **Phase 3.** Phones and cameras on Windows through WPD (Android MTP, iPhone, PTP). A spike first: list a real phone's `DCIM`, read a file, time 1,000 files.
4. **Phase 4.** iPhone and cameras on macOS (ImageCaptureCore), Android on macOS, delete-from-source for phones, safe eject, a "card inserted" prompt, XMP export, zip splitting, Live Photos as one item.

## Failure modes to test

- Card removed while browsing it, while a thumbnail is loading, or mid-Keep; phone locks mid-listing; destination full or on a network share that disappears.
- A source left over from a crash or a closed laptop: removed at start-up, with its rows, thumbnails and picks (nothing pointing at it remains).
- A source never appearing in the library's lists, counts, People, Trips, the map, Sync or Email sets (one test per listing).
- Two cards open at once; the same card plugged into another port (same key).
- Two files with the same name from different source folders (`100APPLE/IMG_0001.JPG`, `101APPLE/IMG_0001.JPG`); the same photo twice on the source; a photo already in the library under another name.
- Files with no date taken; dates in the future or 1970; names with non-Latin characters; paths near the 260-character limit.
- A 20,000-file source (listing and the grid stay responsive, slow USB included); a 4 GB video; a zip over 4 GB.
- The sync folder is offline or unset; Keep must say so and offer another folder.
- Private items never reach the sync folder, and are never analysed.
- Zip: an entry named `../../evil.jpg` or `C:/x.jpg` (nothing is written outside the Inbox); a zip bomb (a small file that expands enormously); an encrypted entry; a truncated or corrupt zip (the entries before the damage are kept, the rest listed); two entries with the same name; non-UTF-8 and non-Latin names; a zip over 4 GB; a zip inside a zip; the same zip imported twice (kept-before works).
- Inbox: full disk before copying (refused with the numbers); Keep into another drive (copy then delete, not a rename); an interrupted copy leaves nothing partial.
- Export of a library with hidden duplicates, private and hidden photos, and online-only files; the counts in the confirmation match what is written.

## Decided and open

**Decided with the user (2026-11-05):**
- Two ways to look at a source: browse in place first (phase 1), the Inbox later for phones and review (phase 2), with the same Keep, Private and Discard commands.
- A source's rows are removed when it goes away, not kept as missing.
- Delete from the source is a separate, red step after the result.
- Zip files are a source, expanded into the Inbox; zip only for now (7z, rar and tar.gz later); the Inbox, with zip as its first source, comes before phones.
- Phone import is Windows first; the Mac does not need it in the first release.

**Still open:**
1. Should **Export leave out private photos by default**? (Proposed: yes, with the count shown.)
2. Several sources open at once (proposed: yes, each its own node).
3. The Keep-into-the-master line: is a plain notice enough, or should Keep into the sync folder ask for confirmation?
