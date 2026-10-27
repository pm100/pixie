import { describe, expect, it } from 'vitest';
import { backgroundProgress } from './backgroundProgress';
import type { AnalysisStatus } from './analysis';

const status = (o: Partial<AnalysisStatus> = {}): AnalysisStatus => ({
  models: 'ready', model_done: 0, model_total: 0, done: 0, total: 0, paused: false, error: null, ...o,
});
const noScan = { seen: 0, indexed: 0, toIndex: 0 };
const idle = { scanning: false, scan: noScan, analysis: status(), people: status() };

describe('backgroundProgress', () => {
  it('says loading while files are being read, with how far, whatever the analyses are doing (or not doing)', () => {
    const off = { ...idle, analysis: status({ models: 'off' }), people: status({ models: 'off' }) };
    const p = backgroundProgress({ ...off, loading: { pending: 25, peak: 100, paused: false } })!;
    expect([p.state, p.text, p.fraction]).toEqual(['working', 'Loading photos 75%', 0.75]);
    expect(backgroundProgress({ ...off, loading: { pending: 0, peak: 100, paused: false } })).toBeNull(); // loaded, and nothing else on
  });
  it('says loading paused when every analysis is off and the Pause is on with files still to load', () => {
    const off = { ...idle, analysis: status({ models: 'off' }), people: status({ models: 'off' }) };
    const p = backgroundProgress({ ...off, loading: { pending: 3, peak: 10, paused: true } })!;
    expect([p.state, p.text]).toEqual(['paused', 'Loading paused']);
  });
  it('says analysing, with the same figure as Settings, while files are still being read and an analysis is on', () => {
    const p = backgroundProgress({ ...idle, analysis: status({ done: 1, total: 2 }), people: status({ done: 1, total: 2 }), load: { fraction: 0.4 }, loading: { pending: 7000, peak: 7200, paused: false } })!;
    expect([p.state, p.text, p.fraction]).toEqual(['working', 'Analysing photos 40%', 0.4]);
    const q = backgroundProgress({ ...idle, analysis: status({ done: 1, total: 2, paused: true }), people: status({ done: 1, total: 2, paused: true }), loading: { pending: 3, peak: 10, paused: true } })!;
    expect([q.state, q.text]).toEqual(['paused', 'Analysis paused']);
  });
  it('keeps saying analysing, with no figure yet, when everything read so far is analysed but more files are coming', () => {
    const p = backgroundProgress({ ...idle, analysis: status({ done: 5, total: 5 }), people: status({ done: 5, total: 5 }), loading: { pending: 100, peak: 100, paused: false } })!;
    expect([p.state, p.text, p.fraction]).toEqual(['working', 'Analysing photos', null]);
  });
  it('shows how far the folders just added have got, not the whole library', () => {
    const whole = { ...idle, analysis: status({ done: 900, total: 1000 }), people: status({ done: 900, total: 1000 }) };
    expect(backgroundProgress(whole)!.text).toBe('Analysing photos 90%');
    const added = backgroundProgress({ ...whole, load: { fraction: 0.17 } })!;
    expect(added.text).toBe('Analysing photos 17%');
    expect(added.fraction).toBe(0.17);
  });

  it('is nothing when nothing is running', () => {
    expect(backgroundProgress(idle)).toBeNull();
    expect(backgroundProgress({ ...idle, analysis: null, people: null })).toBeNull();
    expect(backgroundProgress({ ...idle, analysis: status({ done: 10, total: 10 }), people: status({ done: 10, total: 10 }) })).toBeNull();
  });

  it('shows how far the scan has got while reading', () => {
    const p = backgroundProgress({ ...idle, scanning: true, scan: { seen: 10471, indexed: 3200, toIndex: 10471 } })!;
    expect(p.state).toBe('working');
    expect(p.text).toBe('Reading 3,200 of 10,471');
    expect(p.fraction).toBeCloseTo(3200 / 10471);
  });

  it('shows an unknown amount while a scan is still opening or walking a folder', () => {
    const p = backgroundProgress({ ...idle, scanning: true, scan: { seen: 500, indexed: 0, toIndex: 0 } })!;
    expect(p.text).toBe('Scanning… 500 found');
    expect(p.fraction).toBeNull();
  });

  it('combines the search and face passes into one percentage, with the detail behind it', () => {
    const p = backgroundProgress({ ...idle, analysis: status({ done: 3000, total: 10000 }), people: status({ done: 1000, total: 10000 }) })!;
    expect(p.state).toBe('working');
    expect(p.text).toBe('Analysing photos 20%');
    expect(p.fraction).toBeCloseTo(0.2);
    expect(p.detail).toBe('Still working on: Auto tags, Faces');
  });

  it('counts the duplicate check in the one overall percentage, without a count of its own', () => {
    const p = backgroundProgress({ ...idle, analysis: status({ done: 10000, total: 10000 }), people: status({ done: 10000, total: 10000 }), dupes: status({ done: 2500, total: 10000 }) })!;
    expect(p.state).toBe('working');
    expect(p.text).toBe('Analysing photos 75%'); // two analyses are done, the third a quarter of the way
    expect(p.detail).toBe('Still working on: Duplicates');
  });

  it('counts a finished pass as done work, so the percentage never jumps back when one finishes', () => {
    const p = backgroundProgress({ ...idle, analysis: status({ done: 10000, total: 10000 }), people: status({ done: 2500, total: 10000 }) })!;
    expect(p.fraction).toBeCloseTo(0.625);
    expect(p.detail).toBe('Still working on: Faces');
  });

  it('says a model is being downloaded, with its percentage', () => {
    const p = backgroundProgress({ ...idle, analysis: status({ models: 'downloading', model_done: 62, model_total: 100 }), people: status({ models: 'downloading', model_done: 1, model_total: 4 }) })!;
    expect(p.text).toBe('Getting models… 43%');
    expect(p.state).toBe('working');
  });

  it('shows paused work as paused, not as working', () => {
    const p = backgroundProgress({ ...idle, analysis: status({ done: 10, total: 100, paused: true }), people: status({ done: 10, total: 100, paused: true }) })!;
    expect(p.state).toBe('paused');
    expect(p.text).toBe('Analysis paused');
  });

  it('shows an error first, whatever else is running', () => {
    const p = backgroundProgress({ ...idle, scanning: true, scan: { seen: 5, indexed: 1, toIndex: 5 }, analysis: status({ models: 'error', error: 'offline' }) })!;
    expect(p.state).toBe('error');
    expect(p.text).toBe('Needs attention');
    expect(p.detail).toContain('offline');
  });

  it('the scan comes before the analysis while both are running', () => {
    const p = backgroundProgress({ ...idle, scanning: true, scan: { seen: 100, indexed: 10, toIndex: 100 }, analysis: status({ done: 1, total: 100 }) })!;
    expect(p.text).toBe('Reading 10 of 100');
  });
});
