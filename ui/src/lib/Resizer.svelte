<script lang="ts">
  import { clamp } from './gridMath';

  let { width = $bindable(), min, max, side, onchange }: {
    width: number; min: number; max: number;
    /** Which side of the handle the pane is on: dragging right grows a left pane, shrinks a right one. */
    side: 'left' | 'right';
    onchange?: (width: number) => void;
  } = $props();

  let dragging = $state(false);
  let startX = 0, startW = 0;

  function down(e: PointerEvent) {
    dragging = true;
    startX = e.clientX;
    startW = width;
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
  }
  function move(e: PointerEvent) {
    if (!dragging) return;
    const dx = e.clientX - startX;
    width = clamp(startW + (side === 'left' ? dx : -dx), min, max);
  }
  function up() {
    if (!dragging) return;
    dragging = false;
    onchange?.(width);
  }
</script>

<div class="bar" class:dragging role="separator" aria-orientation="vertical"
     onpointerdown={down} onpointermove={move} onpointerup={up} onpointercancel={up}></div>

<style>
  .bar { cursor: col-resize; background: #e4e4e8; touch-action: none; }
  .bar:hover, .bar.dragging { background: #2f6fed; }
</style>
