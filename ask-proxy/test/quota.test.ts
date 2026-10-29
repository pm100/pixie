import { describe, expect, it } from 'vitest';
import { dayKey, limitFor, nextReset, readQuota, recordUse } from '../src/quota';
import { FakeKV, INSTALL_ID } from './helpers';

const NOON = new Date('2026-10-01T12:00:00Z');

describe('quota', () => {
  it('uses the UTC day and resets at the next UTC midnight', () => {
    expect(dayKey(new Date('2026-10-01T23:59:59Z'))).toBe('2026-10-01');
    expect(nextReset(NOON)).toBe('2026-10-02T00:00:00.000Z');
    expect(nextReset(new Date('2026-12-31T23:00:00Z'))).toBe('2027-01-01T00:00:00.000Z');
  });

  it('defaults to 10 questions a day, overridable by env', () => {
    expect(limitFor({})).toBe(10);
    expect(limitFor({ DAILY_LIMIT: '3' })).toBe(3);
    expect(limitFor({ DAILY_LIMIT: 'lots' })).toBe(10);
    expect(limitFor({ DAILY_LIMIT: '-2' })).toBe(10);
  });

  it('starts at zero used and counts each recorded question', async () => {
    const kv = new FakeKV();
    let read = await readQuota(kv, INSTALL_ID, 5, NOON);
    expect(read.used).toBe(0);
    expect(read.quota).toEqual({ remaining: 5, resetsAt: '2026-10-02T00:00:00.000Z' });
    await recordUse(kv, INSTALL_ID, read.used, NOON);
    read = await readQuota(kv, INSTALL_ID, 5, NOON);
    expect(read.used).toBe(1);
    expect(read.quota.remaining).toBe(4);
  });

  it('never reports a negative remaining count', async () => {
    const kv = new FakeKV();
    await kv.put(`q:${INSTALL_ID}:2026-10-01`, '9');
    expect((await readQuota(kv, INSTALL_ID, 5, NOON)).quota.remaining).toBe(0);
  });

  it('counts each day separately, per install, and expires old counters', async () => {
    const kv = new FakeKV();
    await recordUse(kv, INSTALL_ID, 0, NOON);
    expect((await readQuota(kv, INSTALL_ID, 5, new Date('2026-10-02T00:00:01Z'))).used).toBe(0);
    expect((await readQuota(kv, '00000000-0000-4000-8000-000000000000', 5, NOON)).used).toBe(0);
    expect(kv.ttls.get(`q:${INSTALL_ID}:2026-10-01`)).toBe(172_800);
  });

  it('treats a corrupt stored counter as zero', async () => {
    const kv = new FakeKV();
    await kv.put(`q:${INSTALL_ID}:2026-10-01`, 'garbage');
    expect((await readQuota(kv, INSTALL_ID, 5, NOON)).used).toBe(0);
  });
});
