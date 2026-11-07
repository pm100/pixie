import type { FolderInfo, KeepLayout, KeepProgress, KeepReport, KeepWhich, SourceSummary, ZipLook } from './api';
import { folderName, formatBytes, plural } from './format';

/** Kept photos are filed by year and day inside the folder, the same as Sync does. (The backend can also file by month, or not at all.) */
export const KEEP_LAYOUT: KeepLayout = 'year-date';

/** How a folder shows in the Into list: its name and where it is, or just where it is when the name says the same (a drive root). */
export function folderLabel(path: string): string {
  const name = folderName(path);
  return name === path ? path : `${name} (${path})`;
}

/** What is known of a source, one fact to a row: what was found, how much is new, and the picked ones. */
export function summaryRows(s: SourceSummary, picked = 0): { label: string; value: string }[] {
  if (s.photos + s.videos === 0) return [{ label: 'Found', value: 'No photos or videos' }];
  const parts = [s.photos > 0 ? plural(s.photos, 'photo') : null, s.videos > 0 ? plural(s.videos, 'video') : null].filter((p): p is string => p !== null);
  const rows = [
    { label: 'Found', value: `${parts.join(', ')} (${formatBytes(s.bytes)})` },
    { label: 'New', value: s.new.toLocaleString() },
  ];
  if (picked > 0) rows.push({ label: 'Picked', value: picked.toLocaleString() });
  return rows;
}

/** The rows while a source is still being extracted or read: what has been found so far, counting up, and the new count still to come. */
export function workingRows(extract: { photos: number; videos: number; bytes_done: number; bytes_total: number } | null, seen = 0): { label: string; value: string }[] {
  let found = 'Starting…';
  if (extract !== null) {
    const parts = [extract.photos > 0 ? plural(extract.photos, 'photo') : null, extract.videos > 0 ? plural(extract.videos, 'video') : null].filter((p): p is string => p !== null);
    if (parts.length > 0) found = `${parts.join(', ')} (${formatBytes(extract.bytes_done)} of ${formatBytes(extract.bytes_total)})`;
  } else if (seen > 0) {
    found = plural(seen, 'photo or video', 'photos or videos');
  }
  return [{ label: 'Found', value: found }, { label: 'New', value: '…' }];
}

/** Whether to show where a source is: not when its name already says it (a drive: "Photos (G:)" is at G:\). */
export function showPath(label: string, path: string): boolean {
  const bare = path.replace(/[\\/]+$/, '').toLowerCase();
  return bare !== '' && !label.toLowerCase().includes(bare);
}

/** What a Keep did, in a sentence or two. */
export function keepLine(r: KeepReport): string {
  const done = r.copied > 0 ? `Imported ${plural(r.copied, 'item')} (${formatBytes(r.bytes)}).` : null;
  const already = r.already > 0 ? `${r.already.toLocaleString()} ${r.already === 1 ? 'was' : 'were'} already in your folder.` : null;
  const failed = r.failed > 0 ? `${r.failed.toLocaleString()} could not be imported.` : null;
  if (r.stopped && done === null && already === null) return 'Stopped before anything was imported.';
  const lead = r.stopped ? 'Stopped.' : null;
  const text = [lead, done, already, failed].filter((p): p is string => p !== null).join(' ');
  return text === '' ? 'Nothing was imported.' : text;
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
