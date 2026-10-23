import type { Trip } from './api';

/** The counts beside the entries of the left-hand tree under the filters in force (mirrors the Rust `FacetCounts`). Each
 *  section is counted with every filter except its own, so choosing a folder narrows People, Auto tags and Trips but not
 *  the Folders list. */
export type FacetCounts = {
  photos: number;
  videos: number;
  folders: [number, number][];
  tags: [string, number][];
  tag_groups: number[];
  tag_group_names: string[];
  people: [number, number][];
  trips: number[];
};

/** The same, ready to look up by what the tree shows. An entry that is not in a map matches nothing (0). */
export type Facets = {
  photos: number;
  videos: number;
  folders: Map<number, number>;
  tags: Map<string, number>;
  categories: Map<string, number>;
  people: Map<number, number>;
  /** By the trip's `since`. */
  trips: Map<number, number>;
};

/** `trips` are the trips whose windows were asked for, in that order. */
export function indexFacets(f: FacetCounts, trips: Pick<Trip, 'since'>[]): Facets {
  return {
    photos: f.photos,
    videos: f.videos,
    folders: new Map(f.folders),
    tags: new Map(f.tags),
    categories: new Map(f.tag_group_names.map((name, i) => [name, f.tag_groups[i] ?? 0])),
    people: new Map(f.people),
    trips: new Map(trips.map((t, i) => [t.since, f.trips[i] ?? 0])),
  };
}

/** The count to show for an entry: its filtered count when the counts are known (0 when it is not among them), else the
 *  count the entry came with. */
export const countOf = <K>(counts: Map<K, number> | undefined, key: K, base: number): number => (counts ? (counts.get(key) ?? 0) : base);

/** How many of these trips to say a year holds: those with something in them under the filters, or all of them when no
 *  filtered counts are known. */
export const tripsHeld = (trips: Pick<Trip, 'since'>[], facets: Facets | null): number =>
  facets ? trips.filter((t) => (facets.trips.get(t.since) ?? 0) > 0).length : trips.length;
