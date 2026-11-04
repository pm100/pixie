import { describe, expect, it } from 'vitest';
import { isStale, mosaicDpi, mosaicPageMm, mosaicSavePath, newMosaicDoc, parseMosaicDoc, serializeMosaicDoc, sourceLine, tileCount } from './mosaic';

describe('the mosaic settings', () => {
  it('start with sensible defaults', () => {
    const d = newMosaicDoc('en-GB');
    expect(d).toMatchObject({ v: 1, target: null, zoom: 1, focusX: 0.5, focusY: 0.5, landscape: false, source: 'filtered', blend: 0 });
    expect(d.tileMm).toBe(15);
    expect(d.gap).toBe(8);
  });

  it('come back as they were saved', () => {
    const d = { ...newMosaicDoc(), target: 42, focusX: 0.2, zoom: 2.5, landscape: true, tileMm: 8, source: 'tag:Holiday', gap: 4, blend: 0.3 };
    expect(parseMosaicDoc(serializeMosaicDoc(d))).toEqual(d);
  });

  it('give the defaults for nothing, garbage, or the wrong kind of thing', () => {
    for (const text of [null, '', 'not json', '[]', '7', 'null']) expect(parseMosaicDoc(text)).toEqual(newMosaicDoc());
  });

  it('are kept in range, field by field', () => {
    const d = parseMosaicDoc(JSON.stringify({ v: 1, target: 'x', focusX: 5, focusY: -1, zoom: 99, paper: 'nonsense', tileMm: 1, source: 'else', gap: 0, blend: 7 }));
    expect(d).toMatchObject({ target: null, focusX: 1, focusY: 0, zoom: 4, tileMm: 3, source: 'filtered', gap: 1, blend: 1 });
    expect(d.paper).toBe(newMosaicDoc().paper);
    expect(parseMosaicDoc(JSON.stringify({ tileMm: 5000, gap: 900 }))).toMatchObject({ tileMm: 100, gap: 50 });
  });

  it('turn the page to suit the orientation, and use 150 dpi only for posters', () => {
    const d = { ...newMosaicDoc(), paper: 'a4' };
    expect(mosaicPageMm(d)).toEqual({ w: 210, h: 297 });
    expect(mosaicPageMm({ ...d, landscape: true })).toEqual({ w: 297, h: 210 });
    expect(mosaicDpi(d)).toBe(300);
    expect(mosaicDpi({ ...d, paper: 'a1' })).toBe(150);
  });
});

describe('where the tiles come from', () => {
  it('keeps picked, filtered and a tag, and reads the first version\'s choices as filtered', () => {
    for (const source of ['picked', 'filtered', 'tag:Holiday', 'tag:a b']) expect(parseMosaicDoc(JSON.stringify({ source })).source).toBe(source);
    for (const source of ['library', 'browse']) expect(parseMosaicDoc(JSON.stringify({ source })).source).toBe('filtered');
    for (const source of ['tag:', 'tag', 7, null]) expect(parseMosaicDoc(JSON.stringify({ source })).source).toBe('filtered');
  });
});

describe('how many tiles', () => {
  it('counts whole tiles across and down', () => {
    expect(tileCount(210, 297, 15, 300)).toEqual({ cols: 14, rows: 19, tiles: 266 });
  });

  it('is at least one tile, even when the tile is bigger than the page', () => {
    expect(tileCount(100, 100, 100, 300).tiles).toBe(1);
    expect(tileCount(10, 10, 100, 300)).toEqual({ cols: 1, rows: 1, tiles: 1 });
  });
});

describe('the line about the tiles', () => {
  it('says how many tiles and photos', () => {
    expect(sourceLine(3200, 18000)).toBe('3,200 tiles from 18,000 photos.');
    expect(sourceLine(1, 1)).toBe('1 tile from 1 photo.');
  });

  it('says some will repeat when there are fewer photos than tiles', () => {
    expect(sourceLine(300, 120)).toBe('300 tiles from 120 photos. Some photos will repeat.');
  });
});

describe('a built picture that no longer matches', () => {
  it('is out of date when nothing was built or the key is not known yet', () => {
    expect(isStale(null, 'k')).toBe(true);
    expect(isStale({ key: 'k' }, null)).toBe(true);
  });

  it('is current only while the keys agree', () => {
    expect(isStale({ key: 'k' }, 'k')).toBe(false);
    expect(isStale({ key: 'k' }, 'other')).toBe(true);
  });
});

describe('where saving starts', () => {
  it('is a dated name in the folder, or just the name', () => {
    const day = new Date(2026, 10, 3);
    expect(mosaicSavePath('/Users/me/Pictures', day)).toBe('/Users/me/Pictures/mosaic_2026-11-03.png');
    expect(mosaicSavePath(null, day)).toBe('mosaic_2026-11-03.png');
  });
});
