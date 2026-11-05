import { beforeEach, describe, expect, it, vi } from 'vitest';
import { allCommands, contextModel, menuModel, onCommandsChanged, registerCommand, resetCommands, type Command } from './commands';

const cmd = (id: string, over: Partial<Command> = {}): Command => ({ id, label: id, where: [{ menu: 'File', group: 1 }], run: () => {}, ...over });
const labels = (menu: string) => menuModel(allCommands()).find((m) => m.menu === menu)?.groups.map((g) => g.map((i) => i.label));

describe('commands', () => {
  beforeEach(() => resetCommands());

  it('orders groups by group number and items by registration', () => {
    registerCommand(cmd('b', { where: [{ menu: 'File', group: 2 }] }));
    registerCommand(cmd('a1'));
    registerCommand(cmd('a2'));
    expect(labels('File')).toEqual([['a1', 'a2'], ['b']]);
  });

  it('calls a function label', () => {
    registerCommand(cmd('x', { label: () => 'Undo Tagged 3 photos' }));
    expect(labels('File')).toEqual([['Undo Tagged 3 photos']]);
  });

  it('greys out a disabled command and leaves out an invisible one, dropping an emptied group', () => {
    registerCommand(cmd('on'));
    registerCommand(cmd('off', { enabled: () => false }));
    registerCommand(cmd('gone', { visible: () => false, where: [{ menu: 'File', group: 2 }] }));
    registerCommand(cmd('last', { where: [{ menu: 'File', group: 3 }] }));
    const file = menuModel(allCommands()).find((m) => m.menu === 'File')!;
    expect(file.groups.map((g) => g.map((i) => i.label))).toEqual([['on', 'off'], ['last']]);
    expect(file.groups[0][1].enabled).toBe(false);
    expect(file.groups[0][0].enabled).toBe(true);
  });

  it('puts a danger command last in its group', () => {
    registerCommand(cmd('bad', { danger: true }));
    registerCommand(cmd('fine'));
    expect(labels('File')).toEqual([['fine', 'bad']]);
  });

  it('shows a command placed in two menus in both, with the same label and shortcut', () => {
    registerCommand(cmd('m', { shortcut: 'CmdOrCtrl+M', where: [{ menu: 'File', group: 1 }, { menu: 'View', group: 1 }] }));
    const model = menuModel(allCommands());
    const items = model.flatMap((m) => m.groups.flat());
    expect(items).toHaveLength(2);
    expect(items[0]).toEqual(items[1]);
    expect(items[0].shortcut).toBe('CmdOrCtrl+M');
  });

  it('carries checked', () => {
    registerCommand(cmd('c', { checked: () => true }));
    expect(menuModel(allCommands())[0].groups[0][0].checked).toBe(true);
  });

  it('unregisters, and tells listeners about both changes', () => {
    const seen = vi.fn();
    onCommandsChanged(seen);
    const off = registerCommand(cmd('x'));
    off();
    expect(allCommands()).toHaveLength(0);
    expect(seen).toHaveBeenCalledTimes(2);
  });

  it('refuses a duplicate id', () => {
    registerCommand(cmd('x'));
    expect(() => registerCommand(cmd('x'))).toThrow();
  });

  it('refuses one shortcut twice in a menu, but allows it in another place', () => {
    registerCommand(cmd('a', { shortcut: 'CmdOrCtrl+O' }));
    expect(() => registerCommand(cmd('b', { shortcut: 'CmdOrCtrl+O' }))).toThrow();
    expect(() => registerCommand(cmd('c', { shortcut: 'CmdOrCtrl+O', where: [{ context: 'photo', group: 1 }] }))).not.toThrow();
  });

  it('leaves out a command whose label throws, and keeps the rest', () => {
    registerCommand(cmd('ok'));
    registerCommand(cmd('bad', { label: () => { throw new Error('boom'); } }));
    expect(labels('File')).toEqual([['ok']]);
  });

  it('omits a menu with no items and never lists context placements', () => {
    registerCommand(cmd('ctx', { where: [{ context: 'photo', group: 1 }] }));
    expect(menuModel(allCommands())).toEqual([]);
  });

  it('lists menus in File, Edit, View, Go, Help order', () => {
    registerCommand(cmd('h', { where: [{ menu: 'Help', group: 1 }] }));
    registerCommand(cmd('f'));
    expect(menuModel(allCommands()).map((m) => m.menu)).toEqual(['File', 'Help']);
  });
});

describe('contextModel', () => {
  beforeEach(() => resetCommands());
  const ctx = (id: string, over: Partial<Command<{ n: number }>> = {}): Command<{ n: number }> => ({ id, label: id, where: [{ context: 'photo', group: 1 }], run: () => {}, ...over });
  const labelsOf = (target: { n: number }) => contextModel(allCommands(), 'photo', target).map((g) => g.map((i) => i.label));

  it('lists only the commands for that kind, in groups, with a function label following the target', () => {
    registerCommand(ctx('a', { label: (t) => `Email ${t.n}` }));
    registerCommand(ctx('b', { where: [{ context: 'photo', group: 2 }] }));
    registerCommand(ctx('other', { where: [{ context: 'person', group: 1 }] }));
    registerCommand(cmd('menuOnly'));
    expect(labelsOf({ n: 3 })).toEqual([['Email 3'], ['b']]);
  });

  it('honours enabled, visible and danger against the target', () => {
    registerCommand(ctx('bad', { danger: true }));
    registerCommand(ctx('many', { visible: (t) => t.n > 1 }));
    registerCommand(ctx('off', { enabled: (t) => t.n > 1 }));
    expect(labelsOf({ n: 1 })).toEqual([['off', 'bad']]);
    expect(contextModel(allCommands(), 'photo', { n: 1 })[0][0].enabled).toBe(false);
    expect(labelsOf({ n: 2 })).toEqual([['many', 'off', 'bad']]);
  });

  it('leaves out a command that throws, and gives nothing for a kind with no commands', () => {
    registerCommand(ctx('ok'));
    registerCommand(ctx('boom', { label: () => { throw new Error('x'); } }));
    expect(labelsOf({ n: 1 })).toEqual([['ok']]);
    expect(contextModel(allCommands(), 'face', { n: 1 })).toEqual([]);
  });
});
