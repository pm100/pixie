import { describe, expect, it } from 'vitest';
import { fillSpace } from './collageFill';
import { MIN_FRAME } from './collageDoc';
import type { Rect } from './collageFrames';

const page = { w: 100, h: 100 };
const square: Rect = { x: 0.4, y: 0.4, w: 0.2, h: 0.2 };
const centre = { x: 0.5, y: 0.5 };
const near = (r: Rect | null, e: Rect) => {
  expect(r).not.toBeNull();
  for (const k of ['x', 'y', 'w', 'h'] as const) expect(r![k], k).toBeCloseTo(e[k], 3);
};

describe('fillSpace', () => {
  it('fills an empty page when there is no gap', () => {
    near(fillSpace(square, [], page, 0, centre), { x: 0, y: 0, w: 1, h: 1 });
  });

  it('keeps the margin of half the gap round the page', () => {
    near(fillSpace(square, [], page, 10, centre), { x: 0.05, y: 0.05, w: 0.9, h: 0.9 });
  });

  it('keeps the shape of the frame, and sits as near the point as it can', () => {
    const wide: Rect = { x: 0.3, y: 0.4, w: 0.4, h: 0.2 }; // twice as wide as tall
    const out = fillSpace(wide, [], page, 0, { x: 0.5, y: 0.8 });
    near(out, { x: 0, y: 0.5, w: 1, h: 0.5 });
  });

  it('keeps the shape of a frame on a page that is not square', () => {
    const tall = { w: 100, h: 200 };
    const f: Rect = { x: 0.4, y: 0.4, w: 0.2, h: 0.1 }; // 20 mm by 20 mm: square on paper
    const out = fillSpace(f, [], tall, 0, centre)!;
    expect((out.w * tall.w) / (out.h * tall.h)).toBeCloseTo(1);
    expect(out.w).toBeCloseTo(1);
  });

  it('fills the free space beside another photo, a gap away from it', () => {
    const left: Rect = { x: 0, y: 0, w: 0.5, h: 1 };
    near(fillSpace(square, [left], page, 0, { x: 0.75, y: 0.5 }), { x: 0.5, y: 0.25, w: 0.5, h: 0.5 });
    const gap = fillSpace(square, [left], page, 4, { x: 0.75, y: 0.5 })!;
    expect(gap.x).toBeCloseTo(0.5 + 0.04);
    expect(gap.x + gap.w).toBeCloseTo(1 - 0.02);
  });

  it('shrinks a photo that is bigger than the space it is moved into', () => {
    const big: Rect = { x: 0.1, y: 0.1, w: 0.6, h: 0.6 };
    const blockers: Rect[] = [{ x: 0, y: 0, w: 0.3, h: 1 }, { x: 0.6, y: 0, w: 0.4, h: 1 }]; // a slot 0.3 wide between them
    near(fillSpace(big, blockers, page, 0, { x: 0.45, y: 0.5 }), { x: 0.3, y: 0.35, w: 0.3, h: 0.3 });
  });

  it('fills the slot a photo came from in a full page', () => {
    const slots: Rect[] = [{ x: 0, y: 0, w: 0.5, h: 0.5 }, { x: 0.5, y: 0, w: 0.5, h: 0.5 }, { x: 0, y: 0.5, w: 0.5, h: 0.5 }];
    near(fillSpace({ x: 0.55, y: 0.55, w: 0.3, h: 0.3 }, slots, page, 0, { x: 0.7, y: 0.7 }), { x: 0.5, y: 0.5, w: 0.5, h: 0.5 });
  });

  it('does nothing over another photo, or in the gap beside one', () => {
    const other: Rect = { x: 0.3, y: 0.3, w: 0.4, h: 0.4 };
    expect(fillSpace(square, [other], page, 0, centre)).toBeNull();
    expect(fillSpace(square, [other], page, 10, { x: 0.75, y: 0.5 })).toBeNull(); // 0.05 from the photo: inside its gap
  });

  it('does nothing when the free space is too small for a frame', () => {
    const blockers: Rect[] = [{ x: 0, y: 0, w: 0.48, h: 1 }, { x: 0.52, y: 0, w: 0.48, h: 1 }]; // a 0.04 slot
    expect(fillSpace(square, blockers, page, 0, { x: 0.5, y: 0.5 })).toBeNull();
  });

  it('does nothing for a point off the page, or a frame with no size', () => {
    expect(fillSpace(square, [], page, 0, { x: 1.5, y: 0.5 })).toBeNull();
    expect(fillSpace({ x: 0, y: 0, w: 0, h: 0 }, [], page, 0, centre)).toBeNull();
  });

  it('never overlaps another photo, never leaves the page and never goes below the smallest size', () => {
    const others: Rect[] = [{ x: 0.05, y: 0.05, w: 0.3, h: 0.4 }, { x: 0.6, y: 0.1, w: 0.3, h: 0.3 }, { x: 0.2, y: 0.6, w: 0.5, h: 0.3 }, { x: 0.8, y: 0.55, w: 0.15, h: 0.4 }];
    const frames: Rect[] = [square, { x: 0.1, y: 0.1, w: 0.4, h: 0.1 }, { x: 0.1, y: 0.1, w: 0.1, h: 0.5 }];
    let placed = 0;
    for (const frame of frames) for (const gap of [0, 3, 8]) for (let x = 0.02; x < 1; x += 0.07) for (let y = 0.02; y < 1; y += 0.07) {
      const r = fillSpace(frame, others, page, gap, { x, y });
      if (!r) continue;
      placed++;
      const g = gap / 100;
      expect(r.x, `${x},${y}`).toBeGreaterThanOrEqual(g / 2 - 1e-6);
      expect(r.y).toBeGreaterThanOrEqual(g / 2 - 1e-6);
      expect(r.x + r.w).toBeLessThanOrEqual(1 - g / 2 + 1e-6);
      expect(r.y + r.h).toBeLessThanOrEqual(1 - g / 2 + 1e-6);
      expect(Math.min(r.w, r.h)).toBeGreaterThanOrEqual(MIN_FRAME - 1e-6);
      expect(r.h / r.w).toBeCloseTo(frame.h / frame.w, 3);
      expect(x >= r.x - 1e-6 && x <= r.x + r.w + 1e-6 && y >= r.y - 1e-6 && y <= r.y + r.h + 1e-6, 'it holds the point').toBe(true);
      for (const o of others) {
        const apart = r.x + r.w <= o.x - g + 1e-6 || o.x + o.w + g <= r.x + 1e-6 || r.y + r.h <= o.y - g + 1e-6 || o.y + o.h + g <= r.y + 1e-6;
        expect(apart, `${JSON.stringify(r)} against ${JSON.stringify(o)} (gap ${gap})`).toBe(true);
      }
    }
    expect(placed).toBeGreaterThan(50);
  });
});
