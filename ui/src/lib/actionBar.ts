import { TOOLS } from './tools';

/** Something that can sit on an action bar (the row of buttons that act on photos). Tag is a popover rather than a tool in a tab. */
export interface BarAction { id: string; label: string; built: boolean }

/** Every action that can go on the bar, in the order they appear: Email, Print, Tag…, then the other tools of Send, Create and Organize. Tools that work on the whole library (Clean up) are not here. */
export const BAR_ACTIONS: BarAction[] = (() => {
  const tools = TOOLS.filter((t) => t.group !== 'cleanup' && t.id !== 'tags').map((t) => ({ id: t.id, label: t.label, built: t.built }));
  const [email, print, ...rest] = tools;
  return [email, print, { id: 'tag', label: 'Tag…', built: true }, ...rest];
})();

/** What is on the bar until the user chooses. */
export const DEFAULT_BAR = ['email', 'print', 'tag'];

/** The saved choice, as it should be used: only known actions, once each, in the bar's own order. Anything else (nothing saved, damage) gives the default; an empty list is a real choice. */
export function parseBar(raw: unknown): string[] {
  if (!Array.isArray(raw)) return [...DEFAULT_BAR];
  const wanted = new Set(raw.filter((x): x is string => typeof x === 'string'));
  return BAR_ACTIONS.filter((a) => wanted.has(a.id)).map((a) => a.id);
}

/** The bar's choice with one action switched on or off. */
export function toggled(chosen: readonly string[], id: string, on: boolean): string[] {
  return parseBar(on ? [...chosen, id] : chosen.filter((x) => x !== id));
}

/** The width the rest of Browse's second line needs (the picked controls, the map controls, the dividers and More), and what each button on the bar takes. */
const FIXED_WIDTH = 670;
const BUTTON_WIDTH = 72;

/** How many of the chosen buttons fit in a line of this width; the rest go under More. */
export function directCount(lineWidth: number, wanted: number): number {
  const room = Math.floor((lineWidth - FIXED_WIDTH) / BUTTON_WIDTH);
  return Math.max(0, Math.min(wanted, room));
}

/** The chosen actions that can be used (built ones; the bar leaves out a tool that is not built yet), split into the buttons shown and the ones folded under More. */
export function split(chosen: readonly string[], limit: number, available: (id: string) => boolean = () => true): { direct: string[]; folded: string[] } {
  const usable = parseBar(chosen).filter((id) => BAR_ACTIONS.find((a) => a.id === id)?.built && available(id));
  return { direct: usable.slice(0, Math.max(0, limit)), folded: usable.slice(Math.max(0, limit)) };
}
