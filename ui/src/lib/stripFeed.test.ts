import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createStripFeed, type StripItem } from './stripFeed';

const item = (id: number, kind: 'photo' | 'video' = 'photo'): StripItem => ({ id, mtime: 1, size: 1, name: `IMG_${id}.${kind === 'photo' ? 'jpg' : 'mp4'}`, kind });

/** A load that finishes when the test says so. */
function deferred() {
  let done!: (ok: boolean) => void;
  const promise = new Promise<boolean>((resolve) => (done = resolve));
  return { promise, done };
}
const flush = () => vi.advanceTimersByTimeAsync(0);

beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());

describe('createStripFeed', () => {
  it('shows a photo once it has loaded, and not before', async () => {
    const d = deferred();
    const shown: number[] = [];
    const feed = createStripFeed({ load: () => d.promise, onShow: (i) => shown.push(i.id), concurrency: 1 });
    feed.offer(item(1));
    await flush();
    expect(shown).toEqual([]);
    d.done(true);
    await flush();
    expect(shown).toEqual([1]);
  });

  it('while a load is busy only the newest offer waits; the ones in between are skipped', async () => {
    const loads: Record<number, ReturnType<typeof deferred>> = { 1: deferred(), 4: deferred() };
    const asked: number[] = [];
    const shown: number[] = [];
    const feed = createStripFeed({ load: (i) => { asked.push(i.id); return loads[i.id].promise; }, onShow: (i) => shown.push(i.id), concurrency: 1 });
    feed.offer(item(1));
    feed.offer(item(2));
    feed.offer(item(3));
    feed.offer(item(4));
    await flush();
    expect(asked).toEqual([1]);
    loads[1].done(true);
    await flush();
    expect(asked).toEqual([1, 4]); // 2 and 3 were stale by the time there was room
    loads[4].done(true);
    await vi.advanceTimersByTimeAsync(500);
    expect(shown).toEqual([1, 4]);
  });

  it('a video is shown at once without loading anything', async () => {
    const load = vi.fn(async () => true);
    const shown: number[] = [];
    const feed = createStripFeed({ load, onShow: (i) => shown.push(i.id) });
    feed.offer(item(7, 'video'));
    await flush();
    expect(load).not.toHaveBeenCalled();
    expect(shown).toEqual([7]);
  });

  it('a photo that fails to load is left out', async () => {
    const shown: number[] = [];
    const feed = createStripFeed({ load: async () => false, onShow: (i) => shown.push(i.id) });
    feed.offer(item(1));
    await vi.advanceTimersByTimeAsync(500);
    expect(shown).toEqual([]);
  });

  it('shows at most one item per minGapMs: a second that is ready early waits its turn', async () => {
    const shown: { id: number; at: number }[] = [];
    const feed = createStripFeed({ load: async () => true, onShow: (i) => shown.push({ id: i.id, at: Date.now() }), minGapMs: 100, concurrency: 1 });
    const t0 = Date.now();
    feed.offer(item(1));
    await flush();
    feed.offer(item(2));
    await vi.advanceTimersByTimeAsync(10);
    expect(shown.map((s) => s.id)).toEqual([1]);
    await vi.advanceTimersByTimeAsync(200);
    expect(shown.map((s) => s.id)).toEqual([1, 2]);
    expect(shown[1].at - shown[0].at).toBeGreaterThanOrEqual(100);
    expect(shown[0].at - t0).toBeLessThan(100);
  });

  it('with several loads at once an older photo that finishes late is not shown after a newer one', async () => {
    const loads: Record<number, ReturnType<typeof deferred>> = { 1: deferred(), 2: deferred() };
    const shown: number[] = [];
    const feed = createStripFeed({ load: (i) => loads[i.id].promise, onShow: (i) => shown.push(i.id), concurrency: 2, minGapMs: 0 });
    feed.offer(item(1));
    feed.offer(item(2));
    await flush();
    loads[2].done(true);
    await flush();
    loads[1].done(true);
    await vi.advanceTimersByTimeAsync(300);
    expect(shown).toEqual([2]);
  });

  it('stop() ends it: nothing more is loaded or shown', async () => {
    const d = deferred();
    const load = vi.fn(() => d.promise);
    const shown: number[] = [];
    const feed = createStripFeed({ load, onShow: (i) => shown.push(i.id), concurrency: 1 });
    feed.offer(item(1));
    await flush();
    feed.stop();
    d.done(true);
    feed.offer(item(2));
    await vi.advanceTimersByTimeAsync(500);
    expect(shown).toEqual([]);
    expect(load).toHaveBeenCalledTimes(1);
  });
});
