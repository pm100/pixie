# Mobile companion app: design

Status: **idea, discussed in chat on 2026-11-01, not planned or built.** Issue: see the end of this file. Everything here comes later; no work is scheduled. This file is the starting point, not an agreed spec. Items marked *(my call)* were not decided by the user.

## Intent

A phone app that lets the person find photos the way Browse does (by date, place, people, Auto tags and trips), and mark photos private from where they are standing. It is a companion to the desktop app, not a second photo manager: the desktop does the heavy work (reading, face detection, tagging, duplicates, copying to the master) and the phone shows the result.

## First version: cloud files only

The first version works only on photos that are in the person's cloud drive (OneDrive, iCloud Drive, Google Drive). It has no direct connection to the PC, no relay and no server run by Pixie. *(My reading of "operates only on cloud files": photos that live only on the PC are not visible in the first version.)*

How it would work *(my call, building on #105 and #81)*:

- The desktop writes a small, merge-friendly data folder next to the photos in the cloud drive: the index (paths relative to the folder, dates, places), people names, user tags, Auto tags, and small thumbnails. This extends #105, which so far says thumbnails stay local; the thumbnails would be an opt-in extra.
- The phone reads that folder and browses it. It works with the cloud app's own sync, so it also works when the PC is off.
- Full-size photos are the files in the cloud drive itself. The phone opens them on demand through the cloud app; nothing is copied into the phone app.
- Private photos are left out of that data folder completely (no entry, no thumbnail, no names), as #110 requires.

## Mark private on the phone

- Select one or more photos, tap **Make private**. A lock icon shows, and a **Private** view lists them, as on the desktop. Undo is one tap.
- **It means the same as on the desktop and nothing more**: the photo is kept out of the shared master and out of shared metadata. Its tags and names stay local. It does not stay off the PC (decided by the user).
- The phone cannot reach the PC directly, so the mark is written as a small change file in the cloud data folder. The desktop must apply waiting marks **before** it copies anything to the master, so a photo marked private while the PC was off is never copied.
- *(My guess, to confirm later)*: a rule so that screenshots are private by default, set by the person, never guessed by the picture model, as in #110; the Private view behind the phone's fingerprint or face unlock.

## Goals

- Find a photo on the phone with the same filters as Browse, instantly and offline once the data folder has synced.
- Mark private quickly, and be certain the mark is respected.
- Add nothing the person has to set up beyond choosing the cloud folder.

## Non-goals (first version)

- Photos that are only on the PC (needs a direct connection, a relay, or an upload; see "Later").
- Editing, collages, printing, duplicates, tagging by the person on the phone, or naming people.
- Uploading the phone's own camera photos (the phone as a source), and freeing space on the phone.
- A server run by Pixie, or any account with Pixie.

## Anti-goals

- A private photo shown, named or counted anywhere in shared data.
- The phone reading or writing the desktop's own database, or two writers on one file (use separate small files per writer so cloud sync cannot conflict).
- Surprising downloads: opening a cloud-only photo downloads it, so the phone says so first when the file is large.

## Later (not designed)

- Reaching the PC directly: on the home network, from anywhere (a tool such as Tailscale, or a relay), or through the PC's own connection. This is what makes PC-only photos visible.
- The phone as a source: new camera photos flow to the PC and the master; they are "not yet synced" until marked private or copied.
- "Safe to delete from the phone": show which phone photos are verified in the master and free the space.
- Send from the phone to the PC for printing or a collage.
- On this day and Trips as a home screen.

## Constraints and unknowns

- Platform and technology are undecided: iOS, Android or both; Tauri mobile (the project already uses Tauri and Svelte, though the mobile targets were removed from `src-tauri/Cargo.toml`) or native apps.
- Cloud apps differ: how the phone sees a OneDrive, iCloud or Google Drive folder, and whether it can open a cloud-only file, depends on each provider's phone app and the platform's file access. This needs a spike per provider.
- The data folder needs a format and a merge rule (#105 has the same open questions); how big thumbnails make it for 100,000 photos (a guess: one or two gigabytes, to be measured against the real thumbnail cache).
- Marking private on the phone only works for photos the phone can identify in the PC's index: by relative path and content hash, as in #105.

## Related

#105 (database in a shared folder), #81 (share through a cloud drive), #110 (private photos), #31 (master repository), #99 and #68 (online-only cloud files), #113 (free up space on cloud drives), and the Sync design in `docs/superpowers/specs/2026-10-31-master-repository-design.md`.
