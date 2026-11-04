import { settingsSchema, type Change, type Proposed, type SettingValues } from './askSettings';
import { GO_TO_TARGETS } from './askGoTo';
import { GUIDE } from './askGuide';

/** Matches the proxy's limits (ask-proxy/src/validate.ts; askGuide.test.ts checks the guide against its size limit). */
export const MAX_TURNS = 6;
export const MAX_TURN_CHARS = 600;

export interface Quota { remaining: number; resetsAt: string }

/** A note for the developer that the model drafted. Nothing is sent until the person has read it and pressed Send. */
export interface Feedback { category: 'feature_request' | 'bug' | 'other'; summary: string }

const CATEGORIES: readonly string[] = ['feature_request', 'bug', 'other'];
const MAX_NOTES = 3;
const MAX_NOTE_CHARS = 500;

/** What the card calls a kind of note. */
export function categoryLabel(c: Feedback['category']): string {
  return c === 'feature_request' ? 'Feature request' : c === 'bug' ? 'Problem' : 'Comment';
}

/** One line of the chat. Errors, cards and buttons are shown but never sent to the model. */
export interface ChatMessage {
  role: 'user' | 'assistant' | 'error';
  text: string;
  /** A settings change the model proposed, once checked, and what became of it. */
  changes?: Change[];
  card?: 'open' | 'applied' | 'cancelled';
  /** Answered by the help built into Pixie (askLocal.ts), not by the model: never sent to the model as something it said. */
  local?: boolean;
  /** A place the model offered a button for. */
  goTo?: string;
  /** Notes for the developer the model drafted, still waiting for the person to confirm (a note leaves this list once it is sent). */
  notes?: Feedback[];
  noteCard?: 'open' | 'sent' | 'declined';
}

const MAX_NOTE = 250;

/**
 * What became of a settings change or a note for the developer the model proposed, in a few words for the model: the model sees only
 * the text of earlier messages, so without this it cannot know whether the person pressed Apply or Send. Empty when there is nothing to say.
 */
export function memoryNote(m: ChatMessage): string {
  const parts: string[] = [];
  if (m.changes && m.card) parts.push(m.card === 'applied' ? `Applied: ${m.changes.map((c) => `${c.label} ${c.from} → ${c.to}`).join('; ')}.` : m.card === 'cancelled' ? 'Not applied.' : 'Waiting for the user to press Apply.');
  if (m.noteCard) parts.push(m.noteCard === 'sent' ? 'Passed on to the developer.' : m.noteCard === 'declined' ? 'Not sent to the developer.' : 'Waiting for the user to confirm passing it on.');
  if (parts.length === 0) return '';
  const note = `[${parts.join(' ')}]`;
  return note.length <= MAX_NOTE ? note : `${note.slice(0, MAX_NOTE - 2)}…]`;
}

/** The last turns the proxy takes: from the user, to the user, nothing else in them. An assistant message carries a note of what
 *  became of its proposal, after its words (the words give way if both do not fit). */
export function turnsToSend(history: ChatMessage[]): { role: 'user' | 'assistant'; text: string }[] {
  const said = history.filter((m): m is ChatMessage & { role: 'user' | 'assistant' } => m.role !== 'error' && m.local !== true && m.text.trim() !== '');
  let turns = said.slice(-MAX_TURNS).map((m) => {
    const note = m.role === 'assistant' ? memoryNote(m) : '';
    return { role: m.role, text: note === '' ? m.text.slice(0, MAX_TURN_CHARS) : `${m.text.slice(0, MAX_TURN_CHARS - note.length - 1)}\n${note}` };
  });
  while (turns.length > 0 && turns[0].role !== 'user') turns = turns.slice(1);
  return turns.length > 0 && turns[turns.length - 1].role === 'user' ? turns : [];
}

export interface Reply { reply: string; proposal: Proposed[]; goTo: string | null; feedback: Feedback[]; quota: Quota | null }

const isObject = (v: unknown): v is Record<string, unknown> => typeof v === 'object' && v !== null && !Array.isArray(v);

function parseQuota(raw: unknown): Quota | null {
  return isObject(raw) && typeof raw.remaining === 'number' && typeof raw.resetsAt === 'string' ? { remaining: raw.remaining, resetsAt: raw.resetsAt } : null;
}

/** The day's questions left, from a 429 answer (or any body that carries them), or null. */
export function quotaOf(body: unknown): Quota | null {
  return isObject(body) ? parseQuota(body.quota) : null;
}

/** What the proxy sent, or null if it is not a reply. Anything odd inside it is left out; nothing is trusted. */
export function parseReply(raw: unknown): Reply | null {
  if (!isObject(raw) || typeof raw.reply !== 'string') return null;
  const proposal = Array.isArray(raw.proposal)
    ? raw.proposal.filter((p): p is Proposed => isObject(p) && typeof p.setting === 'string' && 'value' in p).map((p) => ({ setting: p.setting, value: p.value }))
    : [];
  const goTo = typeof raw.goTo === 'string' && (GO_TO_TARGETS as readonly string[]).includes(raw.goTo) ? raw.goTo : null;
  const feedback = Array.isArray(raw.feedback)
    ? raw.feedback
        .filter((f): f is Feedback => isObject(f) && typeof f.category === 'string' && CATEGORIES.includes(f.category) && typeof f.summary === 'string' && f.summary.trim() !== '')
        .slice(0, MAX_NOTES)
        .map((f) => ({ category: f.category, summary: f.summary.trim().slice(0, MAX_NOTE_CHARS) }))
    : [];
  return { reply: raw.reply, proposal, goTo, feedback, quota: parseQuota(raw.quota) };
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
    const q = quotaOf(body);
    return `You have used all of today's questions. You can ask again ${q ? resetText(q.resetsAt, now) : 'tomorrow'}. Until then, Pixie's built-in help answers below.`;
  }
  if (status === 0) return "Can't reach Ask Pixie right now. Pixie's built-in help answers below.";
  return "I couldn't get an answer just now. Pixie's built-in help answers below.";
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
 * The text that goes to the proxy. It is made from the typed turns, the settings, and what this version of the app says about itself
 * (the guide and the places a button can lead to), so there is nowhere for a picture, a name from the library, a file name or a place to be.
 */
export function buildRequest(history: ChatMessage[], app: { platform: string; language: string; tab: string; today: string }, values: SettingValues): string {
  return JSON.stringify({
    messages: turnsToSend(history),
    context: { platform: app.platform, language: app.language, tab: app.tab, today: app.today, settings: settingsSchema(values), guide: GUIDE, places: [...GO_TO_TARGETS] },
  });
}

/** The text that goes to the proxy when the person confirms a note: the note they read, and their system. Nothing else. */
export function feedbackRequest(note: Feedback, platform: string): string {
  return JSON.stringify({ category: note.category, summary: note.summary, platform });
}

/** The date on the person's own clock as YYYY-MM-DD, so "last summer" can be worked out. */
export function dayOf(d: Date): string {
  const two = (n: number) => String(n).padStart(2, '0');
  return `${d.getFullYear()}-${two(d.getMonth() + 1)}-${two(d.getDate())}`;
}
