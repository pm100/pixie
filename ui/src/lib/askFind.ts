/**
 * Ask Pixie's search of the library. The model describes what to look for in words; the names (of tags and people) are matched
 * here against what exists, so no names ever need to be sent to the server. A name that matches nothing, or more than one
 * thing, stops the search: leaving it out would widen the search to photos the person did not mean.
 */
export interface FindArgs {
  text: string | null;
  /** More things that must also be in the picture, each searched on its own. */
  also: string[];
  /** A person (whoever) is in the picture. */
  withPeople: boolean;
  tags: string[];
  people: string[];
  /** YYYY-MM-DD, inclusive. */
  from: string | null;
  to: string | null;
  kind: 'photo' | 'video' | null;
  /** How many were asked for: the best matches, at most this many. */
  limit: number | null;
  pickedOnly: boolean;
  then: ('pick' | 'tag' | 'email' | 'print')[];
  tag: string | null;
}

/** The search the backend runs (mirrors the Rust `FindSpec`; `among` is filled in there by the content search). */
export interface FindSpec {
  kind: 'photo' | 'video' | null;
  since: number | null;
  until: number | null;
  tags: string[];
  people: number[];
  picked: boolean;
  with_people: boolean;
}

export interface Known {
  /** Names of the person's own tags. */
  tags: string[];
  people: { id: number; name: string | null }[];
}

const THEN = ['pick', 'tag', 'email', 'print'] as const;
const DAY = /^\d{4}-\d{2}-\d{2}$/;
const isObject = (v: unknown): v is Record<string, unknown> => typeof v === 'object' && v !== null && !Array.isArray(v);
const strings = (v: unknown): string[] => (Array.isArray(v) ? v.filter((x): x is string => typeof x === 'string' && x.trim() !== '').map((x) => x.trim()) : []);
const day = (v: unknown): string | null => (typeof v === 'string' && DAY.test(v) && !Number.isNaN(Date.parse(v)) ? v : null);

/** What the proxy sent as a search, or null if it is not one. Nothing in it is trusted. */
export function parseFindArgs(raw: unknown): FindArgs | null {
  if (!isObject(raw)) return null;
  const text = typeof raw.text === 'string' && raw.text.trim() !== '' ? raw.text.trim() : null;
  const tag = typeof raw.tag === 'string' && raw.tag.trim() !== '' ? raw.tag.trim() : null;
  const then = (Array.isArray(raw.then) ? raw.then : []).filter((x): x is FindArgs['then'][number] => (THEN as readonly unknown[]).includes(x) && (x !== 'tag' || tag !== null));
  const args: FindArgs = {
    text, also: strings(raw.also).slice(0, 3), withPeople: raw.withPeople === true, tags: strings(raw.tags), people: strings(raw.people), from: day(raw.from), to: day(raw.to),
    kind: raw.kind === 'photo' || raw.kind === 'video' ? raw.kind : null,
    limit: typeof raw.limit === 'number' && Number.isInteger(raw.limit) && raw.limit >= 1 && raw.limit <= 500 ? raw.limit : null, pickedOnly: raw.pickedOnly === true, then, tag,
  };
  return asksForSomething(args) ? args : null;
}

const asksForSomething = (a: FindArgs) => a.text !== null || a.also.length > 0 || a.withPeople || a.tags.length > 0 || a.people.length > 0 || a.from !== null || a.to !== null || a.kind !== null || a.pickedOnly;

const lower = (s: string) => s.toLocaleLowerCase();
const orList = (xs: string[]) => (xs.length <= 2 ? xs.join(' or ') : `${xs.slice(0, -1).join(', ')} or ${xs[xs.length - 1]}`);

function matchTag(name: string, tags: string[]): { tag?: string; problem?: string } {
  const exact = tags.find((t) => lower(t) === lower(name));
  if (exact) return { tag: exact };
  const part = tags.filter((t) => lower(t).includes(lower(name)));
  if (part.length === 1) return { tag: part[0] };
  if (part.length > 1) return { problem: `“${name}” matches more than one of your tags: ${orList(part.map((t) => `“${t}”`))}.` };
  return { problem: `You have no tag called “${name}”.` };
}

function matchPerson(name: string, people: Known['people']): { id?: number; problem?: string } {
  const named = people.filter((p): p is { id: number; name: string } => p.name !== null);
  const exact = named.find((p) => lower(p.name) === lower(name));
  if (exact) return { id: exact.id };
  const q = lower(name);
  const part = named.filter((p) => lower(p.name).split(/\s+/).includes(q) || lower(p.name).startsWith(q));
  if (part.length === 1) return { id: part[0].id };
  if (part.length > 1) return { problem: `“${name}” could be ${orList(part.map((p) => p.name))}.` };
  return { problem: `I do not know anyone called “${name}”.` };
}

export interface Resolved {
  spec: FindSpec;
  /** The things that must be in the picture, for the content search (each searched on its own). */
  texts: string[];
  problems: string[];
  /** There is something to search for and every name was understood. */
  runnable: boolean;
}

const startOfDay = (d: string) => Date.parse(`${d}T00:00:00Z`) / 1000;

export function resolveFind(a: FindArgs, known: Known): Resolved {
  const problems: string[] = [];
  const tags: string[] = [];
  for (const t of a.tags) {
    const m = matchTag(t, known.tags);
    if (m.tag !== undefined) { if (!tags.includes(m.tag)) tags.push(m.tag); } else problems.push(m.problem!);
  }
  const people: number[] = [];
  for (const p of a.people) {
    const m = matchPerson(p, known.people);
    if (m.id !== undefined) { if (!people.includes(m.id)) people.push(m.id); } else problems.push(m.problem!);
  }
  return {
    spec: {
      kind: a.kind, tags, people, picked: a.pickedOnly, with_people: a.withPeople,
      since: a.from === null ? null : startOfDay(a.from),
      until: a.to === null ? null : startOfDay(a.to) + 86_400,
    },
    texts: [a.text, ...a.also].map((t) => (t ?? '').trim()).filter((t) => t !== ''),
    problems,
    runnable: problems.length === 0 && asksForSomething(a),
  };
}

/** The search in words, in two parts: what is looked for ("videos") and how ("that look like “dog”, with Sam, from 2019-06-01 to 2019-06-30"). */
export function describeFindParts(a: FindArgs): { subject: string; tail: string } {
  const subject = `${a.limit !== null ? `the best ${a.limit} ` : ''}${a.pickedOnly ? 'picked ' : ''}${a.kind === 'video' ? 'videos' : 'photos'}`;
  const parts: string[] = [];
  const things = [a.text, ...a.also].filter((t): t is string => t !== null && t.trim() !== '');
  if (things.length > 0) parts.push(`that look like ${things.map((t) => `“${t}”`).join(' and ')}`);
  if (a.tags.length > 0) parts.push(`tagged ${a.tags.map((t) => `“${t}”`).join(' and ')}`);
  if (a.people.length > 0) parts.push(`with ${a.people.join(' and ')}`);
  if (a.withPeople) parts.push('with people in them');
  if (a.from !== null && a.to !== null) parts.push(`from ${a.from} to ${a.to}`);
  else if (a.from !== null) parts.push(`from ${a.from}`);
  else if (a.to !== null) parts.push(`up to ${a.to}`);
  return { subject, tail: parts.length === 0 ? '' : `${parts[0]}${parts.slice(1).map((p) => `, ${p}`).join('')}` };
}

/** The search in words, for the card: "videos that look like “dog”, with Sam, from 2019-06-01 to 2019-06-30". */
export function describeFind(a: FindArgs): string {
  const { subject, tail } = describeFindParts(a);
  return tail === '' ? subject : `${subject} ${tail}`;
}

/** The weakest score a content search keeps (it matches the backend's floor). CLIP scores a good match around 0.25 to 0.30. */
export const CUT_FLOOR = 0.2;
/** Without a number asked for, the matches kept are those within this much of the best one. */
const CLOSE = 0.03;

/**
 * Where to cut a content search's matches (scores, best first) to start with: exactly `limit` of the best when a number was
 * asked for, otherwise the close matches. The person can move it from there. A search that found nothing cuts at the floor.
 */
export function defaultCut(scores: number[], limit: number | null): number {
  if (scores.length === 0) return CUT_FLOOR;
  if (limit !== null) return scores[Math.min(limit, scores.length) - 1];
  return Math.max(CUT_FLOOR, scores[0] - CLOSE - 1e-6);
}

/** The photos at or above the cut (all of them when the search had no scores). Both lists are best match first. */
export function visibleIds(ids: number[], scores: number[] | null, cut: number): number[] {
  return scores === null ? ids : ids.filter((_, i) => scores[i] >= cut);
}

/** What to say when a search for several things at once found only weak matches: the photos are good for one thing and poor for the other. */
export function weakNote(a: FindArgs): string {
  const things = [a.text, ...a.also].filter((t): t is string => t !== null && t.trim() !== '').map((t) => `“${t}”`);
  const list = things.length <= 2 ? things.join(' and ') : `${things.slice(0, -1).join(', ')} and ${things[things.length - 1]}`;
  return `Few photos, if any, look like ${list} together. These are the closest, but many match only one of them.`;
}
