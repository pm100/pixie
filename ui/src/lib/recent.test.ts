import { describe, expect, it } from 'vitest';
import { recentSince } from './recent';

describe('recentSince', () => {
  it('is 30 days before now by default', () => {
    expect(recentSince(1_000_000_000)).toBe(1_000_000_000 - 30 * 86_400);
  });
  it('takes a different span', () => {
    expect(recentSince(1_000_000_000, 7)).toBe(1_000_000_000 - 7 * 86_400);
  });
});
