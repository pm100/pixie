import { describe, expect, it } from 'vitest';
import { ALL_ON, clearsFilters, parseOptions } from './analysisOptions';

describe('parseOptions', () => {
  it('has everything on unless it is said to be off', () => {
    expect(parseOptions(undefined)).toEqual(ALL_ON);
    expect(parseOptions({})).toEqual(ALL_ON);
    expect(parseOptions({ tags: false, faces: true, dupes: false })).toEqual({ tags: false, faces: true, dupes: false });
    expect(parseOptions('nonsense')).toEqual(ALL_ON);
  });
});

describe('clearsFilters', () => {
  it('drops the tag or person choice when its analysis is turned off', () => {
    expect(clearsFilters(ALL_ON)).toEqual({ tag: false, person: false });
    expect(clearsFilters({ tags: false, faces: true, dupes: true })).toEqual({ tag: true, person: false });
    expect(clearsFilters({ tags: true, faces: false, dupes: true })).toEqual({ tag: false, person: true });
  });
});
