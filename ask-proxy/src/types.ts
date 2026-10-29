/** The slice of a Workers KV namespace this service uses (lets tests pass a fake). */
export interface KV {
  get(key: string): Promise<string | null>;
  put(key: string, value: string, options?: { expirationTtl?: number }): Promise<void>;
  list(options: {
    prefix: string;
    cursor?: string;
  }): Promise<{ keys: { name: string }[]; list_complete: boolean; cursor?: string }>;
}

export interface Env {
  QUOTA: KV;
  FEEDBACK: KV;
  RESEND_API_KEY: string;
  DIGEST_TO: string;
  DIGEST_FROM: string;
  /** Questions one install may ask per UTC day. */
  DAILY_LIMIT?: string;
  /** Debug builds of Pixie send this in X-Dev-Key and are not limited. A secret; unset means nobody is exempt. */
  DEV_KEY?: string;
  /** Which model backend answers: "anthropic" (default) or "openai" (any OpenAI-compatible API). */
  PROVIDER?: string;
  /** The model id for the chosen provider. Optional for anthropic, required for openai. */
  MODEL?: string;
  ANTHROPIC_API_KEY?: string;
  /** e.g. https://api.openai.com/v1, https://generativelanguage.googleapis.com/v1beta/openai, http://host:11434/v1 */
  OPENAI_BASE_URL?: string;
  OPENAI_API_KEY?: string;
  /** Some servers want "max_completion_tokens" instead of the default "max_tokens". */
  OPENAI_MAX_TOKENS_FIELD?: string;
  /** 0 to 2. Lower is steadier tool use; unset for models that reject it. */
  OPENAI_TEMPERATURE?: string;
  /** Review runs only: token budget for Anthropic extended thinking (see scripts/live-prompts.mjs). Leave unset in production. */
  THINKING_BUDGET?: string;
}

export interface Turn {
  role: 'user' | 'assistant';
  text: string;
}

/** One setting as the app describes it (ui/src/lib/askSettings.ts). */
export interface SettingInfo {
  key: string;
  label: string;
  description: string;
  type: 'choice' | 'int' | 'float' | 'bool';
  options?: { value: number; label: string }[];
  min?: number;
  max?: number;
  step?: number;
  current?: number | boolean;
  currentLabel?: string;
  default?: number | boolean;
}

export interface Context {
  platform: string;
  /** The BCP 47 language of the app, for the reply. */
  language: string;
  /** The tab the person is on (browse, tools, people, duplicates, settings, ask). */
  tab: string;
  /** The user's date, YYYY-MM-DD, so "last summer" can be worked out. */
  today: string;
  settings: SettingInfo[];
}

export interface AskBody {
  messages: Turn[];
  context: Context;
}

export interface Quota {
  remaining: number;
  resetsAt: string;
}

export interface Proposal {
  setting: string;
  value: number | boolean;
}

export interface Feedback {
  category: string;
  summary: string;
}

export interface ModelResult {
  /** The model's own reasoning, when the provider returns it. For review runs only: never sent to the apps. */
  reasoning?: string;
  reply: string;
  proposal: Proposal[] | null;
  /** A place in the app to offer a button for (one of GO_TO_TARGETS), or null. */
  goTo: string | null;
  feedback: Feedback[];
}

/** A tool the model called, as any provider reports it. */
export interface ToolCall {
  name: string;
  input: unknown;
}

/**
 * The one thing the service needs from a language model: answer a question. Providers throw on
 * any failure; an error may carry a numeric `status` (the HTTP status, for logging).
 */
export interface ModelProvider {
  ask(body: AskBody): Promise<ModelResult>;
}
