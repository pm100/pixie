# Menu bar, right-click menus and the command registry: design

Brainstormed with the person on 2026-11-03. Anything marked **(my call)** was not agreed and can be changed. This builds on the undo design (`2026-11-03-undo-design.md`): Undo and Redo are the first commands that want a menu, and undo labels ("Undo Tagged 3 photos") are shown in it.

## Intent

Pixie's tabbed interface keeps tasks simple, but controls that belong to no single task (add a folder, pause background work, how Browse looks, undo) end up crammed into odd places, and the shortcuts that exist are scattered through the components. The macOS build needs a menu bar anyway. The fix is not a menu bar on its own but **one list of everything Pixie can do** (the *command registry*), from which the menu bar, the right-click menus and the keyboard shortcuts are all made. A new feature registers its commands and they appear in the right places, with no one deciding where to cram a button.

## Goals

- A **native menu bar** (File, Edit, View, Go, Help) on Windows and macOS, made from the registry.
- **Right-click menus** on photos, people, faces, Browse tree nodes, duplicate groups, collage frames and Processing rows, made from the same registry, with the same words and shortcuts as the menus.
- **Shortcuts in one place**: a command declares its key once; the menu shows it and the app handles it.
- A **rule for where a control lives**, so new knobs have an obvious home.
- Menus that **teach**: every shortcut visible in the menu, every command named the same everywhere.

## Non-goals (for this round)

- A command palette (Ctrl+K) or Ask Pixie. The registry is what they will use; the interface is not built here.
- Letting the person change shortcuts.
- Hiding the menu bar on Windows (a Settings choice, "Hide the menu bar; Alt shows it", is the likely next step **(my call: visible by default)**).
- Menus for the separate viewer window on Windows (it keeps its own keys).
- Moving rare, heavy settings out of Settings (folders, analysis, email, trips stay there).
- Per-tool menus (a "Collage" menu): a tool's own controls stay on its page.

## Anti-goals (a failure even if it "works")

- A command that exists only in a menu, with no way to reach it from the page for something done often. The menu is a second route, not the only one.
- Two names or two shortcuts for the same thing.
- A menu item that does nothing, or looks available and fails. Unbuilt features are not listed.
- Native text editing broken: Ctrl+Z, Ctrl+C, Ctrl+A must still work in a text box.
- A control moved out of where it is used so often that the page gets harder to use.

## Where a control lives

| It does this | It goes here |
|---|---|
| Acts on the photos being worked on (tag, pick, email, print, make private) | The tool tab, **and** the right-click menu of those photos |
| Changes how Browse looks (map, grouping, sort order, how photos open) | The **View** menu, with a small toggle on the page where it is used often |
| Is a rare, heavy setting (folders, analysis, email, trips) | **Settings** |
| Is an app-wide command (add folder, pause background work, undo) | A **menu**, with a shortcut |
| Belongs to one tool's document (the collage's gear, Layout) | That tool's own page |

## The command registry (TypeScript)

`ui/src/lib/commands.ts`. A command is registered once, usually where its feature lives, and unregistered when the feature goes (a tool that is not on screen has no commands).

```ts
interface Command<T = unknown> {
  id: string;                                  // 'photo.tag'
  label: string | ((target: T) => string);     // dynamic: 'Undo Tagged 3 photos'
  shortcut?: string;                           // 'CmdOrCtrl+O' (Cmd on a Mac, Ctrl elsewhere)
  where: Placement[];                          // { menu: 'Edit', group: 2 } | { context: 'photo', group: 1 }
  enabled?: (target: T) => boolean;            // greyed out when false
  visible?: (target: T) => boolean;            // left out when false
  danger?: boolean;                            // last in its group, marked
  run: (target: T) => void | Promise<void>;
}
registerCommand(cmd): () => void               // returns the unregister function
```

- **A target** is what the command is about: the photos (ids) for a photo command, a person, a face, a tree node, a duplicate group, a frame, a Processing row, or nothing for an app command.
- **The model**: a pure function `menuModel(commands, state)` turns the registry into the structure of each menu (groups with separators, labels, shortcuts, enabled and visible resolved), and `contextModel(commands, kind, target)` does the same for one right-click. Both are tested without any window.
- **Shortcuts**: one handler for the app (not in text fields, see below) looks a key up in the registry. A shortcut that is also shown in a native menu is registered with the menu item, so the system shows it right; the registry stops two commands taking one shortcut in the same place (a test).

## The menu bar

Made at start-up with Tauri's JavaScript menu API (`@tauri-apps/api/menu`, so no Rust is needed; the capability for it is added), set as the window menu on Windows and the app menu on a Mac, and **updated** (text, enabled, checked) when labels or states change: the undo labels, the People tab being off, whether anything is picked, and so on.

| Menu | Commands (shortcut) |
|---|---|
| **File** | Add folder… (Ctrl+O), Rescan folders, Sync now, Pause background work or Resume (one item, its words follow the state), Open data folder, Settings… (Ctrl+,), Exit (Windows; on a Mac Quit is in the app menu) |
| **Edit** | Undo *what it would undo* (Ctrl+Z), Redo *what it would redo* (Ctrl+Y and Ctrl+Shift+Z), Cut, Copy, Paste (the system's, for text boxes), Select all (Ctrl+A: in Browse and the tool lists it picks all the shown photos; in a text box it selects the text), Clear picks, Tag picked… |
| **View** | Map, Group by date (when #98 is built), Sort newest first and oldest first (a pair), Open photos in the full window or in a separate window (a pair), Show Processing, Full screen (F11), Dark mode (when #96 is built) |
| **Go** | Browse, People (only while face analysis is on), Send, Create, Organize, Clean up, Import/Export, Settings (Ctrl+1 to Ctrl+8, following the tabs that are shown) |
| **Help** | Ask Pixie (F1, when it exists; until then not listed), Keyboard shortcuts, Licences, Report a problem (opens the issue page), About Pixie (on a Mac it is in the app menu) |

On a Mac the app menu (About, Settings, Hide, Quit) and the window items are Tauri's, added to rather than replaced **(my call)**. The text-editing items keep the system's own behaviour so text boxes work.

## Right-click menus

Made at the moment of the click with Tauri's native popup (`Menu.popup()`), from `contextModel`. About eight items at most per menu, in groups, dangerous items last.

| Right-click on | Contents |
|---|---|
| **Photo or selected photos in Browse** | Open, Pick or Unpick, Tag…, Make private or Make public, Show in folder, Open in the default player (videos only), Email…, Print…, Make a collage… |
| **A photo in a tool's list** (Tools, Collage list) | Open, Unpick (Picked only), Show in folder, Add to collage (in the Collage) |
| **Person card** | Open their photos, Rename, Hide or Show, Merge into…, Split group |
| **A face** | Move to…, Not this person, Ignore |
| **Browse tree node** (folder, tag, date, trip, person) | Show only this, Pick all in this, Rename (a tag or a trip), Open folder, Remove folder (dangerous, asks first) |
| **A duplicate group's copy** | Keep this copy, Show in folder, Move the others to the Recycle Bin (dangerous) |
| **A collage frame** | To front, To back, Crop, Remove (the collage's own commands, registered while it is on screen) |
| **A Processing row** | Pause or Resume, Show details |
| **A text box** | The system's own menu, left alone |

- **Selection**: right-clicking something that is not selected selects it first (and only it); right-clicking inside the selection acts on the whole selection.
- **Email…, Print…, Make a collage…** use *exactly the photos right-clicked*: one photo is the tool's "Current" photo; several become the **Picked** set (replacing what was picked, which Undo brings back **(my call)**) and the tool's tab is opened.
- **Keyboard**: the Menu key and Shift+F10 open the menu for the focused item.
- **The browser's own menu**: still switched off (as `noBrowserMenu` does now) except in text boxes; the debug build keeps it so the page can be inspected.

## Platform details and traps

- **Accelerators beat the page**: a native menu shortcut is taken before the page sees the key. For Ctrl+Z, Ctrl+Y and Ctrl+A the command checks whether a text box has the focus and, if so, hands the key to the text box (`document.execCommand('undo' | 'redo' | 'selectAll')`) instead of acting on the app.
- **A Mac has one menu bar for every window**: the commands run in the main window, and while the viewer window has the focus the items that need the main window are greyed **(my call)**.
- **Windows has a menu per window**: only the main window gets one.
- **Mnemonics** on Windows (`&File`) so Alt+F works; text and shortcuts shown by the system.
- **Menu text and tooltips**: AGENTS.md wants every control to have a tooltip; menu items carry their label and shortcut, which is their name; the toolbar toggles that mirror a menu item keep their `title`.

## Controls that move

- *Open photos in: the full window or a separate window* (Settings > Viewing) moves to **View**, and the Settings page keeps nothing for it.
- *Map: Display and Filter* in Browse stay on the page, and Map is also in **View** (the same command).
- *Pick all* and *Clear picks* in the Browse bar stay, and are also in **Edit**.
- The placeholder **?** becomes the **Help** menu's Ask Pixie when that exists; until then it stays as it is.
- The Processing pill stays; **View > Show Processing** opens the same screen.
- *Add folder* in Settings > Folders stays, and is also **File > Add folder…**.

## Delivery in stages

1. **The registry and the shortcut handler**, with `menuModel` and `contextModel` and their tests; the commands that already exist as keys (the viewer's, the grid's) are registered so that nothing is lost.
2. **The menu bar** (File, Edit, View, Go, Help) with the commands that exist today; Undo and Redo join as soon as the undo design lands.
3. **Right-click**: photos in Browse first (with Email, Print and Make a collage handing over the selection), then people, faces, the tree, duplicates, the collage frame and Processing rows.
4. **Moving the controls** listed above, and the AGENTS.md rule that a new feature registers its commands and says where each lives.

## Testing

`menuModel` and `contextModel` are pure: tests check that groups and separators come out in order, a dynamic label follows its target, `enabled` and `visible` are honoured, a dangerous command is last in its group, a command registered for two places appears in both with the same label and shortcut, an unregistered command goes, and that no two commands share a shortcut in one place. The shortcut handler is tested for text boxes (the key is left alone), modifiers (Cmd on a Mac, Ctrl elsewhere) and a key that matches nothing. The native pieces are thin adapters over the model and are checked by running the app on both platforms.

## Left to the builder

The exact shape of `where` and the group numbering; how the model is turned into Tauri `Menu`, `Submenu` and `MenuItem` objects and kept in step with state (including macOS's rules for the app menu); which keys the registry knows how to spell (`CmdOrCtrl`, `Shift`, `,`); how a right-click target is found for each kind of thing (a data attribute on the element, a handler per component, or both); the capability entry for the menu API; and the wording of any command not listed. Every control needs a tooltip (AGENTS.md); counts use `plural`.
