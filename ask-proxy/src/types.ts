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

/** What the user has allowed the model to know about their library (Settings, Ask Pixie): counts and names only. Never pictures,
 *  file or folder names, paths or coordinates. */
export interface Library {
  photos: number;
  picked: number;
  tags: { name: string; count: number }[];
  people: { name: string; count: number }[];
  trips: { name: string | null; from: string; to: string; nights: number; photos: number; videos: number; where: string | null }[];
}

/** The last search that found something, as the person has it in Tools (Ask Pixie results): how many, and in words what they are. */
export interface Results {
  count: number;
  of: string;
}

/** Open a tool on photos the person already has, so a follow-up like "print them" needs no new search. */
export interface OpenToolArgs {
  tool: 'details' | 'email' | 'print' | 'tags';
  source: 'results' | 'picked' | 'tag';
  tag: string | null;
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
  /** Present only when the user allowed it. */
  library?: Library;
  /** Present when a search has found something in this conversation. */
  results?: Results;
}

/** One of the model's own calls to `lookup`, kept so it can be put back in front of the model with its answer. */
export interface LookupCall {
  id: string;
  input: Record<string, unknown>;
}

/** The model asked the app something and got the answers back: its calls, and for each the text the app answered with. */
export interface LookupRound {
  calls: LookupCall[];
  results: string[];
}

/** What the model wants to know before it answers. The app answers on the user's computer and asks again with the answer. */
export interface LookupRequest extends LookupCall {
  what: 'count' | 'tags' | 'people' | 'trips';
  /** For tags, people and trips: part of a name to look for. */
  query: string | null;
  /** For a count: which photos to count (null: all of them). */
  filter: FindArgs | null;
}

export interface AskBody {
  messages: Turn[];
  context: Context;
  /** Earlier rounds of the model asking the app and being answered, oldest first (at most three). */
  lookups?: LookupRound[];
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

/** What to look for among the user's own photos, as the model described it. Names are matched on the user's computer. */
export interface FindArgs {
  /** What is in the picture (colours, objects, scenes), for the content search. */
  text: string | null;
  /** More things that must also be in the picture ("yellow flowers" and "a birthday cake"): each is searched on its own. */
  also: string[];
  /** Photos with a person in them, whoever (a real face, not one in a painting). */
  withPeople: boolean;
  /** The user's own tags, by name (all must match). */
  tags: string[];
  /** People, by name (all must be in the photo). */
  people: string[];
  /** YYYY-MM-DD, inclusive. */
  from: string | null;
  to: string | null;
  kind: 'photo' | 'video' | null;
  /** How many the user asked for ("find 20 pictures"): the best matches, at most this many. */
  limit: number | null;
  pickedOnly: boolean;
  /** What the user wants done with what is found, besides seeing it. */
  then: ('pick' | 'tag' | 'email' | 'print')[];
  tag: string | null;
}

export interface ModelResult {
  /** The model's own reasoning, when the provider returns it. For review runs only: never sent to the apps. */
  reasoning?: string;
  reply: string;
  proposal: Proposal[] | null;
  /** A place in the app to offer a button for (one of GO_TO_TARGETS), or null. */
  goTo: string | null;
  /** A search of the user's photos for the app to run (it asks the user before it changes anything). */
  find: FindArgs | null;
  /** A tool to open on the results, the picked photos or a tag (the user presses a button). */
  openTool: OpenToolArgs | null;
  /** Questions for the app to answer before the model carries on (empty: none). */
  lookup: LookupRequest[];
  feedback: Feedback[];
}

/** A tool the model called, as any provider reports it. */
export interface ToolCall {
  name: string;
  input: unknown;
  /** The provider's id for the call (needed to answer a lookup). */
  id?: string;
}

/**
 * The one thing the service needs from a language model: answer a question. Providers throw on
 * any failure; an error may carry a numeric `status` (the HTTP status, for logging).
 */
export interface ModelProvider {
  ask(body: AskBody): Promise<ModelResult>;
}
