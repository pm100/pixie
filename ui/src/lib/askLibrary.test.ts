import { describe, expect, it } from 'vitest';
import { buildLibrary, MAX_PEOPLE, MAX_TAGS, MAX_TRIPS } from './askLibrary';
import type { Trip } from './api';

const trip = (over: Partial<Trip> = {}): Trip =>
  ({ start: '2019-06-01', end: '2019-06-14', since: 1, until: 2, nights: 13, days: 14, photos: 340, videos: 6, name: null, ...over }) as unknown as Trip;

describe('buildLibrary', () => {
  const src = {
    total: 12_345, picked: 7,
    tags: [{ name: 'holiday', count: 120 }, { name: ' beach day ', count: 30 }, { name: '', count: 5 }],
    people: [{ id: 1, name: 'Rose', count: 45 }, { id: 2, name: null, count: 99 }, { id: 3, name: '  ', count: 4 }, { id: 4, name: 'Sam', count: 10 }],
    trips: [trip({ name: 'Italy' }), trip({ start: '2020-01-01', end: '2020-01-03', nights: 2, photos: 50, videos: 0 })],
    destinations: { 2: 'Lisbon' } as Record<number, string>,
  };

  it('counts, and lists the tags, the named people and the trips', () => {
    const lib = buildLibrary(src);
    expect(lib.photos).toBe(12_345);
    expect(lib.picked).toBe(7);
    expect(lib.tags).toEqual([{ name: 'holiday', count: 120 }, { name: 'beach day', count: 30 }]);
    expect(lib.people).toEqual([{ name: 'Rose', count: 45 }, { name: 'Sam', count: 10 }]);
    expect(lib.trips.map((t) => t.from)).toEqual(['2020-01-01', '2019-06-01']); // newest first
    expect(lib.trips.find((t) => t.name === 'Italy')).toEqual({ name: 'Italy', from: '2019-06-01', to: '2019-06-14', nights: 13, photos: 340, videos: 6, where: null });
  });

  it('leaves out people who have no name and tags with none', () => {
    const lib = buildLibrary(src);
    expect(lib.people.map((p) => p.name)).not.toContain(null);
    expect(lib.tags.map((t) => t.name)).not.toContain('');
  });

  it('says where a trip went when that is known', () => {
    const lib = buildLibrary({ ...src, trips: [trip({ since: 2 })] });
    expect(lib.trips[0].where).toBe('Lisbon');
  });

  it('keeps the biggest tags and people when there are more than fit, and the newest trips', () => {
    const many = Array.from({ length: MAX_TAGS + 20 }, (_, i) => ({ name: `t${i}`, count: i }));
    const lib = buildLibrary({ ...src, tags: many, people: Array.from({ length: MAX_PEOPLE + 5 }, (_, i) => ({ id: i, name: `p${i}`, count: i })), trips: Array.from({ length: MAX_TRIPS + 5 }, (_, i) => trip({ start: `2000-01-${String((i % 28) + 1).padStart(2, '0')}`, since: i })) });
    expect(lib.tags).toHaveLength(MAX_TAGS);
    expect(lib.tags[0].count).toBe(MAX_TAGS + 19);
    expect(lib.people).toHaveLength(MAX_PEOPLE);
    expect(lib.trips).toHaveLength(MAX_TRIPS);
  });

  it('cuts a very long name, so one odd tag cannot fill the request', () => {
    const lib = buildLibrary({ ...src, tags: [{ name: 'x'.repeat(500), count: 1 }] });
    expect(lib.tags[0].name).toHaveLength(60);
  });

  it('holds no folder, file name, path, place coordinate, face or picture', () => {
    const text = JSON.stringify(buildLibrary(src));
    expect(Object.keys(buildLibrary(src)).sort()).toEqual(['people', 'photos', 'picked', 'tags', 'trips']);
    expect(text).not.toMatch(/path|folder|\.jpg|lat|lon|gps|face|[A-Za-z]:[\/]/i);
  });
});
