import { GROUPS, type ToolGroup } from './tools';

export type TabId = 'browse' | 'faces' | ToolGroup | 'settings';
export interface TabInfo { id: TabId; label: string }

/** The tabs in order: Browse, People, the four tool tabs and Settings. People is there only while face analysis is on. */
export function visibleTabs(features: { people?: boolean }): TabInfo[] {
  const all: (TabInfo & { on: boolean })[] = [
    { id: 'browse', label: 'Browse', on: true },
    { id: 'faces', label: 'People', on: features.people ?? true },
    ...GROUPS.map((g) => ({ id: g.id as TabId, label: g.label, on: true })),
    { id: 'settings', label: 'Settings', on: true },
  ];
  return all.filter((t) => t.on).map(({ id, label }) => ({ id, label }));
}
