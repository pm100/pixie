import type { Trip, UserTagCount } from './api';

/**
 * What Ask Pixie may be told about the library when the person has allowed it (Settings, Ask Pixie): counts, the names of their
 * tags and of the people they have named, and their trips. It helps the model get names and dates right ("Rose" the person, not
 * the flower). It never holds pictures, thumbnails, faces, file or folder names, paths, coordinates or where home is.
 */
export interface Library {
  photos: number;
  picked: number;
  tags: { name: string; count: number }[];
  people: { name: string; count: number }[];
  trips: { name: string | null; from: string; to: string; nights: number; photos: number; videos: number; where: string | null }[];
}

/** Most that are listed, so the request stays small: the biggest tags and people, the newest trips. */
export const MAX_TAGS = 150;
export const MAX_PEOPLE = 100;
export const MAX_TRIPS = 40;
const MAX_NAME = 60;

const clean = (s: string | null | undefined): string => (s ?? '').trim().slice(0, MAX_NAME);

export interface LibrarySource {
  total: number;
  picked: number;
  tags: UserTagCount[];
  people: { id: number; name: string | null; count: number }[];
  trips: Trip[];
  /** Where each trip went, by the trip's `since`, for the trips whose places are worked out. */
  destinations: Record<number, string>;
}

export function buildLibrary(src: LibrarySource): Library {
  const byCount = <T extends { count: number }>(list: T[]) => [...list].sort((a, b) => b.count - a.count);
  return {
    photos: src.total,
    picked: src.picked,
    tags: byCount(src.tags.map((t) => ({ name: clean(t.name), count: t.count })).filter((t) => t.name !== '')).slice(0, MAX_TAGS),
    people: byCount(src.people.map((p) => ({ name: clean(p.name), count: p.count })).filter((p) => p.name !== '')).slice(0, MAX_PEOPLE),
    trips: [...src.trips]
      .sort((a, b) => b.start.localeCompare(a.start))
      .slice(0, MAX_TRIPS)
      .map((t) => ({ name: clean(t.name) || null, from: t.start, to: t.end, nights: t.nights, photos: t.photos, videos: t.videos, where: clean(src.destinations[t.since]) || null })),
  };
}
