import { describe, expect, it } from 'vitest';
import { visibleTabs } from './tabs';

describe('visibleTabs', () => {
  it('has Browse, People, the four tool tabs and Settings, in that order', () => {
    expect(visibleTabs({}).map((t) => t.id)).toEqual(['browse', 'faces', 'send', 'create', 'organize', 'cleanup', 'settings']);
  });
  it('leaves People out while face analysis is off', () => {
    expect(visibleTabs({ people: false }).map((t) => t.id)).toEqual(['browse', 'send', 'create', 'organize', 'cleanup', 'settings']);
  });
  it('labels are the words a person would use', () => {
    expect(visibleTabs({}).map((t) => t.label)).toEqual(['Browse', 'People', 'Send', 'Create', 'Organize', 'Clean up', 'Settings']);
  });
});
