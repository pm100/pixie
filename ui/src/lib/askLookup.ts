import { defaultCut, resolveFind, visibleIds, type FindArgs, type FindSpec, type Known } from './askFind';
import type { Library } from './askLibrary';

/**
 * The model asking the app something before it replies ("how many photos of Rose?", "is there a person called Rose?"). The app
 * answers on the person's computer and the answer goes back to the model. Names are answered only if the person shared them
 * (Settings, Ask Pixie); a count says how many, never which.
 */
export interface LookupRequest {
  id: string;
  /** The model's own call, sent back unchanged with the answer. */
  input: Record<string, unknown>;
  what: 'count' | 'tags' | 'people' | 'trips';
  query: string | null;
  filter: FindArgs | null;
}

/** One round: what the model asked, and the text the app answered each question with. */
export interface Round {
  calls: { id: string; input: Record<string, unknown> }[];
  results: string[];
}

/** The most rounds of asking and answering in reply to one question. The proxy takes no more. */
export const MAX_ROUNDS = 3;

const KINDS = ['count', 'tags', 'people', 'trips'] as const;
const CALL_ID = /^[A-Za-z0-9_-]{1,64}$/;
/** What the proxy takes for one answer. */
const MAX_RESULT = 2000;
const MAX_LISTED = 20;

const isObject = (v: unknown): v is Record<string, unknown> => typeof v === 'object' && v !== null && !Array.isArray(v);

/** The questions the proxy sent, those that are well formed. */
export function parseLookups(raw: unknown, parseFilter: (raw: unknown) => FindArgs | null): LookupRequest[] {
  if (!Array.isArray(raw)) return [];
  const out: LookupRequest[] = [];
  for (const r of raw) {
    if (!isObject(r) || typeof r.id !== 'string' || !CALL_ID.test(r.id) || !isObject(r.input)) continue;
    const what = (KINDS as readonly unknown[]).includes(r.what) ? (r.what as LookupRequest['what']) : null;
    if (!what) continue;
    out.push({ id: r.id, input: r.input, what, query: typeof r.query === 'string' ? r.query : null, filter: parseFilter(r.filter) });
  }
  return out.slice(0, MAX_ROUNDS);
}

export interface FindResult { ids: number[]; scores: number[] | null; analysed: number | null }

export interface LookupDeps {
  /** What the person shared, or null if they did not. */
  library: Library | null;
  known: Known;
  find: (texts: string[], spec: FindSpec) => Promise<FindResult>;
}

/** How many photos a search found, as the card would show them: the close matches of a content search, or the number asked for. */
export function countOf(res: FindResult, limit: number | null): number {
  return visibleIds(res.ids, res.scores, defaultCut(res.scores ?? [], limit)).length;
}

/** Fits a list into what the proxy takes by dropping entries from the end. */
function fit(list: unknown[]): string {
  let n = list.length;
  let text = JSON.stringify(list.slice(0, n));
  while (text.length > MAX_RESULT && n > 0) text = JSON.stringify(list.slice(0, --n));
  return text;
}

const has = (name: string | null, query: string | null) => query === null || (name ?? '').toLocaleLowerCase().includes(query.toLocaleLowerCase());

/** The text to send back for one question. Never throws: a failure is an answer in words. */
export async function answerLookup(req: LookupRequest, deps: LookupDeps): Promise<string> {
  if (req.what === 'count') {
    const filter = req.filter;
    try {
      if (filter === null) return JSON.stringify({ count: (await deps.find([], { kind: null, since: null, until: null, tags: [], people: [], picked: false, with_people: false })).ids.length });
      const r = resolveFind(filter, deps.known);
      if (!r.runnable) return JSON.stringify({ error: r.problems[0] ?? 'I could not tell what to count.' });
      return JSON.stringify({ count: countOf(await deps.find(r.texts, r.spec), filter.limit) });
    } catch (e) {
      return JSON.stringify({ error: e instanceof Error ? e.message : String(e) });
    }
  }
  const lib = deps.library;
  if (lib === null) return JSON.stringify({ error: 'The user has not allowed Ask Pixie to see names.' });
  if (req.what === 'tags') return fit(lib.tags.filter((t) => has(t.name, req.query)).slice(0, MAX_LISTED));
  if (req.what === 'people') return fit(lib.people.filter((p) => has(p.name, req.query)).slice(0, MAX_LISTED));
  return fit(
    lib.trips
      .filter((t) => has(t.name, req.query) || has(t.where, req.query) || (req.query !== null && (t.from.includes(req.query) || t.to.includes(req.query))))
      .slice(0, MAX_LISTED)
      .map((t) => ({ name: t.name, from: t.from, to: t.to, where: t.where, nights: t.nights, photos: t.photos })),
  );
}
