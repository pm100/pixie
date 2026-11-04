import { describe, expect, it } from 'vitest';
import { bringToFront, firstFreeSpot, MIN_FRAME, moveRect, newFrameRect, replacePhoto, resizeRect, sendToBack, swapPhotos, topFrameAt, type Rect } from './collageFrames';

const inside = (r: Rect) => r.x >= -1e-9 && r.y >= -1e-9 && r.x + r.w <= 1 + 1e-9 && r.y + r.h <= 1 + 1e-9;
const r: Rect = { x: 0.1, y: 0.1, w: 0.3, h: 0.2 };

describe('moveRect', () => {
  it('moves by the drag and stops at every edge of the page', () => {
    const moved = moveRect(r, 0.2, 0.1);
    expect([moved.x, moved.y, moved.w, moved.h].map((n) => Math.round(n * 1e6) / 1e6)).toEqual([0.3, 0.2, 0.3, 0.2]);
    expect(moveRect(r, -5, -5)).toMatchObject({ x: 0, y: 0 });
    expect(moveRect(r, 5, 5)).toMatchObject({ x: 0.7, y: 0.8 });
  });
});

describe('resizeRect', () => {
  it('moves only the dragged edge', () => {
    const e = resizeRect(r, 'e', 0.2, 0.5, 1, false);
    expect([e.x, e.y, e.w, e.h].map((n) => Math.round(n * 1e6) / 1e6)).toEqual([0.1, 0.1, 0.5, 0.2]);
    const nw = resizeRect(r, 'nw', -0.05, -0.05, 1, false);
    expect(nw.x).toBeCloseTo(0.05);
    expect(nw.y).toBeCloseTo(0.05);
    expect(nw.x + nw.w).toBeCloseTo(0.4);
    expect(nw.y + nw.h).toBeCloseTo(0.3);
  });
  it('keeps the shape on paper when a corner is dragged with keepShape', () => {
    const pageAspect = 2 / 3;
    const square: Rect = { x: 0.1, y: 0.1, w: 0.3, h: 0.2 }; // 0.3 * 2/3 = 0.2 across for 0.2 down: square on paper
    const bigger = resizeRect(square, 'se', 0.2, 0, pageAspect, true);
    expect(bigger.w).toBeCloseTo(0.5);
    expect((bigger.w * pageAspect) / bigger.h).toBeCloseTo(1);
    expect(bigger.x).toBeCloseTo(0.1);
    expect(bigger.y).toBeCloseTo(0.1);
    const nw = resizeRect(square, 'nw', -0.1, 0, pageAspect, true);
    expect(nw.x + nw.w).toBeCloseTo(0.4);
    expect(nw.y + nw.h).toBeCloseTo(0.3);
  });
  it('never goes below the smallest size, even when dragged past the opposite edge', () => {
    for (const handle of ['n', 'ne', 'e', 'se', 's', 'sw', 'w', 'nw'] as const) {
      for (const keep of [false, true]) {
        for (const [dx, dy] of [[5, 5], [-5, -5], [5, -5], [-5, 5]]) {
          const out = resizeRect(r, handle, dx, dy, 0.7, keep);
          expect(out.w, `${handle} ${keep} ${dx},${dy}`).toBeGreaterThanOrEqual(MIN_FRAME - 1e-9);
          expect(out.h, `${handle} ${keep} ${dx},${dy}`).toBeGreaterThanOrEqual(MIN_FRAME - 1e-9);
          expect(inside(out), `${handle} ${keep} ${dx},${dy}: ${JSON.stringify(out)}`).toBe(true);
        }
      }
    }
  });
});

describe('newFrameRect', () => {
  const page = { w: 200, h: 300 };
  it('has the photos shape and about a quarter of the page', () => {
    const f = newFrameRect(3000, 2000, page, { x: 0.5, y: 0.5 });
    expect(f.w * f.h).toBeCloseTo(0.25);
    expect((f.w * page.w) / (f.h * page.h)).toBeCloseTo(1.5);
    expect(inside(f)).toBe(true);
  });
  it('is kept on the page when dropped in a corner or off it', () => {
    expect(newFrameRect(3000, 2000, page, { x: 0, y: 0 })).toMatchObject({ x: 0, y: 0 });
    expect(inside(newFrameRect(3000, 2000, page, { x: 1, y: 1 }))).toBe(true);
    expect(inside(newFrameRect(3000, 2000, page, { x: -4, y: 9 }))).toBe(true);
  });
  it('copes with a photo of unknown size and a very long one', () => {
    expect(inside(newFrameRect(0, 0, page, { x: 0.5, y: 0.5 }))).toBe(true);
    const wide = newFrameRect(10000, 100, page, { x: 0.5, y: 0.5 });
    expect(inside(wide)).toBe(true);
    expect(wide.h).toBeGreaterThanOrEqual(MIN_FRAME);
  });
});

describe('firstFreeSpot', () => {
  const size = { x: 0, y: 0, w: 0.3, h: 0.3 };
  it('is the top left of an empty page', () => {
    expect(firstFreeSpot([], size)).toEqual({ x: 0, y: 0 });
  });
  it('skips what is taken', () => {
    const spot = firstFreeSpot([{ x: 0, y: 0, w: 0.5, h: 0.5 }], size);
    expect(spot.x >= 0.5 - 1e-9 || spot.y >= 0.5 - 1e-9).toBe(true);
    expect(spot.x + size.w).toBeLessThanOrEqual(1 + 1e-9);
  });
  it('falls back to the middle when the page is full', () => {
    expect(firstFreeSpot([{ x: 0, y: 0, w: 1, h: 1 }], size)).toEqual({ x: 0.35, y: 0.35 });
  });
});

describe('order', () => {
  const f = (id: string, x: number): Rect & { id: string } => ({ id, x, y: 0, w: 0.5, h: 0.5 });
  const frames = [f('a', 0), f('b', 0.2), f('c', 0.4)];
  it('finds the frame on top at a point', () => {
    expect(topFrameAt(frames, 0.45, 0.2)?.id).toBe('c');
    expect(topFrameAt(frames, 0.1, 0.2)?.id).toBe('a');
    expect(topFrameAt(frames, 0.2, 0.9)).toBeNull();
    expect(topFrameAt([], 0.2, 0.2)).toBeNull();
  });
  it('brings frames to the front and sends them to the back, keeping their order', () => {
    expect(bringToFront(frames, ['a']).map((x) => x.id)).toEqual(['b', 'c', 'a']);
    expect(bringToFront(frames, ['a', 'b']).map((x) => x.id)).toEqual(['c', 'a', 'b']);
    expect(sendToBack(frames, ['c']).map((x) => x.id)).toEqual(['c', 'a', 'b']);
    expect(sendToBack(frames, ['b', 'c']).map((x) => x.id)).toEqual(['b', 'c', 'a']);
  });
});

describe('replacePhoto', () => {
  const g = (id: string, photo: number) => ({ id, photo, fx: 0.2, fy: 0.9, zoom: 2 });

  it('shows the new photo in that frame, whole and centred, and leaves the other frames alone', () => {
    const frames = [g('a', 1), g('b', 2), g('c', 3)];
    const out = replacePhoto(frames, 'b', 9);
    expect(out.map((f) => f.photo)).toEqual([1, 9, 3]);
    expect(out[1]).toMatchObject({ fx: 0.5, fy: 0.5, zoom: 1 });
    expect(out[0]).toBe(frames[0]);
    expect(out[2]).toBe(frames[2]);
  });

  it('does nothing for a frame that is not there', () => {
    const frames = [g('a', 1)];
    expect(replacePhoto(frames, 'zzz', 9)).toBe(frames);
  });
});

describe('swapPhotos', () => {
  const g = (id: string, photo: number) => ({ id, photo, fx: 0.1, fy: 0.9, zoom: 3 });
  it('exchanges the photos and shows each whole again', () => {
    const out = swapPhotos([g('a', 1), g('b', 2), g('c', 3)], 'a', 'b');
    expect(out.map((x) => x.photo)).toEqual([2, 1, 3]);
    expect(out[0]).toMatchObject({ fx: 0.5, fy: 0.5, zoom: 1 });
    expect(out[2]).toMatchObject({ fx: 0.1, zoom: 3 });
  });
  it('does nothing for a frame that is not there', () => {
    const frames = [g('a', 1)];
    expect(swapPhotos(frames, 'a', 'zz')).toBe(frames);
  });
});
