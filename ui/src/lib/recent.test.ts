import { describe, expect, it } from 'vitest';
import { parseRecentDays, recentLabel, recentSince } from './recent';

describe('recentSince', () => {
  it('is 30 days before now by default', () => {
    expect(recentSince(1_000_000_000)).toBe(1_000_000_000 - 30 * 86_400);
  });
  it('takes a different span', () => {
    expect(recentSince(1_000_000_000, 7)).toBe(1_000_000_000 - 7 * 86_400);
  });
});

describe('parseRecentDays', () => {
  it('defaults to 30 for anything that is not a sensible whole number of days', () => {
    for (const v of [undefined, null, 'x', '7', NaN, 0, -3, 2.5, 100_000, Infinity]) expect(parseRecentDays(v)).toBe(30);
  });
  it('keeps a whole number from 1 to 3650', () => {
    expect(parseRecentDays(1)).toBe(1);
    expect(parseRecentDays(90)).toBe(90);
    expect(parseRecentDays(3650)).toBe(3650);
  });
});

describe('recentLabel', () => {
  it('names the span', () => {
    expect(recentLabel(30)).toBe('Last 30 days');
    expect(recentLabel(1)).toBe('Last day');
  });
});
