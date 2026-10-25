import { describe, expect, it } from 'vitest';
import { DEFAULT_NETWORK_RESCAN, parseNetworkRescan, rescanMinutes } from './networkRescan';

describe('parseNetworkRescan', () => {
  it('uses the default when nothing usable was saved', () => {
    for (const raw of [null, undefined, 5, 'x', []]) expect(parseNetworkRescan(raw)).toEqual(DEFAULT_NETWORK_RESCAN);
  });
  it('keeps a saved choice', () => {
    expect(parseNetworkRescan({ on: false, minutes: 60 })).toEqual({ on: false, minutes: 60 });
  });
  it('turns an interval that is not offered into the nearest one that is', () => {
    expect(parseNetworkRescan({ on: true, minutes: 20 }).minutes).toBe(15);
    expect(parseNetworkRescan({ on: true, minutes: 1000 }).minutes).toBe(120);
    expect(parseNetworkRescan({ on: true, minutes: -3 }).minutes).toBe(5);
    expect(parseNetworkRescan({ on: 'yes', minutes: 'soon' })).toEqual(DEFAULT_NETWORK_RESCAN);
  });
});

describe('rescanMinutes', () => {
  it('is the interval when on and nothing when off', () => {
    expect(rescanMinutes({ on: true, minutes: 15 })).toBe(15);
    expect(rescanMinutes({ on: false, minutes: 15 })).toBeNull();
  });
});
