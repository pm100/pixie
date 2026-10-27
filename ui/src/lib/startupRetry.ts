/** The window can load, and start asking for data, before the backend has finished opening the library; Tauri then refuses
 *  the call with "state not managed". A call refused that way is simply tried again a moment later. */
const NOT_READY = /state not managed/i;
const WAIT_MS = 250;
const TRIES = 120; // about 30 seconds

export const isNotReady = (e: unknown) => NOT_READY.test(String(e));

export async function untilReady<T>(call: () => Promise<T>, wait: (ms: number) => Promise<void> = (ms) => new Promise((r) => setTimeout(r, ms))): Promise<T> {
  for (let i = 0; ; i++) {
    try {
      return await call();
    } catch (e) {
      if (i >= TRIES || !isNotReady(e)) throw e;
      await wait(WAIT_MS);
    }
  }
}
