/** How far along a folder scan is, 0 to 1, from the files found so far and how many items the folder had before; null if that is not known. */
export function scanFraction(seen: number, known: number): number | null {
  if (known <= 0) return null;
  return Math.min(1, Math.max(0, seen / known));
}

/** What a running scan has reported: files found, files read so far, and how many files will be read in all (0 while still walking). */
export type ScanState = { seen: number; indexed: number; toIndex: number };

/** The share of the files to read that are done, 0 to 1; null while the folder is still being walked. */
export const readFraction = (s: ScanState): number | null => (s.toIndex > 0 ? Math.min(1, s.indexed / s.toIndex) : null);

/** The status words: the count found while walking, then "Reading X of Y" while the details of each file are read. */
export const scanText = (s: ScanState): string =>
  s.toIndex > 0
    ? `Reading ${s.indexed.toLocaleString()} of ${s.toIndex.toLocaleString()}`
    : s.seen === 0
      ? 'Opening the folder…'
      : `Scanning… ${s.seen.toLocaleString()} found`;
