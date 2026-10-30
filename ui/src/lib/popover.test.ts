import { describe, expect, it } from 'vitest';
import { placement } from './popover';

const win = { width: 1000, height: 800 };
const at = (top: number, bottom: number, left = 100, right = 180) => ({ top, bottom, left, right });

describe('placement', () => {
  it('opens under the button when there is room', () => {
    expect(placement(at(100, 124), win)).toBe('top:128px;max-height:668px;left:100px');
  });
  it('opens over the button near the bottom of the window, as tall as the room above', () => {
    expect(placement(at(700, 724), win)).toBe('bottom:104px;max-height:692px;left:100px');
  });
  it('stays under the button when there is even less room above', () => {
    expect(placement(at(40, 64), { width: 1000, height: 300 })).toBe('top:68px;max-height:228px;left:100px');
  });
  it('lines up with the right edge of the button', () => {
    expect(placement(at(100, 124, 100, 180), win, { alignRight: true })).toContain('right:820px');
  });
  it('gives a width, lined up with the right edge and kept inside the window', () => {
    expect(placement(at(100, 124, 100, 180), win, { alignRight: true, width: 240 })).toContain('left:8px;width:240px');
    expect(placement(at(100, 124, 990, 1000), win, { width: 240 })).toContain('left:752px;width:240px');
  });
});
