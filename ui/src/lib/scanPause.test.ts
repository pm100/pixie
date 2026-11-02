import { describe, expect, it } from 'vitest';
import { pauseActions } from './scanPause';

describe('Pause and Resume', () => {
  it('pausing pauses the analysis and stops a folder scan that is running', () => {
    expect(pauseActions(true, true, false)).toEqual({ analysisPaused: true, cancelScan: true, startScan: false });
  });

  it('pausing with no scan running only pauses the analysis', () => {
    expect(pauseActions(true, false, false)).toEqual({ analysisPaused: true, cancelScan: false, startScan: false });
  });

  it('resuming carries on the analysis and starts the scan again if pausing had stopped it', () => {
    expect(pauseActions(false, false, true)).toEqual({ analysisPaused: false, cancelScan: false, startScan: true });
    expect(pauseActions(false, false, false)).toEqual({ analysisPaused: false, cancelScan: false, startScan: false });
  });
});
