/** The collage that is kept between sessions: one document, stored as text. Whatever is read back is checked, because it may be old, damaged
 *  or from a newer Pixie: a document that cannot be used gives a fresh page, never an error. */
import type { SaveArgs } from './api';
import { defaultDpi } from './collage';
import { defaultPaperId, findPaper } from './collagePapers';

/** The smallest a frame can be, as a share of the page across and down. */
export const MIN_FRAME = 0.05;

export interface CollageFrame {
  id: string;
  /** The photo's id in the library. */
  photo: number;
  /** Where the frame is, as fractions of the page. */
  x: number; y: number; w: number; h: number;
  /** The point of the photo kept in view (0 to 1), and how far it is zoomed in (1 to 4). */
  fx: number; fy: number; zoom: number;
}

export interface CollageDoc {
  v: 1;
  /** A paper id (see `collagePapers`) or `custom`. */
  paper: string;
  /** The custom size, in cm. */
  customW: number; customH: number;
  landscape: boolean;
  /** Null: the usual resolution for the size. */
  dpi: number | null;
  background: string;
  /** Millimetres. */
  gap: number; corner: number; border: number;
  borderColour: string;
  shadow: boolean;
  /** Snapping as a whole (the Snap box above the page). The three kinds below keep the person's choice while this is off. */
  snap: boolean;
  snapPage: boolean; snapFrames: boolean;
  /** A photo moved into a free space grows or shrinks to fill it. */
  snapFill: boolean;
  /** The template the page was started from, if any (for its label). */
  template: string | null;
  /** Bottom to top. */
  frames: CollageFrame[];
}

export function newDoc(locale?: string): CollageDoc {
  return {
    v: 1, paper: defaultPaperId(locale), customW: 40, customH: 60, landscape: false, dpi: null, background: '#ffffff',
    gap: 4, corner: 0, border: 0, borderColour: '#ffffff', shadow: false, snap: true, snapPage: true, snapFrames: true, snapFill: true, template: null, frames: [],
  };
}

const num = (v: unknown, lo: number, hi: number, fallback: number): number => (typeof v === 'number' && Number.isFinite(v) ? Math.min(hi, Math.max(lo, v)) : fallback);
const bool = (v: unknown, fallback: boolean): boolean => (typeof v === 'boolean' ? v : fallback);
const colour = (v: unknown, fallback: string): string => (typeof v === 'string' && /^#[0-9a-f]{6}$/i.test(v) ? v.toLowerCase() : fallback);

function frameOf(raw: unknown, index: number): CollageFrame | null {
  if (typeof raw !== 'object' || raw === null) return null;
  const f = raw as Record<string, unknown>;
  if (typeof f.photo !== 'number' || !Number.isFinite(f.photo)) return null;
  const w = num(f.w, MIN_FRAME, 1, 0.25);
  const h = num(f.h, MIN_FRAME, 1, 0.25);
  return {
    id: typeof f.id === 'string' && f.id ? f.id : `f${index}`,
    photo: f.photo,
    x: num(f.x, 0, 1 - w, 0), y: num(f.y, 0, 1 - h, 0), w, h,
    fx: num(f.fx, 0, 1, 0.5), fy: num(f.fy, 0, 1, 0.5), zoom: num(f.zoom, 1, 4, 1),
  };
}

/** The document in `text` (null when none was kept), made safe to use. */
export function parseDoc(text: string | null, locale?: string): CollageDoc {
  const fresh = newDoc(locale);
  let raw: unknown;
  try {
    raw = text ? JSON.parse(text) : null;
  } catch {
    return fresh;
  }
  if (typeof raw !== 'object' || raw === null || Array.isArray(raw)) return fresh;
  const d = raw as Record<string, unknown>;
  if (d.v !== 1) return fresh;
  const dpi = typeof d.dpi === 'number' && Number.isInteger(d.dpi) && d.dpi >= 72 && d.dpi <= 600 ? d.dpi : null;
  return {
    v: 1,
    paper: typeof d.paper === 'string' && (d.paper === 'custom' || findPaper(d.paper)) ? d.paper : fresh.paper,
    customW: num(d.customW, 1, 100, fresh.customW), customH: num(d.customH, 1, 100, fresh.customH),
    landscape: bool(d.landscape, false), dpi,
    background: colour(d.background, fresh.background),
    gap: num(d.gap, 0, 50, fresh.gap), corner: num(d.corner, 0, 100, fresh.corner), border: num(d.border, 0, 50, fresh.border),
    borderColour: colour(d.borderColour, fresh.borderColour),
    shadow: bool(d.shadow, false), snap: bool(d.snap, true), snapPage: bool(d.snapPage, true), snapFrames: bool(d.snapFrames, true), snapFill: bool(d.snapFill, true),
    template: typeof d.template === 'string' ? d.template : null,
    frames: (Array.isArray(d.frames) ? d.frames : []).map(frameOf).filter((f): f is CollageFrame => f !== null),
  };
}

export const serializeDoc = (d: CollageDoc): string => JSON.stringify(d);

/** The page in millimetres, turned to suit the orientation. */
export function pageMm(d: CollageDoc): { w: number; h: number } {
  const size = d.paper === 'custom' ? { w: d.customW * 10, h: d.customH * 10 } : (findPaper(d.paper) ?? findPaper(defaultPaperId())!);
  const [short, long] = [Math.min(size.w, size.h), Math.max(size.w, size.h)];
  return d.landscape ? { w: long, h: short } : { w: short, h: long };
}

/** The resolution the page is saved at. */
export function docDpi(d: CollageDoc): number {
  const { w, h } = pageMm(d);
  return d.dpi ?? defaultDpi(w, h);
}

/** What the app needs to draw and save the page. */
export function toSaveArgs(d: CollageDoc): SaveArgs {
  const { w, h } = pageMm(d);
  return {
    paper_w_mm: w, paper_h_mm: h, dpi: docDpi(d), background: d.background, corner_mm: d.corner, border_mm: d.border, border_colour: d.borderColour, shadow: d.shadow,
    frames: d.frames.map((f) => ({ photo_id: f.photo, x: f.x, y: f.y, w: f.w, h: f.h, focus_x: f.fx, focus_y: f.fy, zoom: f.zoom })),
  };
}

/** The paper in words, for the summary above the page. */
export function paperLabel(d: CollageDoc): string {
  return d.paper === 'custom' ? `${d.customW} × ${d.customH} cm` : (findPaper(d.paper)?.label ?? '');
}
