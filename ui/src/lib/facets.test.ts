import { describe, expect, it } from 'vitest';
import { countOf, indexFacets, tripsHeld, type FacetCounts } from './facets';

const counts: FacetCounts = {
  photos: 1180, videos: 60,
  folders: [[1, 900], [2, 340]],
  tags: [['beach', 40], ['dog', 7]],
  tag_groups: [44, 7], tag_group_names: ['Settings', 'Animals'],
  people: [[5, 12]],
  trips: [30, 0],
  recent: 21,
};
const trips = [{ since: 1000 }, { since: 2000 }];

describe('indexFacets', () => {
  it('lets each entry be looked up by what the tree shows', () => {
    const f = indexFacets(counts, trips);
    expect([f.photos, f.videos]).toEqual([1180, 60]);
    expect(f.folders.get(2)).toBe(340);
    expect(f.tags.get('beach')).toBe(40);
    expect(f.categories.get('Animals')).toBe(7);
    expect(f.people.get(5)).toBe(12);
    expect(f.trips.get(1000)).toBe(30);
    expect(f.trips.get(2000)).toBe(0);
    expect(f.recent).toBe(21);
  });
});

describe('countOf', () => {
  const f = indexFacets(counts, trips);
  it('uses the filtered count, and 0 for an entry that is not among them', () => {
    expect(countOf(f.tags, 'beach', 999)).toBe(40);
    expect(countOf(f.tags, 'cat', 999)).toBe(0);
  });
  it('falls back to the count the entry came with until the filtered counts are known', () => {
    expect(countOf(undefined, 'beach', 123)).toBe(123);
  });
});

describe('tripsHeld', () => {
  it('counts the trips with something in them under the filters, or all of them without', () => {
    expect(tripsHeld(trips, indexFacets(counts, trips))).toBe(1);
    expect(tripsHeld(trips, null)).toBe(2);
  });
});
