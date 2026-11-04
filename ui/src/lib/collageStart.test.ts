import { describe, expect, it } from 'vitest';
import type { Photo } from './api';
import { newDoc } from './collageDoc';
import type { CollageFrame } from './collageDoc';
import { framesFromTemplate, newFrameId, relayoutFrames } from './collageStart';

const photo = (id: number, width: number | null = 3000, height: number | null = 2000): Photo => ({ id, folder_id: 1, path: `p${id}.jpg`, size: 1, mtime: 1, width, height, taken_at: null, camera: null, gps_lat: null, gps_lon: null, missing: false, kind: 'photo', duration_ms: null });
const grid2x2 = [{ x: 0, y: 0, w: 0.5, h: 0.5 }, { x: 0.5, y: 0, w: 0.5, h: 0.5 }, { x: 0, y: 0.5, w: 0.5, h: 0.5 }, { x: 0.5, y: 0.5, w: 0.5, h: 0.5 }];

describe('framesFromTemplate', () => {
  it('puts the photos in the spaces in order', () => {
    const { frames, leftOut } = framesFromTemplate(grid2x2, newDoc(), [photo(1), photo(2), photo(3), photo(4)]);
    expect(frames.map((f) => f.photo)).toEqual([1, 2, 3, 4]);
    expect(leftOut).toBe(0);
    expect(frames.every((f) => f.zoom === 1 && f.fx === 0.5 && f.fy === 0.5)).toBe(true);
  });
  it('leaves a space empty when there are too few photos, and counts the photos that do not fit', () => {
    expect(framesFromTemplate(grid2x2, newDoc(), [photo(1), photo(2), photo(3)]).frames).toHaveLength(3);
    const many = framesFromTemplate(grid2x2, newDoc(), [1, 2, 3, 4, 5, 6].map((i) => photo(i)));
    expect(many.frames).toHaveLength(4);
    expect(many.leftOut).toBe(2);
  });
});

describe('relayoutFrames', () => {
  const photos = new Map([1, 2, 3, 4].map((id) => [id, photo(id, id % 2 ? 3000 : 2000, id % 2 ? 2000 : 3000)]));
  const frame = (id: string, p: number): CollageFrame => ({ id, photo: p, x: 0, y: 0, w: 0.1, h: 0.1, fx: 0.2, fy: 0.8, zoom: 3 });
  const frames = [frame('a', 1), frame('b', 2), frame('c', 3), frame('d', 4)];

  it('moves every frame, keeping its photo and its place in the order, and shows the photo whole again', () => {
    const out = relayoutFrames(frames, photos, newDoc(), 0);
    expect(out.map((f) => [f.id, f.photo])).toEqual([['a', 1], ['b', 2], ['c', 3], ['d', 4]]);
    expect(out.every((f) => f.fx === 0.5 && f.fy === 0.5 && f.zoom === 1)).toBe(true);
    expect(out.every((f) => f.w > 0.1)).toBe(true);
  });
  it('gives another arrangement for another index', () => {
    const [first, again] = [relayoutFrames(frames, photos, newDoc(), 0), relayoutFrames(frames, photos, newDoc(), 1)];
    expect(JSON.stringify(first)).not.toBe(JSON.stringify(again));
  });
  it('leaves frames past the 24th where they are', () => {
    const many = Array.from({ length: 26 }, (_, i) => frame(`f${i}`, 1));
    const out = relayoutFrames(many, photos, newDoc(), 0);
    expect(out[24]).toEqual(many[24]);
    expect(out[25]).toEqual(many[25]);
    expect(out[0].w).not.toBe(0.1);
  });
  it('copes with no frames and with a photo that is not known', () => {
    expect(relayoutFrames([], photos, newDoc(), 0)).toEqual([]);
    expect(relayoutFrames([frame('x', 99)], photos, newDoc(), 0)).toHaveLength(1);
  });
});

describe('newFrameId', () => {
  it('is different every time', () => {
    expect(new Set(Array.from({ length: 200 }, newFrameId)).size).toBe(200);
  });
});
