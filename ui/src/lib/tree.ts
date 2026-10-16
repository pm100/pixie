import type { DayGroup } from './api';
import { monthName, weekday } from './format';

export interface TreeNode {
  key: string;
  label: string;
  count: number;
  /** Range of day-group indexes this node covers, inclusive. */
  firstGroup: number;
  lastGroup: number;
  children: TreeNode[];
}

const node = (key: string, label: string, group: number): TreeNode =>
  ({ key, label, count: 0, firstGroup: group, lastGroup: group, children: [] });

/** Year > Month > Day nodes in the order given, then Undated. Groups must already be in listing order. */
export function buildDateTree(groups: DayGroup[]): TreeNode[] {
  const roots: TreeNode[] = [];
  let year: TreeNode | undefined;
  let month: TreeNode | undefined;
  let undated: TreeNode | undefined;

  const add = (n: TreeNode, g: DayGroup, group: number) => {
    n.count += g.count;
    n.lastGroup = group;
  };

  groups.forEach((g, i) => {
    if (g.day === null) {
      undated ??= node('undated', 'Undated', i);
      add(undated, g, i);
      return;
    }
    const [y, m, d] = g.day.split('-');
    if (!year || year.key !== `y:${y}`) {
      year = node(`y:${y}`, y, i);
      roots.push(year);
      month = undefined;
    }
    if (!month || month.key !== `m:${y}-${m}`) {
      month = node(`m:${y}-${m}`, monthName(Number(m)), i);
      year.children.push(month);
    }
    const day = node(`d:${g.day}`, `${weekday(g.day)} ${Number(d)}`, i);
    month.children.push(day);
    add(day, g, i);
    add(month, g, i);
    add(year, g, i);
  });

  if (undated) roots.push(undated);
  return roots;
}

/** Keys of every node (year, month, day) whose range contains `group`. */
export function activeKeys(tree: TreeNode[], group: number): Set<string> {
  const keys = new Set<string>();
  const walk = (nodes: TreeNode[]) => {
    for (const n of nodes) {
      if (group >= n.firstGroup && group <= n.lastGroup) {
        keys.add(n.key);
        walk(n.children);
      }
    }
  };
  walk(tree);
  return keys;
}
