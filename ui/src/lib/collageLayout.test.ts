import { describe, expect, it } from 'vitest';
import { AUTO_LAYOUT_MAX, layoutSuggestion, templateFrames } from './collageLayout';
import { MIN_FRAME, type Rect } from './collageFrames';

const grid2x2 = [{ x: 0, y: 0, w: 0.5, h: 0.5 }, { x: 0.5, y: 0, w: 0.5, h: 0.5 }, { x: 0, y: 0.5, w: 0.5, h: 0.5 }, { x: 0.5, y: 0.5, w: 0.5, h: 0.5 }];
const apart = (a: Rect, b: Rect) => a.x + a.w <= b.x + 1e-9 || b.x + b.w <= a.x + 1e-9 || a.y + a.h <= b.y + 1e-9 || b.y + b.h <= a.y + 1e-9;
const inside = (r: Rect) => r.x >= -1e-9 && r.y >= -1e-9 && r.x + r.w <= 1 + 1e-9 && r.y + r.h <= 1 + 1e-9;
/** The frames reach every edge of the page, inside a margin of `mx` across and `my` down (as fractions of the page). */
const fills = (rects: Rect[], mx: number, my = mx) =>
  Math.abs(Math.min(...rects.map((r) => r.x)) - mx) < 1e-6 && Math.abs(Math.max(...rects.map((r) => r.x + r.w)) - (1 - mx)) < 1e-6 &&
  Math.abs(Math.min(...rects.map((r) => r.y)) - my) < 1e-6 && Math.abs(Math.max(...rects.map((r) => r.y + r.h)) - (1 - my)) < 1e-6;
/** How much of each photo is still shown on average once it is cropped to fill its frame. */
const kept = (rects: Rect[], shapes: { w: number; h: number }[], page: { w: number; h: number }) =>
  rects.reduce((sum, r, i) => { const fa = (r.w * page.w) / (r.h * page.h), a = shapes[i].w / shapes[i].h; return sum + Math.min(fa / a, a / fa); }, 0) / rects.length;

describe('templateFrames', () => {
  it('pulls each cell in by half the gap, so cells are a whole gap apart', () => {
    const f = templateFrames(grid2x2, { w: 100, h: 100 }, 4);
    expect(f[0].w).toBeCloseTo(0.46);
    expect(f[0].h).toBeCloseTo(0.46);
    expect(f[0].x).toBeCloseTo(0.02);
    expect(f[1].x - (f[0].x + f[0].w)).toBeCloseTo(0.04);
    expect(f[2].y - (f[0].y + f[0].h)).toBeCloseTo(0.04);
  });
  it('is the cells themselves with no gap', () => {
    expect(templateFrames(grid2x2, { w: 100, h: 100 }, 0)).toEqual(grid2x2);
  });
  it('leaves the smallest frame when the gap is bigger than a cell, still on the page', () => {
    for (const f of templateFrames(grid2x2, { w: 100, h: 100 }, 500)) {
      expect(f.w).toBeCloseTo(MIN_FRAME);
      expect(inside(f)).toBe(true);
    }
  });
});

describe('layoutSuggestion', () => {
  const page = { w: 200, h: 200 };
  const square = { w: 100, h: 100 };

  it('lays four squares out as two rows of two, in order', () => {
    const { rects, used } = layoutSuggestion([square, square, square, square], page, 0, 0);
    expect(used).toBe(4);
    expect(rects[0].y).toBeCloseTo(rects[1].y);
    expect(rects[2].y).toBeCloseTo(rects[3].y);
    expect(rects[2].y).toBeGreaterThan(rects[0].y);
    expect(rects[1].x).toBeGreaterThan(rects[0].x);
    for (const r of rects) expect(r.w).toBeCloseTo(0.5);
  });

  it('fills the page, stays on it and never overlaps', () => {
    const shapes = [{ w: 3000, h: 1000 }, { w: 1000, h: 2000 }, { w: 3000, h: 2000 }, square, { w: 2000, h: 3000 }, { w: 4000, h: 3000 }];
    const { rects } = layoutSuggestion(shapes, { w: 200, h: 300 }, 0, 0);
    expect(rects).toHaveLength(6);
    expect(fills(rects, 0)).toBe(true);
    rects.forEach((r, i) => {
      expect(inside(r), `photo ${i} ${JSON.stringify(r)}`).toBe(true);
      rects.slice(i + 1).forEach((o, j) => expect(apart(r, o), `${i} and ${i + 1 + j}`).toBe(true));
    });
    expect(rects.reduce((sum, r) => sum + r.w * r.h, 0)).toBeCloseTo(1); // no gap: the frames are the whole page
  });

  it('fills the page inside its margin when there is a gap', () => {
    const { rects } = layoutSuggestion([square, square, square, square, square], { w: 200, h: 300 }, 10, 0);
    expect(fills(rects, 5 / 200, 5 / 300)).toBe(true);
  });

  it('keeps the gap between photos', () => {
    const { rects } = layoutSuggestion([square, square], { w: 200, h: 100 }, 10, 0);
    expect((rects[1].x - (rects[0].x + rects[0].w)) * 200).toBeCloseTo(10);
  });

  it('gives one photo the whole page, cropped to fill it', () => {
    const { rects } = layoutSuggestion([{ w: 300, h: 100 }], { w: 200, h: 300 }, 0, 0);
    expect(rects[0].x).toBeCloseTo(0);
    expect(rects[0].y).toBeCloseTo(0);
    expect(rects[0].w).toBeCloseTo(1);
    expect(rects[0].h).toBeCloseTo(1);
  });

  it('uses at most 24 photos, and none for none', () => {
    const many = Array.from({ length: 30 }, () => square);
    const out = layoutSuggestion(many, page, 2, 0);
    expect(AUTO_LAYOUT_MAX).toBe(24);
    expect(out.used).toBe(24);
    expect(out.rects).toHaveLength(24);
    expect(layoutSuggestion([], page, 2, 0)).toEqual({ rects: [], used: 0 });
  });

  it('takes a photo of unknown size as 3 to 2', () => {
    const { rects } = layoutSuggestion([{ w: 0, h: 0 }], { w: 300, h: 200 }, 0, 0);
    expect((rects[0].w * 300) / (rects[0].h * 200)).toBeCloseTo(1.5);
  });

  it('copes with a gap as big as the page', () => {
    const { rects } = layoutSuggestion([square, square, square], { w: 100, h: 100 }, 100, 0);
    expect(rects).toHaveLength(3);
    for (const r of rects) expect(Number.isFinite(r.x + r.y + r.w + r.h)).toBe(true);
  });

  describe('asking again', () => {
    const shapes = [{ w: 3000, h: 1000 }, { w: 1000, h: 2000 }, { w: 3000, h: 2000 }, { w: 1000, h: 1000 }, { w: 2000, h: 3000 }, { w: 4000, h: 3000 }];
    const sheet = { w: 200, h: 300 };

    it('gives a different arrangement each time, for a good while', () => {
      const seen = new Set(Array.from({ length: 12 }, (_, i) => JSON.stringify(layoutSuggestion(shapes, sheet, 4, i).rects.map((r) => [r.x, r.y, r.w, r.h].map((n) => Math.round(n * 1000))))));
      expect(seen.size).toBeGreaterThanOrEqual(10);
    });

    it('gives the same arrangement for the same index', () => {
      expect(layoutSuggestion(shapes, sheet, 4, 5)).toEqual(layoutSuggestion(shapes, sheet, 4, 5));
    });

    it('fills the page, with the frames on it and apart, whichever index is asked', () => {
      for (let index = 0; index < 30; index++) {
        const { rects } = layoutSuggestion(shapes, sheet, 4, index);
        expect(rects, `index ${index}`).toHaveLength(6);
        expect(fills(rects, 2 / 200, 2 / 300), `index ${index} fills`).toBe(true);
        rects.forEach((r, i) => {
          expect(inside(r), `index ${index} photo ${i} ${JSON.stringify(r)}`).toBe(true);
          rects.slice(i + 1).forEach((o, j) => expect(apart(r, o), `index ${index}: ${i} and ${i + 1 + j}`).toBe(true));
        });
      }
    });

    it('starts with the arrangement that crops the least, and the first ones keep the order', () => {
      const first = kept(layoutSuggestion(shapes, sheet, 4, 0).rects, shapes, sheet);
      for (const index of [1, 2]) expect(kept(layoutSuggestion(shapes, sheet, 4, index).rects, shapes, sheet)).toBeLessThanOrEqual(first + 1e-9);
    });

    it('crops nothing when the shapes suit the page', () => {
      const four = [square, square, square, square];
      expect(kept(layoutSuggestion(four, { w: 200, h: 200 }, 0, 0).rects, four, { w: 200, h: 200 })).toBeCloseTo(1);
    });

    it('copes with one photo, two photos and a very large index', () => {
      expect(layoutSuggestion([{ w: 3, h: 2 }], sheet, 0, 7).rects).toHaveLength(1);
      expect(layoutSuggestion([{ w: 3, h: 2 }, { w: 2, h: 3 }], sheet, 0, 1234567).rects).toHaveLength(2);
      expect(layoutSuggestion(shapes, sheet, 0, -3).rects).toHaveLength(6);
    });
  });
});
