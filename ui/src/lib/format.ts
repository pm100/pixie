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

const cut = (p: string) => Math.max(p.lastIndexOf('/'), p.lastIndexOf('\\'));
export const basename = (p: string): string => p.slice(cut(p) + 1);
/** The name to show for a folder: its last part, or the whole path for a drive root (`P:\`) or `/`, which have none. */
export function folderName(p: string): string {
  const name = basename(p.replace(/[\\/]+$/, ''));
  return name === '' || /^[A-Za-z]:$/.test(name) ? p : name;
}
export const dirname = (p: string): string => (cut(p) < 0 ? '' : p.slice(0, cut(p)));

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
