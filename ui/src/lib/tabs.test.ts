import { describe, expect, it } from 'vitest';
import { visibleTabs } from './tabs';

describe('visibleTabs', () => {
  it('shows only tabs that have something in them (the map is a view inside Browse, not a tab)', () => {
    expect(visibleTabs({ create: false, share: false }).map((t) => t.id)).toEqual(['browse', 'tools', 'faces', 'dupes', 'ask', 'settings']);
  });
  it('adds Create and Share between Duplicates and Settings when they exist', () => {
    expect(visibleTabs({ create: true, share: true }).map((t) => t.id)).toEqual(['browse', 'tools', 'faces', 'dupes', 'create', 'share', 'ask', 'settings']);
    expect(visibleTabs({ create: false, share: true }).map((t) => t.id)).toEqual(['browse', 'tools', 'faces', 'dupes', 'share', 'ask', 'settings']);
  });
  it('always has Tools right after Browse, without a count', () => {
    expect(visibleTabs({ create: false, share: false })[1]).toEqual({ id: 'tools', label: 'Tools' });
  });
  it('has People and Duplicates only while those analyses are on', () => {
    expect(visibleTabs({ create: false, share: false, people: false }).map((t) => t.id)).toEqual(['browse', 'tools', 'dupes', 'ask', 'settings']);
    expect(visibleTabs({ create: false, share: false, dupes: false }).map((t) => t.id)).toEqual(['browse', 'tools', 'faces', 'ask', 'settings']);
  });
  it('labels are the words a person would use', () => {
    expect(visibleTabs({ create: true, share: true }).map((t) => t.label)).toEqual(['Browse', 'Tools', 'People', 'Duplicates', 'Create', 'Share', 'Ask Pixie', 'Settings']);
  });
});
