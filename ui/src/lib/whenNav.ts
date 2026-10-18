import { weekday } from './format';
import type { TreeNode } from './tree';

export type YearEntry = { kind: 'year'; node: TreeNode } | { kind: 'undated'; node: TreeNode };

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

/** Rows for the "When" navigation: every year, in the order of `roots` (newest or oldest first), Undated last. */
export function yearEntries(roots: TreeNode[]): YearEntry[] {
  const out: YearEntry[] = roots.filter((n) => n.key !== 'undated').map((node) => ({ kind: 'year', node }));
  const undated = roots.find((n) => n.key === 'undated');
  if (undated) out.push({ kind: 'undated', node: undated });
  return out;
}

/** The pieces of a day row: day of month, short weekday and whether it falls on a weekend. */
export function dayBadge(day: string): { dom: number; weekday: string; weekend: boolean } {
  const w = weekday(day);
  return { dom: Number(day.slice(8, 10)), weekday: w, weekend: w === 'Sat' || w === 'Sun' };
}
