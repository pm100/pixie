import { storeFeedback } from './feedback';
import { limitFor, readQuota, recordUse } from './quota';
import type { Env, ModelProvider } from './types';
import { isInstallId, MAX_BODY_BYTES, parseAsk } from './validate';

export interface Deps {
  provider: ModelProvider;
  now: () => Date;
}

/** Shown when the model answered with only a tool call, so the user never sees a blank reply. */
export const FEEDBACK_FALLBACK_REPLY = "I've passed that on to the developer.";
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
 * costs the user nothing. Feedback storage failures never fail the request.
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

  let stored = 0;
  for (const item of result.feedback) {
    try {
      await storeFeedback(env.FEEDBACK, {
        at: now.toISOString(),
        installId,
        platform: body.context.platform,
        category: item.category,
        summary: item.summary,
      });
      stored++;
    } catch (error) {
      console.error(`feedback store failed: ${(error as Error).name}`);
    }
  }

  let reply = result.reply;
  if (!reply.trim()) {
    if (stored > 0) reply = FEEDBACK_FALLBACK_REPLY;
    else if (result.proposal) reply = PROPOSAL_FALLBACK_REPLY;
    else if (result.goTo) reply = GO_TO_FALLBACK_REPLY;
  }

  return json(200, {
    reply,
    proposal: result.proposal,
    goTo: result.goTo,
    find: result.find,
    openTool: result.openTool,
    lookup: result.lookup.length > 0 ? result.lookup : null,
    feedbackSent: stored > 0,
    quota: { remaining: dev ? limit : Math.max(0, limit - (used + 1)), resetsAt: quota.resetsAt },
  });
}
