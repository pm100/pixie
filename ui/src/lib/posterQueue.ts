/**
 * Makes poster frames one at a time. The newest request runs first (it is what the user
 * is looking at); a failed id is never retried in this session.
 */
export function createPosterQueue(run: (id: number) => Promise<boolean>) {
  const waiting: number[] = [];
  const waiters = new Map<number, ((ok: boolean) => void)[]>();
  const failed = new Set<number>();
  let running: number | null = null;

  function settle(id: number, ok: boolean) {
    if (!ok) failed.add(id);
    for (const w of waiters.get(id) ?? []) w(ok);
    waiters.delete(id);
  }

  async function pump() {
    if (running !== null) return;
    const id = waiting.pop();
    if (id === undefined) return;
    running = id;
    let ok = false;
    try {
      ok = await run(id);
    } catch {
      ok = false;
    }
    running = null;
    settle(id, ok);
    void pump();
  }

  return {
    request(id: number): Promise<boolean> {
      if (failed.has(id)) return Promise.resolve(false);
      return new Promise<boolean>((resolve) => {
        const list = waiters.get(id);
        if (list) list.push(resolve);
        else waiters.set(id, [resolve]);
        if (id !== running && !waiting.includes(id)) waiting.push(id);
        void pump();
      });
    },
    /** Drop a job that has not started (its tile scrolled away). It may be requested again. */
    cancel(id: number) {
      const i = waiting.indexOf(id);
      if (i >= 0) {
        waiting.splice(i, 1);
        for (const w of waiters.get(id) ?? []) w(false);
        waiters.delete(id);
      }
    },
  };
}
