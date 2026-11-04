import { describe, expect, it } from 'vitest';
import { GO_TO_TARGETS } from './askGoTo';
import { localHelp, photosLine, TOPICS } from './askLocal';
import { TOOLS } from './tools';

describe('help without the server', () => {
  const topic = (q: string) => localHelp(q)?.topics[0];

  it('answers the common questions from the right topic, with a button to the right place', () => {
    const cases: [string, string, string | null][] = [
      ['how do I email a video', 'email', 'send-email'],
      ['how can i print several photos on one page', 'print', 'send-print'],
      ['how do I tag photos', 'tag', 'organize-tags'],
      ['how do I remove a tag', 'untag', 'organize-tags'],
      ['what are duplicates', 'duplicates', 'cleanup-duplicates'],
      ['delete duplicates', 'duplicates', 'cleanup-duplicates'],
      ['how do I make a collage', 'collage', 'create-collage'],
      ['how does finding faces work', 'people', 'people'],
      ['how do I stop the face thing', 'people', 'people'],
      ['my photos have the wrong date', 'dates', 'cleanup-missing-dates'],
      ['where is the sync folder', 'sync', 'transfer-sync'],
      ['how do I add a folder', 'folders', 'settings-folders'],
      ['how do I see my trips', 'trips', 'settings-trips'],
      ['how do I pick photos', 'pick', 'browse'],
      ['the app is slow', 'processing', 'settings-analysis'],
      ['is my data safe', 'privacy', 'settings-ask'],
      ['what does Ask Pixie send', 'privacy', 'settings-ask'],
      ['how do I open a photo bigger', 'viewer', 'settings-viewing'],
    ];
    for (const [question, id, place] of cases) {
      const r = localHelp(question);
      expect(r?.topics[0], question).toBe(id);
      expect(r?.goTo, question).toBe(place);
    }
  });

  it('says what the person has already, for a topic that works on photos, and only for those', () => {
    const picked = localHelp('how do I email these', { picked: 12, chosen: false, workingOn: 'picked' })!;
    expect(picked.text.split('\n')[0]).toContain('12 photos');
    expect(picked.text).toContain('go straight to the tool');
    const chosen = localHelp('how do I print', { picked: 0, chosen: true, workingOn: 'current' })!;
    expect(chosen.text.split('\n')[0]).toContain('Working on: Current');
    expect(localHelp('how do I email these', { picked: 0, chosen: false, workingOn: 'picked' })!.text.split('\n')[0]).toContain('Nothing is picked or chosen yet');
    expect(localHelp('how do I see my trips', { picked: 12, chosen: true, workingOn: 'picked' })!.text).not.toContain('picked');
    expect(localHelp('how do I email a video')!.text.startsWith('To email')).toBe(true);
  });

  it('does not bring up the picked photos when the question is about one video or one photo', () => {
    const state = { picked: 89, chosen: false, workingOn: 'picked' as const };
    for (const q of ['how do I email a video', 'how do I print this photo', 'how do I tag one photo']) {
      const text = localHelp(q, state)!.text;
      expect(text, q).not.toContain('89');
      expect(text, q).not.toContain('picked, so');
    }
  });

  it('never suggests ways of sharing that Pixie does not have, or guesses about the size of a file', () => {
    for (const t of TOPICS) expect(t.text, t.id).not.toMatch(/cloud drive link|may be refused|share it another way/i);
  });

  it('counts one photo as one', () => {
    expect(photosLine({ picked: 1, chosen: false, workingOn: 'picked' })).toContain('**1 photo**');
    expect(photosLine({ picked: 0, chosen: false, workingOn: 'tag' })).toContain('tag');
  });

  it('says plainly that what is not built is not built, with no button', () => {
    for (const q of ['switch to dark mode', 'make the text bigger', 'can I make a slideshow']) {
      expect(topic(q), q).toBe('notyet');
      expect(localHelp(q)?.goTo).toBeNull();
    }
  });

  it('finds a word whatever its ending, and ignores capitals and punctuation', () => {
    expect(topic('PRINTING?!')).toBe('print');
    expect(topic('Faces')).toBe('people');
    expect(topic('duplicates')).toBe('duplicates');
  });

  it('says nothing when nothing fits, so the panel can say it did not find it', () => {
    expect(localHelp('asdf qwer')).toBeNull();
    expect(localHelp('')).toBeNull();
    expect(localHelp('how do I')).toBeNull();
  });

  it('adds a second topic only when it fits nearly as well', () => {
    expect(localHelp('how do I print')?.topics).toEqual(['print']);
    expect(localHelp('email video photo folder sync')?.topics.length).toBeLessThanOrEqual(2);
  });

  it('has a topic for every tool that is built, and only buttons to places that exist', () => {
    const goTos = new Set(TOPICS.map((t) => t.goTo));
    for (const t of TOOLS.filter((x) => x.built)) expect(goTos.has(`${t.group}-${t.id}`), `${t.group}-${t.id}`).toBe(true);
    for (const t of TOPICS) {
      if (t.goTo !== null) expect(GO_TO_TARGETS as readonly string[], t.id).toContain(t.goTo);
    }
  });

  it('lays how-to answers out as numbered steps that count up from 1, and names the controls in bold', () => {
    for (const t of TOPICS) {
      const steps = [...t.text.matchAll(/^(\d+)\. /gm)].map((m) => Number(m[1]));
      expect(steps, t.id).toEqual(steps.map((_, i) => i + 1));
    }
    for (const id of ['email', 'print', 'collage', 'tag', 'duplicates']) {
      const t = TOPICS.find((x) => x.id === id)!;
      expect(t.text, id).toMatch(/^1\. /m);
      expect(t.text, id).toMatch(/\*\*[A-Z][^*]+\*\*/);
    }
  });

  it('keeps each topic short, plain and unique', () => {
    expect(new Set(TOPICS.map((t) => t.id)).size).toBe(TOPICS.length);
    for (const t of TOPICS) {
      expect(t.text.length, t.id).toBeLessThan(600);
      expect(t.text, t.id).not.toMatch(/`|\$\{/);
      expect((t.text.match(/\*\*/g) ?? []).length % 2, `${t.id}: bold marks must pair up`).toBe(0);
      expect(t.keywords.length, t.id).toBeGreaterThan(1);
    }
  });
});
