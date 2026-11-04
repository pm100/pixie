<script lang="ts">
  import { tick, type Snippet } from 'svelte';
  import { fitSize, hangFrom, keepInside, originOf, parseCardPrefs, pointerX, type Box, type Place } from './askCard';
  import { loadPref, savePref } from './prefs';

  /** The Ask Pixie card: floats over the window, hangs from the ? button with a pointer to it and grows out of it when it opens. It stays
   *  built while closed (the conversation inside is kept), and keeps where it was dragged to and how big it was made. */
  let { open, anchor, onclose, children }: {
    open: boolean;
    /** The ? button. */
    anchor: HTMLElement | null;
    onclose: () => void;
    children: Snippet;
  } = $props();

  const kept = parseCardPrefs(loadPref<unknown>('help.card', null));
  let size = $state({ ...kept.size });
  /** Where it was dragged to; null while it still hangs from the button. */
  let moved = $state<{ left: number; top: number } | null>(kept.moved);
  let view = $state({ w: window.innerWidth, h: window.innerHeight });
  let button = $state<Box>({ left: 0, top: 0, right: 0, bottom: 0 });

  function measure() {
    view = { w: window.innerWidth, h: window.innerHeight };
    const r = anchor?.getBoundingClientRect();
    if (r) button = { left: r.left, top: r.top, right: r.right, bottom: r.bottom };
  }
  let card = $state<HTMLElement | null>(null);
  $effect(() => {
    if (!open) return;
    measure();
    void tick().then(() => card?.querySelector('textarea')?.focus({ preventScroll: true }));
  });

  const place = $derived<Place>(moved ? keepInside({ ...moved, w: size.w, h: size.h }, view) : hangFrom(button, size, view));
  const origin = $derived(originOf(button, place));
  const tip = $derived(pointerX(button, place));

  function keep() {
    savePref('help.card', { w: size.w, h: size.h, ...(moved ?? {}) });
  }

  let gesture: { kind: 'move' | 'size'; x: number; y: number; from: Place } | null = null;

  function start(e: PointerEvent, kind: 'move' | 'size') {
    if ((e.target as HTMLElement).closest('button')) return;
    gesture = { kind, x: e.clientX, y: e.clientY, from: { ...place } };
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    e.preventDefault();
  }

  function drag(e: PointerEvent) {
    if (!gesture) return;
    const dx = e.clientX - gesture.x, dy = e.clientY - gesture.y;
    const g = gesture.from;
    if (gesture.kind === 'move') {
      const next = keepInside({ ...g, left: g.left + dx, top: g.top + dy }, view);
      moved = { left: next.left, top: next.top };
    } else {
      // The handle is the bottom-left corner: the right edge stays where it is.
      const next = fitSize({ w: g.w - dx, h: g.h + dy }, view);
      size = next;
      moved = { left: g.left + g.w - next.w, top: g.top };
    }
  }

  function end() {
    if (!gesture) return;
    gesture = null;
    keep();
  }

  /** Back to hanging from the button, at the size it was made. */
  function dock() {
    moved = null;
    keep();
  }

  function key(e: KeyboardEvent) {
    if (e.key === 'Escape') { e.stopPropagation(); onclose(); }
  }
</script>

<svelte:window onresize={measure} />

<!-- The card is not a dialog: nothing behind it is blocked, and it stays while the person works in the window. -->
<div class="card" class:open bind:this={card} role="complementary" aria-label="Ask Pixie" aria-hidden={!open} inert={!open} onkeydown={key}
     style:left="{place.left}px" style:top="{place.top}px" style:width="{place.w}px" style:height="{place.h}px" style:transform-origin="{origin.x}px {origin.y}px">
  {#if !moved}<span class="tip" style:left="{tip - 7}px"></span>{/if}
  <div class="bar" role="presentation" onpointerdown={(e) => start(e, 'move')} onpointermove={drag} onpointerup={end} onpointercancel={end} ondblclick={dock}
       title="Drag to move; double-click to put it back under the ? button">
    <span class="title">Ask Pixie</span>
    <button class="x" onclick={onclose} aria-label="Close Ask Pixie" title="Close Ask Pixie (F1)">✕</button>
  </div>
  <div class="body">{@render children()}</div>
  <div class="grip" role="presentation" onpointerdown={(e) => start(e, 'size')} onpointermove={drag} onpointerup={end} onpointercancel={end} title="Drag to change the size"></div>
</div>

<style>
  .card {
    position: fixed; z-index: 900; display: flex; flex-direction: column; box-sizing: border-box; background: #fff; border: 1px solid #aab3c8; border-radius: 10px;
    box-shadow: 0 10px 30px rgba(20, 30, 60, 0.28), 0 2px 6px rgba(20, 30, 60, 0.18);
    /* Shut: shrunk into the ? button and gone. Open: grown out of it. */
    opacity: 0; transform: scale(0.2); visibility: hidden; pointer-events: none;
    transition: transform 0.18s cubic-bezier(0.2, 0.8, 0.2, 1), opacity 0.14s ease-out, visibility 0s linear 0.18s;
  }
  .card.open { opacity: 1; transform: none; visibility: visible; pointer-events: auto; transition-delay: 0s; }
  @media (prefers-reduced-motion: reduce) { .card, .card.open { transition: none; } }
  .bar { display: flex; align-items: center; gap: 8px; padding: 0 6px 0 12px; height: 34px; flex: none; background: #eaf0ff; border-bottom: 1px solid #cfd8ee; border-radius: 9px 9px 0 0; cursor: move; user-select: none; touch-action: none; }
  .title { flex: 1; font-weight: 600; font-size: 13px; color: #1d3f8f; }
  .x { width: 24px; height: 24px; padding: 0; border: 0; border-radius: 4px; background: transparent; color: #555; font-size: 12px; cursor: pointer; }
  .x:hover { background: #d7e1fa; color: #111; }
  /* The pointer at the top edge that points at the ? button; the same colour as the bar so they read as one shape. */
  .tip { position: absolute; top: -7px; width: 12px; height: 12px; background: #eaf0ff; border-left: 1px solid #aab3c8; border-top: 1px solid #aab3c8; transform: rotate(45deg); border-top-left-radius: 2px; }
  .body { flex: 1; min-height: 0; display: flex; flex-direction: column; border-radius: 0 0 9px 9px; overflow: hidden; }
  /* A grip in the bottom-left corner (the card hangs from the right). */
  .grip { position: absolute; left: 0; bottom: 0; width: 16px; height: 16px; cursor: nesw-resize; touch-action: none; border-bottom-left-radius: 9px;
          background: linear-gradient(45deg, transparent 0 40%, #aab3c8 40% 48%, transparent 48% 62%, #aab3c8 62% 70%, transparent 70%) ; }
</style>
