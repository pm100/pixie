/** What the app reports about the background picture analysis that makes Auto tags possible (mirrors the Rust `AnalysisStatus`). */
export type AnalysisStatus = {
  /** 'off': this analysis is switched off (Settings, Picture analysis), so nothing is fetched or done. */
  models: 'missing' | 'downloading' | 'ready' | 'error' | 'off';
  model_done: number;
  model_total: number;
  done: number;
  total: number;
  paused: boolean;
  error: string | null;
};
