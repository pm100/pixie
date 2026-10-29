import { systemParts } from '../prompt';
import { interpret, MAX_TOKENS, REFUSAL_RESULT, TOOL_SPECS } from '../tools';
import type { AskBody, ModelProvider, ToolCall } from '../types';

export interface OpenAIConfig {
  /** e.g. https://api.openai.com/v1; the path /chat/completions is appended. */
  baseUrl: string;
  apiKey: string;
  model: string;
  /** "max_tokens" for most servers; "max_completion_tokens" for some newer OpenAI models. */
  maxTokensField: string;
  /** Sampling temperature; leave unset for models that only accept their default. */
  temperature?: number;
}

const REQUEST_TIMEOUT_MS = 25_000;

interface ChatResponse {
  choices?: {
    message?: {
      content?: string | null;
      /** Reasoning models served by llama.cpp, vLLM and others return their thinking here (or as `reasoning`). */
      reasoning_content?: string | null;
      reasoning?: string | null;
      refusal?: string | null;
      tool_calls?: { id?: string; function?: { name?: string; arguments?: string } }[];
    };
  }[];
  usage?: { prompt_tokens?: number; completion_tokens?: number; prompt_tokens_details?: { cached_tokens?: number } };
}

function failure(message: string, status?: number): Error {
  return Object.assign(new Error(message), { status });
}

/** Arguments arrive as a JSON string; anything unparseable becomes an empty object (and is ignored). */
function parseArguments(raw: string | undefined): unknown {
  try {
    return raw ? JSON.parse(raw) : {};
  } catch {
    return {};
  }
}

/**
 * Any server that speaks the OpenAI Chat Completions API with tool calling: OpenAI, Gemini's
 * OpenAI-compatible endpoint, Groq, OpenRouter, or a self-hosted Ollama / vLLM. The stable part
 * of the system prompt goes first so servers that cache prompt prefixes can reuse it.
 */
export function makeOpenAIProvider(config: OpenAIConfig, fetchFn: typeof fetch = fetch): ModelProvider {
  const url = `${config.baseUrl.replace(/\/+$/, '')}/chat/completions`;
  const tools = TOOL_SPECS.map((tool) => ({
    type: 'function',
    function: { name: tool.name, description: tool.description, parameters: tool.schema },
  }));
  return {
    async ask(body: AskBody) {
      const { stable, volatile } = systemParts(body.context);
      const response = await fetchFn(url, {
        method: 'POST',
        headers: { Authorization: `Bearer ${config.apiKey}`, 'Content-Type': 'application/json' },
        body: JSON.stringify({
          model: config.model,
          [config.maxTokensField]: MAX_TOKENS,
          ...(config.temperature !== undefined ? { temperature: config.temperature } : {}),
          messages: [
            { role: 'system', content: `${stable}\n\n${volatile}` },
            ...body.messages.map((turn) => ({ role: turn.role, content: turn.text })),
            // What the model asked the app earlier, and what the app answered: its own call, then the answer, for each round.
            ...(body.lookups ?? []).flatMap((round) => [
              {
                role: 'assistant',
                content: null,
                tool_calls: round.calls.map((c) => ({ id: c.id, type: 'function', function: { name: 'lookup', arguments: JSON.stringify(c.input) } })),
              },
              ...round.results.map((r, i) => ({ role: 'tool', tool_call_id: round.calls[i]!.id, content: r })),
            ]),
          ],
          tools,
          tool_choice: 'auto',
        }),
        signal: AbortSignal.timeout(REQUEST_TIMEOUT_MS),
      });
      if (!response.ok) throw failure('openai-compatible request failed', response.status);

      let data: ChatResponse;
      try {
        data = (await response.json()) as ChatResponse;
      } catch {
        throw failure('openai-compatible response was not JSON', response.status);
      }
      const message = data.choices?.[0]?.message;
      if (!message) throw failure('openai-compatible response had no message', response.status);

      const usage = data.usage;
      if (usage) {
        console.log(
          `usage in=${usage.prompt_tokens ?? 0} out=${usage.completion_tokens ?? 0} cache_read=${usage.prompt_tokens_details?.cached_tokens ?? 0}`,
        );
      }

      if (message.refusal) return REFUSAL_RESULT;
      const calls: ToolCall[] = (message.tool_calls ?? []).flatMap((call) =>
        call.function?.name ? [{ name: call.function.name, input: parseArguments(call.function.arguments), id: call.id }] : [],
      );
      const result = interpret(message.content ? [message.content] : [], calls);
      const reasoning = message.reasoning_content || message.reasoning;
      return reasoning ? { ...result, reasoning } : result;
    },
  };
}
