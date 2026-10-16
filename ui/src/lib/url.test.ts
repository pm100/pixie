import { describe, expect, it } from 'vitest';
import { posterUrl, thumbUrl, tileTemplate, videoUrl } from './url';

describe('tileTemplate', () => {
  it('uses the windows host on windows', () => {
    expect(tileTemplate(true)).toBe('http://pixie.localhost/tile/{z}/{x}/{y}');
  });
  it('uses the pixie scheme elsewhere', () => {
    expect(tileTemplate(false)).toBe('pixie://localhost/tile/{z}/{x}/{y}');
  });
});

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

describe('videoUrl and posterUrl', () => {
  it('use the windows host on windows and carry the cache key', () => {
    expect(videoUrl(5, 10, 20, true)).toBe('http://pixie.localhost/video/5?v=10-20');
    expect(posterUrl(5, 10, 20, true)).toBe('http://pixie.localhost/poster/5?v=10-20');
    expect(posterUrl(5, 10, 20, false, 2)).toBe('pixie://localhost/poster/5?v=10-20&r=2');
  });
});
