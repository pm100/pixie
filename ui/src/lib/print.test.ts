import { describe, expect, it } from 'vitest';
import { PAPERS, grid, pageSize, sheets } from './print';

describe('sheets', () => {
  it('puts the photos on pages, perPage to a page, in order', () => {
    expect(sheets([1, 2, 3, 4, 5], 2)).toEqual([[1, 2], [3, 4], [5]]);
  });
  it('has no pages for no photos', () => {
    expect(sheets([], 4)).toEqual([]);
  });
});

describe('grid', () => {
  it('has the columns and rows for each photos-per-page choice', () => {
    expect(grid(1)).toEqual({ cols: 1, rows: 1 });
    expect(grid(2)).toEqual({ cols: 1, rows: 2 });
    expect(grid(4)).toEqual({ cols: 2, rows: 2 });
    expect(grid(6)).toEqual({ cols: 2, rows: 3 });
  });
  it('swaps for a landscape page so the cells stay roomy', () => {
    expect(grid(2, true)).toEqual({ cols: 2, rows: 1 });
    expect(grid(6, true)).toEqual({ cols: 3, rows: 2 });
  });
});

describe('pageSize', () => {
  it('gives the paper in millimetres, turned for landscape', () => {
    expect(pageSize('a4', false)).toEqual({ w: 210, h: 297 });
    expect(pageSize('a4', true)).toEqual({ w: 297, h: 210 });
    expect(pageSize('4x6', false)).toEqual({ w: 101.6, h: 152.4 });
  });
  it('knows A4, Letter, 4x6 and 5x7', () => {
    expect(Object.keys(PAPERS)).toEqual(['a4', 'letter', '4x6', '5x7']);
  });
});
