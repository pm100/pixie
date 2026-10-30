import { describe, expect, it } from 'vitest';
import { PICK_ALL_LIMIT, pickAllState } from './pickAll';

describe('pickAllState', () => {
  it('is available for a set that fits', () => {
    expect(pickAllState(312, false)).toEqual({ enabled: true, title: 'Pick all 312 photos shown' });
    expect(pickAllState(PICK_ALL_LIMIT, false).enabled).toBe(true);
  });
  it('is off when nothing is shown', () => {
    expect(pickAllState(0, false)).toEqual({ enabled: false, title: 'No photos are shown' });
  });
  it('is off over the limit, and says how to get under it', () => {
    const s = pickAllState(PICK_ALL_LIMIT + 1, false);
    expect(s.enabled).toBe(false);
    expect(s.title).toContain('Narrow the filters');
    expect(s.title).toContain('2,001');
  });
  it('is off while only the picked photos are shown, since they are picked already', () => {
    expect(pickAllState(50, true).enabled).toBe(false);
  });
  it('takes the limit as a parameter', () => {
    expect(pickAllState(11, false, 10).enabled).toBe(false);
  });
});
