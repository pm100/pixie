import type { TagUse } from './api';

/** The longest a tag name may be (the same as the library's). */
export const MAX_TAG_LEN = 60;

/** A tag name as it is stored: spaces trimmed and collapsed, one leading `#` dropped; null when nothing is left or it is too long. */
export function normalizeTag(raw: string): string | null {
  const t = raw.trim();
  const name = (t.startsWith('#') ? t.slice(1) : t).split(/\s+/).filter(Boolean).join(' ');
  return name !== '' && [...name].length <= MAX_TAG_LEN ? name : null;
}

export type TagAsRow = { name: string; on: number; all: boolean };

/** What the Tag as… box lists for what is typed: the tags that contain it (first `limit`, in the order given), how many more match,
 *  and the name to offer to create (typed text that no tag has, in any case). `count` is how many photos are being tagged. */
export function tagAsRows(usage: TagUse[], count: number, query: string, limit = 6): { rows: TagAsRow[]; more: number; create: string | null } {
  const q = query.trim().toLowerCase();
  const matches = usage.filter((u) => q === '' || u.name.toLowerCase().includes(q));
  const typed = normalizeTag(query);
  const exists = typed !== null && usage.some((u) => u.name.toLowerCase() === typed.toLowerCase());
  return {
    rows: matches.slice(0, limit).map((u) => ({ name: u.name, on: u.on, all: count > 0 && u.on === count })),
    more: Math.max(0, matches.length - limit),
    create: typed !== null && !exists ? typed : null,
  };
}

/** What Enter does in the Tag as… box: the tag to give, or null for nothing. A tag typed in full (any case) is that tag; one
 *  tag that contains what was typed is that tag; text that no tag contains is a new tag; several matches and no exact one is
 *  left for the person to pick from the list. A tag that is already on every photo is never given again. */
export function enterChoice(usage: TagUse[], count: number, query: string): string | null {
  const typed = normalizeTag(query);
  if (typed === null) return null;
  const lower = typed.toLowerCase();
  const given = (u: TagUse) => (count > 0 && u.on === count ? null : u.name);
  const exact = usage.find((u) => u.name.toLowerCase() === lower);
  if (exact) return given(exact);
  const matches = usage.filter((u) => u.name.toLowerCase().includes(lower));
  if (matches.length === 1) return given(matches[0]);
  return matches.length === 0 ? typed : null;
}

/** Does a change to the picks (touched is undefined) or to one tag change what the chosen Picked or tag lists? stillChosen is
 *  false when the library dropped the choice (its tag has no photos left), which always changes the view. */
export function listingAffected(active: string | null, touched: string | undefined, stillChosen: boolean): boolean {
  if (active === null) return false;
  if (!stillChosen || touched === undefined) return true;
  return active.toLowerCase() === 'tag:' + touched.toLowerCase();
}

/** The tags that every one of the `count` photos has. */
export const tagsOnAll = (usage: TagUse[], count: number): string[] => (count > 0 ? usage.filter((u) => u.on === count).map((u) => u.name) : []);

export type UserTagChoice = { picked: true } | { tag: string };

/** How a chosen Picked or tag travels to the library and back: `picked` or `tag:<name>`. */
export const filterKey = (f: UserTagChoice): string => ('picked' in f ? 'picked' : `tag:${f.tag}`);

export function parseFilterKey(key: string | null): UserTagChoice | null {
  if (key === 'picked') return { picked: true };
  if (key !== null && key.startsWith('tag:') && key.length > 4) return { tag: key.slice(4) };
  return null;
}
