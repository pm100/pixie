import { describe, expect, it } from 'vitest';
import { parseMoved, parseShow } from './popout';

const view = { order: 'newest', folderId: null, kind: null, bounds: null, maxId: 9, since: null, until: null };

describe('parseShow', () => {
  it('accepts a well-formed message', () => {
    expect(parseShow({ index: 2, total: 10, view })).toEqual({ index: 2, total: 10, view, follow: false });
    expect(parseShow({ index: 0, total: 1, view, follow: true })?.follow).toBe(true);
  });
  it('rejects bad indexes, totals and views', () => {
    expect(parseShow(null)).toBeNull();
    expect(parseShow({ index: -1, total: 10, view })).toBeNull();
    expect(parseShow({ index: 10, total: 10, view })).toBeNull();
    expect(parseShow({ index: 'a', total: 10, view })).toBeNull();
    expect(parseShow({ index: 0, total: 0, view })).toBeNull();
    expect(parseShow({ index: 0, total: 1, view: { order: 'sideways' } })).toBeNull();
  });
});

describe('parseMoved', () => {
  it('carries the index with the listing it refers to', () => {
    expect(parseMoved({ index: 4, total: 10, view })).toEqual({ index: 4, total: 10, view });
  });
  it('needs a whole index inside the total, a total and a valid view', () => {
    expect(parseMoved({ index: 1.5, total: 10, view })).toBeNull();
    expect(parseMoved({ index: -2, total: 10, view })).toBeNull();
    expect(parseMoved({ index: 10, total: 10, view })).toBeNull();
    expect(parseMoved({ index: 4, view })).toBeNull();
    expect(parseMoved({ index: 4, total: 10 })).toBeNull();
    expect(parseMoved(undefined)).toBeNull();
  });
});
