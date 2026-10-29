import { settingsSchema, type Change, type Proposed, type SettingValues } from './askSettings';
import { GO_TO_TARGETS } from './askGoTo';
import { parseFindArgs, type FindArgs } from './askFind';
import type { Photo } from './api';
import type { Library } from './askLibrary';
import { parseLookups, type LookupRequest, type Round } from './askLookup';

/** Matches the proxy's limits (ask-proxy/src/validate.ts). */
export const MAX_TURNS = 6;
export const MAX_TURN_CHARS = 600;

export interface Quota { remaining: number; resetsAt: string }

/** One line of the chat. Errors, cards and buttons are shown but never sent to the model. */
export interface ChatMessage {
  role: 'user' | 'assistant' | 'error';
  text: string;
  /** A settings change the model proposed, once checked, and what became of it. */
  changes?: Change[];
  card?: 'open' | 'applied' | 'cancelled';
  /** A place the model offered a button for. */
  goTo?: string;
  /** A search the model asked for, and how it went. */
  found?: FoundState;
}

/** One search in the chat. `ids` are the photos found; `done` lists what has been done with them. */
export interface FoundState {
  args: FindArgs;
  status: 'running' | 'done' | 'problem' | 'failed';
  /** The reasons it was not run, or why it failed. */
  problems: string[];
  /** The photos shown and acted on: those of `allIds` at or above `cut`. */
  ids: number[];
  /** Everything the search found, best match first for a content search. */
  allIds: number[];
  /** For a content search, each photo's score in the order of `allIds`; the person moves `cut` to see more or fewer. */
  scores: number[] | null;
  cut: number;
  /** The first few, to show. */
  preview: Photo[];
  /** Photos analysed so far, for a content search: it can only look at those. */
  analysed: number | null;
  /** What has been done with them: the buttons already pressed. */
  done: { picked?: number; tagged?: { tag: string; ids: number[] }; opened?: string };
}

/** The last turns the proxy takes: from the user, to the user, nothing else in them. */
export function turnsToSend(history: ChatMessage[]): { role: 'user' | 'assistant'; text: string }[] {
  const said = history.filter((m): m is ChatMessage & { role: 'user' | 'assistant' } => m.role !== 'error' && m.text.trim() !== '');
  let turns = said.slice(-MAX_TURNS).map((m) => ({ role: m.role, text: m.text.slice(0, MAX_TURN_CHARS) }));
  while (turns.length > 0 && turns[0].role !== 'user') turns = turns.slice(1);
  return turns.length > 0 && turns[turns.length - 1].role === 'user' ? turns : [];
}

export interface Reply { reply: string; proposal: Proposed[]; goTo: string | null; find: FindArgs | null; lookup: LookupRequest[]; feedbackSent: boolean; quota: Quota | null }

const isObject = (v: unknown): v is Record<string, unknown> => typeof v === 'object' && v !== null && !Array.isArray(v);

function parseQuota(raw: unknown): Quota | null {
  return isObject(raw) && typeof raw.remaining === 'number' && typeof raw.resetsAt === 'string' ? { remaining: raw.remaining, resetsAt: raw.resetsAt } : null;
}

/** What the proxy sent, or null if it is not a reply. Anything odd inside it is left out; nothing is trusted. */
export function parseReply(raw: unknown): Reply | null {
  if (!isObject(raw) || typeof raw.reply !== 'string') return null;
  const proposal = Array.isArray(raw.proposal)
    ? raw.proposal.filter((p): p is Proposed => isObject(p) && typeof p.setting === 'string' && 'value' in p).map((p) => ({ setting: p.setting, value: p.value }))
    : [];
  const goTo = typeof raw.goTo === 'string' && (GO_TO_TARGETS as readonly string[]).includes(raw.goTo) ? raw.goTo : null;
  return { reply: raw.reply, proposal, goTo, find: parseFindArgs(raw.find), lookup: parseLookups(raw.lookup, parseFindArgs), feedbackSent: raw.feedbackSent === true, quota: parseQuota(raw.quota) };
}

/** When the day's questions come back, in the person's own words and clock: "later today at 5:00 PM", "tomorrow at 8:00 AM". */
export function resetText(resetsAt: string, now: Date): string {
  const at = new Date(resetsAt);
  if (Number.isNaN(at.getTime())) return 'tomorrow';
  const time = at.toLocaleTimeString(undefined, { hour: 'numeric', minute: '2-digit' });
  const days = Math.round((new Date(at.getFullYear(), at.getMonth(), at.getDate()).getTime() - new Date(now.getFullYear(), now.getMonth(), now.getDate()).getTime()) / 86_400_000);
  if (days <= 0) return `later today at ${time}`;
  if (days === 1) return `tomorrow at ${time}`;
  return `${at.toLocaleDateString(undefined, { weekday: 'long' })} at ${time}`;
}

/** What to say when no reply came: `status` is the HTTP status, 0 for no connection and -1 when this build has no address for it. */
export function errorText(status: number, body: unknown, now: Date = new Date()): string {
  if (status === -1) return 'Ask Pixie is not set up in this build.';
  if (status === 429) {
    const q = isObject(body) ? parseQuota(body.quota) : null;
    return `You have used all of today's questions. You can ask again ${q ? resetText(q.resetsAt, now) : 'tomorrow'}. Until then, everything Ask Pixie does can also be done by hand in Pixie.`;
  }
  if (status === 0) return "Can't reach Ask Pixie right now. Check your connection and try again.";
  return "I couldn't get an answer just now. Please try again.";
}

export function newInstallId(): string {
  return globalThis.crypto.randomUUID();
}

/** The system, from the browser string: the model is told it so that it names the right keys and menus. */
export function platformName(userAgent: string): 'windows' | 'mac' | 'linux' | 'other' {
  if (/Windows/i.test(userAgent)) return 'windows';
  if (/Mac/i.test(userAgent)) return 'mac';
  if (/Linux|X11/i.test(userAgent)) return 'linux';
  return 'other';
}

/**
 * The text that goes to the proxy. It is made from the typed turns, the settings, and (only when the person allowed it) the names
 * in the library and the answers to the model's own questions, so there is nowhere for a picture, a file name or a place to be.
 */
export function buildRequest(history: ChatMessage[], app: { platform: string; language: string; tab: string; today: string }, values: SettingValues, library: Library | null = null, rounds: Round[] = []): string {
  return JSON.stringify({
    messages: turnsToSend(history),
    context: { platform: app.platform, language: app.language, tab: app.tab, today: app.today, settings: settingsSchema(values), ...(library ? { library } : {}) },
    ...(rounds.length > 0 ? { lookups: rounds } : {}),
  });
}

/** The date on the person's own clock as YYYY-MM-DD, so "last summer" can be worked out. */
export function dayOf(d: Date): string {
  const two = (n: number) => String(n).padStart(2, '0');
  return `${d.getFullYear()}-${two(d.getMonth() + 1)}-${two(d.getDate())}`;
}
