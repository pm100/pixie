import { describe, expect, it } from 'vitest';
import { ALL_ON } from './analysisOptions';
import { buildRequest, currentResults, dayOf, errorText, memoryNote, newInstallId, parseReply, platformName, turnsToSend, type ChatMessage, type FoundState } from './ask';
import { type SettingValues } from './askSettings';
import { DEFAULT_TRIP_PREFS } from './trips';

const u = (text: string): ChatMessage => ({ role: 'user', text });
const a = (text: string): ChatMessage => ({ role: 'assistant', text });

describe('turnsToSend', () => {
  it('sends the last six turns, starting and ending with the user', () => {
    const history = [u('1'), a('2'), u('3'), a('4'), u('5'), a('6'), u('7'), a('8'), u('9')];
    expect(turnsToSend(history).map((t) => t.text)).toEqual(['5', '6', '7', '8', '9']);
  });

  it('never sends error lines, cards or a turn the model did not take part in', () => {
    const history: ChatMessage[] = [u('hi'), { role: 'error', text: 'offline' }, u('again')];
    expect(turnsToSend(history)).toEqual([{ role: 'user', text: 'hi' }, { role: 'user', text: 'again' }]);
  });

  it('cuts a long turn to the most the proxy accepts', () => {
    expect(turnsToSend([u('x'.repeat(2000))])[0].text).toHaveLength(600);
  });

  it('sends nothing when the last thing said was not by the user', () => {
    expect(turnsToSend([u('hi'), a('hello')])).toEqual([]);
  });
});

describe('parseReply', () => {
  const quota = { remaining: 4, resetsAt: '2026-10-29T00:00:00.000Z' };

  it('reads a good reply, with the optional parts', () => {
    expect(parseReply({ reply: 'Hi.', proposal: null, goTo: 'browse', feedbackSent: true, quota }))
      .toEqual({ reply: 'Hi.', proposal: [], goTo: 'browse', find: null, lookup: [], openTool: null, feedbackSent: true, quota });
    expect(parseReply({ reply: 'Ok.', proposal: [{ setting: 'analysis_faces', value: false }], quota }))
      .toMatchObject({ proposal: [{ setting: 'analysis_faces', value: false }], goTo: null, feedbackSent: false });
  });

  it('ignores anything that is not the right shape', () => {
    expect(parseReply(null)).toBeNull();
    expect(parseReply({ reply: 5 })).toBeNull();
    const r = parseReply({ reply: 'x', proposal: [{ setting: 3 }, 'no', { setting: 'a', value: 1 }], goTo: 7, quota: 'lots' });
    expect(r).toEqual({ reply: 'x', proposal: [{ setting: 'a', value: 1 }], goTo: null, find: null, lookup: [], openTool: null, feedbackSent: false, quota: null });
  });
});

describe('parseReply, openTool', () => {
  it('reads a button to open a tool on the results, the picked photos or a tag', () => {
    expect(parseReply({ reply: 'x', openTool: { tool: 'print', source: 'results', tag: null } })?.openTool).toEqual({ tool: 'print', source: 'results', tag: null });
    expect(parseReply({ reply: 'x', openTool: { tool: 'email', source: 'tag', tag: 'rose1' } })?.openTool).toEqual({ tool: 'email', source: 'tag', tag: 'rose1' });
    expect(parseReply({ reply: 'x', openTool: { tool: 'tags', source: 'picked', tag: null } })?.openTool?.source).toBe('picked');
  });

  it('ignores anything else: an unknown tool or source, or a tag source with no tag', () => {
    for (const bad of [null, 'x', { tool: 'delete', source: 'results', tag: null }, { tool: 'print', source: 'everything', tag: null }, { tool: 'print', source: 'tag', tag: null }, { tool: 'print', source: 'tag', tag: '  ' }]) {
      expect(parseReply({ reply: 'x', openTool: bad })?.openTool).toBeNull();
    }
  });
});

describe('parseReply, lookup', () => {
  it('reads the questions the model asked the app, and drops the ones that are not well formed', () => {
    const r = parseReply({ reply: 'One moment.', lookup: [
      { id: 'toolu_1', input: { what: 'people', query: 'rose' }, what: 'people', query: 'rose', filter: null },
      { id: 'bad id', input: {}, what: 'people', query: null, filter: null },
      { id: 'toolu_2', input: {}, what: 'delete', query: null, filter: null },
      { id: 'toolu_3', input: { what: 'count' }, what: 'count', query: null, filter: { text: 'dog' } },
    ] });
    expect(r?.lookup.map((l) => l.id)).toEqual(['toolu_1', 'toolu_3']);
    expect(r?.lookup[1].filter).toMatchObject({ text: 'dog' });
  });
});

describe('buildRequest, rounds', () => {
  const app = { platform: 'windows', language: 'en-US', tab: 'ask', today: '2026-10-28' };
  const values: SettingValues = { viewerMode: 'window', emailLimitMb: 20, recentDays: 30, networkRescan: { on: true, minutes: 30 }, analysis: { ...ALL_ON }, trips: { ...DEFAULT_TRIP_PREFS } };
  const round = { calls: [{ id: 'toolu_1', input: { what: 'people' } }], results: ['[]'] };

  it('sends none unless there are some', () => {
    expect(JSON.parse(buildRequest([u('hi')], app, values)).lookups).toBeUndefined();
    expect(JSON.parse(buildRequest([u('hi')], app, values, null, [])).lookups).toBeUndefined();
  });

  it('sends the rounds so far, last in the request', () => {
    const body = JSON.parse(buildRequest([u('hi')], app, values, null, [round]));
    expect(Object.keys(body)).toEqual(['messages', 'context', 'lookups']);
    expect(body.lookups).toEqual([round]);
  });
});

describe('parseReply, find', () => {
  it('reads a search the proxy asked for, and ignores one that asks for nothing', () => {
    const r = parseReply({ reply: 'x', find: { text: 'pink flowers', then: ['pick'] } });
    expect(r?.find).toMatchObject({ text: 'pink flowers', then: ['pick'], tags: [], people: [] });
    expect(parseReply({ reply: 'x', find: {} })?.find).toBeNull();
    expect(parseReply({ reply: 'x', find: 'x' })?.find).toBeNull();
  });
});

describe('errorText', () => {
  it('says when the questions come back, and that it can all be done by hand', () => {
    const now = new Date(2026, 9, 28, 10, 0);
    const t = errorText(429, { quota: { remaining: 0, resetsAt: new Date(2026, 9, 28, 17, 0).toISOString() } }, now);
    expect(t).toContain('used all of today');
    expect(t).toContain('later today at');
    expect(t).toContain('5:00');
    expect(t).toContain('by hand');
    expect(t).not.toContain('Settings');
  });

  it('says tomorrow, or the day, when that is when they come back', () => {
    const now = new Date(2026, 9, 28, 10, 0);
    const at = (d: Date) => ({ quota: { remaining: 0, resetsAt: d.toISOString() } });
    expect(errorText(429, at(new Date(2026, 9, 29, 8, 0)), now)).toContain('tomorrow at');
    expect(errorText(429, at(new Date(2026, 10, 2, 8, 0)), now)).toMatch(/ask again (Mon|Tue|Wed|Thu|Fri|Sat|Sun)[a-z]* at/);
  });

  it('says tomorrow when it does not know the time', () => {
    expect(errorText(429, {}, new Date(2026, 9, 28, 10, 0))).toContain('tomorrow');
  });
  it('says plainly that the assistant could not be reached', () => {
    expect(errorText(502, {})).toContain('try again');
    expect(errorText(0, {})).toContain('reach');
  });
  it('says Ask Pixie is not set up when the build has no address', () => {
    expect(errorText(-1, {})).toContain('not set up');
  });
});

describe('newInstallId', () => {
  it('is a UUID, and a different one each time', () => {
    const id = newInstallId();
    expect(id).toMatch(/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/);
    expect(newInstallId()).not.toBe(id);
  });
});

describe('buildRequest', () => {
  const values: SettingValues = { viewerMode: 'window', emailLimitMb: 20, recentDays: 30, networkRescan: { on: true, minutes: 30 }, analysis: { ...ALL_ON }, trips: { ...DEFAULT_TRIP_PREFS, home: { lat: 51.5, lon: -0.12 }, confirmed: true } };
  const body = JSON.parse(buildRequest([u('Stop it looking for faces')], { platform: 'windows', language: 'en-US', tab: 'ask', today: '2026-10-28' }, values));

  it('holds the words typed, the app version of the user and the settings, and nothing else', () => {
    expect(Object.keys(body)).toEqual(['messages', 'context']);
    expect(Object.keys(body.context)).toEqual(['platform', 'language', 'tab', 'today', 'settings']);
    expect(body.messages).toEqual([{ role: 'user', text: 'Stop it looking for faces' }]);
    expect(body.context.settings.length).toBeGreaterThan(5);
  });

  it('never carries where home is, a file name, a path or picture data', () => {
    const text = JSON.stringify(body);
    expect(text).not.toContain('51.5');
    expect(text).not.toMatch(/[A-Za-z]:[\/]/);
    expect(text).not.toMatch(/data:image|base64|\.jpe?g|\.png|\.heic/i);
  });
});

describe('buildRequest, library', () => {
  const lib = { photos: 10, picked: 1, tags: [{ name: 'holiday', count: 3 }], people: [{ name: 'Rose', count: 4 }], trips: [] };
  const app = { platform: 'windows', language: 'en-US', tab: 'ask', today: '2026-10-28' };
  const values: SettingValues = { viewerMode: 'window', emailLimitMb: 20, recentDays: 30, networkRescan: { on: true, minutes: 30 }, analysis: { ...ALL_ON }, trips: { ...DEFAULT_TRIP_PREFS } };

  it('sends no library unless one is given, so the switch off means none', () => {
    expect(Object.keys(JSON.parse(buildRequest([u('hi')], app, values)).context)).toEqual(['platform', 'language', 'tab', 'today', 'settings']);
    expect(JSON.parse(buildRequest([u('hi')], app, values, null)).context.library).toBeUndefined();
  });

  it('sends the library, last in the context, when one is given', () => {
    const ctx = JSON.parse(buildRequest([u('hi')], app, values, lib)).context;
    expect(Object.keys(ctx)).toEqual(['platform', 'language', 'tab', 'today', 'settings', 'library']);
    expect(ctx.library).toEqual(lib);
  });
});

describe('platformName', () => {
  it('names the system from the browser string', () => {
    expect(platformName('Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit')).toBe('windows');
    expect(platformName('Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7)')).toBe('mac');
    expect(platformName('Mozilla/5.0 (X11; Linux x86_64)')).toBe('linux');
    expect(platformName('')).toBe('other');
  });
});

describe('dayOf', () => {
  it('is the date on the own clock, as YYYY-MM-DD', () => {
    expect(dayOf(new Date(2026, 9, 28, 23, 59))).toBe('2026-10-28');
    expect(dayOf(new Date(2026, 0, 5, 0, 1))).toBe('2026-01-05');
  });
});

describe('memoryNote', () => {
  const found = (over: Partial<FoundState> = {}): FoundState => ({
    args: { text: 'rose', also: ['flowers'], withPeople: false, tags: [], people: [], from: null, to: null, kind: null, limit: null, pickedOnly: false, then: [], tag: null },
    status: 'done', problems: [], ids: Array.from({ length: 47 }, (_, i) => i), allIds: [], scores: null, cut: 0.2, preview: [], analysed: null, weak: false, done: {}, ...over,
  });
  const said = (over: Partial<ChatMessage>): ChatMessage => ({ role: 'assistant', text: 'ok', ...over });

  it('says what a search found, in the words of the search', () => {
    expect(memoryNote(said({ found: found() }))).toBe('[Found 47 photos that look like “rose” and “flowers”.]');
    expect(memoryNote(said({ found: found({ ids: [1] }) }))).toContain('Found 1 photo ');
  });

  it('says what was done with them: picked, tagged, and that the match was weak', () => {
    const f = found({ weak: true, done: { picked: 47, tagged: { tag: 'rose1', ids: [1, 2] } } });
    const note = memoryNote(said({ found: f }));
    expect(note).toContain('A weak match.');
    expect(note).toContain('Picked 47.');
    expect(note).toContain('Tagged 47 “rose1”.');
  });

  it('does not say they are tagged once the tagging was undone', () => {
    expect(memoryNote(said({ found: found({ done: { tagged: undefined } }) }))).not.toContain('Tagged');
  });

  it('says when a search found nothing, did not run, or failed', () => {
    expect(memoryNote(said({ found: found({ ids: [] }) }))).toBe('[The search found nothing.]');
    expect(memoryNote(said({ found: found({ status: 'problem', problems: ['You have no tag called “x”.'] }) }))).toBe('[The search did not run: You have no tag called “x”.]');
    expect(memoryNote(said({ found: found({ status: 'failed', problems: ['offline'] }) }))).toBe('[The search failed: offline]');
    expect(memoryNote(said({ found: found({ status: 'running' }) }))).toBe('[The search is still running.]');
  });

  it('says what became of a settings change', () => {
    const changes = [{ key: 'analysis_faces', label: 'Find faces', value: false, from: 'On', to: 'Off' }];
    expect(memoryNote(said({ changes, card: 'applied' }))).toBe('[Applied: Find faces On → Off.]');
    expect(memoryNote(said({ changes, card: 'cancelled' }))).toBe('[Not applied.]');
    expect(memoryNote(said({ changes, card: 'open' }))).toBe('[Waiting for the user to press Apply.]');
  });

  it('says it offered to open a tool on the results', () => {
    expect(memoryNote(said({ openTool: { tool: 'print', source: 'results', tag: null } }))).toBe('[Offered a button to open Print on the results.]');
    expect(memoryNote(said({ openTool: { tool: 'email', source: 'tag', tag: 'rose1' } }))).toBe('[Offered a button to open Email on the photos tagged “rose1”.]');
  });

  it('has nothing to say about a plain message, and keeps a note to a sensible length', () => {
    expect(memoryNote(said({}))).toBe('');
    const long = found({ args: { ...found().args, text: 'x'.repeat(500) } });
    expect(memoryNote(said({ found: long })).length).toBeLessThanOrEqual(250);
  });
});

describe('turnsToSend, what happened', () => {
  it('puts the note after the assistant\'s words, and the note survives however long the words are', () => {
    const f: FoundState = { args: { text: 'rose', also: [], withPeople: false, tags: [], people: [], from: null, to: null, kind: null, limit: null, pickedOnly: false, then: [], tag: null }, status: 'done', problems: [], ids: [1, 2, 3], allIds: [], scores: null, cut: 0.2, preview: [], analysed: null, weak: false, done: { tagged: { tag: 'rose1', ids: [1] } } };
    const turns = turnsToSend([u('find rose'), { role: 'assistant', text: 'y'.repeat(900), found: f }, u('print them')]);
    expect(turns[1].text.length).toBeLessThanOrEqual(600);
    expect(turns[1].text).toContain('Tagged 3 “rose1”.');
    expect(turns[1].text.startsWith('yyy')).toBe(true);
  });

  it('leaves a message without a note exactly as it was', () => {
    expect(turnsToSend([u('hi'), a('hello'), u('again')])[1].text).toBe('hello');
  });
});

describe('currentResults', () => {
  it('is the last search that found something, as the card shows it', () => {
    const f = (n: number): FoundState => ({ args: { text: 'rose', also: [], withPeople: false, tags: [], people: [], from: null, to: null, kind: null, limit: null, pickedOnly: false, then: [], tag: null }, status: 'done', problems: [], ids: Array.from({ length: n }, (_, i) => i), allIds: [], scores: null, cut: 0.2, preview: [], analysed: null, weak: false, done: {} });
    expect(currentResults([])).toBeNull();
    expect(currentResults([{ role: 'assistant', text: 'x', found: f(0) }])).toBeNull();
    expect(currentResults([{ role: 'assistant', text: 'x', found: f(5) }, { role: 'assistant', text: 'y', found: f(0) }])).toEqual({ count: 5, of: 'photos that look like “rose”' });
  });
});

describe('buildRequest, results', () => {
  const app = { platform: 'windows', language: 'en-US', tab: 'ask', today: '2026-10-28' };
  const values: SettingValues = { viewerMode: 'window', emailLimitMb: 20, recentDays: 30, networkRescan: { on: true, minutes: 30 }, analysis: { ...ALL_ON }, trips: { ...DEFAULT_TRIP_PREFS } };

  it('tells the model what the current results are, after the library', () => {
    const lib = { photos: 10, picked: 1, tags: [], people: [], trips: [] };
    const ctx = JSON.parse(buildRequest([u('hi')], { ...app, results: { count: 47, of: 'photos that look like “rose”' } }, values, lib)).context;
    expect(Object.keys(ctx)).toEqual(['platform', 'language', 'tab', 'today', 'settings', 'library', 'results']);
    expect(ctx.results).toEqual({ count: 47, of: 'photos that look like “rose”' });
  });

  it('says nothing about results when there are none', () => {
    expect(JSON.parse(buildRequest([u('hi')], { ...app, results: null }, values)).context.results).toBeUndefined();
  });
});
