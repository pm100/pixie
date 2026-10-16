/** The slice of Storage we use, so tests can pass a fake. */
export interface KV {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
}

function browserStore(): KV | null {
  try {
    return globalThis.localStorage ?? null;
  } catch {
    return null; // blocked or unavailable: preferences are a convenience only
  }
}

export function loadPref<T>(key: string, fallback: T, kv: KV | null = browserStore()): T {
  try {
    const s = kv?.getItem(key);
    return s == null ? fallback : (JSON.parse(s) as T);
  } catch {
    return fallback;
  }
}

export function savePref(key: string, value: unknown, kv: KV | null = browserStore()): void {
  try {
    kv?.setItem(key, JSON.stringify(value));
  } catch {
    /* ignore: not worth failing the UI over */
  }
}
