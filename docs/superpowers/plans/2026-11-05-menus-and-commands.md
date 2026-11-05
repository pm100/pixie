# Menu bar and command registry (stages 1 and 2) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** One command registry, a shortcut handler for commands that are not in a menu, and a native menu bar (File, Edit, View, Go, Help) made from it, using only commands that exist today.

**Architecture:** `commands.ts` holds the registry and the pure `menuModel`. `shortcuts.ts` is pure key matching. `menuBar.ts` is a thin adapter from the model to `@tauri-apps/api/menu`. `appCommands.ts` registers Pixie's own commands from a context object of callbacks and getters that `App.svelte` supplies. A Svelte `$effect` in `App.svelte` rebuilds the native menu whenever a label or state it read changes.

**Tech Stack:** Svelte 5, TypeScript, vitest, Tauri 2 (`@tauri-apps/api` 2.12 menu API, no Rust).

**Spec:** `docs/superpowers/specs/2026-11-03-menus-and-commands-design.md` (issue #121). Stages 3 and 4 (right-click menus, moving controls) are a later plan.

## Global Constraints

- Never commit or push; leave every change uncommitted (AGENTS.md, CLAUDE.md). `ui/src/lib/Viewer.svelte` already has the user's uncommitted edits: do not touch it.
- No compiler or `svelte-check` warnings; do not silence them. Delete dead code.
- Counts use `plural` from `ui/src/lib/format.ts` when a count is more than one.
- Every control needs a `title`; menu items are named by their label and shortcut, so they need none. A toolbar control that mirrors a menu item keeps its `title`.
- Shortcuts are spelled `CmdOrCtrl+O` (Cmd on a Mac, Ctrl elsewhere).
- A menu item that does nothing, or looks available and fails, is a failure. Commands for unbuilt features (Undo, Redo, Group by date #98, Dark mode #96, Tag picked…, sort order) are not listed.
- Native text editing must keep working: Ctrl+A, Ctrl+C, Ctrl+V, Ctrl+X in a text box act on the text.
- Windows: only the `main` window gets a menu (the pop-out viewer window, loaded with `?viewer=1`, does not). Mac: one menu bar, so items that need the main window are disabled while the viewer window has focus.
- The Windows menu bar is always visible (my call, from the spec). Ask Pixie opens from Help and F1; the **?** button stays.
- Test style: vitest, `describe`/`it`, files beside the code (`ui/src/lib/x.ts`, `x.test.ts`); run with `cd ui && npx vitest run <file>`.

## Review Focus

- Two commands registered with one shortcut in the same place: `registerCommand` throws, so a typo cannot silently shadow another command.
- A command whose `label` or `enabled` function throws: the menu still builds; that item is left out (never a blank menu bar).
- A menu with no visible items (People tab off removes nothing from File, but Go loses "People"): the menu is omitted rather than shown empty; two separators never touch and a group never starts or ends with one.
- Ctrl+A in a text box (the folder name box, the tag box) selects the text and does not pick all photos.
- The command set changes while the menu is open or being built (a tab turning on): rebuilds are serialised, so the last state wins and no two builds interleave.

---

## File Structure

- Create `ui/src/lib/commands.ts`, `commands.test.ts`: registry, `menuModel`.
- Create `ui/src/lib/shortcuts.ts`, `shortcuts.test.ts`: key spelling and matching.
- Create `ui/src/lib/appCommands.ts`, `appCommands.test.ts`: Pixie's commands from a `CommandContext`.
- Create `ui/src/lib/menuBar.ts`, `menuBar.test.ts`: model to Tauri menu items (a pure `toSpec` plus a thin `applyMenu`).
- Modify `ui/src/App.svelte`: build the context, register, rebuild the menu, replace the F1 handler on `<svelte:window>` (line ~1419).
- Modify `src-tauri/capabilities/default.json`: menu and window permissions.

### Task 1: The command registry and menu model

**Files:**
- Create: `ui/src/lib/commands.ts`
- Test: `ui/src/lib/commands.test.ts`

**Interfaces:**
- Produces:
  ```ts
  export type MenuName = 'File' | 'Edit' | 'View' | 'Go' | 'Help';
  export type Placement = { menu: MenuName; group: number } | { context: string; group: number };
  export interface Command<T = unknown> {
    id: string;
    label: string | ((target: T) => string);
    shortcut?: string;
    where: Placement[];
    enabled?: (target: T) => boolean;
    visible?: (target: T) => boolean;
    checked?: (target: T) => boolean;   // makes it a check item (View pairs, Map)
    danger?: boolean;                   // last in its group
    run: (target: T) => void | Promise<void>;
  }
  export function registerCommand<T>(cmd: Command<T>): () => void;   // returns unregister; throws on a duplicate id or on a shortcut already used in one of its menus
  export function allCommands(): Command[];
  export function onCommandsChanged(fn: () => void): () => void;      // called after any register or unregister
  export function resetCommands(): void;                             // tests only
  export interface ItemModel { id: string; label: string; shortcut?: string; enabled: boolean; checked?: boolean }
  export interface MenuModel { menu: MenuName; groups: ItemModel[][] }
  export function menuModel(commands: readonly Command[]): MenuModel[];   // menus in File, Edit, View, Go, Help order; app commands take target `undefined`
  ```

- [ ] **Step 1: Write the failing tests** in `commands.test.ts` (call `resetCommands()` in `beforeEach`), one `it` each:
  - groups come out in `group` order with one separator between groups (a `MenuModel.groups` array per group), and registration order inside a group;
  - a function `label` is called and its result used;
  - `enabled: () => false` gives `enabled: false`; `visible: () => false` leaves the item out; a group left empty disappears (no empty group between two others);
  - a `danger` command is last in its group;
  - a command with two `where` entries appears in both menus with the same label and shortcut;
  - `registerCommand` returns a function that removes it, and `onCommandsChanged` fires on register and on unregister;
  - registering the same `id` twice throws; two commands with `shortcut: 'CmdOrCtrl+O'` in the same menu throw; the same shortcut in `File` and in a `{ context }` placement does not;
  - a command whose `label` throws is left out and the rest of the menu is intact; a menu with no items is not in the result;
  - `{ context }` placements never appear in `menuModel`.

- [ ] **Step 2: Run** `cd ui && npx vitest run src/lib/commands.test.ts`. Expected: FAIL, module not found.

- [ ] **Step 3: Implement** `commands.ts` as above. Keep the registry in a module-level `Map` (insertion order is the registration order). `menuModel` resolves each function with the target `undefined`.

- [ ] **Step 4: Run** the test file again. Expected: PASS. Leave uncommitted.

### Task 2: Shortcut matching

**Files:**
- Create: `ui/src/lib/shortcuts.ts`
- Test: `ui/src/lib/shortcuts.test.ts`

**Interfaces:**
- Consumes: `Command` from Task 1.
- Produces:
  ```ts
  export interface KeyLike { key: string; ctrlKey: boolean; metaKey: boolean; shiftKey: boolean; altKey: boolean }
  export function matches(e: KeyLike, shortcut: string, mac: boolean): boolean;   // 'CmdOrCtrl+Shift+Z', 'F1', 'CmdOrCtrl+,' ; letters compare case-insensitively
  export function isTextField(el: EventTarget | null): boolean;                    // input (text-like), textarea, select, contenteditable: same test as noBrowserMenu in App.svelte
  export function commandForKey(commands: readonly Command[], e: KeyLike & { target: EventTarget | null }, mac: boolean): Command | undefined;
  ```
  `commandForKey` returns the first enabled, visible command whose shortcut matches and which has **no** `{ menu }` placement (those are native accelerators, handled by the system), and returns `undefined` when `isTextField(e.target)` and the key has Ctrl or Cmd held (the text box keeps its own keys).

- [ ] **Step 1: Write the failing tests**: `matches` for Ctrl on Windows and Cmd on a Mac (`CmdOrCtrl+O` is false for Ctrl on a Mac), Shift as a required modifier (`CmdOrCtrl+Shift+Z` is false without Shift), `F1` with no modifiers (false when Ctrl is held), the comma key, case-insensitivity; `commandForKey` ignores a command that has a menu placement, ignores a disabled one, returns `undefined` for Ctrl+A with a textarea target, and finds a context-only command.
- [ ] **Step 2: Run** the file. Expected: FAIL.
- [ ] **Step 3: Implement** `shortcuts.ts`. `isTextField` must agree with `noBrowserMenu` (`input, textarea, select, [contenteditable="true"]`), so use the same selector via `closest`.
- [ ] **Step 4: Run** the file. Expected: PASS.

### Task 3: Pixie's commands

**Files:**
- Create: `ui/src/lib/appCommands.ts`
- Test: `ui/src/lib/appCommands.test.ts`

**Interfaces:**
- Consumes: `registerCommand`, `Command` (Task 1); `TabInfo`, `TabId` from `ui/src/lib/tabs.ts`.
- Produces:
  ```ts
  export interface CommandContext {
    mac: boolean;
    tabs: () => TabInfo[];                 // visible tabs, in order
    pickTab: (id: TabId) => void;
    addFolders: () => void;                // same picker as the welcome screen (`openPicker`, App.svelte ~1140)
    rescan: () => void;                    // scan every folder
    syncNow: () => void;                   // copyNow
    canSync: () => boolean;                // a master (sync) folder is set
    paused: () => boolean;
    togglePause: () => void;               // pauseResume(!paused)
    openDataFolder: () => void;
    exit: () => void;
    pickAllShown: () => void;  canPickAll: () => boolean;
    clearPicks: () => void;    anyPicked: () => boolean;
    viewerMode: () => 'window' | 'popout';  setViewerMode: (m: 'window' | 'popout') => void;   // use the app's ViewerMode type
    showProcessing: () => void;
    toggleFullScreen: () => void;
    toggleHelp: () => void;                // Ask Pixie
    showShortcuts: () => void;  showLicences: () => void;  reportProblem: () => void;  showAbout: () => void;
    inTextField: () => boolean;            // focus is in a text box
    editText: (op: 'selectAll') => void;   // document.execCommand
  }
  export function registerAppCommands(ctx: CommandContext): () => void;   // registers everything below, returns one unregister
  export function registerGoCommands(ctx: CommandContext): () => void;    // one command per visible tab, 'CmdOrCtrl+1'…, called again when tabs change
  ```
- Commands (id, label, shortcut, menu group): `file.addFolders` "Add folder…" `CmdOrCtrl+O` (File 1); `file.rescan` "Rescan folders" (1); `file.syncNow` "Sync now", enabled by `canSync` (1); `file.pause` label "Pause background work" or "Resume background work" by `paused()` (2); `file.openDataFolder` "Open data folder" (3); `file.settings` "Settings…" `CmdOrCtrl+,` (3, runs `pickTab('settings')`); `file.exit` "Exit" (4, only when `!mac`: on a Mac Quit is in the app menu); `edit.selectAll` "Select all" `CmdOrCtrl+A` (Edit 2: in a text field `editText('selectAll')`, otherwise `pickAllShown`; enabled when `inTextField() || canPickAll()`); `edit.clearPicks` "Clear picks" (Edit 2, enabled by `anyPicked`); `view.viewerFull` "Open photos in the full window" and `view.viewerWindow` "Open photos in a separate window" (View 1, `checked` follows `viewerMode()`); `view.processing` "Show Processing" (View 2); `view.fullScreen` "Full screen" `F11` (View 3); `go.<tabId>` with the tab's label and `CmdOrCtrl+<index+1>` for the first nine (Go 1); `help.ask` "Ask Pixie" `F1` (Help 1); `help.shortcuts` "Keyboard shortcuts", `help.licences` "Licences", `help.report` "Report a problem" (Help 2); `help.about` "About Pixie" (Help 3, only when `!mac`).
- View > Map: App.svelte keeps the map on or off in `view` state (line ~90, rendered at ~1546). Add `view.map` "Map" (`checked`) to View group 1 if its on/off can be read and set from App with no change to `PhotoMap`; if not, leave it out and say so in the handoff.

- [ ] **Step 1: Write the failing tests** with a fake `CommandContext` (plain object of `vi.fn()`): after `registerAppCommands`, `menuModel(allCommands())` has the File labels in the order above; `file.pause` reads "Resume background work" after the fake's `paused` flips; `file.exit` and `help.about` are absent when `mac` is true and present otherwise; Select all runs `editText('selectAll')` and not `pickAllShown` when `inTextField()`; the reverse otherwise; `registerGoCommands` with two tabs gives `CmdOrCtrl+1` and `CmdOrCtrl+2`, and calling the returned unregister then registering with three tabs gives three; the whole set registers with no duplicate shortcut in a menu (the registry would throw).
- [ ] **Step 2: Run** the file. Expected: FAIL.
- [ ] **Step 3: Implement** `registerAppCommands` and `registerGoCommands`; labels that follow state are functions reading the context.
- [ ] **Step 4: Run** the file plus `commands.test.ts`. Expected: PASS.

### Task 4: The native menu bar

**Files:**
- Create: `ui/src/lib/menuBar.ts`
- Test: `ui/src/lib/menuBar.test.ts`
- Modify: `src-tauri/capabilities/default.json`

**Interfaces:**
- Consumes: `MenuModel`, `Command`, `allCommands`, `menuModel` (Task 1).
- Produces:
  ```ts
  export type SpecItem =
    | { kind: 'item'; id: string; text: string; accelerator?: string; enabled: boolean; checked?: boolean }
    | { kind: 'separator' }
    | { kind: 'predefined'; item: 'Cut' | 'Copy' | 'Paste' };
  export interface SpecMenu { text: string; items: SpecItem[] }
  export function toSpec(model: MenuModel[], mac: boolean): SpecMenu[];   // pure
  export function applyMenu(spec: SpecMenu[], run: (id: string) => void, mac: boolean): Promise<void>;   // builds with @tauri-apps/api/menu, sets it; serialised
  ```
  `toSpec`: Windows menu names get a mnemonic (`&File`, `&Edit`, `&View`, `&Go`, `&Help`); groups are joined by one separator; Edit gets `predefined` Cut, Copy, Paste as its first group (before the registry's items) so text boxes keep the system's behaviour; no separator leads, trails or doubles.
  `applyMenu`: `MenuItem.new({ id, text, accelerator, enabled, action: () => run(id) })`, `CheckMenuItem.new` when `checked !== undefined`, `PredefinedMenuItem.new({ item })`, `Submenu.new`, `Menu.new`; then `setAsAppMenu()` on a Mac and `setAsWindowMenu()` elsewhere. Calls are chained on one promise so a second call waits for the first and the last wins.

- [ ] **Step 1: Write the failing tests** for `toSpec`: Windows names have `&`, a Mac's do not; Edit starts with the three predefined items then a separator then the registry's items; separators sit only between groups; accelerators and `enabled`/`checked` carry over; an empty model gives `[]`. (`applyMenu` is checked by running the app, Task 5.)
- [ ] **Step 2: Run** `npx vitest run src/lib/menuBar.test.ts`. Expected: FAIL.
- [ ] **Step 3: Implement** `toSpec` and `applyMenu`. Import the menu classes inside `applyMenu` (dynamic `import('@tauri-apps/api/menu')`) so tests that import `toSpec` do not need Tauri.
- [ ] **Step 4: Add permissions** to `src-tauri/capabilities/default.json`: `core:menu:default`, `core:window:allow-set-fullscreen`, `core:window:allow-is-fullscreen`. Keep `core:window:allow-close`. The capability covers the `viewer` window too, which is harmless because only the main window calls `applyMenu`.
- [ ] **Step 5: Run** the file. Expected: PASS.

### Task 5: Wire it into the app

**Files:**
- Modify: `ui/src/App.svelte`
- Test: manual run (below) plus the whole suite

**Interfaces:**
- Consumes: everything above.

- [ ] **Step 1: Build the `CommandContext`** in `App.svelte` from existing handlers: `pickTab`, `openPicker`/add folders (~1140), the scan-all path used by Settings, `copyNow` (~357), `pauseResume` (~1218; `paused()` is `scanPaused || analysis paused`, matching what the Pause button on the Processing pill shows, so find that state and reuse it, do not invent a second), `openDataFolder` from `api.ts`, `pickAllShown` and `pickAll.enabled` (~525), `clearPicks` and `picked.size`, `viewerMode`/`setViewerMode` (~148, ~164), `activityOpen = true` (~1200) for Show Processing, `toggleHelp` (~574), and `getCurrentWindow()` for `exit` (`close()`) and full screen (`isFullscreen`/`setFullscreen`). `showShortcuts`, `showLicences`, `showAbout`: open Settings on the page that already holds Licences/About (`settingsPage`, ~1597); Keyboard shortcuts has no page yet, so list it only if a page for it exists, otherwise leave the command out (no item that does nothing). `reportProblem` opens the repository's new-issue page with the system opener already used for links (find it; if none exists, leave it out). `mac` is `navigator.userAgent.includes('Mac')`.
- [ ] **Step 2: Register and rebuild.** An `$effect` registers `registerAppCommands(ctx)` and returns its unregister; a second `$effect` registers `registerGoCommands(ctx)` (it reads `tabs`, so it re-runs when People turns on or off); a third runs `const m = menuModel(allCommands())` synchronously (so Svelte tracks every state a label or `enabled` read) then `void applyMenu(toSpec(m, mac), (id) => runById(id), mac)`. Also call that rebuild from `onCommandsChanged`. Do nothing when running as the viewer window (`main.ts` mounts `App` only for the main window, so this holds by construction) and swallow and log a failure of `applyMenu` (the app works without a menu).
- [ ] **Step 3: Replace the F1 handler** on `<svelte:window>` (line ~1419) with `onkeydown` calling `commandForKey(allCommands(), e, mac)` and `preventDefault` + `run` when it finds one. F1 now belongs to Help > Ask Pixie as a native accelerator; keep `<svelte:window oncontextmenu={noBrowserMenu}>` as it is.
- [ ] **Step 4: Run the checks.** `cd ui && npx vitest run && npx svelte-check --tsconfig ./tsconfig.json` (expected: all pass, 0 errors, 0 warnings), then `cargo check --workspace --all-targets` with the separate target dir per `feedback_separate_cargo_target` (expected: clean).
- [ ] **Step 5: Run the app and look** (see the `run` skill; screenshot method in memory `reference_screenshot_tauri_window`). Check: the five menus show with their shortcuts; Pause/Resume changes its words after pausing; Go lists People only while face analysis is on and Ctrl+1 to Ctrl+8 follow the shown tabs; Ctrl+A in the tag box selects its text and in Browse picks all photos; Cut/Copy/Paste work in a text box; F1 opens and closes Ask Pixie; Open photos in follows Settings; the pop-out viewer window has no menu bar. State which of these were checked and which could not be (for example the Mac) in the handoff.
- [ ] **Step 6: Leave everything uncommitted** and tell the user which files changed.

## Self-review

- **Spec coverage:** registry, model, shortcut handler, text-box rule (Tasks 1-3, 5); menu bar with the five menus, Windows mnemonics, Mac app menu left to Tauri (Task 4: only `setAsAppMenu` adds our menus; we do not remove Tauri's), capability (Task 4). Left for stage 3-4 and said in the handoff: right-click, moving controls from Settings, Undo/Redo, Tag picked…, sort, Group by date, Dark mode, Keyboard shortcuts page if none exists. Component-local keys (Viewer, Grid, tool Escape handlers) stay where they are: they are modal and not app commands (my call; the spec asked to register them, which this plan does not).
- **Types:** `Command`, `Placement`, `MenuModel`, `ItemModel` (Task 1) are used unchanged in Tasks 2-4; `CommandContext` is defined once (Task 3) and built once (Task 5).
