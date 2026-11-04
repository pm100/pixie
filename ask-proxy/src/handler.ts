import { storeFeedback } from './feedback';
import { parseFeedback } from './tools';
import { dayKey, limitFor, readQuota, recordUse } from './quota';
import type { Env, ModelProvider } from './types';
import { isInstallId, MAX_BODY_BYTES, parseAsk } from './validate';

export interface Deps {
  provider: ModelProvider;
  now: () => Date;
}

/** Shown when the model answered with only a tool call, so the user never sees a blank reply. */
export const FEEDBACK_FALLBACK_REPLY = 'I can pass that on to the developer if you like.';
export const PROPOSAL_FALLBACK_REPLY = "Here's what I suggest.";
export const GO_TO_FALLBACK_REPLY = "Here's where to find it.";

/**
 * Is this the dev key the server holds? Debug builds of Pixie send it, and are not limited. The server decides: a request that
 * only says it is a dev build gets nothing. No key held, or an empty one, never matches.
 */
export function isDevKey(sent: string | null, held: string | undefined): boolean {
  if (!held || sent === null || sent.length !== held.length) return false;
  let diff = 0;
  for (let i = 0; i < held.length; i++) diff |= sent.charCodeAt(i) ^ held.charCodeAt(i); // not decided by where they first differ
  return diff === 0;
}

function json(status: number, body: unknown): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { 'Content-Type': 'application/json', 'Cache-Control': 'no-store' },
  });
}

/**
 * POST /v1/ask. The model is asked before the question is counted, so an upstream failure
 * costs the user nothing. Feedback the model drafts is only passed back: the app shows it to the person and sends it on
 * (POST /v1/feedback) if they confirm.
 */
export async function handleAsk(request: Request, env: Env, deps: Deps): Promise<Response> {
  const installId = request.headers.get('X-Install-Id');
  if (!isInstallId(installId)) return json(400, { error: 'bad_install_id' });

  const text = await request.text();
  if (text.length > MAX_BODY_BYTES) return json(413, { error: 'too_large' });
  let raw: unknown;
  try {
    raw = JSON.parse(text);
  } catch {
    return json(400, { error: 'bad_request' });
  }
  const parsed = parseAsk(raw);
  if (!parsed.ok) return json(400, { error: 'bad_request', detail: parsed.error });
  const { body } = parsed;

  const now = deps.now();
  const limit = limitFor(env);
  const dev = isDevKey(request.headers.get('X-Dev-Key'), env.DEV_KEY);
  const { used, quota } = await readQuota(env.QUOTA, installId, limit, now);
  if (!dev && used >= limit) return json(429, { error: 'quota_exceeded', quota });

  let result;
  try {
    result = await deps.provider.ask(body);
  } catch (error) {
    // Log only the class, status and (for misconfiguration) our own message: never the prompt,
    // the user's text or a key.
    const { name, message, status } = error as { name?: string; message?: string; status?: number };
    const detail = message?.startsWith('provider misconfigured') ? ` ${message}` : '';
    console.error(`model call failed: ${name ?? 'Error'} status=${status ?? 'n/a'}${detail}`);
    return json(502, { error: 'upstream' });
  }

  if (!dev) await recordUse(env.QUOTA, installId, used, now);

  let reply = result.reply;
  if (!reply.trim()) {
    if (result.feedback.length > 0) reply = FEEDBACK_FALLBACK_REPLY;
    else if (result.proposal) reply = PROPOSAL_FALLBACK_REPLY;
    else if (result.goTo) reply = GO_TO_FALLBACK_REPLY;
  }

  return json(200, {
    reply,
    proposal: result.proposal,
    goTo: result.goTo,
    // Drafts only: nothing is stored until the person confirms in the app, which then calls /v1/feedback.
    feedback: result.feedback,
    quota: { remaining: dev ? limit : Math.max(0, limit - (used + 1)), resetsAt: quota.resetsAt },
  });
}

/** Notes one install may send the developer per UTC day (the app only sends what the person confirmed, so this is a guard, not a limit anyone meets). */
export const MAX_FEEDBACK_PER_DAY = 5;
const MAX_FEEDBACK_BODY = 2048;

/**
 * POST /v1/feedback: stores one note the person confirmed in the app (the model only drafts them). Body {category, summary, platform}.
 */
export async function handleFeedback(request: Request, env: Env, deps: Pick<Deps, 'now'>): Promise<Response> {
  const installId = request.headers.get('X-Install-Id');
  if (!isInstallId(installId)) return json(400, { error: 'bad_install_id' });
  const text = await request.text();
  if (text.length > MAX_FEEDBACK_BODY) return json(413, { error: 'too_large' });
  let raw: unknown;
  try {
    raw = JSON.parse(text);
  } catch {
    return json(400, { error: 'bad_request' });
  }
  const note = parseFeedback(raw);
  const platform = (raw as { platform?: unknown } | null)?.platform;
  if (!note || typeof platform !== 'string' || platform === '' || platform.length > 16) return json(400, { error: 'bad_request' });

  const now = deps.now();
  const counter = `fb:${installId}:${dayKey(now)}`;
  const stored = Number.parseInt((await env.QUOTA.get(counter)) ?? '0', 10);
  const count = Number.isInteger(stored) && stored > 0 ? stored : 0;
  if (count >= MAX_FEEDBACK_PER_DAY) return json(429, { error: 'too_many' });
  try {
    await storeFeedback(env.FEEDBACK, { at: now.toISOString(), installId, platform, category: note.category, summary: note.summary });
  } catch (error) {
    console.error(`feedback store failed: ${(error as Error).name}`);
    return json(500, { error: 'store_failed' });
  }
  await env.QUOTA.put(counter, String(count + 1), { expirationTtl: 172_800 });
  return json(200, { ok: true });
}
