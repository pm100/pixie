/** Laying photos out on a page to start a collage: from a template, or by Pixie (justified rows). All in fractions of the page. */
import type { CollageCell } from './api';
import { MIN_FRAME, type Rect } from './collageFrames';

export const AUTO_LAYOUT_MAX = 24;

/** A template's cells as frames, each pulled in by half the gap (in mm) on every side, so the gap between two cells is the whole gap and the
 *  margin at the page edge half of it. A gap bigger than a cell leaves the smallest frame. */
export function templateFrames(cells: CollageCell[], pageMm: { w: number; h: number }, gapMm: number): Rect[] {
  const [gx, gy] = [gapMm / pageMm.w, gapMm / pageMm.h];
  return cells.map((c) => {
    const w = Math.max(MIN_FRAME, c.w - gx), h = Math.max(MIN_FRAME, c.h - gy);
    return { x: Math.min(c.x + gx / 2, 1 - w), y: Math.min(c.y + gy / 2, 1 - h), w, h };
  });
}

/** One arrangement: the frames, and how much of each photo is kept on average once it is cropped to fill its frame (1 is none cropped). */
interface Plan { rects: Rect[]; kept: number }

/** The rows for `r` rows: the photos split in order so the rows are about as wide as each other. */
function rowsOf(aspects: number[], r: number): number[][] {
  const total = aspects.reduce((s, a) => s + a, 0);
  const rows: number[][] = Array.from({ length: r }, () => []);
  let before = 0;
  aspects.forEach((a, i) => {
    rows[Math.min(r - 1, Math.floor(((before + a / 2) / total) * r))].push(i);
    before += a;
  });
  return rows.filter((row) => row.length > 0);
}

/** The photos in these rows, filling the page inside its margin (half the gap, as in a template). Every row is as wide as the page and the
 *  rows share its height in the proportion they would have had if no photo were cropped, so the frames fill the page and each photo is
 *  cropped as little as the rows allow. */
function plan(aspects: number[], rows: number[][], pageMm: { w: number; h: number }, gapMm: number): Plan {
  const { w: W, h: H } = pageMm;
  const margin = gapMm / 2;
  const [usableW, usableH] = [Math.max(1, W - 2 * margin), Math.max(1, H - 2 * margin)];
  const widthFor = (row: number[]) => Math.max(1, usableW - (row.length - 1) * gapMm);
  const natural = rows.map((row) => widthFor(row) / row.reduce((s, i) => s + aspects[i], 0)); // each row's height if no photo were cropped
  const heightBudget = Math.max(rows.length, usableH - (rows.length - 1) * gapMm);
  const naturalTotal = natural.reduce((s, h) => s + h, 0);
  const rects: Rect[] = new Array(aspects.length);
  let kept = 0;
  let y = margin;
  rows.forEach((row, k) => {
    const h = (heightBudget * natural[k]) / naturalTotal;
    const sum = row.reduce((s, i) => s + aspects[i], 0);
    let x = margin;
    for (const i of row) {
      const w = (widthFor(row) * aspects[i]) / sum;
      rects[i] = { x: x / W, y: y / H, w: w / W, h: h / H };
      const shape = w / h;
      kept += Math.min(shape / aspects[i], aspects[i] / shape); // the share of the photo that is still shown
      x += w + gapMm;
    }
    y += h + gapMm;
  });
  return { rects, kept: kept / aspects.length };
}

/** A repeatable stream of numbers from a seed, for shuffling. */
function seeded(seed: number): () => number {
  let a = seed | 0;
  return () => {
    a = (a + 0x6d2b79f5) | 0;
    let t = Math.imul(a ^ (a >>> 15), 1 | a);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

/** The order the photos are laid out in for a variant: the order they are in for variant 0, a repeatable shuffle for every other. */
function orderFor(n: number, variant: number): number[] {
  const order = Array.from({ length: n }, (_, i) => i);
  if (variant === 0) return order;
  const next = seeded(variant * 7919);
  for (let i = n - 1; i > 0; i--) {
    const j = Math.floor(next() * (i + 1));
    [order[i], order[j]] = [order[j], order[i]];
  }
  return order;
}

/** How many of the best row counts each order offers before the next order is tried. */
const RANKS = 3;

/** One way of laying the photos out in rows that fill the page (`shapes` are the photos' widths and heights); `index` 0 is the one that crops
 *  the photos least. Index 1 and 2 are the next best row counts for the same order, and the next indexes try other orders of the photos, so
 *  asking again and again keeps giving different arrangements (the same index always gives the same one). The frames cover the page inside
 *  its margin, with the gap between them; a photo whose shape does not quite match its frame is cropped to fill it. At most
 *  `AUTO_LAYOUT_MAX` photos are used; `used` says how many. The rectangles are in the order of `shapes`. */
export function layoutSuggestion(shapes: { w: number; h: number }[], pageMm: { w: number; h: number }, gapMm: number, index: number): { rects: Rect[]; used: number } {
  const used = Math.min(shapes.length, AUTO_LAYOUT_MAX);
  if (used === 0) return { rects: [], used: 0 };
  const k = Math.min(RANKS, used);
  const variant = Math.floor(Math.max(0, index) / k);
  const order = orderFor(used, variant);
  const aspects = order.map((i) => (shapes[i].w > 0 && shapes[i].h > 0 ? shapes[i].w / shapes[i].h : 1.5));
  const seen = new Set<string>();
  const plans: Plan[] = [];
  for (let r = 1; r <= used; r++) {
    const rows = rowsOf(aspects, r);
    const key = JSON.stringify(rows);
    if (seen.has(key)) continue;
    seen.add(key);
    plans.push(plan(aspects, rows, pageMm, gapMm));
  }
  plans.sort((a, b) => b.kept - a.kept); // (a stable sort: equally good arrangements keep the fewer rows first)
  const chosen = plans[(Math.max(0, index) % k) % plans.length];
  const rects: Rect[] = new Array(used);
  order.forEach((photo, j) => { rects[photo] = chosen.rects[j]; });
  return { rects, used };
}
