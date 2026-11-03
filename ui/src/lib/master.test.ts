import { describe, expect, it } from 'vitest';
import { hitsActive } from './tagNav';
import { filterNames } from './filterSummary';
import { cleanCollapsed } from './sections';
import { copyLine, lastCopyLine, reminderLine, shouldRemind, sharingRows, type CopyReport } from './master';

describe('without a sync folder', () => {
  it('has no rows in the Browse tree, whatever the counts and choice are', () => {
    expect(sharingRows(null, { unsynced: 5, private: 2 }, 'new')).toEqual([]);
  });
});

describe('with a sync folder', () => {
  it('lists New always, so the person can see it is there, and Private once there is something private', () => {
    expect(sharingRows('/family', { unsynced: 0, private: 0 }, null).map((r) => r.id)).toEqual(['new']);
    expect(sharingRows('/family', { unsynced: 3, private: 2 }, null).map((r) => [r.id, r.count])).toEqual([['new', 3], ['private', 2]]);
  });
  it('keeps Private listed while it is the choice, even when the last private item has gone', () => {
    expect(sharingRows('/family', { unsynced: 0, private: 0 }, 'private').map((r) => r.id)).toEqual(['new', 'private']);
  });
  it('marks the chosen row', () => {
    expect(sharingRows('/family', { unsynced: 3, private: 2 }, 'private').map((r) => r.active)).toEqual([false, true]);
  });
  it('explains each row in a whole sentence', () => {
    for (const r of sharingRows('/family', { unsynced: 3, private: 2 }, null)) expect(r.title).toMatch(/\.$/);
  });
});

describe('the share choice elsewhere', () => {
  it('limits the listing to hits, like the other tree choices', () => {
    expect(hitsActive(null, null, null, null, null, 'new')).toBe(true);
    expect(hitsActive(null, null, null, null, null, null)).toBe(false);
  });
  it('reads as a filter pill', () => {
    const f = { tag: null, person: null, folder: null, trip: null, date: null, kind: null, area: false };
    expect(filterNames({ ...f, share: 'new' })).toEqual(['New, not yet synced']);
    expect(filterNames({ ...f, share: 'private' })).toEqual(['Private']);
  });
  it('is a section the saved collapsed state knows', () => {
    expect(cleanCollapsed(['sharing', 'nonsense'])).toEqual(['sharing']);
  });
});

describe('syncing into the master', () => {
  const idle = { pending: 0, copied: 0, running: false, last: null };
  const report: CopyReport = { at: 1, copied: 0, already: 0, failed: 0, paused: null, problems: [], seconds: 0, bytes: 0 };
  it('says what is waiting, singular and plural', () => {
    expect(copyLine(idle)).toBe('Nothing is waiting to be synced.');
    expect(copyLine({ ...idle, pending: 1 })).toBe('1 item is waiting to be synced.');
    expect(copyLine({ ...idle, pending: 5 })).toBe('5 items are waiting to be synced.');
  });
  it('points to the Processing button while it runs', () => {
    expect(copyLine({ ...idle, running: true })).toMatch(/^Syncing\. .*Processing/);
    expect(copyLine({ ...idle, running: true, pending: 9 })).toMatch(/^Syncing\./);
  });
  it('sums up the last sync and says why it stopped', () => {
    expect(lastCopyLine(report)).toBe('Last sync: nothing to sync.');
    expect(lastCopyLine({ ...report, copied: 12, already: 3, failed: 1 })).toBe('Last sync: 12 synced, 3 already there, 1 failed.');
    expect(lastCopyLine({ ...report, copied: 120, seconds: 125, bytes: 360_000_000 })).toBe('Last sync: 120 synced (360 MB in 2 min 5 s, 2.9 MB/s).');
    expect(lastCopyLine({ ...report, copied: 2, seconds: 4, bytes: 8_000_000 })).toBe('Last sync: 2 synced (8 MB in 4 s, 2.0 MB/s).');
    expect(lastCopyLine({ ...report, copied: 900, seconds: 600, bytes: 4_500_000_000 })).toBe('Last sync: 900 synced (4.5 GB in 10 min 0 s, 7.5 MB/s).');
    expect(lastCopyLine({ ...report, already: 3, seconds: 5 })).toBe('Last sync: 3 already there.');
    expect(lastCopyLine({ ...report, copied: 4, paused: 'the sync folder cannot be reached' })).toBe('Last sync: 4 synced. Stopped: the sync folder cannot be reached.');
  });
});

describe('the start-up reminder', () => {
  it('says how many are new, to which folder, and to mark some private first', () => {
    expect(reminderLine(212, 'D:/Family')).toBe('212 items are new and not yet synced to D:/Family. Before syncing, look through them and mark any you do not want synced as private.');
    expect(reminderLine(1, 'D:/Family')).toContain('1 item is new');
    expect(reminderLine(1, 'D:/Family')).toContain('look through it');
  });
  it('shows only with a sync folder, something waiting, and the reminder on', () => {
    expect(shouldRemind('D:/Family', 5, true)).toBe(true);
    expect(shouldRemind(null, 5, true)).toBe(false);
    expect(shouldRemind('D:/Family', 0, true)).toBe(false);
    expect(shouldRemind('D:/Family', 5, false)).toBe(false);
  });
});
