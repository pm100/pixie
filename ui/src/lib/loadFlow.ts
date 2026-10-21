import type { AnalysisStatus } from './analysis';

/** The first-run / add-a-folder flow: which full-screen step is showing ('off': Browse). */
export type Phase = 'off' | 'welcome' | 'scanning' | 'analysing' | 'allset';

const n = (x: number) => x.toLocaleString();
const plural = (count: number, one: string) => `${n(count)} ${one}${count === 1 ? '' : 's'}`;

/** Where the flow goes when the scan ends. Stopping keeps what was found and shows Browse; finding nothing says so. */
export function phaseAfterScan(o: { stopped: boolean; found: number }): Phase {
  if (o.stopped) return 'off';
  return o.found === 0 ? 'allset' : 'analysing';
}

/** One row of the "now analysing" step. */
export type Line = {
  key: 'previews' | 'search' | 'faces';
  title: string;
  /** One plain sentence on what it is for. */
  why: string;
  text: string;
  /** 0..1, or null when there is no bar yet. */
  fraction: number | null;
  error: string | null;
  paused: boolean;
  done: boolean;
};

/** A line for one analysis pass (search or faces); `label` is "search" or "face" in the model messages. */
function passLine(key: 'search' | 'faces', title: string, why: string, label: 'search' | 'face', s: AnalysisStatus | null, extra: string): Line {
  const line: Line = { key, title, why, text: 'Starting…', fraction: null, error: null, paused: false, done: false };
  if (!s) return line;
  if (s.models === 'error') {
    return { ...line, text: `Waiting for the ${label} model`, error: `Could not get the ${label} model: ${s.error ?? 'unknown error'}` };
  }
  if (s.models === 'missing') return { ...line, text: `Waiting for the ${label} model` };
  if (s.models === 'downloading') {
    const fraction = s.model_total > 0 ? Math.min(1, s.model_done / s.model_total) : 0;
    return { ...line, text: `Getting the ${label} model… ${Math.floor(fraction * 100)}%`, fraction };
  }
  if (s.error) return { ...line, text: `${n(s.done)} of ${n(s.total)}`, error: `${key === 'search' ? 'Analysis' : 'People search'} is having trouble: ${s.error}` };
  if (s.total === 0) return { ...line, text: 'Nothing to do', fraction: 1, done: true };
  if (s.paused && s.done < s.total) return { ...line, text: `Paused at ${n(s.done)} of ${n(s.total)}`, paused: true, fraction: Math.min(1, s.done / s.total) };
  return { ...line, text: `${n(s.done)} of ${n(s.total)}${extra}`, fraction: Math.min(1, s.done / s.total), done: s.done >= s.total };
}

/** The three lines of the analysing step. `previews`: thumbnails left now, and the most that were ever queued. */
export function analysingLines(a: AnalysisStatus | null, p: AnalysisStatus | null, previews: { left: number; max: number }, peopleCount: number): Line[] {
  const previewsDone = previews.left === 0;
  const done = previews.max - previews.left;
  return [
    {
      key: 'previews', title: 'Preparing previews', why: 'So your photos appear at once when you start browsing.',
      text: previewsDone ? 'Done' : `${n(Math.max(0, done))} of ${n(previews.max)}`,
      fraction: previews.max > 0 ? Math.min(1, Math.max(0, done) / previews.max) : previewsDone ? 1 : null,
      error: null, paused: false, done: previewsDone,
    },
    passLine('search', 'Understanding your photos', 'So you can search for "dog on a beach" and browse by what is in them (Auto tags).', 'search', a, ''),
    passLine('faces', 'Finding faces', 'So you can name the people and browse by person.', 'face', p, ` · ${peopleCount === 1 ? '1 person' : `${n(peopleCount)} people`} so far`),
  ];
}

export const analysingFinished = (lines: Line[]): boolean => lines.every((l) => l.done);

/** "3 minutes", "1 second": rounded to whole minutes from a minute up. */
function duration(seconds: number): string {
  return seconds < 60 ? plural(Math.round(seconds), 'second') : plural(Math.round(seconds / 60), 'minute');
}

/** What the end screen says. `problems`: folders that could not be read, so success is never claimed silently. */
export function allSetSummary(o: {
  folders: { name: string; count: number }[]; photos: number; videos: number; seconds: number; facesFound: boolean;
  problems?: string[];
}): { title: string; detail: string; rows: { name: string; text: string }[]; showPeopleBox: boolean; problems: string[] } {
  const items = o.photos + o.videos;
  const problems = o.problems ?? [];
  if (items === 0) {
    return { title: 'Nothing was found', detail: 'Pixie did not find any photos or videos in the folders you chose.', rows: [], showPeopleBox: false, problems };
  }
  return {
    problems,
    title: `All set — ${n(items)} items found`,
    detail: `${plural(o.photos, 'photo')} and ${plural(o.videos, 'video')}, in ${plural(o.folders.length, 'folder')}. That took ${duration(o.seconds)}.`,
    rows: o.folders.map((f) => ({ name: f.name, text: n(f.count) })),
    showPeopleBox: o.facesFound,
  };
}

/** The Start button on the welcome screen. */
export function startLabel(ticked: number): { label: string; disabled: boolean } {
  return ticked === 0 ? { label: 'Start', disabled: true } : { label: `Start — look in ${plural(ticked, 'folder')}`, disabled: false };
}

/** Lets one item through, then drops the rest until `minGapMs` has passed: the strip never floods the screen. */
export function throttleSamples<T>(minGapMs = 200): (item: T, now: number) => T | null {
  let last = -Infinity;
  return (item, now) => {
    if (now - last < minGapMs) return null;
    last = now;
    return item;
  };
}
