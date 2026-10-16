import { describe, expect, it } from 'vitest';
import { loadPref, savePref, type KV } from './prefs';

const fake = (init: Record<string, string> = {}): KV & { data: Record<string, string> } => ({
  data: init,
  getItem(k) { return k in this.data ? this.data[k] : null; },
  setItem(k, v) { this.data[k] = v; },
});

describe('prefs', () => {
  it('round-trips JSON values', () => {
    const kv = fake();
    savePref('w', 300, kv);
    savePref('o', 'oldest', kv);
    expect(loadPref('w', 0, kv)).toBe(300);
    expect(loadPref('o', 'newest', kv)).toBe('oldest');
  });
  it('falls back when missing, corrupt, or storage is unavailable', () => {
    expect(loadPref('x', 7, fake())).toBe(7);
    expect(loadPref('x', 7, fake({ x: '{not json' }))).toBe(7);
    expect(loadPref('x', 7, null)).toBe(7);
    const throwing: KV = { getItem() { throw new Error('blocked'); }, setItem() { throw new Error('blocked'); } };
    expect(loadPref('x', 7, throwing)).toBe(7);
    expect(() => savePref('x', 1, throwing)).not.toThrow();
    expect(() => savePref('x', 1, null)).not.toThrow();
  });
});
