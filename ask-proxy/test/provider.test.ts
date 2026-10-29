import { describe, expect, it } from 'vitest';
import { makeProvider } from '../src/provider';
import { askBody, makeEnv } from './helpers';

const completion = { choices: [{ message: { content: 'from the openai-compatible server' } }] };

function fakeFetch() {
  const calls: { url: string; init: RequestInit }[] = [];
  const fn = (async (url: string, init: RequestInit) => {
    calls.push({ url, init });
    return new Response(JSON.stringify(completion), { status: 200 });
  }) as unknown as typeof fetch;
  return { fn, calls };
}

describe('makeProvider', () => {
  it('defaults to Anthropic and needs its key', () => {
    expect(() => makeProvider(makeEnv({ ANTHROPIC_API_KEY: undefined }))).toThrow('ANTHROPIC_API_KEY');
    expect(makeProvider(makeEnv())).toBeDefined();
    expect(makeProvider(makeEnv({ PROVIDER: 'Anthropic' }))).toBeDefined();
  });

  it('builds an OpenAI-compatible provider from configuration and really uses it', async () => {
    const { fn, calls } = fakeFetch();
    const provider = makeProvider(
      makeEnv({ PROVIDER: 'openai', OPENAI_BASE_URL: 'http://localhost:11434/v1', OPENAI_API_KEY: 'k', MODEL: 'llama3.1' }),
      fn,
    );
    const result = await provider.ask(askBody());
    expect(result.reply).toBe('from the openai-compatible server');
    expect(calls[0]?.url).toBe('http://localhost:11434/v1/chat/completions');
    expect(JSON.parse(calls[0]!.init.body as string).model).toBe('llama3.1');
  });

  it('passes the token-limit field name through', async () => {
    const { fn, calls } = fakeFetch();
    const provider = makeProvider(
      makeEnv({
        PROVIDER: 'openai', OPENAI_BASE_URL: 'https://x/v1', OPENAI_API_KEY: 'k', MODEL: 'm',
        OPENAI_MAX_TOKENS_FIELD: 'max_completion_tokens',
      }),
      fn,
    );
    await provider.ask(askBody());
    expect(JSON.parse(calls[0]!.init.body as string).max_completion_tokens).toBeGreaterThan(0);
  });

  it('passes OPENAI_TEMPERATURE through when it is a sensible number and ignores junk', async () => {
    const sentWith = async (value: string | undefined) => {
      const { fn, calls } = fakeFetch();
      const provider = makeProvider(
        makeEnv({ PROVIDER: 'openai', OPENAI_BASE_URL: 'https://x/v1', OPENAI_API_KEY: 'k', MODEL: 'm', OPENAI_TEMPERATURE: value }),
        fn,
      );
      await provider.ask(askBody());
      return JSON.parse(calls[0]!.init.body as string).temperature;
    };
    expect(await sentWith('0.2')).toBe(0.2);
    expect(await sentWith('0')).toBe(0);
    expect(await sentWith(undefined)).toBeUndefined();
    for (const junk of ['', 'warm', '-1', '3', 'NaN']) expect(await sentWith(junk)).toBeUndefined();
  });

  it('names exactly what is missing for the OpenAI-compatible provider, never a secret value', () => {
    const env = makeEnv({ PROVIDER: 'openai', OPENAI_API_KEY: 'sk-very-secret' });
    let message = '';
    try {
      makeProvider(env);
    } catch (error) {
      message = (error as Error).message;
    }
    expect(message).toContain('OPENAI_BASE_URL');
    expect(message).toContain('MODEL');
    expect(message).not.toContain('OPENAI_API_KEY');
    expect(message).not.toContain('sk-very-secret');
  });

  it('rejects an unknown provider name', () => {
    expect(() => makeProvider(makeEnv({ PROVIDER: 'mystery' }))).toThrow('unknown PROVIDER "mystery"');
  });
});
