import { describe, expect, it } from 'vitest';
import { dupLine, extrasOf, totalSize, type DupGroup } from './dupes';
import type { Photo } from './api';

const photo = (id: number, size = 100): Photo =>
  ({ id, folder_id: 1, path: `p${id}.jpg`, size, mtime: 1, width: null, height: null, taken_at: null, camera: null, gps_lat: null, gps_lon: null, kind: 'photo', duration_ms: null, missing: false });
const group = (hash: string, keeper: number, ...ids: number[]): DupGroup => ({ hash, keeper, photos: ids.map((i) => photo(i)) });

describe('extrasOf', () => {
  it('is every copy but one per set, so a whole set can never be listed', () => {
    expect(extrasOf([group('a', 1, 1, 2, 3), group('b', 4, 4, 5)]).map((p) => p.id)).toEqual([2, 3, 5]);
    expect(extrasOf([group('a', 2, 1, 2, 3), group('b', 5, 4, 5)]).map((p) => p.id)).toEqual([1, 3, 4]);
  });
  it('sizes add up', () => {
    expect(totalSize([photo(1, 10), photo(2, 32)])).toBe(42);
  });
});

describe('dupLine', () => {
  it('says what it found', () => {
    expect(dupLine({ done: 5, total: 5, groups: 0, extra_files: 0, extra_bytes: 0, hidden: 0, paused: false })).toBe('No duplicates found');
    expect(dupLine({ done: 5, total: 5, groups: 2, extra_files: 3, extra_bytes: 2048, hidden: 3, paused: false })).toBe('2 sets of duplicates · 3 extra files · 2.0 KB could be freed');
    expect(dupLine({ done: 5, total: 5, groups: 1, extra_files: 1, extra_bytes: 10, hidden: 1, paused: false })).toContain('1 set of duplicates · 1 extra file ·');
  });
  it('says how far the checking has got', () => {
    expect(dupLine({ done: 3, total: 10, groups: 0, extra_files: 0, extra_bytes: 0, hidden: 0, paused: false })).toBe('Checking for duplicates: 3 of 10 looked at.');
  });
});
