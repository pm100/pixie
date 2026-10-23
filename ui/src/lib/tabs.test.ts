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
  it('labels are the words a person would use', () => {
    expect(visibleTabs({ create: true, share: true }).map((t) => t.label)).toEqual(['Browse', 'People', 'Duplicates', 'Create', 'Share', 'Settings']);
  });
});
