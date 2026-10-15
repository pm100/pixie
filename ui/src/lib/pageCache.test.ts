import { describe, expect, it } from 'vitest';
import { PageCache } from './pageCache';

const tick = () => new Promise((r) => setTimeout(r, 0));

function harness() {
  const calls: number[] = [];
  const pending: Array<{ resolve: (rows: string[]) => void; reject: (e: unknown) => void }> = [];
  const fetchPage = (offset: number, _limit: number) => {
    calls.push(offset);
    return new Promise<string[]>((resolve, reject) => pending.push({ resolve, reject }));
  };
  let changes = 0;
  const cache = new PageCache<string>(fetchPage, 200, () => changes++);
  return { cache, calls, pending, changes: () => changes };
}

describe('PageCache', () => {
  it('fetches each needed page once', () => {
    const h = harness();
    h.cache.ensure(0, 250);
    h.cache.ensure(0, 250);
    expect(h.calls).toEqual([0, 200]);
  });

  it('stores rows at their absolute index and notifies', async () => {
    const h = harness();
    h.cache.ensure(200, 203);
    h.pending[0].resolve(['a', 'b', 'c']);
    await tick();
    expect(h.cache.items[200]).toBe('a');
    expect(h.cache.items[202]).toBe('c');
    expect(h.changes()).toBeGreaterThan(0);
  });

  it('refetches after reset, so the grid is not left blank after a rescan', async () => {
    const h = harness();
    h.cache.ensure(0, 10);
    h.pending[0].resolve(['a']);
    await tick();
    h.cache.reset();
    expect(h.cache.items[0]).toBeUndefined();
    h.cache.ensure(0, 10);
    expect(h.calls).toEqual([0, 0]);
  });

  it('drops a response that arrives after a reset', async () => {
    const h = harness();
    h.cache.ensure(0, 10);
    h.cache.reset();
    h.pending[0].resolve(['stale']);
    await tick();
    expect(h.cache.items[0]).toBeUndefined();
    h.cache.ensure(0, 10);
    h.pending[1].resolve(['fresh']);
    await tick();
    expect(h.cache.items[0]).toBe('fresh');
  });

  it('retries a page whose fetch failed', async () => {
    const h = harness();
    h.cache.ensure(0, 10);
    h.pending[0].reject(new Error('boom'));
    await tick();
    h.cache.ensure(0, 10);
    expect(h.calls).toEqual([0, 0]);
  });
});
