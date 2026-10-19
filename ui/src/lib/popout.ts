import type { View } from './api';
import { parseKindFilter } from './kind';

export const POPOUT_SHOW = 'viewer-show';
export const POPOUT_READY = 'viewer-ready';
export const POPOUT_MOVED = 'viewer-moved';
export const POPOUT_CLOSED = 'viewer-closed';
/** main -> pop-out: close yourself (its photos were removed). */
export const POPOUT_CLOSE = 'viewer-close';

/** `follow` is true for messages sent because the main window's selection changed, false for an explicit "open this". */
export interface ShowMessage { index: number; total: number; view: View; follow: boolean }

const isInt = (v: unknown): v is number => typeof v === 'number' && Number.isInteger(v);

function parseView(v: unknown): View | null {
  if (typeof v !== 'object' || v === null) return null;
  const o = v as Record<string, unknown>;
  if (o.order !== 'newest' && o.order !== 'oldest') return null;
  const num = (x: unknown) => (typeof x === 'number' ? x : null);
  return {
    order: o.order, folderId: num(o.folderId), kind: parseKindFilter(o.kind),
    bounds: (o.bounds ?? null) as View['bounds'], maxId: num(o.maxId), since: num(o.since), until: num(o.until), search: o.search === true,
  };
}

/** Events carry untrusted data; only a message that is fully valid is acted on. */
export function parseShow(v: unknown): ShowMessage | null {
  if (typeof v !== 'object' || v === null) return null;
  const o = v as Record<string, unknown>;
  if (!isInt(o.index) || !isInt(o.total) || o.total < 1 || o.index < 0 || o.index >= o.total) return null;
  const view = parseView(o.view);
  return view ? { index: o.index, total: o.total, view, follow: o.follow === true } : null;
}

/** The pop-out moved to `index` of the listing (`total`, `view`) it was sent; the main window only follows if that is still its listing. */
export function parseMoved(v: unknown): { index: number; total: number; view: View } | null {
  if (typeof v !== 'object' || v === null) return null;
  const o = v as Record<string, unknown>;
  if (!isInt(o.index) || !isInt(o.total) || o.total < 1 || o.index < 0 || o.index >= o.total) return null;
  const view = parseView(o.view);
  return view ? { index: o.index, total: o.total, view } : null;
}
