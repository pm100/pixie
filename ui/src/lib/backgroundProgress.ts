import type { AnalysisStatus } from './analysis';
import { readFraction, scanText, type ScanState } from './scanProgress';

/** What the small progress indicator at the top right of the window shows while Pixie works in the background. */
export type BgProgress = {
  state: 'working' | 'paused' | 'error';
  /** The short line shown in the indicator. */
  text: string;
  /** 0..1, or null when the amount is not known yet. */
  fraction: number | null;
  /** The longer explanation, for the tooltip. */
  detail: string;
};

const n = (x: number) => x.toLocaleString();

/** The progress to show, or null when nothing is running (the indicator then disappears). Errors come first, then the
 *  folder scan, then a model download, then the analysis (Auto tags and faces together), then paused work. */
export function backgroundProgress(o: {
  scanning: boolean; scan: ScanState; analysis: AnalysisStatus | null; people: AnalysisStatus | null; dupes?: AnalysisStatus | null;
}): BgProgress | null {
  const passes = [
    { name: 'Auto tags', s: o.analysis },
    { name: 'Faces', s: o.people },
    { name: 'Duplicates', s: o.dupes ?? null },
  ].filter((p): p is { name: string; s: AnalysisStatus } => p.s !== null);

  const failed = passes.find((p) => p.s.models === 'error' || p.s.error);
  if (failed) {
    return {
      state: 'error', text: 'Needs attention', fraction: null,
      detail: `${failed.name}: ${failed.s.error ?? 'could not get the model'}. Click for details.`,
    };
  }

  if (o.scanning) {
    return { state: 'working', text: scanText(o.scan), fraction: readFraction(o.scan), detail: 'Reading your photo folders' };
  }

  const downloading = passes.filter((p) => p.s.models === 'downloading');
  if (downloading.length > 0) {
    const fractions = downloading.map((p) => (p.s.model_total > 0 ? Math.min(1, p.s.model_done / p.s.model_total) : 0));
    const fraction = fractions.reduce((a, b) => a + b, 0) / fractions.length;
    return { state: 'working', text: `Getting models… ${Math.floor(fraction * 100)}%`, fraction, detail: 'Downloading the picture-analysis and face models, once' };
  }

  const left = passes.filter((p) => p.s.models === 'ready' && p.s.total > 0 && p.s.done < p.s.total);
  if (left.length === 0) return null;
  // The overall percentage is the whole story here; the counts for each step are in Settings, Picture analysis.
  const detail = `Still working on: ${left.map((p) => p.name).join(', ')}`;
  if (left.every((p) => p.s.paused)) {
    return { state: 'paused', text: 'Analysis paused', fraction: null, detail: `${detail}. Click to resume.` };
  }
  const done = left.reduce((a, p) => a + p.s.done, 0);
  const total = left.reduce((a, p) => a + p.s.total, 0);
  const fraction = total > 0 ? done / total : null;
  return { state: 'working', text: `Analysing photos ${Math.floor((fraction ?? 0) * 100)}%`, fraction, detail };
}
