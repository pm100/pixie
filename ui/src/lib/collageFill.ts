/** Fill-the-space snapping: a photo moved into a free space on the page grows or shrinks to fill that space, keeping its shape (so its crop
 *  looks the same). Rectangles are fractions of the page. */
import { MIN_FRAME } from './collageDoc';
import type { Rect } from './collageFrames';

const EPS = 1e-9;

/** The biggest rectangle with the shape of `frame` that contains the point `at`, stays inside the page's margin (half the gap) and keeps
 *  the gap from every one of `others`; of those it is the one whose middle is nearest `at`. Null when `at` is not in free space (it is
 *  over another photo or in the gap beside one) or the free space there is too small for a frame. `pageMm` is the page's size in
 *  millimetres, to turn the gap into fractions across and down. */
export function fillSpace(frame: Rect, others: Rect[], pageMm: { w: number; h: number }, gapMm: number, at: { x: number; y: number }): Rect | null {
  const ratio = frame.h / frame.w; // the frame's height over its width, as fractions of the page: unchanged, so its shape is
  if (!(ratio > 0) || !Number.isFinite(ratio)) return null;
  const [gx, gy] = [gapMm / pageMm.w, gapMm / pageMm.h];
  const [mx, my] = [gx / 2, gy / 2];
  // the other frames, grown by the gap: a frame placed against them stays a whole gap away
  const walls = others.map((o) => ({ x0: o.x - gx, y0: o.y - gy, x1: o.x + o.w + gx, y1: o.y + o.h + gy }));
  const [left, right, top, bottom] = [mx, 1 - mx, my, 1 - my];
  if (at.x < left || at.x > right || at.y < top || at.y > bottom) return null;

  /** The place for a frame `w` wide (the one nearest `at`), or null if there is none. */
  const place = (w: number): Rect | null => {
    const h = w * ratio;
    if (w > right - left + EPS || h > bottom - top + EPS) return null;
    const xs = [left, right - w, at.x - w / 2, ...walls.flatMap((o) => [o.x1, o.x0 - w])];
    const ys = [top, bottom - h, at.y - h / 2, ...walls.flatMap((o) => [o.y1, o.y0 - h])];
    let best: Rect | null = null;
    let bestDistance = Infinity;
    for (const x of xs) {
      if (x < left - EPS || x + w > right + EPS || x > at.x + EPS || x + w < at.x - EPS) continue;
      for (const y of ys) {
        if (y < top - EPS || y + h > bottom + EPS || y > at.y + EPS || y + h < at.y - EPS) continue;
        if (walls.some((o) => x < o.x1 - EPS && x + w > o.x0 + EPS && y < o.y1 - EPS && y + h > o.y0 + EPS)) continue;
        const distance = (x + w / 2 - at.x) ** 2 + (y + h / 2 - at.y) ** 2;
        if (distance < bestDistance) { best = { x, y, w, h }; bestDistance = distance; }
      }
    }
    return best;
  };

  // A frame that fits in a space fits in the same space when it is smaller, so the biggest one is found by halving the range.
  let lo = Math.max(MIN_FRAME, MIN_FRAME / ratio);
  if (!place(lo)) return null;
  let hi = Math.min(right - left, (bottom - top) / ratio);
  if (hi < lo) return place(lo);
  for (let i = 0; i < 28 && hi - lo > 1e-6; i++) {
    const mid = (lo + hi) / 2;
    if (place(mid)) lo = mid; else hi = mid;
  }
  return place(hi - lo < 1e-6 && place(hi) ? hi : lo);
}
