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

  it('has two lists, in this order: always sent, never sent', () => {
    expect([...page.matchAll(/<h3>([^<]+)<\/h3>/g)].map((m) => m[1])).toEqual(['Always sent', 'Never sent']);
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

describe('the first-use screen', () => {
  const tab = source('AskTab.svelte');
  const intro = tab.slice(tab.indexOf('class="intro"'), tab.indexOf('class="chat"'));

  it('says the names of tags, people and trips are never sent', () => {
    expect(intro).toMatch(/never sends[^.]*names of your tags, people or trips/i);
  });
});
