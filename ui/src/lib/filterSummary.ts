import type { MediaKind } from './api';

/** How each filter in force reads, or null when it is not on. */
export type ActiveFilters = {
  tag: string | null; person: string | null; folder: string | null; trip: string | null;
  /** A chosen date ("September 2026") or Last 30 days; the two replace each other, so there is at most one. */
  date: string | null;
  kind: MediaKind | null;
};

/** The filters in force, in the order of the pills in the toolbar, as the pills read. */
export function filterNames(f: ActiveFilters): string[] {
  return [
    f.tag !== null ? `Tagged “${f.tag}”` : null,
    f.person !== null ? `Person “${f.person}”` : null,
    f.kind !== null ? (f.kind === 'photo' ? 'Photos only' : 'Videos only') : null,
    f.folder !== null ? `Folder “${f.folder}”` : null,
    f.trip !== null ? `Trip “${f.trip}”` : null,
    f.date,
  ].filter((n): n is string => n !== null);
}

/** "Clear all" is offered from two filters up: with one, its own ✕ already clears it, and a second control is noise. */
export const offerClearAll = (names: string[]): boolean => names.length >= 2;

/** What "Clear all" will clear, for its tooltip. */
export const clearAllTitle = (names: string[]): string => `Clear ${names.join(', ')}`;
