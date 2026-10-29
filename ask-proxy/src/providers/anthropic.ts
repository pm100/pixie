import type Anthropic from '@anthropic-ai/sdk';
import { systemParts } from '../prompt';
import { interpret, MAX_TOKENS, REFUSAL_RESULT, TOOL_SPECS } from '../tools';
import type { AskBody, ModelProvider, ToolCall } from '../types';

/** A small, fast model is plenty for setup questions; override with the MODEL var. */
export const DEFAULT_ANTHROPIC_MODEL = 'claude-haiku-4-5';

/** The one SDK call this provider makes, so tests can stub it. */
export interface MessagesClient {
  messages: { create(params: Anthropic.MessageCreateParamsNonStreaming): Promise<Anthropic.Message> };
}

const TOOLS: Anthropic.Tool[] = TOOL_SPECS.map((tool) => ({
  name: tool.name,
  description: tool.description,
  input_schema: tool.schema as Anthropic.Tool.InputSchema,
}));

/**
 * Claude via the official SDK. Asks once (no tool loop: proposals are confirmed by the user in
 * the app, not executed here). The system prompt is two blocks, the stable one carrying the
 * cache breakpoint; below the model's minimum cacheable length the breakpoint is simply ignored.
 */
export function makeAnthropicProvider(
  client: MessagesClient,
  model: string = DEFAULT_ANTHROPIC_MODEL,
  /** Extended thinking token budget (review runs only; changes the model's behaviour). */
  thinkingBudget?: number,
): ModelProvider {
  return {
    async ask(body: AskBody) {
      const { stable, volatile } = systemParts(body.context);
      const message = await client.messages.create({
        model,
        max_tokens: thinkingBudget ? MAX_TOKENS + thinkingBudget : MAX_TOKENS,
        ...(thinkingBudget ? { thinking: { type: 'enabled' as const, budget_tokens: thinkingBudget } } : {}),
        system: [{ type: 'text', text: stable, cache_control: { type: 'ephemeral' } }, { type: 'text', text: volatile }],
        tools: TOOLS,
        tool_choice: { type: 'auto' },
        messages: [
          ...body.messages.map((turn) => ({ role: turn.role, content: turn.text })),
        ],
      });

      // Token counts only (no user text), so cache hits can be confirmed in the Workers logs.
      const usage = message.usage;
      if (usage) {
        console.log(
          `usage in=${usage.input_tokens} out=${usage.output_tokens} cache_read=${usage.cache_read_input_tokens ?? 0} cache_write=${usage.cache_creation_input_tokens ?? 0}`,
        );
      }

      if ((message.stop_reason as string) === 'refusal') return REFUSAL_RESULT;

      const texts: string[] = [];
      const thoughts: string[] = [];
      const calls: ToolCall[] = [];
      for (const block of message.content) {
        if (block.type === 'text') texts.push(block.text);
        else if (block.type === 'thinking') thoughts.push(block.thinking);
        else if (block.type === 'tool_use') calls.push({ name: block.name, input: block.input });
      }
      const result = interpret(texts, calls);
      return thoughts.length > 0 ? { ...result, reasoning: thoughts.join('\n\n') } : result;
    },
  };
}
