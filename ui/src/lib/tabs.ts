export type TabId = 'browse' | 'tools' | 'faces' | 'dupes' | 'create' | 'share' | 'settings';
export interface TabInfo { id: TabId; label: string }

/** Create and Share are empty until their first feature ships; an empty tab is not shown. Tools is always there, and says
 *  how many are picked (`picked`) once there are some. */
export function visibleTabs(features: { create: boolean; share: boolean; picked?: number; people?: boolean; dupes?: boolean }): TabInfo[] {
  const all: (TabInfo & { on: boolean })[] = [
    { id: 'browse', label: 'Browse', on: true },
    { id: 'tools', label: (features.picked ?? 0) > 0 ? `Tools (${(features.picked ?? 0).toLocaleString()})` : 'Tools', on: true },
    { id: 'faces', label: 'People', on: features.people ?? true },
    { id: 'dupes', label: 'Duplicates', on: features.dupes ?? true },
    { id: 'create', label: 'Create', on: features.create },
    { id: 'share', label: 'Share', on: features.share },
    { id: 'settings', label: 'Settings', on: true },
  ];
  return all.filter((t) => t.on).map(({ id, label }) => ({ id, label }));
}
