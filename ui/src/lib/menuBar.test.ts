import { describe, expect, it } from 'vitest';
import type { MenuModel } from './commands';
import { groupItems, itemChanges, structureKey, toSpec } from './menuBar';

const item = (id: string, over = {}) => ({ id, label: id, enabled: true, ...over });
const kinds = (items: { kind: string }[]) => items.map((i) => i.kind);

describe('toSpec', () => {
  const model: MenuModel[] = [
    { menu: 'File', groups: [[item('a', { shortcut: 'CmdOrCtrl+O' }), item('b', { enabled: false })], [item('c')]] },
    { menu: 'Edit', groups: [[item('sel', { shortcut: 'CmdOrCtrl+A' })]] },
    { menu: 'View', groups: [[item('v', { checked: true })]] },
  ];

  it('gives Windows menus a mnemonic and a Mac\'s none', () => {
    expect(toSpec(model, false).map((m) => m.text)).toEqual(['&File', '&Edit', '&View']);
    expect(toSpec(model, true).map((m) => m.text)).toEqual(['File', 'Edit', 'View']);
  });

  it('puts one separator between groups and none at the ends', () => {
    const file = toSpec(model, false)[0];
    expect(kinds(file.items)).toEqual(['item', 'item', 'separator', 'item']);
  });

  it('carries the accelerator, enabled and checked', () => {
    const file = toSpec(model, false)[0];
    expect(file.items[0]).toEqual({ kind: 'item', id: 'a', text: 'a', accelerator: 'CmdOrCtrl+O', enabled: true, checked: undefined });
    expect(file.items[1]).toMatchObject({ enabled: false });
    expect(toSpec(model, false)[2].items[0]).toMatchObject({ checked: true });
  });

  it('starts Edit with the systems Cut and Paste (Copy is the apps own where it can copy photos), then a separator, then the registry items', () => {
    const edit = toSpec(model, false)[1];
    expect(edit.items.map((i) => (i.kind === 'predefined' ? i.item : i.kind))).toEqual(['Cut', 'Paste', 'separator', 'item']);
    const mac = toSpec(model, true)[1];
    expect(mac.items.map((i) => (i.kind === 'predefined' ? i.item : i.kind))).toEqual(['Cut', 'Copy', 'Paste', 'separator', 'item']);
  });

  it('still has an Edit menu when no command is placed in it, and gives nothing for an empty model but that', () => {
    const spec = toSpec([], false);
    expect(spec.map((m) => m.text)).toEqual(['&Edit']);
    expect(kinds(spec[0].items)).toEqual(['predefined', 'predefined']);
  });
});

describe('groupItems', () => {
  it('flattens groups with one separator between and none at the ends', () => {
    expect(kinds(groupItems([[item('a')], [item('b'), item('c')]]))).toEqual(['item', 'separator', 'item', 'item']);
    expect(groupItems([])).toEqual([]);
  });
});

describe('updating a menu in place', () => {
  const model = (over = {}): MenuModel[] => [{ menu: 'File', groups: [[item('a', over), item('b')], [item('c', { checked: false })]] }];

  it('has the same structure when only text, enabled or checked change', () => {
    const before = toSpec(model(), false);
    const after = toSpec([{ menu: 'File', groups: [[item('a', { label: 'Resume', enabled: false }), item('b')], [item('c', { checked: true })]] }], false);
    expect(structureKey(after)).toBe(structureKey(before));
    expect(itemChanges(before, after)).toEqual([
      { id: 'a', text: 'Resume', enabled: false },
      { id: 'c', checked: true },
    ]);
  });

  it('reports no changes for the same menu', () => {
    expect(itemChanges(toSpec(model(), false), toSpec(model(), false))).toEqual([]);
  });

  it('has a different structure when an item comes or goes, or its accelerator changes', () => {
    const base = structureKey(toSpec(model(), false));
    expect(structureKey(toSpec([{ menu: 'File', groups: [[item('a')]] }], false))).not.toBe(base);
    expect(structureKey(toSpec(model({ shortcut: 'F5' }), false))).not.toBe(base);
  });
});
