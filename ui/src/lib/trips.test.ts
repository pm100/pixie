import { describe, expect, it } from 'vitest';
import type { Trip } from './api';
import { cleanTripPrefs, DEFAULT_TRIP_PREFS, tripDates, tripLabel, tripRows, tripSummary } from './trips';

const trip = (start: string, end: string, o: Partial<Trip> = {}): Trip => ({
  start, end, since: Date.parse(start) / 1000, until: Date.parse(end) / 1000 + 86_400, days: 1, photos: 1, videos: 0, name: null, ...o,
});

describe('tripDates', () => {
  it('says a one-day trip as a single date', () => expect(tripDates(trip('2023-06-05', '2023-06-05'))).toBe('5 June 2023'));
  it('folds a trip inside one month', () => expect(tripDates(trip('2023-06-05', '2023-06-12'))).toBe('5–12 June 2023'));
  it('names both months when a trip crosses a month', () => expect(tripDates(trip('2023-06-28', '2023-07-03'))).toBe('28 June – 3 July 2023'));
  it('names both years when a trip crosses the new year', () => expect(tripDates(trip('2022-12-28', '2023-01-03'))).toBe('28 December 2022 – 3 January 2023'));
});

describe('tripLabel', () => {
  it('uses the dates until the user names the trip', () => expect(tripLabel(trip('2023-06-05', '2023-06-12'))).toBe('5–12 June 2023'));
  it('uses the name once there is one', () => expect(tripLabel(trip('2023-06-05', '2023-06-12', { name: 'Rome' }))).toBe('Rome'));
  it('ignores a name that is only spaces', () => expect(tripLabel(trip('2023-06-05', '2023-06-05', { name: '   ' }))).toBe('5 June 2023'));
});

describe('tripSummary', () => {
  it('counts days, photos and videos', () => expect(tripSummary({ days: 3, photos: 1200, videos: 2 })).toBe('3 days · 1,200 photos · 2 videos'));
  it('uses the singular and leaves out what is zero', () => expect(tripSummary({ days: 1, photos: 1, videos: 0 })).toBe('1 day · 1 photo'));
  it('leaves out photos when there are only videos', () => expect(tripSummary({ days: 2, photos: 0, videos: 4 })).toBe('2 days · 4 videos'));
});

describe('tripRows', () => {
  const many = Array.from({ length: 12 }, (_, i) => trip(`2023-0${(i % 9) + 1}-0${(i % 9) + 1}`, `2023-0${(i % 9) + 1}-0${(i % 9) + 1}`, { since: 1000 + i }));
  it('shows everything when it fits', () => expect(tripRows(many.slice(0, 5), null, false, 8)).toEqual({ rows: many.slice(0, 5), hidden: 0 }));
  it('cuts the list and says how many are hidden', () => {
    const r = tripRows(many, null, false, 8);
    expect(r.rows).toHaveLength(8);
    expect(r.hidden).toBe(4);
  });
  it('keeps the chosen trip visible even when it is further down', () => {
    const r = tripRows(many, 1000 + 10, false, 8);
    expect(r.rows).toHaveLength(8);
    expect(r.rows.at(-1)!.since).toBe(1010);
  });
  it('shows all when expanded', () => expect(tripRows(many, null, true, 8)).toEqual({ rows: many, hidden: 0 }));
});

describe('cleanTripPrefs', () => {
  it('gives the defaults for nothing or rubbish', () => {
    expect(cleanTripPrefs(undefined)).toEqual(DEFAULT_TRIP_PREFS);
    expect(cleanTripPrefs('x')).toEqual(DEFAULT_TRIP_PREFS);
    expect(cleanTripPrefs(null)).toEqual(DEFAULT_TRIP_PREFS);
  });
  it('keeps good values', () => {
    const p = { home: { lat: 48.8, lon: 2.3 }, confirmed: true, radiusKm: 80, minDays: 2 };
    expect(cleanTripPrefs(p)).toEqual(p);
  });
  it('drops a home that is not a place on Earth, and with it the confirmation', () => {
    expect(cleanTripPrefs({ home: { lat: 91, lon: 0 }, confirmed: true })).toEqual(DEFAULT_TRIP_PREFS);
    expect(cleanTripPrefs({ home: { lat: NaN, lon: 0 }, confirmed: true })).toEqual(DEFAULT_TRIP_PREFS);
  });
  it('is not confirmed without a home', () => expect(cleanTripPrefs({ home: null, confirmed: true }).confirmed).toBe(false));
  it('clamps the radius and the minimum', () => {
    expect(cleanTripPrefs({ radiusKm: -5 }).radiusKm).toBe(50);
    expect(cleanTripPrefs({ radiusKm: 1e9 }).radiusKm).toBe(5000);
    expect(cleanTripPrefs({ minDays: -1 }).minDays).toBe(1);
    expect(cleanTripPrefs({ minDays: 2.9 }).minDays).toBe(2);
    expect(cleanTripPrefs({ minDays: 0 }).minDays).toBe(0);
  });
});
