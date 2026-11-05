/** The command registry: everything Pixie can do, named once. The menu bar (and later the right-click menus and the
 *  shortcuts) are made from this list, so a new feature registers its commands and they appear in the right places. */

export type MenuName = 'File' | 'Edit' | 'View' | 'Go' | 'Help';
export type Placement = { menu: MenuName; group: number } | { context: string; group: number };

export interface Command<T = unknown> {
  id: string;
  label: string | ((target: T) => string);
  /** 'CmdOrCtrl+O': Cmd on a Mac, Ctrl elsewhere. */
  shortcut?: string;
  where: Placement[];
  enabled?: (target: T) => boolean;
  visible?: (target: T) => boolean;
  /** Makes it a check item (a View choice that is on or off). */
  checked?: (target: T) => boolean;
  /** Last in its group. */
  danger?: boolean;
  run: (target: T) => void | Promise<void>;
}

export interface ItemModel { id: string; label: string; shortcut?: string; enabled: boolean; checked?: boolean }
export interface MenuModel { menu: MenuName; groups: ItemModel[][] }

export const MENU_ORDER: readonly MenuName[] = ['File', 'Edit', 'View', 'Go', 'Help'];

const registry = new Map<string, Command>();
const listeners = new Set<() => void>();

const placeKey = (p: Placement) => ('menu' in p ? `menu:${p.menu}` : `context:${p.context}`);

function changed() { for (const fn of [...listeners]) fn(); }

/** Adds a command; returns the function that removes it. Throws on a repeated id, or on a shortcut already used in one of its places. */
export function registerCommand<T>(cmd: Command<T>): () => void {
  if (registry.has(cmd.id)) throw new Error(`Command ${cmd.id} is already registered`);
  if (cmd.shortcut) {
    const mine = new Set(cmd.where.map(placeKey));
    for (const other of registry.values()) {
      if (other.shortcut === cmd.shortcut && other.where.some((p) => mine.has(placeKey(p)))) {
        throw new Error(`${cmd.shortcut} is already used by ${other.id}, in the same place as ${cmd.id}`);
      }
    }
  }
  registry.set(cmd.id, cmd as Command);
  changed();
  return () => {
    if (registry.get(cmd.id) === (cmd as Command)) { registry.delete(cmd.id); changed(); }
  };
}

export function allCommands(): Command[] { return [...registry.values()]; }

/** Called after any register or unregister. Returns the function that stops listening. */
export function onCommandsChanged(fn: () => void): () => void {
  listeners.add(fn);
  return () => { listeners.delete(fn); };
}

/** For tests only. */
export function resetCommands() { registry.clear(); listeners.clear(); }

/** One command resolved against a target, or null when it is left out (not visible, or a function threw, so one bad command cannot blank a menu). */
function resolve(c: Command, target: unknown): ItemModel | null {
  try {
    if (c.visible && !c.visible(target)) return null;
    const item: ItemModel = {
      id: c.id,
      label: typeof c.label === 'function' ? c.label(target) : c.label,
      enabled: c.enabled ? c.enabled(target) : true,
    };
    if (c.shortcut) item.shortcut = c.shortcut;
    if (c.checked) item.checked = c.checked(target);
    return item;
  } catch { return null; }
}

/** Groups items by their group number, in order, a danger command last in its group, dropping empty groups. */
function grouped(entries: Map<number, { item: ItemModel; danger: boolean }[]>): ItemModel[][] {
  return [...entries.entries()]
    .sort((a, b) => a[0] - b[0])
    .map(([, g]) => [...g.filter((x) => !x.danger), ...g.filter((x) => x.danger)].map((x) => x.item))
    .filter((g) => g.length > 0);
}

function collect(commands: readonly Command[], match: (p: Placement) => boolean, target: unknown): ItemModel[][] {
  const byGroup = new Map<number, { item: ItemModel; danger: boolean }[]>();
  for (const c of commands) {
    for (const p of c.where) {
      if (!match(p)) continue;
      const item = resolve(c, target);
      if (!item) continue;
      const g = byGroup.get(p.group) ?? [];
      g.push({ item, danger: !!c.danger });
      byGroup.set(p.group, g);
    }
  }
  return grouped(byGroup);
}

/** The structure of each menu: groups (the menu draws a separator between them) of resolved items. App commands have no target, so functions get `undefined`. */
export function menuModel(commands: readonly Command[]): MenuModel[] {
  const out: MenuModel[] = [];
  for (const menu of MENU_ORDER) {
    const groups = collect(commands, (p) => 'menu' in p && p.menu === menu, undefined);
    if (groups.length > 0) out.push({ menu, groups });
  }
  return out;
}

/** The groups of one right-click menu (`kind` is the `context` name: 'photo'), resolved against what was clicked. */
export function contextModel<T>(commands: readonly Command[], kind: string, target: T): ItemModel[][] {
  return collect(commands, (p) => 'context' in p && p.context === kind, target);
}
