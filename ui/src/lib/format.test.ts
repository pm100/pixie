import { describe, expect, it } from 'vitest';
import { basename, dirname, formatBytes, formatDateTime, formatDay, formatGps, weekday } from './format';

describe('dates', () => {
  it('knows the weekday of a day string', () => {
    expect(weekday('2024-06-01')).toBe('Sat');
    expect(weekday('2024-06-03')).toBe('Mon');
    expect(weekday('2023-12-25')).toBe('Mon');
  });
  it('formats a day heading, and Undated for null', () => {
    expect(formatDay('2024-06-03')).toBe('Mon 3 Jun 2024');
    expect(formatDay(null)).toBe('Undated');
  });
  it('formats a timestamp (stored seconds) as day and minute, or Unknown', () => {
    expect(formatDateTime(1_717_243_200)).toBe('1 Jun 2024, 12:00');
    expect(formatDateTime(null)).toBe('Unknown');
  });
});

describe('sizes and paths', () => {
  it('formats byte counts', () => {
    expect(formatBytes(0)).toBe('0 B');
    expect(formatBytes(1023)).toBe('1023 B');
    expect(formatBytes(1536)).toBe('1.5 KB');
    expect(formatBytes(5 * 1024 * 1024)).toBe('5.0 MB');
  });
  it('splits Windows and POSIX paths', () => {
    expect(basename('C:\\Users\\p\\a.jpg')).toBe('a.jpg');
    expect(dirname('C:\\Users\\p\\a.jpg')).toBe('C:\\Users\\p');
    expect(basename('/x/y/z.jpg')).toBe('z.jpg');
    expect(dirname('/x/y/z.jpg')).toBe('/x/y');
    expect(basename('plain.jpg')).toBe('plain.jpg');
    expect(dirname('plain.jpg')).toBe('');
  });
});

describe('formatGps', () => {
  it('shows hemispheres to 4 decimals, or null when either is missing', () => {
    expect(formatGps(51.5007, -0.1246)).toBe('51.5007° N, 0.1246° W');
    expect(formatGps(-33.8688, 151.2093)).toBe('33.8688° S, 151.2093° E');
    expect(formatGps(null, 5)).toBeNull();
    expect(formatGps(5, null)).toBeNull();
  });
});
