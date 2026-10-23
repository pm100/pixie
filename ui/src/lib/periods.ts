import { monthName } from './format';

/** The `since` that selects the items with no date (mirrors the Rust `UNDATED`): the lowest whole number a browser holds exactly. */
export const UNDATED = -9_007_199_254_740_991;

/** A navigation key (`y:2025`, `m:2025-06`, `d:2025-06-14`) as a time range in unix seconds, UTC: since inclusive, until exclusive.
 *  The key `undated` is the items without a date: a window starting at `UNDATED`. */
export function periodRange(key: string): { since: number; until: number } | null {
  if (key === 'undated') return { since: UNDATED, until: UNDATED + 1 };
  const m = /^(y|m|d):(\d{4})(?:-(\d{2}))?(?:-(\d{2}))?$/.exec(key);
  if (!m) return null;
  const [, kind, ys, ms, ds] = m;
  const y = Number(ys);
  const t = (yy: number, mm: number, dd: number) => Date.UTC(yy, mm - 1, dd) / 1000;
  if (kind === 'y' && ms === undefined) return { since: t(y, 1, 1), until: t(y + 1, 1, 1) };
  const mo = Number(ms);
  if (mo < 1 || mo > 12) return null;
  if (kind === 'm' && ds === undefined) return { since: t(y, mo, 1), until: mo === 12 ? t(y + 1, 1, 1) : t(y, mo + 1, 1) };
  if (kind === 'd' && ds !== undefined) {
    const d = Number(ds);
    const since = t(y, mo, d);
    if (new Date(since * 1000).getUTCDate() !== d) return null; // not a real calendar day (e.g. 02-30)
    return { since, until: since + 86_400 };
  }
  return null;
}

/** A period key as people read it: `2026`, `September 2026`, `19 September 2026`; empty if it is not a period key. */
export function periodLabel(key: string): string {
  if (key === 'undated') return 'items without a date';
  if (periodRange(key) === null) return '';
  const [, y, m, d] = /^[ymd]:(\d{4})(?:-(\d{2}))?(?:-(\d{2}))?$/.exec(key)!;
  return [d ? String(Number(d)) : '', m ? monthName(Number(m)) : '', y].filter(Boolean).join(' ');
}
