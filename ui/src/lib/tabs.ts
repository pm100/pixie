export type TabId = 'browse' | 'map' | 'faces' | 'dupes' | 'create' | 'share' | 'settings';
export interface TabInfo { id: TabId; label: string }

/** Create and Share are empty until their first feature ships; an empty tab is not shown. */
export function visibleTabs(features: { create: boolean; share: boolean }): TabInfo[] {
  const all: (TabInfo & { on: boolean })[] = [
    { id: 'browse', label: 'Browse', on: true },
    { id: 'map', label: 'Map', on: true },
    { id: 'faces', label: 'People', on: true },
    { id: 'dupes', label: 'Duplicates', on: true },
    { id: 'create', label: 'Create', on: features.create },
    { id: 'share', label: 'Share', on: features.share },
    { id: 'settings', label: 'Settings', on: true },
  ];
  return all.filter((t) => t.on).map(({ id, label }) => ({ id, label }));
}

export type PickAction = 'enter-map' | 'show-map' | 'reset-map' | 'leave-map' | 'show';

/**
 * What clicking a tab does. `mode` is whether the library screen is in map mode; `current` is the tab now shown.
 * Clicking Map while looking at the map resets it to the whole world; clicking it from Settings only shows it.
 */
export function pickAction(id: TabId, mode: 'library' | 'map', current: TabId): PickAction {
  if (id === 'map') return mode !== 'map' ? 'enter-map' : current !== 'map' ? 'show-map' : 'reset-map';
  if (id === 'browse' && mode === 'map') return 'leave-map';
  return 'show';
}
