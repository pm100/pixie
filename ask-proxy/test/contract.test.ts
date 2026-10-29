import { describe, expect, it } from 'vitest';
import fixtureJson from './fixtures/context.json';
import { handleAsk } from '../src/handler';
import { systemParts } from '../src/prompt';
import type { Context } from '../src/types';
import { MAX_BODY_BYTES, parseAsk } from '../src/validate';
import { askRequest, makeEnv, stubProvider } from './helpers';

// test/fixtures/context.json is the app's own settings description for the default settings
// (regenerate from ui/: WRITE_ASK_FIXTURE=1 npx vitest run src/lib/askSettings.test.ts).
// These tests keep the proxy's validator and prompt in step with what the app really sends.
const fixture = fixtureJson as unknown as Context;
const body = { messages: [{ role: 'user', text: 'Stop it looking for faces' }], context: fixture };

describe('contract with the app', () => {
  it("accepts the app's real context unchanged", () => {
    const parsed = parseAsk(body);
    expect(parsed.ok).toBe(true);
    expect(parsed.ok && parsed.body.context.settings).toHaveLength(fixture.settings.length);
  });

  it('fits comfortably inside the body size limit', () => {
    expect(JSON.stringify(body).length).toBeLessThan(MAX_BODY_BYTES / 2);
  });

  it('puts every setting, with its options, into the cached prompt block', () => {
    const { stable } = systemParts(fixture);
    for (const setting of fixture.settings) expect(stable).toContain(`"key":"${setting.key}"`);
    expect(stable).toContain('Find faces');
    expect(stable).toContain('Its own window');
  });

  it('runs through the handler end to end with a stubbed model', async () => {
    const { provider } = stubProvider({ reply: 'Turn off Find faces.' });
    const response = await handleAsk(askRequest(body), makeEnv(), { provider, now: () => new Date('2026-10-01T12:00:00Z') });
    expect(response.status).toBe(200);
  });
});
