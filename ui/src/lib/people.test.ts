import { describe, expect, it } from 'vitest';
import { faceCardLine, peopleLine, personLabel, personRows, resolveFaceName, sameNamePerson, type PeopleStatus, type PersonInfo } from './people';
import { faceUrl } from './url';
import { searchActive } from './tagNav';

const status = (o: Partial<PeopleStatus> = {}): PeopleStatus => ({
  models: 'ready', model_done: 0, model_total: 0, done: 0, total: 0, paused: false, error: null, ...o,
});
const people = (n: number): PersonInfo[] => Array.from({ length: n }, (_, i) => ({ id: i + 1, name: null, count: n - i, face_id: i + 100 }));

describe('personLabel', () => {
  it('uses the name, or says the person is unnamed', () => {
    expect(personLabel({ name: 'Gran' })).toBe('Gran');
    expect(personLabel({ name: null })).toBe('Unnamed person');
    expect(personLabel({ name: '' })).toBe('Unnamed person');
  });
});

describe('peopleLine', () => {
  it('describes each state', () => {
    expect(peopleLine(status({ models: 'missing' }))).toBe('Face models not downloaded yet');
    expect(peopleLine(status({ models: 'downloading', model_done: 5, model_total: 20 }))).toBe('Downloading face models… 25%');
    expect(peopleLine(status({ models: 'error', error: 'offline' }))).toBe('Could not get the face models: offline');
    expect(peopleLine(status({ total: 0 }))).toBe('No photos to check for faces yet');
    expect(peopleLine(status({ done: 1200, total: 5000 }))).toBe('Looking for people… 1,200 of 5,000 photos');
    expect(peopleLine(status({ done: 5000, total: 5000 }))).toBe('Faces checked in all 5,000 photos');
    expect(peopleLine(status({ done: 10, total: 50, paused: true }))).toBe('Paused at 10 of 50');
    expect(peopleLine(status({ done: 10, total: 50, error: 'out of memory' }))).toBe('People search is having trouble: out of memory');
  });
});

describe('personRows', () => {
  it('shows the first few and says how many are hidden', () => {
    const r = personRows(people(20), null, false, 8);
    expect(r.rows).toHaveLength(8);
    expect(r.hidden).toBe(12);
  });
  it('shows everyone when expanded or when the list is short', () => {
    expect(personRows(people(20), null, true, 8)).toEqual({ rows: people(20), hidden: 0 });
    expect(personRows(people(5), null, false, 8)).toEqual({ rows: people(5), hidden: 0 });
  });
  it('never hides the chosen person', () => {
    const r = personRows(people(20), 18, false, 8);
    expect(r.rows).toHaveLength(8);
    expect(r.rows.map((p) => p.id)).toContain(18);
  });
});

describe('faceUrl', () => {
  it('uses the pixie scheme for the platform', () => {
    expect(faceUrl(42, true)).toBe('http://pixie.localhost/face/42');
    expect(faceUrl(42, false)).toBe('pixie://localhost/face/42');
  });
  it('asks for a bigger picture when a size is given', () => {
    expect(faceUrl(42, true, 160)).toBe('http://pixie.localhost/face/42?s=160');
    expect(faceUrl(42, false, 320)).toBe('pixie://localhost/face/42?s=320');
  });
});

describe('searchActive with a person', () => {
  it('is on when any of search, tag or person is applied', () => {
    expect(searchActive(null, null, null)).toBe(false);
    expect(searchActive(null, null, 3)).toBe(true);
    expect(searchActive('dog', null, null)).toBe(true);
    expect(searchActive(null, 'beach', null)).toBe(true);
  });
});

describe('faceCardLine', () => {
  it('counts photos and faces in plain words', () => {
    expect(faceCardLine({ photos: 12, faces: 15 })).toBe('12 photos · 15 faces');
    expect(faceCardLine({ photos: 1, faces: 1 })).toBe('1 photo · 1 face');
    expect(faceCardLine({ photos: 1200, faces: 1500 })).toBe('1,200 photos · 1,500 faces');
  });
});

describe('sameNamePerson', () => {
  const list = [{ id: 1, name: 'Gran' }, { id: 2, name: null }, { id: 3, name: 'Sam Smith' }];
  it('finds another person with the same name, ignoring case and spaces', () => {
    expect(sameNamePerson(list, ' gran ', 2)?.id).toBe(1);
    expect(sameNamePerson(list, 'SAM SMITH', 2)?.id).toBe(3);
  });
  it('does not match the person being named, a blank name, or a new name', () => {
    expect(sameNamePerson(list, 'Gran', 1)).toBeNull();
    expect(sameNamePerson(list, '   ', 2)).toBeNull();
    expect(sameNamePerson(list, 'Grandad', 2)).toBeNull();
  });
});

describe('resolveFaceName', () => {
  const list = [{ id: 1, name: 'Gran' }, { id: 2, name: 'Sam Smith' }];
  it('does nothing for a blank name or the name the face already has', () => {
    expect(resolveFaceName(list, '   ', 1)).toEqual({ kind: 'none' });
    expect(resolveFaceName(list, 'gran', 1)).toEqual({ kind: 'none' });
  });
  it('moves the face to the person with that name, ignoring case and spaces', () => {
    expect(resolveFaceName(list, ' sam smith ', 1)).toEqual({ kind: 'move', personId: 2 });
    expect(resolveFaceName(list, 'Gran', null)).toEqual({ kind: 'move', personId: 1 });
  });
  it('starts a new person for a name nobody has', () => {
    expect(resolveFaceName(list, ' Grandad ', 1)).toEqual({ kind: 'new', name: 'Grandad' });
  });
});
