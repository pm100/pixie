<script lang="ts">
  /** The Email button for some photos (the details pane): it opens the email form in a dialog. The Tools tab shows the same
   *  form in its middle column instead. */
  import EmailForm from './EmailForm.svelte';

  let { ids, limitMb }: {
    ids: number[];
    /** The size limit, in megabytes (Settings, Email). */
    limitMb: number;
  } = $props();

  let open = $state(false);
  let note = $state('');
</script>

<div class="email">
  <button onclick={() => { note = ''; open = true; }} disabled={ids.length === 0} title="Open a new email with the photo or photos attached">Email</button>
  {#if note}<span class="note" role="status">{note}</span>{/if}
</div>

{#if open}
  <div class="scrim" role="presentation" onclick={() => (open = false)}></div>
  <div class="dlg" role="dialog" aria-modal="true" aria-label="Email">
    <EmailForm {ids} {limitMb} oncancel={() => (open = false)} onsent={(n) => { note = n; open = false; }} />
  </div>
{/if}

<style>
  .email { display: flex; flex-wrap: wrap; align-items: center; gap: 8px; }
  .note { font-size: 12px; color: #555; }
  .scrim { position: fixed; inset: 0; z-index: 2400; background: rgba(0, 0, 0, .35); }
  .dlg { position: fixed; z-index: 2500; left: 50%; top: 50%; transform: translate(-50%, -50%); width: min(440px, calc(100vw - 32px)); max-height: calc(100vh - 32px); overflow-y: auto; box-sizing: border-box;
         padding: 16px 18px 14px; border-radius: 10px; background: #fff; color: #222; box-shadow: 0 10px 40px rgba(0, 0, 0, .3); }
</style>
