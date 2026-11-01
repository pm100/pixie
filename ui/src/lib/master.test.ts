import { describe, expect, it } from 'vitest';
import { hitsActive } from './tagNav';
import { filterNames } from './filterSummary';
import { cleanCollapsed } from './sections';
import { reviewLine, sharingRows } from './master';

describe('without a master folder', () => {
  it('has no rows in the Browse tree, whatever the counts and choice are', () => {
    expect(sharingRows(null, { held: 5, private: 2 }, 'new')).toEqual([]);
  });
});

describe('with a master folder', () => {
  it('lists New always, so the person can see it is there, and Private once there is something private', () => {
    expect(sharingRows('/family', { held: 0, private: 0 }, null).map((r) => r.id)).toEqual(['new']);
    expect(sharingRows('/family', { held: 3, private: 2 }, null).map((r) => [r.id, r.count])).toEqual([['new', 3], ['private', 2]]);
  });
  it('keeps Private listed while it is the choice, even when the last private item has gone', () => {
    expect(sharingRows('/family', { held: 0, private: 0 }, 'private').map((r) => r.id)).toEqual(['new', 'private']);
  });
  it('marks the chosen row', () => {
    expect(sharingRows('/family', { held: 3, private: 2 }, 'private').map((r) => r.active)).toEqual([false, true]);
  });
  it('explains each row in a whole sentence', () => {
    for (const r of sharingRows('/family', { held: 3, private: 2 }, null)) expect(r.title).toMatch(/\.$/);
  });
});

describe('the review line', () => {
  it('says how many items wait, pluralised only when there are several', () => {
    expect(reviewLine(1)).toBe('1 item is new and not yet shared.');
    expect(reviewLine(212)).toBe('212 items are new and not yet shared.');
    expect(reviewLine(0)).toBe('Nothing is waiting to be shared.');
  });
});

describe('the share choice elsewhere', () => {
  it('limits the listing to hits, like the other tree choices', () => {
    expect(hitsActive(null, null, null, null, null, 'new')).toBe(true);
    expect(hitsActive(null, null, null, null, null, null)).toBe(false);
  });
  it('reads as a filter pill', () => {
    const f = { tag: null, person: null, folder: null, trip: null, date: null, kind: null, area: false };
    expect(filterNames({ ...f, share: 'new' })).toEqual(['New, not yet shared']);
    expect(filterNames({ ...f, share: 'private' })).toEqual(['Private']);
  });
  it('is a section the saved collapsed state knows', () => {
    expect(cleanCollapsed(['sharing', 'nonsense'])).toEqual(['sharing']);
  });
});
