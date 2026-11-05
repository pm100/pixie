import type { Command } from './commands';

export interface KeyLike { key: string; ctrlKey: boolean; metaKey: boolean; shiftKey: boolean; altKey: boolean }

/** Whether a key press is this shortcut ('CmdOrCtrl+Shift+Z', 'F1', 'CmdOrCtrl+,'). Cmd counts on a Mac and Ctrl elsewhere; Shift and Alt must match exactly. */
export function matches(e: KeyLike, shortcut: string, mac: boolean): boolean {
  const parts = shortcut.split('+');
  // A "+" key itself would end the shortcut with an empty part ("CmdOrCtrl++").
  const main = (parts[parts.length - 1] === '' ? '+' : parts[parts.length - 1]).toLowerCase();
  const mods = new Set(parts.slice(0, -1).map((p) => p.toLowerCase()));
  const wantCmd = mods.has('cmdorctrl') || (mac ? mods.has('cmd') : mods.has('ctrl'));
  const wantShift = mods.has('shift');
  const wantAlt = mods.has('alt');
  const haveCmd = mac ? e.metaKey : e.ctrlKey;
  const strayCmd = mac ? e.ctrlKey : e.metaKey;
  return e.key.toLowerCase() === main && haveCmd === wantCmd && !strayCmd && e.shiftKey === wantShift && e.altKey === wantAlt;
}

/** A text box, where the system keeps its own keys and menu. The same test as `noBrowserMenu` in App.svelte. */
export function isTextField(el: EventTarget | null): boolean {
  const t = el as { closest?: (s: string) => unknown } | null;
  return !!t && typeof t.closest === 'function' && !!t.closest('input, textarea, select, [contenteditable="true"]');
}

/** The command for a key press that the app must handle itself: not one with a menu item (the system handles its accelerator), and not a Ctrl or Cmd key in a text box. */
export function commandForKey(commands: readonly Command[], e: KeyLike & { target: EventTarget | null }, mac: boolean): Command | undefined {
  if ((e.ctrlKey || e.metaKey) && isTextField(e.target)) return undefined;
  return commands.find((c) => {
    if (!c.shortcut || c.where.some((p) => 'menu' in p)) return false;
    if (c.visible && !c.visible(undefined)) return false;
    if (c.enabled && !c.enabled(undefined)) return false;
    return matches(e, c.shortcut, mac);
  });
}
