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

/** How a copy into the master folder went (the Rust `copier::Report`). */
export type CopyReport = { at: number; copied: number; already: number; failed: number; waiting: number; paused: string | null; problems: string[] };
export type CopyStatus = { approved: number; copied: number; running: boolean; last: CopyReport | null };
export type CopyProgress = { done: number; total: number };

/** What the master section says about the copying now: how far it has got, or what is waiting. */
export function copyLine(s: CopyStatus, p: CopyProgress | null): string {
  if (s.running) return p !== null && p.total > 0 ? `Copying ${p.done} of ${p.total}…` : 'Copying…';
  if (s.approved === 0) return 'Nothing is approved and waiting to be copied.';
  return `${plural(s.approved, 'item')} ${s.approved === 1 ? 'is' : 'are'} approved and waiting to be copied.`;
}

/** One line on the last copy: what it did, and why it stopped if it did. */
export function lastCopyLine(r: CopyReport): string {
  const parts = [
    r.copied > 0 ? `${r.copied} copied` : '',
    r.already > 0 ? `${r.already} already there` : '',
    r.failed > 0 ? `${r.failed} failed` : '',
    r.waiting > 0 ? `${r.waiting} waiting for download` : '',
  ].filter((x) => x !== '');
  return `Last copy: ${parts.length > 0 ? parts.join(', ') : 'nothing to copy'}.${r.paused !== null ? ` Stopped: ${r.paused}.` : ''}`;
}
