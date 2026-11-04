import { describe, expect, it } from 'vitest';
import { collageFileName, coverCrop, defaultDpi, defaultSavePath, focusFromDrag, frameDpi, overflow, pagePx, sizeProblem } from './collage';

describe('coverCrop (the same numbers as the Rust cover_crop)', () => {
  it('keeps the full height of a wide photo in a square frame', () => {
    expect(coverCrop(400, 200, 100, 100, 0.5, 0.5, 1)).toEqual({ x: 100, y: 0, w: 200, h: 200 });
    expect(coverCrop(400, 200, 100, 100, 0, 0.5, 1).x).toBe(0);
    expect(coverCrop(400, 200, 100, 100, 1, 0.5, 1).x).toBe(200);
  });
  it('keeps the full width of a tall photo in a wide frame, and clamps the focus', () => {
    expect(coverCrop(200, 400, 100, 50, 0.5, 1, 1)).toEqual({ x: 0, y: 300, w: 200, h: 100 });
    expect(coverCrop(200, 400, 100, 50, 0.5, 7, 1).y).toBe(300);
  });
  it('is the whole photo when the shape matches', () => {
    expect(coverCrop(300, 200, 150, 100, 0.3, 0.9, 1)).toEqual({ x: 0, y: 0, w: 300, h: 200 });
  });
  it('shows half the width and height at zoom 2, and limits the zoom to 1..4', () => {
    expect(coverCrop(400, 200, 100, 100, 0.5, 0.5, 2)).toEqual({ x: 150, y: 50, w: 100, h: 100 });
    expect(coverCrop(400, 200, 100, 100, 0.5, 0.5, 9)).toEqual(coverCrop(400, 200, 100, 100, 0.5, 0.5, 4));
    expect(coverCrop(400, 200, 100, 100, 0.5, 0.5, 0.2)).toEqual(coverCrop(400, 200, 100, 100, 0.5, 0.5, 1));
    expect(coverCrop(400, 200, 100, 100, 0.5, 0.5, NaN)).toEqual({ x: 100, y: 0, w: 200, h: 200 });
  });
});

describe('frameDpi', () => {
  const page = { w: 300, h: 200 };
  const half = { w: 0.5, h: 0.5, fx: 0.5, fy: 0.5, zoom: 1 }; // 150 x 100 mm
  it('is the pixels of the part shown over the printed width', () => {
    expect(Math.round(frameDpi(3000, 2000, half, page))).toBe(508);
    expect(Math.round(frameDpi(3000, 2000, { ...half, w: 1, h: 1 }, page))).toBe(254);
    expect(Math.round(frameDpi(3000, 2000, { ...half, zoom: 2 }, page))).toBe(254);
  });
});

describe('focusFromDrag', () => {
  const start = { fx: 0.5, fy: 0.5 };
  it('moves the part shown the other way to the drag', () => {
    expect(focusFromDrag(start, { dx: 25, dy: 0 }, { x: 100, y: 0 })).toEqual({ fx: 0.25, fy: 0.5 });
    expect(focusFromDrag(start, { dx: -25, dy: 0 }, { x: 100, y: 0 })).toEqual({ fx: 0.75, fy: 0.5 });
    expect(focusFromDrag(start, { dx: 0, dy: 10 }, { x: 0, y: 20 })).toEqual({ fx: 0.5, fy: 0 });
  });
  it('stops at the edges of the photo', () => {
    expect(focusFromDrag(start, { dx: 500, dy: 0 }, { x: 100, y: 0 }).fx).toBe(0);
    expect(focusFromDrag(start, { dx: -500, dy: 0 }, { x: 100, y: 0 }).fx).toBe(1);
  });
  it('keeps an axis where the photo does not stick out', () => {
    expect(focusFromDrag({ fx: 0.3, fy: 0.9 }, { dx: 40, dy: 40 }, { x: 0, y: 0 })).toEqual({ fx: 0.3, fy: 0.9 });
  });
});

describe('overflow', () => {
  it('is how far the photo sticks out once it fills the frame', () => {
    expect(overflow(400, 200, 100, 100)).toEqual({ x: 100, y: 0 });
    expect(overflow(200, 400, 100, 100)).toEqual({ x: 0, y: 100 });
    expect(overflow(100, 100, 100, 100)).toEqual({ x: 0, y: 0 });
    expect(overflow(0, 0, 100, 100)).toEqual({ x: 0, y: 0 });
  });
  it('grows with the zoom', () => {
    expect(overflow(400, 200, 100, 100, 2)).toEqual({ x: 300, y: 100 });
  });
});

describe('page size', () => {
  it('uses 300 dpi below A2 and 150 from A2 up', () => {
    expect(defaultDpi(210, 297)).toBe(300);
    expect(defaultDpi(297, 420)).toBe(300);
    expect(defaultDpi(420, 594)).toBe(150);
    expect(defaultDpi(1000, 700)).toBe(150);
  });
  it('gives the pixels of the saved page', () => {
    expect(pagePx(210, 297, 300)).toEqual({ w: 2480, h: 3508 });
    expect(pagePx(700, 1000, 150)).toEqual({ w: 4134, h: 5906 });
  });
  it('explains a page that is too big and accepts one that is not', () => {
    expect(sizeProblem(700, 1000, 150)).toBeNull();
    expect(sizeProblem(700, 1000, 600)).toContain('16,384');
    expect(sizeProblem(0, 100, 300)).not.toBeNull();
  });
});

describe('the name and place a collage is saved with', () => {
  const day = new Date(2026, 10, 3, 14, 5); // 3 November 2026, local time
  it('is collage_ and the date', () => {
    expect(collageFileName(day)).toBe('collage_2026-11-03.png');
    expect(collageFileName(new Date(2026, 0, 9))).toBe('collage_2026-01-09.png');
  });
  it('starts in the folder given, with the separator that folder uses', () => {
    expect(defaultSavePath('D:\\Photos\\Sync', day)).toBe('D:\\Photos\\Sync\\collage_2026-11-03.png');
    expect(defaultSavePath('D:\\Photos\\Sync\\', day)).toBe('D:\\Photos\\Sync\\collage_2026-11-03.png');
    expect(defaultSavePath('/Users/me/Pictures', day)).toBe('/Users/me/Pictures/collage_2026-11-03.png');
    expect(defaultSavePath('/Users/me/Pictures/', day)).toBe('/Users/me/Pictures/collage_2026-11-03.png');
    expect(defaultSavePath('P:\\', day)).toBe('P:\\collage_2026-11-03.png');
  });
  it('is just the name when there is no folder', () => {
    expect(defaultSavePath(null, day)).toBe('collage_2026-11-03.png');
    expect(defaultSavePath(undefined, day)).toBe('collage_2026-11-03.png');
    expect(defaultSavePath('', day)).toBe('collage_2026-11-03.png');
  });
});
