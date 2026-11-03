/**
 * What Pause and Resume do. One Pause covers everything: the analysis is paused, a folder scan that is running is stopped (what it read
 * is kept), and so is a sync into the sync folder (it stops after the file it is on). Resume carries the analysis on and, if pausing
 * had stopped a scan or a sync, starts it again (a scan only reads what is new or changed, a sync only does what is still waiting, so
 * each picks up roughly where it stopped).
 */
export type PauseActions = { analysisPaused: boolean; cancelScan: boolean; startScan: boolean; stopSync: boolean; startSync: boolean };

export const pauseActions = (paused: boolean, scanning: boolean, scanPaused: boolean, syncing = false, syncPaused = false): PauseActions => ({
  analysisPaused: paused,
  cancelScan: paused && scanning,
  startScan: !paused && scanPaused,
  stopSync: paused && syncing,
  startSync: !paused && syncPaused,
});
