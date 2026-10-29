import { describe, expect, it } from 'vitest';
import { CUT_FLOOR, defaultCut, describeFind, parseFindArgs, resolveFind, visibleIds, type FindArgs } from './askFind';

const args = (over: Partial<FindArgs> = {}): FindArgs => ({ text: null, also: [], withPeople: false, tags: [], people: [], from: null, to: null, kind: null, limit: null, pickedOnly: false, then: [], tag: null, ...over });
const known = {
  tags: ['Holiday', 'Beach day', 'Pink flowers'],
  people: [{ id: 1, name: 'Joan Smith' }, { id: 2, name: 'Joan Lee' }, { id: 3, name: 'Sam' }, { id: 4, name: null }],
};

describe('parseFindArgs', () => {
  it('reads what the proxy sent and drops what is not the right shape', () => {
    expect(parseFindArgs({ text: 'pink flowers', tags: ['a', 5], people: 'Joan', from: '2019-01-01', to: 'x', kind: 'video', pickedOnly: true, then: ['pick', 'delete', 'tag'], tag: 'x' }))
      .toEqual(args({ text: 'pink flowers', tags: ['a'], from: '2019-01-01', kind: 'video', pickedOnly: true, then: ['pick', 'tag'], tag: 'x' }));
    expect(parseFindArgs({ text: 'a', also: ['b', '', 3, 'c', 'd', 'e'], withPeople: true })).toMatchObject({ also: ['b', 'c', 'd'], withPeople: true });
    expect(parseFindArgs({ withPeople: true })).not.toBeNull();
    expect(parseFindArgs(null)).toBeNull();
    expect(parseFindArgs('x')).toBeNull();
    expect(parseFindArgs({})).toBeNull();
  });

  it('reads how many were asked for, as a whole number of at least 1', () => {
    expect(parseFindArgs({ text: 'a', limit: 20 })?.limit).toBe(20);
    for (const bad of [0, -1, 'x', null, 1.5]) expect(parseFindArgs({ text: 'a', limit: bad })?.limit).toBeNull();
  });

  it('does not tag with a blank name', () => {
    expect(parseFindArgs({ text: 'a', then: ['tag'], tag: '  ' })?.then).toEqual([]);
  });
});

describe('resolveFind', () => {
  it('turns days into the start of the first and the end of the last, inclusive', () => {
    const r = resolveFind(args({ from: '2019-06-01', to: '2019-06-30' }), known);
    expect(r.spec.since).toBe(Date.UTC(2019, 5, 1) / 1000);
    expect(r.spec.until).toBe(Date.UTC(2019, 6, 1) / 1000);
    expect(r.problems).toEqual([]);
  });

  it('matches tags without regard to case, and by a unique part of the name', () => {
    expect(resolveFind(args({ tags: ['holiday'] }), known).spec.tags).toEqual(['Holiday']);
    expect(resolveFind(args({ tags: ['beach'] }), known).spec.tags).toEqual(['Beach day']);
  });

  it('says so when a tag does not exist and when it is ambiguous, and asks nothing to be searched on it', () => {
    const none = resolveFind(args({ tags: ['wedding'] }), known);
    expect(none.problems).toEqual(['You have no tag called “wedding”.']);
    expect(none.spec.tags).toEqual([]);
    const both = resolveFind(args({ tags: ['a'] }), { ...known, tags: ['Beach', 'Holiday'] });
    expect(both.problems[0]).toContain('matches more than one');
  });

  it('matches people by name or by first name, and reports the ones that clash or are unknown', () => {
    expect(resolveFind(args({ people: ['Sam'] }), known).spec.people).toEqual([3]);
    expect(resolveFind(args({ people: ['joan smith'] }), known).spec.people).toEqual([1]);
    const clash = resolveFind(args({ people: ['Joan'] }), known);
    expect(clash.spec.people).toEqual([]);
    expect(clash.problems[0]).toBe('“Joan” could be Joan Smith or Joan Lee.');
    expect(resolveFind(args({ people: ['Zed'] }), known).problems).toEqual(['I do not know anyone called “Zed”.']);
  });

  it('a find with a problem is not run, so a missing tag cannot widen the search to everything', () => {
    expect(resolveFind(args({ tags: ['wedding'] }), known).runnable).toBe(false);
    expect(resolveFind(args({ text: 'dog' }), known).runnable).toBe(true);
  });

  it('carries the text, kind and picked-only into the spec', () => {
    const r = resolveFind(args({ text: ' pink flowers ', kind: 'photo', pickedOnly: true }), known);
    expect(r.texts).toEqual(['pink flowers']);
    expect(r.spec).toMatchObject({ kind: 'photo', picked: true, with_people: false });
  });

  it('keeps every thing that must be in the picture, and asks for people when people are wanted', () => {
    const r = resolveFind(args({ text: 'yellow flowers', also: [' a birthday cake '], withPeople: true }), known);
    expect(r.texts).toEqual(['yellow flowers', 'a birthday cake']);
    expect(r.spec.with_people).toBe(true);
    expect(r.runnable).toBe(true);
    expect(resolveFind(args({ withPeople: true }), known).runnable).toBe(true);
    expect(resolveFind(args(), known).texts).toEqual([]);
  });

  it('is not runnable when it asks for nothing', () => {
    expect(resolveFind(args(), known).runnable).toBe(false);
  });
});

describe('defaultCut', () => {
  const scores = [0.30, 0.29, 0.28, 0.27, 0.26, 0.25, 0.24, 0.23, 0.22, 0.21];

  it('keeps the close matches: those within 0.03 of the best', () => {
    const cut = defaultCut(scores, null);
    expect(visibleIds(scores.map((_, i) => i), scores, cut)).toEqual([0, 1, 2, 3]);
  });

  it('with a number asked for, keeps exactly that many of the best', () => {
    const cut = defaultCut(scores, 5);
    expect(visibleIds(scores.map((_, i) => i), scores, cut)).toEqual([0, 1, 2, 3, 4]);
  });

  it('asked for more than there are, keeps them all', () => {
    expect(visibleIds(scores.map((_, i) => i), scores, defaultCut(scores, 50))).toHaveLength(10);
  });

  it('never goes below the floor of the search, however weak the best match', () => {
    expect(defaultCut([0.21, 0.205], null)).toBe(CUT_FLOOR);
  });

  it('has nothing to cut when nothing was found', () => {
    expect(defaultCut([], null)).toBe(CUT_FLOOR);
    expect(visibleIds([], [], CUT_FLOOR)).toEqual([]);
  });
});

describe('visibleIds', () => {
  it('lists the photos at or above the cut, best first, and all of them when there are no scores', () => {
    expect(visibleIds([5, 6, 7], [0.3, 0.25, 0.2], 0.25)).toEqual([5, 6]);
    expect(visibleIds([5, 6, 7], null, 0.25)).toEqual([5, 6, 7]);
  });
});

describe('describeFind', () => {
  it('says every thing looked for, and that people are wanted in them', () => {
    expect(describeFind(args({ text: 'yellow flowers', withPeople: true }))).toBe('photos that look like “yellow flowers”, with people in them');
    expect(describeFind(args({ text: 'yellow flowers', also: ['a birthday cake'] }))).toBe('photos that look like “yellow flowers” and “a birthday cake”');
    expect(describeFind(args({ withPeople: true }))).toBe('photos with people in them');
  });

  it('says how many were asked for', () => {
    expect(describeFind(args({ text: 'pink flowers', limit: 20 }))).toBe('the best 20 photos that look like “pink flowers”');
  });

  it('says in words what is being looked for', () => {
    expect(describeFind(args({ text: 'pink flowers' }))).toBe('photos that look like “pink flowers”');
    expect(describeFind(args({ text: 'dog', people: ['Sam'], from: '2019-06-01', to: '2019-06-30', kind: 'video' }))).toBe('videos that look like “dog”, with Sam, from 2019-06-01 to 2019-06-30');
    expect(describeFind(args({ tags: ['Holiday'], pickedOnly: true }))).toBe('picked photos tagged “Holiday”');
  });
});
