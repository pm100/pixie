<script lang="ts">
  import { selectionLabel } from './selectionBar';

  let { count, onclear }: { count: number; onclear: () => void } = $props();
</script>

{#if count > 0}
  <div class="bar" role="status">
    <b>{selectionLabel(count)}</b>
    <button onclick={onclear}>Clear</button>
  </div>
{/if}

<style>
  .bar { position: absolute; left: 50%; transform: translateX(-50%); bottom: 14px; z-index: 3; display: flex; gap: 12px; align-items: center;
         background: #14303a; color: #fff; border-radius: 16px; padding: 6px 14px; box-shadow: 0 2px 10px rgba(0, 0, 0, .35); }
  b { font-weight: 600; }
  button { background: none; border: 0; color: #ffc83d; font: inherit; cursor: pointer; padding: 0; }
</style>
