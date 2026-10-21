import type { AnalysisStatus } from './analysis';

/** The first-run / add-a-folder flow: which full-screen step is showing ('off': Browse). 'background' is the note shown
 *  after "Carry on in the background": what is still running, how to tell when it ends, what to do meanwhile. */
export type Phase = 'off' | 'welcome' | 'scanning' | 'analysing' | 'allset' | 'background';

const n = (x: number) => x.toLocaleString();
const plural = (count: number, one: string) => `${n(count)} ${one}${count === 1 ? '' : 's'}`;

/** Where the flow goes when the scan ends. Stopping keeps what was found and shows Browse; finding nothing, or only
 *  videos (which are not analysed), has nothing to analyse and goes straight to the end screen. */
export function phaseAfterScan(o: { stopped: boolean; found: number; photos: number }): Phase {
  if (o.stopped) return 'off';
  return o.found === 0 || o.photos === 0 ? 'allset' : 'analysing';
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
function passLine(key: 'search' | 'faces', title: string, why: string, label: 'picture-analysis' | 'face', s: AnalysisStatus | null, extra: string, expect: number): Line {
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
  if (s.error) return { ...line, text: `${n(s.done)} of ${n(s.total)}`, error: `${key === 'search' ? 'Picture analysis' : 'Face finding'} is having trouble: ${s.error}` };
  // The status can still describe the library as it was before this scan: until it has counted at least the photos the
  // scan found, an empty or small status says nothing about whether the work is done.
  if (s.total < expect) return { ...line, text: 'Getting ready…' };
  if (s.total === 0) return { ...line, text: 'Nothing to do', fraction: 1, done: true };
  if (s.paused && s.done < s.total) return { ...line, text: `Paused at ${n(s.done)} of ${n(s.total)}`, paused: true, fraction: Math.min(1, s.done / s.total) };
  return { ...line, text: `${n(s.done)} of ${n(s.total)}${extra}`, fraction: Math.min(1, s.done / s.total), done: s.done >= s.total };
}

/** The three lines of the analysing step. `previews`: thumbnails left now, and the most that were ever queued.
 *  `expect`: how many photos the scan found, which each pass must have counted before it can be called done. */
export function analysingLines(a: AnalysisStatus | null, p: AnalysisStatus | null, previews: { left: number; max: number }, peopleCount: number, expect = 0): Line[] {
  const previewsDone = previews.left === 0;
  const done = previews.max - previews.left;
  return [
    {
      key: 'previews', title: 'Preparing previews', why: 'So your photos appear at once when you start browsing.',
      text: previewsDone ? 'Done' : `${n(Math.max(0, done))} of ${n(previews.max)}`,
      fraction: previews.max > 0 ? Math.min(1, Math.max(0, done) / previews.max) : previewsDone ? 1 : null,
      error: null, paused: false, done: previewsDone,
    },
    passLine('search', 'Understanding your photos', 'So you can browse by Auto tags, the labels Pixie adds to say what is in each photo (Beach, Museum, Dog and so on).', 'picture-analysis', a, '', expect),
    passLine('faces', 'Finding faces', 'So you can name the people and browse by person.', 'face', p, ` · ${peopleCount === 1 ? '1 person' : `${n(peopleCount)} people`} so far`, expect),
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

/** The Start button on the welcome screen: just "Start", off while nothing is ticked. */
export function startLabel(ticked: number): { label: string; disabled: boolean } {
  return { label: 'Start', disabled: ticked === 0 };
}
