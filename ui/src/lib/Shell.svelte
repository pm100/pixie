<script lang="ts">
  import type { Snippet } from 'svelte';
  import { loadPref, savePref } from './prefs';
  import Resizer from './Resizer.svelte';

  let { left, center, right }: { left: Snippet; center: Snippet; right: Snippet } = $props();

  let leftW = $state(loadPref('pane.left', 230));
  let rightW = $state(loadPref('pane.right', 280));
</script>

<div class="shell" style:grid-template-columns="{leftW}px 5px minmax(0, 1fr) 5px {rightW}px">
  <aside>{@render left()}</aside>
  <Resizer bind:width={leftW} min={160} max={480} side="left" onchange={(w) => savePref('pane.left', w)} />
  <div class="center">{@render center()}</div>
  <Resizer bind:width={rightW} min={200} max={520} side="right" onchange={(w) => savePref('pane.right', w)} />
  <aside>{@render right()}</aside>
</div>

<style>
  .shell { display: grid; height: 100%; min-height: 0; }
  aside { overflow-y: auto; min-width: 0; background: #fafafb; }
  .center { min-width: 0; min-height: 0; padding: 0 8px; }
</style>
