import type { Env, KV } from './types';

export interface StoredFeedback {
  at: string;
  installId: string;
  platform: string;
  category: string;
  summary: string;
  sent: boolean;
}

const PREFIX = 'f:';

export async function storeFeedback(
  kv: KV,
  item: Omit<StoredFeedback, 'sent'>,
  unique: string = crypto.randomUUID(),
): Promise<void> {
  const record: StoredFeedback = { ...item, sent: false };
  await kv.put(`${PREFIX}${item.at}:${unique}`, JSON.stringify(record));
}

/** Every stored item, oldest first, following KV's list pagination. */
async function listAll(kv: KV): Promise<{ key: string; item: StoredFeedback }[]> {
  const out: { key: string; item: StoredFeedback }[] = [];
  let cursor: string | undefined;
  do {
    const page = await kv.list({ prefix: PREFIX, cursor });
    for (const { name } of page.keys) {
      const raw = await kv.get(name);
      if (raw === null) continue;
      try {
        out.push({ key: name, item: JSON.parse(raw) as StoredFeedback });
      } catch {
        // A corrupt record is skipped rather than blocking the digest.
      }
    }
    cursor = page.list_complete ? undefined : page.cursor;
  } while (cursor);
  return out.sort((a, b) => (a.key < b.key ? -1 : 1));
}

export function renderDigest(items: StoredFeedback[]): { subject: string; text: string } {
  const lines = items.map(
    (i) => `[${i.category}] ${i.summary}\n    ${i.at} - ${i.platform} - install ${i.installId.slice(0, 8)}`,
  );
  return {
    subject: `Ask Pixie: ${items.length} new feedback item${items.length === 1 ? '' : 's'}`,
    text: lines.join('\n\n'),
  };
}

/**
 * Emails everything not yet sent as one digest, then marks it sent (records are kept as the
 * durable log). Returns how many items were sent; throws if the email service refuses, so the
 * items stay unsent and go out in the next digest.
 */
export async function sendDigest(
  env: Pick<Env, 'FEEDBACK' | 'RESEND_API_KEY' | 'DIGEST_TO' | 'DIGEST_FROM'>,
  fetchFn: typeof fetch = fetch,
): Promise<number> {
  const pending = (await listAll(env.FEEDBACK)).filter(({ item }) => !item.sent);
  if (pending.length === 0) return 0;
  const { subject, text } = renderDigest(pending.map((p) => p.item));
  const response = await fetchFn('https://api.resend.com/emails', {
    method: 'POST',
    headers: { Authorization: `Bearer ${env.RESEND_API_KEY}`, 'Content-Type': 'application/json' },
    body: JSON.stringify({ from: env.DIGEST_FROM, to: env.DIGEST_TO, subject, text }),
  });
  if (!response.ok) {
    // Only the error type: the message can echo the recipient address.
    const body = (await response.json().catch(() => ({}))) as { name?: unknown };
    const kind = typeof body.name === 'string' ? ` ${body.name.slice(0, 40)}` : '';
    throw new Error(`digest email failed: ${response.status}${kind}`);
  }
  for (const { key, item } of pending) {
    await env.FEEDBACK.put(key, JSON.stringify({ ...item, sent: true }));
  }
  return pending.length;
}
