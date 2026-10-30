import { describe, expect, it } from 'vitest';
import { clearAllTitle, filterNames, offerClearAll, type ActiveFilters } from './filterSummary';

const none: ActiveFilters = { tag: null, person: null, folder: null, trip: null, date: null, kind: null, area: false };

describe('filterNames', () => {
  it('is empty with nothing chosen', () => {
    expect(filterNames(none)).toEqual([]);
  });
  it('names each filter as its pill reads, in the pills\' order', () => {
    const all: ActiveFilters = { tag: 'Beach', person: 'Anna', camera: 'Pixel 7', folder: 'Pictures', trip: 'Oregon', date: 'Showing only September 2026', kind: 'video', area: true };
    expect(filterNames(all)).toEqual(['Auto tagged “Beach”', 'Person “Anna”', 'Camera “Pixel 7”', 'Videos only', 'In the map area', 'Folder “Pictures”', 'Trip “Oregon”', 'Showing only September 2026']);
  });
});

describe('Clear all', () => {
  it('is offered from two filters up, not for one or none', () => {
    expect(offerClearAll([])).toBe(false);
    expect(offerClearAll(filterNames({ ...none, tag: 'Beach' }))).toBe(false);
    expect(offerClearAll(filterNames({ ...none, tag: 'Beach', kind: 'photo' }))).toBe(true);
  });
  it('says in its tooltip what it will clear', () => {
    expect(clearAllTitle(filterNames({ ...none, tag: 'Beach', kind: 'photo' }))).toBe('Clear Auto tagged “Beach”, Photos only');
  });
});

describe('Picked and your own tags', () => {
  it('are named as the chips read, right after the Auto tag', () => {
    expect(filterNames({ ...none, utag: 'picked' })).toEqual(['Picked']);
    expect(filterNames({ ...none, tag: 'Beach', utag: 'tag:Trip 2024' })).toEqual(['Auto tagged “Beach”', 'Tagged “Trip 2024”']);
    expect(filterNames({ ...none, utag: null })).toEqual([]);
  });
});

describe('missing data', () => {
  it('reads "No location" after the camera, before the media type', () => {
    expect(filterNames({ ...none, camera: 'Pixel 7', missing: 'location', kind: 'photo' })).toEqual(['Camera “Pixel 7”', 'No location', 'Photos only']);
  });
});
