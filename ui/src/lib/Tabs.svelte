<script lang="ts">
  import type { Snippet } from 'svelte';
  import type { TabId, TabInfo } from './tabs';

  let { tabs, active, onpick, status, children }: {
    tabs: TabInfo[]; active: TabId; onpick: (id: TabId) => void;
    /** Shown at the right of the title row. */
    status: Snippet;
    /** The screen of the active tab: every tab swaps this one framed area. */
    children: Snippet;
  } = $props();
</script>

<div class="frame">
  <div class="strip">
    <div class="tabs" role="tablist" aria-label="Sections">
      {#each tabs as t (t.id)}
        <button role="tab" aria-selected={active === t.id} class:on={active === t.id} onclick={() => onpick(t.id)}>{t.label}</button>
      {/each}
    </div>
    <span class="sp"></span>
    <span class="status">{@render status()}</span>
    <!-- A placeholder for the help panel (Ask Pixie will open from here); it does nothing yet. -->
    <button class="help" aria-disabled="true" title="Help: Ask Pixie will open here (coming soon)" tabindex="-1">?</button>
  </div>
  <div class="panel" role="tabpanel">{@render children()}</div>
</div>

<style>
  .frame { display: flex; flex-direction: column; flex: 1; min-height: 0; background: #e9ecf2; }
  .sp { flex: 1; }
  .tabs { display: flex; align-items: flex-end; gap: 2px; }
  .status { display: flex; align-items: center; gap: 10px; align-self: center; padding-bottom: 2px; font-size: 12px; }
  /* The strip's bottom line is the top edge of the panel; the active tab sits over it and is open at the bottom. */
  .strip { display: flex; align-items: flex-end; gap: 2px; padding: 6px 12px 0; border-bottom: 1px solid #c3c8d4; }
  button { position: relative; margin-bottom: -1px; padding: 3px 14px; border: 1px solid #c3c8d4; border-bottom-color: #c3c8d4; border-radius: 6px 6px 0 0; background: #dde1ea; color: #555; font: inherit; font-size: 13px; line-height: 1.3; cursor: pointer; }
  button:hover:not(.on) { background: #e8ebf2; color: #222; }
  button.help { width: 24px; height: 24px; padding: 0; margin: 0 0 3px 6px; border-radius: 50%; border: 1px solid #b8bdca; background: #fff; color: #9aa7c7; font-weight: 700; cursor: default; align-self: center; }
  button.help:hover:not(.on) { background: #fff; color: #9aa7c7; }
  button.on { background: #fff; color: #111; font-weight: 600; border-bottom-color: #fff; box-shadow: 0 2px 0 #2f6fed inset; }
  .panel { position: relative; display: flex; flex-direction: column; flex: 1; min-height: 0; background: #fff; }
</style>
