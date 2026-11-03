import { describe, expect, it } from 'vitest';
import { pauseActions } from './scanPause';

describe('Pause and Resume', () => {
  const none = { analysisPaused: false, cancelScan: false, startScan: false, stopSync: false, startSync: false };

  it('pausing pauses the analysis and stops a folder scan that is running', () => {
    expect(pauseActions(true, true, false)).toEqual({ ...none, analysisPaused: true, cancelScan: true });
  });

  it('pausing with no scan running only pauses the analysis', () => {
    expect(pauseActions(true, false, false)).toEqual({ ...none, analysisPaused: true });
  });

  it('resuming carries on the analysis and starts the scan again if pausing had stopped it', () => {
    expect(pauseActions(false, false, true)).toEqual({ ...none, startScan: true });
    expect(pauseActions(false, false, false)).toEqual(none);
  });

  it('pausing also stops a sync that is running, and resuming starts it again', () => {
    expect(pauseActions(true, false, false, true, false)).toEqual({ ...none, analysisPaused: true, stopSync: true });
    expect(pauseActions(true, false, false, false, false).stopSync).toBe(false);
    expect(pauseActions(false, false, false, false, true)).toEqual({ ...none, startSync: true });
  });
});
