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
    expect(volatile).toContain('analysis_faces (People): On');
  });

  it('tells the model what the user has picked and chosen, in the part that changes, and nothing about which photos', () => {
    const { volatile, stable } = systemParts(CONTEXT);
    expect(volatile).toContain('Photos picked: 12.');
    expect(volatile).toContain('A photo is chosen: yes.');
    expect(volatile).toContain('working on: picked');
    expect(stable).not.toContain('Photos picked');
    const tagged = systemParts({ ...CONTEXT, photos: { picked: 0, chosen: false, workingOn: 'tag' } }).volatile;
    expect(tagged).toContain("working on: one of the user's tags");
    expect(systemParts({ ...CONTEXT, photos: undefined }).volatile).not.toContain('Photos picked');
  });

  it('puts the note the app wrote about the photos at the end of the state, and leaves it out when there is none', () => {
    const withHint = systemParts({ ...CONTEXT, photos: { picked: 89, chosen: false, workingOn: 'picked', hint: 'SET WORKING ON TO CURRENT FIRST' } }).volatile;
    expect(withHint).toContain('About the photos, for this answer: SET WORKING ON TO CURRENT FIRST');
    expect(withHint.indexOf('About the photos')).toBeLessThan(withHint.indexOf('Current values'));
    expect(systemParts(CONTEXT).volatile).not.toContain('About the photos');
  });

  it('keeps the stable part identical when only current values, language or tab change', () => {
    const changed = {
      ...CONTEXT,
      photos: { picked: 0, chosen: false, workingOn: 'current' as const },
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

  it('puts the guide the app sent into the stable part, after the fixed rules', () => {
    const { stable, volatile } = systemParts(CONTEXT);
    expect(stable).toContain(`APP GUIDE
${CONTEXT.guide}`);
    expect(stable.indexOf(CONTEXT.guide)).toBeGreaterThan(FIXED_RULES.length);
    expect(volatile).not.toContain(CONTEXT.guide);
  });

  it('changes the stable part when the app version describes itself differently, so versions do not share an answer', () => {
    const other = { ...CONTEXT, guide: 'A DIFFERENT VERSION', places: ['browse'] };
    expect(systemParts(other).stable).not.toBe(systemParts(CONTEXT).stable);
    expect(systemParts(other).stable).toContain('A DIFFERENT VERSION');
    expect(systemParts(other).stable).not.toContain(CONTEXT.guide);
  });

  it('keeps the rules free of anything about one version of the app: tabs, tools and places come from the guide', () => {
    for (const word of ['Collage', 'Send', 'Organize', 'Clean up', 'send-email', 'Duplicates', 'Browse']) expect(RULES, word).not.toContain(word);
  });

  it('tells the model to give the whole sequence of steps, starting with how to choose the photos', () => {
    expect(RULES).toContain('whole sequence');
    expect(RULES).toContain('how to choose the photos');
  });

  it('tells the model that a feature the app lacks is passed on as feedback, not refused as an image it must not create', () => {
    for (const word of ['does not have', 'feedback', 'soon']) expect(RULES).toContain(word);
    expect(RULES).not.toMatch(/I can't create images/i);
  });
});
