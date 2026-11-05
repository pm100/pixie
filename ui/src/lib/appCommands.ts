import { registerCommand } from './commands';
import type { TabId, TabInfo } from './tabs';
import type { ViewerMode } from './viewerPref';

/** What the commands need from the app: App.svelte builds one from its own handlers and state. The getters are called each time a menu is built, so labels and greyed-out items follow the state. */
export interface CommandContext {
  mac: boolean;
  /** The tabs that are shown, in order. */
  tabs: () => TabInfo[];
  pickTab: (id: TabId) => void;
  addFolders: () => void;
  rescan: () => void;
  syncNow: () => void;
  /** A sync folder is set. */
  canSync: () => boolean;
  paused: () => boolean;
  /** Something is running (or was paused) that Pause or Resume would act on: the same test that enables the button on the Processing screen. */
  canTogglePause: () => boolean;
  togglePause: () => void;
  openDataFolder: () => void;
  exit: () => void;
  pickAllShown: () => void;
  canPickAll: () => boolean;
  clearPicks: () => void;
  anyPicked: () => boolean;
  viewerMode: () => ViewerMode;
  setViewerMode: (m: ViewerMode) => void;
  showProcessing: () => void;
  toggleFullScreen: () => void;
  /** Ask Pixie. */
  toggleHelp: () => void;
  /** Settings, About (which lists the licences too). */
  showAbout: () => void;
  /** Focus is in a text box. */
  inTextField: () => boolean;
  editText: (op: 'selectAll' | 'copy') => void;
  /** What Copy would copy: the Picked photos, or else the one chosen (empty when there is none). */
  copyIds: () => number[];
  /** Exactly one photo (not a video) is what Copy would copy, so Copy image can work. */
  copyImageOk: () => boolean;
  copyToClipboard: (form: 'photo' | 'image' | 'paths') => void;
}

/** Registers Pixie's own commands (everything that is not a tab); returns one function that removes them all. */
export function registerAppCommands(ctx: CommandContext): () => void {
  const offs = [
    registerCommand({ id: 'file.addFolders', label: 'Add folder…', shortcut: 'CmdOrCtrl+O', where: [{ menu: 'File', group: 1 }], run: ctx.addFolders }),
    registerCommand({ id: 'file.rescan', label: 'Rescan folders', where: [{ menu: 'File', group: 1 }], run: ctx.rescan }),
    registerCommand({ id: 'file.syncNow', label: 'Sync now', where: [{ menu: 'File', group: 1 }], enabled: ctx.canSync, run: ctx.syncNow }),
    registerCommand({ id: 'file.pause', label: () => (ctx.paused() ? 'Resume background work' : 'Pause background work'), where: [{ menu: 'File', group: 2 }], enabled: ctx.canTogglePause, run: ctx.togglePause }),
    registerCommand({ id: 'file.openDataFolder', label: 'Open data folder', where: [{ menu: 'File', group: 3 }], run: ctx.openDataFolder }),
    registerCommand({ id: 'file.settings', label: 'Settings…', shortcut: 'CmdOrCtrl+,', where: [{ menu: 'File', group: 3 }], run: () => ctx.pickTab('settings') }),
    registerCommand({
      id: 'edit.selectAll', label: 'Select all', shortcut: 'CmdOrCtrl+A', where: [{ menu: 'Edit', group: 2 }],
      enabled: () => ctx.inTextField() || ctx.canPickAll(),
      run: () => { if (ctx.inTextField()) ctx.editText('selectAll'); else ctx.pickAllShown(); },
    }),
    registerCommand({ id: 'edit.clearPicks', label: 'Clear picks', where: [{ menu: 'Edit', group: 2 }], enabled: ctx.anyPicked, run: ctx.clearPicks }),
    registerCommand({ id: 'view.viewerFull', label: 'Open photos in the full window', where: [{ menu: 'View', group: 1 }], checked: () => ctx.viewerMode() === 'window', run: () => ctx.setViewerMode('window') }),
    registerCommand({ id: 'view.viewerWindow', label: 'Open photos in a separate window', where: [{ menu: 'View', group: 1 }], checked: () => ctx.viewerMode() === 'popout', run: () => ctx.setViewerMode('popout') }),
    registerCommand({ id: 'view.processing', label: 'Show Processing', where: [{ menu: 'View', group: 2 }], run: ctx.showProcessing }),
    registerCommand({ id: 'view.fullScreen', label: 'Full screen', shortcut: 'F11', where: [{ menu: 'View', group: 3 }], run: ctx.toggleFullScreen }),
    registerCommand({ id: 'help.ask', label: 'Ask Pixie', shortcut: 'F1', where: [{ menu: 'Help', group: 1 }], run: ctx.toggleHelp }),
    registerCommand({ id: 'help.about', label: 'About Pixie and licences', where: [{ menu: 'Help', group: 2 }], run: ctx.showAbout }),
  ];
  // Copy: the system's own copies text; this one also copies photos. A Mac keeps the system's until its clipboard code is written.
  if (!ctx.mac) {
    offs.push(registerCommand({
      id: 'edit.copy', label: 'Copy', shortcut: 'CmdOrCtrl+C', where: [{ menu: 'Edit', group: 1 }],
      enabled: () => ctx.inTextField() || ctx.copyIds().length > 0,
      run: () => { if (ctx.inTextField()) ctx.editText('copy'); else ctx.copyToClipboard('photo'); },
    }));
    offs.push(registerCommand({ id: 'edit.copyImage', label: 'Copy image', where: [{ menu: 'Edit', group: 1 }], enabled: ctx.copyImageOk, run: () => ctx.copyToClipboard('image') }));
    offs.push(registerCommand({ id: 'edit.copyPath', label: 'Copy path', where: [{ menu: 'Edit', group: 1 }], enabled: () => ctx.copyIds().length > 0, run: () => ctx.copyToClipboard('paths') }));
  }
  // On a Mac, Quit is in the app menu.
  if (!ctx.mac) offs.push(registerCommand({ id: 'file.exit', label: 'Exit', where: [{ menu: 'File', group: 4 }], run: ctx.exit }));
  return () => offs.forEach((off) => off());
}

/** One command per shown tab, Ctrl+1 to Ctrl+9 in tab order. Register again when the tabs change (People turns on or off). */
export function registerGoCommands(ctx: CommandContext): () => void {
  const offs = ctx.tabs().map((t, i) => registerCommand({
    id: `go.${t.id}`,
    label: t.label,
    shortcut: i < 9 ? `CmdOrCtrl+${i + 1}` : undefined,
    where: [{ menu: 'Go', group: 1 }],
    run: () => ctx.pickTab(t.id),
  }));
  return () => offs.forEach((off) => off());
}
