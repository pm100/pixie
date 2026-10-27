import { describe, expect, it } from 'vitest';
import { titleFor } from './clipTitle';

describe('titleFor', () => {
  it('gives the full text when it is cut off', () => {
    expect(titleFor(300, 180, '  A very long name  ')).toBe('A very long name');
  });
  it('gives nothing when all of it shows (a pixel of rounding is not "cut off")', () => {
    expect(titleFor(180, 180, 'Short')).toBe('');
    expect(titleFor(181, 180, 'Short')).toBe('');
  });
});
