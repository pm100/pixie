import { describe, expect, it } from 'vitest';
import { enterChoice, filterKey, listingAffected, normalizeTag, parseFilterKey, tagAsRows, tagsOnAll } from './userTags';
import type { TagUse } from './api';

const use = (name: string, on: number): TagUse => ({ name, on });
const nine = ['Holiday', 'Kids', 'Recipes', 'Birthdays', 'Roadtrip 2023', 'Wedding', 'Garden', 'Receipts', 'Cats'].map((n) => use(n, 0));

describe('userTags', () => {
  it('tidies a tag name like the library does', () => {
    expect(normalizeTag('  #Holiday  2024 ')).toBe('Holiday 2024');
    expect(normalizeTag(' # ')).toBeNull();
    expect(normalizeTag('x'.repeat(60))).toBe('x'.repeat(60));
    expect(normalizeTag('x'.repeat(61))).toBeNull();
  });
  it('lists six tags and says how many more there are', () => {
    const r = tagAsRows(nine, 2, '');
    expect(r.rows.map((x) => x.name)).toEqual(['Holiday', 'Kids', 'Recipes', 'Birthdays', 'Roadtrip 2023', 'Wedding']);
    expect(r.more).toBe(3);
  });
  it('finds tags by part of the name, ignoring case', () => {
    const r = tagAsRows(nine, 2, 'RE');
    expect(r.rows.map((x) => x.name)).toEqual(['Recipes', 'Receipts']);
    expect(r.more).toBe(0);
  });
  it('marks a tag that every counted photo has, and says how many have the others', () => {
    const r = tagAsRows([use('A', 3), use('B', 1), use('C', 0)], 3, '');
    expect(r.rows).toEqual([{ name: 'A', on: 3, all: true }, { name: 'B', on: 1, all: false }, { name: 'C', on: 0, all: false }]);
    expect(tagAsRows([use('A', 0)], 0, '').rows[0].all).toBe(false);
  });
  it('offers to create what was typed unless a tag with that name exists, in any case', () => {
    expect(tagAsRows(nine, 1, 'Birthday').create).toBe('Birthday');
    expect(tagAsRows(nine, 1, '  #new tag ').create).toBe('new tag');
    expect(tagAsRows(nine, 1, 'holiday').create).toBeNull();
    expect(tagAsRows(nine, 1, '').create).toBeNull();
    expect(tagAsRows(nine, 1, ' # ').create).toBeNull();
  });
  it('lists the tags that every one of the photos has', () => {
    expect(tagsOnAll([use('A', 3), use('B', 1)], 3)).toEqual(['A']);
    expect(tagsOnAll([use('A', 0)], 0)).toEqual([]);
  });
  it('turns the filter into a key and back', () => {
    expect(filterKey({ picked: true })).toBe('picked');
    expect(filterKey({ tag: 'Holiday 2024' })).toBe('tag:Holiday 2024');
    expect(parseFilterKey('picked')).toEqual({ picked: true });
    expect(parseFilterKey('tag:Holiday 2024')).toEqual({ tag: 'Holiday 2024' });
    expect(parseFilterKey('nonsense')).toBeNull();
    expect(parseFilterKey('tag:')).toBeNull();
    expect(parseFilterKey(null)).toBeNull();
  });
  it('Enter takes the one tag that matches what was typed, else makes the tag, else does nothing', () => {
    const u = [use('Holiday', 0), use('Kids', 0), use('Kitchen', 0)];
    expect(enterChoice(u, 2, 'hol')).toBe('Holiday'); // the only match, though not typed in full
    expect(enterChoice(u, 2, 'KIDS')).toBe('Kids'); // exactly a tag, in another case
    expect(enterChoice(u, 2, 'k')).toBeNull(); // two match and neither is exact: pick one in the list
    expect(enterChoice(u, 2, ' #new  tag ')).toBe('new tag'); // nothing matches: a new tag
    expect(enterChoice(u, 2, '')).toBeNull();
    expect(enterChoice(u, 2, ' # ')).toBeNull();
    expect(enterChoice([use('Holiday', 2)], 2, 'holiday')).toBeNull(); // already on all of them
  });
  it('re-lists the photos only when a change touched what is listed', () => {
    expect(listingAffected(null, 'x', true)).toBe(false);
    expect(listingAffected('picked', undefined, true)).toBe(true); // a pick changed
    expect(listingAffected('picked', 'Kids', true)).toBe(false); // a tag changed, Picked did not
    expect(listingAffected('tag:Holiday', 'holiday', true)).toBe(true);
    expect(listingAffected('tag:Holiday', 'Kids', true)).toBe(false);
    expect(listingAffected('tag:Holiday', 'Kids', false)).toBe(true); // the chosen tag went away: the view has to follow
  });
});
