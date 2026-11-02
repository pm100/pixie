import { plural } from './format';

/** Where an item stands with the sync folder, as the tree names it: not yet synced, or kept private. */
export type ShareChoice = 'new' | 'private';
/** What `photoShare` answers for a private photo (the Rust `SHARE_PRIVATE`). */
export const SHARE_PRIVATE = 3;
export type ShareCounts = { unsynced: number; private: number };

/** A row of the Sync section of the Browse tree. */
export type SharingRow = { id: ShareChoice; label: string; count: number; active: boolean; title: string };

/** The rows of the Sync section. With no sync folder there are none: none of the feature shows until one is chosen. */
export function sharingRows(master: string | null, counts: ShareCounts, chosen: ShareChoice | null): SharingRow[] {
  if (master === null) return [];
  const rows: SharingRow[] = [{
    id: 'new', label: 'New, not yet synced', count: counts.unsynced, active: chosen === 'new',
    title: chosen === 'new' ? 'Stop showing only the new items.' : 'Show only the items that are in a folder synced from and not yet in the sync folder.',
  }];
  if (counts.private > 0 || chosen === 'private') {
    rows.push({
      id: 'private', label: 'Private', count: counts.private, active: chosen === 'private',
      title: chosen === 'private' ? 'Stop showing only the private items.' : 'Show only the items kept out of the sync folder.',
    });
  }
  return rows;
}

/** How a copy into the sync folder went (the Rust `copier::Report`). */
export type CopyReport = { at: number; copied: number; already: number; failed: number; paused: string | null; problems: string[]; seconds: number; bytes: number };
export type CopyStatus = { pending: number; copied: number; running: boolean; last: CopyReport | null };
export type CopyProgress = { done: number; total: number; bytes_done: number; bytes_total: number; phase: 'copying' | 'checking' };

/** What the sync section says about the copying now: that it is going (the Processing button has how far), or what is waiting. */
export function copyLine(s: CopyStatus): string {
  if (s.running) return 'Copying. The Processing button at the top right shows how far it has got.';
  if (s.pending === 0) return 'Nothing is waiting to be copied.';
  return `${plural(s.pending, 'item')} ${s.pending === 1 ? 'is' : 'are'} waiting to be copied.`;
}

/** One line on the last copy: what it did, and why it stopped if it did. */
export function lastCopyLine(r: CopyReport): string {
  const parts = [
    r.copied > 0 ? `${r.copied} copied` : '',
    r.already > 0 ? `${r.already} already there` : '',
    r.failed > 0 ? `${r.failed} failed` : '',
  ].filter((x) => x !== '');
  return `Last copy: ${parts.length > 0 ? parts.join(', ') : 'nothing to copy'}${took(r)}.${r.paused !== null ? ` Stopped: ${r.paused}.` : ''}`;
}

/** " (360 MB in 2 min 5 s, 2.9 MB/s)": how much was copied and how fast, so a slow drive shows. Nothing when the run was too short to say. */
function took(r: CopyReport): string {
  if (r.copied === 0 || r.seconds < 1) return '';
  const mb = r.bytes / 1_000_000;
  const time = r.seconds < 60 ? `${r.seconds} s` : `${Math.floor(r.seconds / 60)} min ${r.seconds % 60} s`;
  const size = mb >= 1000 ? `${(mb / 1000).toFixed(1)} GB` : `${Math.round(mb)} MB`;
  return ` (${size} in ${time}, ${(mb / r.seconds).toFixed(1)} MB/s)`;
}

/** The start-up reminder: what it says about the new items waiting to be synced. */
export function reminderLine(held: number, folder: string): string {
  return `${plural(held, 'item')} ${held === 1 ? 'is' : 'are'} new and not yet synced to ${folder}. Before syncing, look through ${held === 1 ? 'it' : 'them'} and mark any you do not want copied as private.`;
}

/** Whether to remind at start: a sync folder is set, something is waiting, and the person has not turned it off. */
export const shouldRemind = (master: string | null, held: number, remind: boolean): boolean =>
  master !== null && held > 0 && remind;
