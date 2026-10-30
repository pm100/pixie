import { describe, expect, it } from 'vitest';
import { countOf, emptyLast, indexFacets, tripsHeld, type FacetCounts } from './facets';

const counts: FacetCounts = {
  photos: 1180, videos: 60,
  folders: [[1, 900], [2, 340]],
  tags: [['beach', 40], ['dog', 7]],
  tag_groups: [44, 7], tag_group_names: ['Settings', 'Animals'],
  people: [[5, 12]],
  trips: [30, 0],
  recent: 21,
  cameras: [['Pixel 7', 800], ['Samsung SM-G930A', 400]],
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

describe('the cameras', () => {
  it('keep the order the backend gave them (most photos first)', () => {
    expect(indexFacets(counts, []).cameras).toEqual([['Pixel 7', 800], ['Samsung SM-G930A', 400]]);
  });
  it('are an empty list from a backend that does not send them', () => {
    const { cameras, ...old } = counts;
    expect(indexFacets(old as FacetCounts, []).cameras).toEqual([]);
  });
});

describe('emptyLast', () => {
  const rows = [{ id: 'a', n: 0 }, { id: 'b', n: 3 }, { id: 'c', n: 0 }, { id: 'd', n: 1 }];

  it('puts the empty entries after the others, each group in its own order', () => {
    expect(emptyLast(rows, (r) => r.n === 0).map((r) => r.id)).toEqual(['b', 'd', 'a', 'c']);
  });

  it('leaves the order alone when nothing is empty', () => {
    const full = rows.filter((r) => r.n > 0);
    expect(emptyLast(full, (r) => r.n === 0)).toEqual(full);
  });

  it('does not change the list it was given', () => {
    const copy = [...rows];
    emptyLast(rows, (r) => r.n === 0);
    expect(rows).toEqual(copy);
  });

  it('orders a parent and its children separately', () => {
    const tree = [
      { id: 'x', n: 0, kids: [{ id: 'x1', n: 0 }, { id: 'x2', n: 0 }] },
      { id: 'y', n: 2, kids: [{ id: 'y1', n: 0 }, { id: 'y2', n: 5 }] },
    ];
    const out = emptyLast(tree, (t) => t.n === 0).map((t) => ({ ...t, kids: emptyLast(t.kids, (k) => k.n === 0) }));
    expect(out.map((t) => t.id)).toEqual(['y', 'x']);
    expect(out[0].kids.map((k) => k.id)).toEqual(['y2', 'y1']);
    expect(out[1].kids.map((k) => k.id)).toEqual(['x1', 'x2']);
  });
});
