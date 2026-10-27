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
  /** Folders were just added and are not all analysed yet: how far those have got (the bar then shows that, not the whole library). */
  load?: { fraction: number | null } | null;
  /** The loading (reading the files found, making their thumbnails), which goes on whatever analyses are on: files still to load,
   *  the most there were at once since it was last idle, and whether the Pause is on. */
  loading?: { pending: number; peak: number; paused: boolean } | null;
}): BgProgress | null {
  const passes = [
    { name: 'Auto tags', s: o.analysis },
    { name: 'Faces', s: o.people },
    { name: 'Duplicates', s: o.dupes ?? null },
  ].filter((p): p is { name: string; s: AnalysisStatus } => p.s !== null && p.s.models !== 'off'); // (one switched off is not part of the work)

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

  // With every analysis switched off the only work is reading the files and making their thumbnails: that is "loading".
  // With any analysis on, the pill says "Analysing" and shows the same figure as Settings, Picture analysis.
  if (passes.length === 0 && o.loading && o.loading.pending > 0) {
    const { pending, peak, paused } = o.loading;
    if (paused) return { state: 'paused', text: 'Loading paused', fraction: null, detail: 'Loading the photos is paused. Click to resume.' };
    const fraction = peak > 0 ? Math.min(1, (peak - pending) / peak) : null;
    return { state: 'working', text: `Loading photos ${Math.floor((fraction ?? 0) * 100)}%`, fraction, detail: 'Reading the photos Pixie found and making their thumbnails' };
  }

  const downloading = passes.filter((p) => p.s.models === 'downloading');
  if (downloading.length > 0) {
    const fractions = downloading.map((p) => (p.s.model_total > 0 ? Math.min(1, p.s.model_done / p.s.model_total) : 0));
    const fraction = fractions.reduce((a, b) => a + b, 0) / fractions.length;
    return { state: 'working', text: `Getting models… ${Math.floor(fraction * 100)}%`, fraction, detail: 'Downloading the picture-analysis and face models, once' };
  }

  const left = passes.filter((p) => p.s.models === 'ready' && p.s.total > 0 && p.s.done < p.s.total);
  if (left.length === 0) {
    // Nothing stored is waiting for analysis, but files are still being read and more will arrive: no number to give yet.
    const reading = o.loading && o.loading.pending > 0 ? o.loading : null;
    if (!reading) return null;
    return reading.paused
      ? { state: 'paused', text: 'Analysis paused', fraction: null, detail: 'Paused. Click to resume.' }
      : { state: 'working', text: 'Analysing photos', fraction: null, detail: 'Reading the photos Pixie found; they are analysed as they arrive' };
  }
  // The overall percentage is the whole story here; the counts for each step are in Settings, Picture analysis.
  const detail = `Still working on: ${left.map((p) => p.name).join(', ')}`;
  if (left.every((p) => p.s.paused)) {
    return { state: 'paused', text: 'Analysis paused', fraction: null, detail: `${detail}. Click to resume.` };
  }
  // The finished passes count as done work, so the percentage never jumps back when one of them finishes.
  const counted = passes.filter((p) => p.s.models === 'ready' && p.s.total > 0);
  const done = counted.reduce((a, p) => a + p.s.done, 0);
  const total = counted.reduce((a, p) => a + p.s.total, 0);
  const fraction = o.load ? o.load.fraction : total > 0 ? done / total : null;
  return { state: 'working', text: `Analysing photos ${Math.floor((fraction ?? 0) * 100)}%`, fraction, detail };
}
