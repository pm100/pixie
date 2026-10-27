<script lang="ts">
  import type { Snippet } from 'svelte';
  import AppIcon from './AppIcon.svelte';

  // The dialog of the first-run / add-a-folder flow: a bordered window floating over the dimmed main window. Its
  // body scrolls inside it when tall; the footer (buttons) stays put. The main window never scrolls.
  // A wizard page gives a `title` (and a `sub`): they are laid out the same on every page, under Pixie's icon. Every screen of
  // the flow is the same size.
  let { label, children, footer, title, sub }: {
    label: string; children: Snippet; footer?: Snippet;
    title?: string; sub?: string;
  } = $props();
</script>

<div class="backdrop" role="presentation">
  <div class="dlg" role="dialog" aria-modal="true" aria-label={label}>
    <div class="body">
      {#if title}
        <header class="wz">
          <AppIcon size={64} />
          <h1>{title}</h1>
          {#if sub}<p class="sub">{sub}</p>{/if}
        </header>
      {/if}
      {@render children()}
    </div>
    {#if footer}<div class="foot">{@render footer()}</div>{/if}
  </div>
</div>

<style>
  .backdrop { position: fixed; inset: 0; z-index: 1400; display: grid; place-items: center; padding: 24px; background: rgba(15, 20, 35, .5); overflow: hidden; }
  .dlg {
    display: flex; flex-direction: column; width: min(640px, 100%); height: min(660px, calc(100vh - 48px));
    background: #fff; color: #111; border: 1px solid #b9bfce; border-radius: 10px; overflow: hidden;
    box-shadow: 0 14px 44px rgba(0, 0, 0, .4);
  }
  .body { flex: 1; min-height: 0; overflow-y: auto; padding: 22px 24px 10px; }
  .foot { display: flex; justify-content: flex-end; align-items: center; gap: 8px; padding: 12px 24px 16px; border-top: 1px solid #e6e9f2; background: #fafbfe; }
  .wz { text-align: center; margin: 0 0 14px; }
  .wz :global(svg) { display: block; margin: 0 auto 8px; filter: drop-shadow(0 3px 6px rgba(0, 0, 0, .2)); }
  .wz :global(.sub) { margin: 0; }
  .dlg :global(h1) { margin: 0 0 4px; font-size: 22px; }
  .dlg :global(.sub) { margin: 0 0 16px; color: #666; }
  .dlg :global(button) { font: inherit; font-size: 14px; padding: 6px 14px; border-radius: 6px; border: 1px solid #c3c8d4; background: #fff; color: #111; cursor: pointer; }
  .dlg :global(button.primary) { background: #2f6fed; border-color: #2f6fed; color: #fff; }
  .dlg :global(button:disabled) { opacity: .5; cursor: default; }
  .dlg :global(.bar) { height: 8px; border-radius: 4px; background: #dde2ee; overflow: hidden; }
  .dlg :global(.bar > div) { height: 100%; background: #2f6fed; transition: width .2s; }
</style>
