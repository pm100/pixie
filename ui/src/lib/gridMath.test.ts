import { describe, expect, it } from 'vitest';
import { clamp, columnsFor, visibleRange } from './gridMath';

describe('columnsFor', () => {
  it('fits as many cells as possible, at least 1', () => {
    expect(columnsFor(800, 180, 6)).toBe(4);
    expect(columnsFor(50, 180, 6)).toBe(1);
  });
});

describe('visibleRange', () => {
  it('returns an empty range for no photos', () => {
    expect(visibleRange(0, 4, 186, 0, 600, 2)).toEqual({ firstIndex: 0, endIndex: 0, rows: 0 });
  });
  it('covers the viewport at the top', () => {
    expect(visibleRange(1000, 4, 186, 0, 600, 0)).toEqual({ firstIndex: 0, endIndex: 20, rows: 250 });
  });
  it('applies overscan and clamps at the end', () => {
    expect(visibleRange(1000, 4, 186, 0, 600, 2).endIndex).toBe(28);
    expect(visibleRange(10, 4, 186, 0, 600, 2)).toEqual({ firstIndex: 0, endIndex: 10, rows: 3 });
  });
  it('starts at the scrolled row', () => {
    expect(visibleRange(1000, 4, 186, 1860, 600, 0).firstIndex).toBe(40);
  });
});

describe('clamp', () => {
  it('limits a value to the range', () => {
    expect(clamp(5, 1, 10)).toBe(5);
    expect(clamp(-3, 1, 10)).toBe(1);
    expect(clamp(99, 1, 10)).toBe(10);
  });
});
