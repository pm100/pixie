import { describe, expect, it } from 'vitest';
import { retryDelay } from './retry';

describe('retryDelay', () => {
  it('backs off and then gives up', () => {
    expect(retryDelay(0)).toBe(400);
    expect(retryDelay(1)).toBeGreaterThan(retryDelay(0)!);
    expect(retryDelay(2)).toBeGreaterThan(retryDelay(1)!);
    expect(retryDelay(3)).toBeNull();
    expect(retryDelay(99)).toBeNull();
  });
});
