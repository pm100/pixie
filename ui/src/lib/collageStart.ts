/** The three ways to begin a collage: from a template, laid out by Pixie, or blank (which is just no frames). */
import type { CollageCell, Photo } from './api';
import { pageMm, type CollageDoc, type CollageFrame } from './collageDoc';
import type { Rect } from './collageFrames';
import { AUTO_LAYOUT_MAX, layoutSuggestion, templateFrames } from './collageLayout';

let counter = 0;
/** A name for a new frame, different from every other. */
export const newFrameId = (): string => `f${Date.now().toString(36)}${(counter++).toString(36)}${Math.random().toString(36).slice(2, 6)}`;

/** A frame showing the whole of `photo` (centred, not zoomed) at `rect`. */
export const makeFrame = (photo: { id: number }, rect: Rect): CollageFrame => ({ id: newFrameId(), photo: photo.id, ...rect, fx: 0.5, fy: 0.5, zoom: 1 });

/** The photos in the spaces of a template, in order. A space with no photo gets no frame; photos past the last space are counted as left out. */
export function framesFromTemplate(cells: CollageCell[], doc: CollageDoc, photos: Photo[]): { frames: CollageFrame[]; leftOut: number } {
  const rects = templateFrames(cells, pageMm(doc), doc.gap);
  const used = Math.min(rects.length, photos.length);
  return { frames: rects.slice(0, used).map((r, i) => makeFrame(photos[i], r)), leftOut: photos.length - used };
}

/** The frames on the page arranged again, as suggestion number `index` (0 is the best fit; see `layoutSuggestion`). Each photo keeps its
 *  shape, so it is shown whole again. Only the first `AUTO_LAYOUT_MAX` frames are arranged; `photos` gives the size of each frame's photo. */
export function relayoutFrames(frames: CollageFrame[], photos: Map<number, Photo>, doc: CollageDoc, index: number): CollageFrame[] {
  const shapes = frames.slice(0, AUTO_LAYOUT_MAX).map((f) => ({ w: photos.get(f.photo)?.width ?? 0, h: photos.get(f.photo)?.height ?? 0 }));
  const { rects } = layoutSuggestion(shapes, pageMm(doc), doc.gap, index);
  return frames.map((f, i) => (i < rects.length ? { ...f, ...rects[i], fx: 0.5, fy: 0.5, zoom: 1 } : f));
}
