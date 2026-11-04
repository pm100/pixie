# Undo: design

Issue #88 ("Undo for actions in Pixie"). Written from the issue and a look at how Pixie keeps its data; the person asked for what can and cannot be undone, and for core plumbing any new feature can use. Anything marked **(my call)** was not agreed and can be changed.

## Intent

Pixie never changes the files on disk, so most of what a person does in it is a choice recorded in Pixie's own database: a tag, a person's name, a pick, which copy to keep. These are cheap to get wrong (a stray click on Hide, a face moved to the wrong person) and tedious to put right by hand. Undo should be there, the same way, wherever they are in the app, and a new feature should get it by following a short recipe, not by inventing its own.

## Goals

- **Ctrl+Z and Ctrl+Y** (also Ctrl+Shift+Z) undo and redo the last change to Pixie's data, from any tab.
- A short **"Tagged 3 photos · Undo"** note after a change, so the person sees what happened and can take it back with one click; an **Undo** and **Redo** button that always says what it would do.
- **Core plumbing**: a feature that changes the database becomes undoable by wrapping its change in one call and registering its table once. No inverse code per feature.
- A clear, stated line between **what undo covers and what it does not** (see "What can and cannot be undone"), shown in the interface where it matters.
- **Safe**: an undo that would overwrite something that has changed since is refused with a message, never half done.

## Non-goals (for this round)

- Undo across a restart. The history lives for the session, and nothing about it is written to the database file.
- Undoing anything that touches files on disk or leaves Pixie (see the cannot list).
- Undoing background work: scans, analysis, thumbnails, duplicate finding, face grouping runs.
- A history list or "undo to here". One step at a time, up and down the stack.
- Undo for the person's settings (analysis switches, date rules, network rescan): they trigger background work, which cannot be undone.
- One timeline across the whole app. Each place with work of its own has its own history (see "One model: undo scopes"), and the keys act on the one you are in.

## Anti-goals (a failure even if it "works")

- An undo that silently loses newer work: a change made after the action, by the person or by background work, being overwritten.
- An undo that leaves the screen showing the old data.
- A feature that changes data and quietly cannot be undone, with nothing saying so.
- Recording that slows ordinary work (tagging 5,000 photos, a scan running).

## What can and cannot be undone

**Undoable (Pixie's own data about the photos, small and the person's own choices):**

| Change | Data | Notes |
|---|---|---|
| Tag and untag photos (user tags) | `user_tags` | Includes removing a tag from every photo |
| Pick, unpick, pick many, clear picks | `picks` | No "Undone" note for single picks (see Behaviour) |
| People: rename, hide and unhide, confirm, move faces, "not this person", new person from faces, merge a group into a person, split a group, ignore and unignore faces | `people`, `faces` (`person_id`, `pinned` and the other columns the commands write), `face_rejections` | A person who is merged away and brought back keeps the same id |
| Regroup people ("Tidy") | the same | Undoable while it is small enough (the size limit below); a big regroup is not |
| Keep this copy (Duplicates) | `photos.keep_pick` | Only the choice of which copy shows, not removing copies |
| Private and shared (#110) | `photos.share` | |
| Which folders feed the sync folder | `folders` (`feeds_master`) | |
| Name a trip | `trip_names` | |
| Contacts: add, edit, delete | `contacts` | |

**Undone through a scope of its own, not through the database:** a tool's own work that lives in the window, such as the Collage's page (every move, resize, crop, remove, Layout and Clear). Its edits are many, small and fast. It uses the same undo model as everything else (see "One model: undo scopes"): the Collage has no undo code of its own beyond saying when a step is finished.

**Cannot be undone, and why:**

| Change | Why |
|---|---|
| Removing duplicate copies (they go to the Recycle Bin) | Files on disk. Restore from the Recycle Bin. |
| Copying into the sync folder | Files on disk. |
| Saving a collage, emailing, printing, exporting | Leaves Pixie. |
| Adding or removing a watched folder | Removing deletes every photo's row and its analysis; adding starts a scan. Both already ask first. |
| Reset the library | Wipes everything; has its own confirmation. |
| Settings (analysis options, date rules, network rescan, email account, the sync folder choice) | They start or stop background work. |
| Scans, analysis, thumbnails, duplicate finding, grouping faces in the background | Not the person's choice; a Pixie job. |
| Any single change that touches more than 10,000 rows | Too big to keep; it still happens, and the "Undo" note says nothing for it. |

This differs from the issue's list of candidates in one place: **adding and removing folders** are not undoable here, because removing one deletes every photo's row and analysis. **Hiding photos (#71)** will be undoable when it is built, by registering its column.

## How it works

### The recording (Rust, `pixie-index`)

Changes are recorded **by the database itself**, so that no feature has to say how to reverse what it did.

- Each undoable table is listed once, in a registry (`UNDO_TABLES` in a new `crates/pixie-index/src/undo.rs`): the table, its primary key, and the columns whose changes are recorded (by default all of them; a table can list only the columns that matter, such as `photos`: `keep_pick`, `share`).
- The journal is a **temporary table** (`CREATE TEMP TABLE undo_journal`, with `temp_store` in memory), and the triggers are **temporary triggers** (`CREATE TEMP TRIGGER … ON main.<table>`). SQLite keeps temporary objects only for the life of the connection and never writes them to the database file, so there is no migration, no clean-up at start-up, nothing in the person's data from undo, and the history is gone when Pixie closes, which is the session-only behaviour wanted. They are created each time the database is opened for every registered table, so adding a table to the registry is all it takes. After an insert, update or delete a trigger writes one row into `undo_journal`: the action, the table, the kind of change, the primary key, the row before and the row after, as JSON with any blob written as a tagged hex string. Foreign-key actions fire the same triggers, so a person deleted and their faces set to no person are both recorded.
- A trigger records only when the change is made **inside an action**. An SQLite function `undo_actor()`, registered on the connection (needs rusqlite's `functions` feature, an existing crate), returns the id of the action being run **by the calling thread** (a thread-local), and NULL on every other thread. The triggers do nothing when it is NULL. This is what keeps a background thread's writes out of a person's action, without any locking.

### Making a change undoable (the recipe for a feature)

```rust
db.undoable("Tagged 3 photos", Undo::Note, || {
    // the change: any number of Db calls, on this thread
})
```

- `undoable(label, how, f)` starts an action, runs `f`, and on success keeps the action if it recorded any rows (a change that changed nothing leaves no step). The label is a past-tense sentence that says what happened, with counts using `plural`.
- `how` is `Undo::Note` (the "… · Undo" note is shown) or `Undo::Quiet` (on the stack, no note; for pick and unpick, which are frequent).
- A new action empties the redo side. The stack keeps the last 50 actions; older ones go.
- An action that records more than 10,000 rows is dropped from the stack (its changes stand) and the event says so.

The whole recipe for a new feature, in order:
1. Wrap the change in `db.undoable(...)`.
2. Add its tables (and the columns that matter) to `UNDO_TABLES`.
3. If the table has a cache or a revision number (as faces have `face_meta.rev`), add an after-undo step for it in the registry, because undo must **raise** a revision, never restore it.
4. Tell the window what to reload for the table (next section).
5. Write the round-trip test (below).
6. If the change cannot be undone, say so in a comment on the command (`// not undoable: <reason>`), and add it to the "cannot" table in this document.

### Undo and redo

`undo()` takes the newest action not yet undone and applies its rows in reverse, in one transaction; `redo()` applies them forwards. Before touching a row, the current row must equal what the action left behind (or be absent, for a delete). If any row does not match, nothing is changed and the person is told **"Can't undo: that has changed since."** (a background job moved the same face, for example). Applying an undo does not record itself. Undo and redo return the tables they changed and the label.

### One model: undo scopes (TypeScript)

Undo is one idea with several places it applies: the database (tags, people, picks), and the work a tool keeps in the window (the Collage page, and whatever comes next: a future editor, a map drawing, a form). They all use one interface, so the keys, the buttons and the note do not know which is which.

```ts
interface UndoScope {
  id: string;
  /** What Undo and Redo would do now, or null when there is nothing. */
  undoLabel: Readable<string | null>;
  redoLabel: Readable<string | null>;
  undo(): Promise<void>;
  redo(): Promise<void>;
}
```

- **The database scope** (`id: 'data'`) is an adapter over the backend: `undo()` and `redo()` call the Rust commands, the labels come from the `undo-state` event, and after each it runs the reload registry (below). It is always registered.
- **A tool's own scope** is made with `createHistory(...)`, a small class that holds a stack of steps (`{ label, undo, redo }`), a limit (100 **(my call)**) and the redo side, and **is** an `UndoScope`. A step is pushed with `history.push(step)`, quietly or with a note.
- **`snapshotHistory<T>({ get, set, equal })`** is the recipe for the commonest case, a tool whose work is one value (the collage document): the tool calls `history.commit('Moved a photo')` when a gesture or action is **finished**, and the helper compares the value with the one it holds, and if they differ records a step that restores the earlier or the later one with `set`. A drag is one step because the tool commits on pointer-up, not on every move. This is the UI's counterpart of `db.undoable`: the feature says what happened and when it is done, and writes no reversal code. Two commits with the same value record nothing.
- **Which scope the keys act on:** the shell holds the *active scope*. A tool that has one calls `activateScope(history)` while it is on screen (and `deactivateScope` when it goes); otherwise the active scope is `data`. Ctrl+Z, Ctrl+Y, the buttons and the note always act on the active scope only; they never fall through to another. When the active scope has nothing to undo the buttons are greyed out.
- **Keys and focus:** Ctrl+Z and Ctrl+Y (also Ctrl+Shift+Z) are handled in one place (`ui/src/lib/undo.ts`), not while typing in a field, where the field's own undo is wanted.
- **The reload registry** (for the database scope): a feature says "when table `faces` changes, run this" (`onTablesChanged(['faces', 'people'], reload)`), and after an undo or redo the backend event names the tables and the app runs those. The existing loaders (`loadTags`, `loadPeople`, `loadPicks`, the Browse refresh) are registered for their tables; nothing else needs to know.

The recipe for a **tool that keeps its own work**, in order: make a `snapshotHistory` over the tool's value; call `commit(label)` at the end of each gesture or action; call `activateScope` while the tool is on screen; write the round-trip test (commit, change, undo gives back the earlier value, redo the later, a no-change commit adds no step).

### Interface

- **Undo and Redo buttons** in the top right beside the Processing pill, as small icons with the tooltip "Undo: Tagged 3 photos (Ctrl+Z)" and greyed out when there is nothing; a visible name, so they are not icon-only mysteries. **(my call on the place)**
- **The note**: for 8 seconds after an `Undo::Note` action, bottom left: the label and an Undo button. A new action replaces it. After an undo: "Undone: Tagged 3 photos" with a Redo button.
- **Keys**: Ctrl+Z, Ctrl+Y, Ctrl+Shift+Z.

## Testing (a rule for every undoable change)

A shared test helper, `assert_undoable(db, label, setup, change)`: take a snapshot of every registered table, run `change` inside `undoable`, take a second snapshot; `undo()` must give back the first, `redo()` the second, and a second `undo()` the first again. Every undoable command has one test of this shape on its `Db` method. The window's model has its own tests: a history's limit, redo emptied by a new step, `snapshotHistory` ignoring a commit that changed nothing and making one step of a long gesture, the active scope being the only one the keys reach, and the database adapter calling the backend and running the reload registry. The plumbing has its own tests: a background thread's writes during an action are not recorded; a conflicting undo changes nothing and says so; an action over the row limit is dropped; the stack cap; redo is emptied by a new action; blobs and foreign-key cascades round-trip.

## Delivery in stages

1. **Plumbing and the first changes**: the journal, `undoable`, `undo`/`redo`, the events, the keys, the buttons and the note, with tags, picks and contacts undoable. (Proves the whole path.)
2. **People**: every people command, with the revision and caches handled.
3. **The rest of the undoable list**: keep this copy, private and shared, sync-folder feeds, trip names.
4. **The Collage on the generic history** (the first user of `snapshotHistory`; it gets no special code), and the AGENTS.md rules: a command that changes user data uses `undoable` or says why not, and a tool with its own work uses a scope.

## Left to the builder

Table and trigger SQL generation (from `PRAGMA table_info` and the registry), the JSON format of a row, the exact event names, how the thread-local is set and cleared (a guard that clears it on every exit), the note's component and where it sits, how the reload registry and the scope registry are stored, and the size of the 8-second note's animation. Every control needs a tooltip (AGENTS.md); counts use `plural`.
