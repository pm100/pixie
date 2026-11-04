/** The Photomosaic tool's settings, kept between sessions as one piece of text. Whatever is read back is checked, because it may be old,
 *  damaged or from a newer Pixie: text that cannot be used gives the defaults, never an error. */
import { dayStamp, defaultDpi, pagePx, pathIn } from './collage';
import { defaultPaperId, findPaper } from './collagePapers';
import { plural } from './format';

export const MIN_TILE_MM = 3;
export const MAX_TILE_MM = 100;
export const MAX_GAP = 50;

export interface MosaicDoc {
  v: 1;
  /** The photo the crop below is for: always the photo chosen in Browse (a different one starts again from the middle). */
  target: number | null;
  /** The point of the photo kept in view (0 to 1), and how far it is zoomed in (1 to 4). */
  focusX: number; focusY: number; zoom: number;
  /** A paper id (see `collagePapers`). */
  paper: string;
  landscape: boolean;
  /** Millimetres. */
  tileMm: number;
  /** Where the tiles come from ("Working on"): `picked`, `filtered` (what Browse shows now) or `tag:<name>`. */
  source: string;
  /** Photos repeat no closer than this many tiles (as far as the library allows). */
  gap: number;
  /** 0 to 1: how far each tile is tinted towards the colour of the target where it sits. */
  blend: number;
}

export function newMosaicDoc(locale?: string): MosaicDoc {
  return { v: 1, target: null, focusX: 0.5, focusY: 0.5, zoom: 1, paper: defaultPaperId(locale), landscape: false, tileMm: 15, source: 'filtered', gap: 8, blend: 0 };
}

const num = (v: unknown, lo: number, hi: number, dflt: number): number => (typeof v === 'number' && Number.isFinite(v) ? Math.min(hi, Math.max(lo, v)) : dflt);

/** A stored choice of where the tiles come from. The first version offered the whole library and what Browse shows: both are now `filtered`
 *  (with no filter, Browse shows the whole library). */
function sourceOf(v: unknown, dflt: string): string {
  if (v === 'picked' || v === 'filtered') return v;
  if (v === 'library' || v === 'browse') return 'filtered';
  return typeof v === 'string' && v.startsWith('tag:') && v.length > 4 ? v : dflt;
}

export function parseMosaicDoc(text: string | null, locale?: string): MosaicDoc {
  const d = newMosaicDoc(locale);
  if (!text) return d;
  let raw: unknown;
  try { raw = JSON.parse(text); } catch { return d; }
  if (typeof raw !== 'object' || raw === null) return d;
  const r = raw as Record<string, unknown>;
  return {
    v: 1,
    target: typeof r.target === 'number' && Number.isInteger(r.target) ? r.target : null,
    focusX: num(r.focusX, 0, 1, d.focusX),
    focusY: num(r.focusY, 0, 1, d.focusY),
    zoom: num(r.zoom, 1, 4, d.zoom),
    paper: typeof r.paper === 'string' && findPaper(r.paper) ? r.paper : d.paper,
    landscape: typeof r.landscape === 'boolean' ? r.landscape : d.landscape,
    tileMm: num(r.tileMm, MIN_TILE_MM, MAX_TILE_MM, d.tileMm),
    source: sourceOf(r.source, d.source),
    gap: Math.round(num(r.gap, 1, MAX_GAP, d.gap)),
    blend: num(r.blend, 0, 1, d.blend),
  };
}

export const serializeMosaicDoc = (d: MosaicDoc): string => JSON.stringify(d);

/** The page in millimetres, turned to suit the orientation. */
export function mosaicPageMm(d: MosaicDoc): { w: number; h: number } {
  const size = findPaper(d.paper) ?? findPaper(defaultPaperId())!;
  const [short, long] = [Math.min(size.w, size.h), Math.max(size.w, size.h)];
  return d.landscape ? { w: long, h: short } : { w: short, h: long };
}

/** The resolution the picture is made at: 300 dpi, or 150 for the big posters. */
export function mosaicDpi(d: MosaicDoc): number {
  const { w, h } = mosaicPageMm(d);
  return defaultDpi(w, h);
}

/** How many whole tiles fit across and down, worked out the way the app does it. */
export function tileCount(pageWmm: number, pageHmm: number, tileMm: number, dpi: number): { cols: number; rows: number; tiles: number } {
  const page = pagePx(pageWmm, pageHmm, dpi);
  const tile = Math.max(1, Math.round((Math.max(0, tileMm) / 25.4) * dpi));
  const cols = Math.max(1, Math.floor(page.w / tile));
  const rows = Math.max(1, Math.floor(page.h / tile));
  return { cols, rows, tiles: cols * rows };
}

/** "3,200 tiles from 18,000 photos." and, when the tiles outnumber the photos, a word that some repeat. */
export function sourceLine(tiles: number, photos: number): string {
  const line = `${plural(tiles, 'tile')} from ${plural(photos, 'photo')}.`;
  return tiles > photos ? `${line} Some photos will repeat.` : line;
}

/** The picture built does not match the settings now: none was built, or the settings have changed since (or are not worked out yet). */
export function isStale(built: { key: string } | null, currentKey: string | null): boolean {
  return built === null || currentKey === null || built.key !== currentKey;
}

/** Where the save dialog starts: `mosaic_2026-11-03.png` in `dir` (or just the name when there is no folder). */
export const mosaicSavePath = (dir: string | null | undefined, date: Date): string => pathIn(dir, `mosaic_${dayStamp(date)}.png`);
