import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';

// The wording about what Ask Pixie sends must not contradict itself: names of tags, people and trips are sent only when the person
// turns on sharing, so they may not be listed as "never sent", and the first-use screen may not say Ask Pixie never sends them.
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

  it('has three lists, in this order: always sent, sent only with sharing, never sent', () => {
    const heads = [...page.matchAll(/<h3>([^<]+)<\/h3>/g)].map((m) => m[1]);
    expect(heads).toEqual(['Share the names in my library', 'Always sent', 'Sent only if you turn on sharing', 'Never sent, whatever you choose']);
  });

  it('puts the names of tags, people and trips only in the list for sharing', () => {
    const only = section(page, 'Sent only if you turn on sharing');
    for (const word of ['tags', 'people', 'trips']) expect(only).toContain(word);
    for (const heading of ['Always sent', 'Never sent, whatever you choose']) {
      const list = section(page, heading);
      expect(list, heading).not.toMatch(/\btags?\b|\bpeople\b|\btrips?\b/i);
    }
  });

  it('keeps the things that are never sent out of the sharing list, and names them as never', () => {
    const never = section(page, 'Never sent, whatever you choose');
    for (const word of ['Pictures', 'faces', 'File names', 'folder', 'home', 'coordinates', 'contacts']) expect(never).toContain(word);
    expect(section(page, 'Sent only if you turn on sharing')).not.toMatch(/home|coordinates|folder|file names/i);
  });

  it('says nothing is sent about the names unless sharing is on', () => {
    expect(page).not.toMatch(/unless you turn on sharing/i);
  });
});

describe('the first-use screen', () => {
  const tab = source('AskTab.svelte');
  const intro = tab.slice(tab.indexOf('class="intro"'), tab.indexOf('class="chat"'));

  it('does not say the names of tags and people are never sent', () => {
    expect(intro).not.toMatch(/never sends[^.]*names of your tags/i);
    expect(intro).toContain('Settings, Ask Pixie');
  });

  it('says what the optional sharing is for, and that it is off until chosen', () => {
    expect(intro).toMatch(/names of your tags, people (and|or) trips/i);
    expect(intro).toMatch(/leave that off/i);
  });
});
