import { describe, expect, it } from 'vitest';
import { GO_TO_TARGETS, goToLabel, placeFor } from './askGoTo';
import { toolById } from './tools';

describe('askGoTo', () => {
  it('names places the proxy accepts: lower-case words joined by hyphens, no repeats', () => {
    for (const t of GO_TO_TARGETS) expect(t).toMatch(/^[a-z]+(-[a-z]+)*$/);
    expect(new Set(GO_TO_TARGETS).size).toBe(GO_TO_TARGETS.length);
    expect(GO_TO_TARGETS.length).toBeLessThanOrEqual(40);
  });

  it('takes every target to a place and gives it a label', () => {
    for (const t of GO_TO_TARGETS) {
      expect(placeFor(t), t).not.toBeNull();
      expect(goToLabel(t).length).toBeGreaterThan(0);
    }
  });

  it('only offers tools that are built, in the tab they are in', () => {
    for (const t of GO_TO_TARGETS) {
      const place = placeFor(t)!;
      if (place.tool === undefined) continue;
      const tool = toolById(place.tool);
      expect(tool?.built, t).toBe(true);
      expect(tool?.group, t).toBe(place.tab);
    }
  });

  it('knows the tab, and the tool or the page where there is one', () => {
    expect(placeFor('browse')).toEqual({ tab: 'browse' });
    expect(placeFor('people')).toEqual({ tab: 'faces' });
    expect(placeFor('send-email')).toEqual({ tab: 'send', tool: 'email' });
    expect(placeFor('cleanup-missing-dates')).toEqual({ tab: 'cleanup', tool: 'missing-dates' });
    expect(placeFor('transfer-sync')).toEqual({ tab: 'transfer', tool: 'sync' });
    expect(placeFor('settings-trips')).toEqual({ tab: 'settings', settings: 'trips' });
    expect(placeFor('settings-ask')).toEqual({ tab: 'settings', settings: 'ask' });
  });

  it('turns away anything else, whatever the model made up', () => {
    for (const bad of ['', 'settings', 'delete', 'tools-email', 'send-nope', 'create-email', 'send-', '-email', '../x', 'Browse', 'duplicates']) expect(placeFor(bad), bad).toBeNull();
  });

  it('labels places in words a person would use', () => {
    expect(goToLabel('send-email')).toBe('Open Email in Send');
    expect(goToLabel('create-collage')).toBe('Open Collage in Create');
    expect(goToLabel('cleanup-duplicates')).toBe('Open Duplicates in Clean up');
    expect(goToLabel('settings-analysis')).toBe('Open Picture analysis in Settings');
    expect(goToLabel('settings-ask')).toBe('Open Ask Pixie in Settings');
    expect(goToLabel('people')).toBe('Open People');
  });
});
