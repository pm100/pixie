import { describe, expect, it } from 'vitest';
import type { FolderInfo, KeepReport, SourceSummary } from './api';
import { LAYOUTS, defaultDestination, keepLine, keepPercent, keepWords, summaryLine } from './importer';

const MB = 1024 ** 2;
const GB = 1024 ** 3;
const summary = (over: Partial<SourceSummary> = {}): SourceSummary => ({ photos: 0, videos: 0, bytes: 0, new: 0, kept_before: 0, looks_same: 0, first: null, last: null, ...over });
const report = (over: Partial<KeepReport> = {}): KeepReport => ({ batch: 1, copied: 0, already: 0, failed: 0, problems: [], bytes: 0, stopped: false, ...over });
const folder = (id: number, path: string): FolderInfo => ({ id, path, count: 0, feeds_master: false });

describe('summaryLine', () => {
  it('says there is nothing when there is nothing', () => {
    expect(summaryLine(summary())).toBe('No photos or videos found.');
  });

  it('counts photos, videos and size, then how many are new', () => {
    expect(summaryLine(summary({ photos: 412, videos: 18, bytes: 6.1 * GB, new: 380, kept_before: 40, looks_same: 10 }))).toBe('412 photos and 18 videos, 6.1 GB. 380 are new.');
  });

  it('only names what is there, and says one when it is one', () => {
    expect(summaryLine(summary({ photos: 1, bytes: 2 * MB, new: 1 }))).toBe('1 photo, 2.0 MB. 1 is new.');
    expect(summaryLine(summary({ videos: 3, bytes: 3 * GB, new: 3 }))).toBe('3 videos, 3.0 GB. 3 are new.');
  });

  it('says so when nothing is new, and why', () => {
    expect(summaryLine(summary({ photos: 5, bytes: 5 * MB, new: 0, kept_before: 5 }))).toBe('5 photos, 5.0 MB. Nothing is new: all of it was kept before or is already in your library.');
  });
});

describe('keepWords', () => {
  it('labels the Keep button with what it will keep', () => {
    expect(keepWords('new', 380)).toBe('Keep 380 new items');
    expect(keepWords('new', 1)).toBe('Keep 1 new item');
    expect(keepWords('picked', 12)).toBe('Keep 12 picked items');
    expect(keepWords('picked', 0)).toBe('Keep picked items');
  });
});

describe('keepLine', () => {
  it('says what was kept, what was already there and what went wrong', () => {
    expect(keepLine(report({ copied: 362, already: 24, failed: 2, bytes: 5.9 * GB }))).toBe('Kept 362 items (5.9 GB). 24 were already in your folder. 2 could not be kept.');
  });

  it('is short when all went well', () => {
    expect(keepLine(report({ copied: 1, bytes: MB }))).toBe('Kept 1 item (1.0 MB).');
  });

  it('says when it was stopped', () => {
    expect(keepLine(report({ copied: 3, bytes: 3 * MB, stopped: true }))).toBe('Stopped. Kept 3 items (3.0 MB).');
    expect(keepLine(report({ stopped: true }))).toBe('Stopped before anything was kept.');
  });

  it('says when there was nothing to do', () => {
    expect(keepLine(report())).toBe('Nothing was kept.');
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

describe('LAYOUTS', () => {
  it('offers the three filings, the date one first', () => {
    expect(LAYOUTS.map((l) => l.id)).toEqual(['year-date', 'year-month', 'flat']);
  });
});
