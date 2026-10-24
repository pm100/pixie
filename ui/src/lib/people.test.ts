import { describe, expect, it } from 'vitest';
import { clickSelection, faceCardLine, firstSection, peopleLine, personLabel, personRows, pickable, refreshOffered, suggestionLine, verdict, type FaceOverview, type PeopleStatus, type PersonInfo } from './people';
import { faceUrl } from './url';
import { hitsActive } from './tagNav';

const status = (o: Partial<PeopleStatus> = {}): PeopleStatus => ({
  models: 'ready', model_done: 0, model_total: 0, done: 0, total: 0, paused: false, rev: 0, error: null, ...o,
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
    expect(peopleLine(status({ done: 10, total: 50, error: 'out of memory' }))).toBe('Face finding is having trouble: out of memory');
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

describe('hitsActive with a person', () => {
  it('is on when a tag or a person is applied', () => {
    expect(hitsActive(null, null)).toBe(false);
    expect(hitsActive(null, 3)).toBe(true);
    expect(hitsActive('beach', null)).toBe(true);
  });
});

describe('faceCardLine', () => {
  it('counts photos in plain words', () => {
    expect(faceCardLine({ photos: 12 })).toBe('12 photos');
    expect(faceCardLine({ photos: 1 })).toBe('1 photo');
    expect(faceCardLine({ photos: 1200 })).toBe('1,200 photos');
  });
});

describe('firstSection', () => {
  const o = (p: Partial<FaceOverview>): FaceOverview => ({ groups: 0, people: 0, unsorted: 0, hidden: 0, unchecked: 0, rev: 0, ...p });
  it('opens on what there is to do: groups to name first', () => {
    expect(firstSection(o({ groups: 3, people: 2 }))).toBe('groups');
    expect(firstSection(o({ people: 2, unsorted: 9 }))).toBe('people');
    expect(firstSection(o({ unsorted: 9, hidden: 1 }))).toBe('unsorted');
    expect(firstSection(o({ hidden: 1 }))).toBe('hidden');
    expect(firstSection(o({}))).toBe('groups');
  });
});

describe('verdict', () => {
  it('says what the faces look like, or nothing when it was not measured', () => {
    expect(verdict({ mixed: null, looks_mixed: false })).toBeNull();
    expect(verdict({ mixed: 0.01, looks_mixed: false })).toEqual({ text: 'Looks like one person', mixed: false });
    expect(verdict({ mixed: 0.3, looks_mixed: true })).toEqual({ text: 'Looks like several people', mixed: true });
  });
});

describe('suggestionLine', () => {
  it('names who a group looks like', () => {
    expect(suggestionLine({ person_id: 1, name: 'Gran', score: 0.6 })).toBe('Looks like Gran');
    expect(suggestionLine({ person_id: 1, name: null, score: 0.6 })).toBeNull();
    expect(suggestionLine(null)).toBeNull();
  });
});

describe('pickable', () => {
  const ppl = [
    { id: 1, name: 'sam', hidden: false },
    { id: 2, name: null, hidden: false },
    { id: 3, name: 'Gran', hidden: false },
    { id: 4, name: 'Zed', hidden: true },
  ];
  it('offers named, visible people in name order, never the one it already is', () => {
    expect(pickable(ppl, null).map((p) => p.name)).toEqual(['Gran', 'sam']);
    expect(pickable(ppl, 3).map((p) => p.name)).toEqual(['sam']);
  });
});

describe('clickSelection', () => {
  const order = [10, 11, 12, 13, 14];
  it('a click picks a face and a second click takes it out', () => {
    const a = clickSelection(order, new Set(), null, 12, false);
    expect([...a.picked]).toEqual([12]);
    const b = clickSelection(order, a.picked, a.anchor, 12, false);
    expect([...b.picked]).toEqual([]);
  });
  it('shift-click picks everything between the last click and this one, in either direction', () => {
    const a = clickSelection(order, new Set(), null, 11, false);
    expect([...clickSelection(order, a.picked, a.anchor, 13, true).picked].sort()).toEqual([11, 12, 13]);
    const c = clickSelection(order, new Set(), null, 14, false);
    expect([...clickSelection(order, c.picked, c.anchor, 12, true).picked].sort()).toEqual([12, 13, 14]);
  });
  it('shift-click with nothing clicked yet just picks that face', () => {
    expect([...clickSelection(order, new Set(), null, 13, true).picked]).toEqual([13]);
  });
});

describe('refreshOffered', () => {
  it('offers a refresh only when faces were regrouped after the list on screen was loaded', () => {
    expect(refreshOffered(5, 4)).toBe(true);
    expect(refreshOffered(4, 4)).toBe(false);
    expect(refreshOffered(3, 4)).toBe(false);
    expect(refreshOffered(undefined, 4)).toBe(false);
    expect(refreshOffered(5, null)).toBe(false);
  });
});
