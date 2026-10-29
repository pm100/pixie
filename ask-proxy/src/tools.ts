import type { Feedback, FindArgs, LookupRequest, ModelResult, Proposal, ToolCall } from './types';

/** Replies are short, but leave room for a reply plus several tool calls. */
export const MAX_TOKENS = 1024;
const MAX_FEEDBACK_PER_TURN = 3;
const MAX_FEEDBACK_CHARS = 500;
const FEEDBACK_CATEGORIES = new Set(['feature_request', 'bug', 'other']);

/** A tool as plain JSON Schema, so each provider can translate it to its own format. */
export interface ToolSpec {
  name: string;
  description: string;
  schema: Record<string, unknown>;
}

/** The places in the app the model may offer a button for. Mirrored by ui/src/lib/askGoTo.ts (a test compares them). */
export const GO_TO_TARGETS = [
  'browse', 'people', 'duplicates', 'tools-details', 'tools-email', 'tools-print', 'tools-tags',
  'settings-folders', 'settings-viewing', 'settings-browsing', 'settings-trips', 'settings-analysis', 'settings-email', 'settings-about',
] as const;

/** What the model may ask the app about. */
export const LOOKUP_KINDS = ['count', 'tags', 'people', 'trips'] as const;
const MAX_LOOKUPS = 3;

export const TOOL_SPECS: ToolSpec[] = [
  {
    name: 'propose_settings_changes',
    description:
      'Propose one or more settings changes for the user to review and confirm. Nothing is changed until they confirm. Always write a sentence to the user in the same reply, saying what you suggest and why.',
    schema: {
      type: 'object',
      properties: {
        changes: {
          type: 'array',
          description: 'The settings to change, in the order they should be applied.',
          items: {
            type: 'object',
            properties: {
              setting: { type: 'string', description: 'A setting key from the schema.' },
              // No "type" here on purpose: it is a number or a boolean, and unions are the part
              // of JSON Schema that providers disagree on most.
              value: {
                description:
                  'The new value: the option value (a number) for choice settings, true or false for bool settings, a number for int and float settings.',
              },
            },
            required: ['setting', 'value'],
          },
        },
      },
      required: ['changes'],
    },
  },
  {
    name: 'go_to',
    description:
      'Offer the user a button that takes them to a place in the app, for a "how do I..." question or after a proposal. Nothing happens until they press it. Always write a sentence to the user in the same reply saying what is there.',
    schema: {
      type: 'object',
      properties: {
        target: { type: 'string', enum: [...GO_TO_TARGETS] },
        say: { type: 'string', description: 'The answer for the user, in their language: one to three short sentences, plain text. It is shown next to the button.' },
      },
      required: ['target', 'say'],
    },
  },
  {
    name: 'find_photos',
    description:
      "Look for photos or videos among the user's own and offer to do something with them. Use it when the user wants to find, pick, tag, email or print photos by what is in them, by a tag, a person, a date or a kind. Pixie runs the search on the user's computer, shows how many it found, and only then asks before it picks or tags anything. Always give the sentence for the user in say.",
    schema: {
      type: 'object',
      properties: {
        text: { type: 'string', description: 'What is in the pictures, in a few plain words (for example "pink flowers", "dog on a beach", "birthday cake"). Used for a content search, which is approximate.' },
        also: { type: 'array', items: { type: 'string' }, description: 'More things that must also be in the same picture, each a few plain words (for example "a birthday cake"). Each is searched for on its own, and a photo must match all of them and text. Do not use it for people: use with_people or people.' },
        with_people: { type: 'boolean', description: 'Only photos with a person in them, whoever. Use this for "with people", "of people", "with someone in them"; never put "people" in text or also.' },
        tags: { type: 'array', items: { type: 'string' }, description: 'Names of tags the user gave their photos, only when the user names them. All must match.' },
        people: { type: 'array', items: { type: 'string' }, description: 'Names of people, as the user said them. All must be in the photo.' },
        from: { type: 'string', description: 'The first day, as YYYY-MM-DD.' },
        to: { type: 'string', description: 'The last day, as YYYY-MM-DD.' },
        kind: { type: 'string', enum: ['photo', 'video'] },
        limit: { type: 'integer', description: 'How many photos the user asked for ("find 20 pictures", "the best 5"): the best matches, at most this many. Leave out when they ask for all of them.' },
        picked_only: { type: 'boolean', description: 'Only look among the photos already picked.' },
        then: { type: 'array', items: { type: 'string', enum: ['pick', 'tag', 'email', 'print'] }, description: 'What the user wants done with what is found, in order. Leave out to only show them.' },
        tag: { type: 'string', description: 'The name of the tag to give them, when then includes tag.' },
        say: { type: 'string', description: 'One or two sentences for the user, in their language, saying what you will look for and do.' },
      },
      required: ['say'],
    },
  },
  {
    name: 'lookup',
    description:
      "Ask Pixie's app something and get the answer back before you reply, when you need a fact you cannot see: how many photos match something, or whether the user has a tag, a person or a trip (only when the library is shown to you, and only for what the LIBRARY section does not already tell you). Use it sparingly, at most three at a time, and never to guess. After the answer, write your reply as usual.",
    schema: {
      type: 'object',
      properties: {
        what: { type: 'string', enum: [...LOOKUP_KINDS], description: 'count: how many photos match the filter below (no filter: all of them). tags, people, trips: the user\'s own whose name contains query.' },
        query: { type: 'string', description: 'For tags, people and trips: part of a name to look for.' },
        text: { type: 'string', description: 'For count: what is in the pictures.' },
        also: { type: 'array', items: { type: 'string' }, description: 'For count: more things that must be in the picture.' },
        with_people: { type: 'boolean', description: 'For count: only photos with a person in them.' },
        tags: { type: 'array', items: { type: 'string' }, description: 'For count: names of the user\'s tags.' },
        people: { type: 'array', items: { type: 'string' }, description: 'For count: names of people.' },
        from: { type: 'string', description: 'For count: the first day, YYYY-MM-DD.' },
        to: { type: 'string', description: 'For count: the last day, YYYY-MM-DD.' },
        kind: { type: 'string', enum: ['photo', 'video'] },
        picked_only: { type: 'boolean' },
      },
      required: ['what'],
    },
  },
  {
    name: 'submit_feedback',
    description:
      'Record a feature request, bug report or other comment about the app for the developer. Use for things no setting can do. Always write a sentence to the user in the same reply, saying you passed it on.',
    schema: {
      type: 'object',
      properties: {
        category: { type: 'string', enum: ['feature_request', 'bug', 'other'] },
        summary: { type: 'string', description: 'One or two sentences capturing what the user wants or reported.' },
      },
      required: ['category', 'summary'],
    },
  },
];

/** Accepts what a model might reasonably send for a value: a number, a boolean, or either as text. */
function coerceValue(value: unknown): number | boolean | null {
  if (typeof value === 'number' && Number.isFinite(value)) return value;
  if (typeof value === 'boolean') return value;
  if (typeof value === 'string') {
    const trimmed = value.trim();
    if (trimmed.toLowerCase() === 'true') return true;
    if (trimmed.toLowerCase() === 'false') return false;
    if (trimmed !== '' && Number.isFinite(Number(trimmed))) return Number(trimmed);
  }
  return null;
}

function parseChanges(input: unknown): Proposal[] {
  const changes = (input as { changes?: unknown } | null)?.changes;
  if (!Array.isArray(changes)) return [];
  const proposals: Proposal[] = [];
  for (const change of changes) {
    const { setting, value } = (change ?? {}) as { setting?: unknown; value?: unknown };
    const coerced = coerceValue(value);
    if (typeof setting === 'string' && coerced !== null) proposals.push({ setting, value: coerced });
  }
  return proposals;
}

const FIND_THEN = ['pick', 'tag', 'email', 'print'] as const;
const DAY = /^\d{4}-\d{2}-\d{2}$/;
const words = (v: unknown, max: number, each: number): string[] =>
  Array.isArray(v) ? v.filter((x): x is string => typeof x === 'string' && x.trim() !== '').map((x) => x.trim().slice(0, each)).slice(0, max) : [];

/** The search the model asked for, reduced to what is well-formed; null when it names nothing to look for. */
function parseFind(input: unknown): { find: FindArgs; say: string } | null {
  const o = (input ?? {}) as Record<string, unknown>;
  const text = typeof o.text === 'string' && o.text.trim() !== '' ? o.text.trim().slice(0, 200) : null;
  const also = words(o.also, 3, 200);
  const withPeople = o.with_people === true;
  const tags = words(o.tags, 10, 60);
  const people = words(o.people, 10, 60);
  const day = (v: unknown) => (typeof v === 'string' && DAY.test(v) && !Number.isNaN(Date.parse(v)) ? v : null);
  const from = day(o.from), to = day(o.to);
  const kind = o.kind === 'photo' || o.kind === 'video' ? o.kind : null;
  const pickedOnly = o.picked_only === true;
  const limit = typeof o.limit === 'number' && Number.isFinite(o.limit) && Math.floor(o.limit) >= 1 && Math.floor(o.limit) <= 500 ? Math.floor(o.limit) : null;
  const tag = typeof o.tag === 'string' && o.tag.trim() !== '' ? o.tag.trim().slice(0, 60) : null;
  const then = (Array.isArray(o.then) ? o.then : [])
    .filter((x): x is (typeof FIND_THEN)[number] => (FIND_THEN as readonly unknown[]).includes(x) && (x !== 'tag' || tag !== null))
    .filter((x, i, all) => all.indexOf(x) === i);
  if (text === null && also.length === 0 && !withPeople && tags.length === 0 && people.length === 0 && from === null && to === null && kind === null && !pickedOnly) return null;
  const say = typeof o.say === 'string' ? o.say.trim().slice(0, 1000) : '';
  return { find: { text, also, withPeople, tags, people, from, to, kind, limit, pickedOnly, then, tag }, say };
}

/** A question to the app, rebuilt from what the model sent; null if it is not one the app could answer. */
function parseLookup(call: ToolCall): LookupRequest | null {
  const o = (call.input ?? {}) as Record<string, unknown>;
  const what = (LOOKUP_KINDS as readonly unknown[]).includes(o.what) ? (o.what as LookupRequest['what']) : null;
  if (!what || typeof call.id !== 'string' || call.id === '') return null;
  const query = typeof o.query === 'string' && o.query.trim() !== '' ? o.query.trim().slice(0, 80) : null;
  const filter = what === 'count' ? (parseFind({ ...o, say: '' })?.find ?? null) : null;
  return { id: call.id, input: o, what, query, filter };
}

function parseFeedback(input: unknown): Feedback | null {
  const { category, summary } = (input ?? {}) as { category?: unknown; summary?: unknown };
  if (typeof category !== 'string' || !FEEDBACK_CATEGORIES.has(category)) return null;
  if (typeof summary !== 'string' || !summary.trim()) return null;
  return { category, summary: summary.trim().slice(0, MAX_FEEDBACK_CHARS) };
}

/**
 * Turns what any provider returned (text blocks plus tool calls) into the service's result.
 * Only well-formed, capped items get through; unknown tools are ignored.
 */
export function interpret(texts: string[], calls: ToolCall[]): ModelResult {
  const proposals: Proposal[] = [];
  const feedback: Feedback[] = [];
  let goTo: string | null = null;
  let find: FindArgs | null = null;
  let say = '';
  const lookup: LookupRequest[] = [];
  for (const call of calls) {
    if (call.name === 'propose_settings_changes') {
      proposals.push(...parseChanges(call.input));
    } else if (call.name === 'go_to') {
      const { target, say: said } = (call.input ?? {}) as { target?: unknown; say?: unknown };
      if (goTo === null && typeof target === 'string' && (GO_TO_TARGETS as readonly string[]).includes(target)) {
        goTo = target;
        if (typeof said === 'string') say = said.trim().slice(0, 1000);
      }
    } else if (call.name === 'find_photos') {
      const parsed: { find: FindArgs; say: string } | null = find === null ? parseFind(call.input) : null;
      if (parsed) {
        find = parsed.find;
        if (parsed.say.length > say.length) say = parsed.say;
      }
    } else if (call.name === 'lookup') {
      const asked = lookup.length < MAX_LOOKUPS ? parseLookup(call) : null;
      if (asked) lookup.push(asked);
    } else if (call.name === 'submit_feedback' && feedback.length < MAX_FEEDBACK_PER_TURN) {
      const item = parseFeedback(call.input);
      if (item) feedback.push(item);
    }
  }
  const text = texts.join('\n').trim();
  return {
    // Models often make the call and write little or nothing beside it (a lead-in ending in a colon), so the answer travels
    // inside the call as well, and the fuller of the two is shown.
    reply: say.length > text.length ? say : text,
    // While the model waits for an answer it does nothing else: what it asked for is decided after the answer comes back.
    proposal: lookup.length === 0 && proposals.length > 0 ? proposals : null,
    goTo: lookup.length === 0 ? goTo : null,
    find: lookup.length === 0 ? find : null,
    lookup,
    feedback,
  };
}

export const REFUSAL_RESULT: ModelResult = { reply: "Sorry, I can't help with that one.", proposal: null, goTo: null, find: null, lookup: [], feedback: [] };
