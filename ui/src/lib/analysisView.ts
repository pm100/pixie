import { plural } from './format';
import type { AnalysisStatus } from './analysis';
import type { DupStatus } from './dupes';

/** The background analyses, in the order they are listed. */
export type CounterKey = 'tags' | 'faces' | 'dupes';

/** One row under the overall bar: the analysis and what it has found so far. */
export type Counter = { key: CounterKey; label: string; text: string; done: boolean };

/** Something the user can act on: a model that failed (Retry), or the analysis being paused (Resume). */
export type Problem = { key: CounterKey; kind: 'error' | 'paused'; text: string };

export type AnalysisView = {
  /** The line above the bar: "Analysing 52%", "Getting models… 43%", "Done". */
  headline: string;
  /** 0..1 for the one bar, or null when there is nothing to show yet. */
  fraction: number | null;
  counters: Counter[];
  problems: Problem[];
  /** Every listed analysis is done. */
  finished: boolean;
  /** Some analysis still has photos left (and nothing is wrong): there is something to pause. */
  working: boolean;
  paused: boolean;
  failed: boolean;
};

/** How far the analysis has got for the photos of some folders, counted from those photos (mirrors the Rust `LoadProgress`). */
export type LoadProgress = { photos: number; shown: number; hashed: number; tagged: number; scanned: number; faces: number; duplicates: number;
  /** Photos and videos a walk found that are not stored yet: the load's totals include them. */
  pending: number; pending_videos: number };

export type AnalysisInput = {
  /** The analyses' states. They say whether a model is on its way, failed or paused; for the whole library
   *  (Settings) they also give the counts. */
  tags: AnalysisStatus | null;
  faces: AnalysisStatus | null;
  dupes: DupStatus | null;
  /** People found in the whole library so far (used for the whole library only). */
  people: number;
  /** Counts for the photos of one load (the folders just added): the counts come from here, not from the statuses, so
   *  they cover only that load and cannot be thrown off by what else is in the library, or by duplicates found
   *  hiding older photos. `null`: a load, but its counts are not known yet. Leave out for the whole library. */
  load?: LoadProgress | null;
  /** Which analyses are switched on (all, by default). One that is off is not listed and nothing waits for it. */
  on?: Partial<Record<CounterKey, boolean>>;
};

const n = (x: number) => x.toLocaleString();
const LABELS: Record<CounterKey, string> = { tags: 'Auto tags', faces: 'People', dupes: 'Duplicates' };
const MODEL_NAMES = { tags: 'picture-analysis', faces: 'face' } as const;
const TROUBLE = { tags: 'Picture analysis', faces: 'Face finding', dupes: 'Duplicate finding' } as const;

/** How far an analysis has got, in the terms the one bar needs. */
type Progress = { done: number; total: number } | { download: number } | null;
type Counts = { done: number; total: number };

type Built = { counter: Counter; problems: Problem[]; progress: Progress; working: boolean };

const row = (key: CounterKey, counter: Partial<Counter>, problems: Problem[] = [], progress: Progress = null, working = false): Built => ({
  counter: { key, label: LABELS[key], text: 'Starting…', done: false, ...counter }, problems, progress, working,
});

/** The row for an analysis that needs a downloaded model (Auto tags, faces). `counts`: how far it has got (null: not
 *  known yet); `found`: what to say it has found. */
function modelRow(key: 'tags' | 'faces', s: AnalysisStatus | null, counts: Counts | null, found: string): Built {
  const model = MODEL_NAMES[key];
  if (!s) return row(key, {});
  if (s.models === 'error') {
    return row(key, { text: `Waiting for the ${model} model` }, [{ key, kind: 'error', text: `Could not get the ${model} model: ${s.error ?? 'unknown error'}` }]);
  }
  if (s.models === 'missing') return row(key, { text: `Waiting for the ${model} model` });
  if (s.models === 'downloading') {
    const fraction = s.model_total > 0 ? Math.min(1, s.model_done / s.model_total) : 0;
    return row(key, { text: `Getting the ${model} model… ${Math.floor(fraction * 100)}%` }, [], { download: fraction });
  }
  if (s.error) return row(key, { text: found }, [{ key, kind: 'error', text: `${TROUBLE[key]} is having trouble: ${s.error}` }]);
  if (!counts) return row(key, { text: 'Getting ready…' });
  if (counts.total === 0) return row(key, { text: 'Nothing to do', done: true });
  const left = counts.done < counts.total;
  const paused = s.paused && left;
  return row(key, { text: paused ? 'Paused' : found, done: !left }, paused ? [{ key, kind: 'paused', text: 'Paused.' }] : [], counts, left && !paused);
}

function dupesRow(paused: boolean, counts: Counts | null, found: string): Built {
  if (!counts) return row('dupes', { text: 'Getting ready…' });
  if (counts.total === 0) return row('dupes', { text: 'Nothing to do', done: true });
  const left = counts.done < counts.total;
  const stopped = paused && left;
  return row('dupes', { text: stopped ? 'Paused' : found, done: !left }, stopped ? [{ key: 'dupes', kind: 'paused', text: 'Paused.' }] : [], counts, left && !stopped);
}

/** What the analysing screen and Settings, Picture analysis, show: one bar, and what each analysis has found. */
export function analysisView(o: AnalysisInput): AnalysisView {
  const on = (k: CounterKey) => o.on?.[k] !== false;
  const load = o.load; // undefined: the whole library; null: a load whose counts are not known yet
  const built: Built[] = [];
  if (on('tags')) {
    const counts: Counts | null = load === undefined ? (o.tags ? { done: o.tags.done, total: o.tags.total } : null) : load && { done: load.tagged, total: load.shown + load.pending };
    built.push(modelRow('tags', o.tags, counts, plural(counts?.done ?? 0, 'photo')));
  }
  if (on('faces')) {
    const counts: Counts | null = load === undefined ? (o.faces ? { done: o.faces.done, total: o.faces.total } : null) : load && { done: load.scanned, total: load.shown + load.pending };
    built.push(modelRow('faces', o.faces, counts, `${n(load === undefined ? o.people : load?.faces ?? 0)} found`));
  }
  if (on('dupes') && (o.dupes || load)) {
    const counts: Counts | null = load === undefined ? (o.dupes ? { done: o.dupes.done, total: o.dupes.total } : null) : load && { done: load.hashed, total: load.photos + load.pending };
    built.push(dupesRow(o.dupes?.paused ?? false, counts, `${n(load === undefined ? o.dupes?.extra_files ?? 0 : load?.duplicates ?? 0)} found`)); // listed once its count is known
  }

  const counters = built.map((b) => b.counter);
  // Pausing is one thing for all the analyses, so it is said once.
  const problems = built.flatMap((b) => b.problems).filter((p, i, all) => p.kind !== 'paused' || all.findIndex((q) => q.kind === 'paused') === i);
  const downloads = built.flatMap((b) => (b.progress && 'download' in b.progress ? [b.progress.download] : []));
  const counted = built.flatMap((b) => (b.progress && 'total' in b.progress ? [b.progress] : []));
  // With every analysis switched off there is nothing to wait for (see `allOff` below: that is not "done").
  const finished = (!on('tags') && !on('faces') && !on('dupes')) || (counters.length > 0 && counters.every((c) => c.done));

  let fraction: number | null = null;
  let headline = 'Starting…';
  if (downloads.length > 0) {
    fraction = downloads.reduce((a, b) => a + b, 0) / downloads.length;
    headline = `Getting models… ${Math.floor(fraction * 100)}%`;
  } else if (counted.length > 0) {
    // The finished analyses count as done work, so the bar never jumps back when one of them finishes.
    const total = counted.reduce((a, p) => a + p.total, 0);
    fraction = total > 0 ? counted.reduce((a, p) => a + p.done, 0) / total : null;
    headline = fraction === null ? 'Starting…' : `Analysing ${Math.floor(fraction * 100)}%`;
  }
  const allOff = !on('tags') && !on('faces') && !on('dupes');
  if (allOff) {
    // Nothing is being done, which is not the same as everything being done: no bar, and no "Done".
    fraction = null;
    headline = 'Analysis is turned off';
  } else if (finished) {
    fraction = 1;
    headline = 'Done';
  }
  return {
    headline, fraction, counters, problems, finished,
    working: built.some((b) => b.working),
    paused: problems.some((p) => p.kind === 'paused'),
    failed: problems.some((p) => p.kind === 'error'),
  };
}
