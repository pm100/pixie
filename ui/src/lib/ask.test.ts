import { describe, expect, it } from 'vitest';
import { ALL_ON } from './analysisOptions';
import { buildRequest, errorText, dayOf, newInstallId, parseReply, platformName, turnsToSend, type ChatMessage } from './ask';
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
      .toEqual({ reply: 'Hi.', proposal: [], goTo: 'browse', find: null, lookup: [], feedbackSent: true, quota });
    expect(parseReply({ reply: 'Ok.', proposal: [{ setting: 'analysis_faces', value: false }], quota }))
      .toMatchObject({ proposal: [{ setting: 'analysis_faces', value: false }], goTo: null, feedbackSent: false });
  });

  it('ignores anything that is not the right shape', () => {
    expect(parseReply(null)).toBeNull();
    expect(parseReply({ reply: 5 })).toBeNull();
    const r = parseReply({ reply: 'x', proposal: [{ setting: 3 }, 'no', { setting: 'a', value: 1 }], goTo: 7, quota: 'lots' });
    expect(r).toEqual({ reply: 'x', proposal: [{ setting: 'a', value: 1 }], goTo: null, find: null, lookup: [], feedbackSent: false, quota: null });
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
