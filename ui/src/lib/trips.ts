import type { Trip } from './api';
import { monthName } from './format';

/** The user's choices for trips: where home is (once they have confirmed it), how far counts as away, how long a trip is. */
export interface TripPrefs {
  home: { lat: number; lon: number } | null;
  /** The user has agreed where home is; trips are not shown before that. */
  confirmed: boolean;
  radiusKm: number;
  /** A trip needs at least this many days away (0 and 1 mean any day away). */
  minDays: number;
}

export const DEFAULT_TRIP_PREFS: TripPrefs = { home: null, confirmed: false, radiusKm: 50, minDays: 1 };
export const MAX_RADIUS_KM = 5000;
export const MAX_MIN_DAYS = 365;

const num = (v: unknown): v is number => typeof v === 'number' && Number.isFinite(v);

/** Whatever was saved, made safe to use: anything odd falls back to the default for that field. */
export function cleanTripPrefs(raw: unknown): TripPrefs {
  const r = (typeof raw === 'object' && raw !== null ? raw : {}) as Record<string, unknown>;
  const h = r.home as Record<string, unknown> | null | undefined;
  const home = h && num(h.lat) && num(h.lon) && Math.abs(h.lat) <= 90 && Math.abs(h.lon) <= 180 ? { lat: h.lat, lon: h.lon } : null;
  return {
    home,
    confirmed: home !== null && r.confirmed === true,
    radiusKm: num(r.radiusKm) && r.radiusKm > 0 ? Math.min(MAX_RADIUS_KM, r.radiusKm) : DEFAULT_TRIP_PREFS.radiusKm,
    minDays: num(r.minDays) && r.minDays >= 0 ? Math.min(MAX_MIN_DAYS, Math.floor(r.minDays)) : DEFAULT_TRIP_PREFS.minDays,
  };
}

const parts = (day: string) => {
  const [y, m, d] = day.split('-').map(Number);
  return { y, m, d };
};

/** The dates of a trip as people say them: `5 June 2023`, `5–12 June 2023`, `28 June – 3 July 2023`, `28 December 2022 – 3 January 2023`. */
export function tripDates(t: Pick<Trip, 'start' | 'end'>): string {
  const a = parts(t.start), b = parts(t.end);
  if (t.start === t.end) return `${a.d} ${monthName(a.m)} ${a.y}`;
  if (a.y === b.y && a.m === b.m) return `${a.d}–${b.d} ${monthName(a.m)} ${a.y}`;
  if (a.y === b.y) return `${a.d} ${monthName(a.m)} – ${b.d} ${monthName(b.m)} ${a.y}`;
  return `${a.d} ${monthName(a.m)} ${a.y} – ${b.d} ${monthName(b.m)} ${b.y}`;
}

/** What the list calls a trip: the user's name for it, or its dates. */
export const tripLabel = (t: Pick<Trip, 'start' | 'end' | 'name'>): string => (t.name?.trim() ? t.name.trim() : tripDates(t));

const plural = (n: number, one: string, many: string) => `${n.toLocaleString()} ${n === 1 ? one : many}`;

/** `3 days · 120 photos · 2 videos`, leaving out what is zero. */
export function tripSummary(t: Pick<Trip, 'days' | 'photos' | 'videos'>): string {
  return [plural(t.days, 'day', 'days'), t.photos ? plural(t.photos, 'photo', 'photos') : '', t.videos ? plural(t.videos, 'video', 'videos') : '']
    .filter(Boolean)
    .join(' · ');
}

/** A trip is chosen by where it starts in time; this is the same number the list reports as `since`. */
export const tripKey = (t: Pick<Trip, 'since'>): number => t.since;

/** The rows the Trips list shows: the first `limit` (or all when expanded), plus the chosen trip even if further down. */
export function tripRows(trips: Trip[], chosen: number | null, expanded: boolean, limit: number): { rows: Trip[]; hidden: number } {
  if (expanded || trips.length <= limit) return { rows: trips, hidden: 0 };
  let rows = trips.slice(0, limit);
  const pick = chosen === null ? undefined : trips.find((t) => t.since === chosen);
  if (pick && !rows.includes(pick)) rows = [...rows.slice(0, limit - 1), pick];
  return { rows, hidden: trips.length - limit };
}
