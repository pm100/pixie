import type { Env, KV, Quota } from './types';

const DEFAULT_DAILY_LIMIT = 10;
const TWO_DAYS_SECONDS = 172_800;

/** UTC calendar day, e.g. "2026-10-01". */
export function dayKey(now: Date): string {
  return now.toISOString().slice(0, 10);
}

/** The next UTC midnight, when the day's counter starts over. */
export function nextReset(now: Date): string {
  return new Date(Date.UTC(now.getUTCFullYear(), now.getUTCMonth(), now.getUTCDate() + 1)).toISOString();
}

function parseLimit(value: string | undefined, fallback: number): number {
  const parsed = value === undefined ? NaN : Number.parseInt(value, 10);
  return Number.isInteger(parsed) && parsed >= 0 ? parsed : fallback;
}

export function limitFor(env: Pick<Env, 'DAILY_LIMIT'>): number {
  return parseLimit(env.DAILY_LIMIT, DEFAULT_DAILY_LIMIT);
}

function counterKey(installId: string, now: Date): string {
  return `q:${installId}:${dayKey(now)}`;
}

export async function readQuota(
  kv: KV,
  installId: string,
  limit: number,
  now: Date,
): Promise<{ used: number; quota: Quota }> {
  const stored = await kv.get(counterKey(installId, now));
  const parsed = stored === null ? 0 : Number.parseInt(stored, 10);
  const used = Number.isInteger(parsed) && parsed > 0 ? parsed : 0;
  return { used, quota: { remaining: Math.max(0, limit - used), resetsAt: nextReset(now) } };
}

/**
 * Counts one question. KV is eventually consistent, so two simultaneous requests can both
 * read the same count; that only ever under-counts by a little, which v1 accepts.
 */
export async function recordUse(kv: KV, installId: string, used: number, now: Date): Promise<void> {
  await kv.put(counterKey(installId, now), String(used + 1), { expirationTtl: TWO_DAYS_SECONDS });
}
