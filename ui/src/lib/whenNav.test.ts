import { describe, expect, it } from 'vitest';
import type { DayGroup } from './api';
import { buildDateTree } from './tree';
import { dayBadge, maxMonthCount, monthCounts, yearEntries } from './whenNav';

const g = (day: string | null, count: number, first_index = 0): DayGroup => ({ day, count, first_index });

describe('monthCounts', () => {
  it('has twelve slots with counts in the right months and zeros elsewhere', () => {
    const [y] = buildDateTree([g('2024-06-03', 2), g('2024-06-01', 3), g('2024-01-09', 4)]);
    expect(monthCounts(y)).toEqual([4, 0, 0, 0, 0, 5, 0, 0, 0, 0, 0, 0]);
  });
});

describe('maxMonthCount', () => {
  it('is the busiest single month, never below 1', () => {
    const t = buildDateTree([g('2024-06-03', 2), g('2023-05-01', 9)]);
    expect(maxMonthCount(t)).toBe(9);
    expect(maxMonthCount([])).toBe(1);
  });
});

describe('yearEntries', () => {
  const days = (ys: number[]) => ys.map((y, i) => g(`${y}-06-01`, 1, i));
  const names = (e: ReturnType<typeof yearEntries>) => e.map((x) => (x.kind === 'year' ? x.node.label : 'undated'));
  it('lists every year as its own row, newest first, with Undated last', () => {
    const roots = buildDateTree([...days([2026, 2025, 2023, 2019, 2015, 2009]), g(null, 3, 6)]);
    expect(names(yearEntries(roots))).toEqual(['2026', '2025', '2023', '2019', '2015', '2009', 'undated']);
  });
  it('works oldest first', () => {
    const roots = buildDateTree(days([2009, 2015, 2019, 2026]));
    expect(names(yearEntries(roots))).toEqual(['2009', '2015', '2019', '2026']);
  });
  it('handles no groups and only undated', () => {
    expect(yearEntries([])).toEqual([]);
    expect(yearEntries(buildDateTree([g(null, 4)])).map((x) => x.kind)).toEqual(['undated']);
  });
});

describe('dayBadge', () => {
  it('splits a day into number, weekday and weekend flag', () => {
    expect(dayBadge('2025-06-14')).toEqual({ dom: 14, weekday: 'Sat', weekend: true });
    expect(dayBadge('2025-06-04')).toEqual({ dom: 4, weekday: 'Wed', weekend: false });
    expect(dayBadge('2025-06-15').weekend).toBe(true);
  });
});
