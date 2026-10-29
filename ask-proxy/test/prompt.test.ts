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

  it('tells the model it can find, pick and tag photos, and how', () => {
    expect(RULES).toContain('find_photos');
    expect(RULES).not.toContain('Do not claim to find');
    expect(RULES).not.toContain('changes settings only');
    for (const word of ['pink flowers', 'tags', 'Never invent a tag', 'then']) expect(RULES).toContain(word);
  });

  it('tells the model in the guide that finding by content is approximate and skips videos', () => {
    const { stable } = systemParts(CONTEXT);
    const section = stable.slice(stable.indexOf('FINDING PHOTOS'));
    expect(section).toContain('approximate');
    expect(section).toContain('videos');
    expect(section).toContain('analysed');
  });

  it('says how a tag is taken off photos, so the model does not invent a button', () => {
    const { stable } = systemParts(CONTEXT);
    expect(stable).toContain('small x on the tag');
  });

  it('explains finding people in photos and two things in one picture', () => {
    const { stable } = systemParts(CONTEXT);
    expect(stable).toContain('with people in them (any person) uses the faces');
    expect(stable).toContain('each is searched for on its own');
    expect(RULES).toContain('with_people');
  });

  describe('with the library', () => {
    const library = {
      photos: 1000, picked: 2,
      tags: [{ name: 'holiday', count: 12 }],
      people: [{ name: 'Rose', count: 40 }],
      trips: [{ name: 'Italy', from: '2019-06-01', to: '2019-06-14', nights: 13, photos: 340, videos: 6, where: 'Rome' }],
    };

    it('lists it in the per-user part, after the rules, and never in the cached part', () => {
      const { stable, volatile } = systemParts({ ...CONTEXT, library });
      expect(volatile).toContain('LIBRARY');
      expect(volatile).toContain('Rose (40)');
      expect(volatile).toContain('holiday (12)');
      expect(volatile).toContain('2019-06-01 to 2019-06-14');
      expect(volatile).toContain('Italy');
      expect(volatile).toContain('Rome');
      expect(stable).not.toContain('Rose');
      expect(systemParts({ ...CONTEXT, library }).stable).toBe(systemParts(CONTEXT).stable);
    });

    it('says plainly when the user has not allowed it, so the model does not pretend to know names', () => {
      expect(systemParts(CONTEXT).volatile).toContain('has not allowed');
      expect(systemParts({ ...CONTEXT, library }).volatile).not.toContain('has not allowed');
    });

    it('tells the model how to use the names, and to check with the user when a word could be a name or a thing', () => {
      for (const word of ['LIBRARY', 'exactly as listed', 'name or']) expect(RULES).toContain(word);
    });
  });

  it('tells the model when and how to ask the app, and not to use it for what it can already see', () => {
    for (const word of ['lookup', 'at most three', 'LIBRARY already']) expect(RULES).toContain(word);
  });

  it('describes in the guide what is sent, including the names only when sharing is on', () => {
    const { stable } = systemParts(CONTEXT);
    const section = stable.slice(stable.indexOf('PRIVACY'));
    expect(section).toContain('only if the user turned on sharing');
    expect(section).toContain('never sends pictures');
  });

  it('tells the model that a castle or a beach is something in the picture, and that near or next to means the same picture', () => {
    for (const word of ['castle', 'near', 'same picture', 'named place']) expect(RULES).toContain(word);
    expect(RULES).not.toContain('You cannot find by place or');
  });

  describe('the current results', () => {
    const results = { count: 47, of: 'photos that look like “rose” and “flowers”' };

    it('tells the model what them and those mean, in the per-user part', () => {
      const { volatile, stable } = systemParts({ ...CONTEXT, results });
      expect(volatile).toContain('RESULTS');
      expect(volatile).toContain('47');
      expect(volatile).toContain('photos that look like “rose” and “flowers”');
      expect(stable).not.toContain('47 photos');
      expect(systemParts({ ...CONTEXT, results }).stable).toBe(systemParts(CONTEXT).stable);
    });

    it('says there are none, so the model does not invent a set', () => {
      expect(systemParts(CONTEXT).volatile).toContain('RESULTS: no search has found anything yet');
    });
  });

  it('tells the model how to follow up on the results, and to read what happened from the notes', () => {
    for (const word of ['open_tool', 'them', 'those', 'RESULTS', 'square brackets']) expect(RULES).toContain(word);
  });

  it('tells the model that a collage is a feature Pixie does not have yet, not an image it must not create', () => {
    for (const word of ['collage', 'does not have', 'feedback']) expect(RULES).toContain(word);
    expect(RULES).not.toMatch(/I can't create images/i);
  });
});
