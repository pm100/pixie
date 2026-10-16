import { describe, expect, it } from 'vitest';
import type { DayGroup } from './api';
import { activeKeys, buildDateTree } from './tree';

const g = (day: string | null, count: number, first_index: number): DayGroup => ({ day, count, first_index });

const groups = [g('2024-06-03', 2, 0), g('2024-06-01', 3, 2), g('2023-12-25', 1, 5), g(null, 4, 6)];

describe('buildDateTree', () => {
  it('nests year > month > day with counts and group ranges, Undated last', () => {
    const t = buildDateTree(groups);
    expect(t.map((n) => [n.key, n.label, n.count, n.firstGroup, n.lastGroup])).toEqual([
      ['y:2024', '2024', 5, 0, 1],
      ['y:2023', '2023', 1, 2, 2],
      ['undated', 'Undated', 4, 3, 3],
    ]);
    const june = t[0].children[0];
    expect([june.key, june.label, june.count, june.firstGroup, june.lastGroup]).toEqual(['m:2024-06', 'June', 5, 0, 1]);
    expect(june.children.map((d) => [d.key, d.label, d.count, d.firstGroup])).toEqual([
      ['d:2024-06-03', 'Mon 3', 2, 0],
      ['d:2024-06-01', 'Sat 1', 3, 1],
    ]);
    expect(t[1].children[0].children[0].label).toBe('Mon 25');
    expect(t[2].children).toEqual([]);
  });

  it('keeps the order it is given (oldest first)', () => {
    const t = buildDateTree([g('2023-12-25', 1, 0), g('2024-06-01', 3, 1), g('2024-06-03', 2, 4)]);
    expect(t.map((n) => n.label)).toEqual(['2023', '2024']);
    expect(t[1].children[0].children.map((d) => d.label)).toEqual(['Sat 1', 'Mon 3']);
  });

  it('is empty for no groups and shows only Undated when nothing is dated', () => {
    expect(buildDateTree([])).toEqual([]);
    const t = buildDateTree([g(null, 7, 0)]);
    expect(t.map((n) => [n.key, n.count])).toEqual([['undated', 7]]);
  });
});

describe('activeKeys', () => {
  it('lists the year, month and day containing a group', () => {
    const t = buildDateTree(groups);
    expect([...activeKeys(t, 1)].sort()).toEqual(['d:2024-06-01', 'm:2024-06', 'y:2024']);
    expect([...activeKeys(t, 3)]).toEqual(['undated']);
    expect(activeKeys(t, 99).size).toBe(0);
    expect(activeKeys(t, -1).size).toBe(0);
  });
});
