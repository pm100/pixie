import { describe, expect, it } from 'vitest';
import { parseKindFilter } from './kind';

describe('parseKindFilter', () => {
  it('accepts the two kinds and treats anything else as no filter', () => {
    expect(parseKindFilter('photo')).toBe('photo');
    expect(parseKindFilter('video')).toBe('video');
    for (const v of [null, undefined, '', 'all', 'VIDEO', 3, {}]) expect(parseKindFilter(v)).toBeNull();
  });
});
