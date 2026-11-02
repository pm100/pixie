/**
 * What Pause and Resume do. One Pause covers everything: the analysis is paused, and a folder scan that is running is stopped
 * (what it read is kept). Resume carries the analysis on and, if pausing had stopped a scan, starts it again (it only reads what is
 * new or changed, so it picks up roughly where it stopped).
 */
export type PauseActions = { analysisPaused: boolean; cancelScan: boolean; startScan: boolean };

export const pauseActions = (paused: boolean, scanning: boolean, scanPaused: boolean): PauseActions => ({
  analysisPaused: paused,
  cancelScan: paused && scanning,
  startScan: !paused && scanPaused,
});
