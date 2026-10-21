import { describe, expect, it } from 'vitest';
import { backgroundProgress } from './backgroundProgress';
import type { AnalysisStatus } from './analysis';

const status = (o: Partial<AnalysisStatus> = {}): AnalysisStatus => ({
  models: 'ready', model_done: 0, model_total: 0, done: 0, total: 0, paused: false, error: null, ...o,
});
const noScan = { seen: 0, indexed: 0, toIndex: 0 };
const idle = { scanning: false, scan: noScan, analysis: status(), people: status() };

describe('backgroundProgress', () => {
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
    expect(p.detail).toBe('Auto tags: 3,000 of 10,000 · Faces: 1,000 of 10,000');
  });

  it('counts only the passes that have work left, once one of them is finished', () => {
    const p = backgroundProgress({ ...idle, analysis: status({ done: 10000, total: 10000 }), people: status({ done: 2500, total: 10000 }) })!;
    expect(p.fraction).toBeCloseTo(0.25);
    expect(p.detail).toBe('Faces: 2,500 of 10,000');
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
