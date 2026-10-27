import { describe, expect, it } from 'vitest';
import { isNotReady, untilReady } from './startupRetry';

const notReady = 'state not managed for field `state` on command `list_folders`. You must call `.manage()` before using this command';

describe('untilReady', () => {
  it('tries again while the backend is not ready, then gives the answer', async () => {
    let calls = 0, waits = 0;
    const r = await untilReady(async () => { if (++calls < 3) throw notReady; return 'ok'; }, async () => { waits++; });
    expect([r, calls, waits]).toEqual(['ok', 3, 2]);
  });

  it('passes any other error straight on', async () => {
    let calls = 0;
    await expect(untilReady(async () => { calls++; throw 'database: disk full'; }, async () => {})).rejects.toBe('database: disk full');
    expect(calls).toBe(1);
  });

  it('gives up if it never becomes ready', async () => {
    let calls = 0;
    await expect(untilReady(async () => { calls++; throw notReady; }, async () => {})).rejects.toBe(notReady);
    expect(calls).toBe(121);
  });

  it('recognises the refusal', () => {
    expect(isNotReady(notReady)).toBe(true);
    expect(isNotReady(new Error('state not managed for field'))).toBe(true);
    expect(isNotReady('no such folder')).toBe(false);
  });
});
