import type { Place } from './api';

export interface PlaceLines {
  primary: string | null;
  secondary: string | null;
}

export function placeLines(p: Place | null): PlaceLines {
  if (!p) return { primary: null, secondary: null };
  if (p.landmark) return { primary: p.landmark, secondary: p.locality };
  return { primary: p.locality, secondary: null };
}
