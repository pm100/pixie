import type { Feedback, ModelResult, Proposal, ToolCall } from './types';

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
  let say = '';
  for (const call of calls) {
    if (call.name === 'propose_settings_changes') {
      proposals.push(...parseChanges(call.input));
    } else if (call.name === 'go_to') {
      const { target, say: said } = (call.input ?? {}) as { target?: unknown; say?: unknown };
      if (goTo === null && typeof target === 'string' && (GO_TO_TARGETS as readonly string[]).includes(target)) {
        goTo = target;
        if (typeof said === 'string') say = said.trim().slice(0, 1000);
      }
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
    proposal: proposals.length > 0 ? proposals : null,
    goTo,
    feedback,
  };
}

export const REFUSAL_RESULT: ModelResult = { reply: "Sorry, I can't help with that one.", proposal: null, goTo: null, feedback: [] };
