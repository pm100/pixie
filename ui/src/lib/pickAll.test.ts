import { describe, expect, it } from 'vitest';
import { pickAllState } from './pickAll';

describe('pickAllState', () => {
  it('is available for any number shown, with no limit', () => {
    expect(pickAllState(312, false)).toEqual({ enabled: true, title: 'Pick all 312 photos shown' });
    expect(pickAllState(1, false).title).toBe('Pick the 1 photo shown');
    expect(pickAllState(250_000, false)).toEqual({ enabled: true, title: 'Pick all 250,000 photos shown' });
  });
  it('says the photos are added to the ones picked now', () => {
    expect(pickAllState(312, false, 5).title).toBe('Pick all 312 photos shown, adding them to the 5 picked now');
    expect(pickAllState(312, false, 1).title).toBe('Pick all 312 photos shown, adding them to the 1 picked now');
  });
  it('is off when nothing is shown', () => {
    expect(pickAllState(0, false)).toEqual({ enabled: false, title: 'No photos are shown' });
  });
  it('is off while only the picked photos are shown, since they are picked already', () => {
    expect(pickAllState(50, true).enabled).toBe(false);
  });
});
