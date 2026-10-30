/** What the popovers share (Tag…, More, the tag list in Working on, the + box): where they open, and when they close. */

/** The room a popover needs below its button before it is put above instead. */
const ROOM_NEEDED = 300;

/** The space between the button and the popover, and the window's edge and the popover. */
const GAP = 4;
const MARGIN = 8;

export interface Anchor { top: number; bottom: number; left: number; right: number }
export interface Window { width: number; height: number }

/** The CSS for a popover fixed to the window beside its button: under it, or over it when there is more room there; it is as tall as the room, and scrolls.
 *  `alignRight` lines the popover's right edge up with the button's (else the left edges); with a `width` it is that wide and kept inside the window. */
export function placement(anchor: Anchor, win: Window, opts: { alignRight?: boolean; width?: number } = {}): string {
  const below = win.height - anchor.bottom - MARGIN;
  const above = anchor.top - MARGIN;
  const up = below < ROOM_NEEDED && above > below;
  const px = (n: number) => `${Math.round(n)}px`;
  let across: string;
  if (opts.width !== undefined) {
    const left = opts.alignRight ? anchor.right - opts.width : anchor.left;
    across = `left:${px(Math.max(MARGIN, Math.min(left, win.width - opts.width - MARGIN)))};width:${px(opts.width)}`;
  } else {
    across = opts.alignRight ? `right:${px(win.width - anchor.right)}` : `left:${px(anchor.left)}`;
  }
  const down = up ? `bottom:${px(win.height - anchor.top + GAP)};max-height:${px(above)}` : `top:${px(anchor.bottom + GAP)};max-height:${px(below)}`;
  return `${down};${across}`;
}

/** `placement` for a button on the page now. */
export function placeBeside(button: HTMLElement, opts: { alignRight?: boolean; width?: number } = {}): string {
  return placement(button.getBoundingClientRect(), { width: window.innerWidth, height: window.innerHeight }, opts);
}

export interface DismissOptions {
  /** The popover is open: only then are the window's events listened to. */
  active: boolean;
  close: () => void;
  /** Close when the window is resized or something outside the popover scrolls (a popover fixed to the window would be left behind). */
  closeOnMove?: boolean;
}

/** An action for the element holding a button and its popover: a press anywhere outside it, or Escape, closes the popover.
 *  The press is judged by the path it took when it happened, so a click on something that has left the page by now (a menu item) still counts as inside. */
export function dismiss(node: HTMLElement, options: DismissOptions) {
  let o = options;
  let off: (() => void) | null = null;

  function start() {
    const away = (e: MouseEvent) => { if (!e.composedPath().includes(node)) o.close(); };
    const key = (e: KeyboardEvent) => { if (e.key === 'Escape') { e.stopPropagation(); o.close(); } };
    const moved = (e: Event) => { if (!(e.target instanceof Node && node.contains(e.target))) o.close(); };
    const closeNow = () => o.close();
    window.addEventListener('mousedown', away);
    window.addEventListener('keydown', key);
    if (o.closeOnMove) {
      window.addEventListener('resize', closeNow);
      window.addEventListener('scroll', moved, true);
    }
    const closeOnMove = o.closeOnMove;
    return () => {
      window.removeEventListener('mousedown', away);
      window.removeEventListener('keydown', key);
      if (closeOnMove) {
        window.removeEventListener('resize', closeNow);
        window.removeEventListener('scroll', moved, true);
      }
    };
  }
  function sync() {
    off?.();
    off = o.active ? start() : null;
  }
  sync();
  return {
    update(next: DismissOptions) { o = next; sync(); },
    destroy() { off?.(); off = null; },
  };
}
