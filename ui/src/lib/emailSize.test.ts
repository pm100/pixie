import { describe, expect, it } from 'vitest';
import { DEFAULT_LIMIT_MB, MB, describeItems, mbText, parseLimit, sentBytes, tooBig } from './emailSize';

describe('parseLimit', () => {
  it('takes a sensible saved number and falls back to the default for anything else', () => {
    expect(parseLimit(15)).toBe(15);
    expect(parseLimit(25.7)).toBe(25);
    expect(parseLimit(0)).toBe(DEFAULT_LIMIT_MB);
    expect(parseLimit(9999)).toBe(DEFAULT_LIMIT_MB);
    expect(parseLimit('20')).toBe(DEFAULT_LIMIT_MB);
    expect(parseLimit(null)).toBe(DEFAULT_LIMIT_MB);
  });
});

describe('tooBig', () => {
  it('counts the third that sending adds', () => {
    expect(sentBytes(15 * MB)).toBeGreaterThan(19 * MB);
    expect(tooBig(10 * MB, 20)).toBe(false);
    expect(tooBig(16 * MB, 20)).toBe(true);
    expect(tooBig(0, 20)).toBe(false);
  });
});

describe('the words', () => {
  it('say sizes in whole megabytes from ten up and one decimal below', () => {
    expect(mbText(4.2 * MB)).toBe('4.2 MB');
    expect(mbText(48 * MB)).toBe('48 MB');
  });
  it('count photos and videos, one of each in the singular', () => {
    expect(describeItems(12, 0)).toBe('12 photos');
    expect(describeItems(1, 1)).toBe('1 photo and 1 video');
    expect(describeItems(0, 3)).toBe('3 videos');
  });
});
