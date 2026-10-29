import { describe, expect, it } from 'vitest';
import { renderDigest, sendDigest, storeFeedback, type StoredFeedback } from '../src/feedback';
import { FakeKV, INSTALL_ID, makeEnv } from './helpers';

const item = (summary: string, at = '2026-10-01T08:00:00.000Z') => ({
  at,
  installId: INSTALL_ID,
  platform: 'ios',
  category: 'feature_request',
  summary,
});

function fakeFetch(ok = true) {
  const calls: { url: string; init: RequestInit }[] = [];
  const fn = (async (url: string, init: RequestInit) => {
    calls.push({ url, init });
    return new Response('{}', { status: ok ? 200 : 500 });
  }) as unknown as typeof fetch;
  return { fn, calls };
}

describe('feedback', () => {
  it('stores each item unsent under a time-ordered key', async () => {
    const kv = new FakeKV();
    await storeFeedback(kv, item('first'), 'a');
    const [key] = [...kv.data.keys()];
    expect(key).toBe('f:2026-10-01T08:00:00.000Z:a');
    expect((JSON.parse(kv.data.get(key!)!) as StoredFeedback).sent).toBe(false);
  });

  it('emails all unsent items in one digest, oldest first, then marks them sent', async () => {
    const env = makeEnv();
    await storeFeedback(env.FEEDBACK, item('later', '2026-10-01T09:00:00.000Z'), 'b');
    await storeFeedback(env.FEEDBACK, item('earlier', '2026-10-01T07:00:00.000Z'), 'a');
    const { fn, calls } = fakeFetch();

    expect(await sendDigest(env, fn)).toBe(2);

    expect(calls).toHaveLength(1);
    expect(calls[0]?.url).toBe('https://api.resend.com/emails');
    const headers = calls[0]!.init.headers as Record<string, string>;
    expect(headers.Authorization).toBe('Bearer re_test');
    const sent = JSON.parse(calls[0]!.init.body as string) as { to: string; from: string; subject: string; text: string };
    expect(sent.to).toBe('me@example.com');
    expect(sent.subject).toContain('2 new feedback items');
    expect(sent.text.indexOf('earlier')).toBeLessThan(sent.text.indexOf('later'));
    for (const value of env.FEEDBACK.data.values()) expect((JSON.parse(value) as StoredFeedback).sent).toBe(true);
  });

  it("names the email service's error type, but not its message (which echoes the address)", async () => {
    const env = makeEnv();
    await storeFeedback(env.FEEDBACK, item('one'), 'a');
    const refusing = (async () =>
      new Response(
        JSON.stringify({ name: 'validation_error', message: 'You can only send testing emails to your own email address (me@example.com).' }),
        { status: 403 },
      )) as unknown as typeof fetch;
    const error = await sendDigest(env, refusing).catch((e) => e as Error);
    expect((error as Error).message).toContain('403');
    expect((error as Error).message).toContain('validation_error');
    expect((error as Error).message).not.toContain('example.com');
  });

  it('sends nothing, and makes no email call, when there is nothing new', async () => {
    const env = makeEnv();
    const { fn, calls } = fakeFetch();
    expect(await sendDigest(env, fn)).toBe(0);
    await storeFeedback(env.FEEDBACK, item('one'), 'a');
    expect(await sendDigest(env, fn)).toBe(1);
    expect(await sendDigest(env, fn)).toBe(0);
    expect(calls).toHaveLength(1);
  });

  it('keeps items unsent when the email service fails, so the next digest retries them', async () => {
    const env = makeEnv();
    await storeFeedback(env.FEEDBACK, item('one'), 'a');
    await expect(sendDigest(env, fakeFetch(false).fn)).rejects.toThrow('500');
    expect(await sendDigest(env, fakeFetch().fn)).toBe(1);
  });

  it('skips corrupt records instead of failing the digest', async () => {
    const env = makeEnv();
    await env.FEEDBACK.put('f:2026-10-01T00:00:00.000Z:bad', '{not json');
    await storeFeedback(env.FEEDBACK, item('good'), 'a');
    expect(await sendDigest(env, fakeFetch().fn)).toBe(1);
  });

  it('renders a singular subject for one item', () => {
    expect(renderDigest([{ ...item('x'), sent: false }]).subject).toContain('1 new feedback item');
    expect(renderDigest([{ ...item('x'), sent: false }]).subject).not.toContain('items');
  });
});
