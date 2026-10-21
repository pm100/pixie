import { describe, expect, it } from 'vitest';
import { analysisFraction, analysisLine, type AnalysisStatus } from './analysis';

const base: AnalysisStatus = { models: 'ready', model_done: 0, model_total: 0, done: 0, total: 0, paused: false, error: null };
const s = (o: Partial<AnalysisStatus>): AnalysisStatus => ({ ...base, ...o });

describe('analysisLine', () => {
  it('says the model is not downloaded yet', () => {
    expect(analysisLine(s({ models: 'missing' }))).toBe('Picture-analysis model not downloaded yet');
  });
  it('shows download percent', () => {
    expect(analysisLine(s({ models: 'downloading', model_done: 42, model_total: 100 }))).toBe('Downloading picture-analysis model… 42%');
  });
  it('shows 0% when the size is not known yet', () => {
    expect(analysisLine(s({ models: 'downloading' }))).toBe('Downloading picture-analysis model… 0%');
  });
  it('shows analysis progress with thousands separators', () => {
    expect(analysisLine(s({ done: 1234, total: 50000 }))).toBe('Analysing photos… 1,234 of 50,000');
  });
  it('says all done', () => {
    expect(analysisLine(s({ done: 50000, total: 50000 }))).toBe('All 50,000 photos analysed');
  });
  it('says nothing to analyse for an empty library', () => {
    expect(analysisLine(s({}))).toBe('No photos to analyse yet');
  });
  it('shows paused', () => {
    expect(analysisLine(s({ done: 1234, total: 50000, paused: true }))).toBe('Paused at 1,234 of 50,000');
  });
  it('reports a model that is failing while photos are being analysed', () => {
    expect(analysisLine(s({ done: 10, total: 50, error: 'no memory' }))).toBe('Picture analysis is having trouble: no memory');
  });
  it('shows the error', () => {
    expect(analysisLine(s({ models: 'error', error: 'offline' }))).toBe('Could not get the picture-analysis model: offline');
  });
});

describe('analysisFraction', () => {
  it('is null when there is nothing to show a bar for', () => {
    expect(analysisFraction(s({}))).toBeNull();
    expect(analysisFraction(s({ models: 'missing' }))).toBeNull();
    expect(analysisFraction(s({ models: 'error', error: 'x' }))).toBeNull();
  });
  it('tracks the download, then the analysis', () => {
    expect(analysisFraction(s({ models: 'downloading', model_done: 25, model_total: 100 }))).toBe(0.25);
    expect(analysisFraction(s({ done: 10, total: 40 }))).toBe(0.25);
    expect(analysisFraction(s({ done: 40, total: 40 }))).toBe(1);
  });
});
