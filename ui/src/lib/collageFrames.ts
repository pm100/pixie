/** Moving, resizing and ordering the frames of a collage. Rectangles are fractions of the page; nothing here knows about the screen. */
import { MIN_FRAME } from './collageDoc';

export { MIN_FRAME };
export interface Rect { x: number; y: number; w: number; h: number }
export type Handle = 'n' | 'ne' | 'e' | 'se' | 's' | 'sw' | 'w' | 'nw';

const clamp = (n: number, lo: number, hi: number) => Math.min(hi, Math.max(lo, n));

/** The rectangle moved by (`dx`, `dy`), kept on the page. */
export function moveRect(r: Rect, dx: number, dy: number): Rect {
  return { ...r, x: clamp(r.x + dx, 0, 1 - r.w), y: clamp(r.y + dy, 0, 1 - r.h) };
}

/** The rectangle with the edge or corner `handle` dragged by (`dx`, `dy`). The opposite edges stay where they are. The frame keeps at
 *  least `MIN_FRAME` and stays on the page. `pageAspect` is the page's width over its height; with `keepShape` a corner keeps the frame's
 *  shape as it is on paper (the width leads). */
export function resizeRect(r: Rect, handle: Handle, dx: number, dy: number, pageAspect: number, keepShape: boolean): Rect {
  const left = handle.includes('w'), right = handle.includes('e'), top = handle.includes('n'), bottom = handle.includes('s');
  if (keepShape && (left || right) && (top || bottom)) {
    const shape = (r.w * pageAspect) / r.h; // the frame's width over its height on paper
    const minW = Math.max(MIN_FRAME, (MIN_FRAME * shape) / pageAspect);
    const room = (right ? 1 - r.x : r.x + r.w) ;
    const roomH = (bottom ? 1 - r.y : r.y + r.h) * (shape / pageAspect);
    const w = clamp(r.w + (right ? dx : -dx), minW, Math.max(minW, Math.min(room, roomH)));
    const h = (w * pageAspect) / shape;
    return { x: left ? r.x + r.w - w : r.x, y: top ? r.y + r.h - h : r.y, w, h };
  }
  let [x0, y0, x1, y1] = [r.x, r.y, r.x + r.w, r.y + r.h];
  if (left) x0 = clamp(x0 + dx, 0, x1 - MIN_FRAME);
  if (right) x1 = clamp(x1 + dx, x0 + MIN_FRAME, 1);
  if (top) y0 = clamp(y0 + dy, 0, y1 - MIN_FRAME);
  if (bottom) y1 = clamp(y1 + dy, y0 + MIN_FRAME, 1);
  return { x: x0, y: y0, w: x1 - x0, h: y1 - y0 };
}

/** A new frame for a photo of `photoW` by `photoH`: the photo's shape, about a quarter of the page, centred on `at`, kept on the page. */
export function newFrameRect(photoW: number, photoH: number, pageMm: { w: number; h: number }, at: { x: number; y: number }): Rect {
  const shape = photoW > 0 && photoH > 0 ? photoW / photoH : 1.5;
  const toPage = pageMm.h / pageMm.w; // a width in page fractions to the same length across the page's height
  let w = Math.sqrt(0.25 * shape * toPage);
  let h = (w * pageMm.w) / (shape * pageMm.h);
  const over = Math.max(w, h, 1);
  [w, h] = [w / over, h / over];
  [w, h] = [Math.max(w, MIN_FRAME), Math.max(h, MIN_FRAME)];
  return { x: clamp(at.x - w / 2, 0, 1 - w), y: clamp(at.y - h / 2, 0, 1 - h), w, h };
}

/** Where a frame of `size` goes without covering any of `existing`: the first place on a coarse grid, left to right then down; the middle
 *  of the page when everywhere is taken. */
export function firstFreeSpot(existing: Rect[], size: Rect): { x: number; y: number } {
  const step = 0.05;
  const free = (x: number, y: number) => existing.every((e) => x + size.w <= e.x + 1e-9 || e.x + e.w <= x + 1e-9 || y + size.h <= e.y + 1e-9 || e.y + e.h <= y + 1e-9);
  for (let y = 0; y <= 1 - size.h + 1e-9; y += step) {
    for (let x = 0; x <= 1 - size.w + 1e-9; x += step) if (free(x, y)) return { x: Math.min(x, 1 - size.w), y: Math.min(y, 1 - size.h) };
  }
  return { x: (1 - size.w) / 2, y: (1 - size.h) / 2 };
}

/** The frame on top at the point (`x`, `y`), the later of any that overlap there. `frames` run bottom to top. */
export function topFrameAt<T extends Rect>(frames: T[], x: number, y: number): T | null {
  for (let i = frames.length - 1; i >= 0; i--) {
    const f = frames[i];
    if (x >= f.x && x <= f.x + f.w && y >= f.y && y <= f.y + f.h) return f;
  }
  return null;
}

/** The frames with those in `ids` moved to the top, keeping their order among themselves. */
export function bringToFront<T extends { id: string }>(frames: T[], ids: string[]): T[] {
  const moved = new Set(ids);
  return [...frames.filter((f) => !moved.has(f.id)), ...frames.filter((f) => moved.has(f.id))];
}

/** The frames with those in `ids` moved to the bottom, keeping their order among themselves. */
export function sendToBack<T extends { id: string }>(frames: T[], ids: string[]): T[] {
  const moved = new Set(ids);
  return [...frames.filter((f) => moved.has(f.id)), ...frames.filter((f) => !moved.has(f.id))];
}

/** The frame `frameId` showing `photo` instead of the one it had (shown whole, centred, not zoomed); every other frame is as it was. */
export function replacePhoto<T extends { id: string; photo: number; fx: number; fy: number; zoom: number }>(frames: T[], frameId: string, photo: number): T[] {
  if (!frames.some((f) => f.id === frameId)) return frames;
  return frames.map((f) => (f.id === frameId ? { ...f, photo, fx: 0.5, fy: 0.5, zoom: 1 } : f));
}

/** The frames `aId` and `bId` with their photos exchanged and each photo shown whole again (centred, not zoomed). */
export function swapPhotos<T extends { id: string; photo: number; fx: number; fy: number; zoom: number }>(frames: T[], aId: string, bId: string): T[] {
  const a = frames.find((f) => f.id === aId), b = frames.find((f) => f.id === bId);
  if (!a || !b) return frames;
  return frames.map((f) => (f.id === aId ? { ...f, photo: b.photo, fx: 0.5, fy: 0.5, zoom: 1 } : f.id === bId ? { ...f, photo: a.photo, fx: 0.5, fy: 0.5, zoom: 1 } : f));
}
