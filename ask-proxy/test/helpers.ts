import type Anthropic from '@anthropic-ai/sdk';
import type { MessagesClient } from '../src/providers/anthropic';
import type { AskBody, Context, Env, KV, ModelProvider, ModelResult } from '../src/types';

export class FakeKV implements KV {
  readonly data = new Map<string, string>();
  readonly ttls = new Map<string, number | undefined>();
  failPuts = false;

  async get(key: string): Promise<string | null> {
    return this.data.get(key) ?? null;
  }

  async put(key: string, value: string, options?: { expirationTtl?: number }): Promise<void> {
    if (this.failPuts) throw new Error('kv down');
    this.data.set(key, value);
    this.ttls.set(key, options?.expirationTtl);
  }

  async list(options: { prefix: string }) {
    const keys = [...this.data.keys()].filter((k) => k.startsWith(options.prefix)).map((name) => ({ name }));
    return { keys, list_complete: true };
  }
}

export const INSTALL_ID = '123e4567-e89b-42d3-a456-426614174000';

export const CONTEXT: Context = {
  platform: 'windows',
  language: 'en-US',
  tab: 'browse',
  today: '2026-10-28',
  settings: [
    {
      key: 'network_rescan_minutes',
      label: 'Scan network folders again every',
      description: 'How often folders on the network are scanned again.',
      type: 'choice',
      options: [
        { value: 5, label: '5 minutes' },
        { value: 30, label: '30 minutes' },
        { value: 120, label: '2 hours' },
      ],
      current: 30,
      currentLabel: '30 minutes',
      default: 30,
    },
    {
      key: 'analysis_faces',
      label: 'Find faces',
      description: 'Look for faces in photos so people can be named.',
      type: 'bool',
      current: true,
      currentLabel: 'On',
      default: true,
    },
  ],
};

export function askBody(text = 'Stop it looking for faces', context: Context = CONTEXT): AskBody {
  return { messages: [{ role: 'user', text }], context };
}

export function makeEnv(overrides: Partial<Env> = {}): Env & { QUOTA: FakeKV; FEEDBACK: FakeKV } {
  return {
    QUOTA: new FakeKV(),
    FEEDBACK: new FakeKV(),
    ANTHROPIC_API_KEY: 'sk-test',
    RESEND_API_KEY: 're_test',
    DIGEST_TO: 'me@example.com',
    DIGEST_FROM: 'Pixie <bot@example.com>',
    ...overrides,
  } as Env & { QUOTA: FakeKV; FEEDBACK: FakeKV };
}

type Block =
  | { type: 'text'; text: string }
  | { type: 'tool_use'; id: string; name: string; input: unknown };

export function message(blocks: Block[], stopReason: string = 'end_turn'): Anthropic.Message {
  return { id: 'msg_test', type: 'message', role: 'assistant', model: 'm', content: blocks, stop_reason: stopReason } as unknown as Anthropic.Message;
}

export const text = (t: string): Block => ({ type: 'text', text: t });
export const tool = (name: string, input: unknown): Block => ({ type: 'tool_use', id: `toolu_${name}`, name, input });

/** A client that returns the queued replies (or throws a queued Error) and records each request. */
export function stubClient(...replies: (Anthropic.Message | Error)[]) {
  const calls: Anthropic.MessageCreateParamsNonStreaming[] = [];
  const client: MessagesClient = {
    messages: {
      async create(params) {
        calls.push(params);
        const next = replies.shift() ?? message([text('(no reply queued)')]);
        if (next instanceof Error) throw next;
        return next;
      },
    },
  };
  return { client, calls };
}

/** A provider that returns the queued results (or throws a queued Error) and records each question. */
export function stubProvider(...results: (Partial<ModelResult> | Error)[]) {
  const calls: AskBody[] = [];
  const provider: ModelProvider = {
    async ask(body) {
      calls.push(body);
      const next = results.shift() ?? { reply: '(no result queued)' };
      if (next instanceof Error) throw next;
      return { reply: '', proposal: null, goTo: null, feedback: [], ...next };
    },
  };
  return { provider, calls };
}

export function askRequest(body: unknown, installId: string | null = INSTALL_ID, devKey: string | null = null): Request {
  const headers: Record<string, string> = { 'Content-Type': 'application/json' };
  if (devKey !== null) headers['X-Dev-Key'] = devKey;
  if (installId !== null) headers['X-Install-Id'] = installId;
  return new Request('https://proxy.test/v1/ask', {
    method: 'POST',
    headers,
    body: typeof body === 'string' ? body : JSON.stringify(body),
  });
}
