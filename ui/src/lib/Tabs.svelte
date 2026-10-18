<script lang="ts">
  import type { Snippet } from 'svelte';
  import type { TabId, TabInfo } from './tabs';

  let { tabs, active, onpick, status }: { tabs: TabInfo[]; active: TabId; onpick: (id: TabId) => void; status: Snippet } = $props();
</script>

<div class="bar" role="tablist" aria-label="Sections">
  <strong class="brand">Pixie</strong>
  {#each tabs as t (t.id)}
    <button role="tab" aria-selected={active === t.id} class:on={active === t.id} onclick={() => onpick(t.id)}>{t.label}</button>
  {/each}
  <span class="sp"></span>
  {@render status()}
</div>

<style>
  .bar { display: flex; align-items: center; gap: 4px; padding: 0 14px; border-bottom: 1px solid #ddd; background: #fafafb; min-height: 42px; }
  .brand { margin-right: 14px; }
  button { border: 0; background: none; font: inherit; padding: 11px 14px 9px; border-bottom: 3px solid transparent; cursor: pointer; color: #444; }
  button.on { border-bottom-color: #2f6fed; color: #111; font-weight: 600; }
  button:hover:not(.on) { background: #f0f2f6; }
  .sp { flex: 1; }
</style>
