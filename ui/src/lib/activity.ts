import { formatByteRate, formatBytes, formatRate, formatTimeLeft, middle, plural } from './format';

/** What Pixie reports about its background work (the Rust `activity` registry, as the `activity` event and `activity_snapshot`). */
export type TaskKind = 'scan' | 'read' | 'thumbnails' | 'auto_tags' | 'faces' | 'analysing' | 'face_groups' | 'duplicates' | 'copy_to_master' | 'models' | 'face_models' | 'cloud_wait' | 'email';
export type RunState = 'running' | 'waiting' | 'paused' | 'error' | 'done';

export type ActivityTask = {
  kind: TaskKind;
  state: RunState;
  done: number;
  total: number | null;
  bytes_done: number;
  bytes_total: number | null;
  per_sec: number | null;
  bytes_per_sec: number | null;
  eta_secs: number | null;
  started_ms: number;
  finished_ms: number | null;
  waiting_secs: number | null;
  note: string | null;
  /** The file a job has been on for a while (a big video): its name, size, and the seconds so far. */
  current: { name: string; bytes: number; secs: number } | null;
  /** Files the job is waiting on, the biggest first (the cloud files being fetched). */
  items: { name: string; bytes: number; secs: number; source: string }[];
  /** The watched folder the work is in now, as listed in Settings, Photo folders. */
  place: string | null;
};

/** Which cloud a cloud wait is for (the picture on its row). */
export type CloudSource = 'onedrive' | 'icloud' | 'google_drive' | 'other';

/** `tasks`: one per kind, what it is doing now (or how its latest run ended); `finished`: earlier runs that ended, newest first. */
export type ActivitySnapshot = { tasks: ActivityTask[]; finished: ActivityTask[]; eta_secs: number | null; now_ms: number };

/** One line of the screen. `fraction` is null when the total is not known (an indeterminate bar). */
export type Row = {
  key: string; kind: TaskKind; label: string; state: RunState; fraction: number | null; counts: string; speed: string; left: string; explain: string;
  /** `explain` in full: a long folder path in it is shortened on the screen. */
  explainFull: string;
  /** The biggest files being waited for (at most `FILES_SHOWN`), and how many more there are. */
  files: { name: string; detail: string }[]; moreFiles: number;
  /** For a cloud wait: the cloud of the biggest file being waited for. Null on every other row. */
  source: CloudSource | null;
  /** The watched folder the work is in now, cut in the middle if long (null: not a row that says), and in full for the tooltip. */
  place: string | null; placeFull: string | null;
};

/** A long folder path on the scan row is cut in the middle: its two ends say the most. */
const shorten = (s: string) => middle(s, 72);

/** The rows that say which watched folder they are in (the line is kept, empty, when it is not known, so nothing below it moves). */
const PLACE_KINDS: TaskKind[] = ['read', 'analysing', 'auto_tags', 'faces'];

const SOURCES: CloudSource[] = ['onedrive', 'icloud', 'google_drive'];

const FILES_SHOWN = 3;

export type Pill = 'working' | 'paused' | 'error' | null;

const LABELS: Record<TaskKind, string> = {
  scan: 'Finding your files',
  read: 'Reading and fingerprinting files',
  thumbnails: 'Making thumbnails',
  auto_tags: 'Auto tags',
  faces: 'Finding faces',
  analysing: 'Analysing photos',
  face_groups: 'Grouping people',
  duplicates: 'Looking for duplicates',
  copy_to_master: 'Syncing to your sync folder',
  models: 'Getting models',
  face_models: 'Getting face models',
  cloud_wait: 'Waiting for your cloud drive',
  email: 'Preparing email',
};

/** What each kind of work counts: [one, many]. */
const THINGS: Record<TaskKind, [string, string]> = {
  scan: ['file', 'files'],
  read: ['file', 'files'],
  thumbnails: ['thumbnail', 'thumbnails'],
  auto_tags: ['photo', 'photos'],
  faces: ['photo', 'photos'],
  analysing: ['photo', 'photos'],
  face_groups: ['face', 'faces'],
  duplicates: ['file', 'files'],
  copy_to_master: ['item', 'items'],
  models: ['file', 'files'],
  face_models: ['file', 'files'],
  cloud_wait: ['file', 'files'],
  email: ['photo', 'photos'],
};

export const label = (kind: TaskKind): string => LABELS[kind];

const capital = (s: string) => s.charAt(0).toUpperCase() + s.slice(1);

/** "42 s", "1 min 30 s". */
const waited = (secs: number): string => (secs < 60 ? `${secs} s` : `${Math.floor(secs / 60)} min ${secs % 60} s`);

const fractionOf = (t: ActivityTask): number | null => {
  const part = t.bytes_total !== null && t.bytes_total > 0 ? t.bytes_done / t.bytes_total : t.total !== null && t.total > 0 ? t.done / t.total : null;
  return part === null ? null : Math.min(1, Math.max(0, part));
};

function countsOf(t: ActivityTask): string {
  const [one, many] = THINGS[t.kind];
  if ((t.kind === 'models' || t.kind === 'face_models') && t.bytes_total !== null) return `${formatBytes(t.bytes_done)} of ${formatBytes(t.bytes_total)}`;
  let text = t.total !== null ? `${t.done.toLocaleString()} of ${plural(t.total, one, many)}` : t.kind === 'scan' ? `${plural(t.done, one, many)} found` : plural(t.done, one, many);
  if (t.bytes_total !== null) text += `, ${formatBytes(t.bytes_done)} of ${formatBytes(t.bytes_total)}`;
  else if (t.bytes_done > 0) text += `, ${formatBytes(t.bytes_done)}`;
  return text;
}

function speedOf(t: ActivityTask): string {
  if (t.bytes_per_sec !== null && t.bytes_per_sec > 0) return formatByteRate(t.bytes_per_sec);
  return t.per_sec !== null && t.per_sec > 0 ? formatRate(t.per_sec, THINGS[t.kind][1]) : ''; // (not "0 photos/min": that is no progress, not a speed)
}

function explainOf(t: ActivityTask): string {
  switch (t.state) {
    case 'paused':
      return ''; // the amber bar and the Resume button say so
    case 'error':
      return t.note ? `Problem: ${t.note}` : 'Something went wrong.';
    case 'waiting':
      return t.kind === 'cloud_wait'
        ? `Waited ${waited(t.waiting_secs ?? 0)} for your cloud drive to download a file before Pixie can read it.`
        : `${t.note ?? 'Waiting'} · ${waited(t.waiting_secs ?? 0)}`;
    case 'done':
      return t.finished_ms !== null ? `Took ${formatTimeLeft(Math.max(0, Math.round((t.finished_ms - t.started_ms) / 1000)))}.` : '';
    default:
      if (t.current) return `Working on ${t.current.name} (${formatBytes(t.current.bytes)}) for ${waited(t.current.secs)}`;
      return t.kind === 'cloud_wait' ? 'Your cloud drive (OneDrive, iCloud Drive) is downloading these files before Pixie can read them.' : t.note ?? '';
  }
}

function rowOf(t: ActivityTask): Row {
  // A cloud wait has no meaningful count (how many files there are keeps changing) or bar: it lists the big files being fetched instead.
  if (t.kind === 'cloud_wait') {
    const files = t.items.slice(0, FILES_SHOWN).map((f) => ({ name: f.name, detail: `${formatBytes(f.bytes)} · waited ${waited(f.secs)}` }));
    return { key: `${t.kind}:${t.started_ms}`, kind: t.kind, label: label(t.kind), state: t.state, fraction: null, counts: '', speed: '', left: '', explain: explainOf(t), explainFull: explainOf(t), files, moreFiles: Math.max(0, t.items.length - FILES_SHOWN), source: SOURCES.find((s) => s === t.items[0]?.source) ?? 'other', place: null, placeFull: null };
  }
  // A job that has not moved for the whole window says so; one with too few samples yet is still being estimated.
  const left = t.state === 'running' ? (t.eta_secs !== null ? formatTimeLeft(t.eta_secs) : t.per_sec === 0 ? 'No progress' : 'Estimating…') : '';
  return { key: `${t.kind}:${t.started_ms}`, kind: t.kind, label: label(t.kind), state: t.state, fraction: t.state === 'done' ? 1 : fractionOf(t), counts: countsOf(t), speed: speedOf(t), left, explain: t.kind === 'scan' ? shorten(explainOf(t)) : explainOf(t), explainFull: explainOf(t), files: [], moreFiles: 0, source: null, place: PLACE_KINDS.includes(t.kind) ? middle(t.place ?? '', 64) : null, placeFull: PLACE_KINDS.includes(t.kind) ? t.place ?? '' : null };
}

/** Errors first, then the rest in a fixed order by kind (the order of LABELS): a row never changes place because it paused, or because
 *  another started first, so the list does not shuffle. */
const ORDER = Object.keys(LABELS);
const rank = (t: ActivityTask): number => (t.state === 'error' ? 0 : 1);
const place = (t: ActivityTask): number => ORDER.indexOf(t.kind);

/**
 * The time left to show: the latest estimate, or the last one while work goes on but there is none at the moment (the estimate is lost
 * when the speed cannot be measured for a spell). Nothing once nothing is running, so the next piece of work starts at "Estimating".
 */
export const holdEstimate = (held: number | null, eta: number | null, running: boolean): number | null => (!running ? null : eta ?? held);

/** `held`: the last estimate worked out (see `holdEstimate`), shown when the snapshot has none. */
export function view(s: ActivitySnapshot | null, held: number | null = null): { running: Row[]; finished: Row[]; overall: string; pill: Pill; held: boolean } {
  const tasks = s?.tasks ?? [];
  // A model download is not shown (it only confuses): a failure is, with its reason and the Retry button.
  const open = tasks.filter((t) => t.state !== 'done' && !((t.kind === 'models' || t.kind === 'face_models') && t.state !== 'error'));
  // Every run that ended, newest first: the latest run of each kind, and the earlier ones (a kind that runs again is also running).
  const finished = [...tasks.filter((t) => t.state === 'done'), ...(s?.finished ?? [])].sort((a, b) => (b.finished_ms ?? 0) - (a.finished_ms ?? 0));
  const running = open.map((t, i) => ({ t, i })).sort((a, b) => rank(a.t) - rank(b.t) || place(a.t) - place(b.t) || a.i - b.i).map(({ t }) => rowOf(t));
  let pill: Pill = null;
  // Pause cannot stop the cloud drive's own download, so a cloud wait going on does not stop the rest showing as paused.
  const pausable = open.filter((t) => t.kind !== 'cloud_wait');
  if (open.length > 0) pill = open.some((t) => t.state === 'error') ? 'error' : pausable.length > 0 && pausable.every((t) => t.state === 'paused') ? 'paused' : 'working';
  let overall: string;
  if (open.length === 0) overall = finished.length > 0 ? 'Everything is done.' : 'Nothing is running.';
  else if (pill === 'error') overall = 'Something needs attention.';
  else if (pill === 'paused') overall = 'Paused.';
  else {
    // The speed shown is the slowest job's: the one the time left is waiting for.
    const lead = open.filter((t) => t.state === 'running').sort((a, b) => (b.eta_secs ?? -1) - (a.eta_secs ?? -1))[0];
    const speed = lead ? speedOf(lead) : '';
    const eta = s?.eta_secs ?? held;
    overall = eta !== null ? `${capital(formatTimeLeft(eta))} left${speed ? ` · ${speed}` : ''}` : 'Estimating time…';
  }
  // Some work is paused, even if other work goes on: the button then offers Resume, which carries on everything.
  return { running, finished: finished.map(rowOf), overall, pill, held: pausable.some((t) => t.state === 'paused') };
}
