import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { GO_TO_TARGETS, goToLabel, placeFor } from './askGoTo';

describe('askGoTo', () => {
  it('lists exactly the places the proxy lets the model offer', () => {
    const source = readFileSync(new URL('../../../ask-proxy/src/tools.ts', import.meta.url), 'utf8');
    const block = /GO_TO_TARGETS = \[([^\]]*)\]/.exec(source)![1];
    const proxy = [...block.matchAll(/'([a-z-]+)'/g)].map((m) => m[1]);
    expect([...GO_TO_TARGETS]).toEqual(proxy);
  });

  it('takes every target to a place and gives it a label', () => {
    for (const t of GO_TO_TARGETS) {
      expect(placeFor(t)).not.toBeNull();
      expect(goToLabel(t).length).toBeGreaterThan(0);
    }
  });

  it('knows the tab, and the page where there is one', () => {
    expect(placeFor('browse')).toEqual({ tab: 'browse' });
    expect(placeFor('people')).toEqual({ tab: 'faces' });
    expect(placeFor('tools-email')).toEqual({ tab: 'tools', tools: 'email' });
    expect(placeFor('settings-trips')).toEqual({ tab: 'settings', settings: 'trips' });
  });

  it('turns away anything else, whatever the model made up', () => {
    for (const bad of ['', 'settings', 'delete', 'tools-delete', '../x', 'Browse']) expect(placeFor(bad)).toBeNull();
  });

  it('labels places in words a person would use', () => {
    expect(goToLabel('tools-email')).toBe('Open Email in Tools');
    expect(goToLabel('settings-analysis')).toBe('Open Picture analysis in Settings');
    expect(goToLabel('duplicates')).toBe('Open Duplicates');
  });
});
