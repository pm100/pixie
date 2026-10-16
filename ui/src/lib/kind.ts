import type { MediaKind } from './api';

/** A stored preference may be anything; only the two real kinds are a filter. */
export function parseKindFilter(v: unknown): MediaKind | null {
  return v === 'photo' || v === 'video' ? v : null;
}
