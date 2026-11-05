import { beforeEach, describe, expect, it, vi } from 'vitest';
import { allCommands, menuModel, resetCommands } from './commands';
import { registerAppCommands, registerGoCommands, type CommandContext } from './appCommands';

function fake(over: Partial<CommandContext> = {}): CommandContext {
  return {
    mac: false,
    tabs: () => [{ id: 'browse', label: 'Browse' }, { id: 'settings', label: 'Settings' }],
    pickTab: vi.fn(), addFolders: vi.fn(), rescan: vi.fn(), syncNow: vi.fn(), canSync: () => true,
    paused: () => false, canTogglePause: () => true, togglePause: vi.fn(), openDataFolder: vi.fn(), exit: vi.fn(),
    pickAllShown: vi.fn(), canPickAll: () => true, clearPicks: vi.fn(), anyPicked: () => true,
    viewerMode: () => 'window', setViewerMode: vi.fn(),
    showProcessing: vi.fn(), toggleFullScreen: vi.fn(), toggleHelp: vi.fn(),
    showAbout: vi.fn(),
    inTextField: () => false, editText: vi.fn(),
    ...over,
  };
}
const find = (id: string) => allCommands().find((c) => c.id === id)!;
const labels = (menu: string) => menuModel(allCommands()).find((m) => m.menu === menu)?.groups.map((g) => g.map((i) => i.label));

describe('appCommands', () => {
  beforeEach(() => resetCommands());

  it('lists the File menu in order', () => {
    registerAppCommands(fake());
    expect(labels('File')).toEqual([
      ['Add folder…', 'Rescan folders', 'Sync now'],
      ['Pause background work'],
      ['Open data folder', 'Settings…'],
      ['Exit'],
    ]);
  });

  it('words Pause by the state', () => {
    let paused = false;
    registerAppCommands(fake({ paused: () => paused }));
    expect(find('file.pause').label).toBeTypeOf('function');
    paused = true;
    expect(labels('File')![1]).toEqual(['Resume background work']);
  });

  it('greys Sync now without a sync folder', () => {
    registerAppCommands(fake({ canSync: () => false }));
    expect(menuModel(allCommands())[0].groups[0][2].enabled).toBe(false);
  });

  it('leaves Exit to the Mac app menu', () => {
    registerAppCommands(fake({ mac: true }));
    expect(allCommands().some((c) => c.id === 'file.exit')).toBe(false);
    resetCommands();
    registerAppCommands(fake({ mac: false }));
    expect(allCommands().some((c) => c.id === 'file.exit')).toBe(true);
  });

  it('greys Pause when nothing is running', () => {
    registerAppCommands(fake({ canTogglePause: () => false }));
    expect(menuModel(allCommands())[0].groups[1][0].enabled).toBe(false);
  });

  it('Select all selects the text in a text box, and picks all photos elsewhere', () => {
    const ctx = fake({ inTextField: () => true });
    registerAppCommands(ctx);
    find('edit.selectAll').run(undefined);
    expect(ctx.editText).toHaveBeenCalledWith('selectAll');
    expect(ctx.pickAllShown).not.toHaveBeenCalled();

    resetCommands();
    const ctx2 = fake({ inTextField: () => false });
    registerAppCommands(ctx2);
    find('edit.selectAll').run(undefined);
    expect(ctx2.pickAllShown).toHaveBeenCalled();
    expect(ctx2.editText).not.toHaveBeenCalled();
  });

  it('checks the viewer choice that is on', () => {
    registerAppCommands(fake({ viewerMode: () => 'popout' }));
    expect(find('view.viewerFull').checked!(undefined)).toBe(false);
    expect(find('view.viewerWindow').checked!(undefined)).toBe(true);
  });

  it('gives each shown tab a Ctrl+number command, and follows the tabs', () => {
    const ctx = fake();
    const off = registerGoCommands(ctx);
    expect(menuModel(allCommands()).find((m) => m.menu === 'Go')!.groups[0].map((i) => [i.label, i.shortcut])).toEqual([
      ['Browse', 'CmdOrCtrl+1'], ['Settings', 'CmdOrCtrl+2'],
    ]);
    find('go.settings').run(undefined);
    expect(ctx.pickTab).toHaveBeenCalledWith('settings');
    off();
    expect(allCommands()).toHaveLength(0);
    registerGoCommands(fake({ tabs: () => [{ id: 'browse', label: 'Browse' }, { id: 'faces', label: 'People' }, { id: 'settings', label: 'Settings' }] }));
    expect(allCommands()).toHaveLength(3);
  });

  it('registers everything without two commands taking one shortcut', () => {
    expect(() => { registerAppCommands(fake()); registerGoCommands(fake()); }).not.toThrow();
  });
});
