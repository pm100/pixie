<script lang="ts">
  import type { Snippet } from 'svelte';
  import { loadPref, savePref } from './prefs';
  import Resizer from './Resizer.svelte';

  let { left, center, right }: { left: Snippet; center: Snippet; right: Snippet } = $props();

  /** The narrowest the left pane may be: the month tiles of the date tree (four of 42 px with 3 px between, indented 28 px and
   *  12 px from the right edge) and the room for a scroll bar. Narrower and the tree would need to scroll sideways. */
  const LEFT_MIN = 250;
  let leftW = $state(Math.max(LEFT_MIN, loadPref('pane.left', 260)));
  let rightW = $state(loadPref('pane.right', 280));
</script>

<div class="shell" style:grid-template-columns="{leftW}px 5px minmax(0, 1fr) 5px {rightW}px">
  <aside>{@render left()}</aside>
  <Resizer bind:width={leftW} min={LEFT_MIN} max={480} side="left" onchange={(w) => savePref('pane.left', w)} />
  <div class="center">{@render center()}</div>
  <Resizer bind:width={rightW} min={200} max={520} side="right" onchange={(w) => savePref('pane.right', w)} />
  <aside>{@render right()}</aside>
</div>

<style>
  .shell { display: grid; height: 100%; min-height: 0; }
  aside { overflow-y: auto; overflow-x: hidden; min-width: 0; background: #fafafb; }
  .center { min-width: 0; min-height: 0; padding: 0 8px; }
</style>
