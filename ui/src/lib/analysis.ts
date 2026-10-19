/** What the app reports about the background analysis that makes search possible (mirrors the Rust `AnalysisStatus`). */
export type AnalysisStatus = {
  models: 'missing' | 'downloading' | 'ready' | 'error';
  model_done: number;
  model_total: number;
  done: number;
  total: number;
  paused: boolean;
  error: string | null;
};

const n = (x: number) => x.toLocaleString();

/** One line for Settings: what is happening right now. */
export function analysisLine(s: AnalysisStatus): string {
  if (s.models === 'error') return `Could not get the search model: ${s.error ?? 'unknown error'}`;
  if (s.models === 'missing') return 'Search model not downloaded yet';
  if (s.models === 'downloading') {
    const pct = s.model_total > 0 ? Math.floor((s.model_done * 100) / s.model_total) : 0;
    return `Downloading search model… ${pct}%`;
  }
  if (s.error) return `Analysis is having trouble: ${s.error}`; // the model is loaded but failing on photos
  if (s.total === 0) return 'No photos to analyse yet';
  if (s.paused && s.done < s.total) return `Paused at ${n(s.done)} of ${n(s.total)}`;
  if (s.done >= s.total) return `All ${n(s.total)} photos analysed`;
  return `Analysing photos… ${n(s.done)} of ${n(s.total)}`;
}

/** How far along the current step is, 0 to 1; null when there is no bar to show. */
export function analysisFraction(s: AnalysisStatus): number | null {
  if (s.models === 'downloading') return s.model_total > 0 ? Math.min(1, s.model_done / s.model_total) : 0;
  if (s.models === 'ready' && s.total > 0) return Math.min(1, s.done / s.total);
  return null;
}

/** A caveat shown with search results while the library is only partly analysed. */
export function searchNote(s: AnalysisStatus): string | null {
  if (s.models !== 'ready' || s.total === 0 || s.done >= s.total) return null;
  return `Searching the ${n(s.done)} of ${n(s.total)} photos analysed so far`;
}
