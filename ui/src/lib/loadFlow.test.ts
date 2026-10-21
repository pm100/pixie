import { describe, expect, it } from 'vitest';
import { allSetSummary, analysingFinished, analysingLines, phaseAfterScan, startLabel } from './loadFlow';
import type { AnalysisStatus } from './analysis';

const status = (o: Partial<AnalysisStatus> = {}): AnalysisStatus => ({
  models: 'ready', model_done: 0, model_total: 0, done: 0, total: 0, paused: false, error: null, ...o,
});
const idle = { left: 0, max: 0 };
const line = (lines: ReturnType<typeof analysingLines>, key: string) => lines.find((l) => l.key === key)!;

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

describe('analysingLines', () => {
  it('has the three lines in order, each saying why it matters', () => {
    const lines = analysingLines(status(), status(), idle, 0);
    expect(lines.map((l) => l.key)).toEqual(['previews', 'search', 'faces']);
    expect(lines.map((l) => l.title)).toEqual(['Preparing previews', 'Understanding your photos', 'Finding faces']);
    expect(lines.every((l) => l.why.length > 10)).toBe(true);
    expect(lines[1].why).toContain('Auto tags'); // the term the Browse list uses, so the user knows what it means
    expect(lines[1].why.toLowerCase()).not.toContain('search'); // there is no text search for now
  });
  it('shows a model download in the same line, with its fraction, until counting starts', () => {
    const lines = analysingLines(status({ models: 'downloading', model_done: 62, model_total: 100 }), status({ models: 'downloading', model_done: 1, model_total: 4 }), idle, 0);
    expect(line(lines, 'search').text).toBe('Getting the picture-analysis model… 62%');
    expect(line(lines, 'search').fraction).toBeCloseTo(0.62);
    expect(line(lines, 'faces').text).toBe('Getting the face model… 25%');
    expect(line(lines, 'search').done).toBe(false);
  });
  it('counts photos once the model is ready, and the faces line says how many people so far', () => {
    const lines = analysingLines(status({ done: 3214, total: 14230 }), status({ done: 1108, total: 14230 }), idle, 14);
    expect(line(lines, 'search').text).toBe('3,214 of 14,230');
    expect(line(lines, 'search').fraction).toBeCloseTo(3214 / 14230);
    expect(line(lines, 'faces').text).toBe('1,108 of 14,230 · 14 people so far');
    expect(analysingLines(status(), status({ done: 1, total: 2 }), idle, 1).find((l) => l.key === 'faces')!.text).toBe('1 of 2 · 1 person so far');
  });
  it('a failed model is an error line with the reason, not done', () => {
    const l = line(analysingLines(status({ models: 'error', error: 'offline' }), status(), idle, 0), 'search');
    expect(l.error).toBe('Could not get the picture-analysis model: offline');
    expect(l.done).toBe(false);
  });
  it('a model that is ready but failing on photos is an error line too', () => {
    const l = line(analysingLines(status({ done: 1, total: 5, error: 'decoder' }), status(), idle, 0), 'search');
    expect(l.error).toBe('Picture analysis is having trouble: decoder');
  });
  it('paused shows paused and is not done', () => {
    const l = line(analysingLines(status({ done: 10, total: 100, paused: true }), status(), idle, 0), 'search');
    expect(l.paused).toBe(true);
    expect(l.done).toBe(false);
    expect(l.text).toBe('Paused at 10 of 100');
  });
  it('previews count down from the largest queue seen', () => {
    const l = line(analysingLines(status(), status(), { left: 250, max: 1000 }, 0), 'previews');
    expect(l.text).toBe('750 of 1,000');
    expect(l.fraction).toBeCloseTo(0.75);
    expect(l.done).toBe(false);
    expect(line(analysingLines(status(), status(), { left: 0, max: 1000 }, 0), 'previews').done).toBe(true);
  });
  it('a line is done when everything is analysed, or when there is nothing to analyse', () => {
    expect(line(analysingLines(status({ done: 5, total: 5 }), status({ total: 0 }), idle, 0), 'search').done).toBe(true);
    expect(line(analysingLines(status({ done: 5, total: 5 }), status({ total: 0 }), idle, 0), 'faces').done).toBe(true);
  });
  it('a missing status (not heard yet) is not done', () => {
    const lines = analysingLines(null, null, idle, 0);
    expect(line(lines, 'search').done).toBe(false);
    expect(analysingFinished(lines)).toBe(false);
  });
});

describe('analysingLines when the scan found photos', () => {
  const stale = status({ done: 0, total: 0 }); // the status still describes the library from before the scan
  it('a pass that has not yet seen the new photos is not done, however empty its status looks', () => {
    const lines = analysingLines(stale, stale, idle, 0, 5000);
    expect(line(lines, 'search').done).toBe(false);
    expect(line(lines, 'faces').done).toBe(false);
    expect(line(lines, 'search').text).toBe('Getting ready…');
    expect(analysingFinished(lines)).toBe(false);
  });
  it('a pass that has seen the new photos and finished them is done', () => {
    const lines = analysingLines(status({ done: 5000, total: 5000 }), status({ done: 5200, total: 5200 }), idle, 3, 5000);
    expect(line(lines, 'search').done && line(lines, 'faces').done).toBe(true);
  });
  it('a pass that has seen them but is still working is not done', () => {
    expect(line(analysingLines(status({ done: 10, total: 5000 }), status(), idle, 0, 5000), 'search').done).toBe(false);
  });
  it('with nothing expected, an empty status really is nothing to do', () => {
    expect(line(analysingLines(status(), status(), idle, 0, 0), 'search').done).toBe(true);
  });
});

describe('analysingFinished', () => {
  it('needs every line done', () => {
    const ok = analysingLines(status({ done: 2, total: 2 }), status({ done: 2, total: 2 }), idle, 0);
    expect(analysingFinished(ok)).toBe(true);
    expect(analysingFinished(analysingLines(status({ done: 1, total: 2 }), status({ done: 2, total: 2 }), idle, 0))).toBe(false);
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
