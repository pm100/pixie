/** Where the Ask Pixie card goes: it hangs from the ? button, can be dragged and resized, and grows out of the button when it opens. */
import { clamp } from './gridMath';

export interface Box { left: number; top: number; right: number; bottom: number }
export interface Size { w: number; h: number }
export interface Place { left: number; top: number; w: number; h: number }

export const DEFAULT_SIZE: Size = { w: 380, h: 540 };
export const MIN_SIZE: Size = { w: 300, h: 280 };
/** Space kept between the card and the window edge, and the gap (with the pointer in it) between the card and the ? button. */
export const MARGIN = 8;
export const GAP = 12;

/** A size that fits in the window, as near the wanted one as it can be. */
export function fitSize(want: Size, view: Size): Size {
  return { w: clamp(want.w, MIN_SIZE.w, Math.max(MIN_SIZE.w, view.w - 2 * MARGIN)), h: clamp(want.h, MIN_SIZE.h, Math.max(MIN_SIZE.h, view.h - 2 * MARGIN)) };
}

/** Hanging from the ? button: the card's right edge a little past the button's, just below it, no taller than the room under it. */
export function hangFrom(anchor: Box, want: Size, view: Size): Place {
  const size = fitSize(want, view);
  const top = anchor.bottom + GAP;
  const h = clamp(size.h, MIN_SIZE.h, Math.max(MIN_SIZE.h, view.h - top - MARGIN));
  return { left: clamp(anchor.right + 6 - size.w, MARGIN, Math.max(MARGIN, view.w - size.w - MARGIN)), top, w: size.w, h };
}

/** Wherever it was dragged to, kept inside the window (the title bar must stay reachable). */
export function keepInside(place: Place, view: Size): Place {
  const size = fitSize({ w: place.w, h: place.h }, view);
  return { ...size, left: clamp(place.left, MARGIN, Math.max(MARGIN, view.w - size.w - MARGIN)), top: clamp(place.top, MARGIN, Math.max(MARGIN, view.h - 48)) };
}

/** The point of the card, in its own coordinates, that is the middle of the ? button: what it grows out of and shrinks back into. */
export function originOf(anchor: Box, place: Place): { x: number; y: number } {
  return { x: (anchor.left + anchor.right) / 2 - place.left, y: (anchor.top + anchor.bottom) / 2 - place.top };
}

/** Where the pointer sits along the top edge so that it points at the ? button, in the card's own coordinates. */
export function pointerX(anchor: Box, place: Place): number {
  return clamp((anchor.left + anchor.right) / 2 - place.left, 16, Math.max(16, place.w - 16));
}

export interface CardPrefs { size: Size; moved: { left: number; top: number } | null }

/** What was kept between sessions, whatever shape it has come back in. */
export function parseCardPrefs(raw: unknown): CardPrefs {
  const o = typeof raw === 'object' && raw !== null ? (raw as Record<string, unknown>) : {};
  const n = (v: unknown): number | null => (typeof v === 'number' && Number.isFinite(v) ? v : null);
  const w = n(o.w), h = n(o.h), left = n(o.left), top = n(o.top);
  return { size: { w: w ?? DEFAULT_SIZE.w, h: h ?? DEFAULT_SIZE.h }, moved: left !== null && top !== null ? { left, top } : null };
}
