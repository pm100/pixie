import { describe, expect, it } from 'vitest';
import { ALL_ON } from './analysisOptions';
import { buildRequest, categoryLabel, photoState, dayOf, errorText, feedbackRequest, memoryNote, newInstallId, parseReply, platformName, turnsToSend, type ChatMessage } from './ask';
import { GO_TO_TARGETS } from './askGoTo';
import { GUIDE } from './askGuide';
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

  it('never sends what the built-in help said as if the model had said it', () => {
    const history: ChatMessage[] = [u('how do I print'), { role: 'error', text: 'used up' }, { role: 'assistant', text: 'Open Send...', local: true }, u('and email?')];
    expect(turnsToSend(history)).toEqual([{ role: 'user', text: 'how do I print' }, { role: 'user', text: 'and email?' }]);
  });

  it('sends nothing when the last thing said was not by the user', () => {
    expect(turnsToSend([u('hi'), a('hello')])).toEqual([]);
  });
});

describe('parseReply', () => {
  const quota = { remaining: 4, resetsAt: '2026-10-29T00:00:00.000Z' };

  it('reads a good reply, with the optional parts', () => {
    expect(parseReply({ reply: 'Hi.', proposal: null, goTo: 'browse', feedback: [{ category: 'bug', summary: ' It crashed. ' }], quota }))
      .toEqual({ reply: 'Hi.', proposal: [], goTo: 'browse', feedback: [{ category: 'bug', summary: 'It crashed.' }], quota });
    expect(parseReply({ reply: 'Ok.', proposal: [{ setting: 'analysis_faces', value: false }], quota }))
      .toMatchObject({ proposal: [{ setting: 'analysis_faces', value: false }], goTo: null, feedback: [] });
  });

  it('ignores anything that is not the right shape', () => {
    expect(parseReply(null)).toBeNull();
    expect(parseReply({ reply: 5 })).toBeNull();
    const r = parseReply({ reply: 'x', proposal: [{ setting: 3 }, 'no', { setting: 'a', value: 1 }], goTo: 7, quota: 'lots' });
    expect(r).toEqual({ reply: 'x', proposal: [{ setting: 'a', value: 1 }], goTo: null, feedback: [], quota: null });
  });

  it('keeps only well-formed notes for the developer, at most three, each cut to the length the proxy takes', () => {
    const note = (category: unknown, summary: unknown) => ({ category, summary });
    const r = parseReply({ reply: 'x', feedback: [note('praise', 'no'), note('bug', 5), note('other', '  '), 'text', note('feature_request', 'a'.repeat(900)), note('bug', 'b'), note('other', 'c'), note('bug', 'd')] });
    expect(r?.feedback.map((f) => f.category)).toEqual(['feature_request', 'bug', 'other']);
    expect(r?.feedback[0].summary).toHaveLength(500);
    expect(parseReply({ reply: 'x', feedback: 'lots' })?.feedback).toEqual([]);
  });
});

describe('errorText', () => {
  it('says when the questions come back, and that the built-in help answers meanwhile', () => {
    const now = new Date(2026, 9, 28, 10, 0);
    const t = errorText(429, { quota: { remaining: 0, resetsAt: new Date(2026, 9, 28, 17, 0).toISOString() } }, now);
    expect(t).toContain('used all of today');
    expect(t).toContain('later today at');
    expect(t).toContain('5:00');
    expect(t).toContain("built-in help");
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
    expect(errorText(502, {})).toContain("couldn't get an answer");
    expect(errorText(0, {})).toContain('reach');
    for (const status of [502, 0, 429]) expect(errorText(status, {}), String(status)).toContain("built-in help");
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
  const body = JSON.parse(buildRequest([u('Stop it looking for faces')], { platform: 'windows', language: 'en-US', tab: 'browse', today: '2026-10-28' }, values, photoState(12, 2, 'photo', 'current')));

  it('holds the words typed, the settings, and what this version of the app says about itself, and nothing else', () => {
    expect(Object.keys(body)).toEqual(['messages', 'context']);
    expect(Object.keys(body.context)).toEqual(['platform', 'language', 'tab', 'today', 'settings', 'guide', 'places', 'photos']);
    expect(body.context.photos).toEqual(photoState(12, 2, 'photo', 'current'));
    expect(body.context.guide).toBe(GUIDE);
    expect(body.context.places).toEqual([...GO_TO_TARGETS]);
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

describe('photoState', () => {
  it('says what the person has and what the tools work on, without saying which tag', () => {
    expect(photoState(3, 1, null, 'picked')).toMatchObject({ picked: 3, pickedVideos: 1, chosen: false, chosenVideo: false, workingOn: 'picked' });
    expect(photoState(0, 0, 'video', 'current')).toMatchObject({ picked: 0, chosen: true, chosenVideo: true, workingOn: 'current' });
    expect(photoState(0, 0, 'photo', 'picked')).toMatchObject({ chosen: true, chosenVideo: false });
    expect(photoState(0, 0, null, 'tag:Holiday 2019')).toMatchObject({ workingOn: 'tag' });
    expect(JSON.stringify(photoState(0, 0, null, 'tag:Holiday 2019'))).not.toContain('Holiday');
    expect(photoState(2, 9, null, 'picked').pickedVideos, 'never more videos than picked').toBe(2);
  });

  it('writes what it means for the steps: one video with other photos picked needs the step that stops the picked photos being used', () => {
    const picked = photoState(89, 0, null, 'picked').hint;
    expect(picked).toContain('89 photos are picked');
    expect(picked).toContain('"Set **Working on** to **Current**."');
    expect(picked).toContain('89 picked photos');
    expect(picked).toContain('Do not mention how many are picked');
    expect(photoState(1, 0, null, 'picked').hint).toContain('1 photo is picked');
    expect(photoState(0, 0, 'photo', 'picked').hint).toContain('No photos are picked and the tools use the photo chosen');
    expect(photoState(0, 0, 'photo', 'current').hint).toContain('Working on: Current');
    expect(photoState(0, 0, null, 'picked').hint).toContain('Nothing is picked or chosen');
    expect(photoState(5, 0, 'photo', 'tag:Holiday').hint).toContain('one of the user\'s tags');
  });

  it('says there is no video to work on, and that the steps start with finding one, when there is none', () => {
    for (const hint of [photoState(89, 0, null, 'picked').hint, photoState(0, 0, 'photo', 'picked').hint, photoState(0, 0, null, 'picked').hint]) {
      expect(hint).toContain('there is no video for the tool yet');
      expect(hint).toContain('opening the tool first would be useless');
      expect(hint).toContain('click **Videos only** under **Library**');
    }
    expect(photoState(89, 0, null, 'picked').hint).toContain('the picked photos include no video');
    expect(photoState(0, 0, 'photo', 'picked').hint).toContain('the chosen item is not a video');
    expect(photoState(0, 0, null, 'picked').hint).toContain('none is chosen');
  });

  it('says a video is there when one is picked or chosen, and does not send the person looking for one', () => {
    const inPicked = photoState(89, 2, null, 'picked').hint;
    expect(inPicked).toContain('holds 2 videos');
    expect(inPicked).not.toContain('Videos only');
    expect(photoState(1, 1, null, 'picked').hint).toContain('holds 1 video ');
    const chosen = photoState(0, 0, 'video', 'picked').hint;
    expect(chosen).toContain('the video the user has chosen will be used');
    expect(chosen).not.toContain('Videos only');
    expect(photoState(0, 0, 'video', 'current').hint).not.toContain('Videos only');
  });

  it('leaves videos out of it while the tools work on a tag, because it cannot know', () => {
    expect(photoState(0, 0, null, 'tag:x').hint).not.toContain('VIDEO');
  });

  it('keeps the note within what the proxy takes, with no name from the library in it', () => {
    for (const [p, v, c, src] of [[89, 0, null, 'picked'], [89, 2, 'video', 'picked'], [0, 0, 'video', 'current'], [0, 0, null, 'picked'], [3, 0, 'photo', 'tag:Holiday 2019']] as const) {
      const hint = photoState(p, v, c, src).hint;
      expect(hint.length, src).toBeLessThanOrEqual(1500); // MAX_HINT_CHARS in ask-proxy/src/validate.ts
      expect(hint).not.toContain('Holiday');
    }
  });
});

describe('what is sent to the developer', () => {
  it('is the note the person read and their system, and nothing else', () => {
    const body = JSON.parse(feedbackRequest({ category: 'feature_request', summary: 'Wants dark mode.' }, 'windows'));
    expect(body).toEqual({ category: 'feature_request', summary: 'Wants dark mode.', platform: 'windows' });
  });

  it('names the kinds of note in words a person would use', () => {
    expect([categoryLabel('feature_request'), categoryLabel('bug'), categoryLabel('other')]).toEqual(['Feature request', 'Problem', 'Comment']);
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
  const said = (over: Partial<ChatMessage>): ChatMessage => ({ role: 'assistant', text: 'ok', ...over });
  const changes = [{ key: 'analysis_faces', label: 'People', value: false, from: 'On', to: 'Off' }];

  it('says what became of a settings change', () => {
    expect(memoryNote(said({ changes, card: 'applied' }))).toBe('[Applied: People On → Off.]');
    expect(memoryNote(said({ changes, card: 'cancelled' }))).toBe('[Not applied.]');
    expect(memoryNote(said({ changes, card: 'open' }))).toBe('[Waiting for the user to press Apply.]');
  });

  it('says what became of a note for the developer, so the model never thinks one was sent when it was not', () => {
    expect(memoryNote(said({ noteCard: 'open' }))).toBe('[Waiting for the user to confirm passing it on.]');
    expect(memoryNote(said({ noteCard: 'sent' }))).toBe('[Passed on to the developer.]');
    expect(memoryNote(said({ noteCard: 'declined' }))).toBe('[Not sent to the developer.]');
    expect(memoryNote(said({ changes, card: 'applied', noteCard: 'declined' }))).toBe('[Applied: People On → Off. Not sent to the developer.]');
  });

  it('has nothing to say about a plain message', () => {
    expect(memoryNote(said({}))).toBe('');
  });
});

describe('turnsToSend, what became of a proposal', () => {
  it('puts the note after the words of the assistant, and the note survives however long the words are', () => {
    const changes = [{ key: 'analysis_faces', label: 'People', value: false, from: 'On', to: 'Off' }];
    const turns = turnsToSend([u('stop faces'), { role: 'assistant', text: 'y'.repeat(900), changes, card: 'applied' }, u('thanks')]);
    expect(turns[1].text.length).toBeLessThanOrEqual(600);
    expect(turns[1].text).toContain('Applied: People On → Off.');
    expect(turns[1].text.startsWith('yyy')).toBe(true);
  });

  it('leaves a message without a note exactly as it was', () => {
    expect(turnsToSend([u('hi'), a('hello'), u('again')])[1].text).toBe('hello');
  });
});
