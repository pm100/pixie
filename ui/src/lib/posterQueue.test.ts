import { describe, expect, it } from 'vitest';
import { createPosterQueue } from './posterQueue';

function deferred() {
  let resolve!: (v: boolean) => void;
  const promise = new Promise<boolean>((r) => (resolve = r));
  return { promise, resolve };
}

describe('posterQueue', () => {
  it('runs one job at a time, newest request first', async () => {
    const started: number[] = [];
    const gates = new Map<number, ReturnType<typeof deferred>>();
    const q = createPosterQueue((id) => {
      started.push(id);
      const d = deferred();
      gates.set(id, d);
      return d.promise;
    });
    const results = [q.request(1), q.request(2), q.request(3)];
    expect(started).toEqual([1]); // the first starts at once; the rest wait
    gates.get(1)!.resolve(true);
    await results[0];
    expect(started).toEqual([1, 3]); // 3 asked after 2, so it goes first
    gates.get(3)!.resolve(true);
    await results[2];
    gates.get(2)!.resolve(false);
    expect(await results[1]).toBe(false);
    expect(started).toEqual([1, 3, 2]);
  });

  it('cancel removes a waiting job but never a running one', async () => {
    const started: number[] = [];
    const gates = new Map<number, ReturnType<typeof deferred>>();
    const q = createPosterQueue((id) => {
      started.push(id);
      const d = deferred();
      gates.set(id, d);
      return d.promise;
    });
    const a = q.request(1);
    const b = q.request(2);
    q.cancel(2);
    q.cancel(1); // already running: ignored
    gates.get(1)!.resolve(true);
    expect(await a).toBe(true);
    expect(await b).toBe(false); // cancelled before it started
    expect(started).toEqual([1]);
  });

  it('does not retry an id that failed, and shares an in-flight request', async () => {
    let runs = 0;
    const q = createPosterQueue(async () => {
      runs++;
      return false;
    });
    expect(await q.request(5)).toBe(false);
    expect(await q.request(5)).toBe(false);
    expect(runs).toBe(1);
    const a = q.request(6);
    const b = q.request(6);
    await Promise.all([a, b]);
    expect(runs).toBe(2);
  });

  it('a throwing job counts as a failure and the queue keeps going', async () => {
    const q = createPosterQueue(async (id) => {
      if (id === 1) throw new Error('decoder blew up');
      return true;
    });
    const [a, b] = [q.request(1), q.request(2)];
    expect(await a).toBe(false);
    expect(await b).toBe(true);
  });
});
