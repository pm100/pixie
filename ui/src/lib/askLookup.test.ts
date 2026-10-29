import { describe, expect, it } from 'vitest';
import { answerLookup, countOf, type LookupRequest } from './askLookup';
import type { FindArgs } from './askFind';
import type { Library } from './askLibrary';

const lib: Library = {
  photos: 1000, picked: 2,
  tags: [{ name: 'holiday', count: 120 }, { name: 'beach day', count: 30 }],
  people: [{ name: 'Rose Smith', count: 45 }, { name: 'Sam', count: 10 }],
  trips: [
    { name: 'Italy', from: '2019-06-01', to: '2019-06-14', nights: 13, photos: 340, videos: 6, where: 'Rome' },
    { name: null, from: '2020-01-01', to: '2020-01-03', nights: 2, photos: 50, videos: 0, where: null },
  ],
};
const args = (over: Partial<FindArgs> = {}): FindArgs => ({ text: null, also: [], withPeople: false, tags: [], people: [], from: null, to: null, kind: null, limit: null, pickedOnly: false, then: [], tag: null, ...over });
const req = (over: Partial<LookupRequest>): LookupRequest => ({ id: 'a', input: {}, what: 'tags', query: null, filter: null, ...over });
const known = { tags: ['holiday', 'beach day'], people: [{ id: 1, name: 'Rose Smith' }, { id: 2, name: 'Sam' }] };

describe('answerLookup, names', () => {
  it('lists the tags, people or trips whose name contains what was asked, with counts', async () => {
    expect(JSON.parse(await answerLookup(req({ what: 'people', query: 'rose' }), { library: lib, known, find: async () => ({ ids: [], scores: null, analysed: null }) })))
      .toEqual([{ name: 'Rose Smith', count: 45 }]);
    expect(JSON.parse(await answerLookup(req({ what: 'tags', query: 'BEACH' }), { library: lib, known, find: async () => ({ ids: [], scores: null, analysed: null }) })))
      .toEqual([{ name: 'beach day', count: 30 }]);
  });

  it('lists up to twenty when no name is asked for', async () => {
    const many: Library = { ...lib, tags: Array.from({ length: 50 }, (_, i) => ({ name: `t${i}`, count: i })) };
    const out = JSON.parse(await answerLookup(req({ what: 'tags' }), { library: many, known, find: async () => ({ ids: [], scores: null, analysed: null }) }));
    expect(out).toHaveLength(20);
  });

  it('describes trips by their dates, name, place, length and number of photos', async () => {
    const out = JSON.parse(await answerLookup(req({ what: 'trips', query: 'rome' }), { library: lib, known, find: async () => ({ ids: [], scores: null, analysed: null }) }));
    expect(out).toEqual([{ name: 'Italy', from: '2019-06-01', to: '2019-06-14', where: 'Rome', nights: 13, photos: 340 }]);
    const none = JSON.parse(await answerLookup(req({ what: 'trips', query: 'paris' }), { library: lib, known, find: async () => ({ ids: [], scores: null, analysed: null }) }));
    expect(none).toEqual([]);
  });

  it('says it is not allowed, and tells nothing, when the person has not shared the names', async () => {
    const out = JSON.parse(await answerLookup(req({ what: 'people', query: 'rose' }), { library: null, known, find: async () => ({ ids: [], scores: null, analysed: null }) }));
    expect(out).toEqual({ error: 'The user has not allowed Ask Pixie to see names.' });
  });

  it('stays within what the proxy takes', async () => {
    const wide: Library = { ...lib, tags: Array.from({ length: 50 }, (_, i) => ({ name: 'x'.repeat(60) + i, count: i })) };
    expect((await answerLookup(req({ what: 'tags' }), { library: wide, known, find: async () => ({ ids: [], scores: null, analysed: null }) })).length).toBeLessThanOrEqual(2000);
  });
});

describe('answerLookup, count', () => {
  it('counts the photos a filter finds', async () => {
    const find = async () => ({ ids: [1, 2, 3], scores: null, analysed: null });
    expect(JSON.parse(await answerLookup(req({ what: 'count', filter: args({ tags: ['holiday'] }) }), { library: null, known, find }))).toEqual({ count: 3 });
  });

  it('counts everything when there is no filter', async () => {
    const find = async () => ({ ids: [1, 2], scores: null, analysed: null });
    expect(JSON.parse(await answerLookup(req({ what: 'count', filter: null }), { library: null, known, find }))).toEqual({ count: 2 });
  });

  it('does not count a name that is not there, and says why', async () => {
    let ran = false;
    const find = async () => { ran = true; return { ids: [1], scores: null, analysed: null }; };
    const out = JSON.parse(await answerLookup(req({ what: 'count', filter: args({ tags: ['wedding'] }) }), { library: null, known, find }));
    expect(out).toEqual({ error: 'You have no tag called “wedding”.' });
    expect(ran).toBe(false);
  });

  it('gives an error in words when the search fails', async () => {
    const find = async () => { throw new Error('The search model is not ready yet.'); };
    expect(JSON.parse(await answerLookup(req({ what: 'count', filter: args({ text: 'dog' }) }), { library: null, known, find }))).toEqual({ error: 'The search model is not ready yet.' });
  });
});

describe('countOf', () => {
  it('counts the close matches of a content search, as the card would show them', () => {
    const scores = [0.30, 0.29, 0.28, 0.27, 0.26, 0.25];
    expect(countOf({ ids: [1, 2, 3, 4, 5, 6], scores, analysed: 99 }, null)).toBe(4);
    expect(countOf({ ids: [1, 2, 3, 4, 5, 6], scores, analysed: 99 }, 2)).toBe(2);
  });
  it('counts all of a search that has no scores', () => {
    expect(countOf({ ids: [1, 2, 3], scores: null, analysed: null }, null)).toBe(3);
  });
});
