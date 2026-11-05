import { describe, expect, it } from 'vitest';
import type { Command } from './commands';
import { commandForKey, isTextField, matches, type KeyLike } from './shortcuts';

const key = (k: string, over: Partial<KeyLike> = {}): KeyLike => ({ key: k, ctrlKey: false, metaKey: false, shiftKey: false, altKey: false, ...over });
const cmd = (id: string, shortcut: string, over: Partial<Command> = {}): Command => ({ id, label: id, shortcut, where: [{ context: 'photo', group: 1 }], run: () => {}, ...over });

describe('matches', () => {
  it('takes Ctrl on Windows and Cmd on a Mac for CmdOrCtrl', () => {
    expect(matches(key('o', { ctrlKey: true }), 'CmdOrCtrl+O', false)).toBe(true);
    expect(matches(key('o', { metaKey: true }), 'CmdOrCtrl+O', false)).toBe(false);
    expect(matches(key('o', { metaKey: true }), 'CmdOrCtrl+O', true)).toBe(true);
    expect(matches(key('o', { ctrlKey: true }), 'CmdOrCtrl+O', true)).toBe(false);
  });

  it('needs Shift when the shortcut has it, and refuses it when not', () => {
    expect(matches(key('Z', { ctrlKey: true, shiftKey: true }), 'CmdOrCtrl+Shift+Z', false)).toBe(true);
    expect(matches(key('z', { ctrlKey: true }), 'CmdOrCtrl+Shift+Z', false)).toBe(false);
    expect(matches(key('z', { ctrlKey: true, shiftKey: true }), 'CmdOrCtrl+Z', false)).toBe(false);
  });

  it('matches a bare function key only with no modifier', () => {
    expect(matches(key('F1'), 'F1', false)).toBe(true);
    expect(matches(key('F1', { ctrlKey: true }), 'F1', false)).toBe(false);
  });

  it('matches the comma and ignores letter case', () => {
    expect(matches(key(',', { ctrlKey: true }), 'CmdOrCtrl+,', false)).toBe(true);
    expect(matches(key('A', { ctrlKey: true }), 'CmdOrCtrl+a', false)).toBe(true);
  });
});

describe('isTextField', () => {
  it('knows what is not an element', () => {
    expect(isTextField(null)).toBe(false);
  });
});

describe('commandForKey', () => {
  const ev = (k: KeyLike, target: EventTarget | null = null) => ({ ...k, target });

  it('finds a context-only command', () => {
    const c = cmd('p', 'F2');
    expect(commandForKey([c], ev(key('F2')), false)).toBe(c);
  });

  it('leaves a command with a menu placement to the native menu', () => {
    const c = cmd('m', 'F2', { where: [{ menu: 'File', group: 1 }] });
    expect(commandForKey([c], ev(key('F2')), false)).toBeUndefined();
  });

  it('skips a disabled or invisible command', () => {
    expect(commandForKey([cmd('d', 'F2', { enabled: () => false })], ev(key('F2')), false)).toBeUndefined();
    expect(commandForKey([cmd('v', 'F2', { visible: () => false })], ev(key('F2')), false)).toBeUndefined();
  });

  it('leaves Ctrl keys alone in a text box', () => {
    const box = { closest: () => ({}) } as unknown as EventTarget;
    const c = cmd('a', 'CmdOrCtrl+A');
    expect(commandForKey([c], ev(key('a', { ctrlKey: true }), box), false)).toBeUndefined();
  });
});
