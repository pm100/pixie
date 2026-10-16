import { describe, expect, it } from 'vitest';
import { thumbUrl } from './url';

describe('thumbUrl', () => {
  it('uses the http host form on Windows', () => {
    expect(thumbUrl('grid', 12, 345, 678, true)).toBe('http://pixie.localhost/grid/12?v=345-678');
  });
  it('uses the custom scheme elsewhere', () => {
    expect(thumbUrl('view', 7, 1, 2, false)).toBe('pixie://localhost/view/7?v=1-2');
  });
  it('adds a cache-busting retry counter only when retrying', () => {
    expect(thumbUrl('grid', 1, 2, 3, true, 0)).toBe('http://pixie.localhost/grid/1?v=2-3');
    expect(thumbUrl('grid', 1, 2, 3, true, 2)).toBe('http://pixie.localhost/grid/1?v=2-3&r=2');
  });
});
