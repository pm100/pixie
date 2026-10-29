import type { AnalysisOptions } from './analysisOptions';
import { NETWORK_MINUTES, type NetworkRescan } from './networkRescan';
import { MAX_NIGHTS, MAX_RADIUS_KM, MAX_STOP_KM, type TripPrefs } from './trips';
import { MAX_RECENT_DAYS } from './recent';
import type { ViewerMode } from './viewerPref';

/**
 * The settings Ask Pixie may change, described for the model (the shape the proxy reads: ask-proxy/src/types.ts) and
 * checked again here before anything is shown or applied. The model only proposes; the person presses Apply.
 * Where home is stays out: that is confirmed on a map, not in words. Folders and the library are actions, not settings.
 */
export interface SettingValues {
  viewerMode: ViewerMode;
  emailLimitMb: number;
  recentDays: number;
  networkRescan: NetworkRescan;
  analysis: AnalysisOptions;
  trips: TripPrefs;
}

export interface SettingInfo {
  key: string;
  label: string;
  description: string;
  type: 'choice' | 'int' | 'float' | 'bool';
  options?: { value: number; label: string }[];
  min?: number;
  max?: number;
  step?: number;
  current: number | boolean;
  currentLabel: string;
  default: number | boolean;
}

export interface Proposed { setting: string; value: unknown }

/** A change that passed the checks: what the card shows, and the value to apply. */
export interface Change { key: string; label: string; value: number | boolean; from: string; to: string }

/** What applying sends to the app, one call per group of settings that changed. */
export interface Handlers {
  viewer: (mode: ViewerMode) => void;
  emailLimit: (mb: number) => void;
  recentDays: (days: number) => void;
  networkRescan: (rescan: NetworkRescan) => void;
  analysis: (options: AnalysisOptions) => void;
  trips: (prefs: TripPrefs) => void;
}

const VIEWER: { value: number; mode: ViewerMode; label: string }[] = [
  { value: 0, mode: 'window', label: 'Its own window' },
  { value: 1, mode: 'popout', label: 'A pop-out' },
];

const onOff = (b: boolean) => (b ? 'On' : 'Off');
const minutesLabel = (m: number) => (m >= 60 && m % 60 === 0 ? `${m / 60} ${m === 60 ? 'hour' : 'hours'}` : `${m} minutes`);

/** Every setting with its current value, in a fixed order. */
export function settingsSchema(v: SettingValues): SettingInfo[] {
  const viewer = VIEWER.find((o) => o.mode === v.viewerMode)!;
  return [
    {
      key: 'viewer_mode', label: 'Open photos in', type: 'choice', description: 'Where a photo opens when you double-click it: in its own window, or in a pop-out that follows the photo chosen in the grid.',
      options: VIEWER.map(({ value, label }) => ({ value, label })), current: viewer.value, currentLabel: viewer.label, default: 0,
    },
    {
      key: 'email_limit_mb', label: 'Email size limit', type: 'int', description: 'The most an email may carry, in megabytes. Bigger sets are offered smaller or split into several emails.',
      min: 1, max: 500, step: 1, current: v.emailLimitMb, currentLabel: `${v.emailLimitMb} MB`, default: 20,
    },
    {
      key: 'recent_days', label: 'Days counted as recent', type: 'int', description: 'How many days "Recent" means in Browse.',
      min: 1, max: MAX_RECENT_DAYS, step: 1, current: v.recentDays, currentLabel: `${v.recentDays} days`, default: 30,
    },
    {
      key: 'network_rescan', label: 'Scan network folders again', type: 'bool', description: 'Whether folders on the network are scanned again from time to time (folders on this computer are watched, so need no setting).',
      current: v.networkRescan.on, currentLabel: onOff(v.networkRescan.on), default: true,
    },
    {
      key: 'network_rescan_minutes', label: 'Scan network folders every', type: 'choice', description: 'How often folders on the network are scanned again, when that is on.',
      options: NETWORK_MINUTES.map((m) => ({ value: m, label: minutesLabel(m) })), current: v.networkRescan.minutes, currentLabel: minutesLabel(v.networkRescan.minutes), default: 30,
    },
    {
      key: 'analysis_tags', label: 'Auto tags', type: 'bool', description: 'Label photos with what is in them (beach, dog, ...), worked out on this computer.',
      current: v.analysis.tags, currentLabel: onOff(v.analysis.tags), default: true,
    },
    {
      key: 'analysis_faces', label: 'Find faces', type: 'bool', description: 'Look for faces in photos so people can be named in the People tab.',
      current: v.analysis.faces, currentLabel: onOff(v.analysis.faces), default: true,
    },
    {
      key: 'analysis_dupes', label: 'Check for duplicates', type: 'bool', description: 'Look for photos that look the same, for the Duplicates tab.',
      current: v.analysis.dupes, currentLabel: onOff(v.analysis.dupes), default: true,
    },
    {
      key: 'trip_radius_km', label: 'Distance that counts as away (km)', type: 'float', description: 'Photos further than this from home, in kilometres, count as taken away from home. Trips need home to be confirmed first.',
      min: 1, max: MAX_RADIUS_KM, current: v.trips.radiusKm, currentLabel: `${v.trips.radiusKm} km`, default: 50,
    },
    {
      key: 'trip_min_nights', label: 'Fewest nights in a trip', type: 'int', description: 'A trip needs at least this many nights away; 0 allows a day trip.',
      min: 0, max: MAX_NIGHTS, step: 1, current: v.trips.minNights, currentLabel: `${v.trips.minNights} nights`, default: 1,
    },
    {
      key: 'trip_stop_km', label: 'Size of a stop (km)', type: 'float', description: 'Photos within this distance of each other, in kilometres, are one stop of a trip.',
      min: 1, max: MAX_STOP_KM, current: v.trips.stopKm, currentLabel: `${v.trips.stopKm} km`, default: 25,
    },
  ];
}

function labelOf(s: SettingInfo, value: number | boolean): string {
  if (typeof value === 'boolean') return onOff(value);
  if (s.type === 'choice') return s.options?.find((o) => o.value === value)?.label ?? String(value);
  const unit = s.key === 'email_limit_mb' ? ' MB' : s.key === 'recent_days' ? ' days' : s.key === 'trip_min_nights' ? ' nights' : s.key.endsWith('_km') ? ' km' : '';
  return `${value}${unit}`;
}

/** Is `value` something this setting allows? */
function allowed(s: SettingInfo, value: unknown): value is number | boolean {
  if (s.type === 'bool') return typeof value === 'boolean';
  if (typeof value !== 'number' || !Number.isFinite(value)) return false;
  if (s.type === 'choice') return s.options?.some((o) => o.value === value) ?? false;
  if (s.type === 'int' && !Number.isInteger(value)) return false;
  return value >= (s.min ?? -Infinity) && value <= (s.max ?? Infinity);
}

/**
 * What the model proposed, reduced to the changes that are safe to show: the setting must exist and the value must be
 * allowed, and differ from the current one. For one setting named twice, the last wins. `dropped` counts the rest.
 */
export function checkProposal(proposed: Proposed[], values: SettingValues): { changes: Change[]; dropped: number } {
  const schema = settingsSchema(values);
  const byKey = new Map<string, Change>();
  let dropped = 0;
  for (const p of proposed) {
    const s = schema.find((x) => x.key === p.setting);
    if (!s || !allowed(s, p.value) || p.value === s.current) { dropped++; continue; }
    if (byKey.has(s.key)) byKey.delete(s.key); // the last one counts, and takes the later place
    byKey.set(s.key, { key: s.key, label: s.label, value: p.value, from: s.currentLabel, to: labelOf(s, p.value) });
  }
  return { changes: [...byKey.values()], dropped };
}

/** Applies the changes, calling each handler at most once with the whole new group of settings. */
export function applyChanges(changes: Change[], values: SettingValues, h: Handlers): void {
  const get = (key: string) => changes.find((c) => c.key === key)?.value;
  const viewer = get('viewer_mode');
  if (typeof viewer === 'number') h.viewer(VIEWER.find((o) => o.value === viewer)!.mode);
  const mb = get('email_limit_mb');
  if (typeof mb === 'number') h.emailLimit(mb);
  const days = get('recent_days');
  if (typeof days === 'number') h.recentDays(days);
  const on = get('network_rescan'), minutes = get('network_rescan_minutes');
  if (on !== undefined || minutes !== undefined) {
    h.networkRescan({ on: typeof on === 'boolean' ? on : values.networkRescan.on, minutes: typeof minutes === 'number' ? minutes : values.networkRescan.minutes });
  }
  const tags = get('analysis_tags'), faces = get('analysis_faces'), dupes = get('analysis_dupes');
  if (tags !== undefined || faces !== undefined || dupes !== undefined) {
    h.analysis({ tags: typeof tags === 'boolean' ? tags : values.analysis.tags, faces: typeof faces === 'boolean' ? faces : values.analysis.faces, dupes: typeof dupes === 'boolean' ? dupes : values.analysis.dupes });
  }
  const radius = get('trip_radius_km'), nights = get('trip_min_nights'), stop = get('trip_stop_km');
  if (radius !== undefined || nights !== undefined || stop !== undefined) {
    h.trips({
      ...values.trips,
      radiusKm: typeof radius === 'number' ? radius : values.trips.radiusKm,
      minNights: typeof nights === 'number' ? nights : values.trips.minNights,
      stopKm: typeof stop === 'number' ? stop : values.trips.stopKm,
    });
  }
}
