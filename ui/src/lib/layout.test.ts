import { describe, expect, it } from 'vitest';
import type { DayGroup } from './api';
import { buildLayout, groupAt, groupTop, revealOffset, rowAt, sameGroups, visibleRows } from './layout';

const g = (count: number, first_index: number, day: string | null = '2024-06-01'): DayGroup => ({ day, count, first_index });

// cols 4, cell row 100, heading 30:
//   heading 0..30, cells(0,4) 30..130, cells(4,1) 130..230,
//   heading 230..260, cells(5,2) 260..360
const layout = () => buildLayout([g(5, 0), g(2, 5, '2024-05-31')], 4, 100, 30);

describe('buildLayout', () => {
  it('puts a heading before each group and wraps its photos into rows', () => {
    const l = layout();
    expect(l.rows.map((r) => [r.kind, r.top, r.h])).toEqual([
      ['heading', 0, 30], ['cells', 30, 100], ['cells', 130, 100], ['heading', 230, 30], ['cells', 260, 100],
    ]);
    const cells = l.rows.filter((r) => r.kind === 'cells');
    expect(cells.map((r) => r.kind === 'cells' && [r.first, r.n])).toEqual([[0, 4], [4, 1], [5, 2]]);
    expect(l.height).toBe(360);
    expect(l.groupTops).toEqual([0, 230]);
  });

  it('does not add a row when a day has exactly cols photos, and does when it has cols + 1', () => {
    expect(buildLayout([g(4, 0)], 4, 100, 30).rows).toHaveLength(2);
    expect(buildLayout([g(5, 0)], 4, 100, 30).rows).toHaveLength(3);
  });

  it('is empty for no groups and treats cols < 1 as 1', () => {
    expect(buildLayout([], 4, 100, 30)).toEqual({ rows: [], height: 0, groupTops: [] });
    expect(buildLayout([g(2, 0)], 0, 100, 30).rows).toHaveLength(3);
  });

  it('handles a library that is only Undated', () => {
    const l = buildLayout([g(6, 0, null)], 4, 100, 30);
    expect(l.rows).toHaveLength(3);
    expect(l.height).toBe(230);
  });
});

describe('rowAt / groupAt / groupTop', () => {
  it('finds the row containing a y offset, clamped to the ends', () => {
    const l = layout();
    expect([-5, 0, 35, 229, 230, 9999].map((y) => rowAt(l, y))).toEqual([0, 0, 1, 2, 3, 4]);
  });
  it('reports the group at an offset and where a group starts', () => {
    const l = layout();
    expect([0, 200, 230, 359].map((y) => groupAt(l, y))).toEqual([0, 0, 1, 1]);
    expect(groupTop(l, 1)).toBe(230);
    expect(groupTop(l, 99)).toBe(360); // past the end: bottom of the list
  });
  it('is safe on an empty layout', () => {
    const l = buildLayout([], 4, 100, 30);
    expect(rowAt(l, 10)).toBe(-1);
    expect(groupAt(l, 10)).toBe(-1);
    expect(groupTop(l, 0)).toBe(0);
  });
});

describe('visibleRows', () => {
  it('returns the rows overlapping the viewport plus overscan', () => {
    const l = layout();
    expect(visibleRows(l, 0, 100, 0)).toEqual([0, 2]);
    expect(visibleRows(l, 0, 100, 1)).toEqual([0, 3]);
    expect(visibleRows(l, 240, 50, 0)).toEqual([3, 5]);
    expect(visibleRows(l, 240, 50, 5)).toEqual([0, 5]);
  });
  it('is [0, 0] when there is nothing to show', () => {
    expect(visibleRows(buildLayout([], 4, 100, 30), 0, 500)).toEqual([0, 0]);
  });
});

describe('sameGroups', () => {
  it('is true only when every group matches', () => {
    expect(sameGroups([g(5, 0), g(2, 5)], [g(5, 0), g(2, 5)])).toBe(true);
    expect(sameGroups([], [])).toBe(true);
    expect(sameGroups([g(5, 0)], [g(6, 0)])).toBe(false);
    expect(sameGroups([g(5, 0)], [g(5, 0), g(1, 5)])).toBe(false);
    expect(sameGroups([g(1, 0, 'a')], [g(1, 0, 'b')])).toBe(false);
  });
});

describe('revealOffset', () => {
  it('centres the row of the photo in the view, within the list', () => {
    expect(revealOffset(layout(), 5, 0, 200)).toBe(160); // row 260..360 would centre at 210, but the list ends at 360
    expect(revealOffset(layout(), 0, 200, 200)).toBe(0); // row 30..130 would centre at -20
    expect(revealOffset(layout(), 4, 0, 100)).toBe(130); // row 130..230 centres at 130 + 50 - 50
  });
  it('does not move when the row of the photo is already fully in view', () => {
    expect(revealOffset(layout(), 1, 0, 200)).toBeNull();
    expect(revealOffset(layout(), 4, 100, 200)).toBeNull();
  });
  it('moves when only part of the row is in view', () => {
    expect(revealOffset(layout(), 4, 0, 200)).not.toBeNull(); // row 130..230, view 0..200
  });
  it('gives nothing for a position the list does not have', () => {
    expect(revealOffset(layout(), 7, 0, 200)).toBeNull();
    expect(revealOffset(layout(), -1, 0, 200)).toBeNull();
  });
});
