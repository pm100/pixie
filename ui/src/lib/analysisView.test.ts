import { describe, expect, it } from 'vitest';
import { analysisView, type AnalysisInput, type LoadProgress } from './analysisView';
import type { AnalysisStatus } from './analysis';
import type { DupStatus } from './dupes';

const status = (o: Partial<AnalysisStatus> = {}): AnalysisStatus => ({
  models: 'ready', model_done: 0, model_total: 0, done: 0, total: 0, paused: false, error: null, ...o,
});
const dups = (o: Partial<DupStatus> = {}): DupStatus => ({ done: 0, total: 0, groups: 0, extra_files: 0, extra_bytes: 0, hidden: 0, paused: false, ...o });
const input = (o: Partial<AnalysisInput> = {}): AnalysisInput => ({ tags: status(), faces: status(), dupes: null, people: 0, ...o });
const counter = (v: ReturnType<typeof analysisView>, key: string) => v.counters.find((c) => c.key === key)!;

describe('the counters', () => {
  it('list Auto tags and People, then Duplicates once their numbers are known', () => {
    expect(analysisView(input()).counters.map((c) => c.label)).toEqual(['Auto tags', 'People']);
    expect(analysisView(input({ dupes: dups({ done: 5, total: 10 }) })).counters.map((c) => c.label)).toEqual(['Auto tags', 'People', 'Duplicates']);
  });

  it('say what has been found, with no "of" totals: photos analysed, people found, duplicates found', () => {
    const v = analysisView(input({ tags: status({ done: 3214, total: 14230 }), faces: status({ done: 1108, total: 14230 }), people: 14, dupes: dups({ done: 9400, total: 14232, extra_files: 412 }) }));
    expect(counter(v, 'tags').text).toBe('3,214 photos');
    expect(counter(v, 'faces').text).toBe('14 found');
    expect(counter(v, 'dupes').text).toBe('412 found');
    expect(v.counters.some((c) => / of /.test(c.text))).toBe(false);
  });

  it('a switched-off analysis is not listed and nothing waits for it', () => {
    const v = analysisView(input({ tags: status({ done: 5, total: 5 }), faces: null, on: { faces: false } }));
    expect(v.counters.map((c) => c.key)).toEqual(['tags']);
    expect(v.finished).toBe(true);
    expect(analysisView(input({ on: { tags: false, faces: false, dupes: false } })).finished).toBe(false); // nothing listed is not "done"
  });
});

describe('the one bar', () => {
  it('follows all the analyses together: the work done over the work there is', () => {
    const v = analysisView(input({ tags: status({ done: 3000, total: 10000 }), faces: status({ done: 1000, total: 10000 }) }));
    expect(v.headline).toBe('Analysing 20%');
    expect(v.fraction).toBeCloseTo(0.2);
    expect(v.working).toBe(true);
  });

  it('counts a finished analysis as done work, so the bar never jumps back when one of them finishes', () => {
    const v = analysisView(input({ tags: status({ done: 10000, total: 10000 }), faces: status({ done: 2500, total: 10000 }) }));
    expect(v.fraction).toBeCloseTo(0.625);
    expect(counter(v, 'tags').done).toBe(true);
  });

  it('shows the model download while a model is on its way', () => {
    const v = analysisView(input({ tags: status({ models: 'downloading', model_done: 62, model_total: 100 }), faces: status({ models: 'downloading', model_done: 1, model_total: 4 }) }));
    expect(v.headline).toBe('Getting models… 43%');
    expect(v.fraction).toBeCloseTo(0.435);
    expect(counter(v, 'tags').text).toBe('Getting the picture-analysis model… 62%');
    expect(counter(v, 'faces').text).toBe('Getting the face model… 25%');
    expect(v.finished).toBe(false);
  });

  it('is full and says Done when everything is done', () => {
    const done = analysisView(input({ tags: status({ done: 2, total: 2 }), faces: status({ done: 2, total: 2 }) }));
    expect(done.finished).toBe(true);
    expect(done.headline).toBe('Done');
    expect(done.fraction).toBe(1);
  });

  it('has no bar until there is something to measure', () => {
    const v = analysisView(input({ tags: null, faces: null }));
    expect(v.fraction).toBeNull();
    expect(v.headline).toBe('Starting…');
    expect(v.finished).toBe(false);
  });
});

describe('what needs the user', () => {
  it('a failed model is an error with the reason, and not done', () => {
    const v = analysisView(input({ tags: status({ models: 'error', error: 'offline' }) }));
    expect(v.problems).toEqual([{ key: 'tags', kind: 'error', text: 'Could not get the picture-analysis model: offline' }]);
    expect(v.failed).toBe(true);
    expect(counter(v, 'tags').done).toBe(false);
  });

  it('a model that is ready but failing on photos is an error too', () => {
    const v = analysisView(input({ tags: status({ done: 1, total: 5, error: 'decoder' }) }));
    expect(v.problems[0].text).toBe('Picture analysis is having trouble: decoder');
  });

  it('paused is said once, however many analyses are paused, and is not done', () => {
    const v = analysisView(input({ tags: status({ done: 10, total: 100, paused: true }), faces: status({ done: 5, total: 100, paused: true }), dupes: dups({ done: 1, total: 100, paused: true }) }));
    expect(v.problems.filter((p) => p.kind === 'paused')).toHaveLength(1);
    expect(v.paused).toBe(true);
    expect(v.working).toBe(false);
    expect(counter(v, 'tags').text).toBe('Paused');
    expect(v.finished).toBe(false);
  });
});

describe('knowing when the whole library is done', () => {
  it('is done when every analysis has done all its photos', () => {
    expect(analysisView(input({ tags: status({ done: 5000, total: 5000 }), faces: status({ done: 5200, total: 5200 }) })).finished).toBe(true);
    expect(analysisView(input({ tags: status({ done: 10, total: 5000 }) })).finished).toBe(false);
  });
  it('with nothing to analyse, an empty status really is nothing to do', () => {
    expect(counter(analysisView(input()), 'tags').done).toBe(true);
  });
  it('a missing status (not heard yet) is not done', () => {
    expect(analysisView(input({ tags: null, faces: null })).finished).toBe(false);
  });
});

describe('a load: the photos of the folders just added', () => {
  const load = (o: Partial<LoadProgress> = {}): LoadProgress => ({ photos: 200, shown: 200, hashed: 200, tagged: 200, scanned: 200, faces: 5, duplicates: 3, ...o });
  // The statuses describe the whole library, which is no use for a load's counts: these disagree on purpose.
  const library = { tags: status({ done: 13881, total: 13881 }), faces: status({ done: 13881, total: 13881 }), dupes: dups({ done: 99, total: 99999, extra_files: 700 }) };

  it('takes its counts from the load, not from the library', () => {
    const v = analysisView(input({ ...library, load: load({ tagged: 120, scanned: 100, hashed: 150, faces: 2, duplicates: 1 }) }));
    expect(counter(v, 'tags').text).toBe('120 photos');
    expect(counter(v, 'faces').text).toBe('2 found');
    expect(counter(v, 'dupes').text).toBe('1 found');
    expect(v.fraction).toBeCloseTo((120 + 100 + 150) / (200 + 200 + 200));
    expect(v.finished).toBe(false);
  });

  it('is done when the photos of the load are all hashed, tagged and scanned', () => {
    const v = analysisView(input({ ...library, load: load() }));
    expect(v.finished).toBe(true);
    expect(v.headline).toBe('Done');
  });

  it('is not thrown off by new photos hiding older copies (the library shows fewer photos than it did)', () => {
    // 332 new photos, all analysed; 16 older photos were hidden because the new copies are shown instead.
    const v = analysisView(input({ ...library, load: load({ photos: 332, shown: 332, hashed: 332, tagged: 332, scanned: 332, duplicates: 16 }) }));
    expect(v.finished).toBe(true);
    expect(counter(v, 'dupes').text).toBe('16 found');
  });

  it('waits while its counts are not known yet, and when nothing is shown there is nothing to do', () => {
    const waiting = analysisView(input({ ...library, load: null }));
    expect(waiting.counters.map((c) => c.text)).toEqual(['Getting ready…', 'Getting ready…', 'Getting ready…']);
    expect(waiting.finished).toBe(false);
    const hidden = analysisView(input({ ...library, load: load({ photos: 10, shown: 0, hashed: 10, tagged: 0, scanned: 0 }) }));
    expect(hidden.finished).toBe(true); // every new photo is a copy of one already shown
  });

  it('still shows a model on its way, and is not done until the photos have their tags', () => {
    const v = analysisView(input({ tags: status({ models: 'downloading', model_done: 1, model_total: 4 }), faces: status({ done: 9, total: 9 }), load: load({ tagged: 0, scanned: 200 }) }));
    expect(v.headline).toBe('Getting models… 25%');
    expect(v.finished).toBe(false);
  });

  it('shows a pause, from the status, while its photos are still being analysed', () => {
    const v = analysisView(input({ tags: status({ paused: true }), faces: status({ paused: true }), dupes: dups({ paused: true }), load: load({ tagged: 50, scanned: 50, hashed: 50 }) }));
    expect(v.paused).toBe(true);
    expect(counter(v, 'tags').text).toBe('Paused');
  });
});
