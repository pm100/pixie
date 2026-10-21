import { describe, expect, it } from 'vitest';
import { readFraction, scanFraction, scanText } from './scanProgress';

describe('scanFraction', () => {
  it('is files found so far over the items the folder had, never above 1', () => {
    expect(scanFraction(250, 1000)).toBe(0.25);
    expect(scanFraction(1000, 1000)).toBe(1);
    expect(scanFraction(1500, 1000)).toBe(1);
  });
  it('is unknown (null) for a folder with no known size, so the bar can just move', () => {
    expect(scanFraction(10, 0)).toBeNull();
    expect(scanFraction(0, 0)).toBeNull();
  });
  it('starts at 0', () => {
    expect(scanFraction(0, 1000)).toBe(0);
  });
});

describe('scanText', () => {
  it('counts files found while the folder is still being walked', () => {
    expect(scanText({ seen: 10_000, indexed: 0, toIndex: 0 })).toBe('Scanning… 10,000 found');
  });
  it('says the folder is being opened before anything has been found, so a slow or sleeping drive shows life', () => {
    expect(scanText({ seen: 0, indexed: 0, toIndex: 0 })).toBe('Opening the folder…');
  });
  it('shows how many of the files to read are done once reading starts', () => {
    expect(scanText({ seen: 10_000, indexed: 3_200, toIndex: 10_000 })).toBe('Reading 3,200 of 10,000');
    expect(scanText({ seen: 10_000, indexed: 0, toIndex: 10_000 })).toBe('Reading 0 of 10,000');
  });
});

describe('readFraction', () => {
  it('is the share of the files to read that are done, unknown while walking', () => {
    expect(readFraction({ seen: 100, indexed: 25, toIndex: 100 })).toBe(0.25);
    expect(readFraction({ seen: 100, indexed: 0, toIndex: 0 })).toBeNull();
  });
});
