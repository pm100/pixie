import { describe, expect, it } from 'vitest';
import { DEFAULT_SIZE, fitSize, GAP, hangFrom, keepInside, MARGIN, MIN_SIZE, originOf, parseCardPrefs, pointerX } from './askCard';

const view = { w: 1200, h: 800 };
const button = { left: 1156, right: 1180, top: 42, bottom: 66 };

describe('the Ask Pixie card', () => {
  it('hangs below the ? button with its right edge just past the button', () => {
    const p = hangFrom(button, DEFAULT_SIZE, view);
    expect(p.top).toBe(button.bottom + GAP);
    expect(p.left + p.w).toBe(button.right + 6);
    expect(p.w).toBe(DEFAULT_SIZE.w);
  });

  it('is never taller than the room under the button, nor wider than the window', () => {
    expect(hangFrom(button, { w: 380, h: 5000 }, view).h).toBe(800 - (66 + GAP) - MARGIN);
    const small = hangFrom(button, DEFAULT_SIZE, { w: 340, h: 500 });
    expect(small.w).toBeLessThanOrEqual(340 - 2 * MARGIN);
    expect(small.left).toBeGreaterThanOrEqual(MARGIN);
    expect(fitSize({ w: 10, h: 10 }, view)).toEqual(MIN_SIZE);
  });

  it('keeps a dragged card inside the window, with its title bar reachable', () => {
    const base = { left: 0, top: 0, w: 380, h: 540 };
    expect(keepInside({ ...base, left: -500, top: -50 }, view)).toMatchObject({ left: MARGIN, top: MARGIN });
    expect(keepInside({ ...base, left: 5000, top: 5000 }, view)).toMatchObject({ left: view.w - 380 - MARGIN, top: view.h - 48 });
  });

  it('grows out of the middle of the ? button, wherever the card has been put', () => {
    const p = hangFrom(button, DEFAULT_SIZE, view);
    const o = originOf(button, p);
    expect(p.left + o.x).toBe(1168);
    expect(p.top + o.y).toBe(54);
    const moved = { ...p, left: 300, top: 300 };
    expect(moved.left + originOf(button, moved).x).toBe(1168);
  });

  it('points the pointer at the button, and not off the corner of the card', () => {
    const p = hangFrom(button, DEFAULT_SIZE, view);
    expect(p.left + pointerX(button, p)).toBe(1168);
    expect(pointerX(button, { ...p, left: 5000 })).toBe(16);
    expect(pointerX(button, { ...p, left: -5000 })).toBe(p.w - 16);
  });

  it('reads what was kept, and uses the defaults for anything odd', () => {
    expect(parseCardPrefs({ w: 400, h: 600, left: 10, top: 20 })).toEqual({ size: { w: 400, h: 600 }, moved: { left: 10, top: 20 } });
    expect(parseCardPrefs({ w: 400 })).toEqual({ size: { w: 400, h: DEFAULT_SIZE.h }, moved: null });
    for (const bad of [null, 'x', 7, [], { w: 'wide', left: NaN }]) expect(parseCardPrefs(bad)).toEqual({ size: DEFAULT_SIZE, moved: null });
  });
});
