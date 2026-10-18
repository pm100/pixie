import { weekday } from './format';
import type { TreeNode } from './tree';

export type YearEntry =
  | { kind: 'year'; node: TreeNode }
  | { kind: 'decade'; key: string; label: string; count: number; firstGroup: number; lastGroup: number; years: TreeNode[] }
  | { kind: 'undated'; node: TreeNode };

/** Items per month of a year node (index 0 = January). Month nodes have keys `m:YYYY-MM`. */
export function monthCounts(year: TreeNode): number[] {
  const out = new Array<number>(12).fill(0);
  for (const m of year.children) out[Number(m.key.slice(-2)) - 1] = m.count;
  return out;
}

/** The busiest single month across the given years, so the bars of different years are comparable. */
export function maxMonthCount(years: TreeNode[]): number {
  let max = 1;
  for (const y of years) if (y.key !== 'undated') for (const c of monthCounts(y)) max = Math.max(max, c);
  return max;
}

/**
 * Rows for the "When" navigation: the last six years as years, older years folded into decades, Undated last.
 * The order of `roots` (newest or oldest first) is kept.
 */
export function yearEntries(roots: TreeNode[], thisYear: number): YearEntry[] {
  const out: YearEntry[] = [];
  let undated: TreeNode | undefined;
  for (const n of roots) {
    if (n.key === 'undated') { undated = n; continue; }
    const year = Number(n.label);
    if (year >= thisYear - 5) { out.push({ kind: 'year', node: n }); continue; }
    const decade = Math.floor(year / 10) * 10;
    const key = `decade:${decade}`;
    const last = out[out.length - 1];
    if (last && last.kind === 'decade' && last.key === key) {
      last.years.push(n);
      last.count += n.count;
      last.firstGroup = Math.min(last.firstGroup, n.firstGroup);
      last.lastGroup = Math.max(last.lastGroup, n.lastGroup);
    } else {
      out.push({ kind: 'decade', key, label: `${decade}s`, count: n.count, firstGroup: n.firstGroup, lastGroup: n.lastGroup, years: [n] });
    }
  }
  if (undated) out.push({ kind: 'undated', node: undated });
  return out;
}

/** The pieces of a day row: day of month, short weekday and whether it falls on a weekend. */
export function dayBadge(day: string): { dom: number; weekday: string; weekend: boolean } {
  const w = weekday(day);
  return { dom: Number(day.slice(8, 10)), weekday: w, weekend: w === 'Sat' || w === 'Sun' };
}
