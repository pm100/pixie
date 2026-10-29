import { describe, expect, it } from 'vitest';
import { isInstallId, MAX_TURN_CHARS, MAX_TURNS, parseAsk } from '../src/validate';
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

  it('drops fields it does not know about', () => {
    const body = { ...askBody(), extra: 1, context: { ...CONTEXT, evil: 'x' } };
    const parsed = parseAsk(body);
    expect(parsed.ok && Object.keys(parsed.body)).toEqual(['messages', 'context']);
    expect(parsed.ok && Object.keys(parsed.body.context)).toEqual(['platform', 'language', 'tab', 'today', 'settings']);
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
    ];
    for (const context of bad) expect(parseAsk({ messages, context }).ok).toBe(false);
  });
});
