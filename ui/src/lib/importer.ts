import type { FolderInfo, KeepLayout, KeepProgress, KeepReport, KeepWhich, SourceSummary, ZipLook } from './api';
import { formatBytes, plural } from './format';

/** How kept photos are filed inside the folder they go to. */
export const LAYOUTS: { id: KeepLayout; label: string; title: string }[] = [
  { id: 'year-date', label: 'Year and date (2024/2024-03-05)', title: 'File each photo in a folder for its year and a folder inside that for its day, the same as Sync does' },
  { id: 'year-month', label: 'Year and month (2024/2024-03)', title: 'File each photo in a folder for its year and a folder inside that for its month' },
  { id: 'flat', label: 'All in the folder', title: 'Put every photo straight into the chosen folder, with no folders inside it' },
];

/** The line under a source once it is read: what it holds, and how much of it is new. */
export function summaryLine(s: SourceSummary): string {
  if (s.photos + s.videos === 0) return 'No photos or videos found.';
  const parts = [s.photos > 0 ? plural(s.photos, 'photo') : null, s.videos > 0 ? plural(s.videos, 'video') : null].filter((p): p is string => p !== null);
  const held = `${parts.join(' and ')}, ${formatBytes(s.bytes)}.`;
  if (s.new === 0) return `${held} Nothing is new: all of it was kept before or is already in your library.`;
  return `${held} ${s.new.toLocaleString()} ${s.new === 1 ? 'is' : 'are'} new.`;
}

/** The label of a Keep button: what it will keep (the count is left out while it is not known, or none). */
export function keepWords(which: KeepWhich, count: number): string {
  if (which === 'picked') return count > 0 ? `Keep ${plural(count, 'picked item')}` : 'Keep picked items';
  return `Keep ${plural(count, 'new item')}`;
}

/** What a Keep did, in a sentence or two. */
export function keepLine(r: KeepReport): string {
  const done = r.copied > 0 ? `Kept ${plural(r.copied, 'item')} (${formatBytes(r.bytes)}).` : null;
  const already = r.already > 0 ? `${r.already.toLocaleString()} ${r.already === 1 ? 'was' : 'were'} already in your folder.` : null;
  const failed = r.failed > 0 ? `${r.failed.toLocaleString()} could not be kept.` : null;
  if (r.stopped && done === null && already === null) return 'Stopped before anything was kept.';
  const lead = r.stopped ? 'Stopped.' : null;
  const text = [lead, done, already, failed].filter((p): p is string => p !== null).join(' ');
  return text === '' ? 'Nothing was kept.' : text;
}

/** How far a Keep has got, 0 to 100, by the bytes. */
export function keepPercent(p: KeepProgress | null): number {
  if (p === null || p.bytes_total <= 0) return 0;
  return Math.min(100, Math.round((p.bytes_done / p.bytes_total) * 100));
}

/** The folder to keep into: the one last used if it is still there, else the sync folder, else the first. */
export function defaultDestination(folders: FolderInfo[], master: string | null, lastUsed: number | null): number | null {
  if (lastUsed !== null && folders.some((f) => f.id === lastUsed)) return lastUsed;
  return folders.find((f) => f.path === master)?.id ?? folders[0]?.id ?? null;
}

/** What a zip holds, in a sentence: the photos and videos that will be extracted, and the other files that will be left out. */
export function zipLine(z: ZipLook): string {
  if (z.photos + z.videos === 0) return 'There are no photos or videos in this zip.';
  const parts = [z.photos > 0 ? plural(z.photos, 'photo') : null, z.videos > 0 ? plural(z.videos, 'video') : null].filter((p): p is string => p !== null);
  const held = `${parts.join(' and ')}, ${formatBytes(z.bytes)}.`;
  return z.other > 0 ? `${held} ${plural(z.other, 'other file')} will be left out.` : held;
}

/** What extracting the zip needs and what is free where the Inbox is (nothing when there is nothing to extract). */
export function spaceLine(z: ZipLook): string {
  if (z.photos + z.videos === 0) return '';
  return z.fits ? `Needs ${formatBytes(z.bytes)}. ${formatBytes(z.free)} is free where the Inbox is.` : `Needs ${formatBytes(z.bytes)} but only ${formatBytes(z.free)} is free where the Inbox is.`;
}

/** Each kind of entry that will be skipped, as a sentence. */
export function zipProblems(z: ZipLook): string[] {
  const entries = (n: number) => plural(n, 'entry', 'entries');
  const out: string[] = [];
  if (z.unsafe_names > 0) out.push(`${entries(z.unsafe_names)} ${z.unsafe_names === 1 ? 'has an unsafe name' : 'have unsafe names'} and will be skipped.`);
  if (z.encrypted > 0) out.push(`${entries(z.encrypted)} ${z.encrypted === 1 ? 'is' : 'are'} encrypted and will be skipped.`);
  if (z.too_big > 0) out.push(`${entries(z.too_big)} ${z.too_big === 1 ? 'expands' : 'expand'} far more than ${z.too_big === 1 ? 'it' : 'they'} should and will be skipped.`);
  if (z.archives > 0) out.push(`${plural(z.archives, 'zip')} inside this zip will not be opened.`);
  return out;
}

/** How far an extraction has got. */
export function extractLine(done: number, total: number): string {
  return total > 0 ? `Extracting ${done.toLocaleString()} of ${total.toLocaleString()}…` : 'Extracting…';
}
