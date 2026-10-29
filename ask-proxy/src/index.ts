import { sendDigest } from './feedback';
import { handleAsk } from './handler';
import { makeProvider } from './provider';
import type { Env, ModelProvider } from './types';

export default {
  async fetch(request: Request, env: Env): Promise<Response> {
    const { pathname } = new URL(request.url);
    if (pathname === '/health' && request.method === 'GET') return new Response('ok');
    if (pathname === '/v1/ask' && request.method === 'POST') {
      // Built lazily so a misconfigured provider fails inside the handler (a logged 502).
      const provider: ModelProvider = { ask: (body) => makeProvider(env).ask(body) };
      return handleAsk(request, env, { provider, now: () => new Date() });
    }
    return new Response('not found', { status: 404 });
  },

  async scheduled(_event: ScheduledController, env: Env, ctx: ExecutionContext): Promise<void> {
    ctx.waitUntil(
      sendDigest(env).then(
        (count) => console.log(`feedback digest sent: ${count} item(s)`),
        (error) => console.error(`feedback digest failed: ${(error as Error).message}`),
      ),
    );
  },
};
