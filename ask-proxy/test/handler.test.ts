import { describe, expect, it, vi } from 'vitest';
import worker from '../src/index';
import { FEEDBACK_FALLBACK_REPLY, GO_TO_FALLBACK_REPLY, handleAsk, PROPOSAL_FALLBACK_REPLY } from '../src/handler';
import { MAX_BODY_BYTES } from '../src/validate';
import { askBody, askRequest, CONTEXT, INSTALL_ID, makeEnv, stubProvider } from './helpers';

const NOW = new Date('2026-10-01T12:00:00Z');

async function ask(
  env = makeEnv(),
  results: Parameters<typeof stubProvider> = [{ reply: 'Hi.' }],
  body: unknown = askBody(),
) {
  const stub = stubProvider(...results);
  const response = await handleAsk(askRequest(body), env, { provider: stub.provider, now: () => NOW });
  return { response, json: (await response.json()) as Record<string, any>, stub, env };
}

describe('POST /v1/ask', () => {
  it('answers with text, no proposal and the remaining quota', async () => {
    const { response, json } = await ask();
    expect(response.status).toBe(200);
    expect(json).toEqual({
      reply: 'Hi.',
      proposal: null,
      goTo: null,
      feedbackSent: false,
      quota: { remaining: 9, resetsAt: '2026-10-02T00:00:00.000Z' },
    });
    expect(response.headers.get('Cache-Control')).toBe('no-store');
  });

  it('hands the validated body to the provider', async () => {
    const { stub } = await ask();
    expect(stub.calls).toHaveLength(1);
    expect(stub.calls[0]?.messages).toEqual([{ role: 'user', text: 'Stop it looking for faces' }]);
    expect(stub.calls[0]?.context.platform).toBe('windows');
  });

  it('passes a settings proposal through for the app to validate', async () => {
    const { json } = await ask(makeEnv(), [
      { reply: 'More tries.', proposal: [{ setting: 'analysis_faces', value: false }] },
    ]);
    expect(json.proposal).toEqual([{ setting: 'analysis_faces', value: false }]);
  });

  it('stores feedback, reports feedbackSent and does not leak it into the reply', async () => {
    const { json, env } = await ask(makeEnv(), [
      { reply: 'Passed that on.', feedback: [{ category: 'feature_request', summary: 'Wants dark mode.' }] },
    ]);
    expect(json.feedbackSent).toBe(true);
    expect(json.reply).toBe('Passed that on.');
    const stored = [...env.FEEDBACK.data.values()].map((v) => JSON.parse(v));
    expect(stored).toEqual([
      { at: NOW.toISOString(), installId: INSTALL_ID, platform: 'windows', category: 'feature_request', summary: 'Wants dark mode.', sent: false },
    ]);
  });

  it('fills in a reply when the model only submitted feedback', async () => {
    const { json } = await ask(makeEnv(), [
      { reply: '', feedback: [{ category: 'feature_request', summary: 'Wants dark mode.' }] },
    ]);
    expect(json.reply).toBe(FEEDBACK_FALLBACK_REPLY);
    expect(json.feedbackSent).toBe(true);
  });

  it('fills in a reply when the model only offered a button to a place in the app', async () => {
    const { json } = await ask(makeEnv(), [{ reply: '', goTo: 'tools-email' }]);
    expect(json.reply).toBe(GO_TO_FALLBACK_REPLY);
    expect(json.goTo).toBe('tools-email');
  });

  it('fills in a reply when the model only proposed a change', async () => {
    const { json } = await ask(makeEnv(), [{ reply: '  ', proposal: [{ setting: 'analysis_faces', value: false }] }]);
    expect(json.reply).toBe(PROPOSAL_FALLBACK_REPLY);
    expect(json.proposal).toEqual([{ setting: 'analysis_faces', value: false }]);
  });

  it('never claims feedback was passed on when it could not be stored', async () => {
    const env = makeEnv();
    env.FEEDBACK.failPuts = true;
    const { json } = await ask(env, [{ reply: '', feedback: [{ category: 'bug', summary: 'It crashed.' }] }]);
    expect(json.reply).toBe('');
  });

  it('keeps a reply the model wrote', async () => {
    const { json } = await ask(makeEnv(), [
      { reply: 'Custom.', proposal: [{ setting: 'analysis_faces', value: false }], feedback: [{ category: 'bug', summary: 'x' }] },
    ]);
    expect(json.reply).toBe('Custom.');
  });

  it('still answers when feedback cannot be stored', async () => {
    const env = makeEnv();
    env.FEEDBACK.failPuts = true;
    const { response, json } = await ask(env, [{ reply: 'Noted.', feedback: [{ category: 'bug', summary: 'It crashed.' }] }]);
    expect(response.status).toBe(200);
    expect(json.feedbackSent).toBe(false);
  });

  it('counts down the quota and returns 429 once it is used up, without calling the provider', async () => {
    const env = makeEnv({ DAILY_LIMIT: '2' });
    expect((await ask(env, [{ reply: '1' }])).json.quota.remaining).toBe(1);
    expect((await ask(env, [{ reply: '2' }])).json.quota.remaining).toBe(0);
    const third = await ask(env, [{ reply: '3' }]);
    expect(third.response.status).toBe(429);
    expect(third.json).toEqual({ error: 'quota_exceeded', quota: { remaining: 0, resetsAt: '2026-10-02T00:00:00.000Z' } });
    expect(third.stub.calls).toHaveLength(0);
  });

  describe('dev builds', () => {
    const KEY = 'a'.repeat(32);
    const asking = async (env: ReturnType<typeof makeEnv>, devKey: string | null) => {
      const stub = stubProvider({ reply: 'ok' });
      const response = await handleAsk(askRequest(askBody(), INSTALL_ID, devKey), env, { provider: stub.provider, now: () => NOW });
      return { response, json: (await response.json()) as Record<string, any>, stub };
    };

    it('are not limited when they send the key the server holds', async () => {
      const env = makeEnv({ DAILY_LIMIT: '1', DEV_KEY: KEY });
      for (let i = 0; i < 3; i++) expect((await asking(env, KEY)).response.status).toBe(200);
    });

    it('do not use up a question, and are told nothing is running low', async () => {
      const env = makeEnv({ DAILY_LIMIT: '2', DEV_KEY: KEY });
      const { json } = await asking(env, KEY);
      expect(json.quota.remaining).toBe(2);
      expect((await asking(env, null)).json.quota.remaining).toBe(1);
    });

    it('are limited like anyone else with a wrong key', async () => {
      const env = makeEnv({ DAILY_LIMIT: '1', DEV_KEY: KEY });
      expect((await asking(env, 'b'.repeat(32))).response.status).toBe(200);
      expect((await asking(env, 'b'.repeat(32))).response.status).toBe(429);
    });

    it('send a key to a server that holds none, and it is ignored', async () => {
      const env = makeEnv({ DAILY_LIMIT: '1' });
      expect((await asking(env, KEY)).response.status).toBe(200);
      expect((await asking(env, KEY)).response.status).toBe(429);
    });

    it('an empty key never matches an empty setting', async () => {
      const env = makeEnv({ DAILY_LIMIT: '1', DEV_KEY: '' });
      expect((await asking(env, '')).response.status).toBe(200);
      expect((await asking(env, '')).response.status).toBe(429);
    });
  });

  it('returns 502 and does not use up a question when the provider fails', async () => {
    const env = makeEnv({ DAILY_LIMIT: '1' });
    const failed = await ask(env, [Object.assign(new Error('boom'), { status: 500 })]);
    expect(failed.response.status).toBe(502);
    expect(failed.json).toEqual({ error: 'upstream' });
    expect((await ask(env, [{ reply: 'ok now' }])).response.status).toBe(200);
  });

  it('logs the failure class and status but not the error message', async () => {
    const error = vi.spyOn(console, 'error').mockImplementation(() => {});
    await ask(makeEnv(), [Object.assign(new Error('invalid key sk-secret and the private question'), { status: 401 })]);
    const logged = error.mock.calls.map((c) => c.join(' ')).join(' | ');
    error.mockRestore();
    expect(logged).toContain('status=401');
    expect(logged).not.toContain('sk-secret');
    expect(logged).not.toContain('private');
  });

  it('does log a misconfiguration message (it names settings, never values)', async () => {
    const error = vi.spyOn(console, 'error').mockImplementation(() => {});
    await ask(makeEnv(), [new Error('provider misconfigured: MODEL not set')]);
    const logged = error.mock.calls.map((c) => c.join(' ')).join(' | ');
    error.mockRestore();
    expect(logged).toContain('provider misconfigured: MODEL not set');
  });

  it('rejects a missing or malformed install id', async () => {
    for (const id of [null, 'nope', '']) {
      const response = await handleAsk(askRequest(askBody(), id), makeEnv(), { provider: stubProvider().provider, now: () => NOW });
      expect(response.status).toBe(400);
      expect(await response.json()).toEqual({ error: 'bad_install_id' });
    }
  });

  it('rejects unparseable, invalid and oversized bodies without calling the provider', async () => {
    const stub = stubProvider();
    const env = makeEnv();
    const status = async (body: unknown) =>
      (await handleAsk(askRequest(body), env, { provider: stub.provider, now: () => NOW })).status;
    expect(await status('{not json')).toBe(400);
    expect(await status({ messages: [], context: CONTEXT })).toBe(400);
    expect(await status('x'.repeat(MAX_BODY_BYTES + 1))).toBe(413);
    expect(stub.calls).toHaveLength(0);
  });
});

describe('worker routing', () => {
  it('answers /health, 404s unknown paths and non-POST asks', async () => {
    const env = makeEnv();
    expect((await worker.fetch(new Request('https://p.test/health'), env)).status).toBe(200);
    expect((await worker.fetch(new Request('https://p.test/nope'), env)).status).toBe(404);
    expect((await worker.fetch(new Request('https://p.test/v1/ask'), env)).status).toBe(404);
  });

  it('turns a misconfigured provider into a 502 that costs the user nothing', async () => {
    const error = vi.spyOn(console, 'error').mockImplementation(() => {});
    const env = makeEnv({ ANTHROPIC_API_KEY: undefined });
    const response = await worker.fetch(askRequest(askBody()), env);
    const logged = error.mock.calls.map((c) => c.join(' ')).join(' | ');
    error.mockRestore();
    expect(response.status).toBe(502);
    expect(logged).toContain('provider misconfigured: ANTHROPIC_API_KEY is not set');
    expect(env.QUOTA.data.size).toBe(0);
  });
});
