<script lang="ts">
  import type { Snippet } from 'svelte';
  import type { TabId, TabInfo } from './tabs';

  let { tabs, active, onpick, status, children, helpOpen = false, onhelp, helpButton = $bindable(null) }: {
    tabs: TabInfo[]; active: TabId; onpick: (id: TabId) => void;
    /** The Ask Pixie card is open (the ? button shows it). */
    helpOpen?: boolean;
    /** The ? button was pressed. */
    onhelp: () => void;
    /** The ? button itself, so the card can hang from it. */
    helpButton?: HTMLElement | null;
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
    <button class="help" class:open={helpOpen} bind:this={helpButton} aria-label="Ask Pixie" aria-pressed={helpOpen} title="Ask Pixie: ask how to do something, or ask for a setting to be changed (F1)" onclick={onhelp}>?</button>
  </div>
  <div class="panel" role="tabpanel">
    <div class="main">{@render children()}</div>
  </div>
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
  button.help { width: 24px; height: 24px; padding: 0; margin: 0 0 3px 6px; border-radius: 50%; border: 1px solid #b8bdca; background: #fff; color: #2f6fed; font-weight: 700; cursor: pointer; align-self: center; }
  button.help:hover:not(.on) { background: #eef2fb; color: #2f6fed; }
  button.help.open { background: #2f6fed; border-color: #2f6fed; color: #fff; }
  button.on { background: #fff; color: #111; font-weight: 600; border-bottom-color: #fff; box-shadow: 0 2px 0 #2f6fed inset; }
  .panel { display: flex; flex-direction: row; flex: 1; min-height: 0; background: #fff; }
  /* The screen of the tab; the pages that cover it are placed against this box. */
  .main { position: relative; display: flex; flex-direction: column; flex: 1; min-width: 0; min-height: 0; }
</style>
