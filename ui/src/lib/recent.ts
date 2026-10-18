/** The start of "Last 30 days": a unix time, `days` days before `nowSeconds`. */
export const recentSince = (nowSeconds: number, days = 30): number => nowSeconds - days * 86_400;

export const DEFAULT_RECENT_DAYS = 30;
export const MAX_RECENT_DAYS = 3650;

/** A stored or typed value is used only if it is a whole number of days from 1 to 3650; otherwise 30. */
export const parseRecentDays = (v: unknown): number =>
  typeof v === 'number' && Number.isInteger(v) && v >= 1 && v <= MAX_RECENT_DAYS ? v : DEFAULT_RECENT_DAYS;

/** The navigation row's text. */
export const recentLabel = (days: number): string => (days === 1 ? 'Last day' : `Last ${days} days`);
