import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';

// The wording about what Ask Pixie sends must agree with what it does: only typed words and settings go, and the names of tags,
// people and trips never do. Settings lists it in two places, so they must not contradict each other.
const source = (name: string) => readFileSync(new URL(`./${name}`, import.meta.url), 'utf8');

function section(text: string, heading: string): string {
  const start = text.indexOf(`<h3>${heading}</h3>`);
  expect(start, `no heading ${heading}`).toBeGreaterThan(-1);
  const rest = text.slice(start + heading.length + 9);
  const end = rest.indexOf('<h3>');
  const stop = rest.indexOf('<p class="hint">');
  return rest.slice(0, [end, stop].filter((n) => n >= 0).sort((a, b) => a - b)[0] ?? rest.length);
}

describe('Settings, Ask Pixie: what is sent', () => {
  const page = source('AskSettings.svelte');

  it('has three lists, in this order: always sent, sent only if you say so, never sent', () => {
    expect([...page.matchAll(/<h3>([^<]+)<\/h3>/g)].map((m) => m[1])).toEqual(['Always sent', 'Sent only if you say so', 'Never sent']);
  });

  it('says a note for the developer is shown first and sent only when Yes is pressed', () => {
    const only = section(page, 'Sent only if you say so');
    for (const word of ['note', 'developer', 'see the note first', 'answer Yes']) expect(only).toContain(word);
  });

  it('keeps the names of tags, people and trips out of what is sent, and lists them as never sent', () => {
    expect(section(page, 'Always sent')).not.toMatch(/\btags?\b|\bpeople\b|\btrips?\b/i);
    const never = section(page, 'Never sent');
    for (const word of ['Pictures', 'faces', 'File names', 'folder', 'tags', 'people', 'trips', 'home', 'coordinates', 'contacts']) expect(never).toContain(word);
  });

  it('offers no switch for sharing the names', () => {
    expect(page).not.toMatch(/library|sharing/i);
  });
});

describe('the note under the chat box', () => {
  const panel = source('AskPanel.svelte');
  const note = panel.slice(panel.indexOf('<p class="note">'), panel.indexOf('</p>', panel.indexOf('<p class="note">')));

  it('says the pictures stay here and the questions are sent, and links to the list of what is sent', () => {
    expect(note).toContain('Your pictures stay on this computer.');
    expect(note).toMatch(/Your questions are sent to a server/);
    expect(note).toContain('What is sent');
  });

  it('asks before anything is sent to the developer: the note is shown, and only the Yes button sends it', () => {
    expect(panel).toContain('Pass this on to the developer?');
    expect(panel).toContain("Nothing is sent unless you press Yes.");
    expect(panel.match(/askFeedback\(/g)).toHaveLength(1);
    expect(panel.indexOf('askFeedback(')).toBeGreaterThan(panel.indexOf('async function sendNotes'));
    expect(panel.indexOf('askFeedback(')).toBeLessThan(panel.indexOf('function onkey'));
  });

  it('is shown whenever the chat is, and the chat is not shown while Ask Pixie is off', () => {
    expect(panel.indexOf('{#if !on}')).toBeGreaterThan(-1);
    expect(panel.indexOf('{:else}')).toBeGreaterThan(panel.indexOf('{#if !on}'));
    expect(panel.indexOf('<p class="note">')).toBeGreaterThan(panel.indexOf('{:else}'));
  });
});
