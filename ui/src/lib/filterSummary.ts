import type { MediaKind } from './api';

/** How each filter in force reads, or null when it is not on. */
export type ActiveFilters = {
  tag: string | null; person: string | null; camera?: string | null;
  /** Picked or one of your own tags, as the library names it: `picked` or `tag:<name>`. */
  utag?: string | null;
  /** The kind of missing data chosen in the tree (`location`). */
  missing?: string | null;
  folder: string | null; trip: string | null;
  /** A chosen date ("September 2026") or Last 30 days; the two replace each other, so there is at most one. */
  date: string | null;
  kind: MediaKind | null;
  /** The grid is limited to the part of the map in view. */
  area: boolean;
};

/** The filters in force, in the order of the pills in the toolbar, as the pills read. */
export function filterNames(f: ActiveFilters): string[] {
  return [
    f.tag !== null ? `Auto tagged “${f.tag}”` : null,
    f.utag == null ? null : f.utag === 'picked' ? 'Picked' : `Tagged “${f.utag.replace(/^tag:/, '')}”`,
    f.person !== null ? `Person “${f.person}”` : null,
    f.camera != null ? `Camera “${f.camera}”` : null,
    f.missing === 'location' ? 'No location' : null,
    f.kind !== null ? (f.kind === 'photo' ? 'Photos only' : 'Videos only') : null,
    f.area ? 'In the map area' : null,
    f.folder !== null ? `Folder “${f.folder}”` : null,
    f.trip !== null ? `Trip “${f.trip}”` : null,
    f.date,
  ].filter((n): n is string => n !== null);
}

/** "Clear all" is offered from two filters up: with one, its own ✕ already clears it, and a second control is noise. */
export const offerClearAll = (names: string[]): boolean => names.length >= 2;

/** What "Clear all" will clear, for its tooltip. */
export const clearAllTitle = (names: string[]): string => `Clear ${names.join(', ')}`;
