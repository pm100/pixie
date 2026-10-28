export type TabId = 'browse' | 'tools' | 'faces' | 'dupes' | 'create' | 'share' | 'settings';
export interface TabInfo { id: TabId; label: string }

/** Create and Share are empty until their first feature ships; an empty tab is not shown. Tools is always there. */
export function visibleTabs(features: { create: boolean; share: boolean; people?: boolean; dupes?: boolean }): TabInfo[] {
  const all: (TabInfo & { on: boolean })[] = [
    { id: 'browse', label: 'Browse', on: true },
    { id: 'tools', label: 'Tools', on: true },
    { id: 'faces', label: 'People', on: features.people ?? true },
    { id: 'dupes', label: 'Duplicates', on: features.dupes ?? true },
    { id: 'create', label: 'Create', on: features.create },
    { id: 'share', label: 'Share', on: features.share },
    { id: 'settings', label: 'Settings', on: true },
  ];
  return all.filter((t) => t.on).map(({ id, label }) => ({ id, label }));
}
