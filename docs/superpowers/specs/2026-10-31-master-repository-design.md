# Master repository: copy new photos into one shared folder, after review

Status: **proposed, flow agreed in chat on 2026-10-31, not built.** Issue: #31. Built together with #110 (private photos); the shared metadata (#105) comes later. Related: #13 (organize layout), #7 (duplicates), #8 (rescan), #99 and #68 (online-only cloud files), #71 (hide), #81 (share via cloud drive).

## Why

New photos arrive on a computer through OneDrive, iCloud or Google Drive. The family library lives on a shared drive (`P:\`) that other people read. Today the user copies new photos over by hand. The goal is that Pixie does the copying, safely, and that nothing reaches the shared drive without the user having looked at it. Some photos must never reach it (#110).

## Goals

- One **master folder** holding every shared original, laid out by date, that can be backed up and read by other people.
- New arrivals are **held for review** in batches. Only approved items are copied.
- Copying never touches an original, never overwrites, verifies every copy, and survives a full disk, an offline master and a restart.
- A visible log of what was copied, skipped and failed.

## Non-goals (first version)

- Moving instead of copying, or deleting originals afterwards.
- Auto-approve rules (a folder that is always shared). Everything is reviewed in batches at first.
- "Make private" for an item already in the master (#110 follow-up).
- Shared metadata such as tags and names (#105).

## No master, no feature

If the user has not defined a master folder, none of this feature's UI exists: no **New** node, no **Private** node or lock icon, no Share these / Make private buttons, no "Feeds the master" ticks, no copy status line, no log. Nothing is held, because without a master there is nothing to share. The only trace is the **Master folder** choice in Settings, which is how it is turned on. Removing the master turns it all off again; items held at that moment go back to being ordinary library items.

## Flow

### 1. Set up (Settings, Master folder)

(Everything below, from step 2 on, appears only once a master folder is set.)

- **Master folder:** the user picks a folder (e.g. `P:\`). Pixie checks it is writable and adds it as a watched folder.
- **Feeds the master:** each watched folder gets a tick. The master itself is never a source.
- **This computer copies new photos in:** on or off, per machine, off by default. Only one machine should have it on.
- **Layout:** `YYYY/YYYY-MM-DD/filename`, from Pixie's own date (so dates fixed with the Missing dates tool file correctly). Items with no date go to `Undated/`. *(Call: date folders, to match the When tree and #13.)*

### 2. Arrival

- A scan of a source finds an item whose content hash is not in the master.
- The item joins the library as usual, marked **New, not yet shared**. Nothing is copied. It is visible and taggable on this machine straight away. *(Call.)*
- An identical file already in the master is skipped and logged, never held.
- An online-only cloud file shows as "Waiting for download" and is not copied until it is on disk. Pixie does not force the download.

### 3. Review (Browse, **New** node with a count)

- The grid shows the held items, grouped by arrival day and source.
- **Share these** (the one primary button) approves the held batch. **Make private** acts on the selected photos.
- Usual path: select the few that are not to be shared, Make private, then Share these for the rest.
- A banner says how many are held and what each action does.
- Nothing leaves the New state without a choice, so a forgotten batch sits there and nothing leaks.

### 4. Copy (approved items only)

1. Written to the master under a temporary name.
2. Size and hash checked against the original.
3. Renamed into place. An existing file is never overwritten; a name clash with different content gets a suffix.
4. The library row then points at the master copy as the keeper; the source original is untouched. The Duplicates tool treats the master copy as the one to keep.
5. A status line shows progress ("Copying 40 of 212").

Disk full or master offline: copying pauses and says why, resumes by itself when the master is back, and a restart loses nothing.

### 5. Private

- A private photo is never copied. It keeps its tags and names locally.
- **Private** is a Browse node with a lock icon, separate from Hide (#71).
- For this feature the rule is simply that the copier reads approved items only. The shared metadata and shared face data rules are in #110 and apply when #105 is built.

### 6. Log (Settings, Master folder)

- The last runs: copied, skipped as already there, failed (with the reason), waiting for download.
- **Copy now** runs it on demand as well as the automatic run.

## Other readers of the master

If other Pixie installs watch the same master, they pick up the copies on their next scan. They will not see the owner's tags or names until #105.

## Open points

- Whether "New" items should show in the When and other trees or only under New (they show everywhere today, as any library item does).
- How a second machine learns which items the first has already approved or made private before #105 exists (until then, each machine reviews its own arrivals).

## Build order (proposed)

1. The held state, the **New** node and Share these / Make private (no copying yet).
2. The copier: layout, temporary name, verify, rename, resume, pause on full or offline.
3. The Settings page: master folder, feeds-the-master ticks, per-machine switch, log, Copy now.
