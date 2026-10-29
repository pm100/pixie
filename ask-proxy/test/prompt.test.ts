import { describe, expect, it } from 'vitest';
import { FIXED_RULES, RULES, systemParts } from '../src/prompt';
import { CONTEXT } from './helpers';

describe('systemParts', () => {
  it('puts rules and schema in the stable part and per-user state in the volatile part', () => {
    const { stable, volatile } = systemParts(CONTEXT);
    expect(stable).toContain(RULES);
    expect(stable).toContain('network_rescan_minutes');
    expect(stable).not.toContain('currentLabel');
    expect(volatile).toContain('Platform: windows');
    expect(volatile).toContain('Language: en-US');
    expect(volatile).toContain('Today is 2026-10-28');
    expect(volatile).toContain('on the browse tab');
    expect(volatile).toContain('analysis_faces (Find faces): On');
  });

  it('keeps the stable part identical when only current values, language or tab change', () => {
    const changed = {
      ...CONTEXT,
      tab: 'settings',
      language: 'fr',
      settings: CONTEXT.settings.map((s) => ({ ...s, current: 99, currentLabel: 'changed' })),
    };
    expect(systemParts(changed).stable).toBe(systemParts(CONTEXT).stable);
    expect(systemParts(changed).volatile).not.toBe(systemParts(CONTEXT).volatile);
  });

  it('tells the model to use the tools', () => {
    expect(RULES).toContain('propose_settings_changes');
    expect(RULES).toContain('submit_feedback');
    expect(RULES).toContain('go_to');
  });

  it('starts the stable prompt with the fixed rules, before the guide, schema or anything else', () => {
    const { stable } = systemParts(CONTEXT);
    expect(stable.startsWith(FIXED_RULES)).toBe(true);
    expect(stable.indexOf('SETTINGS SCHEMA')).toBeGreaterThan(FIXED_RULES.length);
    expect(stable.indexOf('APP GUIDE')).toBeGreaterThan(FIXED_RULES.length);
  });

  it('states the five fixed rules, in order', () => {
    const rules = FIXED_RULES.split(String.fromCharCode(10)).filter((l) => /^[1-5]\. /.test(l));
    expect(rules).toHaveLength(5);
    expect(rules[0]).toContain('must not be ignored');
    expect(rules[1]).toContain('never delete anything');
    expect(rules[2]).toContain('do not create images');
    expect(rules[3]).toContain('only help with using Pixie');
    expect(rules[4]).toContain('never upload, send or ask for an image');
  });

  it('keeps the volatile text, which holds what the user controls, out of the rules', () => {
    expect(systemParts(CONTEXT).volatile).not.toContain('RULES');
  });

  it('explains how to pick and what the star and the Picked button mean', () => {
    const { stable } = systemParts(CONTEXT);
    expect(stable).toContain('top-left corner');
    expect(stable).toContain('star');
    expect(stable).toContain('Picked button');
  });

  it('explains every way of working on several photos, and the steps for print and email', () => {
    const { stable } = systemParts(CONTEXT);
    const section = stable.slice(stable.indexOf('WORKING ON SEVERAL PHOTOS'));
    expect(section.length).toBeGreaterThan(100);
    for (const word of ['Ctrl-click', 'Shift-click', 'top-left corner', 'Working on', 'your tags', 'no Select all', 'photos per page', 'Print...', 'Email']) {
      expect(section).toContain(word);
    }
  });

  it('tells the model to give the whole sequence of steps, starting with how to choose the photos', () => {
    expect(RULES).toContain('whole sequence');
    expect(RULES).toContain('how to choose the photos');
  });

  it('says how a tag is taken off photos, so the model does not invent a button', () => {
    const { stable } = systemParts(CONTEXT);
    expect(stable).toContain('small x on the tag');
  });

  it('says videos are sent as they are, never as a smaller copy', () => {
    const { stable } = systemParts(CONTEXT);
    expect(stable).toContain('Videos are always sent exactly as they are');
    expect(RULES).toContain('Never say Pixie offers a smaller copy of a video');
  });

  it('says in the guide that names from the library are never sent', () => {
    const { stable } = systemParts(CONTEXT);
    const section = stable.slice(stable.indexOf('PRIVACY'));
    expect(section).toContain('never sent');
    expect(section).toContain('never sends pictures');
  });

  it('tells the model that a collage is a feature Pixie does not have yet, not an image it must not create', () => {
    for (const word of ['collage', 'does not have', 'feedback']) expect(RULES).toContain(word);
    expect(RULES).not.toMatch(/I can't create images/i);
  });
});
