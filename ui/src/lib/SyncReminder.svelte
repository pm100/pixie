<script lang="ts">
  /** Shown once when Pixie starts, if items are new and not yet synced. */
  import { onMount } from 'svelte';
  import { plural } from './format';
  import { reminderLine } from './master';

  let { held, folder, copying, onreview, onsync, onclose }: {
    held: number; folder: string;
    /** A copy is already running: Sync now is not offered. */
    copying: boolean;
    onreview: (dontShow: boolean) => void; onsync: (dontShow: boolean) => void; onclose: (dontShow: boolean) => void;
  } = $props();

  let dontShow = $state(false);
  let reviewBtn: HTMLButtonElement;
  onMount(() => reviewBtn.focus());
</script>

<svelte:window onkeydown={(e) => { if (e.key === 'Escape') { e.stopPropagation(); onclose(dontShow); } }} />

<div class="scrim" role="presentation" onclick={(e) => { if (e.target === e.currentTarget) onclose(dontShow); }}>
  <div class="dlg" role="dialog" aria-modal="true" aria-labelledby="sr-title" aria-describedby="sr-msg">
    <h3 id="sr-title">You have new items to sync</h3>
    <p id="sr-msg">{reminderLine(held, folder)}</p>
    <div class="row">
      <button onclick={() => onclose(dontShow)} title="Close this and leave the new items waiting">Later</button>
      {#if !copying}
        <button onclick={() => onsync(dontShow)} title={`Copy all ${plural(held, 'new item')} into the sync folder now`}>Sync now</button>
      {/if}
      <button bind:this={reviewBtn} class="primary" onclick={() => onreview(dontShow)} title="Open Browse showing the new items, where you can mark some private before syncing">Review them</button>
    </div>
    {#if !copying}<p class="note">Sync now copies all {plural(held, 'new item')} into the sync folder.</p>{/if}
    <label class="dont" title="Do not show this reminder when Pixie starts. You can turn it on again in Import/Export, Sync">
      <input type="checkbox" bind:checked={dontShow} /> Don't show this again
    </label>
  </div>
</div>

<style>
  .scrim { position: fixed; inset: 0; z-index: 1500; display: grid; place-items: center; background: rgba(0, 0, 0, .4); }
  .dlg { width: min(460px, calc(100vw - 32px)); padding: 18px 20px; border-radius: 8px; background: #fff; color: #111; box-shadow: 0 8px 30px rgba(0, 0, 0, .35); }
  h3 { margin: 0 0 8px; font-size: 16px; }
  p { margin: 0 0 16px; color: #333; }
  .row { display: flex; justify-content: flex-end; gap: 8px; }
  .note { margin: 8px 0 0; font-size: 12px; color: #666; text-align: right; }
  .dont { display: flex; align-items: center; gap: 6px; margin-top: 12px; font-size: 13px; color: #444; }
</style>
