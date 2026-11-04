import { describe, expect, it } from 'vitest';
import { isInstallId, MAX_GUIDE_CHARS, MAX_HINT_CHARS, MAX_PLACES, MAX_TURN_CHARS, MAX_TURNS, parseAsk } from '../src/validate';
import { askBody, CONTEXT, INSTALL_ID } from './helpers';

describe('isInstallId', () => {
  it('accepts a UUID and rejects everything else', () => {
    expect(isInstallId(INSTALL_ID)).toBe(true);
    for (const bad of [null, '', 'abc', `${INSTALL_ID}x`, '../../etc/passwd', 'g'.repeat(36)]) {
      expect(isInstallId(bad)).toBe(false);
    }
  });
});

describe('parseAsk', () => {
  it('accepts a well-formed body', () => {
    const parsed = parseAsk(askBody());
    expect(parsed.ok).toBe(true);
  });

  it('keeps the photo counts, and leaves out anything odd about them instead of failing the question', () => {
    const photos = { picked: 3, chosen: true, workingOn: 'tag' };
    const ok = parseAsk({ messages: askBody().messages, context: { ...CONTEXT, photos } });
    expect(ok.ok && ok.body.context.photos).toEqual(photos);
    for (const bad of [{ picked: -1, chosen: true, workingOn: 'picked' }, { picked: 1.5, chosen: true, workingOn: 'picked' }, { picked: 1, chosen: 'yes', workingOn: 'picked' }, { picked: 1, chosen: true, workingOn: 'everything' }, 'lots', { picked: 1e9, chosen: true, workingOn: 'picked' }]) {
      const r = parseAsk({ messages: askBody().messages, context: { ...CONTEXT, photos: bad } });
      expect(r.ok, JSON.stringify(bad)).toBe(true);
      expect(r.ok && r.body.context.photos, JSON.stringify(bad)).toBeUndefined();
    }
    const hinted = parseAsk({ messages: askBody().messages, context: { ...CONTEXT, photos: { ...photos, hint: '  Do this.  ' } } });
    expect(hinted.ok && hinted.body.context.photos?.hint).toBe('Do this.');
    for (const hint of ['', '   ', 5, 'x'.repeat(MAX_HINT_CHARS + 1)]) {
      const r = parseAsk({ messages: askBody().messages, context: { ...CONTEXT, photos: { ...photos, hint } } });
      expect(r.ok && r.body.context.photos?.hint, String(hint).slice(0, 10)).toBeUndefined();
      expect(r.ok && r.body.context.photos?.picked).toBe(3);
    }
    const old = parseAsk({ messages: askBody().messages, context: { ...CONTEXT, photos: undefined } });
    expect(old.ok && old.body.context.photos).toBeUndefined();
  });

  it('drops fields it does not know about', () => {
    const body = { ...askBody(), extra: 1, context: { ...CONTEXT, evil: 'x' } };
    const parsed = parseAsk(body);
    expect(parsed.ok && Object.keys(parsed.body)).toEqual(['messages', 'context']);
    expect(parsed.ok && Object.keys(parsed.body.context)).toEqual(['platform', 'language', 'tab', 'today', 'settings', 'photos', 'guide', 'places']);
  });

  it.each([
    ['not an object', 'hello'],
    ['null', null],
    ['no messages', { context: CONTEXT }],
    ['empty messages', { messages: [], context: CONTEXT }],
    ['no context', { messages: askBody().messages }],
  ])('rejects %s', (_name, body) => {
    expect(parseAsk(body).ok).toBe(false);
  });

  it('rejects too many turns, long turns, blank turns and bad roles', () => {
    const turn = (role: string, text: string) => ({ role, text });
    const tooMany = Array.from({ length: MAX_TURNS + 1 }, (_, i) => turn(i % 2 ? 'assistant' : 'user', 'hi'));
    expect(parseAsk({ messages: tooMany, context: CONTEXT }).ok).toBe(false);
    expect(parseAsk({ messages: [turn('user', 'x'.repeat(MAX_TURN_CHARS + 1))], context: CONTEXT }).ok).toBe(false);
    expect(parseAsk({ messages: [turn('user', '   ')], context: CONTEXT }).ok).toBe(false);
    expect(parseAsk({ messages: [turn('system', 'hi')], context: CONTEXT }).ok).toBe(false);
  });

  it('requires the conversation to start and end with the user', () => {
    const user = { role: 'user', text: 'hi' };
    const assistant = { role: 'assistant', text: 'hello' };
    expect(parseAsk({ messages: [assistant, user], context: CONTEXT }).ok).toBe(false);
    expect(parseAsk({ messages: [user, assistant], context: CONTEXT }).ok).toBe(false);
    expect(parseAsk({ messages: [user, assistant, user], context: CONTEXT }).ok).toBe(true);
  });

  it('rejects a malformed context', () => {
    const messages = askBody().messages;
    const bad = [
      { ...CONTEXT, language: 5 },
      { ...CONTEXT, tab: undefined },
      { ...CONTEXT, today: 'yesterday' },
      { ...CONTEXT, today: '2026-13-45' },
      { ...CONTEXT, settings: [] },
      { ...CONTEXT, settings: Array.from({ length: 41 }, () => CONTEXT.settings[0]) },
      { ...CONTEXT, settings: [{ ...CONTEXT.settings[0], type: 'weird' }] },
      { ...CONTEXT, settings: [{ ...CONTEXT.settings[0], description: 'x'.repeat(1001) }] },
      { ...CONTEXT, settings: [{ ...CONTEXT.settings[0], options: [{ value: 'a', label: 'b' }] }] },
      { ...CONTEXT, guide: undefined },
      { ...CONTEXT, guide: '   ' },
      { ...CONTEXT, guide: 'x'.repeat(MAX_GUIDE_CHARS + 1) },
      { ...CONTEXT, places: undefined },
      { ...CONTEXT, places: [] },
      { ...CONTEXT, places: Array.from({ length: MAX_PLACES + 1 }, (_, i) => `place-${'a'.repeat(i % 5 + 1)}${'b'.repeat(Math.floor(i / 5) + 1)}`) },
      { ...CONTEXT, places: ['Browse'] },
      { ...CONTEXT, places: ['send email'] },
      { ...CONTEXT, places: ['browse', 'browse'] },
      { ...CONTEXT, places: ['browse', 3] },
    ];
    for (const context of bad) expect(parseAsk({ messages, context }).ok).toBe(false);
  });
});
