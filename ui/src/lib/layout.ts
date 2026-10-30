import type { DayGroup } from './api';

/** One horizontal strip of the grid: a day heading, or up to `cols` thumbnails. */
export type Row =
  | { kind: 'heading'; group: number; top: number; h: number }
  | { kind: 'cells'; group: number; top: number; h: number; first: number; n: number };

export interface Layout {
  rows: Row[];
  height: number;
  /** Scroll offset of each group's heading. */
  groupTops: number[];
}

/** Lay out day groups: a heading row, then the group's photos wrapped to `cols` per row. */
export function buildLayout(groups: DayGroup[], cols: number, cellH: number, headH: number): Layout {
  const per = Math.max(1, cols);
  const rows: Row[] = [];
  const groupTops: number[] = [];
  let top = 0;
  groups.forEach((g, group) => {
    groupTops.push(top);
    rows.push({ kind: 'heading', group, top, h: headH });
    top += headH;
    for (let k = 0; k < g.count; k += per) {
      rows.push({ kind: 'cells', group, top, h: cellH, first: g.first_index + k, n: Math.min(per, g.count - k) });
      top += cellH;
    }
  });
  return { rows, height: top, groupTops };
}

/** Index of the row containing offset `y` (clamped to the ends); -1 when there are no rows. */
export function rowAt(layout: Layout, y: number): number {
  const { rows } = layout;
  if (rows.length === 0) return -1;
  let lo = 0;
  let hi = rows.length - 1;
  while (lo < hi) {
    const mid = (lo + hi + 1) >> 1;
    if (rows[mid].top <= y) lo = mid;
    else hi = mid - 1;
  }
  return lo;
}

/** Rows [start, end) overlapping the viewport, widened by `overscan` rows each side. */
export function visibleRows(layout: Layout, scrollTop: number, viewH: number, overscan = 2): [number, number] {
  if (layout.rows.length === 0) return [0, 0];
  const first = rowAt(layout, scrollTop);
  const last = rowAt(layout, scrollTop + viewH);
  return [Math.max(0, first - overscan), Math.min(layout.rows.length, last + 1 + overscan)];
}

/** The group shown at offset `y`, or -1 when empty. */
export function groupAt(layout: Layout, y: number): number {
  const i = rowAt(layout, y);
  return i < 0 ? -1 : layout.rows[i].group;
}

/** Scroll offset of a group's heading; the bottom of the list when the group does not exist. */
export function groupTop(layout: Layout, group: number): number {
  return layout.groupTops[group] ?? layout.height;
}

/** Where to scroll so the photo at this position shows: its row centred in the view, within the list. Nothing (stay put) when its row is
 *  already fully in view, or when the list has no such position. */
export function revealOffset(layout: Layout, index: number, scrollTop: number, viewH: number): number | null {
  const row = layout.rows.find((r) => r.kind === 'cells' && index >= r.first && index < r.first + r.n);
  if (!row) return null;
  if (row.top >= scrollTop && row.top + row.h <= scrollTop + viewH) return null;
  return Math.max(0, Math.min(row.top + row.h / 2 - viewH / 2, layout.height - viewH));
}

/** True when two group lists are identical, so a live refresh can skip a pointless grid reload. */
export function sameGroups(a: DayGroup[], b: DayGroup[]): boolean {
  return a.length === b.length && a.every((g, i) => g.day === b[i].day && g.count === b[i].count && g.first_index === b[i].first_index);
}
