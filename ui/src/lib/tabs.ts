export type TabId = 'browse' | 'selected' | 'faces' | 'dupes' | 'create' | 'share' | 'settings';
export interface TabInfo { id: TabId; label: string }

/** Create and Share are empty until their first feature ships; an empty tab is not shown. Selected is always there, and says
 *  how many are picked (`selected`) once there are some. */
export function visibleTabs(features: { create: boolean; share: boolean; selected?: number }): TabInfo[] {
  const all: (TabInfo & { on: boolean })[] = [
    { id: 'browse', label: 'Browse', on: true },
    { id: 'selected', label: (features.selected ?? 0) > 0 ? `Selected (${(features.selected ?? 0).toLocaleString()})` : 'Selected', on: true },
    { id: 'faces', label: 'People', on: true },
    { id: 'dupes', label: 'Duplicates', on: true },
    { id: 'create', label: 'Create', on: features.create },
    { id: 'share', label: 'Share', on: features.share },
    { id: 'settings', label: 'Settings', on: true },
  ];
  return all.filter((t) => t.on).map(({ id, label }) => ({ id, label }));
}
