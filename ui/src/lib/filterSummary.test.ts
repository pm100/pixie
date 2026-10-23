import { describe, expect, it } from 'vitest';
import { clearAllTitle, filterNames, offerClearAll, type ActiveFilters } from './filterSummary';

const none: ActiveFilters = { tag: null, person: null, folder: null, trip: null, date: null, kind: null };

describe('filterNames', () => {
  it('is empty with nothing chosen', () => {
    expect(filterNames(none)).toEqual([]);
  });
  it('names each filter as its pill reads, in the pills\' order', () => {
    const all: ActiveFilters = { tag: 'Beach', person: 'Anna', folder: 'Pictures', trip: 'Oregon', date: 'Showing only September 2026', kind: 'video' };
    expect(filterNames(all)).toEqual(['Tagged “Beach”', 'Person “Anna”', 'Videos only', 'Folder “Pictures”', 'Trip “Oregon”', 'Showing only September 2026']);
  });
});

describe('Clear all', () => {
  it('is offered from two filters up, not for one or none', () => {
    expect(offerClearAll([])).toBe(false);
    expect(offerClearAll(filterNames({ ...none, tag: 'Beach' }))).toBe(false);
    expect(offerClearAll(filterNames({ ...none, tag: 'Beach', kind: 'photo' }))).toBe(true);
  });
  it('says in its tooltip what it will clear', () => {
    expect(clearAllTitle(filterNames({ ...none, tag: 'Beach', kind: 'photo' }))).toBe('Clear Tagged “Beach”, Photos only');
  });
});
