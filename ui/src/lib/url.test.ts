import { describe, expect, it } from 'vitest';
import { thumbUrl } from './url';

describe('thumbUrl', () => {
  it('uses the http host form on Windows', () => {
    expect(thumbUrl('grid', 12, 345, true)).toBe('http://pixie.localhost/grid/12?v=345');
  });
  it('uses the custom scheme elsewhere', () => {
    expect(thumbUrl('view', 7, 1, false)).toBe('pixie://localhost/view/7?v=1');
  });
});
