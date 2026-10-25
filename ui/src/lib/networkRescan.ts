/** How often folders on the network are scanned again (folders on this computer are watched instead, so need no setting). */
export type NetworkRescan = { on: boolean; minutes: number };

/** The choices offered, in minutes. */
export const NETWORK_MINUTES = [5, 15, 30, 60, 120] as const;

export const DEFAULT_NETWORK_RESCAN: NetworkRescan = { on: true, minutes: 30 };

/** What was saved, made safe: anything unreadable gives the default, and an interval that is not offered the nearest offered. */
export function parseNetworkRescan(raw: unknown): NetworkRescan {
  if (typeof raw !== 'object' || raw === null) return DEFAULT_NETWORK_RESCAN;
  const r = raw as { on?: unknown; minutes?: unknown };
  const minutes = typeof r.minutes === 'number' && Number.isFinite(r.minutes) ? r.minutes : DEFAULT_NETWORK_RESCAN.minutes;
  const nearest = NETWORK_MINUTES.reduce((best, m) => (Math.abs(m - minutes) < Math.abs(best - minutes) ? m : best), NETWORK_MINUTES[0]);
  return { on: typeof r.on === 'boolean' ? r.on : DEFAULT_NETWORK_RESCAN.on, minutes: nearest };
}

/** The minutes to tell the backend: none when switched off. */
export const rescanMinutes = (r: NetworkRescan): number | null => (r.on ? r.minutes : null);
