import { plural } from './format';

/** Email size limits, from the sender's side: what a set of files counts as once sent, and what to say about it. */

export const MB = 1024 * 1024;
export const DEFAULT_LIMIT_MB = 20;
/** The limits offered in Settings: a company mailbox, a free mail service, a generous one. */
export const LIMIT_CHOICES_MB = [10, 15, 20, 25, 35];

/** The limit from a saved preference, or the default for anything that is not a sensible number. */
export function parseLimit(raw: unknown): number {
  return typeof raw === 'number' && Number.isFinite(raw) && raw >= 1 && raw <= 500 ? Math.floor(raw) : DEFAULT_LIMIT_MB;
}

/** What files count as once sent: attachments grow by about a third (they travel as text). Mirrors the Rust `sent_size`. */
export const sentBytes = (bytes: number): number => bytes + Math.floor(bytes / 3) + Math.floor(bytes / 50);

/** Does this much (the files' own sizes added up) go over the limit once sent? */
export const tooBig = (totalBytes: number, limitMb: number): boolean => sentBytes(totalBytes) > limitMb * MB;

/** "4.2 MB", "48 MB". */
export const mbText = (bytes: number): string => `${bytes < 10 * MB ? (bytes / MB).toFixed(1) : Math.round(bytes / MB)} MB`;

/** "12 photos", "3 videos", "12 photos and 3 videos". */
export function describeItems(photos: number, videos: number): string {
  const parts = [photos > 0 ? plural(photos, 'photo') : '', videos > 0 ? plural(videos, 'video') : ''].filter((p) => p !== '');
  return parts.join(' and ') || 'Nothing';
}
