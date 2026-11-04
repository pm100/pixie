import { describe, expect, it } from 'vitest';
import { GO_TO_TARGETS } from './askGoTo';
import { localHelp, TOPICS } from './askLocal';
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

  it('keeps each topic short, plain and unique', () => {
    expect(new Set(TOPICS.map((t) => t.id)).size).toBe(TOPICS.length);
    for (const t of TOPICS) {
      expect(t.text.length, t.id).toBeLessThan(600);
      expect(t.text, t.id).not.toMatch(/`|\$\{|\*\*/);
      expect(t.keywords.length, t.id).toBeGreaterThan(1);
    }
  });
});
