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

describe('parseAsk, library', () => {
  const library = {
    photos: 1000, picked: 2,
    tags: [{ name: 'holiday', count: 12 }],
    people: [{ name: 'Rose', count: 40 }],
    trips: [{ name: 'Italy', from: '2019-06-01', to: '2019-06-14', nights: 13, photos: 340, videos: 6, where: 'Rome' }],
  };
  const withLibrary = (lib: unknown) => ({ ...askBody(), context: { ...CONTEXT, library: lib } });

  it('is optional, and kept when well formed', () => {
    const none = parseAsk(askBody());
    expect(none.ok && none.body.context.library).toBeUndefined();
    const some = parseAsk(withLibrary(library));
    expect(some.ok && some.body.context.library).toEqual(library);
  });

  it('keeps only the fields it knows', () => {
    const lib = { ...library, folders: ['C:/x'], tags: [{ name: 'a', count: 1, path: 'p' }] };
    const parsed = parseAsk(withLibrary(lib));
    expect(parsed.ok && Object.keys(parsed.body.context.library!)).toEqual(['photos', 'picked', 'tags', 'people', 'trips']);
    expect(parsed.ok && parsed.body.context.library!.tags).toEqual([{ name: 'a', count: 1 }]);
  });

  it.each([
    ['not an object', 'x'],
    ['a count that is not a number', { ...library, photos: 'many' }],
    ['a tag with no name', { ...library, tags: [{ count: 1 }] }],
    ['too many tags', { ...library, tags: Array.from({ length: 151 }, (_, i) => ({ name: `t${i}`, count: 1 })) }],
    ['too many people', { ...library, people: Array.from({ length: 101 }, (_, i) => ({ name: `p${i}`, count: 1 })) }],
    ['too many trips', { ...library, trips: Array.from({ length: 41 }, () => library.trips[0]) }],
    ['a name that is too long', { ...library, tags: [{ name: 'x'.repeat(81), count: 1 }] }],
    ['a trip with a bad date', { ...library, trips: [{ ...library.trips[0], from: 'soon' }] }],
  ])('rejects %s', (_name, lib) => {
    expect(parseAsk(withLibrary(lib)).ok).toBe(false);
  });
});

describe('parseAsk, lookups', () => {
  const round = { calls: [{ id: 'toolu_1', input: { what: 'people', query: 'rose' } }], results: ['[{"name":"Rose","count":4}]'] };
  const withRounds = (lookups: unknown) => ({ ...askBody(), lookups });

  it('is optional, and kept when well formed', () => {
    const none = parseAsk(askBody());
    expect(none.ok && none.body.lookups).toBeUndefined();
    const some = parseAsk(withRounds([round]));
    expect(some.ok && some.body.lookups).toEqual([round]);
  });

  it('keeps only the fields it knows, and nothing but the key order it expects at the top', () => {
    const parsed = parseAsk({ ...withRounds([{ ...round, extra: 1 }]), evil: 1 });
    expect(parsed.ok && Object.keys(parsed.body)).toEqual(['messages', 'context', 'lookups']);
    expect(parsed.ok && Object.keys(parsed.body.lookups![0]!)).toEqual(['calls', 'results']);
  });

  it.each([
    ['not a list', 'x'],
    ['more than three rounds', [round, round, round, round]],
    ['a round with no calls', [{ calls: [], results: [] }]],
    ['more than three calls in a round', [{ calls: Array.from({ length: 4 }, (_, i) => ({ id: `a${i}`, input: {} })), results: ['a', 'b', 'c', 'd'] }]],
    ['results that do not match the calls', [{ calls: round.calls, results: [] }]],
    ['an id that is not safe to send on', [{ ...round, calls: [{ id: 'a b', input: {} }] }]],
    ['an id that is too long', [{ ...round, calls: [{ id: 'a'.repeat(65), input: {} }] }]],
    ['an input that is not an object', [{ ...round, calls: [{ id: 'a', input: 'x' }] }]],
    ['an input that is too big', [{ ...round, calls: [{ id: 'a', input: { query: 'x'.repeat(2100) } }] }]],
    ['a result that is too long', [{ ...round, results: ['x'.repeat(2001)] }]],
    ['a result that is not text', [{ ...round, results: [5] }]],
  ])('rejects %s', (_name, lookups) => {
    expect(parseAsk(withRounds(lookups)).ok).toBe(false);
  });
});

describe('parseAsk, results', () => {
  const withResults = (results: unknown) => ({ ...askBody(), context: { ...CONTEXT, results } });

  it('is optional, and kept when well formed', () => {
    const none = parseAsk(askBody());
    expect(none.ok && none.body.context.results).toBeUndefined();
    const some = parseAsk(withResults({ count: 47, of: 'photos that look like “rose”' }));
    expect(some.ok && some.body.context.results).toEqual({ count: 47, of: 'photos that look like “rose”' });
  });

  it('keeps only the fields it knows, after the library', () => {
    const parsed = parseAsk(withResults({ count: 1, of: 'x', extra: 1 }));
    expect(parsed.ok && Object.keys(parsed.body.context.results!)).toEqual(['count', 'of']);
  });

  it.each([
    ['not an object', 'x'],
    ['a count that is not a number', { count: 'many', of: 'x' }],
    ['a negative count', { count: -1, of: 'x' }],
    ['no description', { count: 1 }],
    ['a description that is too long', { count: 1, of: 'x'.repeat(301) }],
  ])('rejects %s', (_name, results) => {
    expect(parseAsk(withResults(results)).ok).toBe(false);
  });
});
