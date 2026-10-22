import type { Route, Trip } from './api';

/** The user's choices for trips: where home is (once they have confirmed it), how far counts as away, how long a trip is. */
export interface TripPrefs {
  home: { lat: number; lon: number } | null;
  /** The user has agreed where home is; trips are not shown before that. */
  confirmed: boolean;
  /** Further than this from home is away. Kilometres, whatever the display unit. */
  radiusKm: number;
  /** A trip needs at least this many nights away (0 allows a day trip). */
  minNights: number;
  /** Photos within this distance of each other are one stop of a trip. Kilometres. */
  stopKm: number;
}

export const DEFAULT_TRIP_PREFS: TripPrefs = { home: null, confirmed: false, radiusKm: 50, minNights: 1, stopKm: 25 };
export const MAX_RADIUS_KM = 5000;
export const MAX_STOP_KM = 500;
export const MAX_NIGHTS = 365;

const num = (v: unknown): v is number => typeof v === 'number' && Number.isFinite(v);

/** Whatever was saved, made safe to use: anything odd falls back to the default for that field. */
export function cleanTripPrefs(raw: unknown): TripPrefs {
  const r = (typeof raw === 'object' && raw !== null ? raw : {}) as Record<string, unknown>;
  const h = r.home as Record<string, unknown> | null | undefined;
  const home = h && num(h.lat) && num(h.lon) && Math.abs(h.lat) <= 90 && Math.abs(h.lon) <= 180 ? { lat: h.lat, lon: h.lon } : null;
  const d = DEFAULT_TRIP_PREFS;
  return {
    home,
    confirmed: home !== null && r.confirmed === true,
    radiusKm: num(r.radiusKm) && r.radiusKm > 0 ? Math.min(MAX_RADIUS_KM, r.radiusKm) : d.radiusKm,
    minNights: num(r.minNights) && r.minNights >= 0 ? Math.min(MAX_NIGHTS, Math.floor(r.minNights)) : d.minNights,
    stopKm: num(r.stopKm) && r.stopKm > 0 ? Math.min(MAX_STOP_KM, r.stopKm) : d.stopKm,
  };
}

// --- distance units -------------------------------------------------------------------------------------------

const KM_PER_MILE = 1.609344;
/** Regions that measure road distance in miles. */
const MILE_REGIONS = new Set(['US', 'GB', 'LR', 'MM']);

/** True when people in this locale (for example `en-US`) expect miles rather than kilometres. */
export function usesMiles(locale: string | undefined): boolean {
  if (!locale) return false;
  let region: string | undefined;
  try {
    const l = new Intl.Locale(locale);
    region = (l.region ?? l.maximize().region)?.toUpperCase();
  } catch {
    region = /[-_]([A-Za-z]{2})\b/.exec(locale)?.[1]?.toUpperCase();
  }
  return region !== undefined && MILE_REGIONS.has(region);
}

export type DistanceUnit = 'km' | 'mi';
export const unitFor = (locale: string | undefined): DistanceUnit => (usesMiles(locale) ? 'mi' : 'km');

/** A distance in kilometres as the number to show in `unit`, to one decimal place. */
export function fromKm(km: number, unit: DistanceUnit): number {
  const v = unit === 'mi' ? km / KM_PER_MILE : km;
  return Math.round(v * 10) / 10;
}

/** What the user typed, in `unit`, as kilometres. */
export const toKm = (value: number, unit: DistanceUnit): number => (unit === 'mi' ? value * KM_PER_MILE : value);

// --- names ---------------------------------------------------------------------------------------------------

const MONTHS = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'];

const parts = (day: string) => {
  const [y, m, d] = day.split('-').map(Number);
  return { y, m, d };
};

/** The dates of a trip without the year, which the list shows as a heading: `Jun 5`, `Jun 5–12`, `Jun 28 – Jul 3`. */
export function tripDates(t: Pick<Trip, 'start' | 'end'>): string {
  const a = parts(t.start), b = parts(t.end);
  if (t.start === t.end) return `${MONTHS[a.m - 1]} ${a.d}`;
  if (a.y === b.y && a.m === b.m) return `${MONTHS[a.m - 1]} ${a.d}–${b.d}`;
  return `${MONTHS[a.m - 1]} ${a.d} – ${MONTHS[b.m - 1]} ${b.d}`;
}

/** The dates with the year, for places that do not sit under a year heading: `Jun 5–12, 2023`, `Dec 28, 2022 – Jan 3, 2023`. */
export function tripDatesWithYear(t: Pick<Trip, 'start' | 'end'>): string {
  const a = parts(t.start), b = parts(t.end);
  if (a.y !== b.y) return `${MONTHS[a.m - 1]} ${a.d}, ${a.y} – ${MONTHS[b.m - 1]} ${b.d}, ${b.y}`;
  return `${tripDates(t)}, ${a.y}`;
}

/**
 * What a trip's node says, as the first line and an optional second. Until the user names the trip that is its dates and
 * then where it went. A name replaces all of it: it is one line, or two when it has a ": " in it (the way the name is
 * written when edited, where ": " stands for the line break).
 */
export function tripLines(t: Pick<Trip, 'start' | 'end' | 'name'>, destination: string | undefined): [string, string] {
  const name = t.name?.trim();
  if (!name) return [tripDates(t), destination ?? ''];
  const cut = name.indexOf(': ');
  return cut > 0 ? [name.slice(0, cut), name.slice(cut + 2).trim()] : [name, ''];
}

/** The text of a trip's node on one line, which is what the rename box starts with: `Apr 24 – May 31: Italy, France`. */
export function tripEditText(t: Pick<Trip, 'start' | 'end' | 'name'>, destination: string | undefined): string {
  const name = t.name?.trim();
  if (name) return name;
  const [dates, where] = tripLines(t, destination);
  return where ? `${dates}: ${where}` : dates;
}

/** The name to store for what the user typed in the rename box: none when it is empty or just the automatic text. */
export function nameFromEdit(typed: string, t: Pick<Trip, 'start' | 'end'>, destination: string | undefined): string {
  const text = typed.trim();
  return text === tripEditText({ ...t, name: null }, destination) ? '' : text;
}

/** A trip as one phrase, for a place with no year heading above it (the filter chip, a folded heading): its name or its dates. */
export const tripTitle = (t: Pick<Trip, 'start' | 'end' | 'name'>): string => (t.name?.trim() ? t.name.trim() : tripDatesWithYear(t));

const plural = (n: number, one: string, many: string) => `${n.toLocaleString()} ${n === 1 ? one : many}`;

/** `3 nights · 120 photos · 2 videos` (`Day trip` for no night away), leaving out what is zero. */
export function tripSummary(t: Pick<Trip, 'nights' | 'photos' | 'videos'>): string {
  return [t.nights === 0 ? 'Day trip' : plural(t.nights, 'night', 'nights'), t.photos ? plural(t.photos, 'photo', 'photos') : '', t.videos ? plural(t.videos, 'video', 'videos') : '']
    .filter(Boolean)
    .join(' · ');
}

/** The year a trip is listed under: the year it started. */
export const tripYear = (t: Pick<Trip, 'start'>): number => parts(t.start).y;

/** The trips (newest first) grouped by the year they started in, newest year first. */
export function tripYears(trips: Trip[]): { year: number; trips: Trip[] }[] {
  const out: { year: number; trips: Trip[] }[] = [];
  for (const t of trips) {
    const y = tripYear(t);
    const last = out[out.length - 1];
    if (last && last.year === y) last.trips.push(t);
    else out.push({ year: y, trips: [t] });
  }
  return out;
}

// --- destinations --------------------------------------------------------------------------------------------

/** The town and the country of a place line such as `Florence, Tuscany, Italy` (the country is empty when there is only one part). */
export function townAndCountry(locality: string | null | undefined): { town: string; country: string } | null {
  const bits = (locality ?? '').split(',').map((s) => s.trim()).filter(Boolean);
  if (bits.length === 0) return null;
  return { town: bits[0], country: bits.length > 1 ? bits[bits.length - 1] : '' };
}

const MOST = 3;

/**
 * Where a trip went, in words. `places[i]` is the place line of `route.stops[i]` (null when it is not known).
 * One country: the towns, up to three with the most days, in the order they were visited (`Rome → Florence → Venice +2 more`).
 * Several countries: the countries, in the order first visited. Null when no stop has a known place.
 */
export function destinationOf(route: Route, places: (string | null)[]): string | null {
  const named = route.stops
    .map((s, i) => ({ i, s, p: townAndCountry(places[i]) }))
    .filter((x): x is { i: number; s: Route['stops'][number]; p: { town: string; country: string } } => x.p !== null);
  if (named.length === 0) return null;
  const unique = <T,>(xs: T[]) => [...new Set(xs)];
  const list = (xs: string[]) => (xs.length > MOST ? `${xs.slice(0, MOST).join(', ')} +${xs.length - MOST} more` : xs.join(', '));
  const countries = unique(named.map((x) => x.p.country).filter(Boolean));
  if (countries.length > 1) return list(countries);
  const towns = unique(named.map((x) => x.p.town));
  if (towns.length <= MOST) return towns.join(' → ');
  // The towns where the most days were spent, shown in the order the trip reached them.
  const byDays = new Map<string, number>();
  for (const x of named) byDays.set(x.p.town, (byDays.get(x.p.town) ?? 0) + x.s.days);
  const top = new Set([...towns].sort((a, b) => (byDays.get(b) ?? 0) - (byDays.get(a) ?? 0) || towns.indexOf(a) - towns.indexOf(b)).slice(0, MOST));
  return `${towns.filter((t) => top.has(t)).join(' → ')} +${towns.length - MOST} more`;
}
