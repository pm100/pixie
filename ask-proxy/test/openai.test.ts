import { describe, expect, it, vi } from 'vitest';
import { makeOpenAIProvider, type OpenAIConfig } from '../src/providers/openai';
import { MAX_TOKENS } from '../src/tools';
import { askBody } from './helpers';

const config: OpenAIConfig = {
  baseUrl: 'https://llm.example.com/v1/',
  apiKey: 'sk-secret-key',
  model: 'some-model',
  maxTokensField: 'max_tokens',
};

function fakeFetch(reply: unknown, status = 200) {
  const calls: { url: string; init: RequestInit }[] = [];
  const fn = (async (url: string, init: RequestInit) => {
    calls.push({ url, init });
    return new Response(typeof reply === 'string' ? reply : JSON.stringify(reply), { status });
  }) as unknown as typeof fetch;
  return { fn, calls };
}

const completion = (message: Record<string, unknown>, usage?: unknown) => ({ choices: [{ message }], usage });
const call = (name: string, args: unknown) => ({
  type: 'function',
  function: { name, arguments: typeof args === 'string' ? args : JSON.stringify(args) },
});

describe('openai-compatible provider', () => {
  it('posts to {baseUrl}/chat/completions with the key, model, tools and one system message', async () => {
    const { fn, calls } = fakeFetch(completion({ content: 'ok' }));
    await makeOpenAIProvider(config, fn).ask(askBody('hello'));
    expect(calls[0]?.url).toBe('https://llm.example.com/v1/chat/completions');
    const headers = calls[0]!.init.headers as Record<string, string>;
    expect(headers.Authorization).toBe('Bearer sk-secret-key');
    const body = JSON.parse(calls[0]!.init.body as string);
    expect(body.model).toBe('some-model');
    expect(body.max_tokens).toBe(MAX_TOKENS);
    expect(body.tool_choice).toBe('auto');
    expect(body.tools.map((t: any) => t.function.name)).toEqual(['propose_settings_changes', 'go_to', 'find_photos', 'lookup', 'submit_feedback']);
    expect(body.tools[0].type).toBe('function');
    expect(body.messages[0].role).toBe('system');
    expect(body.messages[0].content).toContain('analysis_faces');
    expect(body.messages[0].content).toContain('Platform: windows');
    expect(body.messages[1]).toEqual({ role: 'user', content: 'hello' });
    expect(body.messages).toHaveLength(2);
  });

  it('puts the stable prompt before the per-user state so prefix caches can reuse it', async () => {
    const { fn, calls } = fakeFetch(completion({ content: 'ok' }));
    await makeOpenAIProvider(config, fn).ask(askBody());
    const system = JSON.parse(calls[0]!.init.body as string).messages[0].content as string;
    expect(system.indexOf('SETTINGS SCHEMA')).toBeLessThan(system.indexOf('USER STATE'));
  });

  it('sends a temperature only when one is configured', async () => {
    const plain = fakeFetch(completion({ content: 'ok' }));
    await makeOpenAIProvider(config, plain.fn).ask(askBody());
    expect(JSON.parse(plain.calls[0]!.init.body as string).temperature).toBeUndefined();
    const tuned = fakeFetch(completion({ content: 'ok' }));
    await makeOpenAIProvider({ ...config, temperature: 0.2 }, tuned.fn).ask(askBody());
    expect(JSON.parse(tuned.calls[0]!.init.body as string).temperature).toBe(0.2);
  });

  it('uses the configured name for the token limit field', async () => {
    const { fn, calls } = fakeFetch(completion({ content: 'ok' }));
    await makeOpenAIProvider({ ...config, maxTokensField: 'max_completion_tokens' }, fn).ask(askBody());
    const body = JSON.parse(calls[0]!.init.body as string);
    expect(body.max_completion_tokens).toBe(MAX_TOKENS);
    expect(body.max_tokens).toBeUndefined();
  });

  it('turns content and tool calls into a result, parsing the JSON-string arguments', async () => {
    const { fn } = fakeFetch(
      completion({
        content: 'More tries.',
        tool_calls: [
          call('propose_settings_changes', { changes: [{ setting: 'analysis_faces', value: false }] }),
          call('submit_feedback', { category: 'bug', summary: 'It crashed.' }),
        ],
      }),
    );
    expect(await makeOpenAIProvider(config, fn).ask(askBody())).toEqual({
      reply: 'More tries.',
      proposal: [{ setting: 'analysis_faces', value: false }],
      goTo: null,
      find: null,
      lookup: [],
      feedback: [{ category: 'bug', summary: 'It crashed.' }],
    });
  });

  it('handles null content with only tool calls, and unparseable arguments', async () => {
    const { fn } = fakeFetch(
      completion({
        content: null,
        tool_calls: [call('propose_settings_changes', '{not json'), call('propose_settings_changes', { changes: [{ setting: 'a', value: 1 }] })],
      }),
    );
    const result = await makeOpenAIProvider(config, fn).ask(askBody());
    expect(result.reply).toBe('');
    expect(result.proposal).toEqual([{ setting: 'a', value: 1 }]);
  });

  it('ignores tool calls without a name', async () => {
    const { fn } = fakeFetch(completion({ content: 'Hi.', tool_calls: [{ function: { arguments: '{}' } }, {}] }));
    expect((await makeOpenAIProvider(config, fn).ask(askBody())).reply).toBe('Hi.');
  });

  it('turns a refusal into a polite reply', async () => {
    const { fn } = fakeFetch(completion({ content: null, refusal: 'I cannot help with that.' }));
    const result = await makeOpenAIProvider(config, fn).ask(askBody());
    expect(result.reply).toContain("can't help");
    expect(result.proposal).toBeNull();
  });

  it('throws with the HTTP status on an error response, without leaking the key or the body', async () => {
    const { fn } = fakeFetch('{"error":"invalid key sk-secret-key"}', 401);
    const error = await makeOpenAIProvider(config, fn).ask(askBody()).catch((e) => e as Error & { status?: number });
    expect(error).toBeInstanceOf(Error);
    expect((error as { status?: number }).status).toBe(401);
    expect((error as Error).message).not.toContain('sk-secret-key');
  });

  it('throws on a response that is not JSON or has no message', async () => {
    await expect(makeOpenAIProvider(config, fakeFetch('<html>').fn).ask(askBody())).rejects.toThrow('not JSON');
    await expect(makeOpenAIProvider(config, fakeFetch({ choices: [] }).fn).ask(askBody())).rejects.toThrow('no message');
    await expect(makeOpenAIProvider(config, fakeFetch({}).fn).ask(askBody())).rejects.toThrow('no message');
  });

  it('lets network failures propagate', async () => {
    const failing = (async () => { throw new TypeError('network down'); }) as unknown as typeof fetch;
    await expect(makeOpenAIProvider(config, failing).ask(askBody())).rejects.toThrow('network down');
  });

  it('logs token counts only', async () => {
    const { fn } = fakeFetch(
      completion({ content: 'secret reply' }, { prompt_tokens: 50, completion_tokens: 7, prompt_tokens_details: { cached_tokens: 40 } }),
    );
    const log = vi.spyOn(console, 'log').mockImplementation(() => {});
    await makeOpenAIProvider(config, fn).ask(askBody('my private question'));
    const logged = log.mock.calls.map((c) => c.join(' ')).join(' | ');
    log.mockRestore();
    expect(logged).toContain('usage in=50 out=7 cache_read=40');
    expect(logged).not.toContain('private');
    expect(logged).not.toContain('secret');
  });

  it('sends what the app found out: the model\'s own call, then the answer, after the turns', async () => {
    const { fn, calls } = fakeFetch(completion({ content: 'ok' }));
    const body = { ...askBody('find rose swimming'), lookups: [{ calls: [{ id: 'call_1', input: { what: 'people', query: 'rose' } }], results: ['[{"name":"Rose","count":4}]'] }] };
    await makeOpenAIProvider(config, fn).ask(body);
    const messages = JSON.parse(calls[0]!.init.body as string).messages.slice(1);
    expect(messages).toEqual([
      { role: 'user', content: 'find rose swimming' },
      { role: 'assistant', content: null, tool_calls: [{ id: 'call_1', type: 'function', function: { name: 'lookup', arguments: '{"what":"people","query":"rose"}' } }] },
      { role: 'tool', tool_call_id: 'call_1', content: '[{"name":"Rose","count":4}]' },
    ]);
  });

  it('reads the id of a lookup the model asks for', async () => {
    const { fn } = fakeFetch(completion({ content: null, tool_calls: [{ id: 'call_7', type: 'function', function: { name: 'lookup', arguments: '{"what":"trips"}' } }] }));
    const r = await makeOpenAIProvider(config, fn).ask(askBody('when was my trip'));
    expect(r.lookup).toEqual([{ id: 'call_7', input: { what: 'trips' }, what: 'trips', query: null, filter: null }]);
  });
});
