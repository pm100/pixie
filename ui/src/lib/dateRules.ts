/** Which dates in a photo count as missing, and when the file's own date stands in (mirrors the Rust `DateRules`). */
export type DateRules = {
  /** Years whose 1 January is a date a camera shows when its clock was never set. */
  odd_years: number[];
  /** Dates before 1 January of this year count as missing too; null for none. */
  before_year: number | null;
  /** Use the file's date when the photo has no date inside it. */
  file_when_none: boolean;
  /** Use the file's date when the date inside is one that counts as missing. */
  file_when_odd: boolean;
};

/** The dates a camera can start from when its clock was never set, as Settings lists them. */
export const ODD_YEARS = [1904, 1970, 1980, 2000] as const;

export const DEFAULT_DATE_RULES: DateRules = { odd_years: [...ODD_YEARS], before_year: 1980, file_when_none: true, file_when_odd: true };

/** The rules with this year's 1 January counted as missing (or not). */
export function withOddYear(rules: DateRules, year: number, on: boolean): DateRules {
  const chosen = new Set(rules.odd_years);
  if (on) chosen.add(year); else chosen.delete(year);
  return { ...rules, odd_years: ODD_YEARS.filter((y) => chosen.has(y)) };
}

const EARLIEST_YEAR = 1826; // the first photograph

/** A year typed in Settings, kept to years a photo can have; blank or anything else is none. */
export function parseBeforeYear(text: string): number | null {
  const n = Number.parseInt(text.trim(), 10);
  if (!Number.isFinite(n)) return null;
  return Math.min(Math.max(n, EARLIEST_YEAR), new Date().getFullYear());
}

export const withBeforeYear = (rules: DateRules, text: string): DateRules => ({ ...rules, before_year: parseBeforeYear(text) });
