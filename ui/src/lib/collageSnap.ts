/** Snapping for the Collage tool: while a frame is moved or resized, its edges and centre line up with the page and with the other frames.
 *  Rectangles are fractions of the page. Nothing here knows about the screen except how many pixels the page is drawn at. */
import { MIN_FRAME, type Handle, type Rect } from './collageFrames';

/** A line to draw: at `at` (a fraction across the page on `axis`), from `from` to `to` along the other axis. `gapMm` marks an equal gap. */
export interface Guide { axis: 'x' | 'y'; at: number; from: number; to: number; gapMm?: number }

export interface SnapContext {
  others: Rect[];
  pageMm: { w: number; h: number };
  /** Half the gap setting. */
  marginMm: number;
  /** Snap to the page's edges, margin and centre lines. */
  page: boolean;
  /** Snap to the other frames. */
  frames: boolean;
  /** How close, in screen pixels, counts as near enough. */
  thresholdPx: number;
  /** The page as drawn on the screen. */
  pagePx: { w: number; h: number };
}

type Kind = 'start' | 'mid' | 'end';
interface Target { pos: number; lo: number; hi: number; gapMm?: number }

const EPS = 1e-9;

/** The accessors for one axis: along it (`start`, `size`) and across it (`cross`, `crossSize`), and its length in mm and on screen. */
function axisOf(axis: 'x' | 'y', ctx: SnapContext) {
  return axis === 'x'
    ? { start: (r: Rect) => r.x, size: (r: Rect) => r.w, cross: (r: Rect) => r.y, crossSize: (r: Rect) => r.h, mm: ctx.pageMm.w, px: ctx.pagePx.w }
    : { start: (r: Rect) => r.y, size: (r: Rect) => r.h, cross: (r: Rect) => r.x, crossSize: (r: Rect) => r.w, mm: ctx.pageMm.h, px: ctx.pagePx.h };
}

/** Everywhere a feature (the start, middle or end of the moving frame) could snap to on one axis. */
function targets(axis: 'x' | 'y', kind: Kind, moving: Rect, ctx: SnapContext): Target[] {
  const a = axisOf(axis, ctx);
  const out: Target[] = [];
  const margin = ctx.marginMm / a.mm;
  if (ctx.page) {
    const page = { lo: 0, hi: 1 };
    if (kind === 'start') out.push({ pos: 0, ...page }, { pos: margin, ...page });
    if (kind === 'end') out.push({ pos: 1, ...page }, { pos: 1 - margin, ...page });
    if (kind === 'mid') out.push({ pos: 0.5, ...page });
  }
  if (ctx.frames) {
    const overlapsAcross = (o: Rect) => a.cross(o) < a.cross(moving) + a.crossSize(moving) && a.cross(moving) < a.cross(o) + a.crossSize(o);
    // the gaps already between frames that sit side by side, and the gap setting: a frame placed next to another leaves one of them
    const gaps = new Set<number>();
    const key = (d: number) => Math.round(d * 1e4) / 1e4;
    if (ctx.marginMm > 0) gaps.add(key((2 * ctx.marginMm) / a.mm));
    for (const p of ctx.others) {
      for (const q of ctx.others) {
        const d = a.start(q) - (a.start(p) + a.size(p));
        if (p !== q && d > 1e-4 && a.cross(p) < a.cross(q) + a.crossSize(q) && a.cross(q) < a.cross(p) + a.crossSize(p)) gaps.add(key(d));
      }
    }
    for (const o of ctx.others) {
      const [lo, hi] = [Math.min(a.cross(o), a.cross(moving)), Math.max(a.cross(o) + a.crossSize(o), a.cross(moving) + a.crossSize(moving))];
      const [s, e] = [a.start(o), a.start(o) + a.size(o)];
      if (kind === 'start') out.push({ pos: s, lo, hi });
      if (kind === 'end') out.push({ pos: e, lo, hi });
      if (kind === 'mid') out.push({ pos: (s + e) / 2, lo, hi });
      if (!overlapsAcross(o)) continue;
      for (const d of gaps) {
        if (kind === 'start') out.push({ pos: e + d, lo, hi, gapMm: d * a.mm });
        if (kind === 'end') out.push({ pos: s - d, lo, hi, gapMm: d * a.mm });
      }
    }
  }
  return out;
}

/** The nearest snap on one axis for the features named in `kinds`: how far to move, and the guides to draw. */
function snapAxis(axis: 'x' | 'y', r: Rect, kinds: Kind[], ctx: SnapContext): { delta: number; guides: Guide[] } {
  const a = axisOf(axis, ctx);
  const threshold = ctx.thresholdPx / Math.max(1, a.px);
  const value = (k: Kind) => (k === 'start' ? a.start(r) : k === 'end' ? a.start(r) + a.size(r) : a.start(r) + a.size(r) / 2);
  const found: { delta: number; target: Target }[] = [];
  for (const k of kinds) for (const t of targets(axis, k, r, ctx)) found.push({ delta: t.pos - value(k), target: t });
  const near = found.filter((f) => Math.abs(f.delta) <= threshold + EPS);
  if (near.length === 0) return { delta: 0, guides: [] };
  const best = near.reduce((m, f) => (Math.abs(f.delta) < Math.abs(m.delta) ? f : m)).delta;
  const guides: Guide[] = [];
  for (const f of near.filter((f) => Math.abs(f.delta - best) < 1e-6)) {
    // (two targets can be the same line, for example the page edge and a margin of nothing)
    if (guides.some((g) => Math.abs(g.at - f.target.pos) < 1e-9 && g.gapMm === f.target.gapMm)) continue;
    guides.push({ axis, at: f.target.pos, from: f.target.lo, to: f.target.hi, ...(f.target.gapMm === undefined ? {} : { gapMm: f.target.gapMm }) });
  }
  return { delta: best, guides };
}

/** The frame being moved, snapped. It never ends up off the page. */
export function snapMove(r: Rect, ctx: SnapContext): { rect: Rect; guides: Guide[] } {
  const sx = snapAxis('x', r, ['start', 'mid', 'end'], ctx);
  const sy = snapAxis('y', r, ['start', 'mid', 'end'], ctx);
  const x = r.x + sx.delta, y = r.y + sy.delta;
  const okX = x >= -EPS && x + r.w <= 1 + EPS, okY = y >= -EPS && y + r.h <= 1 + EPS;
  return { rect: { ...r, x: okX ? x : r.x, y: okY ? y : r.y }, guides: [...(okX ? sx.guides : []), ...(okY ? sy.guides : [])] };
}

/** The frame being resized by `handle`, snapped: only the edges being dragged move. */
export function snapResize(r: Rect, handle: Handle, ctx: SnapContext): { rect: Rect; guides: Guide[] } {
  const guides: Guide[] = [];
  let [x0, y0, x1, y1] = [r.x, r.y, r.x + r.w, r.y + r.h];
  let [movedX, movedY] = [false, false];
  const edge = (axis: 'x' | 'y', kind: Kind) => {
    const s = snapAxis(axis, r, [kind], ctx);
    if (s.delta === 0 && s.guides.length === 0) return;
    const [lo, hi] = axis === 'x' ? [x0, x1] : [y0, y1];
    const [nlo, nhi] = kind === 'start' ? [lo + s.delta, hi] : [lo, hi + s.delta];
    if (nlo < -EPS || nhi > 1 + EPS || nhi - nlo < MIN_FRAME) return;
    if (axis === 'x') [x0, x1, movedX] = [nlo, nhi, true];
    else [y0, y1, movedY] = [nlo, nhi, true];
    guides.push(...s.guides);
  };
  if (handle.includes('w')) edge('x', 'start');
  if (handle.includes('e')) edge('x', 'end');
  if (handle.includes('n')) edge('y', 'start');
  if (handle.includes('s')) edge('y', 'end');
  // (an axis that did not snap is returned exactly as it was)
  return { rect: { x: movedX ? x0 : r.x, y: movedY ? y0 : r.y, w: movedX ? x1 - x0 : r.w, h: movedY ? y1 - y0 : r.h }, guides };
}
