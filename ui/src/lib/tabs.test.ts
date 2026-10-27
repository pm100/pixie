import { describe, expect, it } from 'vitest';
import { visibleTabs } from './tabs';

describe('visibleTabs', () => {
  it('shows only tabs that have something in them (the map is a view inside Browse, not a tab)', () => {
    expect(visibleTabs({ create: false, share: false }).map((t) => t.id)).toEqual(['browse', 'selected', 'faces', 'dupes', 'settings']);
  });
  it('adds Create and Share between Duplicates and Settings when they exist', () => {
    expect(visibleTabs({ create: true, share: true }).map((t) => t.id)).toEqual(['browse', 'selected', 'faces', 'dupes', 'create', 'share', 'settings']);
    expect(visibleTabs({ create: false, share: true }).map((t) => t.id)).toEqual(['browse', 'selected', 'faces', 'dupes', 'share', 'settings']);
  });
  it('always has Selected right after Browse, with its count once something is picked', () => {
    expect(visibleTabs({ create: false, share: false, selected: 0 })[1]).toEqual({ id: 'selected', label: 'Selected' });
    expect(visibleTabs({ create: false, share: false, selected: 12 })[1]).toEqual({ id: 'selected', label: 'Selected (12)' });
  });
  it('has People and Duplicates only while those analyses are on', () => {
    expect(visibleTabs({ create: false, share: false, people: false }).map((t) => t.id)).toEqual(['browse', 'selected', 'dupes', 'settings']);
    expect(visibleTabs({ create: false, share: false, dupes: false }).map((t) => t.id)).toEqual(['browse', 'selected', 'faces', 'settings']);
  });
  it('labels are the words a person would use', () => {
    expect(visibleTabs({ create: true, share: true }).map((t) => t.label)).toEqual(['Browse', 'Selected', 'People', 'Duplicates', 'Create', 'Share', 'Settings']);
  });
});
