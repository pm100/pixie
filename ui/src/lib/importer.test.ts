import { describe, expect, it } from 'vitest';
import type { FolderInfo, KeepReport, SourceSummary, ZipLook } from './api';
import { KEEP_LAYOUT, defaultDestination, folderLabel, extractLine, keepLine, keepPercent, showPath, spaceLine, summaryRows, workingRows, zipLine, zipProblems } from './importer';

const MB = 1024 ** 2;
const GB = 1024 ** 3;
const summary = (over: Partial<SourceSummary> = {}): SourceSummary => ({ photos: 0, videos: 0, bytes: 0, new: 0, kept_before: 0, looks_same: 0, first: null, last: null, ...over });
const report = (over: Partial<KeepReport> = {}): KeepReport => ({ batch: 1, copied: 0, already: 0, failed: 0, problems: [], bytes: 0, stopped: false, ...over });
const folder = (id: number, path: string): FolderInfo => ({ id, path, count: 0, feeds_master: false });

describe('summaryRows', () => {
  it('says there is nothing when there is nothing', () => {
    expect(summaryRows(summary())).toEqual([{ label: 'Found', value: 'No photos or videos' }]);
  });

  it('gives what was found, then how many are new, one to a row', () => {
    expect(summaryRows(summary({ photos: 412, videos: 18, bytes: 6.1 * GB, new: 380 }))).toEqual([
      { label: 'Found', value: '412 photos, 18 videos (6.1 GB)' },
      { label: 'New', value: '380' },
    ]);
  });

  it('only names what is there, and says one when it is one', () => {
    expect(summaryRows(summary({ photos: 1, bytes: 2 * MB, new: 1 }))[0].value).toBe('1 photo (2.0 MB)');
    expect(summaryRows(summary({ videos: 3, bytes: 3 * GB, new: 3 }))[0].value).toBe('3 videos (3.0 GB)');
  });

  it('adds the picked row only when there are some', () => {
    const rows = summaryRows(summary({ photos: 60, bytes: MB, new: 0, kept_before: 50, looks_same: 10 }), 4);
    expect(rows.map((r) => `${r.label}: ${r.value}`)).toEqual(['Found: 60 photos (1.0 MB)', 'New: 0', 'Picked: 4']);
  });
});

describe('workingRows', () => {
  it('counts up what an extraction has produced so far', () => {
    expect(workingRows({ photos: 0, videos: 0, bytes_done: 0, bytes_total: 6 * GB })[0].value).toBe('Starting…');
    expect(workingRows({ photos: 120, videos: 3, bytes_done: 1.5 * GB, bytes_total: 6 * GB })).toEqual([
      { label: 'Found', value: '120 photos, 3 videos (1.5 GB of 6.0 GB)' },
      { label: 'New', value: '…' },
    ]);
    expect(workingRows({ photos: 1, videos: 0, bytes_done: MB, bytes_total: 2 * MB })[0].value).toBe('1 photo (1.0 MB of 2.0 MB)');
  });

  it('counts up what a card has been found to hold while it is read', () => {
    expect(workingRows(null, 0)[0].value).toBe('Starting…');
    expect(workingRows(null, 40)[0].value).toBe('40 photos or videos');
  });
});

describe('showPath', () => {
  it('leaves out a path the name already says', () => {
    expect(showPath('BB2018_PRO (G:)', 'G:\\')).toBe(false);
    expect(showPath('Holiday', 'D:\\Photos\\Holiday')).toBe(true);
    expect(showPath('Pictures', '/Users/me/Pictures')).toBe(true);
  });
});

describe('keepLine', () => {
  it('says what was imported, what was already there and what went wrong', () => {
    expect(keepLine(report({ copied: 362, already: 24, failed: 2, bytes: 5.9 * GB }))).toBe('Imported 362 items (5.9 GB). 24 were already in your folder. 2 could not be imported.');
  });

  it('is short when all went well', () => {
    expect(keepLine(report({ copied: 1, bytes: MB }))).toBe('Imported 1 item (1.0 MB).');
  });

  it('says when it was stopped', () => {
    expect(keepLine(report({ copied: 3, bytes: 3 * MB, stopped: true }))).toBe('Stopped. Imported 3 items (3.0 MB).');
    expect(keepLine(report({ stopped: true }))).toBe('Stopped before anything was imported.');
  });

  it('says when there was nothing to do', () => {
    expect(keepLine(report())).toBe('Nothing was imported.');
  });
});

describe('keepPercent', () => {
  it('is how far the bytes have got, and never more than 100', () => {
    expect(keepPercent(null)).toBe(0);
    expect(keepPercent({ done: 0, total: 0, bytes_done: 0, bytes_total: 0 })).toBe(0);
    expect(keepPercent({ done: 1, total: 4, bytes_done: 25, bytes_total: 100 })).toBe(25);
    expect(keepPercent({ done: 4, total: 4, bytes_done: 120, bytes_total: 100 })).toBe(100);
  });
});

describe('defaultDestination', () => {
  const folders = [folder(1, 'C:\\Photos'), folder(2, 'D:\\Family')];
  it('is the sync folder when it is one of the folders', () => {
    expect(defaultDestination(folders, 'D:\\Family', null)).toBe(2);
  });
  it('keeps the last one used while it still exists', () => {
    expect(defaultDestination(folders, 'D:\\Family', 1)).toBe(1);
    expect(defaultDestination(folders, null, 9)).toBe(1);
  });
  it('is the first folder otherwise, and nothing with no folders', () => {
    expect(defaultDestination(folders, null, null)).toBe(1);
    expect(defaultDestination([], null, null)).toBeNull();
  });
});

describe('folderLabel', () => {
  it('shows the name and the path, or the path alone when they say the same', () => {
    expect(folderLabel('P:\\')).toBe('P:\\');
    expect(folderLabel('D:\\Photos\\2024')).toBe('2024 (D:\\Photos\\2024)');
    expect(folderLabel('/Users/me/Pictures')).toBe('Pictures (/Users/me/Pictures)');
  });
  it('files imported photos by year and day', () => {
    expect(KEEP_LAYOUT).toBe('year-date');
  });
});

const zip = (over: Partial<ZipLook> = {}): ZipLook => ({ name: 'Trip.zip', photos: 0, videos: 0, other: 0, archives: 0, encrypted: 0, unsafe_names: 0, too_big: 0, bytes: 0, free: 100 * GB, fits: true, ...over });

describe('zipLine', () => {
  it('says what is in the zip and what will be left out', () => {
    expect(zipLine(zip({ photos: 412, videos: 18, bytes: 6.1 * GB, other: 3 }))).toBe('412 photos and 18 videos, 6.1 GB. 3 other files will be left out.');
    expect(zipLine(zip({ photos: 1, bytes: 2 * MB, other: 1 }))).toBe('1 photo, 2.0 MB. 1 other file will be left out.');
    expect(zipLine(zip({ videos: 2, bytes: 3 * MB }))).toBe('2 videos, 3.0 MB.');
  });

  it('says so when there is nothing to extract', () => {
    expect(zipLine(zip())).toBe('There are no photos or videos in this zip.');
    expect(zipLine(zip({ other: 4 }))).toBe('There are no photos or videos in this zip.');
  });
});

describe('spaceLine', () => {
  it('says what it needs and what is free', () => {
    expect(spaceLine(zip({ photos: 1, bytes: 6.1 * GB, free: 41 * GB }))).toBe('Needs 6.1 GB. 41.0 GB is free where the Inbox is.');
  });

  it('refuses in words when it does not fit', () => {
    expect(spaceLine(zip({ photos: 1, bytes: 6.1 * GB, free: 2 * GB, fits: false }))).toBe('Needs 6.1 GB but only 2.0 GB is free where the Inbox is.');
  });

  it('says nothing when nothing is to be extracted', () => {
    expect(spaceLine(zip())).toBe('');
  });
});

describe('zipProblems', () => {
  it('is empty when all is well', () => {
    expect(zipProblems(zip({ photos: 3, bytes: 3 }))).toEqual([]);
  });

  it('names each kind of entry that will be skipped, one or many', () => {
    expect(zipProblems(zip({ unsafe_names: 1 }))).toEqual(['1 entry has an unsafe name and will be skipped.']);
    expect(zipProblems(zip({ unsafe_names: 2, encrypted: 1, too_big: 3, archives: 2 }))).toEqual([
      '2 entries have unsafe names and will be skipped.',
      '1 entry is encrypted and will be skipped.',
      '3 entries expand far more than they should and will be skipped.',
      '2 zips inside this zip will not be opened.',
    ]);
    expect(zipProblems(zip({ encrypted: 2 }))).toEqual(['2 entries are encrypted and will be skipped.']);
    expect(zipProblems(zip({ archives: 1 }))).toEqual(['1 zip inside this zip will not be opened.']);
  });
});

describe('extractLine', () => {
  it('counts how far the extraction has got', () => {
    expect(extractLine(3, 400)).toBe('Extracting 3 of 400…');
    expect(extractLine(0, 0)).toBe('Extracting…');
  });
});
