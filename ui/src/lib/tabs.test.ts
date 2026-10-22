import { describe, expect, it } from 'vitest';
import { pickAction, visibleTabs } from './tabs';

describe('visibleTabs', () => {
  it('shows only tabs that have something in them', () => {
    expect(visibleTabs({ create: false, share: false }).map((t) => t.id)).toEqual(['browse', 'map', 'faces', 'dupes', 'settings']);
  });
  it('adds Create and Share between Duplicates and Settings when they exist', () => {
    expect(visibleTabs({ create: true, share: true }).map((t) => t.id)).toEqual(['browse', 'map', 'faces', 'dupes', 'create', 'share', 'settings']);
    expect(visibleTabs({ create: false, share: true }).map((t) => t.id)).toEqual(['browse', 'map', 'faces', 'dupes', 'share', 'settings']);
  });
  it('labels are the words a person would use', () => {
    expect(visibleTabs({ create: true, share: true }).map((t) => t.label)).toEqual(['Browse', 'Map', 'People', 'Duplicates', 'Create', 'Share', 'Settings']);
  });
});

describe('pickAction', () => {
  it('Map: enters the map, shows it again from another tab, or resets it when already looking at it', () => {
    expect(pickAction('map', 'library', 'browse')).toBe('enter-map');
    expect(pickAction('map', 'map', 'settings')).toBe('show-map');
    expect(pickAction('map', 'map', 'map')).toBe('reset-map');
  });
  it('Browse: leaves the map when it was in map mode, otherwise just shows Browse', () => {
    expect(pickAction('browse', 'map', 'map')).toBe('leave-map');
    expect(pickAction('browse', 'map', 'settings')).toBe('leave-map');
    expect(pickAction('browse', 'library', 'settings')).toBe('show');
  });
  it('other tabs are just shown', () => {
    expect(pickAction('settings', 'map', 'map')).toBe('show');
    expect(pickAction('settings', 'library', 'browse')).toBe('show');
  });
});
