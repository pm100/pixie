import { describe, expect, it } from 'vitest';
import { click, emptySelection, single } from './selection';

const none = { ctrl: false, shift: false };
const ctrl = { ctrl: true, shift: false };
const shift = { ctrl: false, shift: true };
const both = { ctrl: true, shift: true };
const list = (s: ReturnType<typeof click>) => [...s.picked].sort((a, b) => a - b);

describe('selection', () => {
  it('plain click selects only that photo', () => {
    const s = click(click(emptySelection, 3, none), 7, none);
    expect(list(s)).toEqual([7]);
    expect(s.anchor).toBe(7);
  });
  it('ctrl-click toggles one photo and moves the anchor', () => {
    let s = click(emptySelection, 3, none);
    s = click(s, 5, ctrl);
    expect(list(s)).toEqual([3, 5]);
    s = click(s, 3, ctrl);
    expect(list(s)).toEqual([5]);
    expect(s.anchor).toBe(3);
  });
  it('shift-click selects the range from the anchor, in either direction', () => {
    let s = click(emptySelection, 4, none);
    expect(list(click(s, 7, shift))).toEqual([4, 5, 6, 7]);
    expect(list(click(s, 2, shift))).toEqual([2, 3, 4]);
    expect(click(s, 7, shift).anchor).toBe(4); // anchor stays for the next range
  });
  it('shift-click replaces the previous range; ctrl+shift adds to the selection', () => {
    let s = click(emptySelection, 4, none);
    s = click(s, 6, shift);
    s = click(s, 5, shift);
    expect(list(s)).toEqual([4, 5]);
    s = click(click(emptySelection, 0, none), 10, ctrl);
    expect(list(click(s, 12, both))).toEqual([0, 10, 11, 12]);
  });
  it('shift-click with no anchor acts like a plain click', () => {
    expect(list(click(emptySelection, 5, shift))).toEqual([5]);
  });
  it('single returns the index only when exactly one is picked', () => {
    expect(single(emptySelection)).toBeNull();
    expect(single(click(emptySelection, 9, none))).toBe(9);
    expect(single(click(click(emptySelection, 1, none), 3, ctrl))).toBeNull();
  });
  it('never mutates the previous selection', () => {
    const a = click(emptySelection, 1, none);
    click(a, 2, ctrl);
    expect(list(a)).toEqual([1]);
    expect(emptySelection.picked.size).toBe(0);
  });
});
