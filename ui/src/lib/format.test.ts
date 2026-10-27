import { describe, expect, it } from 'vitest';
import { basename, dirname, folderName, formatBytes, formatDateTime, formatDay, formatDuration, formatGps, plural, weekday } from './format';

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

describe('formatDuration', () => {
  it('formats seconds, minutes and hours', () => {
    expect(formatDuration(0)).toBe('0:00');
    expect(formatDuration(999)).toBe('0:00');
    expect(formatDuration(42_000)).toBe('0:42');
    expect(formatDuration(61_500)).toBe('1:01');
    expect(formatDuration(3_787_000)).toBe('1:03:07');
  });
  it('is empty for unknown or invalid durations', () => {
    expect(formatDuration(null)).toBe('');
    expect(formatDuration(-5)).toBe('');
    expect(formatDuration(Number.NaN)).toBe('');
  });
});

describe('folderName', () => {
  it('is the last part of a folder path, with or without a trailing slash', () => {
    expect(folderName('C:\\Users\\me\\Pictures')).toBe('Pictures');
    expect(folderName('C:\\Users\\me\\Pictures\\')).toBe('Pictures');
    expect(folderName('/home/me/photos/')).toBe('photos');
  });
  it('is the whole path for a drive root or a share, which has no last part to show', () => {
    expect(folderName('P:\\')).toBe('P:\\');
    expect(folderName('E:/')).toBe('E:/');
    expect(folderName('/')).toBe('/');
  });
});

describe('plural', () => {
  it('pluralises only when there is more than one', () => {
    expect(plural(1, 'item')).toBe('1 item');
    expect(plural(0, 'item')).toBe('0 items');
    expect(plural(2, 'photo')).toBe('2 photos');
    expect(plural(1, 'person', 'people')).toBe('1 person');
    expect(plural(3, 'person', 'people')).toBe('3 people');
    expect(plural(1234, 'video')).toBe('1,234 videos');
  });
});
