import { describe, expect, it } from 'vitest';
import { visibleTabs } from './tabs';

describe('visibleTabs', () => {
  it('shows only tabs that have something in them (the map is a view inside Browse, not a tab)', () => {
    expect(visibleTabs({ create: false, share: false }).map((t) => t.id)).toEqual(['browse', 'faces', 'dupes', 'settings']);
  });
  it('adds Create and Share between Duplicates and Settings when they exist', () => {
    expect(visibleTabs({ create: true, share: true }).map((t) => t.id)).toEqual(['browse', 'faces', 'dupes', 'create', 'share', 'settings']);
    expect(visibleTabs({ create: false, share: true }).map((t) => t.id)).toEqual(['browse', 'faces', 'dupes', 'share', 'settings']);
  });
  it('adds Selected, with its count, right after Browse while something is picked', () => {
    expect(visibleTabs({ create: false, share: false, selected: 0 }).map((t) => t.id)).toEqual(['browse', 'faces', 'dupes', 'settings']);
    const tabs = visibleTabs({ create: false, share: false, selected: 12 });
    expect(tabs.map((t) => t.id)).toEqual(['browse', 'selected', 'faces', 'dupes', 'settings']);
    expect(tabs[1].label).toBe('Selected (12)');
  });
  it('labels are the words a person would use', () => {
    expect(visibleTabs({ create: true, share: true }).map((t) => t.label)).toEqual(['Browse', 'People', 'Duplicates', 'Create', 'Share', 'Settings']);
  });
});
