import { describe, expect, it } from 'vitest';
import { settingsSchema, type SettingValues } from './askSettings';
import { GO_TO_TARGETS } from './askGoTo';
import { GUIDE } from './askGuide';
import { ALL_ON } from './analysisOptions';
import { GROUPS, TOOLS } from './tools';
import { DEFAULT_TRIP_PREFS } from './trips';
import { buildRequest } from './ask';

const lines = GUIDE.split('\n');

describe('the guide Ask Pixie sends with every question', () => {
  it('fits what the proxy accepts, with room for the settings and the conversation', () => {
    const values: SettingValues = { viewerMode: 'window', emailLimitMb: 20, recentDays: 30, networkRescan: { on: true, minutes: 30 }, analysis: { ...ALL_ON }, trips: { ...DEFAULT_TRIP_PREFS } };
    expect(GUIDE.length).toBeLessThanOrEqual(20_000); // MAX_GUIDE_CHARS in ask-proxy/src/validate.ts
    const request = buildRequest([{ role: 'user', text: 'x'.repeat(600) }], { platform: 'windows', language: 'en-US', tab: 'browse', today: '2026-11-03' }, values);
    expect(request.length).toBeLessThan(65_536 / 2); // MAX_BODY_BYTES in ask-proxy/src/validate.ts
    expect(settingsSchema(values).length).toBeLessThanOrEqual(40);
  });

  it('is plain text: no backticks and no template placeholders', () => {
    expect(GUIDE).not.toContain('`');
    expect(GUIDE).not.toContain('${');
  });

  it('names every tab, and every tool by its name', () => {
    for (const g of GROUPS) expect(GUIDE, g.label).toContain(g.label);
    for (const t of TOOLS) expect(GUIDE, t.label).toContain(t.label.replace('…', ''));
  });

  it('says a tool is not built yet exactly when it is not', () => {
    const soon = lines.filter((l) => /\bsoon\b/.test(l) && /listed as soon/.test(l)).join('\n');
    for (const t of TOOLS) {
      const named = soon.includes(t.label.replace('…', ''));
      expect(named, `${t.label} ${t.built ? 'is built' : 'is not built'}`).toBe(!t.built);
    }
  });

  it('only sends the person to places that exist', () => {
    const named = [...GUIDE.matchAll(/go_to with ([a-z]+(?:-[a-z]+)*)/g)].map((m) => m[1]);
    expect(named.length).toBeGreaterThan(3);
    for (const place of named) expect(GO_TO_TARGETS as readonly string[], place).toContain(place);
  });

  it('explains how to pick and what the star and the Picked button mean', () => {
    for (const word of ['top-left corner', 'star', 'Picked button']) expect(GUIDE).toContain(word);
  });

  it('explains every way of working on several photos, and the steps for print and email', () => {
    const section = GUIDE.slice(GUIDE.indexOf('WORKING ON SEVERAL PHOTOS'));
    expect(section.length).toBeGreaterThan(100);
    for (const word of ['Ctrl-click', 'Shift-click', 'top-left corner', 'Working on', 'your tags', 'no Select all', 'photos per page', 'Print...', 'Email']) expect(section).toContain(word);
  });

  it('says how a tag is taken off photos, so the model does not invent a button', () => {
    expect(GUIDE).toContain('small x on the tag');
  });

  it('says videos are sent as they are, never as a smaller copy', () => {
    expect(GUIDE).toContain('Videos are always sent exactly as they are');
    expect(GUIDE).toContain('never say that Pixie offers a smaller copy of a video');
  });

  it('says the collage exists, so nobody is told it is missing', () => {
    expect(GUIDE).toMatch(/Collage in Create, and it exists/);
  });

  it('says how face finding works and where its switch is, under the name on screen', () => {
    expect(GUIDE).toContain('the People checkbox in Settings, Picture analysis');
    expect(GUIDE).toContain('There is no setting called "Find faces"');
    expect(GUIDE).toContain('works on the user');
  });

  it('says in the guide that names from the library are never sent', () => {
    const section = GUIDE.slice(GUIDE.indexOf('PRIVACY'));
    expect(section).toContain('never sent');
    expect(section).toContain('never sends pictures');
  });
});
