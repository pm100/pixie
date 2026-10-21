<script lang="ts">
  import type { Snippet } from 'svelte';

  // The dialog of the first-run / add-a-folder flow: a bordered window floating over the dimmed main window. Its
  // body scrolls inside it when tall; the footer (buttons) stays put. The main window never scrolls.
  let { label, children, footer }: { label: string; children: Snippet; footer?: Snippet } = $props();
</script>

<div class="backdrop" role="presentation">
  <div class="dlg" role="dialog" aria-modal="true" aria-label={label}>
    <div class="body">{@render children()}</div>
    {#if footer}<div class="foot">{@render footer()}</div>{/if}
  </div>
</div>

<style>
  .backdrop { position: fixed; inset: 0; z-index: 1400; display: grid; place-items: center; padding: 24px; background: rgba(15, 20, 35, .5); overflow: hidden; }
  .dlg {
    display: flex; flex-direction: column; width: min(640px, 100%); max-height: calc(100vh - 48px);
    background: #fff; color: #111; border: 1px solid #b9bfce; border-radius: 10px; overflow: hidden;
    box-shadow: 0 14px 44px rgba(0, 0, 0, .4);
  }
  .body { flex: 1; min-height: 0; overflow-y: auto; padding: 22px 24px 10px; }
  .foot { display: flex; justify-content: flex-end; gap: 8px; padding: 12px 24px 16px; border-top: 1px solid #e6e9f2; background: #fafbfe; }
  .dlg :global(h1) { margin: 0 0 4px; font-size: 22px; }
  .dlg :global(.sub) { margin: 0 0 16px; color: #666; }
  .dlg :global(button) { font: inherit; font-size: 14px; padding: 6px 14px; border-radius: 6px; border: 1px solid #c3c8d4; background: #fff; color: #111; cursor: pointer; }
  .dlg :global(button.primary) { background: #2f6fed; border-color: #2f6fed; color: #fff; }
  .dlg :global(button:disabled) { opacity: .5; cursor: default; }
  .dlg :global(.bar) { height: 8px; border-radius: 4px; background: #dde2ee; overflow: hidden; }
  .dlg :global(.bar > div) { height: 100%; background: #2f6fed; transition: width .2s; }
</style>
