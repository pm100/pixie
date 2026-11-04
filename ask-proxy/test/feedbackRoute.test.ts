import { describe, expect, it } from 'vitest';
import worker from '../src/index';
import { handleFeedback, MAX_FEEDBACK_PER_DAY } from '../src/handler';
import type { StoredFeedback } from '../src/feedback';
import { INSTALL_ID, makeEnv } from './helpers';

const NOW = new Date('2026-10-01T12:00:00Z');
const note = { category: 'feature_request', summary: 'Wants dark mode.', platform: 'windows' };

function post(body: unknown, installId: string | null = INSTALL_ID): Request {
  const headers: Record<string, string> = { 'Content-Type': 'application/json' };
  if (installId !== null) headers['X-Install-Id'] = installId;
  return new Request('https://proxy.test/v1/feedback', { method: 'POST', headers, body: typeof body === 'string' ? body : JSON.stringify(body) });
}

const send = (env: ReturnType<typeof makeEnv>, body: unknown, installId: string | null = INSTALL_ID) => handleFeedback(post(body, installId), env, { now: () => NOW });

describe('POST /v1/feedback (a note the person confirmed)', () => {
  it('stores the note for the digest', async () => {
    const env = makeEnv();
    const response = await send(env, note);
    expect(response.status).toBe(200);
    expect(await response.json()).toEqual({ ok: true });
    const stored = [...env.FEEDBACK.data.values()].map((v) => JSON.parse(v) as StoredFeedback);
    expect(stored).toEqual([{ at: NOW.toISOString(), installId: INSTALL_ID, platform: 'windows', category: 'feature_request', summary: 'Wants dark mode.', sent: false }]);
  });

  it.each([
    ['not json', 'nope'],
    ['no summary', { ...note, summary: '  ' }],
    ['a made-up category', { ...note, category: 'praise' }],
    ['no platform', { category: 'bug', summary: 'x' }],
    ['a long platform', { ...note, platform: 'x'.repeat(17) }],
  ])('turns away %s', async (_name, body) => {
    const env = makeEnv();
    expect((await send(env, body)).status).toBe(400);
    expect(env.FEEDBACK.data.size).toBe(0);
  });

  it('needs an install id, and refuses a body too big to be a short note', async () => {
    const env = makeEnv();
    expect((await send(env, note, null)).status).toBe(400);
    expect((await send(env, { ...note, summary: 'x'.repeat(3000) })).status).toBe(413);
  });

  it('keeps long summaries to the length the model may use', async () => {
    const env = makeEnv();
    await send(env, { ...note, summary: 'y'.repeat(600) });
    expect((JSON.parse([...env.FEEDBACK.data.values()][0]!) as StoredFeedback).summary).toHaveLength(500);
  });

  it('stops after a few a day from one install, and starts again the next day', async () => {
    const env = makeEnv();
    for (let i = 0; i < MAX_FEEDBACK_PER_DAY; i++) expect((await send(env, note)).status).toBe(200);
    expect((await send(env, note)).status).toBe(429);
    const tomorrow = await handleFeedback(post(note), env, { now: () => new Date('2026-10-02T01:00:00Z') });
    expect(tomorrow.status).toBe(200);
  });

  it('says so, and counts nothing, when it cannot store the note', async () => {
    const env = makeEnv();
    env.FEEDBACK.failPuts = true;
    expect((await send(env, note)).status).toBe(500);
    env.FEEDBACK.failPuts = false;
    for (let i = 0; i < MAX_FEEDBACK_PER_DAY; i++) expect((await send(env, note)).status).toBe(200);
  });

  it('is reached through the worker at /v1/feedback, by POST only', async () => {
    const env = makeEnv();
    expect((await worker.fetch(post(note), env)).status).toBe(200);
    expect((await worker.fetch(new Request('https://proxy.test/v1/feedback'), env)).status).toBe(404);
  });
});
