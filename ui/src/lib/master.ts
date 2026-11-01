import { plural } from './format';

/** Where an item stands with the master folder, as the tree names it: held for review, or kept private. */
export type ShareChoice = 'new' | 'private';
/** What `photoShare` answers for a private photo (the Rust `SHARE_PRIVATE`). */
export const SHARE_PRIVATE = 3;
export type ShareCounts = { held: number; private: number };

/** A row of the Sharing section of the Browse tree. */
export type SharingRow = { id: ShareChoice; label: string; count: number; active: boolean; title: string };

/** The rows of the Sharing section. With no master folder there are none: none of the master feature shows until one is chosen. */
export function sharingRows(master: string | null, counts: ShareCounts, chosen: ShareChoice | null): SharingRow[] {
  if (master === null) return [];
  const rows: SharingRow[] = [{
    id: 'new', label: 'New, not yet shared', count: counts.held, active: chosen === 'new',
    title: chosen === 'new' ? 'Stop showing only the new items.' : 'Show only the items that arrived since the master folder was set up and have not been looked at.',
  }];
  if (counts.private > 0 || chosen === 'private') {
    rows.push({
      id: 'private', label: 'Private', count: counts.private, active: chosen === 'private',
      title: chosen === 'private' ? 'Stop showing only the private items.' : 'Show only the items kept out of the master folder.',
    });
  }
  return rows;
}

/** The line above the grid while the New items are listed. */
export const reviewLine = (held: number): string =>
  held === 0 ? 'Nothing is waiting to be shared.' : `${plural(held, 'item')} ${held === 1 ? 'is' : 'are'} new and not yet shared.`;
