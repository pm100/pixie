import type { Photo } from './api';
import type { AnalysisStatus } from './analysis';
import { formatBytes } from './format';

export type DupStatus = { done: number; total: number; groups: number; extra_files: number; extra_bytes: number; hidden: number };
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

/** The duplicate check as one more background pass, so it is counted and shown like the others. */
export const dupPass = (s: DupStatus | null): AnalysisStatus | null =>
  s === null ? null : { models: 'ready', model_done: 0, model_total: 0, done: s.done, total: s.total, paused: false, error: null };

/** The copies that would go to the Recycle Bin if `groups` were all tidied: everything but each set's keeper. */
export const extrasOf = (groups: DupGroup[]): Photo[] => groups.flatMap((g) => g.photos.filter((p) => p.id !== g.keeper));

export const totalSize = (photos: Photo[]): number => photos.reduce((sum, p) => sum + p.size, 0);
