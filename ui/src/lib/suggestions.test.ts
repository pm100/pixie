import { describe, expect, it } from 'vitest';
import { addFound, applyCount, countText, listedRows, lookingText, tickedPaths, toRows, toggle } from './suggestions';
import type { Suggestion } from './api';

const s = (path: string, o: Partial<Suggestion> = {}): Suggestion => ({
  path, name: path.split(/[\\/]/).pop()!, kind: 'known', ticked: true, note: null, included_in: null, ...o,
});

describe('toRows', () => {
  it('starts every row without a count', () => {
    expect(toRows([s('C:\\Users\\me\\Pictures')])[0].count).toBeNull();
  });
});

describe('applyCount', () => {
  it('a count fills the matching row only, whatever the slashes and case', () => {
    const rows = toRows([s('C:\\Users\\me\\Pictures'), s('C:\\Users\\me\\Videos')]);
    const out = applyCount(rows, { path: 'c:/users/ME/pictures/', n: 3400, capped: false });
    expect(out[0].count).toEqual({ n: 3400, capped: false });
    expect(out[1].count).toBeNull();
  });
});

describe('addFound', () => {
  it('adds a new unticked candidate', () => {
    const out = addFound(toRows([s('C:\\Pictures')]), s('C:\\Photos\\Trips', { kind: 'further', ticked: false, note: '412 photos and videos' }));
    expect(out.map((r) => r.path)).toEqual(['C:\\Pictures', 'C:\\Photos\\Trips']);
  });
  it('ignores a folder that is already listed, inside a listed one, or that contains one', () => {
    const rows = toRows([s('C:\\Users\\me\\Pictures')]);
    for (const p of ['c:/users/me/pictures', 'C:\\Users\\me\\Pictures\\2020', 'C:\\Users\\me']) {
      expect(addFound(rows, s(p, { kind: 'further', ticked: false }))).toHaveLength(1);
    }
  });
  it('does not treat a folder with a similar name as inside', () => {
    expect(addFound(toRows([s('C:\\Pictures')]), s('C:\\Pictures2', { kind: 'further', ticked: false }))).toHaveLength(2);
  });
});

describe('toggle', () => {
  it('flips one row only', () => {
    const rows = toRows([s('C:\\A'), s('C:\\B')]);
    const out = toggle(rows, 'C:\\B');
    expect(out.map((r) => r.ticked)).toEqual([true, false]);
    expect(toggle(out, 'C:\\B').map((r) => r.ticked)).toEqual([true, true]);
  });
});

describe('tickedPaths', () => {
  it('never returns a folder known to be empty, even if it is ticked', () => {
    const rows = applyCount(toRows([s('C:\Videos'), s('C:\Pictures')]), { path: 'C:\Videos', n: 0, capped: false });
    expect(tickedPaths(rows)).toEqual(['C:\Pictures']);
  });
  it('returns ticked rows, never one already included in another', () => {
    const rows = toRows([s('C:\\Pictures'), s('C:\\Pictures\\Camera Roll', { included_in: 'Pictures' }), s('C:\\Downloads', { ticked: false })]);
    expect(tickedPaths(rows)).toEqual(['C:\\Pictures']);
  });
});

describe('countText', () => {
  it('reads the count, a cap and no count yet', () => {
    expect(countText(null, 'known')).toBe('counting…');
    expect(countText({ n: 3400, capped: false }, 'known')).toBe('about 3,400 items');
    expect(countText({ n: 10000, capped: true }, 'known')).toBe('10,000+ items');
    expect(countText({ n: 0, capped: false }, 'known')).toBe('no photos or videos');
  });
  it('a found candidate has no count to wait for', () => {
    expect(countText(null, 'further')).toBe('');
  });
});

describe('lookingText', () => {
  it('says what is being looked at, and nothing before the first answer', () => {
    expect(lookingText('your home folder')).toBe('Looking in your home folder…');
    expect(lookingText('your other drives')).toBe('Looking in your other drives…');
    expect(lookingText(null)).toBe('');
  });
});

describe('listedRows', () => {
  const counted = (path: string, n: number, capped = false, o: Partial<Suggestion> = {}) => applyCount(toRows([s(path, o)]), { path, n, capped })[0];
  it('hides folders known to hold nothing, but not ones still being counted or capped', () => {
    const rows = [counted('C:\Empty', 0), counted('C:\Full', 12), counted('C:\Huge', 10000, true), ...toRows([s('C:\Counting')])];
    expect(listedRows(rows).map((r) => r.path)).toEqual(['C:\Full', 'C:\Huge', 'C:\Counting']);
  });
  it('hides folders that are included in another suggestion', () => {
    const rows = toRows([s('C:\Pictures'), s('C:\Pictures\Camera Roll', { included_in: 'Pictures' })]);
    expect(listedRows(rows).map((r) => r.path)).toEqual(['C:\Pictures']);
  });
  it('keeps the order the rows came in', () => {
    const rows = toRows([s('C:\B'), s('C:\A', { kind: 'further' })]);
    expect(listedRows(rows).map((r) => r.path)).toEqual(['C:\B', 'C:\A']);
  });
});
