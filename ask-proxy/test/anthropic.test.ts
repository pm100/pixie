import { describe, expect, it, vi } from 'vitest';
import { DEFAULT_ANTHROPIC_MODEL, makeAnthropicProvider } from '../src/providers/anthropic';
import { MAX_TOKENS, TOOL_SPECS } from '../src/tools';
import { askBody, message, stubClient, text, tool } from './helpers';

describe('anthropic provider', () => {
  it('sends the model, token cap, two system blocks (the first cached), tools and turns', async () => {
    const { client, calls } = stubClient(message([text('ok')]));
    await makeAnthropicProvider(client).ask(askBody('hello'));
    const call = calls[0]!;
    expect(call.model).toBe(DEFAULT_ANTHROPIC_MODEL);
    expect(DEFAULT_ANTHROPIC_MODEL).toBe('claude-haiku-4-5');
    expect(call.max_tokens).toBe(MAX_TOKENS);
    expect(call.tool_choice).toEqual({ type: 'auto' });
    expect(call.tools?.map((t) => t.name)).toEqual(TOOL_SPECS.map((t) => t.name));
    expect(call.messages).toEqual([{ role: 'user', content: 'hello' }]);
    const system = call.system as { text: string; cache_control?: unknown }[];
    expect(system).toHaveLength(2);
    expect(system[0]?.cache_control).toEqual({ type: 'ephemeral' });
    expect(system[0]?.text).toContain('analysis_faces');
    expect(system[1]?.cache_control).toBeUndefined();
    expect(system[1]?.text).toContain('Platform: windows');
  });

  it('honours a model override', async () => {
    const { client, calls } = stubClient(message([text('ok')]));
    await makeAnthropicProvider(client, 'claude-sonnet-5-5').ask(askBody());
    expect(calls[0]?.model).toBe('claude-sonnet-5-5');
  });

  it('turns text and tool calls into a result', async () => {
    const { client } = stubClient(
      message(
        [
          text('Give yourself more tries.'),
          tool('propose_settings_changes', { changes: [{ setting: 'analysis_faces', value: false }] }),
          tool('submit_feedback', { category: 'feature_request', summary: 'Wants dark mode.' }),
        ],
        'tool_use',
      ),
    );
    expect(await makeAnthropicProvider(client).ask(askBody())).toEqual({
      reply: 'Give yourself more tries.',
      proposal: [{ setting: 'analysis_faces', value: false }],
      goTo: null,
      feedback: [{ category: 'feature_request', summary: 'Wants dark mode.' }],
    });
  });

  it('turns a refusal into a polite reply with no proposal', async () => {
    const { client } = stubClient(
      message([tool('propose_settings_changes', { changes: [{ setting: 'a', value: 1 }] })], 'refusal'),
    );
    const result = await makeAnthropicProvider(client).ask(askBody());
    expect(result.proposal).toBeNull();
    expect(result.reply).toContain("can't help");
  });

  it('logs token and cache usage, and nothing from the conversation', async () => {
    const reply = message([text('secret reply')]);
    (reply as { usage?: unknown }).usage = { input_tokens: 100, output_tokens: 20, cache_read_input_tokens: 4000 };
    const { client } = stubClient(reply);
    const log = vi.spyOn(console, 'log').mockImplementation(() => {});
    await makeAnthropicProvider(client).ask(askBody('my private question'));
    const logged = log.mock.calls.map((c) => c.join(' ')).join(' | ');
    log.mockRestore();
    expect(logged).toContain('usage in=100 out=20 cache_read=4000 cache_write=0');
    expect(logged).not.toContain('private');
    expect(logged).not.toContain('secret');
  });

  it('lets API errors propagate to the caller', async () => {
    const { client } = stubClient(new Error('boom'));
    await expect(makeAnthropicProvider(client).ask(askBody())).rejects.toThrow('boom');
  });
});
