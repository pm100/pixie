import { describe, expect, it } from 'vitest';
import type { Route, Stop, Trip } from './api';
import {
  cleanTripPrefs, DEFAULT_TRIP_PREFS, destinationOf, fromKm, nameFromEdit, toKm, townAndCountry, tripDates, tripDatesWithYear, tripEditText, tripLines, tripSummary, tripTitle, tripYears, unitFor, usesMiles,
} from './trips';

const trip = (start: string, end: string, o: Partial<Trip> = {}): Trip => ({
  start, end, since: Date.parse(start) / 1000, until: Date.parse(end) / 1000 + 86_400, nights: 1, days: 1, photos: 1, videos: 0, name: null, ...o,
});

describe('tripDates', () => {
  it('says a one-day trip as a single date', () => expect(tripDates(trip('2023-06-05', '2023-06-05'))).toBe('Jun 5'));
  it('folds a trip inside one month', () => expect(tripDates(trip('2023-06-05', '2023-06-12'))).toBe('Jun 5–12'));
  it('names both months when a trip crosses a month', () => expect(tripDates(trip('2023-06-28', '2023-07-03'))).toBe('Jun 28 – Jul 3'));
  it('leaves the year to the heading even across the new year', () => expect(tripDates(trip('2022-12-28', '2023-01-03'))).toBe('Dec 28 – Jan 3'));
});

describe('tripDatesWithYear', () => {
  it('adds the year', () => expect(tripDatesWithYear(trip('2023-06-05', '2023-06-12'))).toBe('Jun 5–12, 2023'));
  it('names both years across the new year', () => expect(tripDatesWithYear(trip('2022-12-28', '2023-01-03'))).toBe('Dec 28, 2022 – Jan 3, 2023'));
});

describe('tripLines', () => {
  const t = trip('2023-04-24', '2023-05-31');
  it('is the dates and then where the trip went', () => expect(tripLines(t, 'Italy, France')).toEqual(['Apr 24 – May 31', 'Italy, France']));
  it('is just the dates until the destination is known', () => expect(tripLines(t, undefined)).toEqual(['Apr 24 – May 31', '']));
  it('is the name alone once the trip is named, and no destination is added', () => expect(tripLines({ ...t, name: 'Europe 2023' }, 'Italy, France')).toEqual(['Europe 2023', '']));
  it('breaks a name at ": " into two lines, as it was typed', () =>
    expect(tripLines({ ...t, name: 'Apr 24 – May 31: Italy and friends' }, 'Italy, France')).toEqual(['Apr 24 – May 31', 'Italy and friends']));
  it('does not break on a colon with no space after it', () => expect(tripLines({ ...t, name: 'Day 1:2' }, undefined)).toEqual(['Day 1:2', '']));
  it('ignores a name that is only spaces', () => expect(tripLines({ ...t, name: '   ' }, 'Italy')).toEqual(['Apr 24 – May 31', 'Italy']));
});

describe('tripEditText', () => {
  const t = trip('2023-04-24', '2023-05-31');
  it('joins the two lines with a colon', () => expect(tripEditText(t, 'Italy, France, Netherlands, +1 more')).toBe('Apr 24 – May 31: Italy, France, Netherlands, +1 more'));
  it('is just the dates when nothing is known about where it went', () => expect(tripEditText(t, undefined)).toBe('Apr 24 – May 31'));
  it('is the name when there is one', () => expect(tripEditText({ ...t, name: 'Europe 2023' }, 'Italy')).toBe('Europe 2023'));
});

describe('nameFromEdit', () => {
  const t = { start: '2023-04-24', end: '2023-05-31' };
  it('stores what the user typed, trimmed', () => expect(nameFromEdit('  Europe 2023 ', t, 'Italy')).toBe('Europe 2023'));
  it('stores nothing when the text was left as it was, so the node stays automatic', () => {
    expect(nameFromEdit('Apr 24 – May 31: Italy', t, 'Italy')).toBe('');
    expect(nameFromEdit('Apr 24 – May 31', t, undefined)).toBe('');
  });
  it('stores nothing for an empty box, which removes a name', () => expect(nameFromEdit('   ', t, 'Italy')).toBe(''));
  it('keeps a partly changed automatic text as the name', () => expect(nameFromEdit('Apr 24 – May 31: Italy and friends', t, 'Italy')).toBe('Apr 24 – May 31: Italy and friends'));
});

describe('tripTitle', () => {
  it('is the dates with the year until the trip is named', () => expect(tripTitle(trip('2023-06-05', '2023-06-12'))).toBe('Jun 5–12, 2023'));
  it('is the name once there is one', () => expect(tripTitle(trip('2023-06-05', '2023-06-12', { name: 'Rome' }))).toBe('Rome'));
});

describe('tripSummary', () => {
  it('counts nights, photos and videos', () => expect(tripSummary({ nights: 3, photos: 1200, videos: 2 })).toBe('3 nights · 1,200 photos · 2 videos'));
  it('uses the singular and leaves out what is zero', () => expect(tripSummary({ nights: 1, photos: 1, videos: 0 })).toBe('1 night · 1 photo'));
  it('calls a trip with no night away a day trip', () => expect(tripSummary({ nights: 0, photos: 0, videos: 4 })).toBe('Day trip · 4 videos'));
});

describe('tripYears', () => {
  it('groups newest-first trips by the year they started, keeping the order', () => {
    const a = trip('2024-03-01', '2024-03-05'), b = trip('2023-12-28', '2024-01-03'), c = trip('2023-06-05', '2023-06-12'), d = trip('2022-01-01', '2022-01-04');
    expect(tripYears([a, b, c, d]).map((g) => [g.year, g.trips.length])).toEqual([[2024, 1], [2023, 2], [2022, 1]]);
    expect(tripYears([a, b, c, d])[1].trips).toEqual([b, c]);
  });
  it('is empty for no trips', () => expect(tripYears([])).toEqual([]));
});

describe('units', () => {
  it('uses miles in the US, the UK, Liberia and Myanmar, and kilometres elsewhere', () => {
    for (const l of ['en-US', 'en-GB', 'en_GB', 'fr-LR', 'my-MM']) expect(usesMiles(l), l).toBe(true);
    for (const l of ['en-CA', 'fr-FR', 'de-DE', 'ja-JP', 'en-AU']) expect(usesMiles(l), l).toBe(false);
  });
  it('guesses the likely region of a bare language', () => {
    expect(usesMiles('en')).toBe(true); // "en" is taken to be en-US
    expect(usesMiles('fr')).toBe(false);
  });
  it('knows nothing without a locale', () => expect(unitFor(undefined)).toBe('km'));
  it('converts for display to one decimal and back', () => {
    expect(fromKm(25, 'km')).toBe(25);
    expect(fromKm(25, 'mi')).toBe(15.5);
    expect(fromKm(50, 'mi')).toBe(31.1);
    expect(toKm(10, 'mi')).toBeCloseTo(16.09344);
    expect(toKm(10, 'km')).toBe(10);
  });
});

const stop = (days: number, o: Partial<Stop> = {}): Stop => ({ lat: 0, lon: 0, first: '2023-06-01', last: '2023-06-02', days, items: days, ...o });
const route = (days: number[]): Route => ({ stops: days.map((d) => stop(d)), path: days.map((_, i) => i) });

describe('townAndCountry', () => {
  it('takes the first and last parts', () => expect(townAndCountry('Florence, Tuscany, Italy')).toEqual({ town: 'Florence', country: 'Italy' }));
  it('has no country for a single part', () => expect(townAndCountry('Florence')).toEqual({ town: 'Florence', country: '' }));
  it('is nothing for nothing', () => {
    expect(townAndCountry(null)).toBeNull();
    expect(townAndCountry('')).toBeNull();
  });
});

describe('destinationOf', () => {
  it('names the town of a single stop', () => expect(destinationOf(route([4]), ['Rome, Lazio, Italy'])).toBe('Rome'));
  it('lists up to three towns in the order visited', () =>
    expect(destinationOf(route([2, 1, 3]), ['Rome, Lazio, Italy', 'Florence, Tuscany, Italy', 'Venice, Veneto, Italy'])).toBe('Rome → Florence → Venice'));
  it('keeps the three towns with the most days, in visit order, and counts the rest', () =>
    expect(destinationOf(route([1, 5, 2, 4, 1]), ['A, r, Italy', 'B, r, Italy', 'C, r, Italy', 'D, r, Italy', 'E, r, Italy'])).toBe('B → C → D +2 more'));
  it('says each town once even when two stops are in it', () =>
    expect(destinationOf(route([1, 1]), ['Rome, Lazio, Italy', 'Rome, Lazio, Italy'])).toBe('Rome'));
  it('names the countries when there are several', () =>
    expect(destinationOf(route([3, 2]), ['Rome, Lazio, Italy', 'Nice, Alpes-Maritimes, France'])).toBe('Italy, France'));
  it('shortens a long list of countries', () =>
    expect(destinationOf(route([1, 1, 1, 1, 1]), ['a, x, A', 'b, x, B', 'c, x, C', 'd, x, D', 'e, x, E'])).toBe('A, B, C +2 more'));
  it('uses the stops whose place is known and none when none is', () => {
    expect(destinationOf(route([2, 2]), [null, 'Rome, Lazio, Italy'])).toBe('Rome');
    expect(destinationOf(route([2, 2]), [null, null])).toBeNull();
    expect(destinationOf({ stops: [], path: [] }, [])).toBeNull();
  });
});

describe('cleanTripPrefs', () => {
  it('gives the defaults for nothing or rubbish', () => {
    expect(cleanTripPrefs(undefined)).toEqual(DEFAULT_TRIP_PREFS);
    expect(cleanTripPrefs('x')).toEqual(DEFAULT_TRIP_PREFS);
    expect(cleanTripPrefs(null)).toEqual(DEFAULT_TRIP_PREFS);
  });
  it('defaults to one night and 25 km stops', () => expect(DEFAULT_TRIP_PREFS).toMatchObject({ minNights: 1, stopKm: 25, radiusKm: 50 }));
  it('keeps good values', () => {
    const p = { home: { lat: 48.8, lon: 2.3 }, confirmed: true, radiusKm: 80, minNights: 2, stopKm: 10 };
    expect(cleanTripPrefs(p)).toEqual(p);
  });
  it('drops a home that is not a place on Earth, and with it the confirmation', () => {
    expect(cleanTripPrefs({ home: { lat: 91, lon: 0 }, confirmed: true })).toEqual(DEFAULT_TRIP_PREFS);
    expect(cleanTripPrefs({ home: { lat: NaN, lon: 0 }, confirmed: true })).toEqual(DEFAULT_TRIP_PREFS);
  });
  it('is not confirmed without a home', () => expect(cleanTripPrefs({ home: null, confirmed: true }).confirmed).toBe(false));
  it('clamps the numbers', () => {
    expect(cleanTripPrefs({ radiusKm: -5 }).radiusKm).toBe(50);
    expect(cleanTripPrefs({ radiusKm: 1e9 }).radiusKm).toBe(5000);
    expect(cleanTripPrefs({ stopKm: 0 }).stopKm).toBe(25);
    expect(cleanTripPrefs({ stopKm: 1e9 }).stopKm).toBe(500);
    expect(cleanTripPrefs({ minNights: -1 }).minNights).toBe(1);
    expect(cleanTripPrefs({ minNights: 2.9 }).minNights).toBe(2);
    expect(cleanTripPrefs({ minNights: 0 }).minNights).toBe(0);
  });
});
