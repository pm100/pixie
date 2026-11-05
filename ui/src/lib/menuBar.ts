import { MENU_ORDER, type ItemModel, type MenuModel } from './commands';

export type SpecItem =
  | { kind: 'item'; id: string; text: string; accelerator?: string; enabled: boolean; checked?: boolean }
  | { kind: 'separator' }
  | { kind: 'predefined'; item: 'Cut' | 'Copy' | 'Paste' };
export interface SpecMenu { text: string; items: SpecItem[] }

/** The system's own text editing items. Copy is not one of them: the app has its own, which copies photos too (and the text in a text box). */
const EDIT_BASICS: SpecItem[] = [
  { kind: 'predefined', item: 'Cut' },
  { kind: 'predefined', item: 'Paste' },
];

/** Groups of items as one flat list, one separator between groups and none at the ends. */
export function groupItems(groups: ItemModel[][]): SpecItem[] {
  const items: SpecItem[] = [];
  for (const group of groups) {
    if (items.length > 0) items.push({ kind: 'separator' });
    for (const i of group) items.push({ kind: 'item', id: i.id, text: i.label, accelerator: i.shortcut, enabled: i.enabled, checked: i.checked });
  }
  return items;
}

/** The menu model as plain data for the native menu. Edit always starts with the system's Cut and Paste, so text boxes keep their own behaviour. Windows menus get a mnemonic (Alt+F). */
export function toSpec(model: MenuModel[], mac: boolean): SpecMenu[] {
  const byName = new Map(model.map((m) => [m.menu, m]));
  const out: SpecMenu[] = [];
  for (const name of MENU_ORDER) {
    const m = byName.get(name);
    if (!m && name !== 'Edit') continue;
    const basics: SpecItem[] = name === 'Edit' ? [...EDIT_BASICS] : [];
    const rest = groupItems(m?.groups ?? []);
    const items = basics.length > 0 && rest.length > 0 ? [...basics, { kind: 'separator' } as SpecItem, ...rest] : [...basics, ...rest];
    out.push({ text: mac ? name : `&${name}`, items });
  }
  return out;
}

export interface ItemChange { id: string; text?: string; enabled?: boolean; checked?: boolean }

/** What can only be changed by building the menu again: the menus, the order of items and separators, their ids, accelerators and whether each is a check item. */
export function structureKey(spec: SpecMenu[]): string {
  return JSON.stringify(spec.map((m) => [m.text, m.items.map((i) => (i.kind === 'item' ? [i.id, i.accelerator ?? '', i.checked !== undefined] : i.kind === 'predefined' ? i.item : '-'))]));
}

/** What changed between two menus of the same structure: the items whose text, enabled or checked differ. */
export function itemChanges(prev: SpecMenu[], next: SpecMenu[]): ItemChange[] {
  const before = new Map<string, SpecItem>();
  for (const m of prev) for (const i of m.items) if (i.kind === 'item') before.set(i.id, i);
  const out: ItemChange[] = [];
  for (const m of next) {
    for (const i of m.items) {
      if (i.kind !== 'item') continue;
      const was = before.get(i.id);
      if (!was || was.kind !== 'item') continue;
      const c: ItemChange = { id: i.id };
      if (i.text !== was.text) c.text = i.text;
      if (i.enabled !== was.enabled) c.enabled = i.enabled;
      if (i.checked !== was.checked) c.checked = i.checked;
      if (Object.keys(c).length > 1) out.push(c);
    }
  }
  return out;
}

type Handle = { setText: (t: string) => Promise<void>; setEnabled: (e: boolean) => Promise<void>; setChecked?: (c: boolean) => Promise<void> };
let building: Promise<void> = Promise.resolve();
/** The menu now installed: its spec and the native item for each id. */
let current: { spec: SpecMenu[]; handles: Map<string, Handle> } | null = null;

/** Builds the native menu bar from the spec and sets it as the app menu on a Mac and the window menu elsewhere. Calls are queued, so a second one waits for the first and the last one wins.
 *  Installing a menu makes Windows re-lay out the window, which shows as the page jerking, and the labels follow state that changes every few seconds while work runs.
 *  So a spec with the same structure as the one installed only updates the items that changed (text, enabled, checked) and a spec the same does nothing. */
export function applyMenu(spec: SpecMenu[], run: (id: string) => void, mac: boolean): Promise<void> {
  const next = building.catch(() => {}).then(async () => {
    if (current && structureKey(current.spec) === structureKey(spec)) {
      for (const c of itemChanges(current.spec, spec)) {
        const h = current.handles.get(c.id);
        if (!h) continue;
        if (c.text !== undefined) await h.setText(c.text);
        if (c.enabled !== undefined) await h.setEnabled(c.enabled);
        if (c.checked !== undefined && h.setChecked) await h.setChecked(c.checked);
      }
      current.spec = spec;
      return;
    }
    const handles = new Map<string, Handle>();
    const { Menu, Submenu } = await import('@tauri-apps/api/menu');
    const submenus = [];
    for (const m of spec) submenus.push(await Submenu.new({ text: m.text, items: await nativeItems(m.items, run, handles) }));
    const menu = await Menu.new({ items: submenus });
    if (mac) await menu.setAsAppMenu(); else await menu.setAsWindowMenu();
    current = { spec, handles };
  });
  building = next;
  return next;
}

async function nativeItems(specs: SpecItem[], run: (id: string) => void, handles?: Map<string, Handle>) {
  const { MenuItem, CheckMenuItem, PredefinedMenuItem } = await import('@tauri-apps/api/menu');
  const items = [];
  for (const i of specs) {
    if (i.kind === 'separator') items.push(await PredefinedMenuItem.new({ item: 'Separator' }));
    else if (i.kind === 'predefined') items.push(await PredefinedMenuItem.new({ item: i.item }));
    else if (i.checked !== undefined) {
      const item = await CheckMenuItem.new({ id: i.id, text: i.text, accelerator: i.accelerator, enabled: i.enabled, checked: i.checked, action: () => run(i.id) });
      handles?.set(i.id, item);
      items.push(item);
    } else {
      const item = await MenuItem.new({ id: i.id, text: i.text, accelerator: i.accelerator, enabled: i.enabled, action: () => run(i.id) });
      handles?.set(i.id, item);
      items.push(item);
    }
  }
  return items;
}

/** Shows a right-click menu at the pointer (the native popup) and resolves when it has been shown. `run` gets the id of the item chosen. */
export async function popupMenu(groups: ItemModel[][], run: (id: string) => void): Promise<void> {
  if (groups.length === 0) return;
  const { Menu } = await import('@tauri-apps/api/menu');
  const menu = await Menu.new({ items: await nativeItems(groupItems(groups), run) });
  await menu.popup();
}
