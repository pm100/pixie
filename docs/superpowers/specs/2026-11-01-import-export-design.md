# Import and Export: design

Status: **proposed, written 2026-11-01 from a short chat, not built.** Issue: #44. Items marked *(my call)* were not decided by the user. The two things the user asked for:

- **Import** from a variety of sources (a USB stick, a card, a directly connected phone, and so on) into a connected folder, the sync folder by default.
- **Export** a selection or everything, as files or zipped.

Related: #31 (Sync, the master folder; `docs/superpowers/specs/2026-10-31-master-repository-design.md`), #110 (private photos), #105 (database in a shared folder), #99 / #68 / #113 (online-only cloud files), the Processing screen plan (`docs/superpowers/plans/2026-11-01-activity-screen.md`), #71 (hide), #7 (duplicates), #13 (copy or move into folders).

## Intent

Getting photos in and out of Pixie should be as safe and as plain as Sync already is: originals are only read, every copy is checked before it counts, nothing is overwritten, and the person can see what happened. Today the only way in is to add a folder Pixie can already reach, and the only way out is Email or Print. A person with a camera card, a USB stick or a phone has no way in, and a person who wants a backup, a gift or a zip for someone else has no way out.

## Where it lives

The **Import/Export** tab (group `transfer` in `ui/src/lib/tools.ts`) gets two tools beside **Sync**: **Import** and **Export**. This settles "Where Import/export goes (#44)" from the tool-tabs design. Both work from the Tools "Working on" set where that makes sense (Export does; Import does not, its photos come from outside the library). Both report progress on the Processing screen once that exists (new kinds `Import` and `Export`); until then each shows its own simple progress line and a Stop button.

## Goals

- Import from a card, a USB stick, any folder (including a network share or cloud folder) and, later, a connected phone, with a preview and a clear idea of what is new.
- Never copy the same photo twice, even weeks later, even if the person deleted the first copy from the library.
- Export a selection, a filtered view or the whole library, as plain files or one zip, as originals or resized copies.
- Be safe: sources are only read (deleting from a source is a separate, explicit step), copies are verified, a stopped or interrupted run can be picked up again.
- Same rules as the rest of Pixie for private photos, hidden duplicates and online-only cloud files.

## Non-goals (first versions)

- Importing from the Apple Photos or Google Photos library, or from the cloud (that is Sync's other direction, and #81).
- Editing, renaming by pattern or fixing dates during import (Fix dates and Missing dates already exist after the photos are in).
- Showing RAW files and camera video formats (a separate piece of work); importing copies them along when asked.
- Burning discs, FTP/SFTP, and writing tags into the photos themselves.

## Import

### The flow

1. **From.** A list of places Pixie can import from, refreshed while the tool is open: cards and USB drives (with their label and size; a drive with a `DCIM` folder at the root is shown as a camera card), phones and cameras (phase 2), and **A folder…** for anything else. A source Pixie imported from before shows when it was last imported.
2. **Look.** Pixie reads the source's list of photos and videos (names, sizes, dates) and shows the count as it goes ("Looking… 3,200 found"), with Stop. It does not read the files yet. A summary follows: "412 photos and 18 videos, 6.1 GB, 3 March to 21 May. 380 are new."
3. **Choose.** A grid of thumbnails with ticks. New items are ticked; items the library already has are grouped as "Already in your library (N)" and unticked. Filters: New only (default), Photos, Videos, a date range. A **Private** checkbox per item and for a selection (see "Private photos and the sync folder").
4. **Into.** A label-then-control row (no controls in mid-sentence): *Folder:* one of Pixie's folders (default: the sync folder; otherwise the last one used, otherwise the first folder); *Layout:* "Year and date (2024/2024-03-05)" (default, the same as Sync), "Year and month", "One folder named…", "Same folders as on the source", "All in one folder".
5. **Import** (the one primary button, labelled with the count: "Import 380 items"). It copies, verifies, then leaves the folder to be read as usual: Pixie scans it, and the usual analysis follows (visible on the Processing screen).
6. **Result.** "Imported 362 photos and 18 videos (5.9 GB) in 3 min. 24 already in your library were skipped. 2 problems." with **Show them** (Browse filtered to this import, by its batch), the list of problems, and, only when the source supports it, **Delete from the card…** (red; see below).

### Rules

- **Originals on the source are only read.** Deleting is never part of the copy. After a run, the result screen can offer **Delete from the source…** for the items that were copied *and verified by size and hash*; it asks for confirmation with the count and says it cannot be undone (a card has no recycle bin). *(My call: a separate step after the result, not a tick box before the copy.)*
- **A copy is only put in place when it is complete and checked.** Each file is copied to a hidden temporary name inside the destination folder, read back and compared (size and hash), then renamed into its final place. A file that is already there is never overwritten: an identical file counts as done and the new copy is dropped; a different file with the same name makes the new copy take a numbered name (`IMG_0001 (2).jpg`). This is the same behaviour as Sync, and the code is shared (see Architecture).
- **The date folder comes from the copy.** The copy is made first under its temporary name, then its date taken is read from the copy (EXIF; otherwise the source's modified date), then it is renamed into `YYYY/YYYY-MM-DD`. This avoids reading each file twice from a slow source. *(My call.)*
- **Interruptions.** A card pulled out, a phone that locks, a full disk: the run pauses with the reason and what was done, partial files are removed, and a later run carries on (already-imported items are recognised by the import log below). One unreadable file (a bad card sector) is listed and skipped; it does not stop the run.
- **Online-only and slow sources.** Nothing skips online-only cloud files (decided for the Processing screen plan); an import from a cloud folder downloads the files it reads and the delay shows on the Processing screen.
- **Supported files only by default.** Photos and videos Pixie supports (`SUPPORTED_EXTENSIONS`, `VIDEO_EXTENSIONS`). Other files on the source (RAW, `.AAE` and `.xmp` sidecars, `.THM`) are counted ("14 other files, not imported") with a tick box **Also copy the other files that go with the photos** (off by default) that copies them beside their photo. Pixie's own and the system's folders are not looked into (`.thumbnails`, `Android/data`, `System Volume Information`, `$RECYCLE.BIN`, macOS `._` files).
- **Live Photos and pairs.** A photo and the short video that goes with it (iPhone Live Photo: `IMG_1234.HEIC` and `IMG_1234.MOV`) are shown and copied as they are, as two items with the same base name; treating them as one is later work.

### Knowing what is new

Reading every file just to find out whether it is already in the library would be slow over USB. Three levels, cheapest first:

1. **The import log** (new table `import_log`: source key, path on the source, size, modified time, hash, import time, batch). Same source, same path, same size and time means imported before, even if the library copy was later deleted. This makes "New since last time" instant. The source key is the volume's serial number or GUID with its label, or the device id for a phone.
2. **Looks the same as something in the library**: same file name (ignoring case) and same size as a library item. Only a hint: such items are unticked and labelled "Looks like IMG_0451.jpg in your library", and the person can tick them.
3. **Identical by hash** after copying: the library's stored hashes (the duplicate check's `full_hash`) are compared with the copy's. An identical copy is dropped and counted as "already in your library". This is the only automatic skip that depends on content.

### Private photos and the sync folder *(decision needed)*

The sync folder is the shared master (#31). A photo that goes straight into it is shared at once, with no chance to mark it private first, which is exactly what the held-for-review design in #110 exists to prevent. Proposal *(my call)*: when the destination feeds the master, the **Choose** step shows a **Private** tick for each item and for a selection. Private items are copied to the person's **Private photos folder** (a local folder chosen in the Sync settings, asked for once if none is set) and marked private, not to the master. If the destination is not the master, there is no Private tick. This needs the private-photo folder from #110's "Make private on an item already in the master" follow-up, so it may land after the first Import.

### Sources

| Source | Windows | macOS | Phase |
|---|---|---|---|
| Card, USB stick, any folder, network share, cloud folder | A drive letter or path | A volume under `/Volumes` or a path | 1 |
| Android phone (MTP) | Windows Portable Devices (WPD) | Not supported by the system; needs a libmtp-style helper | 2 (Windows), later (Mac) |
| iPhone / iPad | Appears as a portable device when Apple's drivers (Apple Devices app / iTunes) are installed; photos under `DCIM` | ImageCaptureCore | 2 (Windows), 3 (Mac) |
| Camera, through its card | A drive letter | A volume | 1 |
| Camera, through a USB cable (PTP/MTP) | WPD | ImageCaptureCore | 2 (Windows), 3 (Mac) |

Notes on phones (to be checked in a spike, not yet tried):
- A phone must be **unlocked** and set to **File transfer** (Android) or must **Trust this computer** (iPhone). The tool says so in words when it finds a device that will not list its files.
- Listing thousands of files over MTP can take minutes. The list streams in, with Stop. A phone's own thumbnail is used where it offers one; otherwise thumbnails are made only for the rows on screen.
- The iPhone setting *Transfer to Mac or PC: Automatic* converts HEIC to JPEG while transferring, which changes the files; with *Keep Originals* they come across as they are. If Optimise iPhone Storage is on, the full-size file may be in iCloud and the phone only has a smaller version. The tool shows a note when it detects either (a HEIC listed as JPEG, or files much smaller than their dimensions suggest).
- A phone may show the same photo in several folders (`DCIM/Camera`, an app's media folder). Items are matched by size and name, so the second sighting is "looks the same".

Detecting sources: Windows already has `GetLogicalDrives` / `GetDriveTypeW` in `src-tauri/src/drives.rs`, which leaves removable drives out on purpose for the first-run look-around. A new `removable_roots()` lists type 2 (removable) drives, plus any fixed or network drive with a `DCIM` folder, polled every two seconds while the Import tool is open. A "card inserted" prompt outside the tool is later work.

## Cameras

Cameras reach Pixie in two ways, and both are Import sources:

1. **Through the card** (a card reader or the computer's card slot). The card is a drive, so this is the phase 1 `FolderSource`; a drive with a `DCIM` folder is shown as a camera card, labelled with the camera's folder names (`100CANON`, `101NIKON`, `100MSDCF`).
2. **Through a USB cable to the camera** (phase 2). The camera appears as a portable device (PTP or MTP) through Windows Portable Devices, the same code as phones. Many cameras have a USB mode setting: *Mass storage* makes the camera appear as a drive (phase 1 works), *PTP / MTP* makes it a portable device (phase 2). The tool tells the person to try the other mode when a connected camera shows nothing. Tethered shooting and camera control are not part of this.

What is different from a phone:

- **RAW and the files that go with it.** Cameras write RAW (`.CR2`, `.CR3`, `.NEF`, `.ARW`, `.RAF`, `.ORF`, `.RW2`, `.DNG`), often as a RAW + JPEG pair with the same base name, and sidecars (`.XMP`, `.XML`, `.THM`). Pixie cannot show RAW yet, so it is a separate piece of work (#116). Until then, a camera source ticks **Also copy the other files that go with the photos** by default *(my call)*, so a RAW is copied beside its JPEG and is never left behind; the grid shows the JPEG and the count reads "412 photos, 412 RAW files copied with them". A RAW-only shooter would import files Pixie then cannot display: the tool warns about this before copying ("These are RAW files. Pixie can copy them but cannot show them yet.").
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

- New crate **`pixie-transfer`** (depends on `pixie-core`, `pixie-index`, `pixie-image`):
  - `trait Source { fn key(&self) -> SourceKey; fn label(&self) -> String; fn list(&self, cb: &mut dyn FnMut(SourceItem)) -> Result<()>; fn open(&self, item: &SourceItem) -> Result<Box<dyn Read>>; fn thumbnail(&self, item: &SourceItem) -> Option<Vec<u8>>; fn delete(&self, item: &SourceItem) -> Result<()>; }`, with `SourceItem { id, name, rel_path, size, modified, kind }`.
  - `FolderSource` (cards, USB, any folder; plain `std::fs`), `WpdSource` (Windows, phase 2, `#[cfg(windows)]`, COM on its own thread), `ImageCaptureSource` (macOS, phase 3, `#[cfg(target_os = "macos")]`).
  - `plan_import(source items, import_log, library) -> Plan` (pure: new / imported before / looks the same), `run_import(plan, dest, layout, stop, progress) -> Report`, `plan_export(...)`, `run_export(...)`.
- **Safe copy is shared.** The part of `crates/pixie-index/src/copier.rs` that copies, verifies, never overwrites and numbers collisions is extracted into a function that takes a `Read` (today it takes a path), so Sync, Import and Export all use it. Sync's tests must pass unchanged.
- **Export zip** uses the `zip` crate (new dependency: run `just notices` and commit `THIRD-PARTY-NOTICES.md` and `ui/src/generated/third-party.json`).
- **Database:** migration adding `import_batch` (id, started, source label, source key, destination folder, counts, bytes) and `import_log` (batch id, source key, path, size, modified, hash, destination photo path), indexed on (source key, path, size, modified). Browse can filter by batch (a facet, like the Sync facets).
- **Tauri commands** in `src-tauri`: `import_sources`, `import_look(source)`, `import_plan`, `import_run`, `import_stop`, `import_delete_from_source`, `export_estimate`, `export_run`, `export_stop`; events for progress until the Processing screen exists.
- **UI:** `ImportTool.svelte`, `ExportTool.svelte`, view-model files `importer.ts` and `exporter.ts` with unit tests, entries in `tools.ts` and `ToolsTab.svelte`. All controls get tooltips; buttons follow the colour rules; text never has a control in the middle of a sentence.

## Phasing

1. **Phase 1.** Import from drives and folders (Windows and macOS), the import log, Export as files and zip, Private handling for the sync folder if the private folder exists (otherwise Import into the sync folder warns that nothing is held back and offers a different folder).
2. **Phase 2.** Phones and cameras on Windows through WPD (Android MTP, iPhone, PTP). A spike first: list a real phone's `DCIM`, read a file, time 1,000 files.
3. **Phase 3.** iPhone and cameras on macOS (ImageCaptureCore), Android on macOS, delete-from-source for phones, safe eject, a "card inserted" prompt, XMP export, zip splitting, Live Photos as one item.

## Failure modes to test

- Card removed mid-copy; phone locks mid-listing; destination full or on a network share that disappears.
- Two files with the same name from different source folders (`100APPLE/IMG_0001.JPG`, `101APPLE/IMG_0001.JPG`); the same photo twice on the source; a photo already in the library under another name.
- Files with no date taken; dates in the future or 1970; names with non-Latin characters; paths near the 260-character limit.
- A 20,000-file source (listing and the grid stay responsive); a 4 GB video; a zip over 4 GB.
- The sync folder is offline or unset; Import must say so and offer another folder.
- Private items imported while the destination is the sync folder never reach it.
- Export of a library with hidden duplicates, private and hidden photos, and online-only files; the counts in the confirmation match what is written.

## Questions for the user

1. Import into the sync folder: is the per-item **Private** tick (copied to a local Private folder) the right way to keep private photos out of the master?
2. Should **Export leave out private photos by default**?
3. Delete from the source: a separate step after the result (proposed) or a tick box before the copy?
4. Phase 2 phone support: Windows first (WPD) with Mac later, or do you need a Mac phone import in the first release?
