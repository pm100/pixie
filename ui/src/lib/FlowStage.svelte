<script lang="ts">
  import type { Snippet } from 'svelte';

  // The full-window frame of the first-run / add-a-folder flow: Browse is not shown behind it.
  let { label, children }: { label: string; children: Snippet } = $props();
</script>

<div class="stage" role="dialog" aria-modal="true" aria-label={label}>
  <div class="card">{@render children()}</div>
</div>

<style>
  .stage { position: fixed; inset: 0; z-index: 1400; display: grid; place-items: center; overflow-y: auto; padding: 24px 16px; background: #f4f6fb; color: #111; }
  .card { width: min(640px, 100%); }
  .stage :global(h1) { margin: 0 0 4px; font-size: 22px; }
  .stage :global(.sub) { margin: 0 0 16px; color: #666; }
  .stage :global(.actions) { display: flex; justify-content: flex-end; gap: 8px; margin-top: 16px; }
  .stage :global(button) { font: inherit; font-size: 14px; padding: 6px 14px; border-radius: 6px; border: 1px solid #c3c8d4; background: #fff; color: #111; cursor: pointer; }
  .stage :global(button.primary) { background: #2f6fed; border-color: #2f6fed; color: #fff; }
  .stage :global(button:disabled) { opacity: .5; cursor: default; }
  .stage :global(.bar) { height: 8px; border-radius: 4px; background: #dde2ee; overflow: hidden; }
  .stage :global(.bar > div) { height: 100%; background: #2f6fed; transition: width .2s; }
</style>
