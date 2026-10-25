import { describe, expect, it } from 'vitest';
import { addLabel, allSetSummary, phaseAfterScan, startLabel } from './loadFlow';

describe('phaseAfterScan', () => {
  it('a stopped scan ends the flow and Browse opens with what was found', () => {
    expect(phaseAfterScan({ stopped: true, found: 500, photos: 400 })).toBe('off');
  });
  it('finding nothing goes straight to the end screen, which says so', () => {
    expect(phaseAfterScan({ stopped: false, found: 0, photos: 0 })).toBe('allset');
  });
  it('finding photos goes on to analysing', () => {
    expect(phaseAfterScan({ stopped: false, found: 12, photos: 10 })).toBe('analysing');
  });
  it('finding only videos has nothing to analyse, so it goes straight to the end screen', () => {
    expect(phaseAfterScan({ stopped: false, found: 12, photos: 0 })).toBe('allset');
  });
});

describe('allSetSummary', () => {
  const folders = [{ name: 'Pictures', count: 3412 }, { name: 'OneDrive Pictures', count: 10980 }, { name: 'Videos', count: 2105 }];
  it('says what was found, per folder, and how long it took', () => {
    const s = allSetSummary({ folders, photos: 14230, videos: 2267, seconds: 200, facesFound: true });
    expect(s.title).toBe('All set — 16,497 items found');
    expect(s.detail).toBe('14,230 photos and 2,267 videos, in 3 folders. That took 3 minutes.');
    expect(s.rows).toEqual([{ name: 'Pictures', text: '3,412' }, { name: 'OneDrive Pictures', text: '10,980' }, { name: 'Videos', text: '2,105' }]);
    expect(s.showPeopleBox).toBe(true);
  });
  it('leaves the people box out when no faces were found', () => {
    expect(allSetSummary({ folders, photos: 5, videos: 0, seconds: 5, facesFound: false }).showPeopleBox).toBe(false);
  });
  it('uses singular words and seconds for small numbers', () => {
    const s = allSetSummary({ folders: [{ name: 'Pictures', count: 2 }], photos: 1, videos: 1, seconds: 1, facesFound: false });
    expect(s.detail).toBe('1 photo and 1 video, in 1 folder. That took 1 second.');
    expect(allSetSummary({ folders: [], photos: 3, videos: 0, seconds: 59, facesFound: false }).detail).toContain('59 seconds');
    expect(allSetSummary({ folders: [], photos: 3, videos: 0, seconds: 60, facesFound: false }).detail).toContain('1 minute.');
  });
  it('lists the folders that could not be read, even when others were found', () => {
    const problems = ['Could not read E:\Photos: not found'];
    const s = allSetSummary({ folders, photos: 5, videos: 0, seconds: 5, facesFound: false, problems });
    expect(s.title).toBe('All set — 5 items found');
    expect(s.problems).toEqual(problems);
    expect(allSetSummary({ folders, photos: 5, videos: 0, seconds: 5, facesFound: false }).problems).toEqual([]);
  });
  it('lists them when nothing at all was found', () => {
    const s = allSetSummary({ folders: [], photos: 0, videos: 0, seconds: 2, facesFound: false, problems: ['unreadable'] });
    expect(s.title).toBe('Nothing was found');
    expect(s.problems).toEqual(['unreadable']);
  });
  it('says nothing was found, and never claims success or shows the people box', () => {
    const s = allSetSummary({ folders, photos: 0, videos: 0, seconds: 4, facesFound: true });
    expect(s.title).toBe('Nothing was found');
    expect(s.showPeopleBox).toBe(false);
    expect(s.rows).toEqual([]);
  });
});

describe('startLabel', () => {
  it('says just Start, and is off with nothing ticked', () => {
    expect(startLabel(0)).toEqual({ label: 'Start', disabled: true });
    expect(startLabel(1)).toEqual({ label: 'Start', disabled: false });
    expect(startLabel(4)).toEqual({ label: 'Start', disabled: false });
  });
});

describe('addLabel', () => {
  it('says Add until something is ticked, then how many folders', () => {
    expect(addLabel(0)).toEqual({ label: 'Add', disabled: true });
    expect(addLabel(1)).toEqual({ label: 'Add 1 folder', disabled: false });
    expect(addLabel(3)).toEqual({ label: 'Add 3 folders', disabled: false });
  });
});
