import type { Photo } from './api';
import { formatBytes } from './format';

export type DupStatus = { done: number; total: number; groups: number; extra_files: number; extra_bytes: number; hidden: number; paused: boolean;
  /** Held back while a folder scan or the loading of photos runs. */
  held?: boolean };
/** `keeper` is the copy Browse shows: the one the user chose, else the oldest. */
export type DupGroup = { hash: string; photos: Photo[]; keeper: number };

const n = (count: number) => count.toLocaleString();

/** The line under the heading: how far the checking has got, or what it found. */
export function dupLine(s: DupStatus): string {
  const checking = s.done < s.total;
  const found = s.groups === 0
    ? 'No duplicates found'
    : `${n(s.groups)} ${s.groups === 1 ? 'set' : 'sets'} of duplicates · ${n(s.extra_files)} extra ${s.extra_files === 1 ? 'file' : 'files'} · ${formatBytes(s.extra_bytes)} could be freed`;
  return checking ? `Checking for duplicates: ${n(s.done)} of ${n(s.total)} looked at. ${s.groups === 0 ? '' : found}`.trim() : found;
}

/** The copies that would go to the Recycle Bin if `groups` were all tidied: everything but each set's keeper. */
export const extrasOf = (groups: DupGroup[]): Photo[] => groups.flatMap((g) => g.photos.filter((p) => p.id !== g.keeper));

export const totalSize = (photos: Photo[]): number => photos.reduce((sum, p) => sum + p.size, 0);

/** How many sets of duplicates are drawn at first, and each time the list is scrolled near its end. A library can have thousands of sets
 *  (each with a picture and a row for every copy), and drawing them all at once froze the window for seconds, on opening the tab and again
 *  when leaving it. */
export const SETS_AT_A_TIME = 30;

/** How many sets to draw when `drawn` were drawn before and there are `total`: at least a screenful, never more than there are. */
export const setsShown = (drawn: number, total: number): number => Math.min(total, Math.max(drawn, SETS_AT_A_TIME));

/** The number to draw once the end of the list has been reached. */
export const moreSets = (drawn: number, total: number): number => Math.min(total, drawn + SETS_AT_A_TIME);

