import { describe, expect, it } from 'vitest';
import type { Rect } from './collageFrames';
import { snapMove, snapResize, type SnapContext } from './collageSnap';

// A 100 x 100 mm page drawn 1000 px wide: 6 px is 0.006 of the page.
const ctx = (over: Partial<SnapContext> = {}): SnapContext => ({ others: [], pageMm: { w: 100, h: 100 }, marginMm: 0, page: true, frames: true, thresholdPx: 6, pagePx: { w: 1000, h: 1000 }, ...over });
const r = (x: number, y: number, w = 0.2, h = 0.2): Rect => ({ x, y, w, h });
const inside = (a: Rect) => a.x >= -1e-9 && a.y >= -1e-9 && a.x + a.w <= 1 + 1e-9 && a.y + a.h <= 1 + 1e-9;

describe('snapMove to the page', () => {
  it('snaps an edge that is near the page edge, with a guide', () => {
    const out = snapMove(r(0.004, 0.3), ctx());
    expect(out.rect.x).toBeCloseTo(0);
    expect(out.guides.filter((g) => g.axis === 'x')).toEqual([{ axis: 'x', at: 0, from: 0, to: 1 }]);
  });
  it('leaves a frame that is further away than the threshold', () => {
    const out = snapMove(r(0.02, 0.3), ctx());
    expect(out.rect).toEqual(r(0.02, 0.3));
    expect(out.guides).toEqual([]);
  });
  it('does not snap to the page when that is turned off', () => {
    const out = snapMove(r(0.004, 0.3), ctx({ page: false }));
    expect(out.rect.x).toBe(0.004);
  });
  it('snaps to the margin, which is half the gap', () => {
    const out = snapMove(r(0.023, 0.3), ctx({ marginMm: 2 }));
    expect(out.rect.x).toBeCloseTo(0.02);
  });
  it('snaps the centre to the centre line of the page', () => {
    const out = snapMove(r(0.403, 0.3), ctx());
    expect(out.rect.x).toBeCloseTo(0.4);
    expect(out.guides.some((g) => g.axis === 'x' && g.at === 0.5)).toBe(true);
  });
});

describe('snapMove to other frames', () => {
  const other = { x: 0.1, y: 0.1, w: 0.2, h: 0.2 };
  it('aligns the edges and the centres', () => {
    expect(snapMove(r(0.104, 0.5), ctx({ others: [other] })).rect.x).toBeCloseTo(0.1);
    const mid = snapMove(r(0.504, 0.103, 0.1, 0.2), ctx({ others: [{ x: 0.45, y: 0.5, w: 0.2, h: 0.2 }] })); // centre .554 against .55
    expect(mid.rect.x).toBeCloseTo(0.5);
  });
  it('leaves the gap setting between neighbours', () => {
    const out = snapMove(r(0.345, 0.15), ctx({ others: [other], marginMm: 2 })); // gap 4 mm = .04: next to .3 means .34
    expect(out.rect.x).toBeCloseTo(0.34);
  });
  it('matches the gap that is already between two other frames', () => {
    const row = [r(0, 0.4), r(0.3, 0.4)]; // a gap of .1 between them
    const out = snapMove(r(0.605, 0.4), ctx({ others: row }));
    expect(out.rect.x).toBeCloseTo(0.6);
    const guide = out.guides.find((g) => g.gapMm !== undefined);
    expect(guide?.gapMm).toBeCloseTo(10);
  });
  it('does not snap to other frames when that is turned off', () => {
    expect(snapMove(r(0.104, 0.5), ctx({ others: [other], frames: false })).rect.x).toBe(0.104);
  });
  it('takes the nearer of two targets', () => {
    const out = snapMove(r(0.005, 0.5), ctx({ others: [{ x: 0.008, y: 0.1, w: 0.2, h: 0.2 }] }));
    expect(out.rect.x).toBeCloseTo(0.008);
  });
  it('never ends up off the page', () => {
    const edge = [{ x: 0.8, y: 0.1, w: 0.2, h: 0.2 }, { x: 0, y: 0.1, w: 0.1, h: 0.2 }];
    for (const x of [0, 0.2, 0.495, 0.5, 0.79, 0.8]) for (const y of [0, 0.1, 0.3, 0.8]) {
      for (const w of [0.2, 0.5, 0.6]) {
        const start = r(Math.min(x, 1 - w), y, w, 0.2);
        expect(inside(snapMove(start, ctx({ others: edge, marginMm: 3 })).rect), `${JSON.stringify(start)}`).toBe(true);
      }
    }
  });
});

describe('snapResize', () => {
  it('moves only the edge being dragged', () => {
    const wide = { x: 0.1, y: 0.5, w: 0.7, h: 0.2 }; // right edge at .8
    const out = snapResize({ x: 0.1, y: 0.1, w: 0.695, h: 0.2 }, 'e', ctx({ others: [wide] }));
    expect(out.rect.x).toBe(0.1);
    expect(out.rect.y).toBe(0.1);
    expect(out.rect.h).toBe(0.2);
    expect(out.rect.w).toBeCloseTo(0.7);
  });
  it('snaps a corner on both axes and leaves the opposite corner alone', () => {
    const out = snapResize({ x: 0.2, y: 0.2, w: 0.596, h: 0.597 }, 'se', ctx());
    expect(out.rect.x).toBe(0.2);
    expect(out.rect.y).toBe(0.2);
    expect(out.rect.x + out.rect.w).toBeCloseTo(0.8 - 0.004 + 0.004, 1);
    expect(out.rect.x + out.rect.w).toBeCloseTo(0.8, 1);
  });
  it('does not shrink a frame below the smallest size', () => {
    const out = snapResize({ x: 0.1, y: 0.1, w: 0.06, h: 0.2 }, 'e', ctx({ others: [{ x: 0.3, y: 0.5, w: 0.2, h: 0.2 }], marginMm: 0 }));
    expect(out.rect.w).toBeGreaterThanOrEqual(0.05 - 1e-9);
  });
});
