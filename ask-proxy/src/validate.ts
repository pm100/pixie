import type { AskBody, Context, Library, LookupRound, Results, SettingInfo, Turn } from './types';

export const MAX_BODY_BYTES = 32_768;
export const MAX_TURNS = 6;
export const MAX_TURN_CHARS = 600;
export const MAX_SETTINGS = 40;

const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

export function isInstallId(value: string | null): value is string {
  return value !== null && UUID.test(value);
}

type Parsed = { ok: true; body: AskBody } | { ok: false; error: string };

const isObject = (v: unknown): v is Record<string, unknown> =>
  typeof v === 'object' && v !== null && !Array.isArray(v);
const str = (v: unknown, max: number): string | null =>
  typeof v === 'string' && v.length <= max ? v : null;
const num = (v: unknown): number | null => (typeof v === 'number' && Number.isFinite(v) ? v : null);

function parseSetting(raw: unknown): SettingInfo | null {
  if (!isObject(raw)) return null;
  const key = str(raw.key, 40);
  const label = str(raw.label, 80);
  const description = str(raw.description, 1000);
  const type = raw.type;
  if (!key || !label || description === null) return null;
  if (type !== 'choice' && type !== 'int' && type !== 'float' && type !== 'bool') return null;
  const setting: SettingInfo = { key, label, description, type };
  if (raw.options !== undefined) {
    if (!Array.isArray(raw.options) || raw.options.length > 20) return null;
    setting.options = [];
    for (const option of raw.options) {
      if (!isObject(option)) return null;
      const value = num(option.value);
      const optionLabel = str(option.label, 60);
      if (value === null || !optionLabel) return null;
      setting.options.push({ value, label: optionLabel });
    }
  }
  for (const field of ['min', 'max', 'step'] as const) {
    if (raw[field] !== undefined) {
      const value = num(raw[field]);
      if (value === null) return null;
      setting[field] = value;
    }
  }
  for (const field of ['current', 'default'] as const) {
    const value = raw[field];
    if (value === undefined) continue;
    if (typeof value !== 'boolean' && num(value) === null) return null;
    setting[field] = value as number | boolean;
  }
  if (raw.currentLabel !== undefined) {
    const label2 = str(raw.currentLabel, 60);
    if (label2 === null) return null;
    setting.currentLabel = label2;
  }
  return setting;
}

export const MAX_LIBRARY_TAGS = 150;
export const MAX_LIBRARY_PEOPLE = 100;
export const MAX_LIBRARY_TRIPS = 40;
const DAY = /^\d{4}-\d{2}-\d{2}$/;

function named(raw: unknown, max: number): { name: string; count: number }[] | null {
  if (!Array.isArray(raw) || raw.length > max) return null;
  const out: { name: string; count: number }[] = [];
  for (const item of raw) {
    if (!isObject(item)) return null;
    const name = str(item.name, 80);
    const count = num(item.count);
    if (!name || count === null) return null;
    out.push({ name, count });
  }
  return out;
}

/** The library the user allowed, rebuilt field by field; null if anything in it is not what it should be. */
function parseLibrary(raw: unknown): Library | null {
  if (!isObject(raw)) return null;
  const photos = num(raw.photos), picked = num(raw.picked);
  const tags = named(raw.tags, MAX_LIBRARY_TAGS), people = named(raw.people, MAX_LIBRARY_PEOPLE);
  if (photos === null || picked === null || !tags || !people) return null;
  if (!Array.isArray(raw.trips) || raw.trips.length > MAX_LIBRARY_TRIPS) return null;
  const trips: Library['trips'] = [];
  for (const t of raw.trips) {
    if (!isObject(t)) return null;
    const from = str(t.from, 10), to = str(t.to, 10);
    const nights = num(t.nights), tp = num(t.photos), tv = num(t.videos);
    const name = t.name === null || t.name === undefined ? null : str(t.name, 80);
    const where = t.where === null || t.where === undefined ? null : str(t.where, 80);
    if (!from || !to || !DAY.test(from) || !DAY.test(to) || nights === null || tp === null || tv === null) return null;
    if ((t.name != null && name === null) || (t.where != null && where === null)) return null;
    trips.push({ name, from, to, nights, photos: tp, videos: tv, where });
  }
  return { photos, picked, tags, people, trips };
}

/** The current results, rebuilt field by field; null if it is not what it should be. */
function parseResults(raw: unknown): Results | null {
  if (!isObject(raw)) return null;
  const count = num(raw.count);
  const of = str(raw.of, 300);
  return count === null || count < 0 || !of ? null : { count, of };
}

function parseContext(raw: unknown): Context | null {
  if (!isObject(raw)) return null;
  const platform = str(raw.platform, 16);
  const language = str(raw.language, 20);
  const tab = str(raw.tab, 20);
  const today = str(raw.today, 10);
  if (!platform || !language || !tab || !today || !/^\d{4}-\d{2}-\d{2}$/.test(today) || Number.isNaN(Date.parse(today))) return null;
  if (!Array.isArray(raw.settings) || raw.settings.length < 1 || raw.settings.length > MAX_SETTINGS) return null;
  const settings: SettingInfo[] = [];
  for (const entry of raw.settings) {
    const setting = parseSetting(entry);
    if (!setting) return null;
    settings.push(setting);
  }
  const context: Context = { platform, language, tab, today, settings };
  if (raw.library !== undefined) {
    const library = parseLibrary(raw.library);
    if (!library) return null;
    context.library = library;
  }
  if (raw.results !== undefined) {
    const results = parseResults(raw.results);
    if (!results) return null;
    context.results = results;
  }
  return context;
}

export const MAX_ROUNDS = 3;
const MAX_CALLS = 3;
const MAX_INPUT_CHARS = 2000;
export const MAX_RESULT_CHARS = 2000;
const CALL_ID = /^[A-Za-z0-9_-]{1,64}$/;

/** The rounds of the model asking the app and being answered; null if anything in them is not what it should be. */
function parseRounds(raw: unknown): LookupRound[] | null {
  if (!Array.isArray(raw) || raw.length > MAX_ROUNDS) return null;
  const rounds: LookupRound[] = [];
  for (const r of raw) {
    if (!isObject(r) || !Array.isArray(r.calls) || !Array.isArray(r.results)) return null;
    if (r.calls.length < 1 || r.calls.length > MAX_CALLS || r.results.length !== r.calls.length) return null;
    const calls: LookupRound['calls'] = [];
    for (const c of r.calls) {
      if (!isObject(c) || typeof c.id !== 'string' || !CALL_ID.test(c.id) || !isObject(c.input)) return null;
      if (JSON.stringify(c.input).length > MAX_INPUT_CHARS) return null;
      calls.push({ id: c.id, input: c.input });
    }
    const results: string[] = [];
    for (const t of r.results) {
      const text = str(t, MAX_RESULT_CHARS);
      if (text === null) return null;
      results.push(text);
    }
    rounds.push({ calls, results });
  }
  return rounds;
}

/** Validates and rebuilds the request body, keeping only known fields within size limits. */
export function parseAsk(raw: unknown): Parsed {
  if (!isObject(raw)) return { ok: false, error: 'body must be an object' };
  if (!Array.isArray(raw.messages) || raw.messages.length < 1 || raw.messages.length > MAX_TURNS) {
    return { ok: false, error: `messages must hold 1-${MAX_TURNS} turns` };
  }
  const messages: Turn[] = [];
  for (const turn of raw.messages) {
    if (!isObject(turn)) return { ok: false, error: 'bad turn' };
    const text = str(turn.text, MAX_TURN_CHARS);
    if ((turn.role !== 'user' && turn.role !== 'assistant') || !text || !text.trim()) {
      return { ok: false, error: 'bad turn' };
    }
    messages.push({ role: turn.role, text });
  }
  if (messages[0]?.role !== 'user' || messages[messages.length - 1]?.role !== 'user') {
    return { ok: false, error: 'conversation must start and end with a user turn' };
  }
  const context = parseContext(raw.context);
  if (!context) return { ok: false, error: 'bad context' };
  if (raw.lookups === undefined) return { ok: true, body: { messages, context } };
  const lookups = parseRounds(raw.lookups);
  return lookups ? { ok: true, body: { messages, context, lookups } } : { ok: false, error: 'bad lookups' };
}
