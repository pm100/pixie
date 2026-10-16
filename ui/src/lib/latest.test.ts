import { describe, it, expect } from 'vitest';
import { createLatest } from './latest';

describe('createLatest', () => {
  it('only the newest token is current', () => {
    const l = createLatest();
    const a = l.next();
    expect(l.isCurrent(a)).toBe(true);
    const b = l.next();
    expect(l.isCurrent(a)).toBe(false);
    expect(l.isCurrent(b)).toBe(true);
  });
});
