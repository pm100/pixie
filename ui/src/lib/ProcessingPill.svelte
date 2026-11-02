<script lang="ts">
  import type { Pill } from './activity';

  // The small indicator at the top right of the window while Pixie works in the background. It always says "Processing" (no
  // percentage and no bar: the screen it opens has the numbers) and is absent when nothing is running.
  let { pill, onclick }: { pill: Pill; onclick: () => void } = $props();
</script>

{#if pill}
  <button class="pill {pill}" title="Pixie is working in the background. Click to see what it is doing and how long it will take." aria-label="Processing" {onclick}>
    {#if pill === 'working'}<span class="spin" aria-hidden="true"></span>{:else if pill === 'error'}<span aria-hidden="true">⚠</span>{:else}<span aria-hidden="true">⏸</span>{/if}
    <span>Processing</span>
  </button>
{/if}

<style>
  .pill { display: inline-flex; align-items: center; gap: 7px; padding: 2px 10px; border: 1px solid #b8c6ee; border-radius: 12px; background: #eaf0ff; color: #1a3f9a; font: inherit; font-size: 12px; cursor: pointer; }
  .pill:hover { background: #dce6ff; }
  .pill.paused { border-color: #d8c58a; background: #fff6dc; color: #7a5b00; }
  .pill.error { border-color: #e3a3a8; background: #fdecea; color: #b00020; }
  .spin { width: 11px; height: 11px; border: 2px solid #b8c6ee; border-top-color: #2f6fed; border-radius: 50%; animation: spin .9s linear infinite; }
  @keyframes spin { to { transform: rotate(360deg); } }
  @media (prefers-reduced-motion: reduce) { .spin { animation-duration: 4s; } }
</style>
