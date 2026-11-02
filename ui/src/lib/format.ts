const MONTHS = ['January', 'February', 'March', 'April', 'May', 'June', 'July', 'August', 'September', 'October', 'November', 'December'];
const WEEKDAYS = ['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat'];

const short = (m: number) => MONTHS[m].slice(0, 3);

/** "YYYY-MM-DD" -> [year, month 1-12, day], or null when malformed. */
function parts(day: string): [number, number, number] | null {
  const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(day);
  return m ? [Number(m[1]), Number(m[2]), Number(m[3])] : null;
}

export const monthName = (month: number): string => MONTHS[month - 1] ?? '';

export function weekday(day: string): string {
  const p = parts(day);
  return p ? WEEKDAYS[new Date(Date.UTC(p[0], p[1] - 1, p[2])).getUTCDay()] : '';
}

/** "Sat 3 Jun 2024"; "Undated" for no date. */
export function formatDay(day: string | null): string {
  if (day === null) return 'Undated';
  const p = parts(day);
  return p ? `${weekday(day)} ${p[2]} ${short(p[1] - 1)} ${p[0]}` : day;
}

/** Stored seconds (EXIF local time kept as if UTC) -> "1 Jun 2024, 12:00". */
export function formatDateTime(seconds: number | null): string {
  if (seconds === null) return 'Unknown';
  const d = new Date(seconds * 1000);
  const hh = String(d.getUTCHours()).padStart(2, '0');
  const mm = String(d.getUTCMinutes()).padStart(2, '0');
  return `${d.getUTCDate()} ${short(d.getUTCMonth())} ${d.getUTCFullYear()}, ${hh}:${mm}`;
}

export function formatBytes(n: number): string {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  if (n < 1024 * 1024 * 1024) return `${(n / (1024 * 1024)).toFixed(1)} MB`;
  return `${(n / (1024 * 1024 * 1024)).toFixed(1)} GB`;
}

/** "11.3 MB/s". */
export const formatByteRate = (perSec: number): string => `${formatBytes(perSec)}/s`;

/** A speed in `unit` (a fixed word such as "photos"): "12 photos/s", "2.3 photos/s", and per minute when it is under one a second. */
export function formatRate(perSec: number, unit: string): string {
  if (perSec >= 10) return `${Math.round(perSec)} ${unit}/s`;
  if (perSec >= 1) return `${perSec.toFixed(1)} ${unit}/s`;
  return `${Math.round(perSec * 60)} ${unit}/min`;
}

/** How long is left, as a guess and worded as one: "under a minute", "about 5 min", "about 1 h 20 min". */
export function formatTimeLeft(secs: number): string {
  if (secs < 60) return 'under a minute';
  const minutes = Math.round(secs / 60);
  if (minutes < 60) return `about ${minutes} min`;
  const h = Math.floor(minutes / 60), m = minutes % 60;
  return m === 0 || h >= 10 ? `about ${h} h` : `about ${h} h ${m} min`;
}

const isDrive = (s: string) => /^[A-Za-z]:$/.test(s);
const cut = (p: string) => Math.max(p.lastIndexOf('/'), p.lastIndexOf('\\'));
export const basename = (p: string): string => p.slice(cut(p) + 1);
/** The name to show for a folder: its last part, or the whole path for a drive root (`P:\`) or `/`, which have none. */
export function folderName(p: string): string {
  const name = basename(p.replace(/[\\/]+$/, ''));
  // `P:` alone means "the current folder on drive P", so a drive root is always shown with its separator.
  if (isDrive(name)) return /[\\/]$/.test(p) ? p : `${p}\\`;
  return name === '' ? p : name;
}
/** The folder a path is in; for a file in the root of a drive that is `P:\`, not `P:`, and `/` for one in the root of `/`. */
export function dirname(p: string): string {
  const i = cut(p);
  if (i < 0) return '';
  const dir = p.slice(0, i);
  return dir === '' || isDrive(dir) ? p.slice(0, i + 1) : dir;
}

export function formatGps(lat: number | null, lon: number | null): string | null {
  if (lat === null || lon === null) return null;
  return `${Math.abs(lat).toFixed(4)}° ${lat < 0 ? 'S' : 'N'}, ${Math.abs(lon).toFixed(4)}° ${lon < 0 ? 'W' : 'E'}`;
}

/** Milliseconds -> "0:42" or "1:03:07"; empty when unknown. */
export function formatDuration(ms: number | null): string {
  if (ms === null || !Number.isFinite(ms) || ms < 0) return '';
  const total = Math.floor(ms / 1000);
  const h = Math.floor(total / 3600), m = Math.floor((total % 3600) / 60), s = total % 60;
  const ss = String(s).padStart(2, '0');
  return h > 0 ? `${h}:${String(m).padStart(2, '0')}:${ss}` : `${m}:${ss}`;
}

/** "1 photo", "2 photos": a thing is pluralised only when there is more than one (`many` for the odd ones, such as "people"). */
export const plural = (count: number, one: string, many = `${one}s`): string => `${count.toLocaleString()} ${count === 1 ? one : many}`;

/** `text` cut in the middle with "…" to at most `max` characters (a long folder path keeps its start and its end). */
export function middle(text: string, max: number): string {
  if (text.length <= max) return text;
  const keep = max - 1;
  const head = Math.ceil(keep / 2);
  return `${text.slice(0, head)}…${text.slice(text.length - (keep - head))}`;
}
