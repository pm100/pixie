/** Maths shared by the Collage tool: the saved page, how a photo is cropped to its frame, and how sharp it will print. The crop rule is the
 *  same as the one the app draws the saved file with (`cover_crop` in `crates/pixie-image/src/collage.rs`), so the preview matches the file. */

/** The longest side of a saved page, in pixels (the same limit as the saving code). */
export const MAX_SIDE_PX = 16384;

/** Print shops ask for 300 dpi on small prints and 150 on posters (A2 and larger). */
export const defaultDpi = (wMm: number, hMm: number): number => (Math.max(wMm, hMm) >= 594 ? 150 : 300);

/** The saved page in pixels. */
export function pagePx(wMm: number, hMm: number, dpi: number): { w: number; h: number } {
  const px = (mm: number) => Math.max(1, Math.round((mm / 25.4) * dpi));
  return { w: px(wMm), h: px(hMm) };
}

/** Why a page cannot be saved at this size and resolution, or null when it can. */
export function sizeProblem(wMm: number, hMm: number, dpi: number): string | null {
  if (!(wMm > 0 && hMm > 0)) return 'Give the page a width and a height.';
  const { w, h } = pagePx(wMm, hMm, dpi);
  if (Math.max(w, h) > MAX_SIDE_PX) return `That page would be ${w.toLocaleString()} × ${h.toLocaleString()} pixels; the longest side can be at most ${MAX_SIDE_PX.toLocaleString()}. Choose a smaller page or a lower resolution.`;
  return null;
}

/** Below this many dots per inch a photo starts to look soft in print. */
export const LOW_DPI = 100;

const clamp01 = (n: number) => Math.min(1, Math.max(0, n));

/** The part of a `srcW` by `srcH` photo that fills a `cellW` by `cellH` frame: the biggest rectangle of the frame's shape that fits in the
 *  photo, shrunk by `zoom` (1 to 4) and placed by the focus (0 is the left or top, 1 the right or bottom). */
export function coverCrop(srcW: number, srcH: number, cellW: number, cellH: number, fx: number, fy: number, zoom: number): { x: number; y: number; w: number; h: number } {
  const [sw, sh, cw, ch] = [Math.max(1, srcW), Math.max(1, srcH), Math.max(1, cellW), Math.max(1, cellH)];
  const clampTo = (n: number, hi: number) => Math.min(hi, Math.max(1, n));
  let [w, h] = sw * ch > cw * sh ? [clampTo(Math.floor((sh * cw) / ch), sw), sh] : [sw, clampTo(Math.floor((sw * ch) / cw), sh)];
  const z = Number.isFinite(zoom) ? Math.min(4, Math.max(1, zoom)) : 1;
  [w, h] = [clampTo(Math.round(w / z), w), clampTo(Math.round(h / z), h)];
  return { x: Math.round((sw - w) * clamp01(fx)), y: Math.round((sh - h) * clamp01(fy)), w, h };
}

/** The focus after dragging a photo inside its frame. The photo moves with the pointer, so the part shown moves the other way:
 *  dragging right lowers the focus. `overflowPx` is how far the photo sticks out of the frame on each axis; an axis with none stays put. */
export function focusFromDrag(start: { fx: number; fy: number }, dragPx: { dx: number; dy: number }, overflowPx: { x: number; y: number }): { fx: number; fy: number } {
  return {
    fx: overflowPx.x > 0 ? clamp01(start.fx - dragPx.dx / overflowPx.x) : start.fx,
    fy: overflowPx.y > 0 ? clamp01(start.fy - dragPx.dy / overflowPx.y) : start.fy,
  };
}

/** How far a photo of `srcW` by `srcH` sticks out of a `cellW` by `cellH` frame once scaled to fill it (and zoomed in). */
export function overflow(srcW: number, srcH: number, cellW: number, cellH: number, zoom = 1): { x: number; y: number } {
  if (srcW <= 0 || srcH <= 0) return { x: 0, y: 0 };
  const scale = Math.max(cellW / srcW, cellH / srcH) * Math.min(4, Math.max(1, zoom));
  return { x: Math.max(0, srcW * scale - cellW), y: Math.max(0, srcH * scale - cellH) };
}

/** How sharp the photo will be in its frame: the pixels of the part that shows, per inch of the printed frame. */
export function frameDpi(photoW: number, photoH: number, frame: { w: number; h: number; fx: number; fy: number; zoom: number }, pageMm: { w: number; h: number }): number {
  const [wMm, hMm] = [Math.max(0.01, frame.w * pageMm.w), Math.max(0.01, frame.h * pageMm.h)];
  const per = (mm: number) => Math.max(1, Math.round((mm / 25.4) * 1000)); // (a fixed scale is enough: only the shape of the frame matters)
  const crop = coverCrop(photoW, photoH, per(wMm), per(hMm), frame.fx, frame.fy, frame.zoom);
  return crop.w / (wMm / 25.4);
}

/** The name a saved collage starts with: `collage_2026-11-03.png`, by the local date. */
export function collageFileName(date: Date): string {
  const two = (n: number) => String(n).padStart(2, '0');
  return `collage_${date.getFullYear()}-${two(date.getMonth() + 1)}-${two(date.getDate())}.png`;
}

/** Where the save dialog starts: that name in `dir` (written with the folder's own kind of separator), or just the name when there is no folder. */
export function defaultSavePath(dir: string | null | undefined, date: Date): string {
  const name = collageFileName(date);
  if (!dir) return name;
  const sep = dir.includes('\\') ? '\\' : '/';
  return dir.endsWith('\\') || dir.endsWith('/') ? `${dir}${name}` : `${dir}${sep}${name}`;
}
