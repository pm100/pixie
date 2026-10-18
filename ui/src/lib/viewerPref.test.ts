import { describe, expect, it } from 'vitest';
import { parseViewerMode } from './viewerPref';

describe('parseViewerMode', () => {
  it('accepts the two real modes', () => {
    expect(parseViewerMode('window')).toBe('window');
    expect(parseViewerMode('popout')).toBe('popout');
  });
  it('anything else is the full-window viewer', () => {
    for (const v of [null, undefined, 3, 'tab', {}]) expect(parseViewerMode(v)).toBe('window');
  });
});
