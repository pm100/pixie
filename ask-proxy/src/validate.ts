import type { AskBody, Context, PhotoInfo, SettingInfo, Turn } from './types';

export const MAX_BODY_BYTES = 65_536;
export const MAX_TURNS = 6;
export const MAX_TURN_CHARS = 600;
export const MAX_SETTINGS = 40;
export const MAX_GUIDE_CHARS = 20_000;
export const MAX_PLACES = 40;
export const MAX_HINT_CHARS = 1500;

const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

/** A place is a short lower-case name, words joined by hyphens: send-email. */
const PLACE = /^[a-z]+(-[a-z]+)*$/;

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

/** The optional photo counts. Anything odd is left out rather than failing the question, so apps of other versions keep working. */
function parsePhotos(raw: unknown): PhotoInfo | null {
  if (!isObject(raw)) return null;
  const picked = num(raw.picked);
  const { chosen, workingOn } = raw;
  if (picked === null || !Number.isInteger(picked) || picked < 0 || picked > 10_000_000 || typeof chosen !== 'boolean') return null;
  if (workingOn !== 'picked' && workingOn !== 'current' && workingOn !== 'tag') return null;
  const hint = typeof raw.hint === 'string' && raw.hint.length <= MAX_HINT_CHARS && raw.hint.trim() !== '' ? raw.hint.trim() : undefined;
  const videos = num(raw.pickedVideos);
  const pickedVideos = videos !== null && Number.isInteger(videos) && videos >= 0 && videos <= picked ? videos : undefined;
  return { picked, ...(pickedVideos !== undefined ? { pickedVideos } : {}), chosen, ...(typeof raw.chosenVideo === 'boolean' ? { chosenVideo: raw.chosenVideo } : {}), workingOn, ...(hint ? { hint } : {}) };
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
  const guide = str(raw.guide, MAX_GUIDE_CHARS);
  if (!guide || !guide.trim()) return null;
  if (!Array.isArray(raw.places) || raw.places.length < 1 || raw.places.length > MAX_PLACES) return null;
  const places: string[] = [];
  for (const place of raw.places) {
    if (typeof place !== 'string' || !PLACE.test(place) || places.includes(place)) return null;
    places.push(place);
  }
  const photos = parsePhotos(raw.photos);
  return { platform, language, tab, today, settings, ...(photos ? { photos } : {}), guide, places };
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
  return { ok: true, body: { messages, context } };
}
